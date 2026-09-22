//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Checks that locked registry and Git sources become reusable native compilation inputs.

import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { promisify } from "node:util";
import { pathToFileURL } from "node:url";

const run = promisify(execFile);
const binary = resolve(process.argv[2]!);
const root = realpathSync(mkdtempSync(join(tmpdir(), "rust-dependencies-")));
const options = { cwd: root, timeout: 180_000, maxBuffer: 8 * 1024 * 1024 };
try {
	const origin = join(root, "origin");
	mkdirSync(join(origin, "library/src"), { recursive: true });
	writeFileSync(join(origin, "Cargo.toml"), '[workspace]\nmembers=["library"]\nresolver="2"\n');
	writeFileSync(join(origin, "library/Cargo.toml"), '[package]\nname="pinned"\nversion="0.1.0"\nedition="2024"\n');
	writeFileSync(join(origin, "library/src/lib.rs"), 'pub fn value() -> u32 { 42 }\n');
	const git = { ...options, cwd: origin, env: { ...process.env, GIT_CONFIG_GLOBAL: "/dev/null", GIT_CONFIG_NOSYSTEM: "1" } };
	await run("git", ["init", "--quiet", "--template="], git);
	await run("git", ["add", "."], git);
	await run("git", ["-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "fixture"], git);
	const revision = (await run("git", ["rev-parse", "HEAD"], git)).stdout.trim();
	mkdirSync(join(root, "app/src"), { recursive: true });
	writeFileSync(join(root, "Cargo.toml"), '[workspace]\nmembers=["app"]\nresolver="2"\n');
	writeFileSync(join(root, "rust-toolchain.toml"), '[toolchain]\nchannel="1.97.1"\n');
	writeFileSync(join(root, "app/Cargo.toml"), `[package]\nname="app"\nversion="0.1.0"\nedition="2024"\n[dependencies]\nitoa="=1.0.15"\npinned={git="${pathToFileURL(origin)}",rev="${revision}"}\n`);
	writeFileSync(join(root, "app/src/main.rs"), 'fn main() { println!("{}", itoa::Buffer::new().format(pinned::value())); }\n');
	await run("rustup", ["run", "1.97.1", "cargo", "generate-lockfile"], options);
	const lock = readFileSync(join(root, "Cargo.lock"));
	await run(binary, ["init"], options);
	for (const phase of ["cold", "warm"]) {
		const { stdout, stderr } = await run(binary, ["build", "app", "--show-full-json-output", "--console", "simple"], options);
		const executable = Object.values(JSON.parse(stdout) as Record<string, string>)[0]!;
		assert.equal((await run(executable, [], options)).stdout.trim(), "42");
		const trace = /Build ID: ([a-f0-9-]+)/.exec(stderr)?.[1];
		assert.ok(trace, stderr);
		const actions = await run(binary, ["log", "what-ran", "--trace-id", trace, "--format", "json", "--filter-category", "rustc.*", "--no-remote"], options);
		if (phase === "warm") assert.equal(actions.stdout.trim(), "");
	}
	assert.deepEqual(readFileSync(join(root, "Cargo.lock")), lock);
	console.log("ok: locked registry and nested Git compilation, warm native reuse");
} finally {
	await run(binary, ["kill"], options);
	rmSync(root, { recursive: true });
}
