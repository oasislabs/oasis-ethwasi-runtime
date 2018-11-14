package cmd

import (
	"context"
	"crypto/ecdsa"
	"encoding/hex"
	"fmt"
	"io"
	"math/big"
	"net/http"
	"net/url"
	"os"
	"strings"
	"sync/atomic"

	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/core/types"
	"github.com/ethereum/go-ethereum/crypto"
	"github.com/ethereum/go-ethereum/ethclient"
	"github.com/ethereum/go-ethereum/rpc"
	"github.com/go-kit/kit/log"
	"github.com/go-kit/kit/log/level"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"

	"github.com/oasislabs/runtime-ethereum/benchmark/benchmarks/transfer"
)

const (
	cfgFaucetAddress = "faucet.address"
)

var (
	faucetCmd = &cobra.Command{
		Use:   "faucet",
		Short: "Lite private faucet",
		Run:   faucetMain,
	}

	flagFaucetAddress string

	gasPrice = big.NewInt(1000000000)
)

func faucetMain(cmd *cobra.Command, args []string) {
	w, err := initLogFile(cmd)
	if err != nil {
		fmt.Fprintf(os.Stderr, "failed to initialize log file: %v", err)
		os.Exit(1)
	}
	logger := flagLogLevel.GetLogger(w)

	if err = runFaucetWorker(logger); err != nil {
		_ = level.Error(logger).Log("msg", "faucet terminated",
			"err", err,
		)
	}
}

type faucetWorker struct {
	logger log.Logger

	rpcClient *rpc.Client
	client    *ethclient.Client

	privateKey *ecdsa.PrivateKey
	nonce      uint64
}

func (w *faucetWorker) handleRequest(resp http.ResponseWriter, req *http.Request) {
	sendErrorResponse := func(statusCode int, err error) {
		resp.WriteHeader(statusCode)
		_, _ = io.WriteString(resp, "err")
	}

	toStr := req.URL.Query().Get("to")
	to, err := parseHexAddress(toStr)
	if err != nil {
		_ = level.Warn(w.logger).Log("msg", "failed to parse to",
			"to", toStr,
		)
		sendErrorResponse(http.StatusBadRequest, err)
		return
	}

	amntStr := req.URL.Query().Get("amnt")
	var amnt big.Int
	if _, ok := amnt.SetString(amntStr, 0); !ok {
		_ = level.Warn(w.logger).Log("msg", "failed to parse amnt",
			"amnt", amntStr,
		)
		sendErrorResponse(http.StatusBadRequest, fmt.Errorf("malformed amount"))
		return
	}
	if amnt.Cmp(&big.Int{}) <= 0 {
		_ = level.Warn(w.logger).Log("msg", "requested amount is <= 0",
			"amnt", amntStr,
		)
		sendErrorResponse(http.StatusBadRequest, fmt.Errorf("invalid amount"))
		return
	}

	nonce := atomic.AddUint64(&w.nonce, 1)
	tx := types.NewTransaction(nonce, to, &amnt, 21000, gasPrice, nil)
	signedTx, err := types.SignTx(tx, types.HomesteadSigner{}, w.privateKey)
	if err != nil {
		_ = level.Warn(w.logger).Log("msg", "failed to sign transaction",
			"err", err,
		)
		sendErrorResponse(http.StatusInternalServerError, err)
		return
	}

	if err = w.client.SendTransaction(context.Background(), signedTx); err != nil {
		_ = level.Warn(w.logger).Log("msg", "failed to send transaction",
			"err", err,
		)
		sendErrorResponse(http.StatusInternalServerError, err)
		return
	}

	_ = level.Info(w.logger).Log("msg", "private wallet funded",
		"account", toStr,
		"amount", amntStr,
	)

	resp.WriteHeader(http.StatusOK)
}

func parseHexAddress(s string) (common.Address, error) {
	s = strings.TrimPrefix(strings.ToLower(s), "0x")
	b, err := hex.DecodeString(s)
	if err != nil {
		return common.Address{}, fmt.Errorf("failed to decode address: %v", err)
	}
	if len(b) != common.AddressLength {
		return common.Address{}, fmt.Errorf("malformed address")
	}

	return common.BytesToAddress(b), nil
}

func runFaucetWorker(logger log.Logger) error {
	if flagFaucetAddress == "" {
		return fmt.Errorf("no faucet address specified")
	}
	if flagGatewayURL == "" {
		return fmt.Errorf("no gateway URL specified")
	}

	w := &faucetWorker{
		logger: log.With(logger, "module", "worker"),
	}

	gatewayURL, err := url.Parse(flagGatewayURL)
	if err != nil {
		return fmt.Errorf("failed to parse gateway URL: %v", err)
	}
	w.rpcClient, err = rpc.Dial(gatewayURL.String())
	if err != nil {
		return fmt.Errorf("failed to connect to gateway: %v", err)
	}
	w.client = ethclient.NewClient(w.rpcClient)
	w.privateKey, err = crypto.HexToECDSA(transfer.FundingAccount)
	if err != nil {
		return fmt.Errorf("failed to decode funding address: %v", err)
	}

	_ = level.Info(w.logger).Log("msg", "initialized",
		"gateway_url", gatewayURL,
		"faucet_address", flagFaucetAddress,
	)

	http.HandleFunc("/", w.handleRequest)

	return http.ListenAndServe(flagFaucetAddress, nil)
}

func faucetInit(debugCmd *cobra.Command) {
	cmd := faucetCmd
	cmd.Flags().StringVar(&flagFaucetAddress, cfgFaucetAddress, ":8080", "Lite faucet address")
	cmd.Flags().StringVar(&flagGatewayURL, cfgGatewayURL, defaultGatewayURL, "JSON-RPC gateway URL")

	for _, v := range []string{
		cfgFaucetAddress,
		cfgGatewayURL,
	} {
		_ = viper.BindPFlag(v, cmd.Flags().Lookup(v))
	}

	debugCmd.AddCommand(cmd)
}
