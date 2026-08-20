//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies release branch synchronization.

import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import type { ScriptExec } from "@dedalus-labs/hollywood/action-runtime";

import {
	commitReleaseMetadata,
	consumeReleaseOverride,
	releaseSyncAction,
	releaseWorkspace,
} from "./release-sync.ts";

type Invocation = Readonly<{
	file: string;
	args: readonly string[];
	exitPolicy: string | undefined;
}>;

function harness(diffExitCode: number): { exec: ScriptExec; invocations: Invocation[] } {
	const invocations: Invocation[] = [];
	return {
		invocations,
		exec: async (file, args, options) => {
			invocations.push({ file, args, exitPolicy: options?.exitPolicy });
			return {
				exitCode: file === "git" && args[0] === "diff" ? diffExitCode : 0,
				stdout: "",
				stderr: "",
			};
		},
	};
}

test("unchanged release metadata does not create an empty commit", async () => {
	const state = harness(0);
	assert.equal(await commitReleaseMetadata(state.exec, "release-please--branches--main"), false);
	assert.deepEqual(state.invocations.map(({ file, args }) => [file, ...args.slice(0, 2)]), [
		["git", "check-ref-format", "--branch"],
		["git", "add", "VERSION"],
		["git", "diff", "--cached"],
	]);
	assert.equal(state.invocations[2]?.exitPolicy, "any");
});

test("changed release metadata is committed to the exact release branch", async () => {
	const state = harness(1);
	assert.equal(await commitReleaseMetadata(state.exec, "release-please--branches--main"), true);
	assert.deepEqual(state.invocations.at(-1), {
		file: "git",
		args: ["push", "origin", "HEAD:release-please--branches--main"],
		exitPolicy: undefined,
	});
});

test("release synchronization reads GitHub's repository root", () => {
	assert.deepEqual(Object.keys(releaseSyncAction.inputs), ["branch"]);
	assert.equal(releaseWorkspace({ GITHUB_WORKSPACE: "/workspace" }), "/workspace");
	assert.throws(() => releaseWorkspace({}), /GITHUB_WORKSPACE is required/);
});

test("release synchronization consumes the one-shot version", () => {
	const workspace = mkdtempSync(join(tmpdir(), "bsmr-release-override-"));
	try {
		writeFileSync(join(workspace, "VERSION"), "0.0.1\n");
		writeFileSync(
			join(workspace, "release-please-config.json"),
			'{"packages":{".":{"release-as":"0.0.1","release-type":"simple"}}}\n',
		);
		assert.equal(consumeReleaseOverride(workspace), true);
		const config = JSON.parse(readFileSync(join(workspace, "release-please-config.json"), "utf8")) as {
			packages: Record<string, Record<string, unknown>>;
		};
		assert.equal(config.packages["."]?.["release-as"], undefined);
		assert.equal(consumeReleaseOverride(workspace), false);
	} finally {
		rmSync(workspace, { force: true, recursive: true });
	}
});
