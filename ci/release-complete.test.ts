//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies release identity before completing release pull-request labels.

import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, test } from "node:test";

import type { ScriptExec } from "@dedalus-labs/hollywood";
import { parse } from "yaml";
import { completeRelease, releaseComplete } from "./release-complete.ts";

const source = "a".repeat(40);
const repository = "dedalus-labs/bsmr";
const directories: string[] = [];
afterEach(() =>
	directories.splice(0).forEach((path) => rmSync(path, { recursive: true, force: true })),
);

/** Model GitHub responses while retaining the real label mutation boundary. */
function fixture() {
	const workspace = mkdtempSync(join(tmpdir(), "release-complete-"));
	directories.push(workspace);
	writeFileSync(join(workspace, "VERSION"), "0.0.4\n");
	writeFileSync(join(workspace, ".release-please-manifest.json"), '{".":"0.0.4"}\n');
	const labels = new Set(["autorelease: pending", "keep"]);
	const pull = {
		number: 12,
		title: "chore(main): release 0.0.4",
		state: "closed",
		merged_at: "2026-09-15T00:00:00Z" as string | null,
		merge_commit_sha: source,
		base: { ref: "main" },
		head: { ref: "release-please--branches--main", repo: { full_name: repository } },
		labels: [] as { name: string }[],
	};
	const state = {
		release: { tag_name: "v0.0.4", target_commitish: source, draft: false, immutable: true },
		tag: source,
		pulls: [pull],
		comparison: { status: "identical", merge_base_commit: { sha: source } },
		labels,
		applyWrites: true,
		writes: [] as string[][],
		reads: [] as string[],
	};
	const exec: ScriptExec = async (file, args) => {
		assert.equal(file, "gh");
		if (args[1] === "--method") {
			state.writes.push([...args]);
			if (args[2] === "POST") {
				assert.deepEqual(args.slice(4), ["-f", "labels[]=autorelease: tagged"]);
				if (state.applyWrites) labels.add(args[5]!.slice("labels[]=".length));
			} else if (args[2] === "DELETE") {
				if (state.applyWrites)
					labels.delete(decodeURIComponent(args[3]!.split("/").at(-1)!));
			} else assert.fail("unexpected write");
			return { exitCode: 0, stdout: "", stderr: "" };
		}
		const path = args[1]!;
		state.reads.push(path);
		let response: unknown;
		if (path.includes("/releases/tags/")) response = state.release;
		else if (path.includes("/commits/refs/tags/")) response = { sha: state.tag };
		else if (path.endsWith("/pulls"))
			response = [
				state.pulls.map((pr) => ({ ...pr, labels: [...labels].map((name) => ({ name })) })),
			];
		else if (path.includes("/compare/")) response = state.comparison;
		else if (path.endsWith("/labels")) {
			const all = [...labels].map((name) => ({ name }));
			response = args.includes("--slurp")
				? [all.slice(0, 30), all.slice(30)]
				: all.slice(0, 30);
		} else assert.fail(`unexpected read ${path}`);
		return { exitCode: 0, stdout: JSON.stringify(response), stderr: "" };
	};
	const input = {
		workspace,
		repository,
		sourceSha: source,
		eventName: "workflow_dispatch",
		ref: "refs/heads/main",
		plan: JSON.stringify({ announcement_tag: "v0.0.4", announcement_tag_is_implicit: false }),
	};
	return { state, pull, input, exec };
}

test("published version completion preserves labels and is idempotent", async () => {
	const world = fixture();
	await completeRelease(world.exec, world.input);
	assert.deepEqual([...world.state.labels].sort(), ["autorelease: tagged", "keep"]);
	assert.deepEqual(world.state.writes, [
		[
			"api",
			"--method",
			"POST",
			`repos/${repository}/issues/12/labels`,
			"-f",
			"labels[]=autorelease: tagged",
		],
		[
			"api",
			"--method",
			"DELETE",
			`repos/${repository}/issues/12/labels/autorelease%3A%20pending`,
		],
	]);
	world.state.writes.length = 0;
	await completeRelease(world.exec, world.input);
	assert.deepEqual(world.state.writes, []);
});

test("an unpublished-version retry can release a descendant of its version PR", async () => {
	const world = fixture();
	world.pull.merge_commit_sha = "b".repeat(40);
	world.state.comparison = {
		status: "ahead",
		merge_base_commit: { sha: world.pull.merge_commit_sha },
	};
	await completeRelease(world.exec, world.input);
	assert.ok(
		world.state.reads.includes(
			`repos/${repository}/compare/${world.pull.merge_commit_sha}...${source}`,
		),
	);
	assert.deepEqual([...world.state.labels].sort(), ["autorelease: tagged", "keep"]);
});

