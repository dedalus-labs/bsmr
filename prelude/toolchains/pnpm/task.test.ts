//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Exercises native task output contracts through the real pnpm CLI.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { access, mkdir, mkdtemp, readFile, realpath, rm, stat, writeFile } from "node:fs/promises";
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

for (const installedSource of [true, false]) {
	test(`workspace executables use declared sources with install source ${installedSource}`, async (t) => {
		const root = await mkdtemp(join(tmpdir(), "bsmr workspace bin "));
		t.after(() => rm(root, { recursive: true, force: true }));
		const source = join(root, "source");
		const install = join(root, "install");
		const output = join(root, "output");
		const manifests = {
			"": { name: "fixture", private: true },
			app: { name: "app", scripts: { build: "registry-tool && local-tool" }, dependencies: { "local-tool": "workspace:*", "registry-tool": "file:../vendor" } },
			tool: { name: "local-tool", version: "1.0.0", bin: { "local-tool": "bin/cli.cjs" } },
		};
		for (const directory of [source, install]) {
			for (const [path, manifest] of Object.entries(manifests)) {
				await mkdir(join(directory, path), { recursive: true });
				await writeFile(join(directory, path, "package.json"), JSON.stringify(manifest));
			}
			await writeFile(join(directory, "pnpm-workspace.yaml"), "packages:\n  - app\n  - tool\n");
		}
		/** Emit a tool whose result identifies the file that executed. */
		async function tool(directory: string, file: string, value: string): Promise<void> {
			await mkdir(directory, { recursive: true });
			await writeFile(join(directory, "cli.cjs"), `#!/usr/bin/env node\r\nrequire("node:fs").writeFileSync(${JSON.stringify(file)}, ${JSON.stringify(value)});\n`, { mode: 0o644 });
		}
		await tool(join(source, "tool/bin"), "result.txt", "declared source");
		if (installedSource) await tool(join(install, "tool/bin"), "result.txt", "stale install source");
		await tool(join(install, "vendor"), "external.txt", "frozen dependency");
		await writeFile(join(install, "vendor/package.json"), JSON.stringify({ name: "registry-tool", version: "1.0.0", bin: { "registry-tool": "cli.cjs" } }));
		const acquired = spawnSync(process.execPath, [pnpm!, "install", "--offline", "--ignore-scripts", "--config.prefer-symlinked-executables=true", "--store-dir", join(root, "store")], {
			cwd: install, encoding: "utf8", env: { PATH: process.env["PATH"], HOME: root, XDG_CACHE_HOME: join(root, "cache"), pnpm_config_update_notifier: "false", pnpm_config_pm_on_fail: "error" },
		});
		assert.equal(acquired.status, 0, acquired.stdout + acquired.stderr);
		const result = spawnSync(process.execPath, [runner,
			"--source", source, "--install", install, "--pnpm", pnpm!,
			"--package", "app", "--script", "build", "--output", output,
			"--outputs", JSON.stringify({ "result.txt": "file", "external.txt": "file" }),
		], { encoding: "utf8", env: { BSMR_SCRATCH_PATH: join(root, "scratch") } });
		assert.equal(result.status, 0, result.stdout + result.stderr);
		assert.equal(await readFile(join(output, "result.txt"), "utf8"), "declared source");
		assert.equal(await readFile(join(output, "external.txt"), "utf8"), "frozen dependency");
		assert.equal(await realpath(join(root, "scratch/package-workspace/app/node_modules/.bin/local-tool")), await realpath(join(root, "scratch/package-workspace/tool/bin/cli.cjs")));
		assert.equal((await stat(join(source, "tool/bin/cli.cjs"))).mode & 0o777, 0o644, "declared source modes remain unchanged");
		assert.match(await readFile(join(source, "tool/bin/cli.cjs"), "utf8"), /^#![^\n]+\r\n/, "declared shebang remains unchanged");
		if (installedSource) assert.match(await readFile(join(install, "tool/bin/cli.cjs"), "utf8"), /stale install source/);
		else await assert.rejects(access(join(install, "tool/bin/cli.cjs")));
	});
}
