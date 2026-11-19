//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies pinned native Go compilation, cache restoration, input invalidation, tool roots, and shared toolchains.

import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
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

/** Build Go beside the Cargo and pnpm manifests that already define the root `toolchains//` package. */
async function monorepo() {
	const directory = join(root, "monorepo");
	workspaces.push(directory);
	cpSync(resolve(import.meta.dirname, "fixtures/go-tools"), directory, { recursive: true });
	const files: Record<string, string> = {
		"go.mod": readFileSync(join(directory, "go.mod"), "utf8").replace("module example.com/tools", "module example.com/monorepo"),
		"svc/main.go": 'package main\nimport "fmt"\nfunc main() { fmt.Print("go beside rust\\n") }\n',
		"Cargo.toml": '[package]\nname = "monorepo"\nversion = "0.1.0"\nedition = "2024"\n',
		"Cargo.lock": 'version = 4\n\n[[package]]\nname = "monorepo"\nversion = "0.1.0"\n',
		"rust-toolchain.toml": '[toolchain]\nchannel = "1.97.1"\n',
		"src/main.rs": 'fn main() { println!("rust beside go"); }\n',
		"package.json": '{ "name": "monorepo", "private": true }\n',
	};
	for (const [path, text] of Object.entries(files)) {
		mkdirSync(resolve(directory, path, ".."), { recursive: true });
		writeFileSync(join(directory, path), text);
	}
	const context = { ...options, cwd: directory };
	await run(executable, ["init"], context);
	await run(executable, ["go", "toolchain", "--version", version], context);
	await run(executable, ["go", "sync"], context);
	assert.equal((await run(executable, ["run", "//svc:bin", "--console", "none"], context)).stdout, "go beside rust\n");
	const greeting = (await run(executable, ["run", "//vendor/example.com/greeter/cmd/greet:bin", "--console", "none"], context)).stdout;
	assert.match(greeting, /^greeter\n/);
	assert.ok(greeting.includes("\nmod\texample.com/greeter\tv1.0.0\t\n"));
	assert.ok(greeting.includes("\ndep\texample.com/greeting\tv0.3.0\t\n"));
	assert.equal((await run(executable, ["run", ".", "--console", "none"], context)).stdout, "rust beside go\n");
	// The Rust frontend's host tools stay the `toolchains//` defaults; Go adds none of its own.
	const bootstrap = await run(executable, ["uquery", "toolchains//:python_bootstrap", "--output-attribute", "bsmr.type"], context);
	assert.match(bootstrap.stdout, /"system_python_bootstrap_toolchain"/);
}

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
	const sdkSource = join(second, ".bsmr-go-sdk/src/fmt/print.go");
	writeFileSync(sdkSource, `${readFileSync(sdkSource, "utf8")}\n// changed SDK input\n`);
	await clean(second);
	const sdk = await build(second, "sdk-input", "compiled:original\n");
	assert.ok(sdk.local.some(({ identity }) => identity.includes("go_bootstrap_binary")));
	await clean(second);
	const automatic = await build(second, "automatic-link", "compiled:original\n", false);
	assert.ok(automatic.local.filter(({ identity }) => identity.includes("(go_link ")).length === 2);
	assert.equal(automatic.cached.filter(({ identity }) => identity.includes("(go_link ")).length, 0);
	// Without the lock, the root build file alone declares the toolchains the system phases select.
	rmSync(join(second, ".bsmr-go-toolchain.json"));
	writeFileSync(join(second, "BUILD.bsmr"), 'load("@prelude//toolchains:demo.bzl", "system_demo_toolchains")\nsystem_demo_toolchains()\n');
	env["PATH"] = `${join(second, ".bsmr-go-sdk/bin")}${delimiter}${process.env["PATH"]}`;
	await systemBootstrap(second, "system-go");
	const os = process.platform === "darwin" ? "darwin" : "linux";
	const arch = process.arch === "arm64" ? "arm64" : "amd64";
	writeFileSync(join(second, "BUILD.bsmr"), `load("@prelude//toolchains:demo.bzl", "system_demo_toolchains")
load("@prelude//toolchains/go:go_bootstrap_toolchain.bzl", "go_bootstrap_distr", "go_bootstrap_toolchain")
system_demo_toolchains(include_go = False)
go_bootstrap_distr(name = "sdk", go_root = ".bsmr-go-sdk", go_os_arch = ("${os}", "${arch}"))
go_bootstrap_toolchain(name = "go_bootstrap", go_bootstrap_distr = ":sdk", env_go_os = "${os}", env_go_arch = "${arch}", visibility = ["PUBLIC"])
`);
	await systemBootstrap(second, "system-python");
	// A module whose only packages come from `tool` directives must still sync its tool binaries.
	const tools = join(cwd, "tools");
	cpSync(resolve(import.meta.dirname, "fixtures/go-tools"), tools, { recursive: true });
	await run(executable, ["go", "sync"], { ...options, cwd: tools });
	// The tool prints its debug.BuildInfo, which must match `go build -trimpath` for the same
	// vendored graph, less VCS stamps and DefaultGODEBUG that Bessemer does not record. The
	// generated toolchain builds with GOEXPERIMENT=none, which suffixes the runtime version.
	const buildInfo = (greeting: string) => `path\texample.com/greeter/cmd/greet
mod\texample.com/greeter\tv1.0.0\t
dep\texample.com/greeting\t${greeting}\t
build\t-buildmode=exe
build\t-compiler=gc
build\t-trimpath=true
build\tCGO_ENABLED=0
build\tGOARCH=${arch}
build\tGOEXPERIMENT=none
build\tGOOS=${os}
build\t${arch === "arm64" ? "GOARM64=v8.0" : "GOAMD64=v1"}
`;
	const greet = async (greeting: string) => {
		const { stdout } = await run(executable, ["run", "//tools/vendor/example.com/greeter/cmd/greet:bin", "--console", "none"], options);
		const [subject, runtime, ...info] = stdout.split("\n");
		assert.equal(subject, "greeter");
		assert.match(runtime ?? "", new RegExp(`^go\tgo${version.replaceAll(".", "\\.")}(?!\\d)`));
		assert.equal(info.join("\n"), buildInfo(greeting));
	};
	await greet("v0.3.0");
	// A dependency version lives only in the module graph, so bumping it must relink the tool.
	for (const file of ["go.mod", "vendor/modules.txt"]) {
		writeFileSync(join(tools, file), readFileSync(join(tools, file), "utf8").replace("greeting v0.3.0", "greeting v0.3.1"));
	}
	await run(executable, ["go", "sync"], { ...options, cwd: tools });
	await greet("v0.3.1");
	// Removing the last tool directive empties the graph, so sync must retire the tool's manifest.
	const toolModule = join(tools, "go.mod");
	writeFileSync(toolModule, readFileSync(toolModule, "utf8").replace("tool example.com/greeter/cmd/greet\n", ""));
	await assert.rejects(run(executable, ["go", "sync", "--check"], { ...options, cwd: tools }), /cmd\/greet\/BUILD\.bsmr/);
	const retired = await run(executable, ["go", "sync"], { ...options, cwd: tools });
	assert.match(retired.stdout, /0 packages, 0 manifests written, 2 removed/);
	await run(executable, ["go", "sync", "--check"], { ...options, cwd: tools });
	assert.ok(!existsSync(join(tools, "vendor/example.com/greeter/cmd/greet/BUILD.bsmr")));
	await monorepo();
	process.stdout.write(`ok: Go ${version} embeds, restoration, source roots, policy isolation, source/SDK invalidation, system-tool exclusion, tool directives, build info, tool retirement, Go beside Cargo and pnpm roots\n`);
} finally {
	await Promise.all(workspaces.map((directory) => run(executable, ["kill"], { ...options, cwd: directory })));
	rmSync(root, { recursive: true });
}
