//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Installs pinned Rust tooling inside the native Mac job's private directories.

import { appendFile } from "node:fs/promises";
import { join } from "node:path";
import { action } from "@dedalus-labs/hollywood/action-runtime";
import { z } from "zod";
import { verifySha256 } from "../verify-sha256.ts";

export const installRust = action({
	name: "Install Rust tooling",
	description: "Verify and install Rust tooling in private directories before selecting a compiler.",
	localActionPath: "rust/install",
	inputs: {}, outputs: {},
	run: async ({ exec }) => {
		if (process.platform !== "darwin" || process.arch !== "arm64")
			throw new Error("native Mac qualification requires Darwin arm64");
		const path = z.string().min(1).parse(process.env["GITHUB_PATH"]);
		const environment = z.string().min(1).parse(process.env["GITHUB_ENV"]);
		const temporary = z.string().min(1).parse(process.env["RUNNER_TEMP"]);
		const cargo = join(temporary, "rust-cargo");
		const rustup = join(temporary, "rust-tools");
		const installer = join(temporary, "rustup-init");
		const python = join(temporary, "python.tar.gz");
		await exec("curl", ["--fail", "--location", "--silent", "--show-error", "--proto", "=https", "--tlsv1.2", "https://github.com/astral-sh/python-build-standalone/releases/download/20260901/cpython-3.13.15%2B20260901-aarch64-apple-darwin-install_only_stripped.tar.gz", "--output", python]);
		await verifySha256(python, "d3904bd6a072246e07aa0bdadee9a14e80521e42a943c0848059feb16a2816dc");
		await exec("tar", ["-xzf", python, "-C", temporary]);
		await exec("curl", ["--fail", "--location", "--silent", "--show-error", "--proto", "=https", "--tlsv1.2", "https://static.rust-lang.org/rustup/archive/1.29.1/aarch64-apple-darwin/rustup-init", "--output", installer]);
		await verifySha256(installer, "ec1b9233e7f72990ecd8e62063fa7f6c3dfc2bec8e97f88bff165f9100ac696a");
		await exec("chmod", ["u+x", installer]);
		await exec(installer, ["-y", "--no-modify-path", "--profile", "minimal", "--default-toolchain", "none"], { env: { CARGO_HOME: cargo, RUSTUP_HOME: rustup } });
		await appendFile(environment, `CARGO_HOME=${cargo}\nRUSTUP_HOME=${rustup}\n`);
		await appendFile(path, `${join(cargo, "bin")}\n${join(temporary, "python/bin")}\n`);
		return {};
	},
});
