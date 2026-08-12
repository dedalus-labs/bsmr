//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Runs correctness- and action-gated Go build benchmarks against BSMR and Bazel.

import { execFileSync, spawnSync } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cpus, freemem, platform, release, tmpdir, totalmem } from "node:os";
import { join, resolve } from "node:path";
import { performance } from "node:perf_hooks";

import {
	applicationTargets,
	generateFixture,
	setDocumentation,
	setExportedAbi,
	setPrivateSeed,
	type Runner,
} from "./fixture.ts";

interface Observation {
	actions: number;
	elapsedMs: number;
	iteration: number;
	outputDigest: string;
	regime: string;
	runner: Runner;
}

interface RunnerConfig {
	instance: string;
	outputBase: string;
	root: string;
	runner: Runner;
}

const repository = resolve(import.meta.dirname, "../..");
const bsmr = resolve(process.env["BSMR_GO_BENCH_BINARY"] ?? "");
if (!process.env["BSMR_GO_BENCH_BINARY"] || !existsSync(bsmr)) {
	throw new Error("BSMR_GO_BENCH_BINARY must name the BSMR binary under test");
}
const bazel = resolve(process.env["BSMR_GO_BENCH_BAZEL"] ?? "");
if (!process.env["BSMR_GO_BENCH_BAZEL"] || !existsSync(bazel)) {
	throw new Error("BSMR_GO_BENCH_BAZEL must name a Bazel or Bazelisk executable");
}
const runs = Number.parseInt(process.env["BSMR_GO_BENCH_RUNS"] ?? "3", 10);
if (!Number.isSafeInteger(runs) || runs < 3) throw new Error("BSMR_GO_BENCH_RUNS must be an integer of at least 3");
const mode = process.env["BSMR_GO_BENCH_MODE"] ?? "pure";
if (mode !== "cgo" && mode !== "pure") throw new Error("BSMR_GO_BENCH_MODE must be `cgo` or `pure`");
const remoteCache = process.env["BSMR_GO_BENCH_REMOTE_CACHE"] ?? "grpc://127.0.0.1:9092";
const runId = randomUUID();
const cacheNamespace = process.env["BSMR_GO_BENCH_CACHE_NAMESPACE"] ?? runId;
const runRoot = join(process.env["BSMR_GO_BENCH_ROOT"] ?? join(tmpdir(), "bsmr-benchmarks"), `go-${mode}-${Date.now()}-${runId}`);
const logs = join(runRoot, "logs");
mkdirSync(logs, { recursive: true });

/** Executes an untimed setup command and preserves complete failure output. */
const execute = (executable: string, args: readonly string[], cwd: string): string => {
	try {
		return execFileSync(executable, args, {
			cwd,
			encoding: "utf8",
			env: process.env,
			maxBuffer: 128 * 1024 * 1024,
		}).trim();
	} catch (error) {
		const failure = error as { stderr?: Buffer | string; stdout?: Buffer | string };
		throw new Error(`command failed: ${executable} ${args.join(" ")}\n${String(failure.stdout ?? "")}${String(failure.stderr ?? "")}`);
	}
};
const hostGo = execute("go", ["version"], repository);
const hostGoVersion = execute("go", ["env", "GOVERSION"], repository).replace(/^go/, "");

/** Returns flags shared by every Bazel command in one fixture. */
const bazelStartup = (config: RunnerConfig): string[] => [`--output_base=${config.outputBase}`];

/** Returns remote-cache flags shared by every measured Bazel build. */
const bazelCache = (config: RunnerConfig): string[] => [
	`--remote_cache=${remoteCache}`,
	`--remote_instance_name=${config.instance}`,
	`--disk_cache=${join(config.outputBase, "disk-cache")}`,
	"--remote_upload_local_results=true",
];

/** Generates, synchronizes, and toolchain-primes one runner without compiling the measured DAG. */
const setup = (runner: Runner, iteration: number): RunnerConfig => {
	const root = join(runRoot, `cold-${iteration}`, runner);
	const config: RunnerConfig = {
		instance: `bsmr-go-${runner}-${cacheNamespace}`,
		outputBase: join(runRoot, `cold-${iteration}`, `${runner}-output`),
		root,
		runner,
	};
	generateFixture(root, runner, repository, remoteCache, config.instance, `${runId}-cold-${iteration}`, mode);
	if (runner === "bsmr") {
		execute(bsmr, ["go", "toolchain", "--version", hostGoVersion], root);
		execute(bsmr, ["go", "sync", ...(mode === "cgo" ? ["--cgo"] : [])], root);
		execute(bsmr, ["--isolation-dir", "go-bench", "build", "//cmd/prime:bin", "--console=simple"], root);
	} else {
		execute(bazel, [...bazelStartup(config), "build", "//cmd/prime:bin", ...bazelCache(config), "--color=no", "--curses=no", "--noshow_progress"], root);
	}
	return config;
};

