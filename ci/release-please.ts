//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Prepares release pull requests only after the current version is immutable.

import { eq, expr, job, stepOutput, uses, workflow } from "@dedalus-labs/hollywood";

import { releaseStateAction } from "./release-state.ts";
import { releaseSyncAction } from "./release-sync.ts";

const releasePleaseAction =
	"googleapis/release-please-action@45996ed1f6d02564a971a2fa1b5860e934307cf7"; // v5.0.0
const checkoutAction = "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1"; // v7.0.1
const pullRequestCreated = eq(stepOutput("release", "prs_created"), "true");
const currentReleasePublished = eq(stepOutput("state", "state"), "published");
const releaseBranch = expr<string>("fromJSON(steps.release.outputs.pr || '{}').headBranchName");

export const releasePlease = workflow({
	name: "Release Please",
	on: {
		push: { branches: ["main"] },
		workflow_dispatch: {},
	},
	permissions: {},
	concurrency: {
		group: "release-please",
		"cancel-in-progress": false,
	},
	jobs: {
		prepare: job({
			name: "Prepare release",
			"runs-on": "ubuntu-24.04",
			"timeout-minutes": 10,
			permissions: {
				actions: "write",
				contents: "write",
				issues: "write",
				"pull-requests": "write",
			},
			steps: [
				{
					name: "Checkout",
					uses: checkoutAction,
					with: { "persist-credentials": false },
				},
				uses(releaseStateAction, {
					id: "state",
					env: { GH_TOKEN: expr<string>("github.token") },
					with: {
						repository: expr<string>("github.repository"),
						workspace: expr<string>("github.workspace"),
					},
				}),
				{
					id: "release",
					name: "Prepare release",
					if: currentReleasePublished,
					uses: releasePleaseAction,
					with: {
						"config-file": "release-please-config.json",
						"manifest-file": ".release-please-manifest.json",
					},
				},
				{
					name: "Checkout release pull request",
					if: pullRequestCreated,
					uses: checkoutAction,
					with: {
						ref: releaseBranch,
						"persist-credentials": false,
					},
				},
				uses(releaseSyncAction, {
					name: "Synchronize release version",
					if: pullRequestCreated,
					env: {
						GH_TOKEN: expr<string>("github.token"),
					},
					with: {
						branch: releaseBranch,
						workspace: expr<string>("github.workspace"),
					},
				}),
			],
		}),
	},
});
