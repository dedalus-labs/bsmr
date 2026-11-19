//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies every declared library format, including outputs of dependency units.

import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { globSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { promisify } from "node:util";

const run = promisify(execFile);
const binary = resolve(process.argv[2]!);
const base = realpathSync(mkdtempSync(join(tmpdir(), "rust-libraries-")));
const root = join(base, "project");
mkdirSync(root);
const env: NodeJS.ProcessEnv = { ...process.env, BSMR_LOCAL_CACHE_DIR: join(base, "cache") };
delete env["RUSTFLAGS"];
delete env["CARGO_ENCODED_RUSTFLAGS"];
const options = { cwd: root, env, timeout: 180_000, maxBuffer: 8 * 1024 * 1024 };
const extension = process.platform === "darwin" ? "dylib" : "so";
const files: Record<string, string> = {
	"Cargo.toml": '[workspace]\nmembers=["app","shared"]\nresolver="2"\n',
	"rust-toolchain.toml": '[toolchain]\nchannel="1.97.1"\n',
	"shared/Cargo.toml": '[package]\nname="shared"\nversion="0.1.0"\nedition="2024"\n[lib]\ncrate-type=["rlib","cdylib","staticlib"]\n',
	"shared/src/lib.rs": '#[unsafe(no_mangle)] pub extern "C" fn value() -> u32 { 7 }\n',
	"app/Cargo.toml": '[package]\nname="app"\nversion="0.1.0"\nedition="2024"\n[dependencies]\nshared={path="../shared"}\n',
	"app/src/main.rs": 'fn main() { println!("{}", shared::value()); }\n',
};

/** Load a fresh process so the operating system cannot reuse an earlier library mapping. */
async function load(path: string, expected: string): Promise<void> {
	const result = await run("python3", ["-c", "import ctypes,sys; print(ctypes.CDLL(sys.argv[1]).value())", path], options);
	assert.equal(result.stdout.trim(), expected);
}

/** Building the executable must also produce its dependency's declared C library. */
async function build(expected: string, cached: boolean, nativeLibraries: string[], lto?: string): Promise<void> {
	const profile = lto ? ["-c", "rust.profile=release"] : [];
	const { stdout, stderr } = await run(binary, ["build", "app", ...profile, "--show-full-json-output", "--console", "simple"], options);
	const executable = Object.values(JSON.parse(stdout) as Record<string, string>)[0]!;
	assert.equal((await run(executable, [], options)).stdout.trim(), expected);
	const libraries = [...new Set(globSync(`bsmr-out/**/libshared*.${extension}`, { cwd: root }).filter((path) => !path.includes("/output_artifacts/")).map((path) => realpathSync(join(root, path))))];
	assert.equal(libraries.length, 1, "all declared formats must materialize when building a dependent executable");
	await load(libraries[0]!, expected);
	const archives = [...new Set(globSync("bsmr-out/**/libshared*.a", { cwd: root }).filter((path) => !path.includes("/output_artifacts/")).map((path) => realpathSync(join(root, path))))];
	assert.equal(archives.length, 1, "static C libraries must also materialize");
	await link(archives[0]!, expected, nativeLibraries);
	const trace = /Build ID: ([a-f0-9-]+)/.exec(stderr)?.[1];
	assert.ok(trace, stderr);
	const log = await run(binary, ["log", "what-ran", "--trace-id", trace, "--format", "json", "--filter-category", "rustc.*", "--no-remote"], options);
	if (cached) assert.ok(!log.stdout.includes('"executor":"Local"'), log.stdout);
	if (lto) {
		const commands = log.stdout.trim().split("\n").map((line) => JSON.parse(line) as { reproducer: { details: { command: string[] } } });
		for (const action of commands) {
			const arguments_ = action.reproducer.details.command.flatMap((argument) => argument.startsWith("@") ? readFileSync(resolve(root, argument.slice(1)), "utf8") : argument).join("\n");
			assert.ok(arguments_.includes(`-Clto=${lto}`), arguments_);
		}
	}
}

/** Link a real C consumer with the platform libraries reported by Cargo's compiler. */
async function link(archive: string, expected: string, nativeLibraries: string[]): Promise<void> {
	const output = join(base, "c-consumer");
	await run("cc", [join(root, "consumer.c"), archive, ...nativeLibraries, "-o", output], options);
	assert.equal((await run(output, [], options)).stdout.trim(), expected);
}

try {
	for (const [path, source] of Object.entries(files)) {
		mkdirSync(resolve(root, path, ".."), { recursive: true });
		writeFileSync(join(root, path), source);
	}
	await run("rustup", ["run", "1.97.1", "cargo", "generate-lockfile", "--offline"], options);
	await run("rustup", ["run", "1.97.1", "cargo", "build", "--locked", "--offline", "-p", "app"], options);
	await load(join(root, `target/debug/deps/libshared.${extension}`), "7");
	const native = await run("rustup", ["run", "1.97.1", "cargo", "rustc", "--locked", "--offline", "-p", "shared", "--lib", "--", "--print", "native-static-libs"], options);
	const libraries = /native-static-libs:\s*([^\n]+)/.exec(native.stderr);
	assert.ok(libraries, native.stderr);
	const nativeLibraries = libraries[1]!.trim().split(/\s+/);
	writeFileSync(join(root, "consumer.c"), '#include <stdio.h>\nextern unsigned value(void);\nint main(void) { printf("%u\\n", value()); }\n');
	await link(join(root, "target/debug/deps/libshared.a"), "7", nativeLibraries);
	const lock = readFileSync(join(root, "Cargo.lock"));
	await run(binary, ["init"], options);
	writeFileSync(join(root, ".bsmr.local"), "[bsmr]\ndefault_allow_cache_upload=true\n");
	await build("7", false, nativeLibraries);
	await build("7", true, nativeLibraries);
	await run(binary, ["clean"], options);
	await build("7", true, nativeLibraries);
	await run(binary, ["clean"], options);
	writeFileSync(join(root, "shared/src/lib.rs"), files["shared/src/lib.rs"]!.replace("7", "9"));
	await build("9", false, nativeLibraries);
	const library = await run(binary, ["build", "shared:lib[cdylib]", "--show-full-json-output"], options);
	await load(Object.values(JSON.parse(library.stdout) as Record<string, string>)[0]!, "9");
	for (const lto of ["thin", "fat"]) {
		writeFileSync(join(root, "Cargo.toml"), files["Cargo.toml"]! + `[profile.release]\nlto="${lto}"\n`);
		const reference = await run("rustup", ["run", "1.97.1", "cargo", "run", "--locked", "--offline", "--release", "-p", "app"], options);
		assert.equal(reference.stdout.trim(), "9");
		await run(binary, ["clean"], options);
		await build("9", false, nativeLibraries, lto);
	}
	for (const kind of ["cdylib", "staticlib"]) {
		writeFileSync(join(root, "shared/Cargo.toml"), files["shared/Cargo.toml"]!.replace('["rlib","cdylib","staticlib"]', JSON.stringify([kind])));
		const result = await run(binary, ["build", "shared", "--show-full-json-output"], options);
		const output = Object.values(JSON.parse(result.stdout) as Record<string, string>)[0]!;
		if (kind === "cdylib") await load(output, "9");
		else await link(output, "9", nativeLibraries);
	}
	assert.deepEqual(readFileSync(join(root, "Cargo.lock")), lock);
	console.log("ok: multiple library outputs, Rust and C consumers, warm reuse, restoration, source edits");
} finally {
	await run(binary, ["kill"], options);
	rmSync(base, { recursive: true });
}
