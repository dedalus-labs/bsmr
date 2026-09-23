//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies trusted workflow routing and the native Mac payload boundary.

import assert from "node:assert/strict";
import { test } from "node:test";
import type { GitHubWorkflowStep } from "@dedalus-labs/hollywood";
import { runnerBuild } from "./build.ts";

test("invariant_office_builds_require_a_dispatch_of_the_trusted_definition", () => {
	assert.deepEqual(Object.keys(runnerBuild.on), ["workflow_dispatch"]);
	assert.match(runnerBuild.jobs.authorize?.if ?? "", /refs\/heads\/main/);
	assert.equal(runnerBuild.jobs.build?.needs, "authorize");
	assert.match(runnerBuild.jobs.build?.if ?? "", /refs\/heads\/main/);
	assert.deepEqual(runnerBuild.jobs.build?.permissions, { contents: "read", actions: "read" });
});

test("invariant_provider_handoff_keeps_mac_arm64_placement", () => {
	const placement = String(runnerBuild.jobs.build?.["runs-on"]);
	assert.match(placement, /macOS.*ARM64.*dedalus-machines/);
	assert.match(placement, /blacksmith-12vcpu-macos-15/);
	assert.match(placement, /macos-15/);
	assert.doesNotMatch(placement, /ubuntu|occ/i);
});

test("invariant_payload_rechecks_ownership_after_queueing", () => {
	const steps: readonly GitHubWorkflowStep[] = runnerBuild.jobs.build.steps;
	const guard = steps.findIndex((step) => "with" in step && step.with?.["operation"] === "authorize");
	const source = steps.findIndex((step) => "with" in step && step.with?.["ref"] === "${{ inputs.revision }}");
	assert.ok(guard >= 0, "a hosted authorization before queueing cannot authorize delayed execution");
	assert.ok(source > guard, "check out only trusted code before revalidating the parent");
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
