//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Comments exact deployment workflow runs on each merged pull request.

import {
	action,
	pathInput,
	stringInput,
	type ScriptExec,
} from "@dedalus-labs/hollywood/action-runtime";
import { expr, job, uses, workflow } from "@dedalus-labs/hollywood";

const checkoutAction = "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1"; // v7.0.1
const commentMarker = "<!-- github-actions:post-merge-deployments -->";
const deploymentWorkflowPaths = new Set([
	".github/workflows/docs.yml",
	".github/workflows/release-publish.yml",
]);

type PushEvent = Readonly<{ after?: unknown; commits?: unknown }>;
type DeploymentRun = Readonly<{ name: string; number: number; url: string }>;

export const postMergeReceiptsAction = action({
	name: "Comment post-merge deployments",
	description: "Link deployment workflow runs on pull requests associated with this push.",
	localActionPath: "ci/post-merge-receipts",
	inputs: {
		eventPath: pathInput({ description: "Path to the GitHub event payload." }),
		repository: stringInput({ description: "GitHub owner/repository name." }),
		token: stringInput({ description: "GitHub token for reading runs and commenting." }),
	},
	outputs: {},
	run: async ({ exec, fs, input, log }) => {
		const event = JSON.parse(await fs.readText(input.eventPath)) as PushEvent;
		const pullRequests = await mergedPullRequests(exec, input.token, input.repository, event);
		if (pullRequests.length === 0) {
			log.info("No merged pull requests are associated with this push");
			return {};
		}
		if (typeof event.after !== "string" || event.after === "") {
			throw new Error("push event after SHA is required");
		}
		const runs = await deploymentRuns(exec, input.token, input.repository, event.after);
		if (runs.length === 0) {
			log.info("No deployment workflows were triggered by this merge");
			return {};
		}
		const body = deploymentComment(runs);
		for (const number of pullRequests) {
			await commentOnPullRequest(exec, input.token, input.repository, number, body);
		}
		return {};
	},
});

/** Return merged pull requests associated with every commit in one push. */
async function mergedPullRequests(
	exec: ScriptExec,
	token: string,
	repository: string,
	event: PushEvent,
): Promise<readonly number[]> {
	const numbers = new Set<number>();
	for (const sha of pushShas(event)) {
		const pulls = await ghArray(exec, token, [
			`repos/${repository}/commits/${sha}/pulls`,
			"--method",
			"GET",
			"-H",
			"Accept: application/vnd.github+json",
		]);
		for (const pull of pulls) {
			const number = numberField(pull, "number");
			const mergedAt = stringField(pull, "merged_at");
			if (number !== null && mergedAt !== null && mergedAt !== "") numbers.add(number);
		}
	}
	return [...numbers].sort((left, right) => left - right);
}

/** Return allowlisted push workflows for the exact merged commit. */
async function deploymentRuns(
	exec: ScriptExec,
	token: string,
	repository: string,
	headSha: string,
): Promise<readonly DeploymentRun[]> {
	const response = await ghObject(exec, token, [
		`repos/${repository}/actions/runs`,
		"--method",
		"GET",
		"-f",
		`head_sha=${headSha}`,
		"-f",
		"event=push",
		"-F",
		"per_page=100",
	]);
	const values = arrayField(response, "workflow_runs");
	const total = numberField(response, "total_count");
	if (total === null || total !== values.length) {
		throw new Error("GitHub workflow run response was truncated or invalid");
	}
	return values.flatMap((value) => deploymentRun(value, headSha));
}

/** Post one idempotent native-bot comment. */
async function commentOnPullRequest(
	exec: ScriptExec,
	token: string,
	repository: string,
	number: number,
	body: string,
): Promise<void> {
	const endpoint = `repos/${repository}/issues/${number}/comments`;
	const comments = await ghArray(exec, token, [endpoint, "--method", "GET", "-f", "per_page=100"]);
	if (comments.some(isDeploymentComment)) return;
	await exec("gh", ["api", endpoint, "--method", "POST", "-f", `body=${body}`], {
		env: { GH_TOKEN: token },
	});
}

/** Render the stable deployment receipt table. */
function deploymentComment(runs: readonly DeploymentRun[]): string {
	const rows = [...runs]
		.sort((left, right) => left.name.localeCompare(right.name))
		.map(({ name, number, url }) => `| ${name.replaceAll("|", "\\|")} | [#${number}](${url}) |`);
	return [
		commentMarker,
		"### Deployments triggered by this merge",
		"",
		"| Workflow | Run |",
		"| --- | --- |",
		...rows,
	].join("\n");
}

