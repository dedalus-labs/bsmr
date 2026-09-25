//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies native cross-crate LTO, source invalidation, and cached profile restoration.

import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { promisify } from "node:util";

const run = promisify(execFile);
const binary = resolve(process.argv[2]!);
const base = realpathSync(mkdtempSync(join(tmpdir(), "rust-lto-")));
const root = join(base, "project");
mkdirSync(root);
const options = { cwd: root, env: { ...process.env, BSMR_LOCAL_CACHE_DIR: join(base, "cache") }, timeout: 120_000, maxBuffer: 8 * 1024 * 1024 };
const manifest = '[workspace]\nmembers=["app","shared","leaf"]\nresolver="2"\n';
try {
	for (const name of ["app", "shared", "leaf"]) {
		mkdirSync(join(root, name, "src"), { recursive: true });
		const dependency = name === "app" ? "shared" : name === "shared" ? "leaf" : undefined;
		writeFileSync(join(root, name, "Cargo.toml"), `[package]\nname="${name}"\nversion="0.1.0"\nedition="2024"\n${dependency === undefined ? "" : `[dependencies]\n${dependency}={path="../${dependency}"}\n`}`);
	}
	writeFileSync(join(root, "Cargo.toml"), manifest);
	writeFileSync(join(root, "rust-toolchain.toml"), '[toolchain]\nchannel="1.97.1"\n');
	writeFileSync(join(root, "app/src/main.rs"), 'fn main() { println!("{}", shared::value()); }\n');
	writeFileSync(join(root, "shared/src/lib.rs"), 'pub fn value() -> u32 { leaf::value() + 1 }\n#[test] fn value_is_positive() { assert!(value() > 0); }\n');
	writeFileSync(join(root, "leaf/src/lib.rs"), 'pub fn value() -> u32 { 41 }\n');
	await run("rustup", ["run", "1.97.1", "cargo", "generate-lockfile", "--offline"], options);
	const lock = readFileSync(join(root, "Cargo.lock"));
	await run(binary, ["init"], options);
	writeFileSync(join(root, ".bsmr.local"), "[bsmr]\ndefault_allow_cache_upload=true\n");
	for (const [lto, expected, cached] of [['"thin"', "42", false], ['"fat"', "42", false], ["true", "42", false], ["false", "42", false], ['"off"', "42", false], ['"thin"', "42", true], ['"thin"', "43", false]] as const) {
		writeFileSync(join(root, "Cargo.toml"), `${manifest}[profile.release]\nlto=${lto}\ncodegen-units=16\n[profile.release.package.leaf]\ncodegen-units=1\n`);
		if (expected === "43") writeFileSync(join(root, "leaf/src/lib.rs"), 'pub fn value() -> u32 { 42 }\n');
		const cargo = await run("rustup", ["run", "1.97.1", "cargo", "run", "--locked", "--offline", "--release", "-p", "app", "-q"], options);
		assert.equal(cargo.stdout.trim(), expected);
		if (cached) await run(binary, ["clean"], options);
		const { stdout, stderr } = await run(binary, ["build", "app", "-c", "rust.profile=release", "--show-full-json-output", "--console", "simple"], options);
		const executable = Object.values(JSON.parse(stdout) as Record<string, string>)[0]!;
		assert.equal((await run(executable, [], options)).stdout.trim(), expected, lto);
		const trace = /Build ID: ([a-f0-9-]+)/.exec(stderr)?.[1];
		assert.ok(trace, stderr);
		const actions = await run(binary, ["log", "what-ran", "--trace-id", trace, "--format", "json", "--filter-category", "rustc.*"], options);
		const executions = actions.stdout.trim().split("\n").filter(Boolean).map((line) => JSON.parse(line) as { reproducer: { executor: string; details: { command?: string[] } } });
		if (cached) {
			assert.ok(executions.every((action) => action.reproducer.executor === "Cache"), actions.stdout);
		} else if (lto === '"thin"' || lto === '"fat"') {
			const commands = executions.filter((action) => action.reproducer.executor === "Local").flatMap((action) => {
				const command = action.reproducer.details.command;
				assert.ok(command, actions.stdout);
				return command.map((argument) => argument.startsWith("@") ? readFileSync(resolve(root, argument.slice(1)), "utf8") : argument);
			}).join("\n");
			assert.ok(commands.includes(`-Clto=${lto.slice(1, -1)}`), commands);
			assert.ok(commands.includes("-Cembed-bitcode=yes"), commands);
		}
		console.log(`ok: lto=${lto} output=${expected} cached=${cached}`);
	}
	await run(binary, ["test", "shared", "-c", "rust.profile=release", "--console", "simple"], options);
	assert.deepEqual(readFileSync(join(root, "Cargo.lock")), lock);
} finally {
	await run(binary, ["kill"], options);
	rmSync(base, { recursive: true });
}
