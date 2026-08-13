//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Executes pinned TypeScript tools in a source overlay over a frozen pnpm install.

import { spawnSync } from "node:child_process";
import { lstat, mkdir, readdir, readFile, realpath, rm, symlink, writeFile } from "node:fs/promises";
import { isAbsolute, join, relative, resolve, sep } from "node:path";

const requiredArguments = new Set(["--config", "--install", "--mode", "--output", "--package-root", "--source"]);

/**
 * Parse the runner's closed command-line schema.
 *
 * @param {readonly string[]} arguments_ - Command-line words after the entrypoint.
 * @returns {{ config: string, install: string, mode: "library" | "typecheck", output: string, packageRoot: string, source: string }}
 */
function parseArguments(arguments_) {
	const values = new Map();
	for (let index = 0; index < arguments_.length; index += 2) {
		const name = arguments_[index];
		const value = arguments_[index + 1];
		if (!requiredArguments.has(name)) throw new Error(`unknown argument '${name ?? ""}'`);
		if (value === undefined || value === "") throw new Error(`argument '${name}' requires a value`);
		if (values.has(name)) throw new Error(`argument '${name}' was provided more than once`);
		values.set(name, value);
	}
	for (const name of requiredArguments) {
		if (!values.has(name)) throw new Error(`missing required argument '${name}'`);
	}
	const mode = values.get("--mode");
	if (mode !== "library" && mode !== "typecheck") throw new Error(`unsupported TypeScript action mode '${mode}'`);
	return {
		config: requireRelativePath("config", values.get("--config")),
		install: resolve(values.get("--install")),
		mode,
		output: resolve(values.get("--output")),
		packageRoot: values.get("--package-root") === "." ? "" : requireRelativePath("package root", values.get("--package-root")),
		source: resolve(values.get("--source")),
	};
}

/**
 * Require one normalized, non-traversing relative path.
 *
 * @param {string} name - Diagnostic field name.
 * @param {string} path - User-supplied relative path.
 * @returns {string}
 */
function requireRelativePath(name, path) {
	const components = path.replaceAll("\\", "/").split("/");
	if (isAbsolute(path) || path === "." || components.includes("") || components.includes(".") || components.includes("..")) {
		throw new Error(`${name} '${path}' must be a normalized relative path`);
	}
	return components.join("/");
}

/**
 * Test whether a path exists without swallowing other filesystem failures.
 *
 * @param {string} path - Absolute path to inspect.
 * @returns {Promise<boolean>}
 */
async function exists(path) {
	try {
		await lstat(path);
		return true;
	} catch (error) {
		if (error.code === "ENOENT") return false;
		throw error;
	}
}

/**
 * Test whether a candidate is nested beneath a directory.
 *
 * @param {string} directory - Absolute parent directory.
 * @param {string} candidate - Absolute candidate path.
 * @returns {boolean}
 */
function isWithin(directory, candidate) {
	const suffix = relative(directory, candidate);
	return suffix === "" || (!suffix.startsWith(`..${sep}`) && suffix !== ".." && !isAbsolute(suffix));
}

/**
 * Recreate declared source paths as read-only links in the scratch workspace.
 *
 * @param {string} source - Declared source tree.
 * @param {string} destination - Scratch workspace directory.
 * @returns {Promise<void>}
 */
async function linkSourceTree(source, destination) {
	await mkdir(destination, { recursive: true });
	for (const entry of await readdir(source, { withFileTypes: true })) {
		const from = join(source, entry.name);
		const to = join(destination, entry.name);
		if (entry.isDirectory()) {
			await linkSourceTree(from, to);
		} else {
			await symlink(await realpath(from), to);
		}
	}
}

/**
 * Find workspace roots represented by declared package manifests.
 *
 * @param {string} source - Current source directory.
 * @param {string} prefix - Source-relative directory being visited.
 * @returns {Promise<string[]>}
 */
async function findPackageRoots(source, prefix = "") {
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
 * @param {string} input - Installed node_modules or scope directory.
 * @param {string} output - Scratch node_modules or scope directory.
 * @param {ReadonlyMap<string, string>} packages - Installed realpath to scratch package root.
 * @returns {Promise<void>}
 */
async function mirrorModules(input, output, packages) {
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
 * @param {string} install - Frozen pnpm installation workspace.
 * @param {string} source - Declared source tree.
 * @param {string} workspace - Scratch workspace.
 * @returns {Promise<void>}
 */
async function linkPackageModules(install, source, workspace) {
	const roots = await findPackageRoots(source);
	const packages = new Map();
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
 * @param {string} packageDirectory - Scratch package directory.
 * @param {string} tool - Direct tool dependency name.
 * @param {string} command - Expected package bin key.
 * @returns {Promise<string>}
 */
async function resolveTool(packageDirectory, tool, command) {
	const manifestPath = join(packageDirectory, "node_modules", ...tool.split("/"), "package.json");
	let manifest;
	try {
		manifest = JSON.parse(await readFile(manifestPath, "utf8"));
	} catch (error) {
		throw new Error(`package-local tool '${tool}' is unavailable: ${error.message}`);
	}
	const executable = typeof manifest.bin === "string" ? manifest.bin : manifest.bin?.[command];
	if (typeof executable !== "string") throw new Error(`package-local tool '${tool}' does not declare bin '${command}'`);
	const packageRoot = await realpath(join(packageDirectory, "node_modules", ...tool.split("/")));
	const bin = resolve(packageRoot, executable);
	if (!isWithin(packageRoot, bin)) throw new Error(`package-local tool '${tool}' declares escaping bin '${executable}'`);
	return bin;
}

/**
 * Run a pinned Node tool and preserve its diagnostics.
 *
 * @param {string} executable - Absolute JavaScript entrypoint.
 * @param {readonly string[]} arguments_ - Tool arguments.
 * @param {string} cwd - Scratch package directory.
 * @returns {void}
 */
function runTool(executable, arguments_, cwd) {
	const result = spawnSync(process.execPath, [executable, ...arguments_], {
		cwd,
		env: { CI: "1", FORCE_COLOR: "0", LANG: "C", LC_ALL: "C", NO_COLOR: "1", SOURCE_DATE_EPOCH: "0", TZ: "UTC" },
		stdio: "inherit",
	});
	if (result.error !== undefined) throw result.error;
	if (result.status !== 0) throw new Error(`TypeScript tool exited with status ${result.status ?? "unknown"}`);
}

/**
 * Execute one hermetic TypeScript action.
 *
 * @param {readonly string[]} arguments_ - Command-line words after the entrypoint.
 * @returns {Promise<void>}
 */
async function main(arguments_) {
	const options = parseArguments(arguments_);
	if (await exists(options.output)) throw new Error(`action output '${options.output}' already exists`);
	const scratchRoot = process.env.BUCK_SCRATCH_PATH;
	if (scratchRoot === undefined || scratchRoot === "") throw new Error("BSMR did not provide BUCK_SCRATCH_PATH");
	const workspace = resolve(scratchRoot, "typescript-workspace");
	for (const input of [options.install, options.source, options.output]) {
		if (isWithin(input, workspace) || isWithin(workspace, input)) throw new Error(`scratch workspace '${workspace}' overlaps '${input}'`);
	}
	await rm(workspace, { force: true, recursive: true });
	await linkSourceTree(options.source, workspace);
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
