//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Routes administrator-approved Mac builds through office and hosted capacity.

import { always, and, command, eq, expr, github, input, job, ne, stepOutput, uses, workflow } from "@dedalus-labs/hollywood";
import { runnerAction } from "./action.ts";
import { providers } from "./api.ts";
import { installRust } from "./rust.ts";

const checkout = "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1";
const token = { GH_TOKEN: expr<string>("github.token") };
const source = expr<string>("github.event_name == 'push' && github.sha || inputs.revision");
const provider = expr<string>("github.event_name == 'push' && 'auto' || inputs.provider");
const names = { machines: "Dedalus Machines", blacksmith: "Blacksmith", github: "GitHub" };
const placement = { machines: { group: "Dedalus Machines", labels: ["self-hosted", "macOS", "ARM64", "dedalus-machines"] }, blacksmith: "blacksmith-12vcpu-macos-15", github: "macos-15" };
/** Engine compilation settings that must not reach consumer qualification. */
export const buildEnvironment = { CARGO_PROFILE_DEV_DEBUG: "0", CARGO_INCREMENTAL: "0" };
const trustedDefinition = expr<boolean>("github.repository == 'dedalus-labs/bsmr' && github.ref == 'refs/heads/main'");
const authorize = uses(runnerAction, {
	name: "Verify build ownership",
	with: {
		operation: "authorize", provider, source, definition: input("definition"), actualDefinition: github.sha,
		repository: github.repository, event: github.eventName, ref: github.ref,
		actor: expr("github.triggering_actor"), parent: input("parent"),
	},
	env: token,
});

