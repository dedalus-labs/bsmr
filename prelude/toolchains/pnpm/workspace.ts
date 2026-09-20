//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Creates writable package workspaces over declared sources and frozen dependencies.

import { chmod, copyFile, lstat, mkdir, readdir, readFile, readlink, realpath, rm, symlink, writeFile } from "node:fs/promises";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";

/**
 * Require one normalized, non-traversing relative path.
 *
 * @param name - Diagnostic field name.
 * @param path - User-supplied relative path.
 * @returns A normalized relative path.
 */
export function requireRelativePath(name: string, path: string): string {
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
export async function exists(path: string): Promise<boolean> {
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
export function isWithin(directory: string, candidate: string): boolean {
	const suffix = relative(directory, candidate);
	return suffix === "" || (!suffix.startsWith(`..${sep}`) && suffix !== ".." && !isAbsolute(suffix));
}

/**
 * Copy declared source paths into the writable scratch workspace.
 *
 * @param source - Declared source tree.
 * @param destination - Scratch workspace directory.
 */
async function copySourceTree(source: string, destination: string, workspace = destination): Promise<void> {
	await mkdir(destination, { recursive: true });
	for (const entry of await readdir(source, { withFileTypes: true })) {
		const from = join(source, entry.name);
		const to = join(destination, entry.name);
		if (entry.isDirectory()) {
			await copySourceTree(from, to, workspace);
		} else if (entry.isFile()) {
			await copyFile(from, to);
		} else {
			await copySourceEntry(from, to, workspace);
		}
	}
}

/**
 * Copy one declared file or preserve one repository-relative source symlink.
 *
 * @param source - Source-tree entry created by BSMR.
 * @param destination - Writable overlay entry.
 * @param workspace - Writable overlay root that symlinks may not escape.
 */
async function copySourceEntry(source: string, destination: string, workspace: string): Promise<void> {
	const sourceTarget = resolve(dirname(source), await readlink(source));
	const declared = await lstat(sourceTarget);
	if (declared.isFile()) {
		await copyFile(source, destination);
		return;
	}
	if (declared.isDirectory()) {
		await copySourceTree(sourceTarget, destination, workspace);
		return;
	}
	if (!declared.isSymbolicLink()) throw new Error(`declared source '${source}' is not a file, directory, or symlink`);
	const target = await readlink(sourceTarget);
	const overlayTarget = resolve(dirname(destination), target);
	if (isAbsolute(target) || !isWithin(workspace, overlayTarget)) {
		throw new Error(`declared symlink '${source}' escapes the source workspace`);
	}
	await symlink(target, destination);
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

/** Apply pnpm's executable mode and shebang normalization only to an owned source copy. */
async function prepareExecutable(path: string, packages: ReadonlyMap<string, string>): Promise<void> {
	if (!(await exists(path))) return; // A preceding package build may produce this bin later.
	const target = await realpath(path);
	if (![...packages.values()].some((root) => isWithin(root, target))) throw new Error(`workspace executable '${path}' escapes declared sources`);
	await chmod(target, 0o755);
	const bytes = await readFile(target);
	const newline = bytes.indexOf(10);
	if (bytes[0] === 35 && bytes[1] === 33 && newline > 0 && bytes[newline - 1] === 13) {
		await writeFile(target, Buffer.concat([bytes.subarray(0, newline - 1), bytes.subarray(newline)]));
	}
}

/** Preserve pnpm's selected bins, including workspace targets absent from the manifest-only install. */
async function mirrorExecutables(input: string, output: string, packages: ReadonlyMap<string, string>): Promise<void> {
	await mkdir(output, { recursive: true });
	const directory = await realpath(input);
	for (const name of await readdir(input)) {
		const link = await readlink(join(input, name));
		let target = resolve(directory, link);
		for (const [installed, workspace] of packages) {
			const suffix = relative(installed, target);
			if (isWithin(installed, target) && !suffix.split(sep).includes("node_modules")) {
				target = join(workspace, suffix);
				await prepareExecutable(target, packages);
				break;
			}
		}
		await symlink(target, join(output, name));
	}
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
		if (entry.name === ".bin") {
			await mirrorExecutables(from, to, packages);
			continue;
		}
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
		const manifest = join(root, "package.json");
		if (await readFile(join(source, manifest), "utf8") !== await readFile(join(install, manifest), "utf8")) {
			throw new Error(`frozen install manifest differs from declared source '${manifest}'`);
		}
		packages.set(await realpath(installedPackage), await realpath(join(workspace, root)));
	}
	for (const root of roots) {
		const input = join(install, root, "node_modules");
		if (await exists(input)) await mirrorModules(input, join(workspace, root, "node_modules"), packages);
	}
}

/** Create an owned source overlay without writing to source or dependency artifacts. */
export async function prepareWorkspace(install: string, source: string, output: string): Promise<string> {
	if (await exists(output)) throw new Error(`action output '${output}' already exists`);
	const scratchRoot = process.env["BSMR_SCRATCH_PATH"];
	if (scratchRoot === undefined || scratchRoot === "") throw new Error("BSMR did not provide BSMR_SCRATCH_PATH");
	const workspace = resolve(scratchRoot, "package-workspace");
	for (const input of [install, source, output]) {
		if (isWithin(input, workspace) || isWithin(workspace, input)) throw new Error(`scratch workspace '${workspace}' overlaps '${input}'`);
	}
	await rm(workspace, { force: true, recursive: true });
	await copySourceTree(source, workspace);
	await linkPackageModules(install, source, workspace);
	return workspace;
}
