//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Owns one runner attempt until completion or proven unassigned cancellation.

export type Run = Readonly<{
	id: number;
	status: string;
	conclusion: string | null;
	html_url: string;
}>;
export type Job = Readonly<{
	status: string;
	conclusion: string | null;
	runner_id: number | null;
	steps: readonly Readonly<{ status: string }>[];
}>;
export type RunIo = Readonly<{
	readRun: (id: number) => Promise<Run>;
	readJobs: (id: number) => Promise<readonly Job[]>;
	cancel: (id: number) => Promise<void>;
	now: () => number;
	sleep: () => Promise<void>;
}>;
export type Deadline = Readonly<{ queueSeconds: number; totalSeconds: number }>;

/** Retain assignment evidence even if the workflow still reports queued. */
export const wasAssigned = (jobs: readonly Job[]): boolean => jobs.some((job) =>
	(job.runner_id !== null && job.runner_id > 0) || job.status === "in_progress" ||
	job.steps.some((step) => step.status !== "queued" && step.status !== "pending"));

/** An empty or incomplete inventory cannot authorize another attempt. */
export const canHandoff = (run: Run, jobs: readonly Job[]): boolean =>
	run.status === "completed" && run.conclusion === "cancelled" && jobs.length > 0 &&
	!wasAssigned(jobs) && jobs.every((job) => job.status === "completed" && job.conclusion === "cancelled");

/** Require the one expected payload to succeed, not merely the workflow wrapper. */
const passed = (run: Run, jobs: readonly Job[]): boolean =>
	run.status === "completed" && run.conclusion === "success" &&
	jobs.length === 1 && jobs[0]?.status === "completed" && jobs[0]?.conclusion === "success";

/** Resolve cancellation from terminal state even if its HTTP response was lost. */
async function cancelAndConfirm(id: number, io: RunIo): Promise<Run> {
	let cause: unknown;
	try { await io.cancel(id); } catch (error: unknown) { cause = error; }
	const deadline = io.now() + 30;
	while (io.now() < deadline) {
		const run = await io.readRun(id);
		if (run.status === "completed") return run;
		await io.sleep();
	}
	throw new Error(`runner cancellation was not confirmed for ${id}`, { cause });
}

/** Permit overflow only when the previous provider provably never accepted the payload. */
export async function waitForRun(id: number, io: RunIo, deadline: Deadline): Promise<"passed" | "unassigned"> {
	if (deadline.queueSeconds <= 0 || deadline.totalSeconds <= deadline.queueSeconds)
		throw new Error("runner deadlines must be positive with queue shorter than total");
	const start = io.now();
	let assigned = false;
	while (io.now() - start < deadline.totalSeconds) {
		const run = await io.readRun(id);
		if (run.status === "completed") {
			if (!passed(run, await io.readJobs(id))) throw new Error(`runner payload ${run.conclusion}: ${run.html_url}`);
			return "passed";
		}
		assigned ||= wasAssigned(await io.readJobs(id));
		if (!assigned && io.now() - start >= deadline.queueSeconds) {
			const terminal = await cancelAndConfirm(id, io);
			const jobs = await io.readJobs(id);
			if (passed(terminal, jobs)) return "passed";
			if (!canHandoff(terminal, jobs)) throw new Error(`runner accepted work or cancellation changed: ${terminal.html_url}`);
			return "unassigned";
		}
		await io.sleep();
	}
	throw new Error(`runner ${id} exceeded its observation deadline`);
}

/** Call from a separate cleanup step, including when the parent is cancelled. */
export async function cancelActiveRun(id: number, io: RunIo): Promise<void> {
	if ((await io.readRun(id)).status !== "completed") await cancelAndConfirm(id, io);
}
