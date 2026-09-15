//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies native Go binaries and internal tests retain embedded package files.

import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { promisify } from "node:util";

const binary = process.argv[2];
assert.ok(binary, "pass the BSMR binary under test");
const executable = resolve(binary);
const version = process.argv[3] ?? "1.26.7";
const prelude = process.argv[4];
const run = promisify(execFile);
const root = realpathSync(mkdtempSync(join(tmpdir(), "bsmr-go-build-")));
const cwd = join(root, "workspace");
const options = { cwd, env: { ...process.env, BSMR_LOCAL_CACHE_DIR: join(root, "cache") }, timeout: 300_000, maxBuffer: 16 * 1024 * 1024 };
mkdirSync(join(cwd, "cmd/probe"), { recursive: true });
writeFileSync(join(cwd, "go.mod"), "module example.com/cache-probe\n\ngo 1.26.0\n");
writeFileSync(join(cwd, "cmd/probe/main.go"), `package main
import ("fmt"; _ "embed")
//go:embed message.txt
var message string
func main() { fmt.Print(message) }
`);
writeFileSync(join(cwd, "cmd/probe/message.txt"), "original\n");
writeFileSync(join(cwd, "cmd/probe/main_test.go"), `package main
import "testing"
func TestMessage(t *testing.T) { if message == "" { t.Fatal("empty embedded message") } }
`);

try {
	await run(executable, ["init"], options);
	const configPath = join(cwd, ".bsmr");
	const config = readFileSync(configPath, "utf8");
	assert.match(config, /toolchains = root/);
	writeFileSync(configPath, config.replace("  none = none\n", "  none = none\n  toolchains = toolchains\n").replace("  toolchains = root\n", ""));
	await run(executable, ["go", "toolchain", "--version", version], options);
	await run(executable, ["kill"], options);
	await run(executable, ["go", "sync"], options);
	if (prelude !== undefined) {
		await run(executable, ["expand-external-cell", "prelude"], options);
		writeFileSync(configPath, readFileSync(configPath, "utf8").replace("  prelude = bundled\n", ""));
		for (const directory of ["go", "go_bootstrap", "toolchains/go"]) {
			cpSync(join(prelude, directory), join(cwd, "prelude", directory), { recursive: true });
		}
	}
	const { stdout } = await run(executable, ["build", "//cmd/probe:bin", "//cmd/probe:test", "--show-full-json-output", "--console", "simple"], options);
	const outputs: Record<string, string> = JSON.parse(stdout);
	const program = outputs["root//cmd/probe:bin"];
	const test = outputs["root//cmd/probe:test"];
	assert.ok(program && test, stdout);
	assert.equal((await run(program, [])).stdout, "original\n");
	assert.match((await run(test, ["-test.run=^TestMessage$", "-test.v"])).stdout, /--- PASS: TestMessage/);
	process.stdout.write(`ok: Go ${version} binary and internal test retain embedded bytes\n`);
} finally {
	await run(executable, ["kill"], options);
	rmSync(root, { recursive: true });
}
