//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies that the Python conformance corpus has immutable, complete identities.

import assert from "node:assert/strict";
import test from "node:test";

import { buildLockArguments, pythonCorpus, runtimeExportArguments } from "./corpus.ts";

test("invariant_corpus_inputs_are_immutable_and_unique", () => {
	assert.equal(new Set(pythonCorpus.map(({ name }) => name)).size, pythonCorpus.length);
	for (const project of pythonCorpus) {
		assert.match(project.commit, /^[0-9a-f]{40}$/);
		assert.match(project.repository, /^https:\/\/github\.com\/.+\.git$/);
		assert.ok(project.buildRequirements.length > 0);
		assert.ok(project.imports.length > 0);
		assert.ok(project.sourceRoots.length > 0);
	}
});

test("invariant_lock_authoring_never_resolves_the_runtime_graph", () => {
	assert.ok(runtimeExportArguments().includes("--frozen"));
	assert.ok(!runtimeExportArguments().includes("--locked"));
	assert.ok(buildLockArguments().includes("--universal"));
	assert.ok(buildLockArguments().includes("--exclude-newer"));
});
