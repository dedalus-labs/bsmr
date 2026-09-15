//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Completes release pull-request metadata after immutable publication.

import { expr, job, uses, workflow, type ScriptExec } from "@dedalus-labs/hollywood";
import { action, stringInput } from "@dedalus-labs/hollywood/action-runtime";

import { releaseState } from "./release-state.ts";
import { releaseVersion } from "./release-version.ts";

type CompletionInput = Readonly<{
	workspace: string;
	repository: string;
	sourceSha: string;
	eventName: string;
	ref: string;
	plan: string;
}>;

/** Require object-shaped metadata before reading its fields. */
function object(value: unknown): Record<string, unknown> {
	if (value === null || typeof value !== "object" || Array.isArray(value))
		throw new Error("invalid release metadata object");
	return value as Record<string, unknown>;
}

/** Read label names without normalizing or replacing unrelated labels. */
function labelNames(value: unknown): string[] {
	if (!Array.isArray(value)) throw new Error("invalid release PR labels");
	return value.map((label) => {
		const name = object(label)["name"];
		if (typeof name !== "string") throw new Error("invalid release PR label name");
		return name;
	});
}

/** Flatten every API page while rejecting malformed pagination output. */
function pages(value: unknown): unknown[] {
	if (!Array.isArray(value) || value.some((page) => !Array.isArray(page)))
		throw new Error("invalid paginated release metadata");
	return value.flat();
}

/** Verify publication and PR ancestry before completing only release labels. */
export async function completeRelease(exec: ScriptExec, input: CompletionInput): Promise<void> {
	if (
		input.repository !== "dedalus-labs/bsmr" ||
		input.eventName !== "workflow_dispatch" ||
		input.ref !== "refs/heads/main"
	)
		throw new Error("release completion requires the main publisher");
	if (!/^[a-f0-9]{40}$/.test(input.sourceSha)) throw new Error("invalid release source SHA");
	const plan = object(JSON.parse(input.plan));
	const version = releaseVersion(input.workspace);
	const tag = `v${version}`;
	if (plan["announcement_tag"] !== tag || plan["announcement_tag_is_implicit"] !== false)
		throw new Error("release completion requires an explicit product tag");
	const prefix = `repos/${input.repository}`;
	const api = async (path: string, args: readonly string[] = []): Promise<unknown> =>
		JSON.parse((await exec("gh", ["api", `${prefix}/${path}`, ...args])).stdout);
	const release = object(await api(`releases/tags/${tag}`));
	if (
		release["tag_name"] !== tag ||
		release["target_commitish"] !== input.sourceSha ||
		typeof release["draft"] !== "boolean" ||
		typeof release["immutable"] !== "boolean"
	)
		throw new Error("release identity mismatch");
	if (releaseState(`${release["draft"]}\t${release["immutable"]}`, tag) !== "published")
		throw new Error("release is not published");
	if (object(await api(`commits/refs/tags/${tag}`))["sha"] !== input.sourceSha)
		throw new Error("release tag source mismatch");
	const pulls = pages(
		await api("pulls", [
			"--method",
			"GET",
			"--paginate",
			"--slurp",
			"-f",
			"state=closed",
			"-f",
			"base=main",
			"-f",
			"head=dedalus-labs:release-please--branches--main",
			"-F",
			"per_page=100",
		]),
	);
	const matches = pulls
		.map(object)
		.filter(
			(pr) =>
				pr["title"] === `chore(main): release ${version}` &&
				pr["state"] === "closed" &&
				typeof pr["merged_at"] === "string" &&
				object(pr["base"])["ref"] === "main" &&
				object(pr["head"])["ref"] === "release-please--branches--main" &&
				object(object(pr["head"])["repo"])["full_name"] === input.repository,
		);
	if (matches.length !== 1)
		throw new Error(`expected one merged version PR, found ${matches.length}`);
	const pr = matches[0]!;
	if (
		typeof pr["number"] !== "number" ||
		!Number.isSafeInteger(pr["number"]) ||
		pr["number"] <= 0 ||
		typeof pr["merge_commit_sha"] !== "string" ||
		!/^[a-f0-9]{40}$/.test(pr["merge_commit_sha"])
	)
		throw new Error("invalid release PR identity");
	const comparison = object(await api(`compare/${pr["merge_commit_sha"]}...${input.sourceSha}`));
	if (
		(comparison["status"] !== "identical" && comparison["status"] !== "ahead") ||
		object(comparison["merge_base_commit"])["sha"] !== pr["merge_commit_sha"]
	)
		throw new Error("release source does not descend from the version PR");
	const before = labelNames(pr["labels"]);
	const labels = `${prefix}/issues/${pr["number"]}/labels`;
	if (!before.includes("autorelease: tagged"))
		await exec("gh", ["api", "--method", "POST", labels, "-f", "labels[]=autorelease: tagged"]);
	if (before.includes("autorelease: pending"))
		await exec("gh", ["api", "--method", "DELETE", `${labels}/autorelease%3A%20pending`]);
	const after = labelNames(
		pages(await api(`issues/${pr["number"]}/labels`, ["--paginate", "--slurp"])),
	);
	if (
		!after.includes("autorelease: tagged") ||
		after.includes("autorelease: pending") ||
		before.some((label) => label !== "autorelease: pending" && !after.includes(label))
	)
		throw new Error("release label readback mismatch");
}

export const completeReleaseAction = action({
	name: "Complete release PR",
	description: "Verify immutable publication before acknowledging its release pull request.",
	localActionPath: "ci/release-complete",
	inputs: {
		workspace: stringInput({ description: "Checked-out release source." }),
		repository: stringInput({ description: "Calling repository." }),
		sourceSha: stringInput({ description: "Published source commit." }),
		eventName: stringInput({ description: "Calling workflow event." }),
		ref: stringInput({ description: "Calling workflow ref." }),
		plan: stringInput({ description: "Cargo-dist publication plan." }),
	},
	outputs: {},
	run: async ({ exec, input }) => {
		await completeRelease(exec, input);
		return {};
	},
});

export const releaseComplete = workflow({
	name: "Complete release metadata",
	on: { workflow_call: { inputs: { plan: { type: "string", required: true } } } },
	permissions: {},
	jobs: {
		complete: job({
			if: expr(
				"github.repository == 'dedalus-labs/bsmr' && github.event_name == 'workflow_dispatch' && github.ref == 'refs/heads/main' && github.event.inputs.tag != 'dry-run'",
			),
			"runs-on": "ubuntu-24.04",
			"timeout-minutes": 5,
			permissions: { contents: "read", "pull-requests": "write" },
			steps: [
				{
					uses: "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
					with: { ref: expr<string>("github.sha"), "persist-credentials": false },
				},
				uses(completeReleaseAction, {
					env: { GH_TOKEN: expr<string>("github.token") },
					with: {
						workspace: expr<string>("github.workspace"),
						repository: expr<string>("github.repository"),
						sourceSha: expr<string>("github.sha"),
						eventName: expr<string>("github.event_name"),
						ref: expr<string>("github.ref"),
						plan: expr<string>("inputs.plan"),
					},
				}),
			],
		}),
	},
});