/** Creates a clean checkout with the final graph and an already-populated remote cache. */
const setupRemoteHit = (runner: Runner, iteration: number, source: RunnerConfig, abi: number, seed: number): RunnerConfig => {
	const root = join(runRoot, `remote-${iteration}`, runner);
	const config: RunnerConfig = {
		instance: source.instance,
		outputBase: join(runRoot, `remote-${iteration}`, `${runner}-output`),
		root,
		runner,
	};
	generateFixture(root, runner, repository, remoteCache, config.instance, `${runId}-cold-${runs}`, mode);
	setExportedAbi(root, mode, abi, seed);
	setDocumentation(root, `${runId}-${runs}`);
	if (runner === "bsmr") {
		execute(bsmr, ["go", "toolchain", "--version", hostGoVersion], root);
		execute(bsmr, ["go", "sync", ...(mode === "cgo" ? ["--cgo"] : [])], root);
	}
	return config;
};

/** Parses materialized BSMR binary paths from full JSON build output. */
const bsmrOutputs = (stdout: string): string[] => {
	const line = stdout.trimEnd().split("\n").findLast((candidate) => candidate.startsWith("{"));
	if (!line) throw new Error(`BSMR build emitted no output map:\n${stdout}`);
	const outputs = Object.values(JSON.parse(line) as Record<string, string>).sort();
	if (outputs.length !== applicationTargets.length) {
		throw new Error(`BSMR emitted ${outputs.length} binaries; expected ${applicationTargets.length}`);
	}
	return outputs;
};

/** Queries Bazel for the exact materialized outputs of all application targets. */
const bazelOutputs = (config: RunnerConfig): string[] => {
	const output = execute(
		bazel,
		[...bazelStartup(config), "cquery", `set(${applicationTargets.join(" ")})`, "--output=files", "--color=no", "--curses=no", "--noshow_progress"],
		config.root,
	);
	const outputs = output.split("\n").filter(Boolean).map((path) => resolve(config.root, path)).sort();
	if (outputs.length !== applicationTargets.length) {
		throw new Error(`Bazel emitted ${outputs.length} binaries; expected ${applicationTargets.length}`);
	}
	return outputs;
};

/** Executes every application and hashes its stdout as the build-correctness oracle. */
const outputDigest = (outputs: readonly string[], root: string): string => {
	const values = outputs.map((output) => execute(output, [], root)).sort();
	return createHash("sha256").update(values.join("\n")).digest("hex");
};

/** Counts logical BSMR package-compilation and binary-link phases that executed. */
const bsmrActionCount = (config: RunnerConfig): number => {
	const output = execute(
		bsmr,
		["--isolation-dir", "go-bench", "log", "what-ran", "--format", "json", "--filter-category", "go_compile|go_link|go_pack"],
		config.root,
	);
	if (output === "") return 0;
	const phases = output.split("\n")
		.map((line) => JSON.parse(line) as { identity?: string; reproducer?: { executor?: string } })
		.filter(({ reproducer }) => reproducer?.executor === "Local" || reproducer?.executor === "Remote")
		.map(({ identity }) => {
			if (!identity) throw new Error("BSMR action record omitted its identity");
			const target = identity.split(" (")[0];
			return `${target}:${identity.includes("(go_link ") ? "link" : "compile"}`;
		});
	return new Set(phases).size;
};

/** Splits Bazel's concatenated JSON-object stream without misreading braces in strings. */
const splitJsonObjects = (content: string): string[] => {
	const objects: string[] = [];
	let depth = 0;
	let escaped = false;
	let start = -1;
	let quoted = false;
	for (let index = 0; index < content.length; index += 1) {
		const character = content[index]!;
		if (quoted) {
			if (escaped) escaped = false;
			else if (character === "\\") escaped = true;
			else if (character === '"') quoted = false;
			continue;
		}
		if (character === '"') quoted = true;
		else if (character === "{") {
			if (depth === 0) start = index;
			depth += 1;
		} else if (character === "}") {
			depth -= 1;
			if (depth < 0 || (depth === 0 && start < 0)) throw new Error("malformed Bazel execution log");
			if (depth === 0) {
				objects.push(content.slice(start, index + 1));
				start = -1;
			}
		} else if (depth === 0 && !/\s/.test(character)) throw new Error("malformed Bazel execution log separator");
	}
	if (depth !== 0 || quoted || escaped || start !== -1) throw new Error("truncated Bazel execution log");
	return objects;
};

