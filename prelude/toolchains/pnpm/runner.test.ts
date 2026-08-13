//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies the pnpm install runner's toolchain, manifest, and state-isolation invariants.

import assert from "node:assert/strict";
import { spawnSync, type SpawnSyncReturns } from "node:child_process";
import { access, mkdtemp, mkdir, readFile, readlink, realpath, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, type TestContext } from "node:test";
import { fileURLToPath } from "node:url";

const packageManager = `pnpm@11.20.0+sha512.${"a".repeat(128)}`;
const pnpm10PackageManager = `pnpm@10.30.3+sha512.${"b".repeat(128)}`;
const runner = fileURLToPath(new URL("./runner.mjs", import.meta.url));

type Fixture = Readonly<{ root: string; source: string; output: string; scratch: string; cli: string }>;
type Invocation = Readonly<{
	args: string[];
	cwd: string;
	execPath: string;
	env: Readonly<Record<string, string | null>>;
}>;

/**
 * Create an isolated pnpm project and recording CLI.
 *
 * @param context - Active test context.
 * @param projectPackageManager - Exact pnpm pin written to package.json.
 * @returns Paths for the isolated fixture.
 */
async function fixture(context: TestContext, projectPackageManager = packageManager): Promise<Fixture> {
	const root = await mkdtemp(join(tmpdir(), "bsmr-pnpm-runner-"));
	context.after(() => rm(root, { force: true, recursive: true }));
	const source = join(root, "source");
	const output = join(root, "output");
	const scratch = join(root, "scratch");
	const cli = join(root, "pnpm.cjs");
	const cliVersion = /^pnpm@(\d+\.\d+\.\d+)/.exec(projectPackageManager)?.[1];
	if (cliVersion === undefined) throw new Error(`test package manager '${projectPackageManager}' is invalid`);
	await mkdir(source);
	await writeFile(
		join(source, "package.json"),
		JSON.stringify({ engines: { node: process.versions.node }, packageManager: projectPackageManager }),
	);
	await writeFile(join(source, "pnpm-lock.yaml"), "lockfileVersion: '9.0'\n");
	await writeFile(join(source, "input.txt"), "declared input\n");
	await writeFile(
		cli,
		`const { mkdirSync, symlinkSync, writeFileSync } = require("node:fs");
if (["--version", "with current --version"].includes(process.argv.slice(2).join(" "))) {
  console.log("${cliVersion}");
  process.exit(0);
}
mkdirSync("node_modules/.bin", { recursive: true });
writeFileSync("node_modules/installed.txt", "installed\\n");
symlinkSync("../installed.txt", "node_modules/.bin/tool");
writeFileSync("node_modules/.modules.yaml", "prunedAt: now\\nstoreDir: /absolute/store\\n");
writeFileSync("node_modules/.pnpm-workspace-state-v1.json", JSON.stringify({ lastValidatedTimestamp: Date.now() }));
writeFileSync(".pnpm-invocation.json", JSON.stringify({
  args: process.argv.slice(2),
  cwd: process.cwd(),
  execPath: process.execPath,
	  env: {
    corepackHome: process.env.COREPACK_HOME,
    home: process.env.HOME,
    npmConfigManagePackageManagerVersions: process.env.NPM_CONFIG_MANAGE_PACKAGE_MANAGER_VERSIONS,
    npmConfigUpdateNotifier: process.env.NPM_CONFIG_UPDATE_NOTIFIER,
    npmConfigUserconfig: process.env.NPM_CONFIG_USERCONFIG,
    npmConfigUserconfigLower: process.env.npm_config_userconfig,
    pnpmHome: process.env.PNPM_HOME,
    pnpmConfigNpmrcAuthFile: process.env.pnpm_config_npmrc_auth_file,
    pnpmConfigPmOnFail: process.env.pnpm_config_pm_on_fail,
    pnpmConfigUpdateNotifier: process.env.pnpm_config_update_notifier,
    xdgCacheHome: process.env.XDG_CACHE_HOME,
	    xdgConfigHome: process.env.XDG_CONFIG_HOME,
	    undeclared: process.env.BSMR_UNDECLARED ?? null,
	  },
}));
`,
	);
	return { root, source, output, scratch, cli };
}

/**
 * Execute the runner with the current exact Node runtime.
 *
 * @param state - Test fixture paths.
 * @param expectedPackageManager - Manifest pin the runner must require.
 * @param expectedNodeRequirement - Manifest Node range the runner must require.
 * @returns The completed runner process.
 */
