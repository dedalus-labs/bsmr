//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Runs correctness-gated Django build benchmarks against tuned Bazel rules_python.

import { spawnSync } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { cpus, platform, release, tmpdir, totalmem } from "node:os";
import { join, relative, resolve, sep } from "node:path";
import { performance } from "node:perf_hooks";

import { bazelVersion, bazeliskSha256, djangoCommit, rulesPythonVersion } from "./config.ts";
import { changedWheelEntries, median, parseBsmrOutputs, performanceGateResults, positiveInteger, runnerOrder, targetEnvironment, targetSource, targetWheel, wheelPayload, type BsmrOutputs, type Runner, type WheelEntry } from "./helpers.ts";

interface CachePaths {
	action: string;
	disk: string;
	http: string;
	repository: string;
}

interface CommandResult {
	elapsedMs: number;
	status: number;
	stderr: string;
	stdout: string;
}

interface Instance {
	cache: CachePaths;
	cwd: string;
	environment: NodeJS.ProcessEnv;
	name: string;
	outputRoot: string;
	outputs?: BsmrOutputs;
	runner: Runner;
}

interface Observation {
	elapsedMs: number;
	iteration: number;
	regime: string;
	runner: Runner;
}

interface Correctness {
	payload: readonly WheelEntry[];
	version: string;
}

const generatedComponents = new Set([".pytest_cache", ".ruff_cache", ".venv", "__pycache__", "bazel-bin", "bazel-out", "bazel-testlogs", "buck-out"]);
const targetEntry = "root//:django-admin";
const targetTest = "root//:test";
const fixture = join(import.meta.dirname, "fixture");
const source = resolve(required("BSMR_BENCH_REPOSITORY"));
const bsmr = resolve(required("BSMR_BENCH_BINARY"));
const bazelisk = resolve(required("BSMR_BENCH_BAZELISK"));
const runs = positiveInteger("BSMR_BENCH_RUNS", 5, 5);
const coldRuns = positiveInteger("BSMR_BENCH_COLD_RUNS", 3, 1);
const runRoot = join(process.env["BSMR_BENCH_ROOT"] ?? join(tmpdir(), "bsmr-benchmarks"), `python-build-systems-${Date.now()}-${randomUUID()}`);
const logs = join(runRoot, "logs");
const observations: Observation[] = [];
mkdirSync(logs, { recursive: true });

/** Returns one required, nonempty environment setting. */
function required(name: string): string {
	const value = process.env[name];
	if (!value) throw new Error(`${name} is required`);
	return value;
}

/** Returns the SHA-256 identity of one file. */
function sha256(path: string): string {
	return createHash("sha256").update(readFileSync(path)).digest("hex");
}

/** Executes one command without a shell and preserves its complete observation. */
function execute(executable: string, args: readonly string[], cwd: string, environment: NodeJS.ProcessEnv): CommandResult {
	const start = performance.now();
	const result = spawnSync(executable, args, { cwd, encoding: "utf8", env: environment, maxBuffer: 256 * 1024 * 1024 });
	if (result.error) throw result.error;
	return { elapsedMs: performance.now() - start, status: result.status ?? -1, stderr: result.stderr, stdout: result.stdout };
}

/** Requires command success while retaining its full diagnostics. */
function requireSuccess(name: string, result: CommandResult): void {
	if (result.status !== 0) throw new Error(`${name} failed with status ${result.status}\n${result.stderr}${result.stdout}`);
}

/** Copies the prepared source without any output or interpreter cache state. */
function copySource(name: string): string {
	const destination = join(runRoot, "checkouts", name);
	cpSync(source, destination, {
		filter: (path) => {
			const components = relative(source, path).split(sep).filter(Boolean);
			return !components.some((component) => generatedComponents.has(component) || component.startsWith("bazel-") || component.endsWith(".pyc"));
		},
		preserveTimestamps: true,
		recursive: true,
		verbatimSymlinks: true,
	});
	return destination;
}

/** Creates every cache directory for one explicit cache-state contract. */
function cachePaths(name: string): CachePaths {
	const root = join(runRoot, "caches", name);
	const paths = { action: join(root, "action"), disk: join(root, "disk"), http: join(root, "http"), repository: join(root, "repository") };
	for (const path of Object.values(paths)) mkdirSync(path, { recursive: true });
	return paths;
}

