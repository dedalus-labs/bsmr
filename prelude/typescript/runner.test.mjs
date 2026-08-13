//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies hermetic TypeScript actions against a manifest-only pnpm installation.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { access, mkdir, mkdtemp, readFile, realpath, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const runner = fileURLToPath(new URL("./runner.mjs", import.meta.url));

/**
 * Write one file after creating its parent directory.
 *
 * @param {string} path - Absolute destination path.
 * @param {string} contents - UTF-8 file contents.
 * @returns {Promise<void>}
 */
async function write(path, contents) {
	await mkdir(dirname(path), { recursive: true });
	await writeFile(path, contents);
}

/**
 * Represent one declared source as the symlink emitted by a BSMR source tree.
 *
 * @param {string} source - Symlink-tree root.
 * @param {string} declared - Real declared-file root.
 * @param {string} path - Workspace-relative source path.
 * @param {string} contents - UTF-8 file contents.
 * @returns {Promise<void>}
 */
async function declare(source, declared, path, contents) {
	const input = join(declared, path);
	const output = join(source, path);
	await write(input, contents);
	await mkdir(dirname(output), { recursive: true });
	await symlink(input, output);
}

/**
 * Create source and install trees with one internal workspace dependency.
 *
 * @param {import("node:test").TestContext} context - Active test context.
 * @param {string} [packageRoot] - Workspace-relative package root, or an empty string for the workspace root.
 * @returns {Promise<{ install: string, output: string, packageRoot: string, root: string, scratch: string, source: string }>}
 */
async function fixture(context, packageRoot = "packages/app") {
	const root = await mkdtemp(join(tmpdir(), "bsmr-typescript-runner-"));
	context.after(() => rm(root, { force: true, recursive: true }));
	const install = join(root, "install");
	const declared = join(root, "declared");
	const source = join(root, "source");
	const scratch = join(root, "scratch");
	const output = join(root, "output");

	await declare(source, declared, join(packageRoot, "package.json"), '{"name":"@demo/app"}\n');
	await declare(source, declared, join(packageRoot, "src/index.ts"), "export const answer = 42;\n");
	await declare(source, declared, join(packageRoot, "tsconfig.json"), '{"extends":"@demo/config/base.json"}\n');
	await declare(source, declared, join(packageRoot, "tsdown.config.ts"), "export default {};\n");
	await declare(source, declared, "packages/config/package.json", '{"name":"@demo/config"}\n');
	await declare(source, declared, "packages/config/base.json", '{"compilerOptions":{"strict":true}}\n');

	await write(join(install, packageRoot, "package.json"), '{"name":"@demo/app"}\n');
	await write(join(install, "packages/config/package.json"), '{"name":"@demo/config"}\n');
	await write(
		join(install, "tools/typescript/package.json"),
		'{"name":"typescript","type":"module","bin":{"tsc":"./bin/tsc.mjs"}}\n',
	);
	await write(
		join(install, "tools/typescript/bin/tsc.mjs"),
		`import { access, readFile } from "node:fs/promises";
import { join } from "node:path";
await access(join(process.cwd(), "src/index.ts"));
const config = await readFile(join(process.cwd(), "node_modules/@demo/config/base.json"), "utf8");
if (!config.includes("strict")) process.exit(2);
if (process.argv.slice(2).join(" ") !== "--project tsconfig.json --noEmit --pretty false") process.exit(3);
if (process.env.BSMR_UNDECLARED !== undefined) process.exit(4);
`,
	);
	await write(
		join(install, "tools/tsdown/package.json"),
		'{"name":"tsdown","type":"module","bin":{"tsdown":"./dist/run.mjs"}}\n',
	);
	await write(
		join(install, "tools/tsdown/dist/run.mjs"),
		`import { access, mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
await access(join(process.cwd(), "src/index.ts"));
const config = await readFile(join(process.cwd(), "node_modules/@demo/config/base.json"), "utf8");
if (!config.includes("strict")) process.exit(2);
const args = process.argv.slice(2);
if (args.slice(0, 2).join(" ") !== "--config tsdown.config.ts" || args[2] !== "--out-dir") process.exit(3);
await mkdir(args[3]);
await writeFile(join(args[3], "index.js"), "export const answer = 42;\\n");
`,
	);

	await mkdir(join(install, packageRoot, "node_modules/@demo"), { recursive: true });
	await symlink(join(install, "tools/typescript"), join(install, packageRoot, "node_modules/typescript"));
	await symlink(join(install, "tools/tsdown"), join(install, packageRoot, "node_modules/tsdown"));
	await symlink(join(install, "packages/config"), join(install, packageRoot, "node_modules/@demo/config"));
	return { install, output, packageRoot: packageRoot || ".", root, scratch, source };
}

/**
 * Execute one runner mode with the exact current Node runtime.
 *
 * @param {{ install: string, output: string, packageRoot: string, root: string, scratch: string, source: string }} state - Fixture paths.
 * @param {"library" | "typecheck"} mode - TypeScript action mode.
 * @returns {import("node:child_process").SpawnSyncReturns<string>}
 */
function runRunner(state, mode) {
	const config = mode === "library" ? "tsdown.config.ts" : "tsconfig.json";
	return spawnSync(
		process.execPath,
		[
			runner,
			"--config",
			config,
			"--install",
			state.install,
			"--mode",
			mode,
			"--output",
			state.output,
			"--package-root",
			state.packageRoot,
			"--source",
			state.source,
		],
		{ encoding: "utf8", env: { ...process.env, BSMR_UNDECLARED: "ambient", BUCK_SCRATCH_PATH: state.scratch } },
	);
}

test("typechecks declared sources through reconstructed pnpm workspace links", async (context) => {
	const state = await fixture(context);
	const result = runRunner(state, "typecheck");
	assert.equal(result.status, 0, result.stderr);
	assert.equal(await readFile(state.output, "utf8"), "ok\n");
	assert.equal(
		await realpath(join(state.scratch, "typescript-workspace/packages/app/node_modules/@demo/config")),
		await realpath(join(state.scratch, "typescript-workspace/packages/config")),
	);
});

test("typechecks a package at the workspace root", async (context) => {
	const state = await fixture(context, "");
	const result = runRunner(state, "typecheck");
	assert.equal(result.status, 0, result.stderr);
	assert.equal(await readFile(state.output, "utf8"), "ok\n");
});

test("emits a library with the package-local locked tsdown binary", async (context) => {
	const state = await fixture(context);
	const result = runRunner(state, "library");
	assert.equal(result.status, 0, result.stderr);
	assert.equal(await readFile(join(state.output, "index.js"), "utf8"), "export const answer = 42;\n");
});

test("fails before execution when the package does not declare its compiler", async (context) => {
	const state = await fixture(context);
	await rm(join(state.install, "packages/app/node_modules/typescript"));
	const result = runRunner(state, "typecheck");
	assert.equal(result.status, 1);
	assert.match(result.stderr, /package-local tool 'typescript' is unavailable/);
	await assert.rejects(access(state.output));
});
