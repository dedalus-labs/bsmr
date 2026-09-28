//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Executes pinned TypeScript tools in a source overlay over a frozen pnpm install.

import { spawnSync } from "node:child_process";
import { readdir, readFile, realpath, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";

import { isWithin, prepareWorkspace, requireRelativePath } from "../toolchains/pnpm/workspace.ts";

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
	const workspace = await prepareWorkspace(options.install, options.source, options.output);
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