/** Normalizes Bazel's streamed JSON execution log into individual action records. */
const bazelExecutionEntries = (path: string): Record<string, unknown>[] => {
	if (!existsSync(path)) throw new Error(`Bazel did not create execution log: ${path}`);
	const content = readFileSync(path, "utf8").trim();
	if (content === "") return [];
	return splitJsonObjects(content).map((entry) => JSON.parse(entry) as Record<string, unknown>);
};

/** Counts Bazel rules_go compilation, assembly, and linking actions that missed cache. */
const bazelActionCount = (path: string): number => bazelExecutionEntries(path).filter((entry) => {
	const mnemonic = entry["mnemonic"];
	return typeof mnemonic === "string" && /^Go(?:Compile|Link|Asm|Pack)/.test(mnemonic) && entry["cacheHit"] !== true;
}).length;

/** Executes one timed build, then enforces output and action evidence outside the timed region. */
const measure = (config: RunnerConfig, regime: string, iteration: number): Observation & { outputs: string[] } => {
	const executionLog = join(logs, `${regime}-${iteration}-bazel-execution.json`);
	rmSync(executionLog, { force: true });
	const args = config.runner === "bsmr"
		? ["--isolation-dir", "go-bench", "build", "-M", "all", ...applicationTargets, "--show-full-json-output", "--console=simple"]
		: [...bazelStartup(config), "build", ...applicationTargets, ...bazelCache(config), `--execution_log_json_file=${executionLog}`, "--color=no", "--curses=no", "--noshow_progress", "--show_result=0"];
	const executable = config.runner === "bsmr" ? bsmr : bazel;
	const start = performance.now();
	const result = spawnSync(executable, args, {
		cwd: config.root,
		encoding: "utf8",
		env: { ...process.env, CI: "1", CGO_ENABLED: mode === "cgo" ? "1" : "0", NO_COLOR: "1" },
		maxBuffer: 128 * 1024 * 1024,
	});
	const elapsedMs = performance.now() - start;
	const output = `${result.stdout ?? ""}${result.stderr ?? ""}`;
	writeFileSync(join(logs, `${regime}-${iteration}-${config.runner}.log`), output);
	if (result.error || result.status !== 0) {
		throw new Error(`${config.runner} ${regime} failed (${result.status}): ${String(result.error ?? "")}\n${output}`);
	}
	const outputs = config.runner === "bsmr" ? bsmrOutputs(result.stdout) : bazelOutputs(config);
	const actions = config.runner === "bsmr" ? bsmrActionCount(config) : bazelActionCount(executionLog);
	return {
		actions,
		elapsedMs,
		iteration,
		outputDigest: outputDigest(outputs, config.root),
		outputs,
		regime,
		runner: config.runner,
	};
};

/** Rejects regimes that executed compiler work despite no graph-relevant source change. */
const validateActionBoundary = (observation: Observation): void => {
	const expectedZero = ["docs", "noop", "remote", "restore"].includes(observation.regime);
	if (expectedZero && observation.actions !== 0) {
		throw new Error(`${observation.runner} ${observation.regime}: expected zero Go actions, observed ${observation.actions}`);
	}
	if (!expectedZero && observation.actions === 0) {
		throw new Error(`${observation.runner} ${observation.regime}: expected Go actions, observed zero`);
	}
};

const observations: Observation[] = [];
let active: Record<Runner, RunnerConfig> | undefined;
let outputs: Record<Runner, string[]> | undefined;
/** Stores a measured sample without leaking machine-specific output paths into results. */
const record = (observation: ReturnType<typeof measure>): void => {
	observations.push({
		actions: observation.actions,
		elapsedMs: observation.elapsedMs,
		iteration: observation.iteration,
		outputDigest: observation.outputDigest,
		regime: observation.regime,
		runner: observation.runner,
	});
};

for (let iteration = 1; iteration <= runs; iteration += 1) {
	const configs = { bazel: setup("bazel", iteration), bsmr: setup("bsmr", iteration) };
	const order: readonly Runner[] = iteration % 2 === 0 ? ["bazel", "bsmr"] : ["bsmr", "bazel"];
	const measured = Object.fromEntries(order.map((runner) => [runner, measure(configs[runner], "cold", iteration)])) as Record<Runner, ReturnType<typeof measure>>;
	for (const runner of order) record(measured[runner]);
	active = configs;
	outputs = { bazel: measured.bazel.outputs, bsmr: measured.bsmr.outputs };
}
if (!active || !outputs) throw new Error("benchmark produced no active fixtures");

