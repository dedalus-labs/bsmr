//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies Cargo configuration through actual native compiler and test actions.

import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { promisify } from "node:util";

const run = promisify(execFile);
const binary = resolve(process.argv[2]!);
const base = realpathSync(mkdtempSync(join(tmpdir(), "rust-configured-")));
const root = join(base, "project");
mkdirSync(root);
const env: NodeJS.ProcessEnv = { ...process.env, BSMR_LOCAL_CACHE_DIR: join(base, "cache") };
delete env["RUSTFLAGS"];
delete env["CARGO_ENCODED_RUSTFLAGS"];
const options = { cwd: root, env, timeout: 120_000, maxBuffer: 8 * 1024 * 1024 };
const config = '[build]\nbuild-dir="unowned-build"\n[target.\'cfg(unix)\']\nrustflags=' + JSON.stringify(["--cfg", "configured", "--cfg", 'marker="$(location :never)"']) + "\n";
const files: Record<string, string> = {
	"Cargo.toml": '[workspace]\nmembers=["app", "shared", "tests", "unrelated"]\nresolver="2"\n[profile.dev]\nopt-level=1\ndebug=1\n[profile.test]\ndebug-assertions=false\n[workspace.lints.rust]\nunused="deny"\n',
	"rust-toolchain.toml": '[toolchain]\nchannel="1.97.1"\n',
	".cargo/config.toml": config,
	"shared/Cargo.toml": '[package]\nname="shared"\nversion="0.1.0"\nedition="2024"\n[features]\ndefault=["fast"]\nfast=[]\n[lints]\nworkspace=true\n',
	"shared/src/lib.rs": '#[cfg(all(feature="fast", configured, marker="$(location :never)"))]\npub fn value() -> u32 { 7 }\n',
	"app/Cargo.toml": '[package]\nname="app"\nversion="0.1.0"\nedition="2024"\nauthors=["Example $(location :never)"]\n[dependencies]\nrenamed={package="shared", path="../shared"}\n[dev-dependencies]\ntest_support={path="../tests"}\n',
	"app/src/main.rs": 'fn main() { println!("{}:{}:{}", renamed::value(), env!("CARGO_PKG_AUTHORS"), include_str!("../data.txt")); }\n#[test] fn configured_test() { assert!(!cfg!(debug_assertions)); assert_eq!(renamed::value(), test_support::expected()); }\n',
	"app/data.txt": "asset",
	"app/fixture/Cargo.toml": '[package]\nname="fixture"\nversion="0.1.0"\n[workspace]\n',
	"app/fixture/src/lib.rs": 'compile_error!("fixture source is data");\n',
	"tests/Cargo.toml": '[package]\nname="test_support"\nversion="0.1.0"\nedition="2024"\n',
	"tests/src/lib.rs": 'pub fn expected() -> u32 { 7 }\n',
	"unrelated/Cargo.toml": '[package]\nname="unrelated"\nversion="0.1.0"\nedition="2024"\n',
	"unrelated/src/lib.rs": 'compile_error!("unrelated source must not compile");\n',
	"unrelated/build.rs": 'compile_error!("unrelated build script must not compile");\n',
};
files["app/src/main.rs"] += 'const _: &str = include_str!("../fixture/Cargo.toml");\n';

/** Run the selected binary and count compiler invocations in its exact trace. */
async function build(): Promise<string[]> {
	const { stdout, stderr } = await run(binary, ["build", "app", "--show-full-json-output", "--console", "simple"], options);
	const executable = Object.values(JSON.parse(stdout) as Record<string, string>)[0]!;
	assert.equal((await run(executable, [], options)).stdout.trim(), "7:Example $(location :never):asset");
	const trace = /Build ID: ([a-f0-9-]+)/.exec(stderr)?.[1];
	assert.ok(trace, stderr);
	const log = await run(binary, ["log", "what-ran", "--trace-id", trace, "--format", "json", "--filter-category", "rustc.*", "--no-remote"], options);
	return log.stdout.trim().split("\n").filter(Boolean);
}

