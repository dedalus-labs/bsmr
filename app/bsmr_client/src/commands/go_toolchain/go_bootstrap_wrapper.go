//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Package main executes Go bootstrap commands with Bessemer-owned scratch state.
// It clears the host environment and forwards only variables declared by the action.

package main

import (
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
)

// main runs the acquired Go SDK with repository-local cache and output paths.
func main() {
	if len(os.Args) < 2 {
		fmt.Fprintln(os.Stderr, "usage: go_bootstrap_wrapper <go> [options] <args>")
		os.Exit(2)
	}
	wrapped, err := filepath.Abs(os.Args[1])
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	flags := flag.NewFlagSet("go_bootstrap_wrapper", flag.ContinueOnError)
	workdir := flags.String("workdir", "", "working directory")
	output := flags.String("output", "", "stdout destination")
	if err := flags.Parse(os.Args[2:]); err != nil {
		os.Exit(2)
	}
	command := exec.Command(wrapped, flags.Args()...)
	command.Env = []string{
		"GOENV=off",
		"GOEXPERIMENT=",
		"GOFLAGS=",
		"GOTOOLCHAIN=local",
	}
	forwardEnvironment(command, "CGO_ENABLED", "GO111MODULE", "GOARCH", "GOOS")
	command.Dir = *workdir
	command.Stderr = os.Stderr
	command.Stdout = os.Stdout
	if err := appendAbsoluteEnvironment(command, "GOROOT"); err != nil {
		fail(err)
	}
	outputFile, err := redirectOutput(command, *output)
	if err != nil {
		fail(err)
	}
	if scratch := os.Getenv("BSMR_SCRATCH_PATH"); scratch != "" {
		absolute, err := filepath.Abs(scratch)
		if err != nil {
			fmt.Fprintln(os.Stderr, err)
			os.Exit(1)
		}
		command.Env = append(command.Env, "GOCACHE="+absolute, "TMPDIR="+absolute)
	}
	runErr := command.Run()
	if err := closeOutput(outputFile); err != nil {
		fail(err)
	}
	if runErr != nil {
		if exit, ok := runErr.(*exec.ExitError); ok {
			os.Exit(exit.ExitCode())
		}
		fmt.Fprintln(os.Stderr, runErr)
		os.Exit(1)
	}
}

// forwardEnvironment copies only action-declared variables required by bootstrap builds.
func forwardEnvironment(command *exec.Cmd, names ...string) {
	for _, name := range names {
		if value, ok := os.LookupEnv(name); ok {
			command.Env = append(command.Env, name+"="+value)
		}
	}
}

// appendAbsoluteEnvironment forwards one path-valued variable without cwd dependence.
func appendAbsoluteEnvironment(command *exec.Cmd, name string) error {
	value := os.Getenv(name)
	if value == "" {
		return nil
	}
	absolute, err := filepath.Abs(value)
	if err != nil {
		return err
	}
	command.Env = append(command.Env, name+"="+absolute)
	return nil
}

// redirectOutput opens an explicit stdout destination when the rule requests one.
func redirectOutput(command *exec.Cmd, path string) (*os.File, error) {
	if path == "" {
		return nil, nil
	}
	file, err := os.Create(path)
	if err != nil {
		return nil, err
	}
	command.Stdout = file
	return file, nil
}

// closeOutput reports delayed filesystem failures before the wrapper exits successfully.
func closeOutput(file *os.File) error {
	if file == nil {
		return nil
	}
	return file.Close()
}

// fail writes one bootstrap error and terminates with the wrapper failure code.
func fail(err error) {
	fmt.Fprintln(os.Stderr, err)
	os.Exit(1)
}
