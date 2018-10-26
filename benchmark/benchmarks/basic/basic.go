// Package basic implements the trivial benchmarks.
package basic

import (
	"context"
	"math/big"

	"github.com/ethereum/go-ethereum/common/hexutil"
	"github.com/ethereum/go-ethereum/ethclient"
	"github.com/go-kit/kit/log/level"
	"github.com/spf13/cobra"

	"github.com/oasislabs/runtime-ethereum/benchmark/benchmarks/api"
)

type benchEthBlockNumber struct {
}

func (bench *benchEthBlockNumber) Name() string {
	return "eth_blockNumber"
}

func (bench *benchEthBlockNumber) Scenario(ctx context.Context, state *api.State) (uint64, error) {
	var result hexutil.Big
	err := state.RPCClient.CallContext(ctx, &result, "eth_blockNumber")
	if err != nil {
		return 0, err
	}
	if state.Config.LogVerboseDebug {
		_ = level.Debug(state.Logger).Log("result", (*big.Int)(&result))
	}
	return 1, err
}

type benchNetVersion struct {
}

func (bench *benchNetVersion) Name() string {
	return "net_version"
}

func (bench *benchNetVersion) Prepare(ctx context.Context, state *api.State) error {
	state.State = ethclient.NewClient(state.RPCClient)
	return nil
}

func (bench *benchNetVersion) Scenario(ctx context.Context, state *api.State) (uint64, error) {
	client := state.State.(*ethclient.Client)
	version, err := client.NetworkID(ctx)
	if err != nil {
		return 0, err
	}
	if state.Config.LogVerboseDebug {
		_ = level.Debug(state.Logger).Log("result", version)
	}
	return 1, err
}

func (bench *benchNetVersion) Cleanup(state *api.State) {
	client := state.State.(*ethclient.Client)
	client.Close()
}

type benchEthGetBlockByNumber struct {
}

func (bench *benchEthGetBlockByNumber) Name() string {
	return "eth_getBlockByNumber"
}

func (bench *benchEthGetBlockByNumber) Prepare(ctx context.Context, state *api.State) error {
	state.State = ethclient.NewClient(state.RPCClient)
	return nil
}

func (bench *benchEthGetBlockByNumber) Scenario(ctx context.Context, state *api.State) (uint64, error) {
	client := state.State.(*ethclient.Client)
	blk, err := client.BlockByNumber(ctx, nil)
	if err != nil {
		return 0, err
	}
	if state.Config.LogVerboseDebug {
		_ = level.Debug(state.Logger).Log("result", blk)
	}
	return 1, err
}

func (bench *benchEthGetBlockByNumber) Cleanup(state *api.State) {
	client := state.State.(*ethclient.Client)
	client.Close()
}

// Init initializes and registers the benchmark suites.
func Init(cmd *cobra.Command) {
	api.RegisterBenchmark(&benchEthBlockNumber{})
	api.RegisterBenchmark(&benchNetVersion{})
	api.RegisterBenchmark(&benchEthGetBlockByNumber{})
}
