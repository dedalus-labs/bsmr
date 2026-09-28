//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies the bundled release entrypoint's compiler environment.

import { execFileSync } from "node:child_process";
import { chmodSync, copyFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

test("MSVC release commands preserve native discovery and static runtime linking", { skip: process.platform === "win32" }, () => {
	const root = mkdtempSync(join(tmpdir(), "bsmr-release-env-"));
	const target = "x86_64-pc-windows-msvc";
	try {
		for (const directory of ["bin", "ci", "tools/cargo", "tools/release", `target/${target}/dist`, `target/${target}/release`]) {
			mkdirSync(join(root, directory), { recursive: true });
		}
		copyFileSync(resolve("ci/distribute.mjs"), join(root, "ci/distribute.mjs"));
		writeFileSync(join(root, "rust-toolchain"), 'channel = "nightly-2026-04-11"\n');
		writeFileSync(join(root, "tools/cargo/rust-toolchain.toml"), 'channel = "1.98.0"\n');
		writeFileSync(join(root, `target/${target}/dist/bsmr.exe`), "engine");
		writeFileSync(join(root, `target/${target}/release/bsmr-cargo.exe`), "planner");
		const probe = join(root, "bin/rustup");
		writeFileSync(probe, '#!/usr/bin/env node\nconst assert = require("node:assert/strict"); assert.equal(process.env.CC, undefined); assert.equal(process.env.CXX, undefined); assert.equal(process.env.RUSTFLAGS, "--cfg tokio_unstable -Ctarget-feature=+crt-static");\n');
		chmodSync(probe, 0o700);
		execFileSync(process.execPath, [join(root, "ci/distribute.mjs")], {
			cwd: root,
			env: { ...process.env, PATH: `${join(root, "bin")}:${process.env["PATH"]}`, CARGO_DIST_TARGET: target, CC: "cl.exe", CXX: "cl.exe", RUSTFLAGS: "--cfg tokio_unstable" },
			stdio: "pipe",
		});
	} finally {
		rmSync(root, { recursive: true, force: true });
	}
});
