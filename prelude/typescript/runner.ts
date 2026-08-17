//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Executes pinned TypeScript tools in a source overlay over a frozen pnpm install.

import { spawnSync } from "node:child_process";
import { copyFile, lstat, mkdir, readdir, readFile, realpath, rm, symlink, writeFile } from "node:fs/promises";
import { isAbsolute, join, relative, resolve, sep } from "node:path";

const requiredArguments = new Set(["--config", "--install", "--mode", "--output", "--package-root", "--source"]);

type Mode = "library" | "typecheck";
type RunnerOptions = Readonly<{
	config: string;
	install: string;
	mode: Mode;
	output: string;
	packageRoot: string;
	source: string;
}>;

/** Return one required value from the validated command-line map. */
function requiredArgument(values: ReadonlyMap<string, string>, name: string): string {
	const value = values.get(name);
	if (value === undefined) throw new Error(`missing required argument '${name}'`);
	return value;
}

/**
 * Parse the runner's closed command-line schema.
 *
 * @param arguments_ - Command-line words after the entrypoint.
 * @returns Validated runner options.
 */
function parseArguments(arguments_: readonly string[]): RunnerOptions {
	const values = new Map<string, string>();
	for (let index = 0; index < arguments_.length; index += 2) {
		const name = arguments_[index];
		const value = arguments_[index + 1];
		if (name === undefined || !requiredArguments.has(name)) throw new Error(`unknown argument '${name ?? ""}'`);
		if (value === undefined || value === "") throw new Error(`argument '${name}' requires a value`);
		if (values.has(name)) throw new Error(`argument '${name}' was provided more than once`);
		values.set(name, value);
	}
	const mode = requiredArgument(values, "--mode");
	if (mode !== "library" && mode !== "typecheck") throw new Error(`unsupported TypeScript action mode '${mode}'`);
	const config = requiredArgument(values, "--config");
	const install = requiredArgument(values, "--install");
	const output = requiredArgument(values, "--output");
	const packageRoot = requiredArgument(values, "--package-root");
	const source = requiredArgument(values, "--source");
	return {
		config: requireRelativePath("config", config),
		install: resolve(install),
		mode,
		output: resolve(output),
		packageRoot: packageRoot === "." ? "" : requireRelativePath("package root", packageRoot),
		source: resolve(source),
	};
}

/**
 * Require one normalized, non-traversing relative path.
 *
 * @param name - Diagnostic field name.
 * @param path - User-supplied relative path.
 * @returns A normalized relative path.
 */
function requireRelativePath(name: string, path: string): string {
	const components = path.replaceAll("\\", "/").split("/");
	if (isAbsolute(path) || path === "." || components.includes("") || components.includes(".") || components.includes("..")) {
		throw new Error(`${name} '${path}' must be a normalized relative path`);
	}
	return components.join("/");
}

/** Return whether an unknown failure carries the requested system error code. */
function hasErrorCode(error: unknown, code: string): error is NodeJS.ErrnoException {
	return error instanceof Error && "code" in error && error.code === code;
}

/**
 * Test whether a path exists without swallowing other filesystem failures.
 *
 * @param path - Absolute path to inspect.
 * @returns Whether the path exists.
 */
async function exists(path: string): Promise<boolean> {
	try {
		await lstat(path);
		return true;
	} catch (error) {
		if (hasErrorCode(error, "ENOENT")) return false;
		throw error;
	}
}

/**
 * Test whether a candidate is nested beneath a directory.
 *
 * @param directory - Absolute parent directory.
 * @param candidate - Absolute candidate path.
 * @returns Whether the candidate is within the directory.
 */
function isWithin(directory: string, candidate: string): boolean {
	const suffix = relative(directory, candidate);
	return suffix === "" || (!suffix.startsWith(`..${sep}`) && suffix !== ".." && !isAbsolute(suffix));
}

/**
 * Copy declared source paths into the writable scratch workspace.
 *
 * @param source - Declared source tree.
 * @param destination - Scratch workspace directory.
 */
async function copySourceTree(source: string, destination: string): Promise<void> {
	await mkdir(destination, { recursive: true });
	for (const entry of await readdir(source, { withFileTypes: true })) {
		const from = join(source, entry.name);
		const to = join(destination, entry.name);
		if (entry.isDirectory()) {
			await copySourceTree(from, to);
		} else {
			await copyFile(from, to);
		}
	}
}

/**
 * Find workspace roots represented by declared package manifests.
 *
 * @param source - Current source directory.
 * @param prefix - Source-relative directory being visited.
 * @returns Workspace-relative package roots.
 */
async function findPackageRoots(source: string, prefix = ""): Promise<string[]> {
	const directory = join(source, prefix);
	const entries = await readdir(directory, { withFileTypes: true });
	const roots = entries.some((entry) => !entry.isDirectory() && entry.name === "package.json") ? [prefix] : [];
	for (const entry of entries) {
		if (entry.isDirectory() && entry.name !== "node_modules") {
			roots.push(...(await findPackageRoots(source, join(prefix, entry.name))));
		}
	}
	return roots;
}

/**
 * Mirror one pnpm node_modules level while rebasing workspace links to scratch.
 *
 * @param input - Installed node_modules or scope directory.
 * @param output - Scratch node_modules or scope directory.
 * @param packages - Installed realpath to scratch package root.
 */
