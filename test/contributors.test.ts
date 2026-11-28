//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies the vouched-contributor trust policy.

import * as assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { execFileSync, spawnSync, type SpawnSyncReturns } from "node:child_process";
import { test } from "node:test";

import { parse } from "yaml";

import { checkContributor } from "../ci/contributors.ts";

type CheckoutStep = Readonly<{ uses: string; with: { ref: string } }>;
type RunStep = Readonly<{ run: string; env: Record<string, string> }>;
type PullRequestEvent = Readonly<{
	pull_request: {
		base: { ref: string; sha: string };
		head: { ref: string; sha: string };
		user: { login: string };
	};
}>;

const [checkout, check]: readonly [CheckoutStep, RunStep] = parse(
	readFileSync(new URL("../.github/workflows/contributors.yml", import.meta.url), "utf8"),
).jobs.cla.steps;

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

/**
 * Run git with a fixed identity and return its trimmed stdout.
 *
 * @param cwd - Repository directory.
 * @param args - Git arguments.
 * @returns Trimmed standard output.
 */
function git(cwd: string, ...args: string[]): string {
	return execFileSync("git", ["-c", "user.name=test", "-c", "user.email=test@example.com", ...args], {
		cwd,
		encoding: "utf8",
	}).trim();
}

/**
 * Commit VOUCHED.td with the given contents on the current branch.
 *
 * @param clone - Working clone.
 * @param vouched - New VOUCHED.td contents.
 * @returns The new commit sha.
 */
function commitVouched(clone: string, vouched: string): string {
	writeFileSync(join(clone, "VOUCHED.td"), vouched);
	git(clone, "add", "VOUCHED.td");
	git(clone, "commit", "--quiet", "-m", "vouch");
	return git(clone, "rev-parse", "HEAD");
}

/**
 * Substitute GitHub event expressions the way the runner does, failing on any it cannot resolve.
 *
 * @param text - Workflow value containing `${{ github.event.* }}` expressions.
 * @param event - Pull request event payload.
 * @returns The value with every expression replaced.
 * @throws An error when an expression does not name an event string.
 */
function resolve(text: string, event: PullRequestEvent): string {
	return text.replace(/\$\{\{\s*github\.event\.([\w.]+)\s*\}\}/g, (_, path: string) => {
		const value: unknown = path.split(".").reduce<unknown>((node, key) => (node as Record<string, unknown>)[key], event);
		if (typeof value !== "string") throw new Error(`unresolved expression github.event.${path}`);
		return value;
	});
}

/**
 * Run the generated CLA job against a repository: check out its ref from origin, then run its check.
 *
 * @param origin - Bare repository standing in for the GitHub base repository.
 * @param event - Pull request event that triggered the job.
 * @returns The check step's process result.
 */
function runClaJob(origin: string, event: PullRequestEvent): SpawnSyncReturns<string> {
	assert.match(checkout.uses, /^actions\/checkout@/);
	const workspace = mkdtempSync(join(tmpdir(), "bsmr-cla-job-"));
	try {
		git(workspace, "init", "--quiet");
		git(workspace, "fetch", "--quiet", "--depth=1", origin, resolve(checkout.with.ref, event));
		git(workspace, "checkout", "--quiet", "--detach", "FETCH_HEAD");
		const env = Object.fromEntries(Object.entries(check.env).map(([key, value]) => [key, resolve(value, event)]));
		return spawnSync("bash", ["--noprofile", "--norc", "-eo", "pipefail", "-c", check.run], {
			cwd: workspace,
			encoding: "utf8",
			env: { ...process.env, ...env },
		});
	} finally {
		rmSync(workspace, { force: true, recursive: true });
	}
}

test("CLA reads VOUCHED.td from the base branch tip, never from the pull request", () => {
	const root = mkdtempSync(join(tmpdir(), "bsmr-cla-"));
	try {
		const origin = join(root, "origin.git");
		const clone = join(root, "clone");
		git(root, "init", "--quiet", "--bare", "--initial-branch=main", origin);
		git(origin, "config", "uploadpack.allowAnySHA1InWant", "true");
		git(root, "init", "--quiet", "--initial-branch=main", clone);
		const branchPoint = commitVouched(clone, "github:octocat\n");
		git(clone, "push", "--quiet", origin, "main");

		git(clone, "switch", "--quiet", "-c", "self-vouch");
		const selfVouch = commitVouched(clone, "github:hubot\ngithub:octocat\n");
		git(clone, "push", "--quiet", origin, "self-vouch");

		git(clone, "switch", "--quiet", "main");
		commitVouched(clone, "github:capy-ai[bot]\ngithub:octocat\n");
		git(clone, "push", "--quiet", origin, "main");

		const event = (login: string, head: string, sha: string): PullRequestEvent => ({
			pull_request: { base: { ref: "main", sha: branchPoint }, head: { ref: head, sha }, user: { login } },
		});

		const stale = runClaJob(origin, event("capy-ai[bot]", "bot-change", branchPoint));
		assert.equal(stale.status, 0, stale.stderr);
		assert.match(stale.stdout, /@capy-ai\[bot\] has accepted CLA\.md/);

		const self = runClaJob(origin, event("hubot", "self-vouch", selfVouch));
		assert.equal(self.status, 1);
		assert.match(self.stderr, /@hubot is not listed in VOUCHED\.td/);
	} finally {
		rmSync(root, { force: true, recursive: true });
	}
});
