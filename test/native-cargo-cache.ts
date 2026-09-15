//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies native Cargo reuse, source invalidation, and output isolation.

import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { promisify } from "node:util";

const binary = process.argv[2];
assert.ok(binary, "pass the BSMR binary under test");
const executable = resolve(binary);
const run = promisify(execFile);
const root = realpathSync(mkdtempSync(join(tmpdir(), "bsmr-cargo-cache-")));
const checkouts: string[] = [];
const env = {
	...process.env,
	BSMR_LOCAL_CACHE_DIR: join(root, "cache"),

};

/** Creates a distinct checkout with the requested Rust source bytes. */
function checkout(name: string, message: string): string {
	const cwd = join(root, name);
	mkdirSync(join(cwd, "probe/src"), { recursive: true });
	checkouts.push(cwd);
	writeFileSync(join(cwd, ".bsmr"), `[project]
root = .
[cells]
root = .
prelude = prelude
none = none
[cell_aliases]
config = prelude
ovr_config = prelude
upstream = none
toolchains = root
[external_cells]
prelude = bundled
[parser]
target_platform_detector_spec = target:root//...->prelude//platforms:default target:prelude//...->prelude//platforms:default
[build]
execution_platforms = prelude//platforms:default
`);
	writeFileSync(join(cwd, "Cargo.toml"), '[workspace]\nmembers = ["probe"]\nresolver = "3"\n');
	writeFileSync(join(cwd, "probe/Cargo.toml"), '[package]\nname = "cache_probe"\nversion = "0.1.0"\nedition = "2024"\n');
	writeFileSync(join(cwd, "Cargo.lock"), 'version = 4\n[[package]]\nname = "cache_probe"\nversion = "0.1.0"\n');
	writeFileSync(join(cwd, "rust-toolchain.toml"), '[toolchain]\nchannel = "nightly-2026-04-11"\n');
	writeFileSync(join(cwd, "probe/src/main.rs"), `fn main() { println!("${message}"); }\n`);
	return cwd;
}

/** Builds real Rust code and returns the output bytes and execution counters. */
async function build(cwd: string, message: string) {
	const { stdout, stderr } = await run(executable, ["build", "probe", "--show-full-json-output", "--console", "simple"], { cwd, env });
	const outputs: Record<string, string> = JSON.parse(stdout);
	assert.equal(Object.keys(outputs).length, 1);
	const output = Object.values(outputs)[0];
	assert.ok(output);
	const program = join(output, "debug", "cache_probe");
	assert.equal((await run(program, [])).stdout, `${message}\n`);
	const counters = /Commands: 1 \(cached: (\d+), remote: 0, local: (\d+)\)/.exec(stderr);
	assert.ok(counters, stderr);
	return {
		program,
		digest: createHash("sha256").update(readFileSync(program)).digest("hex"),
		cached: Number(counters[1]),
		local: Number(counters[2]),
	};
}

try {
	const first = await build(checkout("first", "original"), "original");
	assert.equal(first.local, 1);
	const second = await build(checkout("second", "original"), "original");
	assert.equal(second.cached, 1, "identical source in a different checkout must reuse the action");
	assert.equal(second.digest, first.digest);
	writeFileSync(second.program, "mutated checkout output");
	const third = await build(checkout("third", "original"), "original");
	assert.equal(third.cached, 1);
	assert.equal(third.digest, first.digest, "mutable output must not change shared cache bytes");
	const changed = await build(checkout("fourth", "changed"), "changed");
	assert.equal(changed.local, 1, "changed source must execute");
	process.stdout.write("ok: Cargo reuse, source invalidation, output isolation\n");
} finally {
	await Promise.all(checkouts.map((cwd) => run(executable, ["kill"], { cwd, env })));
	rmSync(root, { recursive: true });
}
