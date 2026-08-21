//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies the fail-closed Reindeer protocol adapter.

import assert from "node:assert/strict";
import { test } from "node:test";

import { externalReindeerConfig } from "./rust-build-dependencies.ts";

const externalProduct = ["bu", "ck"].join("");

test("invariant_reindeer_receives_its_required_schema", () => {
	const actual = externalReindeerConfig(
		'[bsmr]\nfile_name = "BUILD.bsmr"\nbsmrfile_imports = "rules"\n',
		"/repo/tools/build/third-party/rust",
	);
	assert.equal(
		actual,
		`third_party_dir = "/repo/tools/build/third-party/rust"\n[${externalProduct}]\nfile_name = "BUILD.bsmr"\n${externalProduct}file_imports = "rules"\n`,
	);
});

test("design_reindeer_adapter_rejects_schema_drift", () => {
	assert.throws(() => externalReindeerConfig('[bsmr]\nfile_name = "BUILD.bsmr"\n', "/third-party"), /bsmrfile_imports/);
	assert.throws(
		() => externalReindeerConfig('[bsmr]\n[bsmr]\nbsmrfile_imports = "rules"\n', "/third-party"),
		/\[bsmr\]/,
	);
});
