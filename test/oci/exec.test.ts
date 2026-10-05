//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies literal arguments, fixture context, failure propagation, and completed cancellation.

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

test("invariant_fixture_deadlines_wait_for_the_child_to_exit", async () => {
	const result = await timedExec(1)(process.execPath, ["-e", "console.log(process.pid);setInterval(()=>{},1000)"], { exitPolicy: "any" });
	assert.equal(result.exitCode, 124);
	const pid = Number(result.stdout.trim());
	assert.ok(Number.isSafeInteger(pid) && pid > 0);
	assert.throws(() => process.kill(pid, 0), { code: "ESRCH" });
});
