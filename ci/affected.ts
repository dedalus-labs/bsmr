//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Determines whether a change requires Rust CI.

import {
	action, choiceInput, stringInput, stringOutput,
	type ActionInputValues, type ScriptExec,
} from "@dedalus-labs/hollywood/action-runtime";

const inputs = {
	eventName: choiceInput({
		description: "GitHub event that started the workflow.",
		options: ["pull_request", "push", "merge_group", "workflow_dispatch"] as const,
	}),
	baseSha: stringInput({ description: "Pull request base commit.", default: "" }),
	mergeGroupBaseSha: stringInput({ description: "Merge-group base commit.", default: "" }),
	beforeSha: stringInput({ description: "Commit a push replaced.", default: "" }),
	headSha: stringInput({ description: "Candidate head commit.", default: "" }),
} as const;

type Inputs = ActionInputValues<typeof inputs>;

const rustNeutralPaths = [
	/^(?:docs|\.agents|\.claude|\.vscode)\//,
	/^[^/]+\.md$/,
	/^\.github\/(?:CODEOWNERS|dependabot\.yml|pull_request_template\.md)$/,
	/^(?:LICENSE-(?:APACHE|MIT)|NOTICE|mkdocs\.yml)$/,
];

export const rustAffected = (files: readonly string[]): boolean =>
	files.length === 0 ||
	files.some((file) => !rustNeutralPaths.some((pattern) => pattern.test(file)));

const requireSha = (name: string, value: string): string => {
	if (!/^[0-9a-f]{40}$/.test(value)) throw new Error(`${name} must be a full commit SHA`);
	return value;
};

export const pullRequestFiles = async (
	exec: ScriptExec,
	baseSha: string,
	headSha: string,
): Promise<readonly string[]> => {
	const base = requireSha("base SHA", baseSha);
	const head = requireSha("head SHA", headSha);
	const mergeBase = requireSha(
		"merge base",
		(await exec("git", ["merge-base", base, head])).stdout.trim(),
	);
	const diff = await exec("git", ["diff", "--name-only", "--no-renames", "-z", mergeBase, head]);
	return diff.stdout.split("\0").filter(Boolean);
};

/** GitHub's `before` commit for a push that created its ref. */
const newRefSha = "0".repeat(40);

/**
 * List the files that differ between two exact commits.
 *
 * @param exec - Process runner for git.
 * @param baseName - Name of the base commit in validation errors.
 * @param baseSha - Commit the candidate is compared against.
 * @param headSha - Candidate commit.
 * @returns Changed paths, with both sides of each rename.
 * @throws An error when either commit is not a full SHA.
 */
const commitRangeFiles = async (
	exec: ScriptExec,
	baseName: string,
	baseSha: string,
	headSha: string,
): Promise<readonly string[]> => {
	const base = requireSha(baseName, baseSha);
	const head = requireSha("head SHA", headSha);
	const diff = await exec("git", ["diff", "--name-only", "--no-renames", "-z", base, head]);
	return diff.stdout.split("\0").filter(Boolean);
};

/**
 * Decide whether the changes behind a workflow event can affect Rust.
 *
 * @param exec - Process runner for git.
 * @param input - Event name and the commits it supplies.
 * @returns Whether the Rust lanes must run.
 * @throws An error when a compared commit is not a full SHA.
 */
export const rustAffectedForEvent = async (exec: ScriptExec, input: Inputs): Promise<boolean> => {
	if (input.eventName === "pull_request") {
		return rustAffected(await pullRequestFiles(exec, input.baseSha, input.headSha));
	}
	if (input.eventName === "merge_group") {
		return rustAffected(
			await commitRangeFiles(exec, "merge-group base SHA", input.mergeGroupBaseSha, input.headSha),
		);
	}
	if (input.eventName === "push") {
		// A push that creates a ref has no previous commit to compare, so it runs Rust CI.
		if (input.beforeSha === newRefSha) return true;
		return rustAffected(
			await commitRangeFiles(exec, "push base SHA", input.beforeSha, input.headSha),
		);
	}
	return true;
};

export const rustAffectedAction = action({
	name: "Detect Rust changes",
	description: "Determine whether a workflow must run the Rust CI lanes.",
	localActionPath: "ci/rust-affected",
	inputs,
	outputs: { rust: stringOutput({ description: "Whether Rust CI must run." }) },
	run: async ({ exec, input, log }) => {
		const affected = await rustAffectedForEvent(exec, input);
		log.info(`Rust CI: ${affected ? "run" : "skip"}`);
		return { rust: String(affected) };
	},
});
