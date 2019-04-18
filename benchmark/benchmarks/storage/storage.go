// Package storage contains 4 benchmarks calling the Readwrite smart contract
// to benchmark the storage backend under the following scenarios:
// - storageSeqWrites  - writes 1KB block to 100 sequential locations 1000-times
// - storageSeqReads   - writes 1KB block to 100 sequential locations and reads
//                       them 1000-times
// - storageRandWrites - writes 1KB block to 100 pseudo-random locations 1000-
//                       times
// - storageRandReads  - writes 1KB block to 100 pseudo-random locations and
//                       reads them 1000-times

package storage

import (
	"context"
	"crypto/ecdsa"
	"math/big"

	"github.com/ethereum/go-ethereum/accounts/abi/bind"
	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/crypto"
	"github.com/ethereum/go-ethereum/ethclient"
	"github.com/go-kit/kit/log/level"
	"github.com/spf13/cobra"

	"github.com/oasislabs/runtime-ethereum/benchmark/benchmarks/api"
	"github.com/oasislabs/runtime-ethereum/benchmark/contracts/readwrite"
)

var (
	gasLimit = uint64(100000)
	randSeed = uint64(0x132A20CE0B5776A1) // some 64-bit prime number

	flagChunkSize      uint64
	flagNumLocations   uint64
	flagNumRepetitions uint64
)

const (
	cfgChunkSize      = "benchmarks.storage.chunkSize"
	cfgNumLocations   = "benchmarks.storage.numLocations"
	cfgNumRepetitions = "benchmarks.storage.numRepetitions"
	fundingAccount    = "533d62aea9bbcb821dfdda14966bb01bfbbb53b7e9f5f0d69b8326e052e3450c"
)

type benchStorageSeqWrites struct {
	auth             bind.TransactOpts
	contractInstance map[uint64]readwrite.Readwrite
}

func prepareReadWriteContract(ctx context.Context, state *api.State) (*bind.TransactOpts, *common.Address, *readwrite.Readwrite, error) {
	client := ethclient.NewClient(state.RPCClient)
	state.State = client

	privKey, err := crypto.HexToECDSA(fundingAccount)
	if err != nil {
		panic(err)
	}

	publicKey := privKey.Public()
	publicKeyECDSA, ok := publicKey.(*ecdsa.PublicKey)
	if !ok {
		_ = level.Error(state.Logger).Log("msg", "Error while obtaining publicKey from privateKey.")
		return nil, nil, nil, nil
	}

	fromAddress := crypto.PubkeyToAddress(*publicKeyECDSA)
	nonce, err := client.PendingNonceAt(context.Background(), fromAddress)
	if err != nil {
		_ = level.Error(state.Logger).Log("msg", "Error while computing nonce.", "err", err)
		return nil, nil, nil, err
	}

	gasPrice, err := client.SuggestGasPrice(context.Background())
	if err != nil {
		_ = level.Error(state.Logger).Log("msg", "Error while suggesting gas price.", "err", err)
		return nil, nil, nil, err
	}

	auth := *bind.NewKeyedTransactor(privKey)
	auth.Nonce = big.NewInt(int64(nonce))
	auth.Value = big.NewInt(0) // in wei
	auth.GasLimit = gasLimit
	auth.GasPrice = gasPrice

	address, _, instance, err := readwrite.DeployReadwrite(&auth, client)
	if err != nil {
		_ = level.Error(state.Logger).Log("msg", "Error while deploying Readwrite contract.", "err", err)
		return nil, nil, nil, err
	}

	return &auth, &address, instance, nil
}

func (bs *benchStorageSeqWrites) Name() string {
	return "storageSeqWrites"
}

func (bs *benchStorageSeqWrites) Prepare(ctx context.Context, state *api.State) error {
	auth, _, contractInstance, err := prepareReadWriteContract(ctx, state)
	if err != nil {
		_ = level.Error(state.Logger).Log("msg", "Readwrite contract not deployed successfully :(", "err", err)
		return err
	}

	bs.auth = *auth
	bs.contractInstance[state.Id] = *contractInstance

	return nil
}

func (bs *benchStorageSeqWrites) Scenario(ctx context.Context, state *api.State) (uint64, error) {
	ci := bs.contractInstance[state.Id]

	// Write 1KB block of zeros to locations 1...100, and repeat 1000-times
	if _, err := ci.WriteSeq(&bs.auth, make([]byte, flagChunkSize), flagNumLocations, flagNumRepetitions); err != nil {
		_ = level.Error(state.Logger).Log("msg", "Error while calling WriteExpSeq.", "err", err)
		return 0, err
	}

	return 1, nil
}

func (bs *benchStorageSeqWrites) Cleanup(state *api.State) {
	client := state.State.(*ethclient.Client)
	client.Close()
}

type benchStorageSeqReads struct {
	auth             bind.TransactOpts
	contractInstance map[uint64]readwrite.Readwrite
}

func (bs *benchStorageSeqReads) Name() string {
	return "storageSeqReads"
}

func (bs *benchStorageSeqReads) Prepare(ctx context.Context, state *api.State) error {
	auth, _, contractInstance, err := prepareReadWriteContract(ctx, state)
	if err != nil {
		_ = level.Error(state.Logger).Log("msg", "Readwrite contract not deployed successfully :(", "err", err)
		return err
	}

	bs.auth = *auth
	bs.contractInstance[state.Id] = *contractInstance

	return nil
}

