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
const toolchain = process.argv[3] ?? "1.97.1";
const root = realpathSync(mkdtempSync(join(tmpdir(), "rust-dependencies-")));
const options = { cwd: root, timeout: 180_000, maxBuffer: 8 * 1024 * 1024 };
try {
	const origin = join(root, "origin");
	mkdirSync(join(origin, "library/src"), { recursive: true });
	writeFileSync(join(origin, "Cargo.toml"), '[workspace]\nmembers=["library"]\nresolver="2"\n');
	writeFileSync(join(origin, "library/Cargo.toml"), '[package]\nname="pinned"\nversion="0.1.0"\nedition="2024"\n');
	writeFileSync(join(origin, "library/src/lib.rs"), '#![deny(dead_code)]\nfn unused() {}\npub fn value() -> u32 { 42 }\n');
	const git = { ...options, cwd: origin, env: { ...process.env, GIT_CONFIG_GLOBAL: "/dev/null", GIT_CONFIG_NOSYSTEM: "1" } };
	await run("git", ["init", "--quiet", "--template="], git);
	await run("git", ["add", "."], git);
	await run("git", ["-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "fixture"], git);
	const revision = (await run("git", ["rev-parse", "HEAD"], git)).stdout.trim();
	mkdirSync(join(root, "app/src"), { recursive: true });
	mkdirSync(join(root, ".cargo"));
	writeFileSync(join(root, ".cargo/config.toml"), '[target.x86_64-pc-windows-msvc]\nrustflags=["-C", "target-feature=+crt-static"]\n');
	writeFileSync(join(root, "Cargo.toml"), '[workspace]\nmembers=["app"]\nexclude=["dep"]\nresolver="2"\n');
	mkdirSync(join(root, "dep/src"), { recursive: true });
	writeFileSync(join(root, "dep/Cargo.toml"), '[package]\nname="dep"\nversion="0.1.0"\nedition="2024"\n');
	writeFileSync(join(root, "dep/src/lib.rs"), 'pub fn value() -> u32 { 0 }\n');
	writeFileSync(join(root, "rust-toolchain.toml"), `[toolchain]\nchannel=${JSON.stringify(toolchain)}\n`);
	writeFileSync(join(root, "app/Cargo.toml"), `[package]\nname="app"\nversion="0.1.0"\nedition="2024"\n[dependencies]\nitoa="=1.0.15"\npinned={git="${pathToFileURL(origin)}",rev="${revision}"}\ndep={path="../dep"}\n`);
	writeFileSync(join(root, "app/src/main.rs"), 'fn main() { println!("{}", itoa::Buffer::new().format(pinned::value() + dep::value())); }\n');
	await run("rustup", ["run", toolchain, "cargo", "generate-lockfile"], options);
	const lock = readFileSync(join(root, "Cargo.lock"));
	const reference = await run("rustup", ["run", toolchain, "cargo", "run", "--locked", "-p", "app"], options);
	assert.equal(reference.stdout.trim(), "42", "Cargo caps lints in external dependencies");
	await run(binary, ["init"], options);
	for (const [phase, expected] of [["cold", "42"], ["warm", "42"], ["edit", "43"]]) {
		if (phase === "edit") writeFileSync(join(root, "dep/src/lib.rs"), 'pub fn value() -> u32 { 1 }\n');
		const { stdout, stderr } = await run(binary, ["build", "app", "--show-full-json-output", "--console", "simple"], options);
		const executable = Object.values(JSON.parse(stdout) as Record<string, string>)[0]!;
		assert.equal((await run(executable, [], options)).stdout.trim(), expected);
		const trace = /Build ID: ([a-f0-9-]+)/.exec(stderr)?.[1];
		assert.ok(trace, stderr);
		const actions = await run(binary, ["log", "what-ran", "--trace-id", trace, "--format", "json", "--filter-category", "rustc.*", "--no-remote"], options);
		if (phase === "warm") assert.equal(actions.stdout.trim(), "");
	}
	assert.deepEqual(readFileSync(join(root, "Cargo.lock")), lock);
	writeFileSync(join(root, "dep/src/lib.rs"), '#![deny(dead_code)]\nfn unused() {}\npub fn value() -> u32 { 1 }\n');
	await assert.rejects(run("rustup", ["run", toolchain, "cargo", "build", "--locked", "-p", "app"], options), /never used|dead_code/);
	await assert.rejects(run(binary, ["build", "app"], options), /never used|dead_code/, "local path dependencies must retain their lint errors");
	await assert.rejects(run(binary, ["build", "dep"], options), /Cargo directory `root\/\/dep` is not a workspace member/, "dependency-only packages cannot become public build roots");
	console.log("ok: locked registry/Git and excluded path sources, warm reuse, edit invalidation");
} finally {
	await run(binary, ["kill"], options);
	rmSync(root, { recursive: true });
}
