//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Checks that native Cargo selections invalidate one warm entrypoint correctly.

import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { promisify } from "node:util";

const run = promisify(execFile);
const binary = resolve(process.argv[2]!);
const root = realpathSync(mkdtempSync(join(tmpdir(), "rust-selection-")));
const options = { cwd: root, env: { ...process.env, BSMR_LOCAL_CACHE_DIR: join(root, "cache") }, timeout: 120_000, maxBuffer: 8 * 1024 * 1024 };
try {
	mkdirSync(join(root, "app/src"), { recursive: true });
	writeFileSync(join(root, "Cargo.toml"), '[workspace]\nmembers=["app"]\nresolver="2"\n');
	writeFileSync(join(root, "rust-toolchain.toml"), '[toolchain]\nchannel="1.97.1"\n');
	writeFileSync(join(root, "app/Cargo.toml"), '[package]\nname="app"\nversion="0.1.0"\nedition="2024"\n[features]\ndefault=["fast"]\nfast=[]\nextra=[]\n');
	writeFileSync(join(root, "app/src/main.rs"), 'fn main() { println!("{}:{}", u8::from(cfg!(feature="fast")) + 2 * u8::from(cfg!(feature="extra")), cfg!(debug_assertions)); }\n');
	await run("rustup", ["run", "1.97.1", "cargo", "generate-lockfile", "--offline"], options);
	const lock = readFileSync(join(root, "Cargo.lock"));
	await run(binary, ["init"], options);
	writeFileSync(join(root, ".bsmr.local"), "[bsmr]\ndefault_allow_cache_upload=true\n");
	const cases = [
		{ selection: [], expected: "1:true", cached: false },
		{ selection: ["rust.profile=release"], expected: "1:false", cached: false },
		{ selection: ["rust.features=app/extra"], expected: "3:true", cached: false },
		{ selection: ["rust.default_features=false"], expected: "0:true", cached: false },
		{ selection: ["rust.all_features=true"], expected: "3:true", cached: false },
		{ selection: ["rust.profile=release", "rust.features=app/extra", "rust.default_features=false"], expected: "2:false", cached: false },
		{ selection: ["rust.features=app/extra,app/fast", "rust.default_features=false"], expected: "3:true", cached: false },
		{ selection: ["rust.features=app/extra app/fast", "rust.default_features=false"], expected: "3:true", cached: false },
		{ selection: [], expected: "1:true", cached: true },
	] as const;
	for (const { selection, expected, cached } of cases) {
		const flags = selection.flatMap((value) => ["-c", value]);
		const { stdout, stderr } = await run(binary, ["build", "app", ...flags, "--show-full-json-output", "--console", "simple"], options);
		const executable = Object.values(JSON.parse(stdout) as Record<string, string>)[0]!;
		assert.equal((await run(executable, [], options)).stdout.trim(), expected, selection.join(","));
		if (cached) {
			const trace = /Build ID: ([a-f0-9-]+)/.exec(stderr)?.[1];
			assert.ok(trace, stderr);
			const actions = await run(binary, ["log", "what-ran", "--trace-id", trace, "--format", "json", "--filter-category", "rustc.*"], options);
			const executions = actions.stdout.trim().split("\n").filter(Boolean).map((line) => JSON.parse(line) as { reproducer: { executor: string } });
			assert.ok(executions.every((action) => action.reproducer.executor === "Cache"), actions.stdout);
		}
	}
	await assert.rejects(run(binary, ["build", "app", "-c", "rust.features=missing"], options), /feature/);
	await assert.rejects(run(binary, ["build", "app", "-c", "rust.profile=missing"], options), /profile/);
	await assert.rejects(run(binary, ["build", "app", "-c", "rust.default_features=maybe"], options), /bool|boolean/);
	assert.deepEqual(readFileSync(join(root, "Cargo.lock")), lock);
	console.log("ok: profile/features select native code, invalid selections fail, restored defaults reuse");
} finally {
	await run(binary, ["kill"], options);
	rmSync(root, { recursive: true });
}
