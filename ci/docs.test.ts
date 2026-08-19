//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies documentation publication stays trusted and least privileged.

import assert from "node:assert/strict";
import test from "node:test";

import { docs } from "./docs.ts";

test("documentation validates trusted changes but deploys only from main", () => {
	assert.match(docs.jobs.build?.if ?? "", /dedalus-labs\/bsmr/);
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
	assert.deepEqual(docs.concurrency, {
		group: "pages-${{ github.ref }}",
		"cancel-in-progress": false,
	});
	assert.equal(
		docs.jobs.deploy?.if,
		"github.event_name == 'push' && github.ref == 'refs/heads/main'",
	);
	assert.deepEqual(docs.jobs.deploy?.permissions, {
		pages: "write",
		"id-token": "write",
	});
});

test("documentation uses pinned actions and strict MkDocs validation", () => {
	const steps = docs.jobs.build?.steps ?? [];
	assert.ok(
		steps.every((step) => !("uses" in step) || /@[0-9a-f]{40}$/.test(step.uses)),
	);
	assert.ok(
		steps.some(
			(step) =>
				"run" in step &&
				step.run.kind === "command" &&
				step.run.file === "python" &&
				step.run.args.join(" ") === "-m mkdocs build --strict -f mkdocs.yml",
		),
	);
});
