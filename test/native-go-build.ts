//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies pinned native Go compilation, cache restoration, and input invalidation.

import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, delimiter, join, resolve } from "node:path";
import { promisify } from "node:util";

const binary = process.argv[2];
assert.ok(binary, "pass the BSMR binary under test");
const executable = resolve(binary);
const version = process.argv[3] ?? "1.26.7";
const prelude = process.argv[4];
const run = promisify(execFile);
const root = realpathSync(mkdtempSync(join(tmpdir(), "bsmr-go-build-")));
const cwd = join(root, "workspace");
const env: NodeJS.ProcessEnv = { ...process.env, BSMR_LOCAL_CACHE_DIR: join(root, "cache") };
const options = { cwd, env, timeout: 300_000, maxBuffer: 16 * 1024 * 1024 };
const workspaces = [cwd];
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

type Action = { identity: string; reproducer: { executor: string } };

/** Execute the resulting program and internal test, then inspect this build's actions. */
async function build(directory: string, phase: string, message = "original\n", internal = true) {
	const flags = internal ? ["-c", "go.link_mode=internal"] : [];
	const context = { ...options, cwd: directory };
	const { stdout, stderr } = await run(executable, ["build", "//cmd/probe:bin", "//cmd/probe:test", ...flags, "--show-full-json-output", "--console", "simple"], context);
	const outputs: Record<string, string> = JSON.parse(stdout);
	const program = outputs["root//cmd/probe:bin"];
	const test = outputs["root//cmd/probe:test"];
	assert.ok(program && test, stdout);
	assert.equal((await run(program, [])).stdout, message);
	assert.match((await run(test, ["-test.run=^TestMessage$", "-test.v"])).stdout, /--- PASS: TestMessage/);
	const trace = /Build ID: ([a-f0-9-]+)/.exec(stderr)?.[1];
	assert.ok(trace, stderr);
	const log = await run(executable, ["log", "what-ran", "--trace-id", trace, "--format", "json", "--filter-category", "go_.*", "--no-remote"], context);
	const actions: Action[] = log.stdout.trim() === "" ? [] : log.stdout.trim().split("\n").map((line) => JSON.parse(line));
	const local = actions.filter(({ reproducer }) => reproducer.executor === "Local");
	const cached = actions.filter(({ reproducer }) => reproducer.executor === "Cache");
	assert.equal(actions.length, local.length + cached.length);
	const digest = createHash("sha256").update(readFileSync(program)).digest("hex");
	process.stdout.write(`${JSON.stringify({ phase, trace, local: local.length, cached: cached.length, digest })}\n`);
	return { digest, local, cached, actions };
}

/** Drop this workspace's outputs and daemon while retaining the independent shared cache. */
async function clean(directory: string) {
	await run(executable, ["clean"], { ...options, cwd: directory });
}

