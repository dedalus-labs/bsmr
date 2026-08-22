//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Validates the current product version against immutable GitHub releases.

import { action, stringInput, stringOutput } from "@dedalus-labs/hollywood/action-runtime";

import { releaseVersion } from "./release-version.ts";

type ReleaseState = "absent" | "published";

/** Resolve one product tag from tab-separated GitHub release records. */
export function releaseState(source: string, tag: string): ReleaseState {
	const state = source.trimEnd();
	if (state === "") return "absent";
	if (state === "true\tfalse") throw new Error(`${tag} is still a draft`);
	if (state !== "false\ttrue") throw new Error(`${tag} is published but mutable`);
	return "published";
}

export const releaseStateAction = action({
	name: "Inspect release state",
	description: "Validate the current product version against immutable GitHub releases.",
	localActionPath: "ci/release-state",
	inputs: {
		repository: stringInput({ description: "GitHub owner/repository name." }),
		workspace: stringInput({ description: "Checked-out repository root." }),
	},
	outputs: {
		state: stringOutput({ description: "Whether the current version is absent or published." }),
		tag: stringOutput({ description: "Current product release tag." }),
	},
	run: async ({ exec, input }) => {
		if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(input.repository)) {
			throw new Error(`repository must use owner/name form: ${input.repository}`);
		}
		const tag = `v${releaseVersion(input.workspace)}`;
		const releases = await exec("gh", [
			"api",
			"--paginate",
			`repos/${input.repository}/releases?per_page=100`,
			"--jq",
			`.[] | select(.tag_name == "${tag}") | [.draft, .immutable] | @tsv`,
		]);
		return { state: releaseState(releases.stdout, tag), tag };
	},
});
