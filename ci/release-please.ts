//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Defines release preparation and dispatch through Release Please and dist.

import { command, eq, expr, format, job, stepOutput, uses, workflow } from "@dedalus-labs/hollywood";

import { releaseSyncAction } from "./release-sync.ts";

const releasePleaseAction =
	"googleapis/release-please-action@45996ed1f6d02564a971a2fa1b5860e934307cf7"; // v5.0.0
const checkoutAction = "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1"; // v7.0.1
const releaseCreated = eq(stepOutput("release", "release_created"), "true");
const pullRequestCreated = eq(stepOutput("release", "prs_created"), "true");
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
					id: "release",
					name: "Prepare release",
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
				{
					name: "Run release pull request checks",
					if: pullRequestCreated,
					env: {
						GH_TOKEN: expr<string>("github.token"),
					},
					run: command({
						file: "gh",
						args: [
							"workflow",
							"run",
							"ci.yml",
							"--repo",
							expr<string>("github.repository"),
							"--ref",
							releaseBranch,
						],
					}),
				},
				{
					name: "Build and publish release",
					if: releaseCreated,
					env: {
						GH_TOKEN: expr<string>("github.token"),
					},
					run: command({
						file: "gh",
						args: [
							"workflow",
							"run",
							"release.yml",
							"--repo",
							expr<string>("github.repository"),
							"--ref",
							stepOutput("release", "tag_name"),
							"--field",
							format("tag={0}", stepOutput("release", "tag_name")),
						],
					}),
				},
			],
		}),
	},
});