/** Creates one runner checkout with no hidden cache-path defaults. */
function instance(runner: Runner, name: string, cache: CachePaths): Instance {
	const cwd = copySource(`${name}-${runner}`);
	const outputRoot = join(runRoot, "outputs", `${name}-${runner}`);
	mkdirSync(outputRoot, { recursive: true });
	const environment = {
		...process.env,
		BSMR_HTTP_CACHE_DIR: cache.http,
		BSMR_LOCAL_CACHE_DIR: cache.action,
		CI: "1",
		NO_COLOR: "1",
		USE_BAZEL_VERSION: bazelVersion,
	};
	return { cache, cwd, environment, name, outputRoot, runner };
}

/** Returns the exact optimized command for one runner. */
function command(value: Instance): readonly [string, readonly string[]] {
	if (value.runner === "bsmr") {
		return [bsmr, ["--isolation-dir", "benchmark", "build", targetWheel, targetEnvironment, targetEntry, targetSource, "--show-full-json-output", "--console", "none"]];
	}
	return [bazelisk, [
		`--output_user_root=${value.outputRoot}`,
		"build",
		"//:django-admin",
		"//:django-wheel",
		`--repository_cache=${value.cache.repository}`,
		`--disk_cache=${value.cache.disk}`,
		"--experimental_repository_cache_hardlinks",
		"--incompatible_default_to_explicit_init_py",
		"--spawn_strategy=local",
		"--watchfs",
		"--color=no",
		"--curses=no",
	]];
}

/** Executes and records one measured or setup build. */
function build(value: Instance, regime: string, iteration: number, measured: boolean): CommandResult {
	const [executable, args] = command(value);
	const result = execute(executable, args, value.cwd, value.environment);
	writeFileSync(join(logs, `${regime}-${iteration}-${value.runner}.log`), `${result.stderr}${result.stdout}`);
	requireSuccess(`${value.runner} ${regime}`, result);
	if (value.runner === "bsmr") value.outputs = parseBsmrOutputs(result.stdout);
	if (measured) observations.push({ elapsedMs: result.elapsedMs, iteration, regime, runner: value.runner });
	return result;
}

/** Executes and records one correctness-gated test invocation. */
function runTest(value: Instance, regime: string, iteration: number, measured: boolean): CommandResult {
	const invocation = value.runner === "bsmr"
		? [bsmr, ["--isolation-dir", "benchmark", "test", targetTest, "--console", "none"]] as const
		: [bazelisk, [
			`--output_user_root=${value.outputRoot}`,
			"test",
			"//:django-test",
			`--repository_cache=${value.cache.repository}`,
			`--disk_cache=${value.cache.disk}`,
			"--experimental_repository_cache_hardlinks",
			"--incompatible_default_to_explicit_init_py",
			"--spawn_strategy=local",
			"--watchfs",
			"--color=no",
			"--curses=no",
		]] as const;
	const result = execute(invocation[0], invocation[1], value.cwd, value.environment);
	writeFileSync(join(logs, `${regime}-${iteration}-${value.runner}.log`), `${result.stderr}${result.stdout}`);
	requireSuccess(`${value.runner} ${regime}`, result);
	if (measured) observations.push({ elapsedMs: result.elapsedMs, iteration, regime, runner: value.runner });
	return result;
}

/** Returns the runnable Django version produced by one completed build. */
function builtVersion(value: Instance): string {
	const executable = join(value.cwd, "bazel-bin", "django-admin");
	const result = value.runner === "bsmr"
		? execute(bsmr, ["--isolation-dir", "benchmark", "run", targetEntry, "--", "--version"], value.cwd, value.environment)
		: execute(executable, ["--version"], value.cwd, value.environment);
	if (value.runner === "bazel" && !existsSync(executable)) throw new Error(`Bazel omitted django-admin: ${executable}`);
	requireSuccess(`${value.runner} django-admin --version`, result);
	return result.stdout.trim().split("\n").at(-1) ?? "";
}

/** Requires each runner to produce one wheel and the expected executable behavior. */
function assertCorrect(value: Instance, expected?: Correctness): Correctness {
	if (value.runner === "bsmr") assertSourceUnmodified(value);
	const wheelDirectory = value.runner === "bsmr" ? value.outputs?.wheel : join(value.cwd, "bazel-bin");
	if (!wheelDirectory) throw new Error("BSMR wheel output is unavailable");
	const wheels = readdirSync(wheelDirectory).filter((name) => name.endsWith(".whl"));
	if (wheels.length !== 1) throw new Error(`${value.runner} produced ${wheels.length} wheels`);
	const payload = wheelPayload(join(wheelDirectory, wheels[0]!), "django/");
	const version = builtVersion(value);
	if (expected !== undefined && version !== expected.version) throw new Error(`${value.runner} produced Django ${version}, expected ${expected.version}`);
	if (expected !== undefined && JSON.stringify(payload) !== JSON.stringify(expected.payload)) throw new Error(`${value.runner} produced a different Django wheel payload`);
	return { payload, version };
}

