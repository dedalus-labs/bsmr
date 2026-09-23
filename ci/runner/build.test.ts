//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies trusted workflow routing and the native Mac payload boundary.

import assert from "node:assert/strict";
import { test } from "node:test";
import type { GitHubWorkflowStep } from "@dedalus-labs/hollywood";
import { runnerBuild } from "./build.ts";

test("office builds use main pushes or explicit administrator dispatches", () => {
	assert.deepEqual(Object.keys(runnerBuild.on), ["push", "workflow_dispatch"]);
	assert.deepEqual(runnerBuild.on.push, { branches: ["main"] });
	assert.match(String(runnerBuild.concurrency?.group), /github.event_name == 'push'.*github.run_id/);
	assert.match(runnerBuild.jobs.authorize?.if ?? "", /refs\/heads\/main/);
	assert.equal(runnerBuild.jobs.build?.needs, "authorize");
	assert.match(runnerBuild.jobs.build?.if ?? "", /refs\/heads\/main/);
	assert.deepEqual(runnerBuild.jobs.build?.permissions, { contents: "read", actions: "read" });
});

test("invariant_provider_handoff_keeps_mac_arm64_placement", () => {
	const placement = String(runnerBuild.jobs.build?.["runs-on"]);
	assert.match(placement, /"group":"Dedalus Machines"/);
	assert.match(placement, /macOS.*ARM64.*dedalus-machines/);
	assert.match(placement, /blacksmith-12vcpu-macos-15/);
	assert.match(placement, /macos-15/);
	assert.doesNotMatch(placement, /ubuntu|occ/i);
});

test("invariant_payload_rechecks_ownership_after_queueing", () => {
	const steps: readonly GitHubWorkflowStep[] = runnerBuild.jobs.build.steps;
	const guard = steps.findIndex((step) => "with" in step && step.with?.["operation"] === "authorize");
	const source = steps.findIndex((step) => "with" in step && step.with?.["ref"] === "${{ github.event_name == 'push' && github.sha || inputs.revision }}");
	assert.ok(guard >= 0, "a hosted authorization before queueing cannot authorize delayed execution");
	assert.ok(source > guard, "check out only trusted code before revalidating the parent");
});

test("a fresh runner installs Rust tooling before selecting a compiler", () => {
	const steps: readonly GitHubWorkflowStep[] = runnerBuild.jobs.build.steps;
	const install = steps.findIndex((step) => "uses" in step && step.uses === "./.github/actions/rust/install");
	const compiler = steps.findIndex((step) => step.name === "Install pinned Rust compiler");
	const source = steps.findIndex((step) => "with" in step && step.with?.["ref"] === "${{ github.event_name == 'push' && github.sha || inputs.revision }}");
	assert.ok(install >= 0 && compiler > install);
	assert.ok(install < source, "the approved source need not contain the workflow's installer action");
});

test("only the reviewed workflow revision writes native compiler caches", () => {
	assert.ok(!Object.hasOwn(runnerBuild.jobs.build, "env"), "engine profile overrides must not reach consumer qualification");
	const steps: readonly GitHubWorkflowStep[] = runnerBuild.jobs.build.steps;
	const caches = steps.filter((step) => "uses" in step && step.uses.startsWith("Swatinem/rust-cache@"));
	assert.equal(caches.length, 2);
	for (const cache of caches) {
		assert.ok("uses" in cache);
		assert.equal(cache.with?.["save-if"], "${{ (github.event_name == 'push' && github.sha || inputs.revision) == github.sha }}");
	}
});

test("invariant_every_dispatch_has_independent_cancellation_cleanup", () => {
	const steps: readonly GitHubWorkflowStep[] = runnerBuild.jobs.dispatch?.steps ?? [];
	for (const provider of ["machines", "blacksmith", "github"]) {
		const dispatch = steps.find((step) => step.id === `${provider}_dispatch`);
		const wait = steps.find((step) => step.id === `${provider}_wait`);
		const cleanup = steps.find((step) => "with" in step && step.with?.["operation"] === "cleanup" && String(step.with["run"]).includes(`${provider}_dispatch`));
		assert.ok(dispatch);
		assert.ok(wait);
		assert.match(String(dispatch.name), /^Request /);
		assert.match(String(wait.name), /^Wait for /);
		assert.ok(cleanup);
		assert.equal(cleanup.if, "${{ always() }}");
	}
	assert.match(steps.find((step) => step.id === "blacksmith_dispatch")?.if ?? "", /machines_wait.outputs.result == 'unassigned'/);
	assert.match(steps.find((step) => step.id === "github_dispatch")?.if ?? "", /blacksmith_wait.outputs.result == 'unassigned'/);
});
