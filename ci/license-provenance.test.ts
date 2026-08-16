//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies source selection and fork-boundary provenance classification.

import assert from "node:assert/strict";
import { test } from "node:test";

import { classify, isSource, parseChanges } from "./license-provenance.ts";

test("changes preserve destination and origin paths", () => {
	const changes = parseChanges("A\0new.ts\0R100\0old.rs\0new.rs\0M\0same.py\0");
	assert.deepEqual(changes.get("new.ts"), { status: "A" });
	assert.deepEqual(changes.get("new.rs"), { oldPath: "old.rs", status: "R100" });
	assert.deepEqual(changes.get("same.py"), { status: "M" });
});

test("source selection excludes behavioral fixtures", () => {
	assert.equal(isSource("app/bsmr/src/main.rs"), true);
	assert.equal(isSource("app/bsmr/BUILD.bsmr"), true);
	assert.equal(isSource("tests/core/test_empty_data/.bsmr"), false);
	assert.equal(isSource("tests/core/console/fixtures/my_genrule0.proto"), false);
	assert.equal(isSource("packages/rust/starlark/starlark/src/docs/tests/golden/object.golden.md"), false);
	assert.equal(isSource("tests/snapshots/native.golden.md"), false);
	assert.equal(isSource("tests/snapshots/expr_fstring.golden"), false);
	assert.equal(isSource("package.json"), false);
});

test("classification follows the immutable fork boundary", () => {
	const meta = "// Copyright (c) Meta Platforms, Inc. and affiliates.";
	assert.equal(classify("new", { status: "A" }), "dedalus");
	assert.equal(classify(meta, { status: "A" }), "upstream-modified");
	assert.equal(classify(meta, { oldPath: "old.rs", status: "R100" }), "upstream");
	assert.equal(classify(meta, { oldPath: "old.rs", status: "R099" }), "upstream-modified");
	assert.equal(classify(meta), "upstream");
	assert.equal(classify("load(\"//rules:defs.bzl\", \"rule\")\n"), "upstream-modified");
});
