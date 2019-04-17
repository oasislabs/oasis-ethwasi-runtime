// Package api implements the common interface exposed by all benchmarks.
package api

import (
	"context"
	"net/url"
	"sync"
	"sync/atomic"
	"time"

	"github.com/ethereum/go-ethereum/rpc"
	"github.com/go-kit/kit/log"
	"github.com/go-kit/kit/log/level"
	"github.com/prometheus/client_golang/prometheus"
	"github.com/spf13/cobra"
)

var benchmarkMap = make(map[string]Benchmark)

// Config is a benchmark run configuration.
type Config struct {
	Logger log.Logger
	URL    *url.URL

	Concurrency     int
	Duration        time.Duration
	Rate            uint
	LogVerboseDebug bool
}

// RunBenchmark runs the benchmark with the provided configuration.
func (cfg *Config) RunBenchmark(ctx context.Context, benchmark Benchmark) error {
	logger := log.With(cfg.Logger, "benchmark", benchmark.Name())

	_ = level.Info(logger).Log("msg", "starting benchmark")

	states := make([]*State, 0, cfg.Concurrency)
	defer func() {
		if bulkCleanupable, ok := benchmark.(BulkCleanupable); ok {
			bulkCleanupable.BulkCleanup(ctx, states)
		}

		for _, state := range states {
			if cleanupable, ok := benchmark.(Cleanupable); ok {
				cleanupable.Cleanup(state)
			}
			if state.RPCClient != nil {
				state.RPCClient.Close()
			}
		}
	}()

	// Prepare each benchmark go routine's state.
	for i := 0; i < cfg.Concurrency; i++ {
		var err error

		state := &State{
			Id:     uint64(i),
			Config: cfg,
			Logger: log.With(logger, "goroutine", i),
		}
		state.RPCClient, err = rpc.DialContext(ctx, cfg.URL.String())
		if err != nil {
			return err
		}

		if prepareable, ok := benchmark.(Prepareable); ok {
			if err = prepareable.Prepare(ctx, state); err != nil {
				state.RPCClient.Close()
				return err
			}
		}
		states = append(states, state)
	}
	_ = level.Info(logger).Log("msg", "preparation done")

	if bulkPreparable, ok := benchmark.(BulkPreparable); ok {
		if err := bulkPreparable.BulkPrepare(ctx, states); err != nil {
			return err
		}
		_ = level.Info(logger).Log("msg", "bulk-preparation done")
	}

	// Spawn each benchmark go routine.
	errCh := make(chan error, cfg.Concurrency)
	stopCh := make(chan struct{})
	counter := new(atomicCounter)
	var wg sync.WaitGroup

	var didHalt bool
	doHalt := func() {
		if !didHalt {
			close(stopCh)
			wg.Wait()
			didHalt = true
		}
	}
	defer doHalt()

	wg.Add(cfg.Concurrency)
	timeBefore := time.Now()

	var interval int64
	if cfg.Rate != 0 {
		interval = time.Second.Nanoseconds() / int64(cfg.Rate)
	}

	for i := 0; i < cfg.Concurrency; i++ {
		go func(state *State) {
			defer wg.Done()

			began, count := time.Now(), int64(0)
			for {
				select {
				case <-ctx.Done():
					_ = level.Debug(state.Logger).Log("msg", "canceled")
					return
				case <-stopCh:
					_ = level.Debug(state.Logger).Log("msg", "finished")
					return
				default:
				}

				iters, err := benchmark.Scenario(ctx, state)
				if err != nil {
					// The cancelation can also interrupt a scenario in
					// progress.
					if err == context.Canceled {
						_ = level.Debug(state.Logger).Log("msg", "canceled")
						return
					}

					_ = level.Error(state.Logger).Log("msg", "iteration failed",
						"err", err,
					)
					errCh <- err
					return
				}
				counter.Add(iters)

				if interval != 0 {
					// Rate limit
					now, next := time.Now(), began.Add(time.Duration(count*interval))
					time.Sleep(next.Sub(now))
					count++
				}

			}
		}(states[i])
	}

	duration := cfg.Duration
	timeStart := time.Now()
	countStart := counter.Get()
	_ = level.Info(logger).Log("msg", "threads started")

	doSleep := func(sleepDuration time.Duration, descr string) (time.Time, uint64, error) {
		_ = level.Info(logger).Log("msg", "begin "+descr)
		select {
		case <-ctx.Done():
			_ = level.Info(logger).Log("msg", "canceled during "+descr)
			return time.Time{}, 0, context.Canceled
		case err := <-errCh:
			return time.Time{}, 0, err
		case <-time.After(sleepDuration):
		}

		return time.Now(), counter.Get(), nil
	}

	// First 10% of time will be discarded.
	timeMidBefore, countMidBefore, err := doSleep(duration/10, "first 10%")
	if err != nil {
		return err
	}

	// Middle 80% of time will be counted.
	timeMidAfter, countMidAfter, err := doSleep(duration/10*8, "middle 80%")
	if err != nil {
		return err
	}

	// Last 10% of time will be discarded.
	timeEnd, countEnd, err := doSleep(duration/10, "last 10%")
	if err != nil {
		return err
	}

	// Signal end of run and wait for everything to finish.
	doHalt()
	timeAfter := time.Now()
	countAfter := counter.Get()
	_ = level.Info(logger).Log("msg", "threads joined")

	// Derive the actually useful results.
	midCount := countMidAfter - countMidBefore
	midDur := timeMidAfter.Sub(timeMidBefore)
	midDurMs := uint64(midDur / time.Millisecond)
	throughputInv := float64(midDurMs) / float64(midCount)
	throughput := float64(midCount) / midDur.Seconds()

	_ = level.Info(logger).Log("msg", "middle 80%",
		"calls", midCount,
		"duration", midDur,
		"calls_per_sec", throughput,
	)
	setAndRegisterGauge(benchmark.Name()+"_mid_count", float64(midCount))
	setAndRegisterGauge(benchmark.Name()+"_mid_dur_ms", float64(midDurMs))
	setAndRegisterGauge(benchmark.Name()+"_throughput_inv", throughputInv)
	setAndRegisterGauge(benchmark.Name()+"_throughput", throughput)

	// Log the optional (informative) extra results.
	totalCount := countEnd - countStart
	totalDur := timeEnd.Sub(timeStart)
	_ = level.Info(logger).Log("msg", "overall",
		"calls", totalCount,
		"duration", totalDur,
		"calls_per_sec", float64(totalCount)/totalDur.Seconds(),
	)

	beforeCount := countStart
	beforeDur := timeStart.Sub(timeBefore)
	_ = level.Info(logger).Log("msg", "ramp-up",
		"calls", beforeCount,
		"duration", beforeDur,
		"calls_per_sec", float64(beforeCount)/beforeDur.Seconds(),
	)

	afterCount := countAfter - countEnd
	afterDur := timeAfter.Sub(timeEnd)
	_ = level.Info(logger).Log("msg", "ramp-down",
		"calls", afterCount,
		"duration", afterDur,
		"calls_per_sec", float64(afterCount)/afterDur.Seconds(),
	)

	return nil
}