try {
	for (const [path, source] of Object.entries(files)) {
		mkdirSync(resolve(root, path, ".."), { recursive: true });
		writeFileSync(join(root, path), source);
	}
	const cargo = (await run("rustup", ["which", "--toolchain", "1.97.1", "cargo"])).stdout.trim();
	await run(cargo, ["generate-lockfile", "--offline"], { ...options, env: { ...env, RUSTC: resolve(cargo, "../rustc") } });
	const lock = readFileSync(join(root, "Cargo.lock"));
	await run(binary, ["init"], options);
	cpSync(resolve(import.meta.dirname, "../../prelude"), join(root, "prelude"), { recursive: true });
	writeFileSync(join(root, ".bsmr.local"), "[external_cells]\nprelude = disabled\n[bsmr]\ndefault_allow_cache_upload = true\n");
	const cold = await build();
	assert.equal(cold.length, 2, "build must compile only app and its ordinary dependency");
	assert.deepEqual(await build(), [], "unchanged build must execute no compiler actions");
	writeFileSync(join(root, "unrelated/src/lib.rs"), 'compile_error!("still unrelated");\n');
	assert.deepEqual(await build(), [], "unrelated edits must not replan or compile app");
	await run(cargo, ["test", "--locked", "--offline", "-p", "app", "--bin", "app", "--target-dir", "target/reference", "--config", 'build.build-dir="target/reference"'], { ...options, env: { ...env, RUSTC: resolve(cargo, "../rustc") } });
	await run(binary, ["test", "app", "--console", "simple"], options);
	writeFileSync(join(root, "shared/src/lib.rs"), files["shared/src/lib.rs"]! + 'fn unused() {}\n');
	await assert.rejects(build(), /never used|dead_code/, "workspace lint must reach rustc");
	writeFileSync(join(root, "shared/src/lib.rs"), files["shared/src/lib.rs"]!);
	writeFileSync(join(root, ".cargo/config.toml"), config.replace('"configured"', '"different"'));
	await assert.rejects(build(), /cannot find function|not found/, "configuration edits must invalidate the plan");
	writeFileSync(join(root, ".cargo/config.toml"), config + '\n[env]\nEXPLICIT_BUILD_VALUE="configured"\n');
	await assert.rejects(build(), /unsupported Cargo environment configuration/, "unmodeled environment values must not disappear silently");
	writeFileSync(join(root, ".cargo/config.toml"), '[build]\nrustflags=["--extern", "hidden=outside.rlib"]\n');
	await assert.rejects(build(), /unsupported compiler flag/, "file overrides must not bypass dependency identity");
	writeFileSync(join(root, ".cargo/config.toml"), config);
	await build();
	assert.deepEqual(readFileSync(join(root, "Cargo.lock")), lock);
	mkdirSync(join(root, "recipe"));
	writeFileSync(join(root, "recipe/main.rs"), 'fn main() { print!("{}", include_str!("../data/value.txt")); }');
	writeFileSync(join(root, "recipe/BUILD.bsmr"), [
		'load("@prelude//rust:sources.bzl", "rust_filegroup")',
		'rust_filegroup(name="sources", mapped_srcs={"main.rs":"src/main.rs", "value.txt":"data/value.txt"})',
		'rust_binary(name="read", crate="read", edition="2024", crate_root="src/main.rs", srcs_filegroup=":sources", verify_inputs=True, _rust_toolchain="root//:__bsmr_rust")',
	].join("\n"));
	for (const value of ["alpha", "beta", "alpha"]) {
		writeFileSync(join(root, "recipe/value.txt"), value);
		const result = await run(binary, ["build", "recipe:read", "--show-full-json-output"], options);
		const executable = Object.values(JSON.parse(result.stdout) as Record<string, string>)[0]!;
		assert.equal((await run(executable, [], options)).stdout, value);
	}
	const outside = join(base, "outside.txt");
	writeFileSync(outside, "outside");
	writeFileSync(join(root, "recipe/main.rs"), `fn main() { print!("{}", include_str!(${JSON.stringify(outside)})); }`);
	await assert.rejects(run(binary, ["build", "recipe:read"], options), /undeclared Rust source input/);
	console.log("ok: configured features, target flags, workspace lints, declared data inputs, dev dependencies, invalidation, and warm reuse");
} finally {
	await run(binary, ["kill"], options);
	rmSync(base, { recursive: true });
}