function runRunner(
	state: Fixture,
	expectedPackageManager = packageManager,
	expectedNodeRequirement = process.versions.node,
): SpawnSyncReturns<string> {
	return spawnSync(
		process.execPath,
		[
			runner,
			"--source",
			state.source,
			"--output",
			state.output,
			"--pnpm-cli",
			state.cli,
			"--package-manager",
			expectedPackageManager,
			"--node-version",
			process.versions.node,
			"--node-requirement",
			expectedNodeRequirement,
		],
		{
			encoding: "utf8",
			env: {
				...process.env,
				BSMR_UNDECLARED: "ambient",
				BUCK_SCRATCH_PATH: state.scratch,
				HOME: join(state.root, "ambient-home"),
				NPM_CONFIG_USERCONFIG: join(state.root, "ambient-npmrc"),
				XDG_CONFIG_HOME: join(state.root, "ambient-xdg-config"),
			},
		},
	);
}

test("installs once with the exact toolchain and action-local state", async (context) => {
	const state = await fixture(context);
	const result = runRunner(state);
	assert.equal(result.status, 0, result.stderr);

	const invocation = JSON.parse(await readFile(join(state.output, ".pnpm-invocation.json"), "utf8")) as Invocation;
	assert.deepEqual(invocation.args, [
		"with",
		"current",
		"install",
		"--frozen-lockfile",
		"--ignore-scripts",
		"--store-dir",
		join(state.scratch, "pnpm", "pnpm-store"),
		"--config.prefer-symlinked-executables=true",
	]);
	assert.equal(invocation.cwd, await realpath(state.output));
	assert.equal(invocation.execPath, process.execPath);
	assert.deepEqual(invocation.env, {
		corepackHome: join(state.scratch, "pnpm", "corepack"),
		home: join(state.scratch, "pnpm", "home"),
		npmConfigManagePackageManagerVersions: "false",
		npmConfigUpdateNotifier: "false",
		npmConfigUserconfig: join(state.scratch, "pnpm", "empty-npmrc"),
		npmConfigUserconfigLower: join(state.scratch, "pnpm", "empty-npmrc"),
		pnpmHome: join(state.scratch, "pnpm", "pnpm-home"),
		pnpmConfigNpmrcAuthFile: join(state.scratch, "pnpm", "empty-npmrc"),
		pnpmConfigPmOnFail: "error",
		pnpmConfigUpdateNotifier: "false",
		xdgCacheHome: join(state.scratch, "pnpm", "xdg-cache"),
		xdgConfigHome: join(state.scratch, "pnpm", "xdg-config"),
		undeclared: null,
	});
	assert.equal(await readFile(join(state.output, "input.txt"), "utf8"), "declared input\n");
	assert.equal(await readFile(join(state.output, "node_modules", "installed.txt"), "utf8"), "installed\n");
	assert.equal(await readlink(join(state.output, "node_modules", ".bin", "tool")), "../installed.txt");
	await assert.rejects(access(join(state.output, ".bsmr")));
	await assert.rejects(access(join(state.output, "node_modules", ".modules.yaml")));
	await assert.rejects(access(join(state.output, "node_modules", ".pnpm-workspace-state-v1.json")));
	await access(join(state.scratch, "pnpm", "pnpm-store"));
});

test("uses pnpm 10 without the pnpm 11 toolchain bypass", async (context) => {
	const state = await fixture(context, pnpm10PackageManager);
	const result = runRunner(state, pnpm10PackageManager);
	assert.equal(result.status, 0, result.stderr);

	const invocation = JSON.parse(await readFile(join(state.output, ".pnpm-invocation.json"), "utf8")) as Invocation;
	assert.deepEqual(invocation.args, [
		"install",
		"--frozen-lockfile",
		"--ignore-scripts",
		"--store-dir",
		join(state.scratch, "pnpm", "pnpm-store"),
		"--config.prefer-symlinked-executables=true",
	]);
});

test("accepts the Node range validated by the native frontend", async (context) => {
	const state = await fixture(context);
	const nodeRequirement = ">=24.0.0";
	await writeFile(
		join(state.source, "package.json"),
		JSON.stringify({ engines: { node: nodeRequirement }, packageManager }),
	);
	const result = runRunner(state, packageManager, nodeRequirement);
	assert.equal(result.status, 0, result.stderr);
});

test("rejects mutable state inside the cached output", async (context) => {
	const state = await fixture(context);
	const result = runRunner({ ...state, scratch: state.output });
	assert.equal(result.status, 1);
	assert.match(result.stderr, /mutable state .* must be outside cached output/);
	await assert.rejects(access(join(state.output, ".pnpm-invocation.json")));
});