for (let iteration = 1; iteration <= runs; iteration += 1) {
	for (const runner of ["bsmr", "bazel"] as const) record(measure(active[runner], "noop", iteration));
}
let seed = 1;
for (let iteration = 1; iteration <= runs; iteration += 1) {
	seed = 100 + iteration;
		for (const runner of ["bsmr", "bazel"] as const) setPrivateSeed(active[runner].root, mode, seed);
	for (const runner of ["bsmr", "bazel"] as const) record(measure(active[runner], "private", iteration));
}
for (let iteration = 1; iteration <= runs; iteration += 1) {
		for (const runner of ["bsmr", "bazel"] as const) setExportedAbi(active[runner].root, mode, 100 + iteration, seed);
	for (const runner of ["bsmr", "bazel"] as const) record(measure(active[runner], "api", iteration));
}
for (let iteration = 1; iteration <= runs; iteration += 1) {
	for (const runner of ["bsmr", "bazel"] as const) setDocumentation(active[runner].root, `${runId}-${iteration}`);
	for (const runner of ["bsmr", "bazel"] as const) record(measure(active[runner], "docs", iteration));
}
for (let iteration = 1; iteration <= runs; iteration += 1) {
	for (const runner of ["bsmr", "bazel"] as const) {
		for (const output of outputs[runner]) rmSync(output, { force: true });
		const observation = measure(active[runner], "restore", iteration);
		record(observation);
		outputs[runner] = observation.outputs;
	}
}

const finalAbi = 100 + runs;
const finalDigests = new Set([
	outputDigest(outputs.bsmr, active.bsmr.root),
	outputDigest(outputs.bazel, active.bazel.root),
]);
if (finalDigests.size !== 1) throw new Error("final BSMR and Bazel outputs differ before remote restoration");
const finalDigest = finalDigests.values().next().value;
if (!finalDigest) throw new Error("final output digest is unavailable");
for (let iteration = 1; iteration <= runs; iteration += 1) {
	for (const runner of ["bsmr", "bazel"] as const) {
		const clone = setupRemoteHit(runner, iteration, active[runner], finalAbi, seed);
		const observation = measure(clone, "remote", iteration);
		if (observation.outputDigest !== finalDigest) {
			throw new Error(`${runner} remote output differs from the populated-cache output`);
		}
		record(observation);
	}
}

for (const observation of observations) validateActionBoundary(observation);
for (const group of Map.groupBy(observations, ({ regime, iteration }) => `${regime}:${iteration}`).values()) {
	if (new Set(group.map(({ outputDigest: digest }) => digest)).size !== 1) {
		throw new Error(`logical output mismatch: ${JSON.stringify(group)}`);
	}
	if (new Set(group.map(({ actions }) => actions)).size !== 1) {
		throw new Error(`invalidation-cut mismatch: ${JSON.stringify(group)}`);
	}
}

/** Computes one median with the required lower bound of three samples. */
const median = (values: readonly number[]): number => {
	if (values.length < 3) throw new Error(`median requires at least three samples, got ${values.length}`);
	const sorted = [...values].sort((left, right) => left - right);
	return Number(sorted[Math.floor(sorted.length / 2)]!.toFixed(3));
};

const medians = Object.fromEntries(Array.from(
	Map.groupBy(observations, ({ regime, runner }) => `${regime}:${runner}`).entries(),
	([key, values]) => [key, { actions: median(values.map(({ actions }) => actions)), elapsedMs: median(values.map(({ elapsedMs }) => elapsedMs)) }],
));
const report = {
	environment: {
		arch: process.arch,
		cacheNamespace,
		cCompiler: mode === "cgo" ? execute("cc", ["--version"], repository) : null,
		cpus: cpus().length,
		freeMemory: freemem(),
		go: hostGo,
		mode,
		node: process.version,
		platform: platform(),
		release: release(),
		remoteCache,
		rulesGo: "0.62.0",
		runs,
		totalMemory: totalmem(),
		versions: {
			bazel: execute(bazel, [...bazelStartup(active.bazel), "version"], active.bazel.root),
			bsmr: execute(bsmr, ["--version"], repository),
		},
	},
	medians,
	observations,
};
writeFileSync(join(runRoot, "results.json"), `${JSON.stringify(report, null, 2)}\n`);
process.stdout.write(`${join(runRoot, "results.json")}\n`);
