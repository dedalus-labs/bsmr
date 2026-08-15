//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Proves Python installation snapshots compare semantic artifacts, not installer noise.

import assert from "node:assert/strict";
import { join } from "node:path";
import { test } from "node:test";

import { compareSnapshots, snapshotEnvironment } from "./snapshot.ts";

const fixture = (name: string): string => join(import.meta.dirname, "testdata", name);

test("invariant_installer_paths_do_not_change_environment_identity", () => {
	assert.deepEqual(snapshotEnvironment(fixture("uv")), snapshotEnvironment(fixture("bsmr")));
});

test("invariant_installed_payload_drift_fails_conformance", () => {
	assert.deepEqual(compareSnapshots(snapshotEnvironment(fixture("uv")), snapshotEnvironment(fixture("drift"))), [
		"files.demo.py.digest",
	]);
});