export const runnerBuild = workflow({
	name: "Build Rust on Mac",
	"run-name": expr<string>("format('Rust build {0}', github.event_name == 'push' && github.sha || inputs.revision)"),
	concurrency: {
		group: expr<string>("github.event_name == 'push' && 'rust-mac-main' || format('rust-mac-{0}', github.run_id)"),
		"cancel-in-progress": true,
	},
	on: { push: { branches: ["main"] }, workflow_dispatch: { inputs: {
		revision: { description: "Exact source SHA approved by the administrator.", type: "string", required: true },
		provider: { description: "Capacity provider, or automatic selection.", type: "choice", options: ["auto", ...providers], default: "auto" },
		definition: { description: "Parent workflow definition SHA for owned child dispatches.", type: "string", default: "" },
		parent: { description: "Parent workflow run ID for owned child dispatches.", type: "string", default: "" },
	} } },
	permissions: { contents: "read" },
	jobs: {
		authorize: job({
			name: "Verify build authorization", if: trustedDefinition, "runs-on": "ubuntu-24.04", "timeout-minutes": 5,
			permissions: { contents: "read", actions: "read" },
			steps: [
				{ uses: checkout, with: { ref: github.sha, "persist-credentials": false } },
				authorize,
			],
		}),
		dispatch: job({
			name: "Select Mac build capacity", needs: "authorize", if: eq(provider, "auto"),
			"runs-on": "ubuntu-24.04", "timeout-minutes": 75, permissions: { contents: "read", actions: "write" },
			steps: [
				{ uses: checkout, with: { ref: github.sha, "persist-credentials": false } },
				...providers.flatMap((selected, index) => {
					const previous = providers[index - 1];
					const eligible = previous === undefined ? expr<boolean>("success()") : eq(stepOutput(`${previous}_wait`, "result"), "unassigned");
					return [
						uses(runnerAction, { id: `${selected}_dispatch`, name: `Request ${names[selected]} runner`, if: eligible, with: { operation: "dispatch", provider: selected, source, definition: github.sha, parent: github.runId }, env: token }),
						uses(runnerAction, { id: `${selected}_wait`, name: `Wait for ${names[selected]} build`, if: eligible, with: { operation: "wait", provider: selected, run: stepOutput(`${selected}_dispatch`, "id") }, env: token }),
						uses(runnerAction, { name: `Clean up ${names[selected]} attempt`, if: always(), with: { operation: "cleanup", run: stepOutput(`${selected}_dispatch`, "id") }, env: token }),
					];
				}),
				uses(runnerAction, { name: "Require a successful build", with: { operation: "complete", result: expr("steps.github_wait.outputs.result || steps.blacksmith_wait.outputs.result || steps.machines_wait.outputs.result") }, env: token }),
			],
		}),
		build: job({
			name: "Build Rust", needs: "authorize", if: and(trustedDefinition, ne(provider, "auto")),
			"runs-on": expr(`fromJSON('${JSON.stringify(placement)}')[inputs.provider]`), "timeout-minutes": 60,
			permissions: { contents: "read", actions: "read" },
			steps: [
				{ uses: checkout, with: { ref: github.sha, "persist-credentials": false } },
				authorize,
				{ uses: "actions/setup-node@249970729cb0ef3589644e2896645e5dc5ba9c38", with: { "node-version": "26.5.1" } },
				{ name: "Verify native Mac architecture", run: command({ file: "node", args: ["-e", "const a = require('node:assert/strict'); a.equal(process.platform, 'darwin'); a.equal(process.arch, 'arm64');"] }) },
				uses(installRust, { with: {} }),
				{ uses: checkout, with: { ref: source, "persist-credentials": false } },
				{ name: "Install pinned Rust compiler", run: command({ file: "rustup", args: ["toolchain", "install", "nightly-2026-04-11", "--profile", "minimal", "--no-self-update"] }) },
				{
					name: "Restore engine cache",
					uses: "Swatinem/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32",
					env: buildEnvironment,
					with: { "prefix-key": "bsmr-v2", "shared-key": "rust", "cache-bin": false, "save-if": eq(source, github.sha) },
				},
				{ name: "Build BSMR", env: buildEnvironment, run: command({ file: "cargo", args: ["build", "--locked", "--bin", "bsmr", "-j", "2"] }) },
				{ name: "Install planner compiler", run: command({ file: "rustup", args: ["toolchain", "install", "1.98.0", "--profile", "minimal", "--no-self-update"] }) },
				{
					name: "Restore planner cache",
					uses: "Swatinem/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32",
					env: { ...buildEnvironment, RUSTUP_TOOLCHAIN: "1.98.0" },
					with: { "prefix-key": "bsmr-v1", "shared-key": "planner", "cache-bin": false, workspaces: "tools/cargo -> target", "save-if": eq(source, github.sha) },
				},
				{ name: "Build Cargo planner", env: buildEnvironment, run: command({ file: "rustup", args: ["run", "1.98.0", "cargo", "build", "--locked", "--manifest-path", "tools/cargo/Cargo.toml", "--target-dir", "tools/cargo/target", "-j", "2"] }) },
				{ name: "Install Cargo planner", run: command({ file: "cp", args: ["tools/cargo/target/debug/bsmr-cargo", "target/debug/bsmr-cargo"] }) },
				{ name: "Install qualification compiler", run: command({ file: "rustup", args: ["toolchain", "install", "1.97.1", "--profile", "minimal", "--no-self-update"] }) },
				{ name: "Verify compiler archive metadata", run: command({ file: "python3", args: ["test/rust/catalog.py", "target/debug/bsmr"] }) },
				{ name: "Verify native Rust builds", run: command({ file: "node", args: ["test/native-rust-build.ts", "target/debug/bsmr"] }) },
				{ name: "Verify configured Cargo builds", run: command({ file: "node", args: ["test/rust/configured.ts", "target/debug/bsmr"] }) },
				{ name: "Verify joint Cargo selections", run: command({ file: "python3", args: ["test/rust/roots.py", "target/debug/bsmr"] }) },
				{ name: "Verify Rust library formats", run: command({ file: "node", args: ["test/rust/libraries.ts", "target/debug/bsmr"] }) },
				{ name: "Verify Cargo integration tests", run: command({ file: "python3", args: ["test/rust/tests.py", "target/debug/bsmr"] }) },
			],
		}),
	},
});
