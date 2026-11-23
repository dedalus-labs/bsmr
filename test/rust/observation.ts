//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Proves system compiler replacement invalidates a warm daemon before cache reuse.

import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { promisify } from "node:util";

const exec = promisify(execFile);
const binary = resolve(process.argv[2]!);
const prelude = process.argv[3];
const root = realpathSync(mkdtempSync(join(tmpdir(), "rust-observation-")));
const cwd = join(root, "project");
const toolchain = join(root, "toolchain");
const compiler = join(toolchain, "bin/rustc");
const env: NodeJS.ProcessEnv = { ...process.env, PATH: `${toolchain}/bin:${process.env["PATH"]}`, BSMR_LOCAL_CACHE_DIR: join(root, "cache") };
delete env["RUSTUP_TOOLCHAIN"];
const options = { cwd, env, maxBuffer: 8 * 1024 * 1024 };
/** Read only actual rustc execution events for this build. */
async function build(): Promise<string[]> {
	const result = await exec(binary, ["build", ":leaf", "--console", "simple"], options);
	const trace = /Build ID: ([a-f0-9-]+)/.exec(result.stderr)?.[1];
	assert.ok(trace, result.stderr);
	const log = await exec(binary, ["log", "what-ran", "--trace-id", trace, "--format", "json", "--filter-category", "rustc"], options);
	return log.stdout.trim().split("\n").filter(Boolean).map((line) => (JSON.parse(line) as { reproducer: { executor: string } }).reproducer.executor);
}
mkdirSync(cwd);
try {
	const source = (await exec("rustup", ["run", "nightly-2026-04-11", "rustc", "--print", "sysroot"])).stdout.trim();
	mkdirSync(join(toolchain, "bin"), { recursive: true });
	cpSync(join(source, "bin/rustc"), compiler);
	cpSync(join(source, "lib"), join(toolchain, "lib"), { recursive: true, verbatimSymlinks: true });
	await exec(binary, ["init"], options);
	cpSync(resolve(import.meta.dirname, "../fixtures/system-rust"), cwd, { recursive: true });
	if (prelude !== undefined) {
		cpSync(resolve(prelude), join(cwd, "prelude"), { recursive: true });
		writeFileSync(join(cwd, ".bsmr.local"), "[external_cells]\nprelude = disabled\n");
	}
	assert.deepEqual(await build(), ["Local"]);
	assert.deepEqual(await build(), []);
	const version = (await exec(compiler, ["-vV"], options)).stdout;
	writeFileSync(compiler, Buffer.concat([readFileSync(compiler), Buffer.from([0])]));
	assert.equal((await exec(compiler, ["-vV"], options)).stdout, version);
	assert.deepEqual(await build(), ["Local"]);
	console.log("ok: compiler content replacement invalidates same-daemon results");
	await exec(binary, ["clean"], options);
	assert.deepEqual(await build(), ["Cache"]);
} finally {
	await exec(binary, ["kill"], options);
	rmSync(root, { recursive: true });
}
