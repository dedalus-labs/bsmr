//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies fail-closed Rust vulnerability filtering.

import assert from "node:assert/strict";
import test from "node:test";

import type { ScriptExec } from "@dedalus-labs/hollywood/action-runtime";

import { actionableVulnerabilities, auditRustDependencies } from "./osv-audit.ts";

const input = { scanner: "/scanner", report: "/report.json" };
const vulnerability = (informational?: string) => ({
	id: "RUSTSEC-1",
	summary: "example",
	affected: [
		{
			...(informational === undefined ? {} : { database_specific: { informational } }),
		},
	],
});
const report = (entry: unknown) => ({
	results: [{ packages: [{ vulnerabilities: [entry] }] }],
});
const scanner = (exitCode: number): ScriptExec => async (_file, _args, options) => {
	assert.equal(options?.exitPolicy, "any");
	return { exitCode, stdout: "", stderr: "scanner error" };
};

test("invariant_only_explicitly_unmaintained_findings_are_ignored", () => {
	assert.deepEqual(actionableVulnerabilities(report(vulnerability("unmaintained"))), []);
	assert.deepEqual(actionableVulnerabilities(report(vulnerability())), [
		{ id: "RUSTSEC-1", summary: "example" },
	]);
});

test("invariant_scanner_and_report_failures_block_the_audit", async () => {
	await assert.rejects(
		auditRustDependencies(scanner(2), async () => JSON.stringify({ results: [] }), input),
		/exited 2/,
	);
	await assert.rejects(
		auditRustDependencies(scanner(1), async () => JSON.stringify(report(vulnerability())), input),
		/actionable OSV vulnerabilities/,
	);
	assert.throws(() => actionableVulnerabilities({}), /results must be an array/);
});
