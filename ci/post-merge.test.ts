//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Proves post-merge comments expose only exact deployment workflow runs.

import assert from "node:assert/strict";
import test from "node:test";

import {
	runAction,
	type CommandResult,
	type ScriptExec,
} from "@dedalus-labs/hollywood";

import { postMergeReceipts, postMergeReceiptsAction } from "./post-merge.ts";

const result = (stdout: string): CommandResult => ({ exitCode: 0, stderr: "", stdout });
const workflowRun = (headSha: string, name: string, path: string, number: number) => ({
	event: "push",
	head_sha: headSha,
	html_url: `https://github.test/actions/runs/${number}`,
	name,
	path,
	run_number: number,
});

test("post-merge comments link only deployment runs", async () => {
	const mergeSha = "a".repeat(40);
	const calls: string[][] = [];
	const exec: ScriptExec = async (_command, args) => {
		calls.push([...args]);
		const endpoint = args[1] ?? "";
		if (endpoint.endsWith(`/commits/${mergeSha}/pulls`)) {
			return result(JSON.stringify([{ merged_at: "2026-08-25T14:30:00Z", number: 148 }]));
		}
		if (endpoint.endsWith("/actions/runs")) {
			return result(
				JSON.stringify({
					total_count: 3,
					workflow_runs: [
						workflowRun(mergeSha, "Docs", ".github/workflows/docs.yml", 30),
						workflowRun(
							mergeSha,
							"Publish release",
							".github/workflows/release-publish.yml",
							20,
						),
						workflowRun(mergeSha, "CI", ".github/workflows/ci.yml", 10),
					],
				}),
			);
		}
		if (endpoint.endsWith("/issues/148/comments")) return result("[]");
		if (args.includes("POST")) return result("");
		throw new Error(`unexpected gh api call: ${args.join(" ")}`);
	};

	await runAction(postMergeReceiptsAction, {
		exec,
		fs: { readText: async () => JSON.stringify({ after: mergeSha, commits: [] }) },
		runner: { uidGid: "1001:1001" },
		with: { eventPath: "event.json", repository: "dedalus-labs/bsmr", token: "token" },
	});

	const body = calls.flat().find((argument) => argument.startsWith("body=")) ?? "";
	assert.match(body, /### Deployments triggered by this merge/);
	assert.match(body, /Docs.*#30/);
	assert.match(body, /Publish release.*#20/);
	assert.doesNotMatch(body, /\| CI \|/);
	assert.ok(calls.some((args) => args.includes(`head_sha=${mergeSha}`)));
});

test("post-merge workflow grants only native comment permissions", () => {
	assert.deepEqual(postMergeReceipts.permissions, {
		actions: "read",
		contents: "read",
		issues: "write",
		"pull-requests": "read",
	});
	const step = postMergeReceipts.jobs.comment.steps[1];
	assert.ok(step !== undefined && "with" in step);
	assert.equal(step.with?.["token"], "${{ github.token }}");
});
