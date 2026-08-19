//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Defines Bessemer's generated documentation build and deployment workflow.

import { command, expr, job, workflow } from "@dedalus-labs/hollywood";

const checkoutAction =
	"actions/checkout@df4cb1c069e1874edd31b4311f1884172cec0e10"; // v6.0.3
const deployPagesAction =
	"actions/deploy-pages@cd2ce8fcbc39b97be8ca5fce6e763baed58fa128"; // v5.0.0
const setupPythonAction =
	"actions/setup-python@a309ff8b426b58ec0e2a45f0f869d46889d02405"; // v6.2.0
const uploadPagesArtifactAction =
	"actions/upload-pages-artifact@7b1f4a764d45c48632c6b24a0339c27f5614fb0b"; // v4.0.0

const docsPaths = [
	".github/workflows/docs.yml",
	"ci/docs.test.ts",
	"ci/docs.ts",
	"docs/**",
	"mkdocs.yml",
	"README.md",
] as const;
const trustedCiRun = expr<boolean>(
	"github.repository == 'dedalus-labs/bsmr' && (github.event_name != 'pull_request' || github.event.pull_request.head.repo.full_name == github.repository)",
);

export const docs = workflow({
	name: "Docs",
	on: {
		push: { branches: ["main"], paths: docsPaths },
		pull_request: { branches: ["main"], paths: docsPaths },
		workflow_dispatch: {},
	},
	concurrency: {
		group: "pages-${{ github.ref }}",
		"cancel-in-progress": false,
	},
	permissions: { contents: "read" },
	jobs: {
		build: job({
			name: "Build",
			if: trustedCiRun,
			"runs-on": "blacksmith-2vcpu-ubuntu-2404",
			"timeout-minutes": 10,
			steps: [
				{ uses: checkoutAction, with: { "persist-credentials": false } },
				{ uses: setupPythonAction, with: { "python-version": "3.13" } },
				{
					name: "Install documentation dependencies",
					run: command({ file: "python", args: ["-m", "pip", "install", "-r", "docs/requirements.txt"] }),
				},
				{
					name: "Build documentation",
					run: command({ file: "python", args: ["-m", "mkdocs", "build", "--strict", "-f", "mkdocs.yml"] }),
				},
				{ name: "Upload Pages artifact", uses: uploadPagesArtifactAction, with: { path: "site" } },
			],
		}),
		deploy: job({
			name: "Deploy",
			needs: "build",
			if: "github.event_name == 'push' && github.ref == 'refs/heads/main'",
			"runs-on": "ubuntu-24.04",
			"timeout-minutes": 10,
			permissions: {
				pages: "write",
				"id-token": "write",
			},
			environment: {
				name: "github-pages",
				url: "${{ steps.deployment.outputs.page_url }}",
			},
			steps: [{ id: "deployment", name: "Deploy to GitHub Pages", uses: deployPagesAction }],
		}),
	},
});
