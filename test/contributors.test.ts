//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies the vouched-contributor trust policy.

import * as assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

import { checkContributor } from "../ci/contributors.ts";

const runCheck = (vouched: string, author: string) => {
	const root = mkdtempSync(join(tmpdir(), "bsmr-contributor-"));
	try {
		writeFileSync(join(root, "VOUCHED.td"), vouched);
		return spawnSync("node", ["-e", checkContributor], {
			cwd: root,
			encoding: "utf8",
			env: {
				...process.env,
				CONTRIBUTOR_CHECK: "Vouch",
				PR_AUTHOR: author,
			},
		});
	} finally {
		rmSync(root, { force: true, recursive: true });
	}
};

test("contributor checks fail closed", () => {
	const vouched = runCheck("github:octocat\n", "OctoCat");
	assert.equal(vouched.status, 0);
	assert.match(vouched.stdout, /@OctoCat is listed in VOUCHED\.td/);

	const unknown = runCheck("github:octocat\n", "hubot");
	assert.equal(unknown.status, 1);
	assert.match(unknown.stderr, /@hubot is not listed in VOUCHED\.td/);

	const denounced = runCheck(
		"github:octocat\n-github:octocat compromised account\n",
		"octocat",
	);
	assert.equal(denounced.status, 1);
	assert.match(denounced.stderr, /@octocat is denounced in VOUCHED\.td: compromised account/);
});