/** Validate and select one exact deployment workflow run. */
function deploymentRun(value: unknown, headSha: string): readonly DeploymentRun[] {
	const event = stringField(value, "event");
	const runHeadSha = stringField(value, "head_sha");
	const path = stringField(value, "path");
	if (event !== "push" || runHeadSha !== headSha || path === null) {
		throw new Error("GitHub returned a workflow run outside the exact push query");
	}
	if (!deploymentWorkflowPaths.has(path)) return [];
	const name = stringField(value, "name");
	const number = numberField(value, "run_number");
	const url = stringField(value, "html_url");
	if (name === null || number === null || number < 1 || url === null) {
		throw new Error("GitHub returned an invalid deployment workflow run");
	}
	return [{ name, number, url }];
}

/** Return every nonempty commit ID carried by one push event. */
function pushShas(event: PushEvent): readonly string[] {
	const shas = new Set<string>();
	if (typeof event.after === "string" && event.after !== "") shas.add(event.after);
	if (Array.isArray(event.commits)) {
		for (const commit of event.commits) {
			const id = stringField(commit, "id");
			if (id !== null && id !== "") shas.add(id);
		}
	}
	return [...shas];
}

/** Report whether the native bot already posted this receipt. */
function isDeploymentComment(comment: unknown): boolean {
	const user = objectField(comment, "user");
	return (
		stringField(user, "login") === "github-actions[bot]" &&
		stringField(comment, "body")?.includes(commentMarker) === true
	);
}

/** Read one GitHub API array response. */
async function ghArray(
	exec: ScriptExec,
	token: string,
	args: readonly string[],
): Promise<readonly unknown[]> {
	const value = JSON.parse((await exec("gh", ["api", ...args], { env: { GH_TOKEN: token } })).stdout) as unknown;
	if (!Array.isArray(value)) throw new Error("GitHub API response must be an array");
	return value;
}

/** Read one GitHub API object response. */
async function ghObject(
	exec: ScriptExec,
	token: string,
	args: readonly string[],
): Promise<Record<string, unknown>> {
	const value = JSON.parse((await exec("gh", ["api", ...args], { env: { GH_TOKEN: token } })).stdout) as unknown;
	if (value === null || typeof value !== "object" || Array.isArray(value)) {
		throw new Error("GitHub API response must be an object");
	}
	return value as Record<string, unknown>;
}

/** Read one object field without trusting external JSON. */
function objectField(value: unknown, key: string): Record<string, unknown> | null {
	const field = recordField(value, key);
	return field !== null && typeof field === "object" && !Array.isArray(field)
		? (field as Record<string, unknown>)
		: null;
}

/** Read one array field without trusting external JSON. */
function arrayField(value: unknown, key: string): readonly unknown[] {
	const field = recordField(value, key);
	if (!Array.isArray(field)) throw new Error(`GitHub API field ${key} must be an array`);
	return field;
}

/** Read one numeric field without trusting external JSON. */
function numberField(value: unknown, key: string): number | null {
	const field = recordField(value, key);
	return typeof field === "number" ? field : null;
}

/** Read one string field without trusting external JSON. */
function stringField(value: unknown, key: string): string | null {
	const field = recordField(value, key);
	return typeof field === "string" ? field : null;
}

/** Read one record field without trusting external JSON. */
function recordField(value: unknown, key: string): unknown | null {
	if (value === null || typeof value !== "object") return null;
	return (value as Record<string, unknown>)[key] ?? null;
}

export const postMergeReceipts = workflow({
	name: "PR: Post-merge deployments",
	on: { push: { branches: ["main"] }, workflow_dispatch: {} },
	permissions: { actions: "read", contents: "read", issues: "write", "pull-requests": "read" },
	jobs: {
		comment: job({
			name: "Comment deployment runs",
			if: "github.repository == 'dedalus-labs/bsmr'",
			"runs-on": "ubuntu-24.04",
			"timeout-minutes": 5,
			steps: [
				{ uses: checkoutAction, with: { "persist-credentials": false } },
				uses(postMergeReceiptsAction, {
					with: {
						eventPath: expr<string>("github.event_path"),
						repository: expr<string>("github.repository"),
						token: expr<string>("github.token"),
					},
				}),
			],
		}),
	},
});
