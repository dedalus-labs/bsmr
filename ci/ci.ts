//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Defines Bessemer's generated CI workflow.

import {
	GitHubJobResult, always, and, command, eq, expr, format, github, job, needsOutput,
	needsResultIs, not, or, stepOutput, uses, workflow,
	type GitHubJobResultValue,
} from "@dedalus-labs/hollywood";

import { rustAffectedAction } from "./affected.ts";
import { cliReferenceAction } from "./cli-reference.ts";
import { osvAuditAction } from "./osv-audit.ts";
import { verifySha256Action } from "./verify-sha256.ts";

const trustedCiRun = expr<boolean>(
	"github.repository == 'dedalus-labs/bsmr' && (github.event_name != 'pull_request' || github.event.pull_request.head.repo.full_name == github.repository)",
);
const saveRustCache = and(
	eq(github.eventName, "push"),
	eq(github.ref, "refs/heads/main"),
);
const osvScannerUrl =
	"https://github.com/google/osv-scanner/releases/download/v2.4.0/osv-scanner_linux_amd64";
const osvScannerSha256 =
	"15314940c10d26af9c6649f150b8a47c1262e8fc7e17b1d1029b0e479e8ed8a0";
const dotSlashUrl =
	"https://github.com/facebook/dotslash/releases/download/v0.5.9/dotslash-linux-musl.x86_64.v0.5.9.tar.gz";
const dotSlashSha256 =
	"4c75c6eb7890ae35993b962073f6d9bbe78b42b81a5691303ad70f63bfbf7196";
const checkout = {
	name: "Checkout",
	uses: "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
	with: { "persist-credentials": false },
} as const;
const setupNode = {
	name: "Set up Node",
	uses: "actions/setup-node@249970729cb0ef3589644e2896645e5dc5ba9c38",
	with: { "node-version": "26.5.1" },
} as const;
const installRust = {
	name: "Install pinned Rust toolchain",
	run: command({
		file: "rustup",
		args: [
			"toolchain",
			"install",
			"nightly-2026-04-11",
			"--profile",
			"minimal",
			"--component",
			"clippy",
			"--component",
			"llvm-tools-preview",
			"--component",
			"rust-src",
			"--no-self-update",
		],
	}),
} as const;
const rustCache = (save: boolean | typeof saveRustCache) =>
	({
		name: "Restore Rust cache",
		uses: "Swatinem/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32",
		with: {
			"prefix-key": "bsmr-v1",
			"save-if": save,
			"shared-key": "rust",
		},
	}) as const;
const rustEnvironment = {
	CARGO_INCREMENTAL: "0",
} as const;
const rustPermissions = {
	contents: "read",
} as const;
const rustLaneIds = ["rust_audit", "rust_quality", "rust_tests", "rust_self_host"] as const;
const rustAffected = eq(needsOutput("affected", "rust"), "true");
const rustLanesHave = (result: GitHubJobResultValue) =>
	and(
		needsResultIs("rust_audit", result),
		needsResultIs("rust_quality", result),
		needsResultIs("rust_tests", result),
		needsResultIs("rust_self_host", result),
	);
