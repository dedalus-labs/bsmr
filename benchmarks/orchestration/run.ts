//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Runs correctness-gated task-orchestration benchmarks against BSMR, Nx, and Turborepo.

import { execFileSync, spawnSync } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, truncateSync, writeFileSync } from "node:fs";
import { cpus, freemem, platform, release, tmpdir, totalmem } from "node:os";
import { join, resolve } from "node:path";
import { performance } from "node:perf_hooks";

import { generateFixture, packageNames, type Runner } from "./fixture.ts";

interface Observation {
	elapsedMs: number;
	executions: number;
	iteration: number;
	outputDigest: string;
	regime: string;
	runner: Runner;
}

interface RunnerConfig {
	args: readonly string[];
	cwd: string;
	executable: string;
	trace: string;
}

const repository = resolve(import.meta.dirname, "../..");
const binary = process.env["BSMR_BENCH_BINARY"];
if (!binary) throw new Error("BSMR_BENCH_BINARY must name the BSMR binary under test");
if (!existsSync(binary)) throw new Error(`BSMR_BENCH_BINARY does not exist: ${binary}`);
const runs = Number.parseInt(process.env["BSMR_BENCH_RUNS"] ?? "3", 10);
if (!Number.isSafeInteger(runs) || runs < 3) throw new Error("BSMR_BENCH_RUNS must be an integer of at least 3");
const concurrency = Number.parseInt(process.env["BSMR_BENCH_CONCURRENCY"] ?? String(cpus().length), 10);
if (!Number.isSafeInteger(concurrency) || concurrency < 1) throw new Error("BSMR_BENCH_CONCURRENCY must be positive");
const remoteCache = process.env["BSMR_BENCH_REMOTE_CACHE"] ?? "grpc://127.0.0.1:9092";
const runRoot = join(process.env["BSMR_BENCH_ROOT"] ?? join(tmpdir(), "bsmr-benchmarks"), `orchestration-${Date.now()}-${randomUUID()}`);
const logs = join(runRoot, "logs");
mkdirSync(logs, { recursive: true });

/** Executes a setup command and returns trimmed stdout or fails with its complete output. */
const execute = (executable: string, args: readonly string[], cwd: string): string => {
	try {
		return execFileSync(executable, args, { cwd, encoding: "utf8", env: process.env, maxBuffer: 64 * 1024 * 1024 }).trim();
	} catch (error) {
		const failure = error as { stderr?: Buffer | string; stdout?: Buffer | string };
		throw new Error(`command failed: ${executable} ${args.join(" ")}\n${String(failure.stdout ?? "")}${String(failure.stderr ?? "")}`);
	}
};

const configs = Object.fromEntries((["bsmr", "turbo", "nx"] as const).map((runner) => {
	const cwd = join(runRoot, runner);
	const trace = generateFixture(cwd, runner, repository, remoteCache);
	writeFileSync(trace, "");
	return [runner, { cwd, trace }];
})) as Record<Runner, Pick<RunnerConfig, "cwd" | "trace">>;
for (const runner of ["turbo", "nx"] as const) execute("pnpm", ["install", "--no-frozen-lockfile"], configs[runner].cwd);
const runners: Record<Runner, RunnerConfig> = {
	bsmr: { ...configs.bsmr, executable: resolve(binary), args: ["build", "-M", "all", ...packageNames.map((name) => `//:${name}`), "--console=simple"] },
	turbo: { ...configs.turbo, executable: join(configs.turbo.cwd, "node_modules/.bin/turbo"), args: ["run", "build", `--concurrency=${concurrency}`, "--output-logs=errors-only"] },
	nx: { ...configs.nx, executable: join(configs.nx.cwd, "node_modules/.bin/nx"), args: ["run-many", "-t", "build", "--all", `--parallel=${concurrency}`, "--outputStyle=static"] },
};

/** Updates a package input with identical content across all orchestrators. */
const setSource = (runner: Runner, name: string, token: string): void => writeFileSync(join(runners[runner].cwd, `packages/${name}/src.txt`), `${name}: ${token}\n`);

/** Updates an intentionally untracked documentation input. */
const setDocs = (runner: Runner, token: string): void => writeFileSync(join(runners[runner].cwd, "README.md"), `orchestration benchmark: ${token}\n`);

/** Recursively finds logical task outputs for correctness and restoration gates. */
const outputs = (runner: Runner): string[] => {
	const found: string[] = [];
	/** Walks the generated output tree without following symlinks. */
	const visit = (directory: string): void => {
		for (const entry of readdirSync(directory, { withFileTypes: true })) {
			const path = join(directory, entry.name);
			if (entry.isDirectory()) visit(path);
			else if (entry.name === "output.json") found.push(path);
		}
	};
	visit(runner === "bsmr" ? join(runners.bsmr.cwd, "bsmr-out") : join(runners[runner].cwd, "packages"));
	return found.sort();
};

