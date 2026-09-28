//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Runs a native package script and publishes its declared output closure.

import { spawnSync } from "node:child_process";
import { cp, lstat, mkdir, readdir, readFile, realpath, symlink, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { parseArgs } from "node:util";

import { exists, isWithin, prepareWorkspace, requireRelativePath } from "./workspace.ts";

type OutputKind = "file" | "directory";

/** Reject ambiguous ownership before executing the package script. */
function outputs(value: string): ReadonlyMap<string, OutputKind> {
	const parsed: unknown = JSON.parse(value);
	if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) throw new Error("outputs must be a path-to-kind object");
	const result = new Map<string, OutputKind>();
	for (const [path, kind] of Object.entries(parsed)) {
		if (requireRelativePath("output", path) !== path || path.split("/").includes("node_modules")) throw new Error(`invalid output '${path}'`);
		if (kind !== "file" && kind !== "directory") throw new Error(`invalid output kind '${kind}'`);
		for (const other of result.keys()) {
			if (path.startsWith(`${other}/`) || other.startsWith(`${path}/`)) throw new Error(`overlapping outputs '${path}' and '${other}'`);
		}
		result.set(path, kind);
	}
	if (result.size === 0) throw new Error("at least one output is required");
	return result;
}

/** Require portable output trees with no links back into mutable scratch or dependencies. */
async function validateOutput(path: string, kind: OutputKind): Promise<void> {
	const stat = await lstat(path);
	if (kind === "file" ? !stat.isFile() : !stat.isDirectory()) throw new Error(`required ${kind} '${path}' has the wrong type`);
	if (kind === "directory") {
		for (const entry of await readdir(path, { withFileTypes: true })) {
			await validateOutput(join(path, entry.name), entry.isDirectory() ? "directory" : "file");
		}
	}
}

/** Execute a pinned pnpm script in an owned workspace, then validate all outputs before publication. */
async function main(): Promise<void> {
	const names = ["source", "install", "pnpm", "package", "script", "outputs", "output"];
	const { values } = parseArgs({ options: Object.fromEntries(names.map((name) => [name, { type: "string" as const }])) });
	/** Read one nonempty required option. */
	function required(name: string): string {
		const value = values[name];
		if (typeof value !== "string" || value === "") throw new Error(`--${name} is required`);
		return value;
	}
	const source = resolve(required("source"));
	const install = resolve(required("install"));
	const output = resolve(required("output"));
	const pnpm = resolve(required("pnpm"));
	const packageRoot = required("package") === "." ? "" : requireRelativePath("package", required("package"));
	const script = required("script");
	if (script.startsWith("-") || script.startsWith("/")) throw new Error("script must name one package.json script");
	const declared = outputs(required("outputs"));
	if (process.platform === "win32") throw new Error("pnpm_task requires a POSIX host shell");
	const workspace = await prepareWorkspace(install, source, output);
	const cwd = await realpath(join(workspace, packageRoot));
	const manifest = JSON.parse(await readFile(join(cwd, "package.json"), "utf8")) as { scripts?: Record<string, unknown> };
	if (typeof manifest.scripts?.[script] !== "string" || manifest.scripts[script] === "") throw new Error(`package does not declare script '${script}'`);
	for (const path of declared.keys()) {
		if (await exists(join(cwd, path))) throw new Error(`output '${path}' overlaps declared inputs`);
	}
	const state = join(workspace, ".bsmr");
	if (await exists(state)) throw new Error("sources may not use the reserved .bsmr directory");
	const bin = join(state, "bin");
	await mkdir(bin, { recursive: true });
	await symlink(process.execPath, join(bin, "node"));
	await writeFile(join(bin, "pnpm"), `#!/usr/bin/env node\nimport(${JSON.stringify(pathToFileURL(pnpm).href)});\n`, { mode: 0o755 });
	const userconfig = join(state, "npmrc");
	await writeFile(userconfig, "");
	const result = spawnSync(process.execPath, [pnpm, "run", script], {
		cwd, stdio: "inherit",
		env: {
			CI: "1", LANG: "C", LC_ALL: "C", TZ: "UTC", SOURCE_DATE_EPOCH: "0",
			HOME: state, TMPDIR: state, XDG_CONFIG_HOME: state, XDG_CACHE_HOME: state,
			PATH: `${bin}:/usr/bin:/bin`,
			NPM_CONFIG_USERCONFIG: userconfig, npm_config_userconfig: userconfig,
			NPM_CONFIG_MANAGE_PACKAGE_MANAGER_VERSIONS: "false", pnpm_config_pm_on_fail: "error",
			NPM_CONFIG_UPDATE_NOTIFIER: "false", pnpm_config_update_notifier: "false",
			// Dependency acquisition belongs to the frozen install action, including nested pnpm calls.
			pnpm_config_verify_deps_before_run: "false",
			NPM_CONFIG_SCRIPT_SHELL: "/bin/sh", npm_config_script_shell: "/bin/sh",
		},
	});
	if (result.error !== undefined) throw result.error;
	if (result.status !== 0) throw new Error(`pnpm task '${script}' failed: ${result.signal ?? result.status}`);
	for (const [path, kind] of declared) {
		const produced = join(cwd, path);
		if (!isWithin(cwd, await realpath(produced))) throw new Error(`output '${path}' escapes its package`);
		await validateOutput(produced, kind);
	}
	for (const path of declared.keys()) {
		await mkdir(dirname(join(output, path)), { recursive: true });
		await cp(join(cwd, path), join(output, path), { recursive: true, errorOnExist: true, force: false });
	}
}

main().catch((error: unknown) => {
	console.error(error instanceof Error ? error.message : String(error));
	process.exitCode = 1;
});
