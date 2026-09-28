//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies opaque compiler values cannot introduce build-macro dependencies.

import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { promisify } from "node:util";

const run = promisify(execFile);
const binary = resolve(process.argv[2]!);
const root = realpathSync(mkdtempSync(join(tmpdir(), "rust-literals-")));
const options = { cwd: root, timeout: 120_000, maxBuffer: 8 * 1024 * 1024 };
const value = "literal \\$(location :absent)";
const flags = ["--cfg", 'marker="$(location :absent)"'];
const rule = `rust_binary(name="probe", crate="probe", edition="2024", crate_root="main.rs", srcs=["main.rs"], literal_env=${JSON.stringify({ NOTE: value })}, literal_rustc_flags=${JSON.stringify(flags)}, _rust_toolchain="root//:__bsmr_rust")\n`;
try {
	mkdirSync(join(root, "src"));
	mkdirSync(join(root, "probe"));
	writeFileSync(join(root, "Cargo.toml"), '[package]\nname="fixture"\nversion="0.1.0"\nedition="2024"\n');
	writeFileSync(join(root, "rust-toolchain.toml"), '[toolchain]\nchannel="1.97.1"\n');
	writeFileSync(join(root, "src/lib.rs"), "");
	writeFileSync(join(root, "probe/BUILD.bsmr"), rule);
	writeFileSync(join(root, "probe/main.rs"), '#[cfg(marker="$(location :absent)")] fn main() { println!("{}", env!("NOTE")); }\n');
	const cargo = (await run("rustup", ["which", "--toolchain", "1.97.1", "cargo"])).stdout.trim();
	await run(cargo, ["generate-lockfile", "--offline"], { ...options, env: { ...process.env, RUSTC: resolve(cargo, "../rustc") } });
	await run(binary, ["init"], options);
	cpSync(resolve(import.meta.dirname, "../../prelude"), join(root, "prelude"), { recursive: true });
	writeFileSync(join(root, ".bsmr.local"), "[external_cells]\nprelude = disabled\n");
	const build = await run(binary, ["build", "probe:probe", "--show-full-json-output", "--console", "simple"], options);
	const executable = Object.values(JSON.parse(build.stdout) as Record<string, string>)[0]!;
	assert.equal((await run(executable, [], options)).stdout.trim(), value);
	writeFileSync(join(root, "probe/BUILD.bsmr"), rule.replace('name="probe",', 'name="probe", env={"NOTE":"ambiguous"},'));
	await assert.rejects(run(binary, ["build", "probe:probe"], options), /both env and literal_env/);
	console.log("ok: literal flags and environment preserve build-macro text; duplicate keys fail");
} finally {
	await run(binary, ["kill"], options);
	rmSync(root, { recursive: true });
}
