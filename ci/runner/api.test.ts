//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies runner authorization and dispatch identity against GitHub response contracts.

import assert from "node:assert/strict";
import { test } from "node:test";
import { RunnerApi, payloadName, workflowFile } from "./api.ts";

const sha = "a".repeat(40);
const other = "b".repeat(40);

test("invariant_dispatch_uses_one_returned_run_id", async () => {
	let requests = 0;
	const api = new RunnerApi("fixture", async (url, options) => {
		requests++;
		assert.equal(String(url), `https://api.github.com/repos/dedalus-labs/bsmr/actions/workflows/${workflowFile}/dispatches`);
		assert.equal(options?.method, "POST");
		assert.deepEqual(JSON.parse(String(options?.body)), { ref: "main", return_run_details: true, inputs: { provider: "machines", revision: sha, definition: other, parent: "7" } });
		return Response.json({ workflow_run_id: 8, html_url: "https://github.com/dedalus-labs/bsmr/actions/runs/8" });
	});
	assert.equal((await api.dispatch({ provider: "machines", source: sha, definition: other, parent: "7" })).id, "8");
	assert.equal(requests, 1);
});

test("invariant_ambiguous_dispatch_cannot_create_a_second_run", async () => {
	let requests = 0;
	const api = new RunnerApi("fixture", async () => { requests++; throw new Error("response lost"); });
	await assert.rejects(api.dispatch({ provider: "machines", source: sha, definition: other, parent: "7" }), /response lost/);
	assert.equal(requests, 1);
});

test("invariant_job_inventory_is_complete_before_filtering", async () => {
	for (const response of [{ total_count: 2, jobs: [] }, { total_count: 1, jobs: [{ name: payloadName }] }]) {
		const api = new RunnerApi("fixture", async () => Response.json(response));
		await assert.rejects(api.jobs(1));
	}
});

test("invariant_write_permission_does_not_authorize_office_execution", async () => {
	const api = new RunnerApi("fixture", async () => Response.json({ user: { permissions: { admin: false, push: true } } }));
	await assert.rejects(api.administrator("contributor"), /administrator/);
});

test("invariant_parent_identity_and_liveness_authorize_only_its_source", async () => {
	const parent = { id: 7, status: "in_progress", conclusion: null, head_sha: sha, event: "workflow_dispatch", path: `.github/workflows/${workflowFile}`, display_title: `Rust build ${other}`, triggering_actor: { login: "administrator" } };
	for (const change of [{ status: "completed" }, { conclusion: "cancelled" }, { head_sha: other }, { display_title: `Rust build ${sha}` }, { path: ".github/workflows/unrelated.yml" }]) {
		let requests = 0;
		const api = new RunnerApi("fixture", async () => { requests++; return Response.json({ ...parent, ...change }); });
		await assert.rejects(api.parent({ id: "7", definition: sha, source: other }));
		assert.equal(requests, 1);
	}
	const api = new RunnerApi("fixture", async (url) => Response.json(String(url).includes("collaborators") ? { user: { permissions: { admin: true } } } : parent));
	await api.parent({ id: "7", definition: sha, source: other });
});

test("invariant_api_errors_remain_visible", async () => {
	const api = new RunnerApi("fixture", async () => new Response(null, { status: 403 }));
	await assert.rejects(api.jobs(1), /HTTP 403/);
});

test("an acknowledged run can precede its read endpoints", async () => {
	const api = new RunnerApi("fixture", async () => new Response(null, { status: 404 }));
	assert.equal(await api.jobs(7), undefined);
	assert.equal(await api.io().readRun(7), undefined);
	await assert.rejects(api.administrator("administrator"), /HTTP 404/);
});

test("a push parent delegates only its reviewed main revision", async () => {
	const parent = { id: 7, status: "in_progress", conclusion: null, head_sha: sha, event: "push", path: `.github/workflows/${workflowFile}`, display_title: `Rust build ${sha}`, triggering_actor: { login: "administrator" } };
	const api = new RunnerApi("fixture", async (url) => Response.json(String(url).includes("collaborators") ? { user: { permissions: { admin: true } } } : parent));
	await api.parent({ id: "7", definition: sha, source: sha });
	await assert.rejects(api.parent({ id: "7", definition: sha, source: other }));
});
