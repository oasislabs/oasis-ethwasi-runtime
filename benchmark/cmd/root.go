// Package cmd implements the commands for the benchmark executable.
package cmd

import (
	"fmt"
	"io"
	"os"
	"strings"

	gethLog "github.com/ethereum/go-ethereum/log"
	"github.com/go-kit/kit/log"
	"github.com/go-kit/kit/log/level"
	"github.com/spf13/cobra"
)

var (
	rootCmd = &cobra.Command{
		Use:   "benchmark",
		Short: "Ethereum synthetic benchmark",
		Run:   benchmarkMain,
	}

	logLevelMap = map[logLevel]string{
		levelError: "ERROR",
		levelWarn:  "WARN",
		levelInfo:  "INFO",
		levelDebug: "DEBUG",
	}
)

// Execute spawns the main entry point of the command.
func Execute() {
	if err := rootCmd.Execute(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

type logLevel int

const (
	levelError logLevel = iota
	levelWarn
	levelInfo
	levelDebug
)

func (lvl *logLevel) String() string {
	s, ok := logLevelMap[*lvl]
	if !ok {
		panic("invalid log level")
	}

	return s
}

func (lvl *logLevel) Set(s string) error {
	revMap := make(map[string]logLevel)
	for str, v := range logLevelMap {
		revMap[v] = str
	}

	newLvl, ok := revMap[strings.ToUpper(s)]
	if !ok {
		return fmt.Errorf("invalid log level: '%v'", s)
	}
	*lvl = newLvl

	return nil
}

func (lvl *logLevel) Type() string {
	return "[ERROR,WARN,INFO,DEBUG]"
}

func (lvl *logLevel) GetLogger(w io.Writer) log.Logger {

	// Suppress geth logging entirely.
	gethLog.Root().SetHandler(gethLog.DiscardHandler())

	logger := log.NewJSONLogger(log.NewSyncWriter(w))
	logger = log.With(logger, "ts", log.DefaultTimestampUTC)

	switch *lvl {
	case levelError:
		return level.NewFilter(logger, level.AllowError())
	case levelWarn:
		return level.NewFilter(logger, level.AllowWarn())
	case levelInfo:
		return level.NewFilter(logger, level.AllowInfo())
	case levelDebug:
		return level.NewFilter(logger, level.AllowDebug())
	default:
		panic("invalid log level")
	}
}

func init() {
	benchmarkInit(rootCmd)
}
