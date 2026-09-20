//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Exercises native task output contracts through the real pnpm CLI.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { access, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, type TestContext } from "node:test";
import { fileURLToPath } from "node:url";

const runner = fileURLToPath(new URL("./task.mjs", import.meta.url));
const pnpm = process.env["npm_execpath"];
assert.ok(pnpm, "run through pnpm so tests use the repository's pinned package manager");

/** Create a package with native script chaining and isolated source/install trees. */
async function fixture(t: TestContext, program = 'mkdirSync("assets"); writeFileSync("assets/index.html", "page"); writeFileSync("cli.mjs", "export const answer = 42;");') {
	const root = await mkdtemp(join(tmpdir(), "bsmr task "));
	t.after(() => rm(root, { recursive: true, force: true }));
	const source = join(root, "source");
	const install = join(root, "install");
	const output = join(root, "output");
	await mkdir(source);
	await mkdir(install);
	const manifest = JSON.stringify({ scripts: { build: "pnpm run emit", emit: "node build.mjs" } });
	await writeFile(join(source, "package.json"), manifest);
	await writeFile(join(install, "package.json"), manifest);
	await writeFile(join(source, "build.mjs"), `import { mkdirSync, writeFileSync, symlinkSync } from "node:fs";\n${program}`);
	/** Invoke the runner with one explicit output contract. */
	function run(outputs: Record<string, string> = { "cli.mjs": "file", assets: "directory" }, script = "build") {
		return spawnSync(process.execPath, [runner,
			"--source", source, "--install", install, "--pnpm", pnpm!,
			"--package", ".", "--script", script, "--output", output,
			"--outputs", JSON.stringify(outputs),
		], { encoding: "utf8", env: { BSMR_SCRATCH_PATH: join(root, "scratch") } });
	}
	return { run, source, output };
}

test("native script chaining publishes a detached executable and its assets", async (t) => {
	const state = await fixture(t);
	const result = state.run();
	assert.equal(result.status, 0, result.stdout + result.stderr);
	assert.equal(await readFile(join(state.output, "assets/index.html"), "utf8"), "page");
	const program = spawnSync(process.execPath, ["--input-type=module", "-e", `import { answer } from ${JSON.stringify(join(state.output, "cli.mjs"))}; console.log(answer);`], { encoding: "utf8" });
	assert.equal(program.status, 0, program.stderr);
	assert.equal(program.stdout.trim(), "42");
	await assert.rejects(access(join(state.output, "package.json")));
	await assert.rejects(access(join(state.source, "cli.mjs")));
});

for (const [name, program, pattern] of [
	["missing required output", 'writeFileSync("cli.mjs", "ok");', /ENOENT.*assets/],
	["wrong output type", 'mkdirSync("cli.mjs"); mkdirSync("assets");', /required file.*wrong type/],
	["escaping symlink", 'symlinkSync("/etc/hosts", "cli.mjs"); mkdirSync("assets");', /escapes its package/],
	["nested symlink", 'writeFileSync("cli.mjs", "ok"); mkdirSync("assets"); symlinkSync("/etc/hosts", "assets/link");', /wrong type/],
	["failed script", 'process.exit(7);', /task 'build' failed/],
] as const) {
	test(`${name} cannot publish successful output`, async (t) => {
		const state = await fixture(t, program);
		const result = state.run();
		assert.notEqual(result.status, 0);
		assert.match(result.stderr, pattern);
		await assert.rejects(access(state.output));
	});
}

test("outputs cannot overlap each other or declared sources", async (t) => {
	const state = await fixture(t);
	for (const outputs of [{ assets: "directory", "assets/index.html": "file" }, { "../outside": "file" }, { "package.json": "file" }]) {
		const result = state.run(outputs);
		assert.notEqual(result.status, 0);
		assert.match(result.stderr, /overlapping outputs|normalized relative path|overlaps declared inputs/);
		await assert.rejects(access(state.output));
	}
});

test("a missing script is an error even when pnpm could match a pattern", async (t) => {
	const state = await fixture(t);
	const result = state.run(undefined, "/build/");
	assert.notEqual(result.status, 0);
	assert.match(result.stderr, /must name one package.json script/);
});


test("source manifests must agree with the frozen dependency artifact", async (t) => {
	const state = await fixture(t);
	await writeFile(join(state.source, "package.json"), JSON.stringify({ scripts: { build: "node build.mjs" } }));
	const result = state.run();
	assert.notEqual(result.status, 0);
	assert.match(result.stderr, /frozen install manifest differs/);
	await assert.rejects(access(state.output));
});