const rustResultsAccepted = and(
	needsResultIs("affected", GitHubJobResult.Success),
	or(
		rustLanesHave(GitHubJobResult.Success),
		rustLanesHave(GitHubJobResult.Skipped),
	),
);
const runnerTemp = expr<string>("runner.temp");
const osvScannerPath = format("{0}/osv-scanner", runnerTemp);
const osvReportPath = format("{0}/osv.json", runnerTemp);
const dotSlashArchivePath = format("{0}/dotslash.tar.gz", runnerTemp);
const addPathProgram = String.raw`const fs = require("node:fs");
const path = process.argv[1];
const output = process.env.GITHUB_PATH;
if (!path || !output) throw new Error("path and GITHUB_PATH are required");
fs.appendFileSync(output, path + "\n");`;
const installOsvScanner = [
	{
		name: "Download pinned OSV Scanner",
		run: command({
			file: "curl",
			args: [
				"--proto",
				"=https",
				"--tlsv1.2",
				"--fail",
				"--location",
				"--silent",
				"--show-error",
				osvScannerUrl,
				"--output",
				osvScannerPath,
			],
		}),
	},
	uses(verifySha256Action, {
		name: "Verify OSV Scanner",
		with: { path: osvScannerPath, expected: osvScannerSha256 },
	}),
	{
		name: "Make OSV Scanner executable",
		run: command({ file: "chmod", args: ["500", osvScannerPath] }),
	},
] as const;
const installDotSlash = [
	{
		name: "Download pinned DotSlash",
		run: command({
			file: "curl",
			args: [
				"--proto",
				"=https",
				"--tlsv1.2",
				"--fail",
				"--location",
				"--silent",
				"--show-error",
				dotSlashUrl,
				"--output",
				dotSlashArchivePath,
			],
		}),
	},
	uses(verifySha256Action, {
		name: "Verify DotSlash",
		with: { path: dotSlashArchivePath, expected: dotSlashSha256 },
	}),
	{
		name: "Extract DotSlash",
		run: command({ file: "tar", args: ["-xzf", dotSlashArchivePath, "-C", runnerTemp] }),
	},
	{
		name: "Add DotSlash to PATH",
		run: command({ file: "node", args: ["-e", addPathProgram, runnerTemp] }),
	},
] as const;

