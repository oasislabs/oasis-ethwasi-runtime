// Package transfer implementes the synthetic transfer benchmark.
package transfer

import (
	"context"
	"crypto/ecdsa"
	"crypto/rand"
	"math/big"
	"strings"
	"sync"

	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/core/types"
	"github.com/ethereum/go-ethereum/crypto"
	"github.com/ethereum/go-ethereum/ethclient"
	"github.com/go-kit/kit/log/level"
	"github.com/spf13/cobra"

	"github.com/oasislabs/runtime-ethereum/benchmark/benchmarks/api"
)

var (
	gasPrice       = big.NewInt(1000000000)
	transferAmount = big.NewInt(1)
	fundAmount     = big.NewInt(100000000000000000)
)

const gasLimit = 1000000

type benchTransfer struct {
	fundingAccount *transferAccount
}

type transferAccount struct {
	client *ethclient.Client

	privateKey *ecdsa.PrivateKey
	nonce      uint64
}

func (account *transferAccount) newTransfer(nonce uint64, dst common.Address, amount *big.Int) (*types.Transaction, error) {
	tx := types.NewTransaction(nonce, dst, amount, gasLimit, gasPrice, nil)
	return types.SignTx(tx, types.HomesteadSigner{}, account.privateKey)
}

func (bench *benchTransfer) Name() string {
	return "transfer"
}

func (bench *benchTransfer) Prepare(ctx context.Context, state *api.State) error {
	privKey, err := crypto.GenerateKey()
	if err != nil {
		return err
	}
	state.State = &transferAccount{
		client:     ethclient.NewClient(state.RPCClient),
		privateKey: privKey,
	}

	return nil
}

func (bench *benchTransfer) BulkPrepare(ctx context.Context, states []*api.State) error {
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

func (bench *benchTransfer) Cleanup(state *api.State) {
	account := (state.State).(*transferAccount)
	if account.client != nil {
		account.client.Close()
	}
}

func privKeyToAddress(privKey *ecdsa.PrivateKey) common.Address {
	pubKey := (privKey.Public()).(*ecdsa.PublicKey)
	return crypto.PubkeyToAddress(*pubKey)
}

// Init initializes and registers the synthetic transfer benchmark.
func Init(cmd *cobra.Command) {
	// TODO: This probably shouldn't be hardcoded, but just initialize
	// the funding account here for now.
	const (
		fundingAccount = "533d62aea9bbcb821dfdda14966bb01bfbbb53b7e9f5f0d69b8326e052e3450c"
		fundingAddress = "0x7110316b618d20d0c44728ac2a3d683536ea682b"
	)

	privKey, err := crypto.HexToECDSA(fundingAccount)
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