/** Rejects a backend that wrote generated state into BSMR's immutable source input. */
function assertSourceUnmodified(value: Instance): void {
	const sourceOutput = value.outputs?.source;
	if (!sourceOutput) throw new Error("BSMR source output is unavailable");
	const pending = [sourceOutput];
	while (pending.length > 0) {
		const directory = pending.pop()!;
		for (const entry of readdirSync(directory, { withFileTypes: true })) {
			if (!entry.isDirectory()) continue;
			if (generatedComponents.has(entry.name) || entry.name.startsWith("bazel-") || entry.name.endsWith(".egg-info")) throw new Error(`BSMR's PEP 517 action mutated its source input: ${join(directory, entry.name)}`);
			pending.push(join(directory, entry.name));
		}
	}
}

/** Rewrites one leaf source to a unique but semantically inert benchmark state. */
function editLeaf(value: Instance, original: string, iteration: number): void {
	writeFileSync(join(value.cwd, "django/views/generic/base.py"), `${original.trimEnd()}\n# BSMR benchmark leaf ${iteration}.\n`);
}

/** Deletes materialized outputs while preserving each runner's reusable cache. */
function deleteOutputs(value: Instance): void {
	if (value.runner === "bsmr") {
		if (!value.outputs) throw new Error("BSMR outputs are unavailable for restoration");
		rmSync(value.outputs.environment, { recursive: true });
		rmSync(value.outputs.wheel, { recursive: true });
		return;
	}
	const directory = join(value.cwd, "bazel-bin");
	for (const name of readdirSync(directory)) {
		if (name === "django-admin" || name.startsWith("django-admin.") || name.startsWith("django-")) rmSync(join(directory, name), { force: true, recursive: true });
	}
}

/** Stops one persistent runner after its final observation. */
function stop(value: Instance): void {
	const invocation = value.runner === "bsmr"
		? [bsmr, ["--isolation-dir", "benchmark", "kill"]] as const
		: [bazelisk, [`--output_user_root=${value.outputRoot}`, "shutdown"]] as const;
	execute(invocation[0], invocation[1], value.cwd, value.environment);
}

/** Hashes the complete checked-in benchmark contract. */
function configurationDigest(): string {
	const root = import.meta.dirname;
	const files = [
		join(root, "config.ts"),
		join(root, "helpers.ts"),
		join(root, "prepare.ts"),
		join(root, "run.ts"),
		...readdirSync(fixture).map((name) => join(fixture, name)),
	].sort();
	const hash = createHash("sha256");
	for (const path of files) hash.update(relative(root, path)).update("\0").update(readFileSync(path)).update("\0");
	return hash.digest("hex");
}

