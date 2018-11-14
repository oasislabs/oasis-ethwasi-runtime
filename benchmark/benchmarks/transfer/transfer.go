// Package transfer implementes the synthetic transfer benchmark.
package transfer

import (
	"context"
	"crypto/ecdsa"
	"crypto/rand"
	"fmt"
	"math/big"
	"net/http"
	"net/url"
	"strings"
	"sync"

	"github.com/ethereum/go-ethereum"
	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/core/types"
	"github.com/ethereum/go-ethereum/crypto"
	"github.com/ethereum/go-ethereum/ethclient"
	"github.com/go-kit/kit/log/level"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
	"golang.org/x/net/context/ctxhttp"

	"github.com/oasislabs/runtime-ethereum/benchmark/benchmarks/api"
)

const (
	// FundingAccount is the account in the development environment
	// genesis block that has a large amount of funds.
	FundingAccount = "533d62aea9bbcb821dfdda14966bb01bfbbb53b7e9f5f0d69b8326e052e3450c"

	cfgFaucetURL    = "benchmarks.transfer.faucet_url"
	cfgWatchNewHead = "benchmarks.transfer.watch_new_head"

	transferCost = 21000 // Simple transfers always cost this much gas.
)

var (
	flagFaucetURL    string
	flagWatchNewHead bool

	gasPrice       = big.NewInt(1000000000)
	transferAmount = big.NewInt(1)
	fundAmount     = big.NewInt(100000000000000000)
)

type benchTransfer struct {
	fundingAccount *transferAccount
}

type transferAccount struct {
	client *ethclient.Client

	newHeads *newHeadWatcher

	privateKey *ecdsa.PrivateKey
	nonce      uint64
}

func (account *transferAccount) newTransfer(nonce uint64, dst common.Address, amount *big.Int) (*types.Transaction, error) {
	tx := types.NewTransaction(nonce, dst, amount, transferCost, gasPrice, nil)
	return types.SignTx(tx, types.HomesteadSigner{}, account.privateKey)
}

func (account *transferAccount) drainTo(ctx context.Context, dst common.Address) error {
	transferFee := big.NewInt(transferCost)
	transferFee.Mul(transferFee, gasPrice)

	// Query the account's balance.
	accountAddr := privKeyToAddress(account.privateKey)
	balance, err := account.client.BalanceAt(ctx, accountAddr, nil)
	if err != nil {
		return err
	}
	if balance.Cmp(&big.Int{}) <= 0 {
		// Account balance is <= 0 already.
		return nil
	}

	// Transfer off the remaining balance back to the funding account.
	balance.Sub(balance, transferFee)
	if balance.Cmp(&big.Int{}) <= 0 {
		return fmt.Errorf("insufficient balance to transfer: %v", balance)
	}
	tx, err := account.newTransfer(account.nonce, dst, balance)
	if err != nil {
		return err
	}
	if err = account.client.SendTransaction(ctx, tx); err != nil {
		return err
	}

	// Query the account's final balance, ensure it is zero.
	balance, err = account.client.BalanceAt(ctx, accountAddr, nil)
	if err != nil {
		return err
	}
	if balance.Cmp(&big.Int{}) != 0 {
		return fmt.Errorf("transfer: non-zero final balance: %v", balance)
	}
	return nil
}

type newHeadWatcher struct {
	sync.WaitGroup

	state *api.State

	sub ethereum.Subscription
	ch  chan *types.Header
}

func (w *newHeadWatcher) Stop() {
	w.sub.Unsubscribe()
	w.Wait()
}

func (w *newHeadWatcher) worker() {
	defer w.Done()

	for {
		select {
		case err, ok := <-w.sub.Err():
			if ok {
				_ = level.Error(w.state.Logger).Log("msg", "failed to receive newHead",
					"err", err,
				)
			}
			return
		case hdr := <-w.ch:
			if w.state.Config.LogVerboseDebug {
				_ = level.Debug(w.state.Logger).Log("msg", "newHead received from subscription",
					"header", hdr,
				)
			}
		}
	}
}

func watchNewHeads(ctx context.Context, state *api.State, client *ethclient.Client) (*newHeadWatcher, error) {
	var (
		watcher = newHeadWatcher{
			state: state,
			ch:    make(chan *types.Header),
		}
		err error
	)
	if watcher.sub, err = client.SubscribeNewHead(ctx, watcher.ch); err != nil {
		return nil, err
	}

	watcher.Add(1)
	go watcher.worker()

	return &watcher, nil
}

func (bench *benchTransfer) Name() string {
	return "transfer"
}

func (bench *benchTransfer) Prepare(ctx context.Context, state *api.State) error {
	privKey, err := crypto.GenerateKey()
	if err != nil {
		return err
	}

	account := &transferAccount{
		client:     ethclient.NewClient(state.RPCClient),
		privateKey: privKey,
	}

	if flagWatchNewHead {
		if account.newHeads, err = watchNewHeads(ctx, state, account.client); err != nil {
			account.client.Close()
			return err
		}
	}

	state.State = account

	return nil
}