/** Hashes parsed logical results independent of each tool's output paths. */
const outputDigest = (runner: Runner): string => {
	const results = outputs(runner).map((path) => JSON.parse(readFileSync(path, "utf8")) as { digest: string; name: string }).sort((left, right) => left.name.localeCompare(right.name));
	if (results.length !== packageNames.length) throw new Error(`${runner}: expected ${packageNames.length} outputs, observed ${results.length}`);
	return createHash("sha256").update(results.map(({ name, digest }) => `${name}:${digest}`).join("\n")).digest("hex");
};

/** Counts executed tasks from the append-only workload trace. */
const executionCount = (runner: Runner): number => {
	const trace = readFileSync(runners[runner].trace, "utf8");
	return trace === "" ? 0 : trace.trimEnd().split("\n").length;
};

/** Executes one measured build and enforces task-count and output-correctness gates. */
const measure = (runner: Runner, regime: string, iteration: number, expectedExecutions: number | null): Observation => {
	truncateSync(runners[runner].trace, 0);
	const config = runners[runner];
	const start = performance.now();
	const result = spawnSync(config.executable, config.args, { cwd: config.cwd, encoding: "utf8", env: { ...process.env, CI: "1", NO_COLOR: "1", NX_CLOUD: "false", NX_DAEMON: "true", TURBO_TELEMETRY_DISABLED: "1" }, maxBuffer: 64 * 1024 * 1024 });
	const elapsedMs = performance.now() - start;
	const output = `${result.stdout ?? ""}${result.stderr ?? ""}`;
	writeFileSync(join(logs, `${regime}-${iteration}-${runner}.log`), output);
	if (result.status !== 0) throw new Error(`${runner} ${regime} failed (${result.status})\n${output}`);
	const executions = executionCount(runner);
	if (expectedExecutions !== null && executions !== expectedExecutions) throw new Error(`${runner} ${regime}: expected ${expectedExecutions} executions, observed ${executions}`);
	return { elapsedMs, executions, iteration, outputDigest: outputDigest(runner), regime, runner };
};

/** Removes materialized outputs while preserving each tool's reusable cache. */
const removeOutputs = (runner: Runner): void => {
	if (runner === "bsmr") {
		for (const output of outputs(runner)) rmSync(output);
		return;
	}
	for (const name of packageNames) rmSync(join(runners[runner].cwd, `packages/${name}/dist`), { force: true, recursive: true });
};

const observations: Observation[] = [];
const order = ["bsmr", "turbo", "nx"] as const;
const token = randomUUID();
for (const runner of order) measure(runner, "warmup", 0, null);
for (let iteration = 1; iteration <= runs; iteration += 1) {
	for (const runner of order) observations.push(measure(runner, "noop", iteration, 0));
}
for (let iteration = 1; iteration <= runs; iteration += 1) {
	for (const runner of order) {
		setSource(runner, "app0", `${token}-leaf-${iteration}`);
		observations.push(measure(runner, "leaf", iteration, 1));
	}
}
for (let iteration = 1; iteration <= runs; iteration += 1) {
	for (const runner of order) {
		setSource(runner, "shared", `${token}-full-${iteration}`);
		observations.push(measure(runner, "full", iteration, packageNames.length));
	}
}
for (let iteration = 1; iteration <= runs; iteration += 1) {
	for (const runner of order) {
		setDocs(runner, `${token}-docs-${iteration}`);
		observations.push(measure(runner, "docs", iteration, 0));
	}
}
for (let iteration = 1; iteration <= runs; iteration += 1) {
	for (const runner of order) {
		removeOutputs(runner);
		observations.push(measure(runner, "restore", iteration, 0));
	}
}
for (const group of Map.groupBy(observations, ({ regime, iteration }) => `${regime}:${iteration}`).values()) {
	if (new Set(group.map(({ outputDigest: digest }) => digest)).size !== 1) {
		throw new Error(`logical output mismatch: ${JSON.stringify(group)}`);
	}
}

const medians = Object.fromEntries(
	Array.from(
		Map.groupBy(observations, ({ regime, runner }) => `${regime}:${runner}`).entries(),
		([key, values]) => {
			const samples = values.map(({ elapsedMs }) => elapsedMs).sort((left, right) => left - right);
			return [key, Number(samples[Math.floor(samples.length / 2)]!.toFixed(3))];
		},
	),
);
const report = {
	environment: {
		arch: process.arch,
		concurrency,
		cpus: cpus().length,
		freeMemory: freemem(),
		node: process.version,
		platform: platform(),
		release: release(),
		remoteCache,
		runs,
		totalMemory: totalmem(),
		versions: {
			bsmr: execute(resolve(binary), ["--version"], repository),
			nx: execute(runners.nx.executable, ["--version"], runners.nx.cwd),
			pnpm: execute("pnpm", ["--version"], repository),
			turbo: execute(runners.turbo.executable, ["--version"], runners.turbo.cwd),
		},
	},
	medians,
	observations,
};
writeFileSync(join(runRoot, "results.json"), `${JSON.stringify(report, null, 2)}\n`);
process.stdout.write(`${join(runRoot, "results.json")}\n`);