export const ci = workflow({
	name: "CI",
	on: {
		push: { branches: ["main"] },
		pull_request: {},
		merge_group: { types: ["checks_requested"] },
		workflow_dispatch: {},
	},
	permissions: {},
	concurrency: {
		group: format("{0}-{1}", github.workflow, github.ref),
		"cancel-in-progress": true,
	},
	env: {
		FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true,
	},
	jobs: {
		affected: job({
			name: "Affected paths",
			if: trustedCiRun,
			"runs-on": "ubuntu-24.04",
			"timeout-minutes": 5,
			permissions: rustPermissions,
			outputs: { rust: stepOutput<string>("check", "rust") },
			steps: [
				{
					...checkout,
					with: { ...checkout.with, "fetch-depth": 0 },
				},
				uses(rustAffectedAction, {
					id: "check",
					with: {
						eventName: github.eventName,
						baseSha: expr<string>("github.event.pull_request.base.sha"),
						headSha: expr<string>("github.event.pull_request.head.sha"),
					},
				}),
			],
		}),
		dependencies: job({
			name: "Dependency review",
			if: and(trustedCiRun, eq(github.eventName, "pull_request")),
			"runs-on": "ubuntu-24.04",
			"timeout-minutes": 10,
			permissions: { contents: "read" },
			steps: [
				{
					name: "Checkout",
					uses: "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
					with: { "persist-credentials": false },
				},
				{
					name: "Review dependencies",
					uses: "actions/dependency-review-action@a1d282b36b6f3519aa1f3fc636f609c47dddb294",
					with: { "fail-on-severity": "high" },
				},
			],
		}),
		workflows: job({
			name: "Generated workflows",
			if: trustedCiRun,
			"runs-on": "ubuntu-24.04",
			"timeout-minutes": 10,
			permissions: { contents: "read" },
			steps: [
				{
					...checkout,
					with: { ...checkout.with, "fetch-depth": 0 },
				},
				setupNode,
				{
					name: "Set up pnpm",
					run: command({ file: "npm", args: ["install", "--global", "pnpm@10.30.3"] }),
				},
				{
					name: "Install dependencies",
					run: command({
						file: "pnpm",
						args: ["install", "--frozen-lockfile", "--ignore-scripts"],
					}),
				},
				{
					name: "Audit dependencies",
					run: command({ file: "pnpm", args: ["audit", "--audit-level", "high"] }),
				},
				{
					name: "Check workflow source",
					run: command({ file: "pnpm", args: ["run", "ci", "check"] }),
				},
			],
		}),
		rust_audit: job({
			name: "Rust / Dependencies",
			needs: "affected",
			if: and(trustedCiRun, rustAffected),
			"runs-on": "ubuntu-24.04",
			"timeout-minutes": 10,
			permissions: rustPermissions,
			steps: [
				checkout,
				...installOsvScanner,
				uses(osvAuditAction, {
					name: "Audit Rust dependencies",
					with: { scanner: osvScannerPath, report: osvReportPath },
				}),
			],
		}),
		rust_quality: job({
			name: "Rust / Quality",
			needs: "affected",
			if: and(trustedCiRun, rustAffected),
			"runs-on": "blacksmith-8vcpu-ubuntu-2404",
			"timeout-minutes": 30,
			permissions: rustPermissions,
			env: rustEnvironment,
			steps: [
				checkout,
				installRust,
				rustCache(false),
				{
					name: "Lint Rust",
					run: command({ file: "python3", args: ["test.py", "--ci", "--git", "--lint-rust-only"] }),
				},
				{
					name: "Check Rust documentation",
					run: command({ file: "python3", args: ["test.py", "--ci", "--git", "--rustdoc-only"] }),
				},
			],
		}),
		rust_tests: job({
			name: "Rust / Tests",
			needs: "affected",
			if: and(trustedCiRun, rustAffected),
			"runs-on": "blacksmith-16vcpu-ubuntu-2404",
			"timeout-minutes": 30,
			permissions: rustPermissions,
			env: rustEnvironment,
			steps: [
				checkout,
				installRust,
				rustCache(saveRustCache),
				{
					name: "Run Rust tests",
					run: command({ file: "python3", args: ["test.py", "--ci", "--git", "--test-only"] }),
				},
			],
		}),
		rust_self_host: job({
			name: "Rust / Self-host",
			needs: "affected",
			if: and(trustedCiRun, rustAffected),
			"runs-on": "blacksmith-8vcpu-ubuntu-2404",
			"timeout-minutes": 30,
			permissions: rustPermissions,
			env: rustEnvironment,
			steps: [
				checkout,
				installRust,
				rustCache(false),
				{
					name: "Build BSMR",
					run: command({ file: "cargo", args: ["build", "--locked", "--bin", "bsmr"] }),
				},
				...installDotSlash,
				{
					name: "Generate Rust build dependencies",
					run: command({
						file: "./tools/bin/reindeer",
						args: ["--third-party-dir", "tools/build/third-party/rust", "buckify"],
					}),
				},
				{
					name: "Check Starlark",
					run: command({
						file: "python3",
						args: ["test.py", "--ci", "--git", "--bsmr=target/debug/bsmr", "--lint-starlark-only"],
					}),
				},
				{
					name: "Validate application graph",
					run: command({
						file: "target/debug/bsmr",
						args: ["--isolation-dir=ci", "uquery", "deps(//app/...)"],
					}),
				},
				{
					name: "Validate build-support graph",
					run: command({
						file: "target/debug/bsmr",
						args: ["--isolation-dir=ci", "targets", "bsmr_build//..."],
					}),
				},
				uses(cliReferenceAction, {
					name: "Check CLI reference",
					with: { bsmr: "target/debug/bsmr", expected: "docs/reference/cli.md" },
				}),
			],
		}),
		rust: job({
			name: "Rust",
			if: and(always(), trustedCiRun),
			needs: ["affected", ...rustLaneIds],
			"runs-on": "ubuntu-24.04",
			"timeout-minutes": 5,
			permissions: {},
			steps: [
				{
					name: "Accept complete Rust CI",
					if: rustResultsAccepted,
					run: command({ file: "true", args: [] }),
				},
				{
					name: "Reject incomplete Rust CI",
					if: not(rustResultsAccepted),
					run: command({ file: "false", args: [] }),
				},
			],
		}),
	},
});
