//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Proves Cargo graph import reaches native rustc actions and isolates invalidation.

import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { promisify } from "node:util";

const run = promisify(execFile);
const binary = resolve(process.argv[2]!);
const root = realpathSync(mkdtempSync(join(tmpdir(), "native-rust-")));
const rustc = (await run("rustup", ["which", "--toolchain", "nightly-2026-04-11", "rustc"])).stdout.trim();
const cargo = (await run("rustup", ["which", "--toolchain", "nightly-2026-04-11", "cargo"])).stdout.trim();
const env: NodeJS.ProcessEnv = { ...process.env, PATH: `${resolve(rustc, "..")}:${process.env["PATH"]}`, BSMR_LOCAL_CACHE_DIR: join(root, "cache"), BSMR_UNDECLARED_VALUE: "ambient" };
delete env["RUSTFLAGS"];
delete env["CARGO_ENCODED_RUSTFLAGS"];
const cwd = join(root, "project");
mkdirSync(cwd);
const options = { cwd, env, timeout: 120_000, maxBuffer: 8 * 1024 * 1024 };
const checkouts = [cwd];
const projects: Record<string, string> = {
	"Cargo.toml": '[workspace]\nmembers=["app", "core", "unrelated", "both"]\nresolver="2"\n',
	"rust-toolchain.toml": '[toolchain]\nchannel="nightly-2026-04-11"\n',
	"both/Cargo.toml": '[package]\nname="both"\nversion="0.1.0"\nedition="2024"\n',
	"both/src/lib.rs": 'pub fn value() -> u32 { 7 } #[test] fn library_test() { assert_eq!(value(), 7); }\n',
	"both/src/main.rs": 'fn main() { assert_eq!(both::value(), 7); } #[test] fn binary_test() { main(); }\n',
	"core/Cargo.toml": '[package]\nname="probe_core"\nversion="0.1.0"\nedition="2024"\n',
	"core/src/lib.rs": 'pub fn value() -> u32 { 7 }\n',
	"unrelated/Cargo.toml": '[package]\nname="unrelated"\nversion="0.1.0"\nedition="2024"\n',
	"unrelated/src/lib.rs": 'pub fn unrelated() -> u32 { 1 }\n',
	"app/Cargo.toml": '[package]\nname="probe_app"\nversion="0.1.0"\nedition="2024"\n[dependencies]\nrenamed = { package="probe_core", path="../core" }\n',
	"app/src/main.rs": 'fn main() { println!("{}:{}",renamed::value(),option_env!("BSMR_UNDECLARED_VALUE").unwrap_or("clean")); }\n',
};
for (const [path, text] of Object.entries(projects)) {
	mkdirSync(resolve(cwd, path, ".."), { recursive: true });
	writeFileSync(join(cwd, path), text);
}
/** Build and inspect actual native compiler commands. */
async function build(phase: string, expected: string, directory = cwd) {
	const context = { ...options, cwd: directory };
	const { stdout, stderr } = await run(binary, ["build", "app", "--show-full-json-output", "--console", "simple"], context);
	const output = Object.values(JSON.parse(stdout) as Record<string, string>)[0]!;
	assert.equal((await run(output, [], context)).stdout.trim(), expected);
	const trace = /Build ID: ([a-f0-9-]+)/.exec(stderr)?.[1];
	assert.ok(trace, stderr);
	const log = await run(binary, ["log", "what-ran", "--trace-id", trace, "--format", "json", "--filter-category", "rustc.*", "--no-remote"], context);
	console.log(JSON.stringify({ phase, trace, actions: log.stdout }));
	return log.stdout;
}
try {
	await run("python3", ["-m", "unittest", "discover", "-s", resolve(import.meta.dirname, "../prelude/rust/tools/tests"), "-p", "*_test.py"], { ...options, env: { ...env, RUSTC: rustc } });
	await run(binary, ["init"], options);
	const config = readFileSync(join(cwd, ".bsmr"), "utf8");
	writeFileSync(join(cwd, ".bsmr"), config + "\n[bsmr]\ndefault_allow_cache_upload = true\n");
	await run(cargo, ["generate-lockfile", "--offline"], options);
	const cold = await build("cold", "7:clean");
	await run(binary, ["build", "both"], options);
	await run(binary, ["test", "both:lib", "both:both", "--console", "simple"], options);
	assert.ok(cold.includes("rustc"), "must execute native rustc actions");
	writeFileSync(join(cwd, "unrelated/src/lib.rs"), "pub fn unrelated() -> u32 { 2 }\n");
	assert.equal(await build("unrelated", "7:clean"), "", "unrelated crate must not recompile app");
	writeFileSync(join(cwd, "core/src/lib.rs"), "pub fn value() -> u32 { 9 }\n#[test] fn correct_value() { assert_eq!(value(), 9); }\n");
	assert.ok((await build("dependency", "9:clean")).includes("rustc"));
	const libraryBuild = await run(binary, ["build", "core", "--show-full-json-output"], options);
	const libraryOutput = Object.values(JSON.parse(libraryBuild.stdout) as Record<string, string>)[0]!;
	assert.ok(libraryOutput.endsWith(".rlib"), "Cargo library builds must materialize linkable code, not only metadata");
	writeFileSync(join(root, "consumer.rs"), 'fn main() { assert_eq!(probe_core::value(), 9); }');
	await run(rustc, ["--edition=2024", join(root, "consumer.rs"), "--extern", `probe_core=${libraryOutput}`, "-o", join(root, "consumer")], options);
	await run(join(root, "consumer"), [], options);
	const metadataBuild = await run(binary, ["build", "core:lib[check]", "--show-full-json-output"], options);
	assert.ok(Object.values(JSON.parse(metadataBuild.stdout) as Record<string, string>)[0]!.endsWith(".rmeta"));
	// Manifest edits must invalidate inference without a synchronization command.
	const manifest = readFileSync(join(cwd, "app/Cargo.toml"), "utf8");
	writeFileSync(join(cwd, "app/Cargo.toml"), manifest.replace("renamed =", "changed ="));
	await assert.rejects(run(binary, ["build", "app"], options), /renamed/);
	writeFileSync(join(cwd, "app/Cargo.toml"), manifest);
	assert.ok(!(await build("manifestRestored", "9:clean")).includes('"executor":"Local"'), "restoring a manifest must reuse compiler results");
	for (const path of ["BUILD.bsmr", "app/BUILD.bsmr", "core/BUILD.bsmr", ".bsmr-rust-manifests.json", "toolchains"]) {
		assert.ok(!existsSync(join(cwd, path)), `inference must not write ${path}`);
	}
	await run(binary, ["test", "core", "--console", "simple"], options);
	const library = readFileSync(join(cwd, "core/src/lib.rs"), "utf8");
	writeFileSync(join(cwd, "core/src/lib.rs"), library + '\n#[test] fn detects_failure() { assert_eq!(value(), 999); }\n');
	await assert.rejects(run(binary, ["test", "core", "--console", "simple"], options), /FAIL|failed/);
	writeFileSync(join(cwd, "core/src/lib.rs"), library);
	mkdirSync(join(cwd, "recipe"));
	writeFileSync(join(cwd, "recipe/BUILD.bsmr"), 'genrule(name="report", out="report.txt", cmd="$(exe root//app:app) > $OUT")\n');
	writeFileSync(join(cwd, "recipe/consume.rs"), 'fn main() { assert_eq!(probe_core::value(), 9); }');
	writeFileSync(join(cwd, "recipe/BUILD.bsmr"), readFileSync(join(cwd, "recipe/BUILD.bsmr"), "utf8") + 'rust_binary(name="consume", crate="consume", edition="2024", crate_root="consume.rs", srcs=["consume.rs"], named_deps={"probe_core":"root//core:lib"}, _rust_toolchain="root//:__bsmr_rust")\n');
	const nativeConsumer = await run(binary, ["build", "recipe:consume", "--show-full-json-output"], options);
	await run(Object.values(JSON.parse(nativeConsumer.stdout) as Record<string, string>)[0]!, [], options);
	const recipe = await run(binary, ["build", "recipe:report", "--show-full-json-output"], options);
	const report = Object.values(JSON.parse(recipe.stdout) as Record<string, string>)[0]!;
	assert.equal(readFileSync(report, "utf8").trim(), "9:clean");
	const clone = join(root, "clone");
	cpSync(cwd, clone, { recursive: true, filter: (path) => !path.includes("/bsmr-out") });
	checkouts.push(clone);
	const reused = await build("crossCheckout", "9:clean", clone);
	assert.ok(reused.includes('"executor":"Cache"'), "a new checkout must restore native Rust actions");
	assert.ok(!reused.includes('"executor":"Local"'), "identical native compiler inputs must not execute again");
	// Included files remain keyed across edits and cache reuse, even with spaces.
	const main = readFileSync(join(cwd, "app/src/main.rs"), "utf8");
	const asset = join(cwd, "app/src/data #$: file.txt");
	writeFileSync(asset, "alpha");
	writeFileSync(join(cwd, "app/src/main.rs"), 'fn main() { print!("{}", include_str!("data #$: file.txt")); }');
	await build("declaredInput", "alpha");
	writeFileSync(asset, "beta");
	await build("declaredInputChanged", "beta");
	const outside = join(root, "outside.txt");
	writeFileSync(outside, "outside");
	for (const source of [`include_str!(${JSON.stringify(outside)})`, 'env!("HOME")']) {
		writeFileSync(join(cwd, "app/src/main.rs"), `fn main() { print!("{}", ${source}); }`);
		await assert.rejects(run(binary, ["build", "app"], options), /undeclared Rust (source|environment) input/);
		await run(binary, ["kill"], options);
		await assert.rejects(run(binary, ["build", "app"], options), /undeclared Rust (source|environment) input/);
	}
	writeFileSync(join(cwd, "app/src/main.rs"), main);
	console.log("ok: declared input changes rebuild; undeclared source/environment reads cannot publish results");
	const bothManifest = readFileSync(join(cwd, "both/Cargo.toml"), "utf8");
	writeFileSync(join(cwd, "both/Cargo.toml"), bothManifest + '[[bin]]\nname="lib"\npath="src/main.rs"\n');
	await assert.rejects(run(binary, ["build", "both:lib"], options), /binary target name `lib` is reserved/);
	writeFileSync(join(cwd, "both/Cargo.toml"), bothManifest);
	writeFileSync(join(cwd, "app/build.rs"), 'fn main() { panic!("must not execute during graph import"); }');
	await assert.rejects(run(binary, ["build", "app"], options), /custom-build/);
	console.log("ok: inferred Rust, native tests, composable recipes, manifest invalidation, cache reuse");
} finally {
	await Promise.all(checkouts.map((directory) => run(binary, ["kill"], { ...options, cwd: directory })));
	rmSync(root, { recursive: true });
}
