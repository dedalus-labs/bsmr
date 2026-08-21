//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Runs one frozen pnpm install with an exact Node runtime and action-local state.

import { spawnSync } from "node:child_process";
import { access, cp, lstat, mkdir, readdir, readFile, readlink, rm, writeFile } from "node:fs/promises";
import { dirname, isAbsolute, join, resolve, sep } from "node:path";

const requiredArguments = new Set([
	"--node-requirement",
	"--node-version",
	"--output",
	"--package-manager",
	"--pnpm-cli",
	"--source",
]);

type JsonObject = Record<string, unknown>;
type RunnerOptions = Readonly<{
	nodeRequirement: string;
	nodeVersion: string;
	output: string;
	packageManager: string;
	pnpmCli: string;
	source: string;
}>;
type StatePaths = Readonly<{
	corepackHome: string;
	home: string;
	pnpmHome: string;
	store: string;
	userconfig: string;
	xdgCacheHome: string;
	xdgConfigHome: string;
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
 * @throws {Error} When an argument is unknown, duplicated, or missing a value.
 */
function parseArguments(arguments_: readonly string[]): RunnerOptions {
	const values = new Map<string, string>();
	for (let index = 0; index < arguments_.length; index += 2) {
		const name = arguments_[index];
		const value = arguments_[index + 1];
		if (name === undefined || !requiredArguments.has(name)) throw new Error(`unknown argument '${name ?? ""}'`);
		if (value === undefined) throw new Error(`argument '${name}' requires a value`);
		if (values.has(name)) throw new Error(`argument '${name}' was provided more than once`);
		values.set(name, value);
	}
	const nodeRequirement = requiredArgument(values, "--node-requirement");
	const nodeVersion = requiredArgument(values, "--node-version");
	const output = requiredArgument(values, "--output");
	const packageManager = requiredArgument(values, "--package-manager");
	const pnpmCli = requiredArgument(values, "--pnpm-cli");
	const source = requiredArgument(values, "--source");
	return {
		nodeRequirement,
		nodeVersion,
		output,
		packageManager,
		pnpmCli,
		source,
	};
}

/** Return whether an unknown failure carries the requested system error code. */
function hasErrorCode(error: unknown, code: string): error is NodeJS.ErrnoException {
	return error instanceof Error && "code" in error && error.code === code;
}

/**
 * Read and parse the project's package manifest.
 *
 * @param source - Declared project input directory.
 * @returns The parsed JSON object.
 * @throws {Error} When package.json is absent, invalid, or not a JSON object.
 */
async function readManifest(source: string): Promise<JsonObject> {
	const path = join(source, "package.json");
	let manifest: unknown;
	try {
		manifest = JSON.parse(await readFile(path, "utf8"));
	} catch (error) {
		throw new Error(`cannot read a valid package.json at '${path}': ${error instanceof Error ? error.message : String(error)}`);
	}
	if (manifest === null || typeof manifest !== "object" || Array.isArray(manifest)) {
		throw new Error(`package.json at '${path}' must contain a JSON object`);
	}
	return manifest as JsonObject;
}

/**
 * Assert that the executing runtime and manifest match the configured toolchain.
 *
 * @param manifest - Parsed package.json object.
 * @param nodeVersion - Exact configured Node version.
 * @param nodeRequirement - Root manifest requirement validated by the native frontend.
 * @param packageManager - Exact configured Corepack-style pnpm pin.
 * @returns Exact pnpm version encoded in the package-manager pin.
 * @throws {Error} When any version or integrity invariant is violated.
 */
function validateToolchain(
	manifest: JsonObject,
	nodeVersion: string,
	nodeRequirement: string,
	packageManager: string,
): string {
	if (!/^\d+\.\d+\.\d+$/.test(nodeVersion)) {
		throw new Error(`Node version '${nodeVersion}' is not an exact semantic version`);
	}
	if (process.versions.node !== nodeVersion) {
		throw new Error(`Node runtime ${process.versions.node} does not match configured version ${nodeVersion}`);
	}
	const packageManagerMatch = /^pnpm@(\d+)\.(\d+)\.(\d+)\+sha512\.[0-9a-f]{128}$/.exec(packageManager);
	if (packageManagerMatch === null) {
		throw new Error(`package manager '${packageManager}' is not an exact pnpm version with a sha512 digest`);
	}
	const major = Number(packageManagerMatch[1]);
	if (major !== 10 && major !== 11) {
		throw new Error("pnpm toolchains support exact pnpm 10 and 11 versions");
	}
	const engines = manifest["engines"];
	const manifestNode = engines !== null && typeof engines === "object" && !Array.isArray(engines)
		? (engines as JsonObject)["node"]
		: undefined;
	if (manifestNode !== nodeRequirement) {
		throw new Error(
			`package.json engines.node '${manifestNode ?? ""}' does not match configured requirement '${nodeRequirement}'`,
		);
	}
	if (manifest["packageManager"] !== packageManager) {
		throw new Error(
			`package.json packageManager '${manifest["packageManager"] ?? ""}' does not match configured pin '${packageManager}'`,
		);
	}
	return `${packageManagerMatch[1]}.${packageManagerMatch[2]}.${packageManagerMatch[3]}`;
}

/**
 * Select the supported invocation contract for an exact pnpm version.
 *
 * @param pnpmVersion - Exact validated pnpm version.
 * @param arguments_ - pnpm command arguments.
 * @returns Arguments for the selected pnpm contract.
 * @throws {Error} When the validated-version invariant is violated.
 */
function pnpmArguments(pnpmVersion: string, arguments_: readonly string[]): string[] {
	switch (Number(pnpmVersion.split(".")[0])) {
		case 10:
			return [...arguments_];
		case 11:
			return ["with", "current", ...arguments_];
		default:
			throw new Error(`unsupported validated pnpm version '${pnpmVersion}'`);
	}
}

/**
 * Assert that the build-system-owned output path is absent.
 *
 * @param output - Declared action output directory.
 * @throws {Error} When the path exists or cannot be inspected.
 */
async function requireAbsent(output: string): Promise<void> {
	try {
		await lstat(output);
	} catch (error) {
		if (hasErrorCode(error, "ENOENT")) return;
		throw error;
	}
	throw new Error(`action output '${output}' already exists`);
}

/**
 * Resolve package-manager state beneath BSMR's action scratch directory.
 *
 * @param output - Declared action output directory.
 * @returns Absolute mutable-state directory outside the cached output.
 * @throws {Error} When BSMR did not provide isolated scratch space or placed it inside the output.
 */
function packageManagerState(output: string): string {
	if (process.platform === "win32") {
		throw new Error("pnpm install adapter requires relocatable executable symlinks and does not support Windows");
	}
	const scratch = process.env["BSMR_SCRATCH_PATH"];
	if (scratch === undefined || scratch === "") {
		throw new Error("BSMR did not provide BSMR_SCRATCH_PATH for pnpm mutable state");
	}
	const state = resolve(scratch, "pnpm");
	if (state === output || state.startsWith(`${output}${sep}`)) {
		throw new Error(`pnpm mutable state '${state}' must be outside cached output '${output}'`);
	}
	return state;
}

/**
 * Reject a project input that occupies BSMR's reserved workspace path.
 *
 * @param output - Writable copy of declared project inputs.
 * @throws {Error} When `.bsmr` exists or cannot be inspected.
 */
async function requireReservedPathAbsent(output: string): Promise<void> {
	try {
		await lstat(join(output, ".bsmr"));
	} catch (error) {
		if (hasErrorCode(error, "ENOENT")) return;
		throw error;
	}
	throw new Error("project inputs may not contain the reserved '.bsmr' path");
}

/**
 * Create the writable project copy and its private package-manager state.
 *
 * @param source - Declared project input directory.
 * @param output - Declared action output directory.
 * @param state - BSMR-owned scratch directory for mutable package-manager state.
 * @returns Isolated package-manager state paths.
 * @throws {Error} When inputs use BSMR's reserved state directory.
 */
async function prepareWorkspace(source: string, output: string, state: string): Promise<StatePaths> {
	await requireAbsent(output);
	await mkdir(dirname(output), { recursive: true });
	await cp(source, output, { dereference: true, errorOnExist: true, force: false, recursive: true });
	await requireReservedPathAbsent(output);
	await rm(state, { force: true, recursive: true });
	await mkdir(state, { recursive: true });
	const paths = {
		corepackHome: join(state, "corepack"),
		home: join(state, "home"),
		pnpmHome: join(state, "pnpm-home"),
		store: join(state, "pnpm-store"),
		userconfig: join(state, "empty-npmrc"),
		xdgCacheHome: join(state, "xdg-cache"),
		xdgConfigHome: join(state, "xdg-config"),
	};
	await Promise.all([
		mkdir(paths.corepackHome),
		mkdir(paths.home),
		mkdir(paths.pnpmHome),
		mkdir(paths.store),
		mkdir(paths.xdgCacheHome),
		mkdir(paths.xdgConfigHome),
		writeFile(paths.userconfig, "", { flag: "wx" }),
	]);
	return paths;
}

/**
 * Remove pnpm bookkeeping whose timestamps and absolute store paths poison CAS identity.
 *
 * @param output - Installed project output directory.
 * @throws {Error} When pnpm did not produce the closed install-layout contract.
 */
async function removeMutableInstallMetadata(output: string): Promise<void> {
	await Promise.all([
		rm(join(output, "node_modules", ".modules.yaml"), { force: true }),
		rm(join(output, "node_modules", ".pnpm-workspace-state-v1.json"), { force: true }),
	]);
}

/**
 * Require every installed executable link to remain valid after CAS relocation.
 *
 * @param output - Installed project output directory.
 * @throws {Error} When pnpm emits a command shim or absolute symlink.
 */
async function requireRelocatableExecutables(output: string): Promise<void> {
	const directory = join(output, "node_modules", ".bin");
	let names;
	try {
		names = await readdir(directory);
	} catch (error) {
		if (hasErrorCode(error, "ENOENT")) return;
		throw error;
	}
	for (const name of names) {
		const executable = join(directory, name);
		if (!(await lstat(executable)).isSymbolicLink()) {
			throw new Error(`pnpm executable '${executable}' must be a relative symlink`);
		}
		if (isAbsolute(await readlink(executable))) {
			throw new Error(`pnpm executable '${executable}' must not target an absolute path`);
		}
	}
}

/**
 * Invoke the exact pnpm CLI with frozen resolution and isolated mutable state.
 *
 * @param pnpmCli - Verified pnpm CLI artifact.
 * @param output - Writable project output directory.
 * @param pnpmVersion - Exact pnpm version required by the toolchain.
 * @param paths - Action-local state paths.
 * @throws {Error} When pnpm cannot start, is terminated, or exits unsuccessfully.
 */
function install(pnpmCli: string, output: string, pnpmVersion: string, paths: StatePaths): void {
	const environment: NodeJS.ProcessEnv = {
		COREPACK_HOME: paths.corepackHome,
		HOME: paths.home,
		// pnpm 10 and 11 use different settings to forbid toolchain replacement.
		NPM_CONFIG_MANAGE_PACKAGE_MANAGER_VERSIONS: "false",
		NPM_CONFIG_UPDATE_NOTIFIER: "false",
		NPM_CONFIG_USERCONFIG: paths.userconfig,
		PATH: dirname(process.execPath),
		PNPM_HOME: paths.pnpmHome,
		XDG_CACHE_HOME: paths.xdgCacheHome,
		XDG_CONFIG_HOME: paths.xdgConfigHome,
		npm_config_userconfig: paths.userconfig,
		pnpm_config_update_notifier: "false",
		pnpm_config_npmrc_auth_file: paths.userconfig,
		pnpm_config_pm_on_fail: "error",
	};
	const version = spawnSync(process.execPath, [pnpmCli, "--version"], {
		cwd: paths.home,
		encoding: "utf8",
		env: environment,
	});
	if (version.error !== undefined) throw new Error(`cannot execute pnpm: ${version.error.message}`);
	if (version.signal !== null) throw new Error(`pnpm version check was terminated by signal ${version.signal}`);
	if (version.status !== 0) throw new Error(`pnpm version check exited with status ${version.status}: ${version.stderr.trim()}`);
	if (version.stdout.trim() !== pnpmVersion) {
		throw new Error(`pnpm CLI ${version.stdout.trim()} does not match configured version ${pnpmVersion}`);
	}
	const result = spawnSync(
		process.execPath,
		[
			pnpmCli,
			...pnpmArguments(pnpmVersion, [
				"install",
				"--frozen-lockfile",
				"--ignore-scripts",
				"--store-dir",
				paths.store,
				"--config.prefer-symlinked-executables=true",
			]),
		],
		{
			cwd: output,
			env: environment,
			stdio: "inherit",
		},
	);
	if (result.error !== undefined) throw new Error(`cannot execute pnpm: ${result.error.message}`);
	if (result.signal !== null) throw new Error(`pnpm was terminated by signal ${result.signal}`);
	if (result.status !== 0) throw new Error(`pnpm exited with status ${result.status}`);
}

/**
 * Validate inputs and perform one frozen install.
 *
 * @param options - Parsed runner options.
 * @throws {Error} When a declared input or install invariant fails.
 */
async function run(options: RunnerOptions): Promise<void> {
	const source = resolve(options.source);
	const output = resolve(options.output);
	const pnpmCli = resolve(options.pnpmCli);
	const manifest = await readManifest(source);
	const pnpmVersion = validateToolchain(manifest, options.nodeVersion, options.nodeRequirement, options.packageManager);
	await access(join(source, "pnpm-lock.yaml"));
	await access(pnpmCli);
	const paths = await prepareWorkspace(source, output, packageManagerState(output));
	install(pnpmCli, output, pnpmVersion, paths);
	await requireRelocatableExecutables(output);
	await removeMutableInstallMetadata(output);
}

try {
	await run(parseArguments(process.argv.slice(2)));
} catch (error) {
	console.error(error instanceof Error ? error.message : String(error));
	process.exitCode = 1;
}