/** Exercise the actual bootstrap action twice without admitting system tools to cache. */
async function systemBootstrap(directory: string, name: string) {
	const context = { ...options, cwd: directory };
	for (const phase of ["cold", "repeated"]) {
		await clean(directory);
		const { stdout, stderr } = await run(executable, ["build", "prelude//go/tools:pkg_analyzer", "--show-full-json-output", "--console", "simple"], context);
		const output = Object.values(JSON.parse(stdout) as Record<string, string>)[0];
		assert.ok(output && readFileSync(output).length > 0);
		const trace = /Build ID: ([a-f0-9-]+)/.exec(stderr)?.[1];
		assert.ok(trace, stderr);
		const log = await run(executable, ["log", "what-ran", "--trace-id", trace, "--format", "json", "--filter-category", "go_bootstrap_binary", "--no-remote"], context);
		const actions: Action[] = log.stdout.trim().split("\n").map((line) => JSON.parse(line));
		assert.ok(actions.length > 0);
		assert.ok(actions.every(({ reproducer }) => reproducer.executor === "Local"), "system tools cannot enter the shared cache");
		process.stdout.write(`${JSON.stringify({ phase: `${name}-${phase}`, trace, local: actions.length, cached: 0 })}\n`);
	}
}

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
	const cold = await build(cwd, "cold");
	assert.ok(cold.local.length > 0);
	assert.equal(cold.cached.length, 0);
	assert.equal((await build(cwd, "warm")).actions.length, 0);
	await clean(cwd);
	const restored = await build(cwd, "restored");
	assert.equal(restored.local.length, 0, "deleted outputs must restore after a daemon restart");
	assert.ok(restored.cached.length > 0);
	assert.equal(restored.digest, cold.digest);
	await clean(cwd);
	const second = join(root, "second");
	cpSync(cwd, second, { recursive: true, filter: (path) => basename(path) !== "bsmr-out" });
	workspaces.push(second);
	const fresh = await build(second, "fresh-root");
	assert.equal(fresh.local.length, 0, "source-root paths must not enter action identity");
	assert.equal(fresh.digest, cold.digest);
	writeFileSync(join(second, "furl-policy.yaml"), "readiness: healthy\n");
	await clean(second);
	assert.equal((await build(second, "policy-only")).local.length, 0);
	const sourcePath = join(second, "cmd/probe/main.go");
	writeFileSync(sourcePath, readFileSync(sourcePath, "utf8").replace("fmt.Print(message)", 'fmt.Print("compiled:" + message)'));
	await clean(second);
	const compiled = await build(second, "compiled-source", "compiled:original\n");
	assert.ok(compiled.local.some(({ identity }) => identity.includes("go_compile") && identity.includes("cmd/probe")));
	assert.notEqual(compiled.digest, cold.digest);
	const sdkSource = join(second, "toolchains/.bsmr-go-sdk/src/fmt/print.go");
	writeFileSync(sdkSource, `${readFileSync(sdkSource, "utf8")}\n// changed SDK input\n`);
	await clean(second);
	const sdk = await build(second, "sdk-input", "compiled:original\n");
	assert.ok(sdk.local.some(({ identity }) => identity.includes("go_bootstrap_binary")));
	await clean(second);
	const automatic = await build(second, "automatic-link", "compiled:original\n", false);
	assert.ok(automatic.local.filter(({ identity }) => identity.includes("(go_link ")).length === 2);
	assert.equal(automatic.cached.filter(({ identity }) => identity.includes("(go_link ")).length, 0);
	writeFileSync(join(second, "toolchains/BUILD.bsmr"), 'load("@prelude//toolchains:demo.bzl", "system_demo_toolchains")\nsystem_demo_toolchains()\n');
	env["PATH"] = `${join(second, "toolchains/.bsmr-go-sdk/bin")}${delimiter}${process.env["PATH"]}`;
	await systemBootstrap(second, "system-go");
	const os = process.platform === "darwin" ? "darwin" : "linux";
	const arch = process.arch === "arm64" ? "arm64" : "amd64";
	writeFileSync(join(second, "toolchains/BUILD.bsmr"), `load("@prelude//toolchains:demo.bzl", "system_demo_toolchains")
load("@prelude//toolchains/go:go_bootstrap_toolchain.bzl", "go_bootstrap_distr", "go_bootstrap_toolchain")
system_demo_toolchains(include_go = False)
go_bootstrap_distr(name = "sdk", go_root = ".bsmr-go-sdk", go_os_arch = ("${os}", "${arch}"))
go_bootstrap_toolchain(name = "go_bootstrap", go_bootstrap_distr = ":sdk", env_go_os = "${os}", env_go_arch = "${arch}", visibility = ["PUBLIC"])
`);
	await systemBootstrap(second, "system-python");
	process.stdout.write(`ok: Go ${version} embeds, restoration, source roots, policy isolation, source/SDK invalidation, system-tool exclusion\n`);
} finally {
	await Promise.all(workspaces.map((directory) => run(executable, ["kill"], { ...options, cwd: directory })));
	rmSync(root, { recursive: true });
}
