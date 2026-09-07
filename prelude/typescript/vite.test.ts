//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies Vite build integration against hermetic pnpm installations.

import assert from "node:assert/strict";
import { spawnSync, type SpawnSyncReturns } from "node:child_process";
import { access, mkdir, mkdtemp, readFile, readdir, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test, type TestContext } from "node:test";
import { fileURLToPath } from "node:url";

const runner = fileURLToPath(new URL("./runner.mjs", import.meta.url));

type Fixture = Readonly<{
	declared: string;
	install: string;
	output: string;
	packageRoot: string;
	root: string;
	scratch: string;
	source: string;
}>;

/**
 * Write one file after creating its parent directory.
 *
 * @param path - Absolute destination path.
 * @param contents - UTF-8 file contents.
 */
async function write(path: string, contents: string): Promise<void> {
	await mkdir(dirname(path), { recursive: true });
	await writeFile(path, contents);
}

/**
 * Represent one declared source as the symlink emitted by a BSMR source tree.
 *
 * @param source - Symlink-tree root.
 * @param declared - Real declared-file root.
 * @param path - Workspace-relative source path.
 * @param contents - UTF-8 file contents.
 */
async function declare(source: string, declared: string, path: string, contents: string): Promise<void> {
	const input = join(declared, path);
	const output = join(source, path);
	await write(input, contents);
	await mkdir(dirname(output), { recursive: true });
	await symlink(input, output);
}

/**
 * Create a mock Vite installation with build capability.
 *
 * @param context - Active test context.
 * @param packageRoot - Workspace-relative package root.
 * @returns Paths for the isolated fixture.
 */
async function viteFixture(context: TestContext, packageRoot = "packages/app"): Promise<Fixture> {
	const root = await mkdtemp(join(tmpdir(), "bsmr-vite-runner-"));
	context.after(() => rm(root, { force: true, recursive: true }));
	const install = join(root, "install");
	const declared = join(root, "declared");
	const source = join(root, "source");
	const scratch = join(root, "scratch");
	const output = join(root, "output");

	await declare(source, declared, join(packageRoot, "package.json"), '{"name":"@demo/app","type":"module"}\n');
	await declare(source, declared, join(packageRoot, "index.html"), '<!DOCTYPE html><html><body></body></html>\n');
	await declare(source, declared, join(packageRoot, "src/main.tsx"), 'console.log("Hello Vite");\n');
	await declare(
		source,
		declared,
		join(packageRoot, "vite.config.ts"),
		'export default { build: { outDir: "dist" } };\n',
	);

	await write(join(install, packageRoot, "package.json"), '{"name":"@demo/app","type":"module"}\n');
	await write(
		join(install, "tools/vite/package.json"),
		'{"name":"vite","type":"module","bin":{"vite":"./bin/vite.mjs"}}\n',
	);
	await write(
		join(install, "tools/vite/bin/vite.mjs"),
		`import { access, mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
const args = process.argv.slice(2);
if (args[0] !== "build") process.exit(1);
const configIndex = args.indexOf("--config");
if (configIndex === -1 || args[configIndex + 1] !== "vite.config.ts") process.exit(2);
const outDirIndex = args.indexOf("--outDir");
if (outDirIndex === -1) process.exit(3);
const outDir = args[outDirIndex + 1];
await access(join(process.cwd(), "src/main.tsx"));
await access(join(process.cwd(), "index.html"));
await mkdir(outDir, { recursive: true });
await mkdir(join(outDir, "assets"), { recursive: true });
await writeFile(join(outDir, "index.html"), "<!DOCTYPE html><html><body>Built</body></html>\\n");
await writeFile(join(outDir, "assets/main.js"), "console.log('Vite built');\\n");
`,
	);

	await mkdir(join(install, packageRoot, "node_modules"), { recursive: true });
	await symlink(join(install, "tools/vite"), join(install, packageRoot, "node_modules/vite"));
	return { declared, install, output, packageRoot: packageRoot || ".", root, scratch, source };
}

/**
 * Execute the Vite runner with the exact current Node runtime.
 *
 * @param state - Fixture paths.
 * @returns The completed runner process.
 */
function runVite(state: Fixture): SpawnSyncReturns<string> {
	return spawnSync(
		process.execPath,
		[
			runner,
			"--config",
			"vite.config.ts",
			"--install",
			state.install,
			"--mode",
			"vite",
			"--output",
			state.output,
			"--package-root",
			state.packageRoot,
			"--source",
			state.source,
		],
		{ encoding: "utf8", env: { ...process.env, BSMR_SCRATCH_PATH: state.scratch } },
	);
}

test("builds a Vite application with the package-local locked Vite binary", async (context) => {
	const state = await viteFixture(context);
	const result = runVite(state);
	assert.equal(result.status, 0, result.stderr);
	const indexHtml = await readFile(join(state.output, "index.html"), "utf8");
	assert.match(indexHtml, /Built/);
	const mainJs = await readFile(join(state.output, "assets/main.js"), "utf8");
	assert.match(mainJs, /Vite built/);
});

test("builds a Vite application at the workspace root", async (context) => {
	const state = await viteFixture(context, "");
	const result = runVite(state);
	assert.equal(result.status, 0, result.stderr);
	const files = await readdir(state.output);
	assert.ok(files.includes("index.html"));
});

test("fails when Vite is not declared in the package dependencies", async (context) => {
	const state = await viteFixture(context);
	await rm(join(state.install, "packages/app/node_modules/vite"));
	const result = runVite(state);
	assert.equal(result.status, 1);
	assert.match(result.stderr, /package-local tool 'vite' is unavailable/);
	await assert.rejects(access(state.output));
});

test("fails when Vite produces an empty build output", async (context) => {
	const state = await viteFixture(context);
	await write(
		join(state.install, "tools/vite/bin/vite.mjs"),
		`import { mkdir } from "node:fs/promises";
const args = process.argv.slice(2);
const outDirIndex = args.indexOf("--outDir");
const outDir = args[outDirIndex + 1];
await mkdir(outDir, { recursive: true });
`,
	);
	const result = runVite(state);
	assert.equal(result.status, 1);
	assert.match(result.stderr, /vite produced empty output/);
});

test("rejects Vite execution when source files are missing", async (context) => {
	const state = await viteFixture(context);
	await rm(join(state.source, state.packageRoot, "src"), { recursive: true, force: true });
	const result = runVite(state);
	assert.equal(result.status, 1);
});

test("preserves workspace dependency links for Vite builds", async (context) => {
	const state = await viteFixture(context);
	await declare(state.source, state.declared, "packages/shared/package.json", '{"name":"@demo/shared"}\n');
	await declare(state.source, state.declared, "packages/shared/index.ts", 'export const value = 42;\n');
	await write(join(state.install, "packages/shared/package.json"), '{"name":"@demo/shared"}\n');
	await mkdir(join(state.install, state.packageRoot, "node_modules/@demo"), { recursive: true });
	await symlink(join(state.install, "packages/shared"), join(state.install, state.packageRoot, "node_modules/@demo/shared"));

	const result = runVite(state);
	assert.equal(result.status, 0, result.stderr);
});
