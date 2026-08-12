//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies the generated CI workflow contract.

import assert from "node:assert/strict";
import test from "node:test";

import { command, type ScriptExec } from "@dedalus-labs/hollywood";

import { pullRequestFiles, rustAffected, rustAffectedForEvent } from "./affected.ts";
import { ci } from "./ci.ts";
import { docs } from "./docs.ts";

const jobs = ci.jobs;
const rustLanes = [
	"rust_audit",
	"rust_quality",
	"rust_tests",
	"rust_self_host",
	"rust_sandbox",
] as const;
const trustedCiRun =
	"github.repository == 'dedalus-labs/bsmr' && (github.event_name != 'pull_request' || github.event.pull_request.head.repo.full_name == github.repository)";

function unsafeScript(name: string): string {
	const step = jobs.rust_sandbox?.steps.find((candidate) => candidate.name === name);
	assert.ok(step !== undefined && "run" in step);
	assert.equal(step.run.kind, "unsafe-shell");
	return step.run.kind === "unsafe-shell" ? step.run.script : "";
}

test("Rust remains the required aggregate check", () => {
	assert.equal(jobs.rust?.name, "Rust");
	assert.equal(jobs.rust?.["runs-on"], "ubuntu-24.04");
	assert.equal(jobs.rust?.if, `\${{ always() && ${trustedCiRun} }}`);
	assert.deepEqual(jobs.rust?.needs, ["affected", ...rustLanes]);
	const steps = jobs.rust?.steps ?? [];
	assert.deepEqual(steps.map((step) => ("run" in step ? step.run : null)), [
		command({ file: "true", args: [] }),
		command({ file: "false", args: [] }),
	]);
	assert.match(steps[1]?.if ?? "", /!\(needs\.affected\.result == 'success'/);
});

test("Rust lanes run only for trusted affected changes", () => {
	assert.equal(jobs.affected?.["runs-on"], "ubuntu-24.04");
	assert.equal(jobs.affected?.if, `\${{ ${trustedCiRun} }}`);
	assert.equal(jobs.affected?.outputs?.rust, "${{ steps.check.outputs.rust }}");
	const checkout = jobs.affected?.steps[0];
	assert.ok(checkout !== undefined && "with" in checkout);
	assert.deepEqual(checkout.with, {
		"persist-credentials": false,
		"fetch-depth": 0,
		"sparse-checkout": ".github/actions/ci/rust-affected",
	});
	const check = jobs.affected?.steps.at(-1);
	assert.ok(check !== undefined && "uses" in check);
	assert.equal(check.uses, "./.github/actions/ci/rust-affected");
	assert.equal(check.with?.["merge-group-base-sha"], "${{ github.event.merge_group.base_sha }}");
	assert.equal(check.with?.["head-sha"], "${{ github.event.pull_request.head.sha || github.sha }}");
	for (const id of rustLanes) {
		assert.equal(jobs[id]?.needs, "affected");
		assert.equal(
			jobs[id]?.if,
			`\${{ ${trustedCiRun} && needs.affected.outputs.rust == 'true' }}`,
		);
	}
});

test("Rust affected paths fail closed", () => {
	assert.equal(rustAffected([".github/dependabot.yml"]), false);
	assert.equal(rustAffected(["docs/users/getting_started.md"]), false);
	assert.equal(rustAffected(["README.md", ".github/CODEOWNERS"]), false);
	assert.equal(rustAffected(["Cargo.lock"]), true);
	assert.equal(rustAffected(["app/bsmr/src/main.rs"]), true);
	assert.equal(rustAffected(["app/bsmr_core/src/pattern/target_pattern.md"]), true);
	assert.equal(rustAffected(["prelude/rust/rust_binary.bzl"]), true);
	assert.equal(rustAffected(["ci/ci.ts"]), true);
	assert.equal(rustAffected([]), true);
});

test("Pull request paths preserve both sides of a rename", async () => {
	const base = "a".repeat(40);
	const head = "b".repeat(40);
	const mergeBase = "c".repeat(40);
	const calls: string[][] = [];
	const exec: ScriptExec = async (file, args) => {
		calls.push([file, ...args]);
		return {
			exitCode: 0,
			stderr: "",
			stdout:
				args[0] === "merge-base"
					? mergeBase
					: "app/bsmr/src/renamed.rs\0docs/renamed.md\0",
		};
	};
	const files = await pullRequestFiles(exec, base, head);
	assert.equal(rustAffected(files), true);
	assert.deepEqual(calls, [
		["git", "merge-base", base, head],
		["git", "diff", "--name-only", "--no-renames", "-z", mergeBase, head],
	]);
});

test("Pull request paths require immutable commit IDs", async () => {
	const fail: ScriptExec = async () => assert.fail("exec must not run");
	await assert.rejects(pullRequestFiles(fail, "main", "b".repeat(40)), /base SHA/);
});

test("Merge-group paths classify the exact candidate against its base", async () => {
	const base = "a".repeat(40);
	const head = "b".repeat(40);
	const calls: string[][] = [];
	const exec: ScriptExec = async (file, args) => {
		calls.push([file, ...args]);
		return { exitCode: 0, stderr: "", stdout: "docs/users/getting_started.md\0" };
	};
	assert.equal(
		await rustAffectedForEvent(exec, {
			eventName: "merge_group",
			baseSha: "",
			mergeGroupBaseSha: base,
			headSha: head,
		}),
		false,
	);
	assert.deepEqual(calls, [["git", "diff", "--name-only", "--no-renames", "-z", base, head]]);
});

test("Rust compilation uses sized Blacksmith runners", () => {
	assert.equal(jobs.rust_quality?.["runs-on"], "blacksmith-8vcpu-ubuntu-2404");
	assert.equal(jobs.rust_tests?.["runs-on"], "blacksmith-16vcpu-ubuntu-2404");
	assert.equal(jobs.rust_self_host?.["runs-on"], "blacksmith-8vcpu-ubuntu-2404");
	assert.equal(jobs.rust_sandbox?.["runs-on"], "ubuntu-24.04");
	const kvm = unsafeScript("Initialize nested KVM");
	assert.match(kvm, /if ! test -c \/dev\/kvm/);
	assert.match(kvm, /setfacl -m "u:\$\(id -un\):rw" \/dev\/kvm/);
	assert.match(kvm, /sudo modprobe kvm_intel/);
	assert.match(kvm, /sudo modprobe kvm_amd/);
	assert.match(kvm, /test -w \/dev\/kvm/);
	assert.match(
		unsafeScript("Build sandbox components"),
		/sandbox_probe\.rs -o "\$RUNNER_TEMP\/sandbox-probe"/,
	);
	assert.ok(
		jobs.rust_self_host?.steps.some(
			(step) =>
				"run" in step &&
				step.run.kind === "command" &&
				step.run.args.includes("--lint-starlark-only"),
		),
	);
});

test("Firecracker cleanup is gated by acquired resources", () => {
	const steps = jobs.rust_sandbox?.steps ?? [];
	const stop = steps.find((step) => step.name === "Stop isolated launcher");
	const verify = steps.find(
		(step) => step.name === "Verify complete sandbox cleanup",
	);
	const remove = steps.find((step) => step.name === "Remove host sentinel");

	assert.match(stop?.if ?? "", /steps\.sandbox_launcher\.outputs\.started == 'true'/);
	assert.equal(stop?.if, verify?.if);
	assert.match(remove?.if ?? "", /steps\.host_sentinel\.outputs\.created == 'true'/);
});

test("Firecracker installs its bundle beneath the immutable system prefix", () => {
	const steps = jobs.rust_sandbox?.steps ?? [];
	const assemble = steps.find(
		(step) => step.name === "Assemble pinned execution bundle",
	);
	const launcher = steps.find((step) => step.name === "Start isolated launcher");

	assert.ok(assemble !== undefined && "run" in assemble);
	assert.equal(assemble.run.kind, "unsafe-shell");
	assert.match(unsafeScript("Assemble pinned execution bundle"), /\/usr\/local\/share\/bsmr\/firecracker/);
	assert.ok(launcher !== undefined && "run" in launcher);
	assert.equal(launcher.run.kind, "unsafe-shell");
	assert.match(unsafeScript("Start isolated launcher"), /\/usr\/local\/share\/bsmr\/firecracker/);
});

test("self-hosting keeps the CLI reference derived from clap", () => {
	assert.ok(
		jobs.rust_self_host?.steps.some(
			(step) =>
				"uses" in step && step.uses === "./.github/actions/ci/cli-reference",
		),
	);
});

test("workflow checks retain the provenance boundary", () => {
	const workflowCheckout = jobs.workflows?.steps[0];
	assert.ok(workflowCheckout !== undefined && "with" in workflowCheckout);
	assert.ok("fetch-depth" in workflowCheckout.with);
	assert.equal(workflowCheckout.with["fetch-depth"], 0);
});

test("Rust lanes share one trusted cache writer", () => {
	for (const id of ["rust_quality", "rust_tests", "rust_self_host"] as const) {
		const cache = jobs[id].steps.find(
			(step) => "uses" in step && step.uses.startsWith("Swatinem/rust-cache@"),
		);

		assert.ok(cache !== undefined);
		assert.ok("with" in cache);
		assert.ok("shared-key" in cache.with);
		assert.equal(cache.with["shared-key"], "rust");
		assert.equal(
			cache.with["save-if"],
			id === "rust_tests"
				? "${{ github.event_name == 'push' && github.ref == 'refs/heads/main' }}"
				: false,
		);
	}
});

test("docs deploy only from a trusted main build", () => {
	assert.deepEqual(docs.on.pull_request, {
		branches: ["main"],
		paths: [
			".github/workflows/docs.yml",
			"ci/docs.test.ts",
			"ci/docs.ts",
			"docs/**",
			"mkdocs.yml",
			"README.md",
		],
	});
	assert.equal(docs.jobs.build?.if, `\${{ ${trustedCiRun} }}`);
	assert.equal(
		docs.jobs.deploy?.if,
		"github.event_name == 'push' && github.ref == 'refs/heads/main'",
	);
	assert.equal(docs.jobs.deploy?.needs, "build");
	assert.deepEqual(docs.jobs.deploy?.permissions, {
		pages: "write",
		"id-token": "write",
	});
});
