//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies the generated dependency-update policy.

import assert from "node:assert/strict";
import test from "node:test";

import { parse } from "yaml";

import { dependabotConfig, renderDependabotConfig } from "./dependabot.ts";

test("compatible updates are grouped without hiding breaking changes", () => {
	const config = dependabotConfig();
	assert.deepEqual(
		config.updates.map((update) => update.directory),
		["/", "/tools/build/third-party/rust"],
	);
	for (const update of config.updates) {
		assert.deepEqual(update.groups.routine, {
			"applies-to": "version-updates",
			patterns: ["*"],
			"update-types": ["minor", "patch"],
		});
		assert.deepEqual(update.groups.security, {
			"applies-to": "security-updates",
			patterns: ["*"],
		});
	}
});

test("generated policy round trips through GitHub YAML semantics", () => {
	assert.deepEqual(parse(renderDependabotConfig(), { version: "1.1" }), dependabotConfig());
});
