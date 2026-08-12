//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Measures native package-path resolution against an explicit target-label control.

import { spawnSync } from "node:child_process";
import { randomUUID } from "node:crypto";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { cpus, platform, release, tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { performance } from "node:perf_hooks";

interface Sample {
	deltaMs: number;
	explicitMs: number;
	iteration: number;
	nativeMs: number;
}

const binary = process.env["BSMR_BENCH_BINARY"];
if (!binary) throw new Error("BSMR_BENCH_BINARY must name the BSMR binary under test");
const executable = resolve(binary);
if (!existsSync(executable)) throw new Error(`BSMR_BENCH_BINARY does not exist: ${executable}`);
const runs = Number.parseInt(process.env["BSMR_BENCH_RUNS"] ?? "51", 10);
if (!Number.isSafeInteger(runs) || runs < 15) throw new Error("BSMR_BENCH_RUNS must be an integer of at least 15");
const maximumMedianOverheadMs = Number.parseFloat(process.env["BSMR_BENCH_MAX_NATIVE_OVERHEAD_MS"] ?? "1");
if (!Number.isFinite(maximumMedianOverheadMs) || maximumMedianOverheadMs < 0) {
	throw new Error("BSMR_BENCH_MAX_NATIVE_OVERHEAD_MS must be non-negative");
}
const root = join(process.env["BSMR_BENCH_ROOT"] ?? join(tmpdir(), "bsmr-benchmarks"), `native-api-${Date.now()}-${randomUUID()}`);

/** Writes one fixture file and creates its parent directory. */
const write = (path: string, contents: string): void => {
	const destination = join(root, path);
	mkdirSync(dirname(destination), { recursive: true });
	writeFileSync(destination, contents);
};

/** Serializes deterministic fixture JSON with a trailing newline. */
const json = (value: unknown): string => `${JSON.stringify(value, null, 2)}\n`;

write(".bsmr", `[cells]
root = .
prelude = prelude
none = none

[cell_aliases]
config = prelude
ovr_config = prelude
upstream = none
toolchains = root

[external_cells]
prelude = bundled

[parser]
target_platform_detector_spec = target:root//...->prelude//platforms:default target:prelude//...->prelude//platforms:default target:toolchains//...->prelude//platforms:default

[build]
execution_platforms = prelude//platforms:default
`);
write("package.json", json({ name: "@bsmr/benchmark", private: true, engines: { node: "26.5.1" }, packageManager: "pnpm@11.20.0+sha512.9a6f330a95b66446ea088faf1521405a8a01f07fde7124cc9958dfed52d4bb436737e65b08f85f37b46fcba375092558ac51262b816844b22f63406ed166bfee" }));
write("pnpm-lock.yaml", "lockfileVersion: '9.0'\n");
write("pnpm-workspace.yaml", "packages:\n  - apps/*\n");
write("apps/api/package.json", json({ name: "@bsmr/api", private: true }));
write("apps/api/src/index.ts", "export const api = true;\n");
write("apps/api/tsconfig.json", json({ compilerOptions: { strict: true } }));
write("apps/api/tsdown.config.ts", "export default {};\n");

const common = ["targets", "--isolation-dir", `native-api-${randomUUID()}`, "--console", "simple"];
const commands = {
	explicit: [...common, "root//apps/api:api"],
	native: [...common, "apps/api"],
};

/** Executes one target lookup, validates its result, and returns elapsed milliseconds. */
const measure = (args: readonly string[]): number => {
	const start = performance.now();
	const result = spawnSync(executable, args, { cwd: root, encoding: "utf8", env: { ...process.env, NO_COLOR: "1" } });
	const elapsedMs = performance.now() - start;
	if (result.status !== 0) throw new Error(`${result.stderr}${result.stdout}`);
	if (result.stdout.trim() !== "root//apps/api:api") throw new Error(`unexpected target result: ${result.stdout}`);
	return elapsedMs;
};

for (let iteration = 0; iteration < 5; iteration += 1) {
	measure(commands.explicit);
	measure(commands.native);
}
const samples: Sample[] = [];
for (let iteration = 0; iteration < runs; iteration += 1) {
	const order = iteration % 2 === 0 ? ["explicit", "native"] as const : ["native", "explicit"] as const;
	const elapsed = new Map(order.map((name) => [name, measure(commands[name])]));
	const explicitMs = elapsed.get("explicit")!;
	const nativeMs = elapsed.get("native")!;
	samples.push({ deltaMs: nativeMs - explicitMs, explicitMs, iteration, nativeMs });
}

/** Returns the nearest-rank percentile from a non-empty sample. */
const percentile = (values: readonly number[], fraction: number): number => {
	const sorted = [...values].sort((left, right) => left - right);
	return sorted[Math.min(Math.floor(sorted.length * fraction), sorted.length - 1)]!;
};

const medianDeltaMs = percentile(samples.map(({ deltaMs }) => deltaMs), 0.5);
const report = {
	environment: { arch: process.arch, cpus: cpus().length, node: process.version, platform: platform(), release: release() },
	gate: { maximumMedianOverheadMs, passed: medianDeltaMs <= maximumMedianOverheadMs },
	results: {
		explicitMedianMs: percentile(samples.map(({ explicitMs }) => explicitMs), 0.5),
		medianDeltaMs,
		nativeMedianMs: percentile(samples.map(({ nativeMs }) => nativeMs), 0.5),
		nativeP95Ms: percentile(samples.map(({ nativeMs }) => nativeMs), 0.95),
	},
	runs,
	samples,
};
write("results.json", `${JSON.stringify(report, null, 2)}\n`);
process.stdout.write(`${join(root, "results.json")}\n`);
if (!report.gate.passed) {
	throw new Error(`native path median overhead ${medianDeltaMs.toFixed(3)}ms exceeds ${maximumMedianOverheadMs.toFixed(3)}ms`);
}
