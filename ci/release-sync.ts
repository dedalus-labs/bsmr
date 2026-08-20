//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Synchronizes and commits release metadata.

import {
	action,
	pathInput,
	stringInput,
	type ScriptExec,
} from "@dedalus-labs/hollywood/action-runtime";

import { synchronizeReleaseVersion } from "./release-version.ts";

const releasePaths = [
	"VERSION",
	".release-please-manifest.json",
	"app/bsmr/Cargo.toml",
	"Cargo.lock",
	"dist-workspace.toml",
	"CHANGELOG.md",
] as const;

/** Commit and push synchronized release metadata when it changed. */
export async function commitReleaseMetadata(exec: ScriptExec, branch: string): Promise<boolean> {
	await exec("git", ["check-ref-format", "--branch", branch]);
	await exec("git", ["add", ...releasePaths]);
	const staged = await exec("git", ["diff", "--cached", "--quiet"], { exitPolicy: "any" });
	if (staged.exitCode === 0) return false;
	if (staged.exitCode !== 1) throw new Error(`git diff --cached exited ${staged.exitCode}`);
	await exec("git", ["config", "user.name", "github-actions[bot]"]);
	await exec("git", ["config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com"]);
	await exec("gh", ["auth", "setup-git"]);
	await exec("git", ["commit", "--message", "chore(release): synchronize product version"]);
	await exec("git", ["push", "origin", `HEAD:${branch}`]);
	return true;
}

export const releaseSyncAction = action({
	name: "Synchronize release version",
	description: "Synchronize derived versions and push the reviewed release branch.",
	localActionPath: "ci/release-sync",
	inputs: {
		branch: stringInput({ description: "Release Please pull-request branch." }),
		workspace: pathInput({ description: "Checked-out repository root." }),
	},
	outputs: {},
	run: async ({ exec, input, log }) => {
		synchronizeReleaseVersion(input.workspace);
		const committed = await commitReleaseMetadata(exec, input.branch);
		log.info(committed ? "Committed synchronized release metadata" : "Release metadata already synchronized");
		return {};
	},
});
