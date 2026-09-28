//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies native Rust preserves the pinned compiler's language-feature boundary.

import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { promisify } from "node:util";

const run = promisify(execFile);
const binary = resolve(process.argv[2]!);
const root = realpathSync(mkdtempSync(join(tmpdir(), "rust-stable-")));
const cwd = join(root, "project");
const env = { ...process.env };
delete env["RUSTC_BOOTSTRAP"];
delete env["RUSTFLAGS"];
delete env["CARGO_ENCODED_RUSTFLAGS"];
const options = { cwd, env, timeout: 120_000, maxBuffer: 8 * 1024 * 1024 };
const stableSource = "pub fn value<const N: u32>() -> u32 { N + 1 }\n";
const nightlySource = "#![feature(never_type)]\npub fn value<const N: u32>() -> u32 { let _: Option<!> = None; N + 1 }\n";
const files: Record<string, string> = {
	"Cargo.toml": '[workspace]\nmembers=["app", "middle", "leaf"]\nresolver="2"\n',
	"rust-toolchain.toml": '[toolchain]\nchannel="1.97.1"\n',
	"leaf/Cargo.toml": '[package]\nname="leaf"\nversion="0.1.0"\nedition="2024"\n',
	"leaf/src/lib.rs": nightlySource,
	"middle/Cargo.toml": '[package]\nname="middle"\nversion="0.1.0"\nedition="2024"\n[dependencies]\nleaf={path="../leaf"}\n',
	"middle/src/lib.rs": "pub fn value() -> u32 { leaf::value::<41>() }\n",
	"app/Cargo.toml": '[package]\nname="app"\nversion="0.1.0"\nedition="2024"\n[dependencies]\nmiddle={path="../middle"}\n',
	"app/src/main.rs": 'fn main() { println!("{}", middle::value()); }\n#[test] fn value() { assert_eq!(middle::value(), 42); }\n',
};
for (const [path, source] of Object.entries(files)) {
	mkdirSync(resolve(cwd, path, ".."), { recursive: true });
	writeFileSync(join(cwd, path), source);
}

/** Build the dependency chain and return the executed compiler-action identities. */
async function build(): Promise<string[]> {
	const { stdout, stderr } = await run(binary, ["build", "app", "--show-full-json-output", "--console", "simple"], options);
	const output = Object.values(JSON.parse(stdout) as Record<string, string>)[0]!;
	assert.equal((await run(output, [], options)).stdout.trim(), "42");
	const trace = /Build ID: ([a-f0-9-]+)/.exec(stderr)?.[1];
	assert.ok(trace, stderr);
	const log = await run(binary, ["log", "what-ran", "--trace-id", trace, "--format", "json", "--filter-category", "rustc.*", "--no-remote"], options);
	const identities = log.stdout.trim().split("\n").filter(Boolean).map((line) => (JSON.parse(line) as { identity: string }).identity);
	console.log(JSON.stringify({ trace, identities }));
	return identities;
}

try {
	await run(binary, ["init"], options);
	cpSync(resolve(import.meta.dirname, "../../prelude"), join(cwd, "prelude"), { recursive: true });
	writeFileSync(join(cwd, ".bsmr.local"), "[external_cells]\nprelude = disabled\n");
	const cargo = (await run("rustup", ["which", "--toolchain", "1.97.1", "cargo"])).stdout.trim();
	const cargoOptions = { ...options, env: { ...env, RUSTC: resolve(cargo, "../rustc") } };
	await run(cargo, ["generate-lockfile", "--offline"], cargoOptions);
	await assert.rejects(run(cargo, ["build", "--frozen", "-p", "app"], cargoOptions), /E0554/);
	console.log("ok: pinned Cargo rejects nightly-only source");
	await assert.rejects(build(), /E0554/, "native stable Rust must reject nightly-only source");
	writeFileSync(join(cwd, "leaf/src/lib.rs"), stableSource);
	const stable = await build();
	assert.equal(stable.length, 3, "stable dependencies must reuse their linked rlibs for full metadata");
	assert.ok(stable.every((identity) => !identity.includes("(rustc metadata")));
	await run(binary, ["build", "leaf", "--console", "simple"], options);
	await run(binary, ["test", "app", "--console", "simple"], options);
	writeFileSync(join(cwd, "rust-toolchain.toml"), '[toolchain]\nchannel="nightly-2026-04-11"\n');
	writeFileSync(join(cwd, "leaf/src/lib.rs"), nightlySource);
	const nightly = await build();
	assert.ok(nightly.some((identity) => identity.includes("(rustc metadata")), "nightly must retain pipelined full metadata");
	console.log("ok: stable feature rejection, generic dependency metadata, stable tests, nightly pipeline");
} finally {
	await run(binary, ["kill"], options);
	rmSync(root, { recursive: true });
}
