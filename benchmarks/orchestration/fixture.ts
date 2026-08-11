//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Generates equivalent 33-package task graphs for BSMR, Nx, and Turborepo.

import { mkdirSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

export type Runner = "bsmr" | "nx" | "turbo";

const cores = Array.from({ length: 8 }, (_, index) => `core${index}`);
const libraries = Array.from({ length: 16 }, (_, index) => `lib${index}`);
const applications = Array.from({ length: 8 }, (_, index) => `app${index}`);

export const dependencies = new Map<string, readonly string[]>([
	["shared", []],
	...cores.map((name) => [name, ["shared"]] as const),
	...libraries.map(
		(name, index) =>
			[name, [cores[index % cores.length]!, cores[(index + 1) % cores.length]!]] as const,
	),
	...applications.map(
		(name, index) => [name, [libraries[index * 2]!, libraries[index * 2 + 1]!]] as const,
	),
]);

export const packageNames = [...dependencies.keys()];

/** Writes one generated fixture file beneath the benchmark root. */
const write = (root: string, path: string, contents: string): void => {
	const destination = join(root, path);
	mkdirSync(dirname(destination), { recursive: true });
	writeFileSync(destination, contents);
};

/** Serializes deterministic fixture JSON with a trailing newline. */
const json = (value: unknown): string => `${JSON.stringify(value, null, 2)}\n`;

/** Renders the deterministic CPU workload shared by all three orchestrators. */
const workload = (): string => `import { appendFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
import { createHash } from "node:crypto";
const values = new Map();
for (let index = 2; index < process.argv.length; index += 2) {
  const key = process.argv[index], value = process.argv[index + 1];
  if (!key || !value) throw new Error("arguments must be key-value pairs");
  if (key === "--dep") values.set(key, [...(values.get(key) ?? []), value]);
  else values.set(key, value);
}
const name = values.get("--name"), input = values.get("--input");
const output = values.get("--output"), trace = values.get("--trace");
if (!name || !input || !output || !trace) throw new Error("missing required argument");
const deps = (values.get("--dep") ?? []).map((path) => JSON.parse(readFileSync(path, "utf8")))
  .sort((left, right) => left.name.localeCompare(right.name));
let digest = createHash("sha256").update(name).update("\\0").update(readFileSync(input))
  .update("\\0").update(deps.map((dep) => dep.digest).join("\\0")).digest("hex");
for (let index = 0; index < 25_000; index += 1) digest = createHash("sha256").update(digest).digest("hex");
mkdirSync(dirname(output), { recursive: true });
writeFileSync(output, JSON.stringify({ name, digest }) + "\\n");
appendFileSync(trace, name + "\\n");
`;

/** Renders the BSMR execution platform that enables standard remote action caching. */
const platform = (): string => `load("@prelude//cfg/exec_platform:marker.bzl", "get_exec_platform_marker")
def _impl(ctx):
    constraints = dict()
    constraints.update(ctx.attrs.cpu[ConfigurationInfo].constraints)
    constraints.update(ctx.attrs.os[ConfigurationInfo].constraints)
    cfg = ConfigurationInfo(constraints = constraints, values = {})
    info = ExecutionPlatformInfo(label = ctx.label.raw_target(), configuration = cfg, executor_config = CommandExecutorConfig(local_enabled = True, remote_enabled = False, remote_cache_enabled = True, allow_cache_uploads = True, use_windows_path_separators = host_info().os.is_windows))
    return [DefaultInfo(), info, PlatformInfo(label = str(ctx.label.raw_target()), configuration = cfg), ExecutionPlatformRegistrationInfo(platforms = [info], exec_marker_constraint = get_exec_platform_marker())]
cache_platform = rule(impl = _impl, attrs = {"cpu": attrs.dep(providers = [ConfigurationInfo]), "os": attrs.dep(providers = [ConfigurationInfo])})
`;

/** Renders BSMR rules with the same dependency graph as the native workspace tools. */
const buildManifest = (trace: string): string => {
	const rules = [...dependencies].map(([name, deps]) => {
		const args = deps.map((dep) => `--dep $(location :${dep})`).join(" ");
		return `genrule(\n    name = "${name}",\n    srcs = ["scripts/build.mjs", "packages/${name}/src.txt"],\n    cmd = '$(location toolchains//:node) $SRCDIR/scripts/build.mjs --name ${name} --input $SRCDIR/packages/${name}/src.txt ${args} --output $OUT --trace "${trace}"',\n    out = "output.json",\n    labels = ["large_copy"],\n)\n`;
	});
	const outputs = packageNames.map((name) => `"${name}.json": ":${name}"`).join(", ");
	return `${rules.join("\n")}\nfilegroup(name = "all", srcs = {${outputs}})\n`;
};

/** Generates one isolated runner fixture and returns its absolute trace path. */
export const generateFixture = (
	root: string,
	runner: Runner,
	repository: string,
	remoteCache: string,
): string => {
	if (/\s/.test(root)) throw new Error(`benchmark path must not contain whitespace: ${root}`);
	rmSync(root, { force: true, recursive: true });
	mkdirSync(root, { recursive: true });
	const trace = join(dirname(root), `${runner}-executions.log`);
	write(root, "README.md", "generated by benchmarks/orchestration/fixture.ts\n");
	write(root, "pnpm-workspace.yaml", "packages:\n  - packages/*\n");
	write(root, "scripts/build.mjs", workload());
	write(root, "nx.json", json({ namedInputs: { default: ["{projectRoot}/**/*", "!{projectRoot}/dist/**"] }, targetDefaults: { build: { cache: true, dependsOn: ["^build"], inputs: ["default", "^default"], outputs: ["{projectRoot}/dist"] } } }));
	write(root, "turbo.json", json({ $schema: "https://turborepo.com/schema.json", tasks: { build: { dependsOn: ["^build"], inputs: ["src.txt"], outputs: ["dist/**"] } } }));
	write(root, "package.json", json({ name: `orchestration-bench-${runner}`, private: true, packageManager: "pnpm@10.30.3", workspaces: ["packages/*"], devDependencies: runner === "nx" ? { nx: "23.1.1" } : runner === "turbo" ? { turbo: "2.10.9" } : {} }));
	for (const [name, deps] of dependencies) {
		const depArgs = deps.map((dep) => `--dep ../${dep}/dist/output.json`);
		const command = ["node ../../scripts/build.mjs", `--name ${name}`, "--input src.txt", ...depArgs, "--output dist/output.json", `--trace ${trace}`].join(" ");
		write(root, `packages/${name}/package.json`, json({ name: `bench-${name}`, private: true, scripts: { build: command }, dependencies: Object.fromEntries(deps.map((dep) => [`bench-${dep}`, "workspace:*"])) }));
		write(root, `packages/${name}/src.txt`, `${name}: baseline\n`);
	}
	if (runner !== "bsmr") return trace;
	write(root, ".bsmrroot", "\n");
	write(root, ".bsmrconfig", `[cells]\nroot = .\nprelude = prelude\ntoolchains = toolchains\nnone = none\n[cell_aliases]\nconfig = prelude\novr_config = prelude\nbuck = none\nfbcode = none\nfbcode_macros = none\nfbsource = none\nupstream = none\n[external_cells]\nprelude = disabled\n[buildfile]\nname = BUILD.bsmr\n[bsmr]\ndigest_algorithms = SHA256\n[bsmr_re_client]\naction_cache_address = ${remoteCache}\nengine_address = ${remoteCache}\ncas_address = ${remoteCache}\ntls = false\ninstance_name = bsmr-orchestration-v1\n[build]\nexecution_platforms = //platforms:default\n[parser]\ntarget_platform_detector_spec = target:root//...->//platforms:default target:toolchains//...->//platforms:default\n`);
	write(root, "toolchains/.bsmrconfig", "[buildfile]\nname = BUILD.bsmr\n");
	write(root, "toolchains/BUILD.bsmr", `load("@prelude//toolchains:genrule.bzl", "system_genrule_toolchain")\nsystem_genrule_toolchain(name = "genrule", visibility = ["PUBLIC"])\nexport_file(name = "node", src = "node", visibility = ["PUBLIC"])\n`);
	write(root, "platforms/defs.bzl", platform());
	write(root, "platforms/BUILD.bsmr", `load("@prelude//platforms:defs.bzl", "host_configuration")\nload(":defs.bzl", "cache_platform")\ncache_platform(name = "default", cpu = host_configuration.cpu, os = host_configuration.os, visibility = ["PUBLIC"])\n`);
	write(root, "BUILD.bsmr", buildManifest(trace));
	symlinkSync(join(repository, "prelude"), join(root, "prelude"), "dir");
	symlinkSync(process.execPath, join(root, "toolchains/node"), "file");
	return trace;
};
