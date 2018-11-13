package cmd

import (
	"context"
	"fmt"
	"io"
	"net/url"
	"os"
	"os/signal"
	"strings"
	"time"

	"github.com/go-kit/kit/log/level"
	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/client_golang/prometheus/push"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"

	"github.com/oasislabs/runtime-ethereum/benchmark/benchmarks/api"
	"github.com/oasislabs/runtime-ethereum/benchmark/benchmarks/basic"
	"github.com/oasislabs/runtime-ethereum/benchmark/benchmarks/playback"
	"github.com/oasislabs/runtime-ethereum/benchmark/benchmarks/transfer"
)

const (
	cfgBenchmarks            = "benchmarks"
	cfgBenchmarksConcurrency = "benchmarks.concurrency"
	cfgBenchmarksDuration    = "benchmarks.duration"
	cfgBenchmarksRate        = "benchmarks.rate"

	cfgLogLevel        = "log.level"
	cfgLogVerboseDebug = "log.verbose_debug"
	cfgLogFile         = "log.file"

	cfgGatewayURL = "gateway_url"

	cfgPrometheusPushAddr          = "prometheus.push.addr"
	cfgPrometheusPushJobName       = "prometheus.push.job_name"
	cfgPrometheusPushInstanceLabel = "prometheus.push.instance_label"
)

var (
	flagBenchmarks            benchmarkValues
	flagBenchmarksConcurrency uint
	flagBenchmarksDuration    time.Duration
	flagBenchmarksRate        uint

	flagLogLevel        logLevel
	flagLogVerboseDebug bool
	flagLogFile         string

	flagGatewayURL string

	flagPrometheusPushAddr          string
	flagPrometheusPushJobName       string
	flagPrometheusPushInstanceLabel string
)

func benchmarkMain(cmd *cobra.Command, args []string) {
	w, err := initLogFile(cmd)
	if err != nil {
		fmt.Fprintf(os.Stderr, "failed to initialize log file: %v", err)
		os.Exit(1)
	}
	logger := flagLogLevel.GetLogger(w)

	flagBenchmarks.deduplicate()
	if len(flagBenchmarks.benchmarks) == 0 {
		_ = level.Error(logger).Log("err", "insufficient benchmarks requested")
		os.Exit(1)
	}

	// Build the config.
	var cfg api.Config
	cfg.Logger = logger
	gatewayURL, _ := cmd.Flags().GetString(cfgGatewayURL)
	if cfg.URL, err = url.Parse(gatewayURL); err != nil {
		_ = level.Error(logger).Log("msg", "failed to parse gateway URL",
			"err", err,
		)
		os.Exit(1)
	}
	cfg.Duration, _ = cmd.Flags().GetDuration(cfgBenchmarksDuration)
	concurrency, _ := cmd.Flags().GetUint(cfgBenchmarksConcurrency)
	if concurrency == 0 {
		concurrency = 1
	}
	cfg.Concurrency = int(concurrency)

	cfg.Rate, _ = cmd.Flags().GetUint(cfgBenchmarksRate)
	verboseDebug, _ := cmd.Flags().GetBool(cfgLogVerboseDebug)
	cfg.LogVerboseDebug = verboseDebug && flagLogLevel == levelDebug

	sigCh := make(chan os.Signal)
	signal.Notify(sigCh, os.Interrupt)
	ctx, cancelFn := context.WithCancel(context.Background())
	go func() {
		<-sigCh
		_ = level.Info(logger).Log("msg", "user requested interrupt")
		cancelFn()
	}()

	for _, benchmark := range flagBenchmarks.benchmarks {
		if err = cfg.RunBenchmark(ctx, benchmark); err != nil {
			if err == context.Canceled {
				break
			}

			_ = level.Error(logger).Log("msg", "failed to run benchmark",
				"err", err,
				"benchmark", benchmark.Name(),
			)
			os.Exit(1)
		}
	}

	if err := pushMetrics(cmd); err != nil {
		_ = level.Error(logger).Log("msg", "failed to push metrics",
			"err", err,
		)
	}
}

