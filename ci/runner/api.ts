//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Validates the GitHub run, job, and authorization records owned by this workflow.

import { setTimeout } from "node:timers/promises";
import { z } from "zod";
import type { RunIo } from "./lifecycle.ts";

export const workflowFile = "runner-build.yml";
export const payloadName = "Build Rust";
export const revision = z.string().regex(/^[0-9a-f]{40}$/);
export const providers = ["machines", "blacksmith", "github"] as const;
type Dispatch = Readonly<{ provider: typeof providers[number]; source: string; definition: string; parent: string }>;
const runId = z.coerce.number().int().positive();
const login = z.string().regex(/^[A-Za-z0-9][A-Za-z0-9-]*$/);
const runSchema = z.object({
	id: runId,
	status: z.enum(["queued", "in_progress", "requested", "waiting", "pending", "completed"]),
	conclusion: z.string().nullable(),
	html_url: z.url(),
});
const jobSchema = z.object({
	name: z.string(), status: z.string(), conclusion: z.string().nullable(),
	runner_id: z.number().nullable(), steps: z.array(z.object({ status: z.string() })),
});

/** Retain the HTTP status so observation lag cannot hide authorization failures. */
class ApiError extends Error {
	readonly status: number;
	/** Keep the endpoint and status available without exposing response credentials. */
	constructor(path: string, status: number) {
		super(`runner API ${path}: HTTP ${status}`);
		this.status = status;
	}
}

/** Bind every operation to this repository, with authentication supplied only through GH_TOKEN. */
export class RunnerApi {
	private readonly token: string;
	private readonly request: typeof fetch;

	/** Retain one scoped credential and transport for the lifetime of the action. */
	constructor(token: string | undefined, request: typeof fetch = fetch) {
		this.token = z.string().min(1).parse(token);
		this.request = request;
	}

	/** Reject incomplete job lists before applying the one-payload lifecycle policy. */
	async jobs(id: number) {
		const response = await this.observe(`actions/runs/${runId.parse(id)}/jobs?per_page=100`);
		if (response === undefined) return undefined;
		const result = z.object({ total_count: z.number().int(), jobs: z.array(jobSchema) })
			.parse(response);
		if (result.total_count !== result.jobs.length) throw new Error("incomplete runner job inventory");
		return result.jobs.filter((job) => job.name === payloadName);
	}

	/** Expose the same bounded observation and cancellation operations as the monorepo controller. */
	io(): RunIo {
		return {
			readRun: async (id) => {
				const response = await this.observe(`actions/runs/${runId.parse(id)}`);
				return response === undefined ? undefined : runSchema.parse(response);
			},
			readJobs: async (id) => this.jobs(id),
			cancel: async (id) => { await this.call(`actions/runs/${runId.parse(id)}/cancel`, {}); },
			now: () => Date.now() / 1000,
			sleep: async () => { await setTimeout(15_000); },
		};
	}

	/** A dispatch receipt may precede readable state. The lifecycle owns its deadline. */
	private async observe(path: string): Promise<unknown> {
		try { return await this.call(path); }
		catch (error: unknown) {
			if (error instanceof ApiError && error.status === 404) return undefined;
			throw error;
		}
	}

	/** Verify fresh administrator permission rather than trusting a user-controlled workflow input. */
	async administrator(actor: string): Promise<void> {
		const user = z.object({ user: z.object({ permissions: z.object({ admin: z.boolean() }) }) })
			.parse(await this.call(`collaborators/${login.parse(actor)}/permission`));
		if (!user.user.permissions.admin) throw new Error("office build dispatch requires a repository administrator");
	}

	/** An unfinished parent must own the same workflow definition and source before a child can run. */
	async parent(input: Readonly<{ id: string; definition: string; source: string }>): Promise<void> {
		const parent = z.object({
			id: runId, status: z.literal("in_progress"), conclusion: z.null(),
			head_sha: revision, event: z.literal("workflow_dispatch"), path: z.literal(`.github/workflows/${workflowFile}`),
			display_title: z.string(), triggering_actor: z.object({ login }),
		}).parse(await this.call(`actions/runs/${runId.parse(input.id)}`));
		if (parent.head_sha !== input.definition || parent.display_title !== `Rust build ${input.source}`)
			throw new Error("parent workflow does not own this source and definition");
		await this.administrator(parent.triggering_actor.login);
	}

	/** Return only after GitHub identifies the single run created by this dispatch. */
	async dispatch(input: Dispatch): Promise<{ id: string; url: string }> {
		const result = z.object({ workflow_run_id: runId, html_url: z.url() }).parse(await this.call(
			`actions/workflows/${workflowFile}/dispatches`,
			{ ref: "main", return_run_details: true, inputs: { provider: input.provider, revision: revision.parse(input.source), definition: revision.parse(input.definition), parent: String(runId.parse(input.parent)) } },
		));
		return { id: String(result.workflow_run_id), url: result.html_url };
	}

	/** Run one API operation with a deadline. A lost mutation response is never retried. */
	async call(path: string, fields?: Readonly<Record<string, string | boolean | Readonly<Record<string, string>>>>): Promise<unknown> {
		const response = await this.request(`https://api.github.com/repos/dedalus-labs/bsmr/${path}`, {
			method: fields === undefined ? "GET" : "POST",
			headers: { Authorization: `Bearer ${this.token}`, Accept: "application/vnd.github+json", "Content-Type": "application/json", "X-GitHub-Api-Version": "2026-03-10" },
			signal: AbortSignal.timeout(10_000),
			...(fields === undefined ? {} : { body: JSON.stringify(fields) }),
		});
		if (!response.ok) throw new ApiError(path, response.status);
		if (response.status === 204) return null;
		return response.json();
	}
}
