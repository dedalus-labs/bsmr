//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies CLI command selection, ordering, and fail-fast execution.

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";

import type { ScriptExec } from "@dedalus-labs/hollywood";

import { type CliContext, runCli } from "./cli.ts";

type Invocation = Readonly<{ file: string; args: readonly string[]; cwd: string | undefined }>;

/**
 * Create an isolated CLI context that records process invocations.
 *
 * @param exec - Optional process executor used to exercise failures.
 * @returns The test context and its ordered invocation log.
 */
function harness(exec?: ScriptExec): { context: CliContext; invocations: Invocation[] } {
	const invocations: Invocation[] = [];
	return {
		context: {
			root: "/repo",
			exec:
				exec ??
				(async (file, args, options) => {
					invocations.push({ file, args, cwd: options?.cwd });
					return { exitCode: 0, stdout: "", stderr: "" };
				}),
			stdout: { write: () => undefined },
			stderr: { write: () => undefined },
		},
		invocations,
	};
}

test("check uses one typed command tree", async () => {
	const state = harness();
	await runCli(["check"], state.context);

	assert.deepEqual(
		state.invocations.map(({ file, args }) => [file, ...args.slice(0, 2)]),
		[
			["pnpm", "exec", "tsc"],
			["node", "--test", "ci/ci.test.ts"],
			["pnpm", "exec", "hollywood"],
			["node", "ci/dependabot.ts"],
			["node", "ci/license.ts", "generated"],
			["pnpm", "exec", "rolldown"],
			["node", "ci/license.ts", "check"],
			["node", "--check", ".github/actions/ci/osv-audit/dist/index.js"],
			["node", "--check", ".github/actions/ci/rust-affected/dist/index.js"],
			["node", "--check", ".github/actions/ci/verify-sha256/dist/index.js"],
			["git", "diff", "--exit-code"],
			["pnpm", "exec", "hollywood"],
		],
	);
	assert.deepEqual(state.invocations[1]?.args, [
		"--test",
		"ci/ci.test.ts",
		"ci/cli-reference.test.ts",
		"ci/cli.test.ts",
		"ci/dependabot.test.ts",
		"ci/docs.test.ts",
		"ci/license-preamble.test.ts",
		"ci/license-provenance.test.ts",
		"ci/license.test.ts",
		"ci/osv-audit.test.ts",
		"ci/verify-sha256.test.ts",
		"benchmarks/python-build-systems/run.test.ts",
		"benchmarks/python-conformance/run.test.ts",
		"benchmarks/python-conformance/snapshot.test.ts",
		"prelude/typescript/runner.test.ts",
		"prelude/toolchains/pnpm/runner.test.ts",
		"test/contributors.test.ts",
	]);
	assert.deepEqual(state.invocations[10]?.args, [
		"diff",
		"--exit-code",
		"--",
		".github/dependabot.yml",
		".github/actions",
		".github/workflows",
		"prelude/toolchains/pnpm/runner.mjs",
		"prelude/typescript/runner.mjs",
	]);
	assert.ok(state.invocations.every(({ cwd }) => cwd === "/repo"));
});

test("package exposes one command entrypoint", async () => {
	const packageJson = JSON.parse(await readFile(new URL("../package.json", import.meta.url), "utf8")) as {
		scripts: Record<string, string>;
	};
	assert.deepEqual(Object.keys(packageJson.scripts), ["ci"]);
});

test("words select nested commands", async () => {
	const state = harness();
	await runCli(["build", "actions"], state.context);
	assert.deepEqual(state.invocations.map(({ file }) => file), ["pnpm"]);
});

test("license check uses the provenance verifier", async () => {
	const state = harness();
	await runCli(["check", "license"], state.context);
	assert.deepEqual(state.invocations.map(({ file, args }) => [file, ...args]), [
		["node", "ci/license.ts", "check"],
	]);
});

test("unknown commands fail before execution", async () => {
	const state = harness();
	await assert.rejects(runCli(["check-actions"], state.context), /unknown command/);
	assert.deepEqual(state.invocations, []);
});

test("check stops at the first failure", async () => {
	let calls = 0;
	const state = harness(async () => {
		calls += 1;
		throw new Error("nope");
	});
	await assert.rejects(runCli(["check"], state.context), /nope/);
	assert.equal(calls, 1);
});