func pushMetrics(cmd *cobra.Command) error {
	addr, _ := cmd.Flags().GetString(cfgPrometheusPushAddr)
	if addr == "" {
		return nil
	}

	jobName, _ := cmd.Flags().GetString(cfgPrometheusPushJobName)
	if jobName == "" {
		return fmt.Errorf("metrics: %v required for metrics push mode", cfgPrometheusPushJobName)
	}
	instanceLabel, _ := cmd.Flags().GetString(cfgPrometheusPushInstanceLabel)
	if instanceLabel == "" {
		return fmt.Errorf("metrics: %v required for metrics push mode", cfgPrometheusPushInstanceLabel)
	}

	pusher := push.New(addr, jobName).Grouping("instance", instanceLabel).Gatherer(prometheus.DefaultGatherer)
	return pusher.Push()
}

type benchmarkValues struct {
	benchmarks []api.Benchmark
}

func (v *benchmarkValues) deduplicate() {
	var benchmarks []api.Benchmark
	seen := make(map[string]bool)
	for _, v := range v.benchmarks {
		name := v.Name()
		if !seen[name] {
			benchmarks = append(benchmarks, v)
			seen[name] = true
		}
	}

	v.benchmarks = benchmarks
}

func (v *benchmarkValues) String() string {
	var names []string
	for _, v := range v.benchmarks {
		names = append(names, v.Name())
	}

	return strings.Join(names, ",")
}

func (v *benchmarkValues) Set(sVec string) error {
	registeredBenchmarks := api.Benchmarks()

	for _, s := range strings.Split(sVec, ",") {
		bench, ok := registeredBenchmarks[s]
		if !ok {
			return fmt.Errorf("unknown benchmark: '%v'", s)
		}

		v.benchmarks = append(v.benchmarks, bench)
	}

	return nil
}

func (v *benchmarkValues) Type() string {
	registeredBenchmarks := api.Benchmarks()

	var benchmarks []string
	for name := range registeredBenchmarks {
		benchmarks = append(benchmarks, name)
	}

	return strings.Join(benchmarks, ",")
}

func initLogFile(cmd *cobra.Command) (io.Writer, error) {
	fn, _ := cmd.Flags().GetString(cfgLogFile)
	if fn == "" {
		return os.Stdout, nil
	}
	w, err := os.Create(fn)
	if err != nil {
		return nil, err
	}
	return w, nil
}

func benchmarkInit(cmd *cobra.Command) {
	cmd.Flags().VarP(&flagBenchmarks, cfgBenchmarks, "b", "Benchmarks")
	cmd.Flags().UintVar(&flagBenchmarksConcurrency, cfgBenchmarksConcurrency, 1, "Benchmark concurrency")
	cmd.Flags().DurationVar(&flagBenchmarksDuration, cfgBenchmarksDuration, 30*time.Second, "Benchmark duration")
	cmd.Flags().UintVar(&flagBenchmarksRate, cfgBenchmarksRate, 1, "Benchmark maximum per second rate per concurrent connection")
	cmd.Flags().Var(&flagLogLevel, cfgLogLevel, "Log level")
	cmd.Flags().BoolVar(&flagLogVerboseDebug, cfgLogVerboseDebug, false, "Extremely verbose debug logging")
	cmd.Flags().StringVar(&flagLogFile, cfgLogFile, "", "Log file (default stdout)")
	cmd.Flags().StringVar(&flagGatewayURL, cfgGatewayURL, "ws://127.0.0.1:8546", "JSON-RPC gateway URL")
	cmd.Flags().StringVar(&flagPrometheusPushAddr, cfgPrometheusPushAddr, "", "Prometheus push gateway address")
	cmd.Flags().StringVar(&flagPrometheusPushJobName, cfgPrometheusPushJobName, "", "Prometheus push `job` name")
	cmd.Flags().StringVar(&flagPrometheusPushInstanceLabel, cfgPrometheusPushInstanceLabel, "", "Prometheus push `instance` label")

	for _, v := range []string{
		cfgBenchmarks,
		cfgBenchmarksConcurrency,
		cfgBenchmarksDuration,
		cfgBenchmarksRate,
		cfgLogLevel,
		cfgLogVerboseDebug,
		cfgLogFile,
		cfgGatewayURL,
		cfgPrometheusPushAddr,
		cfgPrometheusPushJobName,
		cfgPrometheusPushInstanceLabel,
	} {
		viper.BindPFlag(v, cmd.Flags().Lookup(v)) // nolint: errcheck
	}

	for _, fn := range []api.SuiteInitFn{
		basic.Init,
		playback.Init,
		transfer.Init,
	} {
		fn(cmd)
	}
}
