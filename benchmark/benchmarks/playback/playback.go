// Package playback implements the Ethereum transaction playback benchmarks.
package playback

import (
	"context"
	"encoding/hex"
	"io"
	"os"

	"github.com/ethereum/go-ethereum/rlp"
	"github.com/go-kit/kit/log/level"
	"github.com/pkg/errors"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"

	"github.com/oasislabs/runtime-ethereum/benchmark/benchmarks/api"
)

const (
	cfgDataset = "benchmarks.playback.dataset"
	cfgNumTxns = "benchmarks.playback.transactions"
)

var (
	flagDataset string
	flagNumTxns int
)

type benchPlayback struct {
}

func (bench *benchPlayback) Name() string {
	return "playback"
}

func (bench *benchPlayback) BulkPrepare(ctx context.Context, states []*api.State) error {
	if flagDataset == "" {
		return errors.New("dataset filename not specified")
	}

	logger := states[0].Config.Logger

	// Load dataset and preprocess up to the specified number of transactions.
	dataset, err := os.Open(flagDataset)
	if err != nil {
		return errors.Wrap(err, "failed to open dataset")
	}
	defer dataset.Close()

	_ = level.Info(logger).Log("msg", "parsing dataset")
	txns := make([]string, 0, flagNumTxns)
	stream := rlp.NewStream(dataset, 0)
	// Blocks are written one after another into the exported blocks file.
	// https://github.com/paritytech/parity/blob/v1.9.7/parity/blockchain.rs#L595
BlockLoop:
	for {
		// Each block is a 3-list of (header, transactions, uncles).
		// https://github.com/paritytech/parity/blob/v1.9.7/ethcore/src/encoded.rs#L188
		if _, err := stream.List(); err != nil {
			if err == io.EOF {
				break
			}
			return errors.Wrap(err, "unable to parse dataset")
		}

		// Skip header.
		_, _ = stream.Raw()

		// Read transaction list.
		if _, err := stream.List(); err != nil {
			return errors.Wrap(err, "unable to parse transaction list")
		}

		for {
			if len(txns) >= flagNumTxns {
				break BlockLoop
			}

			// Read transaction.
			txn, err := stream.Raw()
			if err == rlp.EOL {
				break
			} else if err != nil {
				return errors.Wrap(err, "unable to parse transaction")
			}

			txns = append(txns, "0x"+hex.EncodeToString(txn))
		}

		// End of transaction list.
		if err := stream.ListEnd(); err != nil {
			return errors.Wrap(err, "unable to parse transaction list")
		}

		// Skip uncles.
		_, _ = stream.Raw()

		// End of block.
		if err := stream.ListEnd(); err != nil {
			return errors.Wrap(err, "unable to parse dataset")
		}
	}

	_ = level.Info(logger).Log("msg", "loaded transactions from dataset",
		"num_txns", len(txns),
	)

	// Distribute transactions among the goroutines.
	for i, txn := range txns {
		s := states[i%len(states)]

		var state []string
		if s.State == nil {
			state = make([]string, 0)
		} else {
			state = (s.State).([]string)
		}

		state = append(state, txn)
		s.State = state
	}

	return nil
}

func (bench *benchPlayback) Scenario(ctx context.Context, state *api.State) (uint64, error) {
	txns := state.State.([]string)
	if len(txns) == 0 {
		// No more transactions. We return an error as this situation may invalidate the
		// benchmark results. The correct remedy is to add more transactions.
		return 0, errors.New("exhausted all transactions, add more transactions")
	}

	txn, txns := txns[0], txns[1:]
	state.State = txns

	// Submit raw transaction.
	// TODO: Currently we just ignore errors, should we count them?
	_ = state.RPCClient.CallContext(ctx, nil, "eth_sendRawTransaction", txn)

	return 1, nil
}

func (bench *benchPlayback) Cleanup(state *api.State) {
}

// Init initializes and registers the benchmark suites.
func Init(cmd *cobra.Command) {
	cmd.Flags().StringVar(&flagDataset, cfgDataset, "", "Playback dataset file (binary block dump from Parity)")
	cmd.Flags().IntVar(&flagNumTxns, cfgNumTxns, 10000, "Number of transactions to replay (0 = all)")

	for _, v := range []string{
		cfgDataset,
		cfgNumTxns,
	} {
		viper.BindPFlag(v, cmd.Flags().Lookup(v)) // nolint: errcheck
	}

	api.RegisterBenchmark(&benchPlayback{})
}
