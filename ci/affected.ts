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
	headSha: stringInput({ description: "Candidate head commit.", default: "" }),
} as const;

type Inputs = ActionInputValues<typeof inputs>;

const rustNeutralPaths = [
	/^(?:docs|\.claude|\.vscode)\//,
	/^[^/]+\.md$/,
	/^\.github\/(?:CODEOWNERS|dependabot\.yml|pull_request_template\.md)$/,
	/^(?:LICENSE-(?:APACHE|MIT)|NOTICE)$/,
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

const mergeGroupFiles = async (
	exec: ScriptExec,
	baseSha: string,
	headSha: string,
): Promise<readonly string[]> => {
	const base = requireSha("merge-group base SHA", baseSha);
	const head = requireSha("head SHA", headSha);
	const diff = await exec("git", ["diff", "--name-only", "--no-renames", "-z", base, head]);
	return diff.stdout.split("\0").filter(Boolean);
};

export const rustAffectedForEvent = async (exec: ScriptExec, input: Inputs): Promise<boolean> => {
	if (input.eventName === "pull_request") {
		return rustAffected(await pullRequestFiles(exec, input.baseSha, input.headSha));
	}
	if (input.eventName === "merge_group") {
		return rustAffected(await mergeGroupFiles(exec, input.mergeGroupBaseSha, input.headSha));
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
