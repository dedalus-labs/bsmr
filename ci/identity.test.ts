//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies that repository identity excludes upstream product nomenclature.

import * as assert from "node:assert/strict";
import { test } from "node:test";

import { identityFindings } from "./identity.ts";

const upstreamProduct = ["bu", "ck"].join("");

test("invariant_owned_paths_and_text_use_bessemer_names", () => {
	const findings = identityFindings([
		{ path: `src/${upstreamProduct}_event.rs`, text: "" },
		{ path: "src/event.rs", text: `struct ${upstreamProduct[0]?.toUpperCase()}${upstreamProduct.slice(1)}Event;` },
	]);

	assert.deepEqual(findings, [
		`src/${upstreamProduct}_event.rs: upstream product name in path`,
		"src/event.rs:1: upstream product name",
	]);
});

test("design_legal_provenance_and_bucket_terms_remain_valid", () => {
	const findings = identityFindings([
		{ path: "src/cache.rs", text: "let bucket_count = buckets.len();" },
		{ path: "src/bucket/index.rs", text: "" },
		{
			path: "src/upstream.rs",
			text: [
				`// Upstream-Source: facebook/${upstreamProduct}2@0123456789abcdef`,
				`// https://github.com/facebook/${upstreamProduct}2/releases/tag/toolchain`,
			].join("\n"),
		},
		{ path: "NOTICE", text: `Derived from ${upstreamProduct}2.` },
	]);

	assert.deepEqual(findings, []);
});
