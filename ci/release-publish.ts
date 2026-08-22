//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Publishes one validated product version only after dist builds every artifact.

import {
	command,
	eq,
	expr,
	format,
	job,
	stepOutput,
	uses,
	workflow,
} from "@dedalus-labs/hollywood";

import { releaseStateAction } from "./release-state.ts";

const currentReleaseAbsent = eq(stepOutput("state", "state"), "absent");
const checkoutAction = "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1"; // v7.0.1

export const publishRelease = workflow({
	name: "Publish release",
	on: {
		push: { branches: ["main"], paths: [".release-please-manifest.json"] },
		workflow_dispatch: {},
	},
	permissions: {},
	concurrency: { group: "publish-release", "cancel-in-progress": false },
	jobs: {
		publish: job({
			name: "Publish current version",
			"runs-on": "ubuntu-24.04",
			"timeout-minutes": 10,
			permissions: { actions: "write", contents: "read" },
			steps: [
				{ uses: checkoutAction, with: { ref: "main", "persist-credentials": false } },
				uses(releaseStateAction, {
					id: "state",
					env: { GH_TOKEN: expr<string>("github.token") },
					with: {
						repository: expr<string>("github.repository"),
						workspace: expr<string>("github.workspace"),
					},
				}),
				{
					name: "Build and publish release",
					if: currentReleaseAbsent,
					env: { GH_TOKEN: expr<string>("github.token") },
					run: command({
						file: "gh",
						args: [
							"workflow",
							"run",
							"release.yml",
							"--repo",
							expr<string>("github.repository"),
							"--ref",
							"main",
							"--field",
							format("tag={0}", stepOutput("state", "tag")),
						],
					}),
				},
			],
		}),
	},
});
