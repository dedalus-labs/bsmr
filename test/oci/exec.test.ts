//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies literal arguments, fixture context, failure propagation, and command deadlines.

import assert from "node:assert/strict";
import { mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { timedExec } from "./exec.ts";

test("invariant_fixture_commands_preserve_literal_arguments_and_context", async () => {
	const cwd = realpathSync(mkdtempSync(join(tmpdir(), "bsmr-oci-exec-")));
	try {
		const argument = "spaces; $VARIABLE 'quotes' $(not-a-command)";
		const result = await timedExec(10)(process.execPath, ["-e",
			"console.log(JSON.stringify([process.argv[1],process.cwd(),process.env.BSMR_FIXTURE_VALUE]))", argument],
		{ cwd, env: { BSMR_FIXTURE_VALUE: "fixture" } });
		assert.deepEqual(JSON.parse(result.stdout), [argument, cwd, "fixture"]);
		assert.equal(result.exitCode, 0);
	} finally {
		rmSync(cwd, { recursive: true });
	}
});

test("invariant_failed_fixture_commands_retain_diagnostics", async () => {
	await assert.rejects(timedExec(10)(process.execPath, ["-e", "console.error('fixture failure');process.exit(7)"]),
		/timeout exited 7: fixture failure/);
});

test("invariant_fixture_deadlines_fail_unfinished_commands", async () => {
	const result = await timedExec(0.1)("/bin/sleep", ["2"], { exitPolicy: "any" });
	assert.equal(result.exitCode, 124);
});
