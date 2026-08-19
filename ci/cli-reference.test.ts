//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies CLI reference drift detection.

import assert from "node:assert/strict";
import test from "node:test";

import type { ScriptExec } from "@dedalus-labs/hollywood/action-runtime";

import { checkCliReference, verifyCliReference } from "./cli-reference.ts";

test("committed CLI documentation must equal parser output", () => {
	assert.doesNotThrow(() => verifyCliReference("usage\n", "usage\n"));
	assert.throws(() => verifyCliReference("usage\n", "changed\n"), /cli\.md is stale/);
});

test("CLI documentation is generated through the built executable", async () => {
	const exec: ScriptExec = async (file, args) => {
		assert.equal(file, "target/debug/bsmr");
		assert.deepEqual(args, ["docs", "markdown-help-doc", "all"]);
		return { exitCode: 0, stdout: "usage\n", stderr: "" };
	};
	await assert.doesNotReject(
		checkCliReference(exec, async (path) => {
			assert.equal(path, "docs/reference/cli.md");
			return "usage\n";
		}, { bsmr: "target/debug/bsmr", expected: "docs/reference/cli.md" }),
	);
});
