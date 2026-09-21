//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Builds both installed executables for cargo-dist's selected target.

import { copyFileSync, readFileSync, realpathSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { nodeExec, type Command, type ScriptExec } from "@dedalus-labs/hollywood";

const targets = ["aarch64-apple-darwin", "x86_64-apple-darwin", "aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc"] as const;
/** Read the exact channel from a repository-owned Rust toolchain file. */
function compiler(path: string): string {
	const pins = [...readFileSync(path, "utf8").matchAll(/^channel = "([^"\n]+)"$/gm)];
	if (pins.length !== 1) throw new Error(`${path}: expected one compiler pin`);
	return pins[0]![1]!;
}

/** Keep compiler selection, cache placement, and release profiles explicit. */
export function releaseCommands(target: string, engines: Readonly<{ engine: string; planner: string }>): readonly Command[] {
	if (!targets.some((known) => known === target)) throw new Error(`unsupported release target: ${target}`);
	return [
		{ file: "rustup", args: ["toolchain", "install", engines.engine, "--profile", "minimal", "--component", "llvm-tools-preview", "--component", "rust-src", "--no-self-update"] },
		{ file: "rustup", args: ["target", "add", "--toolchain", engines.engine, target] },
		{ file: "rustup", args: ["toolchain", "install", engines.planner, "--profile", "minimal", "--component", "llvm-tools-preview", "--no-self-update"] },
		{ file: "rustup", args: ["target", "add", "--toolchain", engines.planner, target] },
		{ file: "rustup", args: ["run", engines.engine, "cargo", "build", "--locked", "--bin", "bsmr", "--profile", "dist", "--target", target] },
		{ file: "rustup", args: ["run", engines.planner, "cargo", "build", "--locked", "--manifest-path", "tools/cargo/Cargo.toml", "--release", "--target", target, "--target-dir", "target"] },
	];
}

/** Stage only successful builds where cargo-dist expects its declared binaries. */
export async function distribute(root: string, target: string, exec: ScriptExec = nodeExec): Promise<void> {
	for (const command of releaseCommands(target, { engine: compiler(join(root, "rust-toolchain")), planner: compiler(join(root, "tools/cargo/rust-toolchain.toml")) })) {
		process.stderr.write(`${command.file} ${command.args.join(" ")}\n`);
		const result = await exec(command.file, command.args, { cwd: root });
		process.stdout.write(result.stdout);
		process.stderr.write(result.stderr);
	}
	const suffix = target.endsWith("windows-msvc") ? ".exe" : "";
	for (const [name, profile] of [["bsmr", "dist"], ["bsmr-cargo", "release"]] as const) {
		const filename = `${name}${suffix}`;
		copyFileSync(join(root, "target", target, profile, filename), join(root, "tools", "release", filename));
	}
}

/** Require the target supplied by cargo-dist before starting either compiler. */
async function main(): Promise<void> {
	const target = process.env["CARGO_DIST_TARGET"];
	if (target === undefined) throw new Error("CARGO_DIST_TARGET is required");
	await distribute(resolve(dirname(fileURLToPath(import.meta.url)), ".."), target);
}

if (process.argv[1] !== undefined && realpathSync(process.argv[1]) === fileURLToPath(import.meta.url)) {
	await main();
}
