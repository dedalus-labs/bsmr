//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Runs differential Python installation, import, failure, and test conformance.

import { createHash, randomUUID } from "node:crypto";
import { cpSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { cpus, release, tmpdir, totalmem } from "node:os";
import { basename, delimiter, dirname, join, relative, resolve, sep } from "node:path";
import { performance } from "node:perf_hooks";
import { spawnSync } from "node:child_process";

import { compareSnapshots, snapshotEnvironment } from "./snapshot.ts";

interface CommandResult {
	elapsedMs: number;
	status: number;
	stderr: string;
	stdout: string;
}

interface ImportObservation {
	error?: string;
	name: string;
	ok: boolean;
	type?: string;
}

interface DarwinBuildTarget {
	hostPlatform: string;
	machine: string;
}

const pythonTarget = "root//:__bsmr_python_distribution";
const uvTarget = "root//:__bsmr_uv_distribution";
const missingImport = "__bsmr_conformance_missing__";
const macosDeploymentTarget = "13.0";
const darwinBuildTargets: Readonly<Record<string, DarwinBuildTarget>> = {
	"aarch64-apple-darwin": { hostPlatform: "macosx-13.0-arm64", machine: "arm64" },
	"x86_64-apple-darwin": { hostPlatform: "macosx-13.0-x86_64", machine: "x86_64" },
};
const generatedSourceComponents = new Set([
	".bsmr",
	".mypy_cache",
	".pytest_cache",
	".ruff_cache",
	".venv",
	"__pycache__",
	"bsmr-out",
	"build",
	"dist",
	"node_modules",
	"target",
	"target-bsmr",
]);

/** Requires one non-empty environment setting. */
const setting = (name: string): string => {
	const value = process.env[name];
	if (!value) throw new Error(`${name} is required`);
	return value;
};

/** Executes one argv-only command and preserves its complete observation. */
const execute = (executable: string, args: readonly string[], cwd: string, environment = process.env): CommandResult => {
	const start = performance.now();
	const result = spawnSync(executable, args, { cwd, encoding: "utf8", env: environment, maxBuffer: 256 * 1024 * 1024 });
	if (result.error) throw result.error;
	return { elapsedMs: performance.now() - start, status: result.status ?? -1, stderr: result.stderr, stdout: result.stdout };
};

/** Builds exact targets under either the repository cache or an explicit isolation. */
export const bsmrBuildArguments = (targets: readonly string[], isolation: string | undefined): string[] => [
	...(isolation ? ["--isolation-dir", isolation] : []),
	"build",
	...targets,
	"--show-full-json-output",
	"--console",
	"none",
];

/** Rejects a cold-cache claim unless its isolated output tree does not exist. */
export const assertBsmrCacheState = (repository: string, isolation: string | undefined, cacheState: string): void => {
	if (cacheState !== "empty-isolation") return;
	if (!isolation) throw new Error("empty-isolation requires BSMR_BENCH_ISOLATION_DIR");
	const output = join(repository, "bsmr-out", isolation);
	if (existsSync(output)) throw new Error(`empty BSMR isolation already exists: '${output}'`);
};

/** Parses typed absolute output paths for every requested BSMR target. */
export const parseBuildOutputs = (stdout: string, targets: readonly string[]): Record<string, string> => {
	const value: unknown = JSON.parse(stdout.trim());
	if (typeof value !== "object" || value === null || Array.isArray(value)) throw new Error("BSMR build output must be a JSON object");
	const outputs = value as Record<string, unknown>;
	for (const target of targets) {
		if (typeof outputs[target] !== "string" || outputs[target] === "") throw new Error(`missing output for ${target}`);
	}
	return Object.fromEntries(targets.map((target) => [target, outputs[target] as string]));
};

/** Maps the environment rule's default manifest to its materialized import root. */
export const environmentRoot = (manifest: string): string => {
	const suffix = ".manifest.json";
	if (!manifest.endsWith(suffix)) throw new Error(`expected an environment manifest, received '${manifest}'`);
	return join(dirname(manifest), `${basename(manifest, suffix)}.overlay`);
};

/** Returns BSMR's finite Python platform spelling for the current host. */
const pythonPlatform = (): string => {
	const platform = `${process.platform}-${process.arch}`;
	const values: Record<string, string> = {
		"darwin-arm64": "aarch64-apple-darwin",
		"darwin-x64": "x86_64-apple-darwin",
		"linux-arm64": "aarch64-unknown-linux-gnu",
		"linux-x64": "x86_64-unknown-linux-gnu",
	};
	const value = values[platform];
	if (!value) throw new Error(`unsupported Python conformance platform '${platform}'`);
	return value;
};

/** Returns BSMR's canonical Darwin wheel baseline for one uv target triple. */
export const darwinBuildTarget = (platform: string): DarwinBuildTarget | undefined => darwinBuildTargets[platform];

/** Returns the immutable content identity of one tool or lock input. */
export const sha256File = (path: string): string => createHash("sha256").update(readFileSync(path)).digest("hex");

/** Copies declared first-party roots without mutable build or installer state. */
export const copyProjectSources = (repository: string, destination: string, roots: readonly string[]): readonly string[] => {
	const normalized = roots.map((root) => {
		const source = resolve(repository, root);
		const path = relative(repository, source);
		if (path === "") return ".";
		if (path === ".." || path.startsWith(`..${sep}`)) throw new Error(`project source root escapes the repository: '${root}'`);
		return path;
	});
	const unique = [...new Set(normalized)];
	const copyRoots = unique.filter((root) => !unique.some((ancestor) => ancestor !== root && (ancestor === "." || root.startsWith(`${ancestor}${sep}`))));
	for (const root of copyRoots) {
		const source = root === "." ? repository : join(repository, root);
		const output = root === "." ? destination : join(destination, root);
		mkdirSync(dirname(output), { recursive: true });
		cpSync(source, output, {
			filter: (path) => {
				const components = relative(source, path).split(sep).filter(Boolean);
				if (components[0] === ".git") return true;
				return !components.some((component) => generatedSourceComponents.has(component)
					|| component.endsWith(".egg-info")
					|| component.endsWith(".pyc")
					|| component === "uv.lock"
					|| (component.startsWith("pylock.") && component.endsWith(".toml")));
			},
			recursive: true,
		});
	}
	return normalized.map((root) => root === "." ? destination : join(destination, root));
};

/** Fails one observed command with its exact stdout and stderr. */
const requireSuccess = (name: string, result: CommandResult): void => {
	if (result.status !== 0) throw new Error(`${name} failed with status ${result.status}\n${result.stderr}${result.stdout}`);
};

/** Reads one pinned tool's self-reported version without ambient configuration. */
const toolVersion = (executable: string, cwd: string): string => {
	const result = execute(executable, ["--version"], cwd, { LANG: "C.UTF-8", PATH: "/bin:/usr/bin" });
	requireSuccess(`${basename(executable)} --version`, result);
	const value = `${result.stdout}${result.stderr}`.trim();
	if (!value) throw new Error(`${basename(executable)} --version returned no identity`);
	return value;
};

/** Runs imports under one exact interpreter and search-root sequence. */
const probeImports = (python: string, roots: readonly string[], imports: readonly string[], cwd: string): readonly ImportObservation[] => {
	const probe = join(import.meta.dirname, "probe.py");
	const args = [probe, ...roots.flatMap((root) => ["--root", root]), ...[...imports, missingImport].flatMap((name) => ["--import", name])];
	const result = execute(python, args, cwd, {
		LANG: "C.UTF-8",
		PATH: "/bin:/usr/bin",
		PYTHONDONTWRITEBYTECODE: "1",
		PYTHONNOUSERSITE: "1",
	});
	requireSuccess("Python import probe", result);
	return JSON.parse(result.stdout) as ImportObservation[];
};

/** Requires requested imports to succeed and the canonical missing import to fail identically. */
const compareImports = (uv: readonly ImportObservation[], bsmr: readonly ImportObservation[]): void => {
	if (JSON.stringify(uv) !== JSON.stringify(bsmr)) throw new Error(`import behavior differs\nuv=${JSON.stringify(uv)}\nbsmr=${JSON.stringify(bsmr)}`);
	const unexpected = uv.filter((result) => result.name === missingImport ? result.ok || result.type !== "ModuleNotFoundError" : !result.ok);
	if (unexpected.length > 0) throw new Error(`unexpected import behavior: ${JSON.stringify(unexpected)}`);
};

/** Parses one optional JSON argv setting. */
const commandSetting = (name: string): readonly string[] | undefined => {
	const source = process.env[name];
	if (!source) return undefined;
	const value: unknown = JSON.parse(source);
	if (!Array.isArray(value) || !value.every((item) => typeof item === "string")) throw new Error(`${name} must be a JSON string array`);
	return value;
};

/** Writes a deterministic JSON observation beneath the immutable run directory. */
const writeJson = (runRoot: string, name: string, value: unknown): void => {
	writeFileSync(join(runRoot, name), `${JSON.stringify(value, null, 2)}\n`);
};

/** Executes one configured repository's complete differential conformance gate. */
const main = (): void => {
	const executable = resolve(setting("BSMR_BENCH_BINARY"));
	const repository = resolve(setting("BSMR_BENCH_REPOSITORY"));
	if (!existsSync(executable)) throw new Error(`BSMR_BENCH_BINARY does not exist: ${executable}`);
	if (!existsSync(repository)) throw new Error(`BSMR_BENCH_REPOSITORY does not exist: ${repository}`);
	const lock = process.env["BSMR_BENCH_PYTHON_LOCK"] ?? "pylock.toml";
	const buildLock = process.env["BSMR_BENCH_PYTHON_BUILD_LOCK"] ?? "pylock.build.toml";
	const environmentTarget = process.env["BSMR_BENCH_PYTHON_ENVIRONMENT"] ?? "root//:__bsmr_python_environment";
	const buildEnvironmentTarget = process.env["BSMR_BENCH_PYTHON_BUILD_ENVIRONMENT"] ?? "root//:__bsmr_python_build_environment";
	const projectEnvironmentTarget = process.env["BSMR_BENCH_PYTHON_PROJECT_ENVIRONMENT"];
	const isolation = process.env["BSMR_BENCH_ISOLATION_DIR"];
	const bsmrCacheState = process.env["BSMR_BENCH_CACHE_STATE"] ?? "repository-local-state-preserved";
	assertBsmrCacheState(repository, isolation, bsmrCacheState);
	const imports = (process.env["BSMR_BENCH_PYTHON_IMPORTS"] ?? "").split(",").filter(Boolean);
	const runRoot = join(process.env["BSMR_BENCH_ROOT"] ?? join(tmpdir(), "bsmr-benchmarks"), `python-conformance-${Date.now()}-${randomUUID()}`);
	const uvRoot = join(runRoot, "uv");
	const uvBuildRoot = join(runRoot, "uv-build");
	const uvProjectRoot = join(runRoot, "uv-projects");
	const uvSourceRoot = join(runRoot, "uv-sources");
	const mutable = join(runRoot, "state");
	const platform = pythonPlatform();
	mkdirSync(uvRoot, { recursive: true });
	mkdirSync(mutable);

	const targets = [pythonTarget, uvTarget, buildEnvironmentTarget, environmentTarget, ...(projectEnvironmentTarget ? [projectEnvironmentTarget] : [])];
	const buildArguments = bsmrBuildArguments(targets, isolation);
	const build = execute(executable, buildArguments, repository);
	requireSuccess("BSMR environment build", build);
	const outputs = parseBuildOutputs(build.stdout, targets);
	const warm = execute(executable, buildArguments, repository);
	requireSuccess("BSMR warm environment build", warm);
	parseBuildOutputs(warm.stdout, targets);
	const noOp = execute(executable, buildArguments, repository);
	requireSuccess("BSMR no-op environment build", noOp);
	parseBuildOutputs(noOp.stdout, targets);
	const python = outputs[pythonTarget]!;
	const uv = outputs[uvTarget]!;
	const bsmrRoot = environmentRoot(outputs[environmentTarget]!);

	const isolated: NodeJS.ProcessEnv = {
		AR: "ar",
		CFLAGS: "-g0",
		CXXFLAGS: "-g0",
		HOME: join(mutable, "home"),
		LANG: "C.UTF-8",
		NO_COLOR: "1",
		PATH: "/bin:/usr/bin",
		SOURCE_DATE_EPOCH: "315532800",
		UV_CACHE_DIR: join(mutable, "uv-cache"),
		UV_PYTHON_DOWNLOADS: "never",
		XDG_CACHE_HOME: join(mutable, "xdg-cache"),
		XDG_CONFIG_HOME: join(mutable, "xdg-config"),
	};
	for (const path of [isolated["HOME"], isolated["UV_CACHE_DIR"], isolated["XDG_CACHE_HOME"], isolated["XDG_CONFIG_HOME"]]) mkdirSync(path!);
	const buildSync = execute(uv, ["pip", "sync", buildLock, "--target", uvBuildRoot, "--python", python, "--python-platform", platform, "--no-python-downloads", "--strict", "--preview-features", "pylock", "--color", "never", "--no-progress"], repository, { ...isolated, UV_NO_CONFIG: "1" });
	requireSuccess("uv build-environment sync", buildSync);
	const uvBuildSnapshot = snapshotEnvironment(uvBuildRoot);
	const bsmrBuildSnapshot = snapshotEnvironment(environmentRoot(outputs[buildEnvironmentTarget]!));
	writeJson(runRoot, "uv-build-snapshot.json", uvBuildSnapshot);
	writeJson(runRoot, "bsmr-build-snapshot.json", bsmrBuildSnapshot);
	const buildDifferences = compareSnapshots(uvBuildSnapshot, bsmrBuildSnapshot);
	if (buildDifferences.length > 0) throw new Error(`build environments differ:\n${buildDifferences.join("\n")}`);
	const darwinTarget = darwinBuildTarget(platform);
	if (darwinTarget) {
		const shim = join(mutable, "target-platform");
		mkdirSync(shim);
		writeFileSync(join(shim, "sitecustomize.py"), `import platform\n\n\ndef _bsmr_mac_ver():\n    return ('${macosDeploymentTarget}.0', ('', '', ''), '${darwinTarget.machine}')\n\n\nplatform.mac_ver = _bsmr_mac_ver\n`);
		isolated["MACOSX_DEPLOYMENT_TARGET"] = macosDeploymentTarget;
		isolated["_PYTHON_HOST_PLATFORM"] = darwinTarget.hostPlatform;
		isolated["PYTHONPATH"] = shim;
	}
	isolated["PATH"] = [join(uvBuildRoot, "bin"), isolated["PATH"]!].join(delimiter);
	isolated["PYTHONPATH"] = [isolated["PYTHONPATH"], uvBuildRoot].filter(Boolean).join(delimiter);
	const sync = execute(uv, ["pip", "sync", lock, "--target", uvRoot, "--python", python, "--python-platform", platform, "--no-python-downloads", "--no-build-isolation", "--strict", "--preview-features", "pylock", "--color", "never", "--no-progress"], repository, { ...isolated, UV_NO_CONFIG: "1" });
	requireSuccess("uv baseline sync", sync);

	const uvSnapshot = snapshotEnvironment(uvRoot);
	const bsmrSnapshot = snapshotEnvironment(bsmrRoot);
	writeJson(runRoot, "uv-snapshot.json", uvSnapshot);
	writeJson(runRoot, "bsmr-snapshot.json", bsmrSnapshot);
	const differences = compareSnapshots(uvSnapshot, bsmrSnapshot);
	if (differences.length > 0) throw new Error(`installed artifacts differ:\n${differences.join("\n")}`);
	const sourceRoots = (process.env["BSMR_BENCH_PYTHON_SOURCE_ROOTS"] ?? ".")
		.split(",")
		.map((path) => resolve(repository, path));
	const uvImportRoots = [uvRoot];
	const bsmrImportRoots = [bsmrRoot];
	let uvProjectBuildMs: number | undefined;
	if (projectEnvironmentTarget) {
		mkdirSync(uvProjectRoot);
		const copiedSourceRoots = copyProjectSources(repository, uvSourceRoot, sourceRoots);
		const install = execute(uv, ["pip", "install", ...copiedSourceRoots, "--target", uvProjectRoot, "--python", python, "--python-platform", platform, "--no-python-downloads", "--no-build-isolation", "--no-deps", "--no-sources", "--color", "never", "--no-progress"], uvSourceRoot, isolated);
		requireSuccess("uv project build", install);
		uvProjectBuildMs = install.elapsedMs;
		const bsmrProjectRoot = outputs[projectEnvironmentTarget]!;
		const uvProjectSnapshot = snapshotEnvironment(uvProjectRoot);
		const bsmrProjectSnapshot = snapshotEnvironment(bsmrProjectRoot);
		writeJson(runRoot, "uv-project-snapshot.json", uvProjectSnapshot);
		writeJson(runRoot, "bsmr-project-snapshot.json", bsmrProjectSnapshot);
		const projectDifferences = compareSnapshots(uvProjectSnapshot, bsmrProjectSnapshot);
		if (projectDifferences.length > 0) throw new Error(`first-party artifacts differ:\n${projectDifferences.join("\n")}`);
		uvImportRoots.unshift(uvProjectRoot);
		bsmrImportRoots.unshift(bsmrProjectRoot);
	}
	compareImports(probeImports(python, uvImportRoots, imports, repository), probeImports(python, bsmrImportRoots, imports, repository));

	const testTarget = process.env["BSMR_BENCH_PYTHON_TEST_TARGET"];
	const testCommand = commandSetting("BSMR_BENCH_PYTHON_TEST_COMMAND");
	let tests: { bsmrMs: number; uvMs: number } | undefined;
	if ((testTarget === undefined) !== (testCommand === undefined)) throw new Error("test target and command must be configured together");
	if (testTarget && testCommand) {
		const bsmrTest = execute(executable, ["test", testTarget, "--console", "none"], repository);
		const uvTest = execute(python, testCommand, repository, { ...isolated, PATH: [join(uvRoot, "bin"), process.env["PATH"] ?? ""].join(delimiter), PYTHONNOUSERSITE: "1", PYTHONPATH: uvImportRoots.join(delimiter) });
		if (bsmrTest.status !== uvTest.status || uvTest.status !== 0) throw new Error(`test behavior differs: uv=${uvTest.status}, bsmr=${bsmrTest.status}`);
		tests = { bsmrMs: bsmrTest.elapsedMs, uvMs: uvTest.elapsedMs };
	}

	const processor = cpus()[0];
	if (!processor) throw new Error("the host exposes no logical processors");
	const report = {
		artifacts: {
			buildDistributions: uvBuildSnapshot.distributions.length,
			buildFiles: Object.keys(uvBuildSnapshot.files).length,
			distributions: uvSnapshot.distributions.length,
			files: Object.keys(uvSnapshot.files).length,
		},
		buildEnvironmentTarget,
		buildLock,
		cacheState: {
			bsmr: bsmrCacheState,
			uv: "empty-per-run",
		},
		correctness: "pass",
		environmentTarget,
		imports,
		lock,
		machine: {
			architecture: process.arch,
			logicalCpus: cpus().length,
			memoryBytes: totalmem(),
			operatingSystem: process.platform,
			processor: processor.model,
			release: release(),
		},
		platform,
		repository,
		tests,
		timingMs: {
			bsmrBuild: build.elapsedMs,
			bsmrNoOp: noOp.elapsedMs,
			bsmrWarm: warm.elapsedMs,
			uvBuildSync: buildSync.elapsedMs,
			uvProjectBuild: uvProjectBuildMs,
			uvSync: sync.elapsedMs,
		},
		tools: {
			bsmr: { sha256: sha256File(executable) },
			python: { sha256: sha256File(python), version: toolVersion(python, repository) },
			uv: { sha256: sha256File(uv), version: toolVersion(uv, repository) },
		},
	};
	writeJson(runRoot, "results.json", report);
	process.stdout.write(`${join(runRoot, "results.json")}\n`);
};

if (import.meta.main) main();
