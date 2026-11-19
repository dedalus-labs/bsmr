//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Determines whether a change requires Rust CI and the BSMR self-build.

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

const selfBuildPaths = [
	/(?:^|\/)(?:Cargo\.toml|Cargo\.lock|BUILD\.bsmr|PACKAGE)$/,
	/^(?:prelude|tools\/build|tools\/cargo)\//,
	/^ci\/rust-build-dependencies/,
	/^(?:\.github\/workflows\/ci\.yml|ci\/affected\.ts)$/,
];

export const selfBuildAffected = (files: readonly string[]): boolean =>
	files.length === 0 ||
	files.some((file) => selfBuildPaths.some((pattern) => pattern.test(file)));

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
 * List the files a workflow event changes.
 *
 * @param exec - Process runner for git.
 * @param input - Event name and the commits it supplies.
 * @returns Changed paths, or undefined when the event has no comparison base.
 * @throws An error when a compared commit is not a full SHA.
 */
const eventFiles = async (exec: ScriptExec, input: Inputs): Promise<readonly string[] | undefined> => {
	if (input.eventName === "pull_request") return pullRequestFiles(exec, input.baseSha, input.headSha);
	if (input.eventName === "merge_group") {
		return commitRangeFiles(exec, "merge-group base SHA", input.mergeGroupBaseSha, input.headSha);
	}
	if (input.eventName === "push" && input.beforeSha !== newRefSha) {
		return commitRangeFiles(exec, "push base SHA", input.beforeSha, input.headSha);
	}
	return undefined;
};

/**
 * Decide which Rust lanes the changes behind a workflow event require.
 *
 * Without a comparison base every lane runs. Every push runs the self-build, so
 * main always proves that BSMR builds itself.
 *
 * @param exec - Process runner for git.
 * @param input - Event name and the commits it supplies.
 * @returns Whether the Rust lanes and the self-build must run.
 * @throws An error when a compared commit is not a full SHA.
 */
export const affectedForEvent = async (
	exec: ScriptExec,
	input: Inputs,
): Promise<{ rust: boolean; selfBuild: boolean }> => {
	const files = await eventFiles(exec, input);
	if (files === undefined) return { rust: true, selfBuild: true };
	return { rust: rustAffected(files), selfBuild: input.eventName === "push" || selfBuildAffected(files) };
};

export const rustAffectedAction = action({
	name: "Detect Rust changes",
	description: "Determine whether a workflow must run the Rust CI lanes.",
	localActionPath: "ci/rust-affected",
	inputs,
	outputs: {
		rust: stringOutput({ description: "Whether Rust CI must run." }),
		selfBuild: stringOutput({ description: "Whether BSMR must build itself." }),
	},
	run: async ({ exec, input, log }) => {
		const { rust, selfBuild } = await affectedForEvent(exec, input);
		log.info(`Rust CI: ${rust ? "run" : "skip"}`);
		log.info(`Self-build: ${selfBuild ? "run" : "skip"}`);
		return { rust: String(rust), selfBuild: String(selfBuild) };
	},
});
