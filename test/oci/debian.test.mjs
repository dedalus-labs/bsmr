//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Rejects ambient resolver code and escaped base state before APT can execute.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtemp, mkdir, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

const resolver = resolve(import.meta.dirname, "../../prelude/debian/lock.py");

/** Run the real CLI against a private base with no usable network inputs. */
async function fixture(t) {
	const root = await mkdtemp(join(tmpdir(), "bsmr-debian-lock-"));
	t.after(() => rm(root, { recursive: true, force: true }));
	await mkdir(join(root, "base/var/lib/dpkg"), { recursive: true });
	await writeFile(join(root, "base/var/lib/dpkg/status"), "");
	const output = join(root, "lock.json");
	const args = [resolver, "--root", join(root, "base"), "--base-digest", `sha256:${"a".repeat(64)}`,
		"--platform", "linux/arm64", "--repository", "https://example.invalid/debian", "--suite", "bookworm", "--output", output, "curl"];
	return { root, output, args };
}

test("invariant resolver rejects ambient APT configuration before hooks or network requests", async (t) => {
	const { root, output, args } = await fixture(t);
	const config = join(root, "apt.conf"), marker = join(root, "hook-ran");
	await writeFile(config, `APT::Update::Pre-Invoke { "touch ${marker}"; };\n`);
	const result = spawnSync("python3", args, { env: { ...process.env, APT_CONFIG: config }, encoding: "utf8", timeout: 5000 });
	assert.equal(result.status, 1, result.stderr);
	assert.match(result.stderr, /DEBIAN_AMBIENT_CONFIGURATION/);
	await assert.rejects(readFile(marker), { code: "ENOENT" });
	await assert.rejects(readFile(output), { code: "ENOENT" });
});

test("invariant resolver cannot follow base package status outside the declared root", async (t) => {
	const { root, output, args } = await fixture(t);
	const status = join(root, "base/var/lib/dpkg/status"), outside = join(root, "outside");
	await writeFile(outside, "private");
	await rm(status);
	await symlink(outside, status);
	const result = spawnSync("python3", args, { encoding: "utf8", timeout: 5000 });
	assert.equal(result.status, 1, result.stderr);
	assert.match(result.stderr, /DEBIAN_INVALID_BASE_FILE/);
	assert.equal(await readFile(outside, "utf8"), "private");
	await assert.rejects(readFile(output), { code: "ENOENT" });
});
