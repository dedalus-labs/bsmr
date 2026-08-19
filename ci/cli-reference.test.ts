//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies CLI reference drift detection.

import assert from "node:assert/strict";
import test from "node:test";

import { verifyCliReference } from "./cli-reference.ts";

test("committed CLI documentation must equal parser output", () => {
	assert.doesNotThrow(() => verifyCliReference("usage\n", "usage\n"));
	assert.throws(() => verifyCliReference("usage\n", "changed\n"), /cli\.md is stale/);
});
