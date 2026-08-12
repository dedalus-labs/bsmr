//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Generates equivalent 25-library, eight-binary Go DAGs for BSMR and Bazel.

import { mkdirSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

export type Runner = "bazel" | "bsmr";
export type GoBuildMode = "cgo" | "pure";

interface Package {
	dependencies: readonly string[];
	name: string;
	path: string;
	type: "binary" | "library";
}

const cores = Array.from({ length: 8 }, (_, index) => `core${index}`);
const libraries = Array.from({ length: 16 }, (_, index) => `lib${index}`);
const applications = Array.from({ length: 8 }, (_, index) => `app${index}`);

export const packages: readonly Package[] = [
	{ dependencies: [], name: "shared", path: "shared", type: "library" },
	...cores.map((name) => ({ dependencies: ["shared"], name, path: `core/${name}`, type: "library" as const })),
	...libraries.map((name, index) => ({
		dependencies: [cores[index % cores.length]!, cores[(index + 1) % cores.length]!],
		name,
		path: `lib/${name}`,
		type: "library" as const,
	})),
	...applications.map((name, index) => ({
		dependencies: [libraries[index * 2]!, libraries[index * 2 + 1]!],
		name,
		path: `cmd/${name}`,
		type: "binary" as const,
	})),
];

export const applicationTargets = applications.map((name) => `//cmd/${name}:bin`);

/** Writes one generated fixture file beneath the benchmark root. */
const write = (root: string, path: string, contents: string): void => {
	const destination = join(root, path);
	mkdirSync(dirname(destination), { recursive: true });
	writeFileSync(destination, contents);
};

/** Returns the stable import path for one logical package. */
const importPath = (module: string, name: string): string => {
	const packageDefinition = packages.find((candidate) => candidate.name === name);
	if (!packageDefinition) throw new Error(`unknown package: ${name}`);
	return `${module}/${packageDefinition.path}`;
};

/** Returns the stable build label for one logical package. */
const target = (name: string): string => {
	const packageDefinition = packages.find((candidate) => candidate.name === name);
	if (!packageDefinition) throw new Error(`unknown package: ${name}`);
	return `//${packageDefinition.path}:${packageDefinition.type === "binary" ? "bin" : "lib"}`;
};

/** Renders a library whose exported shape and private implementation can change independently. */
const sharedSource = (mode: GoBuildMode, abi: number, seed: number): string => mode === "cgo" ? `package shared

/*
#include "shared.h"
*/
import "C"

const ABI = ${abi}

//go:noinline
func Value() int { return int(C.bsmr_private_seed()) }
` : `package shared

const ABI = ${abi}

//go:noinline
func Value() int { return privateSeed() }

//go:noinline
func privateSeed() int { return ${seed} }
`;

/** Renders the native implementation used by the cgo benchmark mode. */
const sharedCSource = (seed: number): string => `#include "shared.h"

int bsmr_private_seed(void) { return ${seed}; }
`;

/** Renders the package-local C API used by the cgo benchmark mode. */
const sharedHeader = (): string => `#ifndef BSMR_GO_BENCH_SHARED_H
#define BSMR_GO_BENCH_SHARED_H

int bsmr_private_seed(void);

#endif
`;

/** Renders one ordinary library in the shared benchmark graph. */
const librarySource = (module: string, packageDefinition: Package, index: number): string => {
	const imports = packageDefinition.dependencies
		.map((dependency) => `\t${dependency} "${importPath(module, dependency)}"`)
		.join("\n");
	const values = packageDefinition.dependencies.map((dependency) => `${dependency}.Value()`).join(" + ");
	const abi = packageDefinition.dependencies.includes("shared") ? " + shared.ABI" : "";
	return `package ${packageDefinition.name}

import (
${imports}
)

//go:noinline
func Value() int { return ${values}${abi} + ${index} }
`;
};

/** Renders one executable used as a correctness oracle. */
const binarySource = (module: string, packageDefinition: Package, index: number): string => {
	const imports = packageDefinition.dependencies
		.map((dependency) => `\t${dependency} "${importPath(module, dependency)}"`)
		.join("\n");
	const values = packageDefinition.dependencies.map((dependency) => `${dependency}.Value()`).join(" + ");
	return `package main

import (
\t"fmt"
${imports}
)

func main() { fmt.Printf("${packageDefinition.name}:%d\\n", ${values} + ${index}) }
`;
};

/** Renders one Bazel rules_go package declaration. */
const bazelManifest = (module: string, packageDefinition: Package, mode: GoBuildMode): string => {
	const rule = packageDefinition.type === "binary" ? "go_binary" : "go_library";
	const sources = packageDefinition.name === "shared" && mode === "cgo"
		? ["shared.c", "shared.go", "shared.h"]
		: [packageDefinition.type === "binary" ? "main.go" : `${packageDefinition.name}.go`];
	const sourceAttribute = sources.map((source) => `        "${source}",`).join("\n");
	const dependencies = packageDefinition.dependencies.map((dependency) => `        "${target(dependency)}",`).join("\n");
	const importAttribute = packageDefinition.type === "library"
		? `    importpath = "${module}/${packageDefinition.path}",\n`
		: "";
	const pureAttribute = packageDefinition.type === "binary" ? `    pure = "${mode === "cgo" ? "off" : "on"}",\n` : "";
	const cgoAttribute = packageDefinition.name === "shared" && mode === "cgo" ? "    cgo = True,\n" : "";
	const depsAttribute = dependencies === "" ? "" : `    deps = [\n${dependencies}\n    ],\n`;
	return `load("@rules_go//go:def.bzl", "${rule}")

${rule}(
    name = "${packageDefinition.type === "binary" ? "bin" : "lib"}",
    srcs = [
${sourceAttribute}
    ],
${importAttribute}${pureAttribute}${cgoAttribute}${depsAttribute}    visibility = ["//visibility:public"],
)
`;
};

/** Renders the minimal BSMR configuration needed for direct Go actions and REAPI caching. */
const bsmrConfig = (remoteCache: string, instance: string): string => `[cells]
root = .
prelude = prelude
toolchains = toolchains
none = none
[cell_aliases]
config = prelude
ovr_config = prelude
buck = none
[external_cells]
prelude = disabled
[buildfile]
name = BUILD.bsmr
[bsmr]
default_allow_cache_upload = true
digest_algorithms = SHA256
file_watcher = fs_hash_crawler
[bsmr_re_client]
action_cache_address = ${remoteCache}
engine_address = ${remoteCache}
cas_address = ${remoteCache}
tls = false
instance_name = ${instance}
[build]
execution_platforms = root//platforms:default
[parser]
target_platform_detector_spec = target:root//...->root//platforms:default target:prelude//...->root//platforms:default target:toolchains//...->root//platforms:default
`;

/** Renders a local execution platform backed by the configured REAPI action cache. */
const bsmrPlatform = (): string => `load("@prelude//cfg/exec_platform:marker.bzl", "get_exec_platform_marker")

def _impl(ctx):
    constraints = dict()
    constraints.update(ctx.attrs.cpu[ConfigurationInfo].constraints)
    constraints.update(ctx.attrs.os[ConfigurationInfo].constraints)
    configuration = ConfigurationInfo(constraints = constraints, values = {})
    executor = CommandExecutorConfig(
        local_enabled = True,
        remote_enabled = False,
        remote_cache_enabled = True,
        allow_cache_uploads = True,
        use_windows_path_separators = host_info().os.is_windows,
    )
    platform = ExecutionPlatformInfo(label = ctx.label.raw_target(), configuration = configuration, executor_config = executor)
    return [DefaultInfo(), platform, PlatformInfo(label = str(ctx.label.raw_target()), configuration = configuration), ExecutionPlatformRegistrationInfo(platforms = [platform], exec_marker_constraint = get_exec_platform_marker())]

cache_platform = rule(impl = _impl, attrs = {"cpu": attrs.dep(providers = [ConfigurationInfo]), "os": attrs.dep(providers = [ConfigurationInfo])})
`;

/** Creates one isolated, deterministic runner fixture. */
export const generateFixture = (
	root: string,
	runner: Runner,
	repository: string,
	remoteCache: string,
	instance: string,
	moduleSuffix: string,
	mode: GoBuildMode,
): void => {
	if (/\s/.test(root)) throw new Error(`benchmark path must not contain whitespace: ${root}`);
	rmSync(root, { force: true, recursive: true });
	mkdirSync(root, { recursive: true });
	const module = `example.com/bsmr-go-bench/${moduleSuffix}`;
	write(root, "go.mod", `module ${module}\n\ngo 1.26.0\n`);
	write(root, "README.md", "generated by benchmarks/go/fixture.ts\n");
	write(root, "cmd/prime/main.go", "package main\n\nfunc main() {}\n");
	for (const [index, packageDefinition] of packages.entries()) {
		const source = packageDefinition.name === "shared"
			? sharedSource(mode, 1, 1)
			: packageDefinition.type === "library"
				? librarySource(module, packageDefinition, index)
				: binarySource(module, packageDefinition, index);
		write(root, `${packageDefinition.path}/${packageDefinition.type === "binary" ? "main" : packageDefinition.name}.go`, source);
		if (packageDefinition.name === "shared" && mode === "cgo") {
			write(root, "shared/shared.c", sharedCSource(1));
			write(root, "shared/shared.h", sharedHeader());
		}
		if (runner === "bazel") write(root, `${packageDefinition.path}/BUILD.bazel`, bazelManifest(module, packageDefinition, mode));
	}
	if (runner === "bazel") {
		write(root, ".bazelversion", "9.2.0\n");
		write(root, "MODULE.bazel", `module(name = "bsmr_go_bench")\nbazel_dep(name = "rules_go", version = "0.62.0")\ngo_sdk = use_extension("@rules_go//go:extensions.bzl", "go_sdk")\ngo_sdk.host()\n`);
		write(root, "cmd/prime/BUILD.bazel", `load("@rules_go//go:def.bzl", "go_binary")\ngo_binary(name = "bin", srcs = ["main.go"], pure = "${mode === "cgo" ? "off" : "on"}", visibility = ["//visibility:public"])\n`);
		return;
	}
	write(root, ".bsmrroot", "\n");
	write(root, ".bsmrconfig", bsmrConfig(remoteCache, instance));
	write(root, "BUILD.bsmr", "# Native Go targets are generated by `bsmr go sync`.\n");
	write(root, "platforms/defs.bzl", bsmrPlatform());
	write(root, "platforms/BUILD.bsmr", `load("@prelude//platforms:defs.bzl", "host_configuration")\nload(":defs.bzl", "cache_platform")\ncache_platform(name = "default", cpu = host_configuration.cpu, os = host_configuration.os, visibility = ["PUBLIC"])\n`);
	symlinkSync(join(repository, "prelude"), join(root, "prelude"), "dir");
};

/** Changes only private implementation data in the root library. */
export const setPrivateSeed = (root: string, mode: GoBuildMode, seed: number): void => {
	if (mode === "cgo") write(root, "shared/shared.c", sharedCSource(seed));
	else write(root, "shared/shared.go", sharedSource(mode, 1, seed));
};

/** Changes exported API data in the root library. */
export const setExportedAbi = (root: string, mode: GoBuildMode, abi: number, seed: number): void => {
	write(root, "shared/shared.go", sharedSource(mode, abi, seed));
	if (mode === "cgo") write(root, "shared/shared.c", sharedCSource(seed));
};

/** Changes an input intentionally outside both build graphs. */
export const setDocumentation = (root: string, token: string): void => write(root, "README.md", `Go benchmark: ${token}\n`);