async function mirrorModules(input: string, output: string, packages: ReadonlyMap<string, string>): Promise<void> {
	await mkdir(output, { recursive: true });
	for (const entry of await readdir(input, { withFileTypes: true })) {
		if (entry.name === ".pnpm") continue;
		const from = join(input, entry.name);
		const to = join(output, entry.name);
		if (entry.isDirectory() && entry.name.startsWith("@")) {
			await mirrorModules(from, to, packages);
			continue;
		}
		const target = await realpath(from);
		await symlink(packages.get(target) ?? target, to);
	}
}

/**
 * Reconstruct package-local dependency links over the declared source overlay.
 *
 * @param install - Frozen pnpm installation workspace.
 * @param source - Declared source tree.
 * @param workspace - Scratch workspace.
 */
async function linkPackageModules(install: string, source: string, workspace: string): Promise<void> {
	const roots = await findPackageRoots(source);
	const packages = new Map<string, string>();
	for (const root of roots) {
		const installedPackage = join(install, root);
		if (!(await exists(installedPackage))) throw new Error(`frozen install is missing declared workspace package '${root}'`);
		packages.set(await realpath(installedPackage), join(workspace, root));
	}
	for (const root of roots) {
		const input = join(install, root, "node_modules");
		if (await exists(input)) await mirrorModules(input, join(workspace, root, "node_modules"), packages);
	}
}

/**
 * Resolve the package-local executable declared by a locked npm package.
 *
 * @param packageDirectory - Scratch package directory.
 * @param tool - Direct tool dependency name.
 * @param command - Expected package bin key.
 * @returns The validated JavaScript entrypoint.
 */
async function resolveTool(packageDirectory: string, tool: string, command: string): Promise<string> {
	const manifestPath = join(packageDirectory, "node_modules", ...tool.split("/"), "package.json");
	let manifest: unknown;
	try {
		manifest = JSON.parse(await readFile(manifestPath, "utf8"));
	} catch (error) {
		throw new Error(`package-local tool '${tool}' is unavailable: ${error instanceof Error ? error.message : String(error)}`);
	}
	if (manifest === null || typeof manifest !== "object" || Array.isArray(manifest)) {
		throw new Error(`package-local tool '${tool}' has an invalid package manifest`);
	}
	const binValue = (manifest as Record<string, unknown>)["bin"];
	const executable = typeof binValue === "string"
		? binValue
		: binValue !== null && typeof binValue === "object" && !Array.isArray(binValue)
			? (binValue as Record<string, unknown>)[command]
			: undefined;
	if (typeof executable !== "string") throw new Error(`package-local tool '${tool}' does not declare bin '${command}'`);
	const packageRoot = await realpath(join(packageDirectory, "node_modules", ...tool.split("/")));
	const executablePath = resolve(packageRoot, executable);
	if (!isWithin(packageRoot, executablePath)) throw new Error(`package-local tool '${tool}' declares escaping bin '${executable}'`);
	return executablePath;
}

/**
 * Run a pinned Node tool and preserve its diagnostics.
 *
 * @param executable - Absolute JavaScript entrypoint.
 * @param arguments_ - Tool arguments.
 * @param cwd - Scratch package directory.
 */
function runTool(executable: string, arguments_: readonly string[], cwd: string): void {
	const result = spawnSync(process.execPath, [executable, ...arguments_], {
		cwd,
		env: {
			CI: "1",
			FORCE_COLOR: "0",
			LANG: "C",
			LC_ALL: "C",
			NO_COLOR: "1",
			SOURCE_DATE_EPOCH: "0",
			TZ: "UTC",
		},
		stdio: "inherit",
	});
	if (result.error !== undefined) throw result.error;
	if (result.status !== 0) throw new Error(`TypeScript tool exited with status ${result.status ?? "unknown"}`);
}

/**
 * Execute one hermetic TypeScript action.
 *
 * @param arguments_ - Command-line words after the entrypoint.
 */
async function main(arguments_: readonly string[]): Promise<void> {
	const options = parseArguments(arguments_);
	if (await exists(options.output)) throw new Error(`action output '${options.output}' already exists`);
	const scratchRoot = process.env["BUCK_SCRATCH_PATH"];
	if (scratchRoot === undefined || scratchRoot === "") throw new Error("BSMR did not provide BUCK_SCRATCH_PATH");
	const workspace = resolve(scratchRoot, "typescript-workspace");
	for (const input of [options.install, options.source, options.output]) {
		if (isWithin(input, workspace) || isWithin(workspace, input)) throw new Error(`scratch workspace '${workspace}' overlaps '${input}'`);
	}
	await rm(workspace, { force: true, recursive: true });
	await copySourceTree(options.source, workspace);
	await linkPackageModules(options.install, options.source, workspace);
	const packageDirectory = join(workspace, options.packageRoot);
	if (options.mode === "typecheck") {
		const tool = await resolveTool(packageDirectory, "typescript", "tsc");
		runTool(tool, ["--project", options.config, "--noEmit", "--pretty", "false"], packageDirectory);
		await writeFile(options.output, "ok\n", { flag: "wx" });
		return;
	}
	const tool = await resolveTool(packageDirectory, "tsdown", "tsdown");
	runTool(tool, ["--config", options.config, "--out-dir", options.output], packageDirectory);
	const files = await readdir(options.output);
	if (files.length === 0) throw new Error(`tsdown produced empty output '${options.output}'`);
}

main(process.argv.slice(2)).catch((error) => {
	console.error(error instanceof Error ? error.message : error);
	process.exitCode = 1;
});
