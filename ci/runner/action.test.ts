//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies that unapproved revisions cannot reach runner execution.

import assert from "node:assert/strict";
import { afterEach, beforeEach, test } from "node:test";
import { runAction } from "@dedalus-labs/hollywood";
import { runnerAction } from "./action.ts";

const originalFetch = globalThis.fetch;
const originalToken = process.env["GH_TOKEN"];
const sha = "a".repeat(40);
const inputs = { operation: "authorize", provider: "auto", source: sha, definition: "", actualDefinition: sha, repository: "dedalus-labs/bsmr", event: "workflow_dispatch", ref: "refs/heads/main", actor: "administrator", parent: "" } as const;

beforeEach(() => { process.env["GH_TOKEN"] = "fixture"; });
afterEach(() => {
	globalThis.fetch = originalFetch;
	if (originalToken === undefined) delete process.env["GH_TOKEN"];
	else process.env["GH_TOKEN"] = originalToken;
});

/** Exercise the production action with a replaced network boundary only. */
async function authorize(override: Partial<Record<keyof typeof inputs, string>> = {}) {
	return runAction(runnerAction, {
		with: { ...inputs, ...override },
		exec: async () => assert.fail("authorization must not execute project code"),
		fs: { readText: async () => assert.fail("authorization must not read project files") },
		runner: { uidGid: "1000:1000" },
	});
}

test("invariant_only_main_dispatches_can_authorize_builds", async () => {
	globalThis.fetch = async () => assert.fail("invalid workflow reached GitHub");
	for (const override of [{ ref: "refs/heads/feature" }, { event: "pull_request" }, { repository: "fork/bsmr" }, { source: "main" }])
		await assert.rejects(authorize(override));
});

test("invariant_administrator_permission_is_checked_before_source_access", async () => {
	let requests = 0;
	globalThis.fetch = async (url) => {
		requests++;
		assert.match(String(url), /collaborators\/administrator\/permission$/);
		return Response.json({ user: { permissions: { admin: false } } });
	};
	await assert.rejects(authorize(), /administrator/);
	assert.equal(requests, 1);
});

test("invariant_approved_source_is_an_exact_commit", async () => {
	const paths: string[] = [];
	globalThis.fetch = async (url) => {
		paths.push(String(url));
		return Response.json(paths.length === 1 ? { user: { permissions: { admin: true } } } : { sha });
	};
	await authorize();
	assert.equal(paths.length, 2);
	assert.ok(paths[1]?.endsWith(`/commits/${sha}`));
});

test("invariant_changed_child_definition_cannot_run", async () => {
	globalThis.fetch = async () => assert.fail("changed child definition reached GitHub");
	await assert.rejects(authorize({ provider: "machines", parent: "7", definition: "b".repeat(40) }), /definition changed/);
});
