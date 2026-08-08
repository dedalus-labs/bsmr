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
			["pnpm", "exec", "rolldown"],
			["node", "--check", ".github/actions/ci/rust-affected/dist/index.js"],
			["git", "diff", "--exit-code"],
			["pnpm", "exec", "hollywood"],
		],
	);
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
