//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies queue overflow without executing or replaying an accepted payload.

import assert from "node:assert/strict";
import { test } from "node:test";
import { cancelActiveRun, canHandoff, waitForRun, type Job, type Run, type RunIo } from "./lifecycle.ts";

const queued: Run = { id: 1, status: "queued", conclusion: null, html_url: "https://github.invalid/run/1" };
const pending: Job = { status: "queued", conclusion: null, runner_id: null, steps: [] };
const cancelled = { ...queued, status: "completed", conclusion: "cancelled" };
const cancelledJob = { ...pending, status: "completed", conclusion: "cancelled" };
const deadline = { queueSeconds: 60, totalSeconds: 3600 };

/** Model one owned run and a clock that advances only at observation boundaries. */
function fixture() {
	const state = { now: 0, cancellations: 0, run: queued, jobs: [pending] };
	const io: RunIo = {
		readRun: async () => state.run,
		readJobs: async () => state.jobs,
		cancel: async () => { state.cancellations++; state.run = cancelled; state.jobs = [cancelledJob]; },
		now: () => state.now,
		sleep: async () => { state.now += 15; },
	};
	return { state, io };
}

test("invariant_overflow_requires_confirmed_unassigned_cancellation", async () => {
	const { state, io } = fixture();
	assert.equal(await waitForRun(1, io, deadline), "unassigned");
	assert.equal(state.now, 60);
	assert.equal(state.cancellations, 1);
	assert.equal(canHandoff(cancelled, []), false);
	assert.equal(canHandoff(cancelled, [pending]), false);
});

test("invariant_terminal_failure_is_never_replayed", async () => {
	for (const conclusion of ["failure", "cancelled", "timed_out", "skipped", "neutral"]) {
		const { state, io } = fixture();
		state.run = { ...cancelled, conclusion };
		state.jobs = [{ ...cancelledJob, conclusion }];
		await assert.rejects(waitForRun(1, io, deadline), /runner payload/);
		assert.equal(state.cancellations, 0);
	}
});

test("invariant_assignment_during_cancellation_prevents_overflow", async () => {
	for (const job of [{ ...cancelledJob, runner_id: 31 }, { ...cancelledJob, steps: [{ status: "completed" }] }]) {
		const { state, io } = fixture();
		await assert.rejects(waitForRun(1, { ...io, cancel: async () => { state.run = cancelled; state.jobs = [job]; } }, deadline), /accepted work/);
	}
});

test("invariant_verified_completion_survives_a_lost_cancel_response", async () => {
	for (const conclusion of ["success", "cancelled"]) {
		const { state, io } = fixture();
		const result = await waitForRun(1, { ...io, cancel: async () => {
			state.run = { ...cancelled, conclusion };
			state.jobs = [{ ...cancelledJob, conclusion }];
			throw new Error("response lost");
		} }, deadline);
		assert.equal(result, conclusion === "success" ? "passed" : "unassigned");
	}
});

test("invariant_unknown_cancellation_is_bounded_without_overflow", async () => {
	const { state, io } = fixture();
	await assert.rejects(waitForRun(1, { ...io, cancel: async () => { state.cancellations++; } }, deadline), /not confirmed/);
	assert.equal(state.now, 90);
	assert.equal(state.cancellations, 1);
});

test("invariant_assignment_disables_the_queue_deadline", async () => {
	const { state, io } = fixture();
	state.jobs = [{ ...pending, status: "in_progress", runner_id: 31 }];
	assert.equal(await waitForRun(1, { ...io, sleep: async () => {
		state.now += 120;
		state.run = { ...cancelled, conclusion: "success" };
		state.jobs = [{ ...cancelledJob, conclusion: "success", runner_id: 31 }];
	} }, deadline), "passed");
	assert.equal(state.cancellations, 0);
});

test("invariant_workflow_success_requires_exactly_one_successful_payload", async () => {
	for (const jobs of [[], [cancelledJob], [{ ...cancelledJob, conclusion: "success" }, { ...cancelledJob, conclusion: "success" }]]) {
		const { state, io } = fixture();
		state.run = { ...cancelled, conclusion: "success" };
		state.jobs = jobs;
		await assert.rejects(waitForRun(1, io, deadline), /runner payload/);
	}
});

test("invariant_cleanup_owns_only_unfinished_runs", async () => {
	const { state, io } = fixture();
	await cancelActiveRun(1, io);
	await cancelActiveRun(1, io);
	assert.equal(state.cancellations, 1);
});

test("invariant_api_failure_cannot_authorize_overflow", async () => {
	const { state, io } = fixture();
	await assert.rejects(waitForRun(1, { ...io, readRun: async () => { throw new Error("API unavailable"); } }, deadline), /API unavailable/);
	assert.equal(state.cancellations, 0);
});