/** Runs the complete cache-state matrix and writes its immutable report. */
function main(): void {
	for (const path of [source, bsmr, bazelisk]) if (!existsSync(path)) throw new Error(`benchmark input does not exist: ${path}`);
	if (sha256(bazelisk) !== bazeliskSha256) throw new Error(`Bazelisk digest does not match v1.29.0: ${bazelisk}`);
	const observedCommit = execute("git", ["rev-parse", "HEAD"], source, process.env).stdout.trim();
	if (observedCommit !== djangoCommit) throw new Error(`expected Django ${djangoCommit}, observed ${observedCommit}`);

	for (let iteration = 1; iteration <= coldRuns; iteration += 1) {
		const instances: Record<Runner, Instance> = {
			bazel: instance("bazel", `acquisition-${iteration}`, cachePaths(`acquisition-${iteration}-bazel`)),
			bsmr: instance("bsmr", `acquisition-${iteration}`, cachePaths(`acquisition-${iteration}-bsmr`)),
		};
		let expected: Correctness | undefined;
		for (const runner of runnerOrder(iteration)) {
			build(instances[runner], "acquisition-cold", iteration, true);
			expected = assertCorrect(instances[runner], expected);
		}
		for (const value of Object.values(instances)) stop(value);
	}

	const shared = cachePaths("shared");
	const seedBsmr = instance("bsmr", "seed", shared);
	const seedBazel = instance("bazel", "seed", shared);
	build(seedBsmr, "seed", 0, false);
	const expected = assertCorrect(seedBsmr);
	build(seedBazel, "seed", 0, false);
	assertCorrect(seedBazel, expected);
	stop(seedBsmr);
	stop(seedBazel);

	for (let iteration = 1; iteration <= runs; iteration += 1) {
		const bsmrCache = { ...shared, action: cachePaths(`provisioned-${iteration}-bsmr`).action };
		const bazelCache = { ...shared, disk: cachePaths(`provisioned-${iteration}-bazel`).disk };
		const instances: Record<Runner, Instance> = {
			bazel: instance("bazel", `provisioned-${iteration}`, bazelCache),
			bsmr: instance("bsmr", `provisioned-${iteration}`, bsmrCache),
		};
		for (const runner of runnerOrder(iteration)) {
			build(instances[runner], "provisioned-cold", iteration, true);
			assertCorrect(instances[runner], expected);
		}
		for (const value of Object.values(instances)) stop(value);
	}

	for (let iteration = 1; iteration <= runs; iteration += 1) {
		const instances: Record<Runner, Instance> = {
			bazel: instance("bazel", `shared-${iteration}`, shared),
			bsmr: instance("bsmr", `shared-${iteration}`, shared),
		};
		for (const runner of runnerOrder(iteration)) {
			build(instances[runner], "shared-cache-fresh-checkout", iteration, true);
			assertCorrect(instances[runner], expected);
		}
		for (const value of Object.values(instances)) stop(value);
	}

	const resident: Record<Runner, Instance> = {
		bazel: instance("bazel", "resident", shared),
		bsmr: instance("bsmr", "resident", shared),
	};
	for (const value of Object.values(resident)) {
		build(value, "resident-seed", 0, false);
		assertCorrect(value, expected);
	}
	for (let iteration = 1; iteration <= runs; iteration += 1) {
		for (const runner of runnerOrder(iteration)) build(resident[runner], "resident-noop", iteration, true);
	}
	for (const runner of runnerOrder(1)) runTest(resident[runner], "test-first", 1, true);
	for (let iteration = 1; iteration <= runs; iteration += 1) {
		for (const runner of runnerOrder(iteration)) runTest(resident[runner], "test-cached", iteration, true);
	}

	const originalLeaf = readFileSync(join(source, "django/views/generic/base.py"), "utf8");
	let residentExpected = expected;
	for (let iteration = 1; iteration <= runs; iteration += 1) {
		let edited: Correctness | undefined;
		for (const runner of runnerOrder(iteration)) {
			editLeaf(resident[runner], originalLeaf, iteration);
			build(resident[runner], "leaf-edit", iteration, true);
			edited = assertCorrect(resident[runner], edited);
		}
		const changed = changedWheelEntries(expected.payload, edited?.payload ?? []);
		if (JSON.stringify(changed) !== JSON.stringify(["django/views/generic/base.py"])) throw new Error(`the leaf edit changed unexpected Django payload paths: ${changed.join(", ")}`);
		if (edited !== undefined) residentExpected = edited;
	}
	for (let iteration = 1; iteration <= runs; iteration += 1) {
		for (const runner of runnerOrder(iteration)) {
			deleteOutputs(resident[runner]);
			build(resident[runner], "output-restoration", iteration, true);
			assertCorrect(resident[runner], residentExpected);
		}
	}
	for (const value of Object.values(resident)) stop(value);

	const medians = Object.fromEntries(Array.from(Map.groupBy(observations, ({ regime, runner }) => `${regime}:${runner}`).entries(), ([key, values]) => [key, Number(median(values.map(({ elapsedMs }) => elapsedMs)).toFixed(3))]));
	const performanceGates = performanceGateResults(medians);
	const processor = cpus()[0];
	if (!processor) throw new Error("the host exposes no logical processors");
	const report = {
		configuration: {
			bazel: { spawnStrategy: "local", version: bazelVersion, watchfs: true },
			bazelisk: { sha256: bazeliskSha256, version: "1.29.0" },
			contractSha256: configurationDigest(),
			djangoCommit,
			rulesPythonVersion,
		},
		correctness: { djangoPayloadFiles: expected.payload.length, djangoVersion: expected.version, result: "pass" },
		machine: { architecture: process.arch, logicalCpus: cpus().length, memoryBytes: totalmem(), node: process.version, operatingSystem: platform(), processor: processor.model, release: release() },
		medians,
		observations,
		performanceGates,
		runs: { acquisitionCold: coldRuns, other: runs },
		tools: {
			bsmr: { path: bsmr, sha256: sha256(bsmr) },
			bazelisk: { path: bazelisk, sha256: sha256(bazelisk) },
		},
	};
	const output = join(runRoot, "results.json");
	writeFileSync(output, `${JSON.stringify(report, null, 2)}\n`);
	const regressions = performanceGates.filter(({ pass }) => !pass);
	if (regressions.length > 0) throw new Error(`performance release gates failed; inspect ${output}: ${regressions.map(({ regime }) => regime).join(", ")}`);
	process.stdout.write(`${output}\n`);
}

if (import.meta.main) main();
