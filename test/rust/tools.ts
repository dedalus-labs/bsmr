//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Runs native tool contract tests with the actual pinned compiler, not its rustup shim.

import { execFileSync } from "node:child_process";

const compiler = execFileSync("rustup", ["which", "--toolchain", "1.97.1", "rustc"], { encoding: "utf8" }).trim();
execFileSync("python3", ["-B", "-m", "unittest", "discover", "-s", "prelude/rust/tools/tests", "-p", "*_test.py"], {
	env: { ...process.env, RUSTC: compiler },
	stdio: "inherit",
	timeout: 120_000,
});