func setAndRegisterGauge(name string, value float64) {
	const prefix = "web3_benchmark_"

	g := prometheus.NewGauge(
		prometheus.GaugeOpts{
			Name: prefix + name,
		},
	)
	prometheus.MustRegister(g)
	g.Set(value)
}

// Benchmark is the interface exposed by each benchmark.
type Benchmark interface {
	Name() string
	Scenario(context.Context, *State) (uint64, error)
}

// Prepareable is the interface exposed by benchmarks requiring a
// pre-flight prepare step.
type Prepareable interface {
	Prepare(context.Context, *State) error
}

// BulkPreparable is the interface exposed by benchmarks that require
// a bulk pre-flight prepare step.
//
// If a benchmark also is a Preparable, the BulkPrepare operation will
// be invoked after every Prepeare operation has been completed.
type BulkPreparable interface {
	BulkPrepare(context.Context, []*State) error
}

// BulkCleanupable is the interface exposed by benchmarks that require
// a bulk cleanup step.
//
// If a benchmark is also a Cleanupable, the BulkCleanup operation will
// be invoked before any Prepare operations are dispatched.
type BulkCleanupable interface {
	BulkCleanup(context.Context, []*State)
}

// Cleanupable is the interface exposed by benchmarks require a
// post-flight cleanup step.
type Cleanupable interface {
	Cleanup(*State)
}

// State is the per-goroutine benchmark state.
type State struct {
	Id        uint64
	Config    *Config
	Logger    log.Logger
	RPCClient *rpc.Client

	State interface{}
}

// RegisterBenchmark registers a new benchmark.
func RegisterBenchmark(bench Benchmark) {
	name := bench.Name()
	if _, ok := benchmarkMap[name]; ok {
		panic("benchmark already registered: " + name)
	}
	benchmarkMap[name] = bench
}

// Benchmarks returns a map of all registered benchmarks.
func Benchmarks() map[string]Benchmark {
	return benchmarkMap
}

type atomicCounter struct {
	value uint64
}

func (c *atomicCounter) Get() uint64 {
	return atomic.LoadUint64(&c.value)
}

func (c *atomicCounter) Add(incr uint64) {
	atomic.AddUint64(&c.value, incr)
}

// SuiteInitFn is the initializer exposed by each benchmark suite package.
type SuiteInitFn func(*cobra.Command)