func (bs *benchStorageSeqReads) Scenario(ctx context.Context, state *api.State) (uint64, error) {
	ci := bs.contractInstance[state.Id]

	// Write 1KB block of zeros to locations 1...100
	if _, err := ci.WriteSeq(&bs.auth, make([]byte, flagChunkSize), flagNumLocations, 1); err != nil {
		_ = level.Error(state.Logger).Log("msg", "Error while calling WriteExpSeq.", "err", err)
		return 0, err
	}
	// Read 1KB blocks of zeros to locations 1..100 1000-times
	if _, err := ci.ReadSeq(&bs.auth, flagNumLocations, flagNumRepetitions); err != nil {
		_ = level.Error(state.Logger).Log("msg", "Error while calling ReadExpSeq.", "err", err)
		return 0, err
	}

	return 1, nil
}

func (bs *benchStorageSeqReads) Cleanup(state *api.State) {
	client := state.State.(*ethclient.Client)
	client.Close()
}

type benchStorageRandWrites struct {
	auth             bind.TransactOpts
	contractInstance map[uint64]readwrite.Readwrite
}

func (bs *benchStorageRandWrites) Name() string {
	return "storageRandWrites"
}

func (bs *benchStorageRandWrites) Prepare(ctx context.Context, state *api.State) error {
	auth, _, contractInstance, err := prepareReadWriteContract(ctx, state)
	if err != nil {
		_ = level.Error(state.Logger).Log("msg", "Readwrite contract not deployed successfully :(", "err", err)
		return err
	}

	bs.auth = *auth
	bs.contractInstance[state.Id] = *contractInstance

	return nil
}

func (bs *benchStorageRandWrites) Scenario(ctx context.Context, state *api.State) (uint64, error) {
	ci := bs.contractInstance[state.Id]

	// Write 1KB block of zeros to 100 pseudo-random locations 1000-times
	if _, err := ci.WriteRand(&bs.auth, randSeed, make([]byte, flagChunkSize), flagNumLocations, flagNumRepetitions); err != nil {
		_ = level.Error(state.Logger).Log("msg", "Error while calling WriteExpSeq.", "err", err)
		return 0, err
	}

	return 1, nil
}

func (bs *benchStorageRandWrites) Cleanup(state *api.State) {
	client := state.State.(*ethclient.Client)
	client.Close()
}

type benchStorageRandReads struct {
	auth             bind.TransactOpts
	contractInstance map[uint64]readwrite.Readwrite
}

func (bs *benchStorageRandReads) Name() string {
	return "storageRandReads"
}

func (bs *benchStorageRandReads) Prepare(ctx context.Context, state *api.State) error {
	auth, _, contractInstance, err := prepareReadWriteContract(ctx, state)
	if err != nil {
		_ = level.Error(state.Logger).Log("msg", "Readwrite contract not deployed successfully :(", "err", err)
		return err
	}

	bs.auth = *auth
	bs.contractInstance[state.Id] = *contractInstance

	return nil
}

func (bs *benchStorageRandReads) Scenario(ctx context.Context, state *api.State) (uint64, error) {
	ci := bs.contractInstance[state.Id]

	// Write 1KB block of zeros to 100 pseudo-random locations 1000-times
	if _, err := ci.WriteRand(&bs.auth, randSeed, make([]byte, flagChunkSize), flagNumLocations, 1); err != nil {
		_ = level.Error(state.Logger).Log("msg", "Error while calling WriteExpSeq.", "err", err)
		return 0, err
	}

	// Read 1KB block of zeros from 100 pseudo-random locations 1000-times
	if _, err := ci.ReadRand(&bs.auth, randSeed, flagNumLocations, flagNumRepetitions); err != nil {
		_ = level.Error(state.Logger).Log("msg", "Error while calling ReadExpSeq.", "err", err)
		return 0, err
	}

	return 1, nil
}

func (bs *benchStorageRandReads) Cleanup(state *api.State) {
	client := state.State.(*ethclient.Client)
	client.Close()
}

// Init initializes and registers the benchmark suites.
func Init(cmd *cobra.Command) {
	cmd.Flags().Uint64Var(&flagChunkSize, cfgChunkSize, 1000, "Size of each read/written chunk in bytes")
	cmd.Flags().Uint64Var(&flagNumLocations, cfgNumLocations, 100, "Number of read/written locations")
	cmd.Flags().Uint64Var(&flagNumRepetitions, cfgNumRepetitions, 1000, "Number of times each read/write sequence is repeated inside smart contract")

	api.RegisterBenchmark(&benchStorageSeqWrites{contractInstance: make(map[uint64]readwrite.Readwrite)})
	api.RegisterBenchmark(&benchStorageSeqReads{contractInstance: make(map[uint64]readwrite.Readwrite)})
	api.RegisterBenchmark(&benchStorageRandWrites{contractInstance: make(map[uint64]readwrite.Readwrite)})
	api.RegisterBenchmark(&benchStorageRandReads{contractInstance: make(map[uint64]readwrite.Readwrite)})
}