func (bench *benchTransfer) BulkPrepare(ctx context.Context, states []*api.State) error {
	// Ensure that there is sufficeint balance in the funding account.
	if err := bench.ensureMinBalance(ctx, states); err != nil {
		return err
	}

	// Generate and sign all of the initial funding transactions.
	txs := make([]*types.Transaction, 0, len(states))
	for i := 0; i < len(states); i++ {
		account := (states[i].State).(*transferAccount)

		addr := privKeyToAddress(account.privateKey)
		tx, err := bench.fundingAccount.newTransfer(bench.fundingAccount.nonce, addr, fundAmount)
		if err != nil {
			_ = level.Error(states[i].Logger).Log("msg", "failed to create/sign transaction",
				"err", err,
			)
			return err
		}

		bench.fundingAccount.nonce++

		txs = append(txs, tx)
	}

	// Use each state's ethclient instance to dispatch all of the initial funding
	// requests concurrently.
	var wg sync.WaitGroup
	errCh := make(chan error, len(states))
	for i := 0; i < len(states); i++ {
		wg.Add(1)
		go func(idx int) {
			defer wg.Done()

			account := (states[idx].State).(*transferAccount)
			if err := account.client.SendTransaction(ctx, txs[idx]); err != nil {
				_ = level.Error(states[idx].Logger).Log("msg", "failed to fund account",
					"err", err,
				)
				errCh <- err
				return
			}

			_ = level.Info(states[idx].Logger).Log("msg", "funded account")
		}(i)
	}
	wg.Wait()
	select {
	case err := <-errCh:
		return err
	default:
		return nil
	}
}

func (bench *benchTransfer) Scenario(ctx context.Context, state *api.State) (uint64, error) {
	account := (state.State).(*transferAccount)

	var recipient common.Address
	if _, err := rand.Read(recipient[:]); err != nil {
		return 0, err
	}

	tx, err := account.newTransfer(account.nonce, recipient, transferAmount)
	if err != nil {
		return 0, err
	}
	if err = account.client.SendTransaction(ctx, tx); err != nil {
		return 0, err
	}

	account.nonce++
	return 1, nil
}

func (bench *benchTransfer) BulkCleanup(ctx context.Context, states []*api.State) {
	fundingAccountAddr := privKeyToAddress(bench.fundingAccount.privateKey)

	var wg sync.WaitGroup
	for i := 0; i < len(states); i++ {
		wg.Add(1)
		go func(idx int) {
			defer wg.Done()

			if states[idx].State == nil {
				return
			}
			account := (states[idx].State).(*transferAccount)

			defer func() {
				if account.newHeads != nil {
					account.newHeads.Stop()
				}
				if account.client != nil {
					account.client.Close()
				}
			}()

			if err := account.drainTo(ctx, fundingAccountAddr); err != nil {
				_ = level.Error(states[idx].Logger).Log("msg", "failed to drain balance",
					"err", err,
				)
			}
		}(i)
	}

	wg.Wait()
}

func (bench *benchTransfer) ensureMinBalance(ctx context.Context, states []*api.State) error {
	// Work out the balance required to fund all the accounts, including
	// transaction fees.
	txFees := big.NewInt(transferCost)
	minBalance := big.NewInt(int64(len(states)))
	txFees.Mul(txFees, gasPrice)
	txFees.Mul(txFees, minBalance)
	minBalance.Mul(fundAmount, minBalance)
	minBalance.Add(minBalance, txFees)

	logger := states[0].Logger
	client := (states[0].State).(*transferAccount).client
	fundingAccountAddr := privKeyToAddress(bench.fundingAccount.privateKey)
	balance, err := client.BalanceAt(ctx, fundingAccountAddr, nil)
	if err != nil {
		return err
	}

	_ = level.Debug(logger).Log("msg", "funding account balance",
		"balance", balance,
		"required_balance", &minBalance,
	)

	// Sufficient balance is present in the account.
	if balance.Cmp(minBalance) > 0 {
		return nil
	}

	// Hit up the faucet's private endpoint for more money.
	if flagFaucetURL == "" {
		return fmt.Errorf("insufficient funds, no faucet configured")
	}
	u, err := url.Parse(flagFaucetURL)
	if err != nil {
		return fmt.Errorf("invalid faucet URL: %v", err)
	}
	q := u.Query()
	q.Set("to", fundingAccountAddr.Hex())
	q.Set("amnt", minBalance.String())
	u.RawQuery = q.Encode()

	_ = level.Debug(logger).Log("msg", "requesting funding from faucet",
		"url", u,
	)

	resp, err := ctxhttp.Get(ctx, nil, u.String())
	if err != nil {
		return fmt.Errorf("failed to query faucet: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("faucet failed funding: %v", resp.StatusCode)
	}

	return nil
}

func privKeyToAddress(privKey *ecdsa.PrivateKey) common.Address {
	pubKey := (privKey.Public()).(*ecdsa.PublicKey)
	return crypto.PubkeyToAddress(*pubKey)
}

// Init initializes and registers the synthetic transfer benchmark.
func Init(cmd *cobra.Command) {
	cmd.Flags().StringVar(&flagFaucetURL, cfgFaucetURL, "", "Faucet private endpoint URL")
	cmd.Flags().BoolVar(&flagWatchNewHead, cfgWatchNewHead, false, "Subscribe for `newHeads` events")

	for _, v := range []string{
		cfgFaucetURL,
		cfgWatchNewHead,
	} {
		viper.BindPFlag(v, cmd.Flags().Lookup(v)) // nolint: errcheck
	}

	const fundingAddress = "0x7110316b618d20d0c44728ac2a3d683536ea682b"
	privKey, err := crypto.HexToECDSA(FundingAccount)
	if err != nil {
		panic(err)
	}

	// Sanity check the addresss.
	addr := privKeyToAddress(privKey)
	if strings.ToLower(addr.Hex()) != fundingAddress {
		panic("funding address does not match funding private key")
	}

	api.RegisterBenchmark(&benchTransfer{
		fundingAccount: &transferAccount{
			privateKey: privKey,
		},
	})
}