const invalid: readonly [string, (world: ReturnType<typeof fixture>) => void][] = [
	["PR event", (w) => Object.assign(w.input, { eventName: "pull_request" })],
	["non-main ref", (w) => Object.assign(w.input, { ref: "refs/heads/topic" })],
	["fork caller", (w) => Object.assign(w.input, { repository: "other/bsmr" })],
	[
		"dry run",
		(w) =>
			Object.assign(w.input, {
				plan: '{"announcement_tag":"v0.0.4","announcement_tag_is_implicit":true}',
			}),
	],
	[
		"wrong planned version",
		(w) =>
			Object.assign(w.input, {
				plan: '{"announcement_tag":"v0.0.5","announcement_tag_is_implicit":false}',
			}),
	],
	["malformed plan", (w) => Object.assign(w.input, { plan: "null" })],
	["invalid source", (w) => Object.assign(w.input, { sourceSha: "main" })],
	["draft release", (w) => Object.assign(w.state.release, { draft: true })],
	["mutable release", (w) => Object.assign(w.state.release, { immutable: false })],
	["wrong release tag", (w) => Object.assign(w.state.release, { tag_name: "v0.0.3" })],
	[
		"wrong release source",
		(w) => Object.assign(w.state.release, { target_commitish: "b".repeat(40) }),
	],
	["wrong peeled tag", (w) => Object.assign(w.state, { tag: "b".repeat(40) })],
	["no matching version PR", (w) => Object.assign(w.state, { pulls: [] })],
	["ambiguous version PR", (w) => w.state.pulls.push({ ...w.pull, number: 13 })],
	["unmerged PR", (w) => Object.assign(w.pull, { merged_at: null })],
	["wrong version PR", (w) => Object.assign(w.pull, { title: "chore(main): release 0.0.3" })],
	["fork version PR", (w) => Object.assign(w.pull.head.repo, { full_name: "other/bsmr" })],
	["invalid PR number", (w) => Object.assign(w.pull, { number: 0 })],
	["divergent source", (w) => Object.assign(w.state.comparison, { status: "diverged" })],
	[
		"wrong comparison base",
		(w) => Object.assign(w.state.comparison.merge_base_commit, { sha: "b".repeat(40) }),
	],
];
for (const [name, change] of invalid)
	test(`${name} cannot write release labels`, async () => {
		const world = fixture();
		change(world);
		await assert.rejects(completeRelease(world.exec, world.input));
		assert.deepEqual(world.state.writes, []);
	});

test("label completion requires a successful readback", async () => {
	const world = fixture();
	world.state.applyWrites = false;
	await assert.rejects(completeRelease(world.exec, world.input), /readback mismatch/);
});

test("completion preserves labels beyond the first API page", async () => {
	const world = fixture();
	for (let index = 0; index < 31; index++) world.state.labels.add(`keep-${index}`);
	await completeRelease(world.exec, world.input);
	assert.equal(world.state.labels.size, 33);
});

test("completion runs after announcement with only its declared token permissions", () => {
	const caller = parse(readFileSync(".github/workflows/release.yml", "utf8")).jobs[
		"custom-release-complete"
	];
	assert.deepEqual(caller.needs, ["plan", "announce"]);
	assert.equal(caller.uses, "./.github/workflows/release-complete.yml");
	assert.deepEqual(caller.with, { plan: "${{ needs.plan.outputs.val }}" });
	assert.deepEqual(caller.permissions, { contents: "read", "pull-requests": "write" });
	assert.deepEqual(releaseComplete.on, {
		workflow_call: { inputs: { plan: { type: "string", required: true } } },
	});
	assert.deepEqual(releaseComplete.jobs.complete?.permissions, caller.permissions);
	assert.equal(
		releaseComplete.jobs.complete?.if,
		"${{ github.repository == 'dedalus-labs/bsmr' && github.event_name == 'workflow_dispatch' && github.ref == 'refs/heads/main' && github.event.inputs.tag != 'dry-run' }}",
	);
	const generated = parse(readFileSync(".github/workflows/release-complete.yml", "utf8"));
	assert.equal(generated.jobs.complete.if, releaseComplete.jobs.complete?.if);
	assert.equal(releaseComplete.jobs.complete?.steps[0]?.with?.ref, "${{ github.sha }}");
});