test("rejects a nonrelocatable executable shim", async (context) => {
	const state = await fixture(context);
	const cli = await readFile(state.cli, "utf8");
	await writeFile(
		state.cli,
		cli.replace(
			'symlinkSync("../installed.txt", "node_modules/.bin/tool");',
			'writeFileSync("node_modules/.bin/tool", "/absolute/build/path");',
		),
	);
	const result = runRunner(state);
	assert.equal(result.status, 1);
	assert.match(result.stderr, /must be a relative symlink/);
});

test("accepts an install without package executables or mutable metadata", async (context) => {
	const state = await fixture(context);
	const cli = await readFile(state.cli, "utf8");
	await writeFile(
		state.cli,
		cli
			.replace('mkdirSync("node_modules/.bin", { recursive: true });', 'mkdirSync("node_modules");')
			.replace('symlinkSync("../installed.txt", "node_modules/.bin/tool");', "")
			.replace('writeFileSync("node_modules/.modules.yaml", "prunedAt: now\\nstoreDir: /absolute/store\\n");', "")
			.replace(
				'writeFileSync("node_modules/.pnpm-workspace-state-v1.json", JSON.stringify({ lastValidatedTimestamp: Date.now() }));',
				"",
			),
	);
	const result = runRunner(state);
	assert.equal(result.status, 0, result.stderr);
});

test("rejects a packageManager mismatch before invoking pnpm", async (context) => {
	const state = await fixture(context);
	const result = runRunner(state, `pnpm@11.19.0+sha512.${"b".repeat(128)}`);
	assert.equal(result.status, 1);
	assert.match(result.stderr, /package\.json packageManager/);
	await assert.rejects(access(join(state.output, ".pnpm-invocation.json")));
});

test("rejects an unsupported pnpm major before invoking pnpm", async (context) => {
	const unsupported = `pnpm@12.0.0+sha512.${"c".repeat(128)}`;
	const state = await fixture(context, unsupported);
	const result = runRunner(state, unsupported);
	assert.equal(result.status, 1);
	assert.match(result.stderr, /support exact pnpm 10 and 11 versions/);
	await assert.rejects(access(join(state.output, ".pnpm-invocation.json")));
});

test("rejects a pnpm bundle that lies about its configured version", async (context) => {
	const state = await fixture(context);
	const cli = await readFile(state.cli, "utf8");
	await writeFile(state.cli, cli.replace('console.log("11.20.0")', 'console.log("11.19.0")'));
	const result = runRunner(state);
	assert.equal(result.status, 1);
	assert.match(result.stderr, /pnpm CLI 11\.19\.0 does not match configured version 11\.20\.0/);
	await assert.rejects(access(join(state.output, ".pnpm-invocation.json")));
});

test("rejects a Node mismatch before invoking pnpm", async (context) => {
	const state = await fixture(context);
	const result = spawnSync(
		process.execPath,
		[
			runner,
			"--source",
			state.source,
			"--output",
			state.output,
			"--pnpm-cli",
			state.cli,
			"--package-manager",
			packageManager,
			"--node-version",
			"0.0.0",
			"--node-requirement",
			process.versions.node,
		],
		{ encoding: "utf8" },
	);
	assert.equal(result.status, 1);
	assert.match(result.stderr, /Node runtime/);
	await assert.rejects(access(join(state.output, ".pnpm-invocation.json")));
});

test("rejects an engines.node mismatch before invoking pnpm", async (context) => {
	const state = await fixture(context);
	await writeFile(
		join(state.source, "package.json"),
		JSON.stringify({ engines: { node: "0.0.0" }, packageManager }),
	);
	const result = runRunner(state);
	assert.equal(result.status, 1);
	assert.match(result.stderr, /engines\.node/);
	await assert.rejects(access(join(state.output, ".pnpm-invocation.json")));
});

test("requires the authoritative lockfile before invoking pnpm", async (context) => {
	const state = await fixture(context);
	await rm(join(state.source, "pnpm-lock.yaml"));
	const result = runRunner(state);
	assert.equal(result.status, 1);
	assert.match(result.stderr, /pnpm-lock\.yaml/);
	await assert.rejects(access(join(state.output, ".pnpm-invocation.json")));
});

test("reserves the action-local state path", async (context) => {
	const state = await fixture(context);
	await mkdir(join(state.source, ".bsmr"));
	const result = runRunner(state);
	assert.equal(result.status, 1);
	assert.match(result.stderr, /reserved '.bsmr'/);
	await assert.rejects(access(join(state.output, ".pnpm-invocation.json")));
});

test("refuses to overwrite an existing action output", async (context) => {
	const state = await fixture(context);
	await mkdir(state.output);
	const result = runRunner(state);
	assert.equal(result.status, 1);
	assert.match(result.stderr, /already exists/);
});
