//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Defines Bessemer's generated CI workflow.

import {
	GitHubJobResult, always, and, command, eq, expr, format, github, job, needsOutput,
	needsResultIs, not, or, stepOutput, unsafeShell, uses, workflow,
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
const firecrackerVersion = "1.16.1";
const firecrackerArchiveUrl = `https://github.com/firecracker-microvm/firecracker/releases/download/v${firecrackerVersion}/firecracker-v${firecrackerVersion}-x86_64.tgz`;
const firecrackerArchiveSha256 =
	"382a02a869e4d6d5cb14c40577f9545e8458021ea8b0b2d3fc10ec14d9c242e6";
const firecrackerKernelUrl =
	"https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/v1.15/x86_64/vmlinux-6.1.155";
const firecrackerKernelSha256 =
	"e20e46d0c36c55c0d1014eb20576171b3f3d922260d9f792017aeff53af3d4f2";
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
const uploadArtifact = {
	name: "Upload raw Firecracker benchmarks",
	uses: "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02",
	with: {
		"if-no-files-found": "error",
		name: "firecracker-startup-benchmarks",
		path: format("{0}/*-startup.json", expr<string>("runner.temp")),
		"retention-days": 30,
	},
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
const rustLaneIds = [
	"rust_audit",
	"rust_quality",
	"rust_tests",
	"rust_self_host",
	"rust_sandbox",
] as const;
const rustAffected = eq(needsOutput("affected", "rust"), "true");
const rustLanesHave = (result: GitHubJobResultValue) =>
	and(
		needsResultIs("rust_audit", result),
		needsResultIs("rust_quality", result),
		needsResultIs("rust_tests", result),
		needsResultIs("rust_self_host", result),
		needsResultIs("rust_sandbox", result),
	);
const rustResultsAccepted = and(
	needsResultIs("affected", GitHubJobResult.Success),
	or(
		rustLanesHave(GitHubJobResult.Success),
		rustLanesHave(GitHubJobResult.Skipped),
	),
);
const runnerTemp = expr<string>("runner.temp");
const firecrackerTest = (name: string) =>
	command({
		file: "cargo",
		args: [
			"test",
			"--locked",
			"-p",
			"bsmr_sandbox",
			"--test",
			"firecracker",
			name,
			"--",
			"--ignored",
			"--exact",
			"--nocapture",
		],
	});
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
					run: command({ file: "node", args: ["ci/rust-build-dependencies.mjs"] }),
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
		rust_sandbox: job({
			name: "Rust / Firecracker sandbox",
			needs: "affected",
			if: and(trustedCiRun, rustAffected),
			"runs-on": "ubuntu-24.04",
			"timeout-minutes": 20,
			permissions: rustPermissions,
			env: rustEnvironment,
			steps: [
				checkout,
				installRust,
				rustCache(false),
				{
					name: "Initialize nested KVM",
					run: unsafeShell([
						'if ! test -c /dev/kvm; then sudo modprobe kvm; if grep -qw vmx /proc/cpuinfo; then sudo modprobe kvm_intel; elif grep -qw svm /proc/cpuinfo; then sudo modprobe kvm_amd; else echo "x86_64 CPU exposes neither VMX nor SVM" >&2; exit 1; fi; fi',
						'sudo setfacl -m "u:$(id -un):rw" /dev/kvm',
						"test -c /dev/kvm",
						"test -r /dev/kvm",
						"test -w /dev/kvm",
					].join("\n")),
				},
				{
					name: "Build sandbox components",
					run: unsafeShell([
						"rustup target add x86_64-unknown-linux-musl",
						"cargo build --locked -p bsmr_sandbox --bin bsmr-sandboxd --bin bsmr-sandbox-bundle",
						"cargo build --locked -p bsmr_sandbox --bin bsmr-sandbox-guest --target x86_64-unknown-linux-musl",
						'rustc --target x86_64-unknown-linux-musl test/fixtures/sandbox_probe.rs -o "$RUNNER_TEMP/sandbox-probe"',
					].join("\n")),
				},
				{
					name: "Assemble pinned execution bundle",
					run: unsafeShell([
						'mkdir -p "$RUNNER_TEMP/firecracker-release" "$RUNNER_TEMP/firecracker-bundle" "$RUNNER_TEMP/firecracker-rootfs/dev" "$RUNNER_TEMP/firecracker-rootfs/proc" "$RUNNER_TEMP/firecracker-rootfs/sbin" "$RUNNER_TEMP/firecracker-rootfs/workspace"',
						`curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error ${firecrackerArchiveUrl} --output "$RUNNER_TEMP/firecracker.tgz"`,
						`echo "${firecrackerArchiveSha256}  $RUNNER_TEMP/firecracker.tgz" | sha256sum --check`,
						'tar -xzf "$RUNNER_TEMP/firecracker.tgz" -C "$RUNNER_TEMP/firecracker-release"',
						`install -m 0555 "$RUNNER_TEMP/firecracker-release/release-v${firecrackerVersion}-x86_64/firecracker-v${firecrackerVersion}-x86_64" "$RUNNER_TEMP/firecracker-bundle/firecracker"`,
						`install -m 0555 "$RUNNER_TEMP/firecracker-release/release-v${firecrackerVersion}-x86_64/jailer-v${firecrackerVersion}-x86_64" "$RUNNER_TEMP/firecracker-bundle/jailer"`,
						`curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error ${firecrackerKernelUrl} --output "$RUNNER_TEMP/firecracker-bundle/kernel"`,
						`echo "${firecrackerKernelSha256}  $RUNNER_TEMP/firecracker-bundle/kernel" | sha256sum --check`,
						'install -m 0555 target/x86_64-unknown-linux-musl/debug/bsmr-sandbox-guest "$RUNNER_TEMP/firecracker-rootfs/sbin/bsmr-sandbox-guest"',
						'truncate --size 64M "$RUNNER_TEMP/firecracker-bundle/rootfs"',
						'mkfs.ext4 -q -F -d "$RUNNER_TEMP/firecracker-rootfs" "$RUNNER_TEMP/firecracker-bundle/rootfs"',
						`target/debug/bsmr-sandbox-bundle --directory "$RUNNER_TEMP/firecracker-bundle" --firecracker-version ${firecrackerVersion} --architecture x86_64`,
						'sudo install -d -o root -g root -m 0755 /usr/local/share/bsmr/firecracker /usr/local/libexec',
						'test "$(stat -c %d "$RUNNER_TEMP/firecracker-bundle")" = "$(stat -c %d /usr/local/share/bsmr/firecracker)"',
						'sudo ln "$RUNNER_TEMP/firecracker-bundle/manifest.json" "$RUNNER_TEMP/firecracker-bundle/kernel" "$RUNNER_TEMP/firecracker-bundle/rootfs" "$RUNNER_TEMP/firecracker-bundle/snapshot" "$RUNNER_TEMP/firecracker-bundle/memory" "$RUNNER_TEMP/firecracker-bundle/firecracker" "$RUNNER_TEMP/firecracker-bundle/jailer" /usr/local/share/bsmr/firecracker/',
						'sudo chown root:root /usr/local/share/bsmr/firecracker/manifest.json /usr/local/share/bsmr/firecracker/kernel /usr/local/share/bsmr/firecracker/rootfs /usr/local/share/bsmr/firecracker/snapshot /usr/local/share/bsmr/firecracker/memory /usr/local/share/bsmr/firecracker/firecracker /usr/local/share/bsmr/firecracker/jailer',
						'sudo chmod 0444 /usr/local/share/bsmr/firecracker/manifest.json /usr/local/share/bsmr/firecracker/kernel /usr/local/share/bsmr/firecracker/rootfs /usr/local/share/bsmr/firecracker/snapshot /usr/local/share/bsmr/firecracker/memory',
						'sudo chmod 0555 /usr/local/share/bsmr/firecracker/firecracker /usr/local/share/bsmr/firecracker/jailer',
						"sudo install -o root -g root -m 0555 target/debug/bsmr-sandboxd /usr/local/libexec/bsmr-sandboxd",
					].join("\n")),
				},
				{
					name: "Create host sentinel",
					id: "host_sentinel",
					run: unsafeShell([
						"! getent passwd 61000",
						"! getent group 61000",
						"sudo install -o root -g root -m 0400 /dev/null /bsmr-host-sentinel",
						'echo "created=true" >> "$GITHUB_OUTPUT"',
					].join("\n")),
				},
				{
					name: "Start fresh-boot oracle",
					id: "fresh_launcher",
					run: unsafeShell([
						'sudo systemd-run --unit=bsmr-sandboxd-fresh --property=KillMode=control-group -- /usr/local/libexec/bsmr-sandboxd --bundle /usr/local/share/bsmr/firecracker/manifest.json --socket /run/bsmr/sandboxd-fresh.sock --jail-root /var/lib/bsmr/jailer --uid-base 61000 --gid-base 61000 --socket-gid "$(id -g)" --max-vms 4 --boot-mode fresh',
						'echo "started=true" >> "$GITHUB_OUTPUT"',
					].join("\n")),
				},
				{
					name: "Run fresh-boot conformance corpus",
					env: {
						BSMR_SANDBOX_BUNDLE:
							"/usr/local/share/bsmr/firecracker/manifest.json",
						BSMR_SANDBOX_PROBE: format(
							"{0}/sandbox-probe",
							expr<string>("runner.temp"),
						),
						BSMR_SANDBOX_SOCKET: "/run/bsmr/sandboxd-fresh.sock",
					},
					run: firecrackerTest("firecracker_conformance"),
				},
				{
					name: "Benchmark fresh-boot oracle",
					env: {
						BSMR_SANDBOX_BENCHMARK_OUT: format(
							"{0}/fresh-startup.json",
							expr<string>("runner.temp"),
						),
						BSMR_SANDBOX_BUNDLE:
							"/usr/local/share/bsmr/firecracker/manifest.json",
						BSMR_SANDBOX_MODE: "fresh",
						BSMR_SANDBOX_PROBE: format(
							"{0}/sandbox-probe",
							expr<string>("runner.temp"),
						),
						BSMR_SANDBOX_SOCKET: "/run/bsmr/sandboxd-fresh.sock",
					},
					run: firecrackerTest("firecracker_startup_benchmark"),
				},
				{
					name: "Stop fresh-boot oracle",
					if: and(
						always(),
						eq(stepOutput("fresh_launcher", "started"), "true"),
					),
					run: command({
						file: "sudo",
						args: ["systemctl", "stop", "bsmr-sandboxd-fresh.service"],
					}),
				},
				{
					name: "Verify fresh-boot cleanup",
					if: and(
						always(),
						eq(stepOutput("fresh_launcher", "started"), "true"),
					),
					run: unsafeShell([
						"sudo journalctl --unit=bsmr-sandboxd-fresh.service --no-pager",
						'test -z "$(sudo find /var/lib/bsmr/jailer -mindepth 2 -print -quit)"',
						'if test -d /sys/fs/cgroup/bsmr; then test -z "$(sudo find /sys/fs/cgroup/bsmr -mindepth 1 -type d -print -quit)"; fi',
					].join("\n")),
				},
				{
					name: "Start isolated launcher",
					id: "sandbox_launcher",
					run: unsafeShell([
						'sudo systemd-run --unit=bsmr-sandboxd --property=KillMode=control-group -- /usr/local/libexec/bsmr-sandboxd --bundle /usr/local/share/bsmr/firecracker/manifest.json --socket /run/bsmr/sandboxd.sock --jail-root /var/lib/bsmr/jailer --uid-base 61000 --gid-base 61000 --socket-gid "$(id -g)" --max-vms 4',
						'echo "started=true" >> "$GITHUB_OUTPUT"',
					].join("\n")),
				},
				{
					name: "Run real-microVM conformance corpus",
					env: {
						BSMR_SANDBOX_BUNDLE: "/usr/local/share/bsmr/firecracker/manifest.json",
						BSMR_SANDBOX_PROBE: format("{0}/sandbox-probe", runnerTemp),
						BSMR_SANDBOX_SOCKET: "/run/bsmr/sandboxd.sock",
					},
					run: firecrackerTest("firecracker_conformance"),
				},
				{
					name: "Benchmark snapshot restoration",
					env: {
						BSMR_SANDBOX_BENCHMARK_OUT: format(
							"{0}/snapshot-startup.json",
							expr<string>("runner.temp"),
						),
						BSMR_SANDBOX_BUNDLE:
							"/usr/local/share/bsmr/firecracker/manifest.json",
						BSMR_SANDBOX_MODE: "snapshot",
						BSMR_SANDBOX_PROBE: format(
							"{0}/sandbox-probe",
							expr<string>("runner.temp"),
						),
						BSMR_SANDBOX_SOCKET: "/run/bsmr/sandboxd.sock",
					},
					run: firecrackerTest("firecracker_startup_benchmark"),
				},
				uploadArtifact,
				{
					name: "Enforce snapshot speedup",
					env: {
						BSMR_SANDBOX_FRESH_BENCHMARK: format(
							"{0}/fresh-startup.json",
							expr<string>("runner.temp"),
						),
						BSMR_SANDBOX_SNAPSHOT_BENCHMARK: format(
							"{0}/snapshot-startup.json",
							expr<string>("runner.temp"),
						),
					},
					run: firecrackerTest("firecracker_snapshot_speedup"),
				},
				{
					name: "Stop isolated launcher",
					if: and(always(), eq(stepOutput("sandbox_launcher", "started"), "true")),
					run: command({
						file: "sudo",
						args: ["systemctl", "stop", "bsmr-sandboxd.service"],
					}),
				},
				{
					name: "Verify complete sandbox cleanup",
					if: and(always(), eq(stepOutput("sandbox_launcher", "started"), "true")),
					run: unsafeShell([
						"sudo journalctl --unit=bsmr-sandboxd.service --no-pager",
						'test -z "$(sudo find /var/lib/bsmr/jailer -mindepth 2 -print -quit)"',
						'if test -d /sys/fs/cgroup/bsmr; then test -z "$(sudo find /sys/fs/cgroup/bsmr -mindepth 1 -type d -print -quit)"; fi',
					].join("\n")),
				},
				{
					name: "Remove host sentinel",
					if: and(always(), eq(stepOutput("host_sentinel", "created"), "true")),
					run: command({ file: "sudo", args: ["unlink", "/bsmr-host-sentinel"] }),
				},
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
