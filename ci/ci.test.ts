//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies the generated CI workflow contract.

import assert from "node:assert/strict";
import { globSync, readFileSync } from "node:fs";
import test from "node:test";

import { command, runAction, type ScriptExec } from "@dedalus-labs/hollywood";

import { pullRequestFiles, rustAffected, rustAffectedForEvent } from "./affected.ts";
import { ci } from "./ci.ts";
import { docs } from "./docs.ts";
import { typescriptCache } from "./typescript/cache.ts";
import ociTools from "../prelude/oci/tools.json" with { type: "json" };
import registryFixture from "../test/oci/registry.json" with { type: "json" };
import { ociCache } from "./oci/cache.ts";
import { nativeCommands } from "./oci/native.ts";
import { releaseVersion } from "./release-version.ts";

const jobs = ci.jobs;

test("OCI fixture dependencies are installed before native qualification", () => {
	const steps = jobs.rust_self_host.steps;
	const installed = steps.findIndex((step) => step.name === "Install dependencies");
	const qualified = steps.findIndex((step) => "uses" in step && step.uses === "./.github/actions/oci/cache");
	assert.ok(installed >= 0 && installed < qualified);
	const installation = steps[installed];
	assert.ok(installation && "run" in installation);
	assert.deepEqual(installation.run, command({ file: "pnpm", args: ["install", "--frozen-lockfile", "--ignore-scripts"] }));
});

test("OCI qualification verifies its shared encoder pin before running mandatory image tests", () => {
	const steps = jobs.workflows.steps;
	const download = steps.findIndex((step) => step.name === "Download pinned OCI encoder");
	const verify = steps.findIndex((step) => step.name === "Verify OCI encoder");
	const execute = steps.findIndex((step) => step.name === "Check workflow source");
	assert.ok(download >= 0 && download < verify && verify < execute);
	const verification = steps[verify];
	assert.ok(verification && "uses" in verification);
	assert.deepEqual(verification.with, {
		path: "${{ format('{0}/oci-img', runner.temp) }}",
		expected: ociTools.img.assets["linux-amd64"].sha256,
	});
	const execution = steps[execute];
	assert.ok(execution && "env" in execution);
	assert.deepEqual(execution.env, { BSMR_OCI_IMG: "${{ format('{0}/oci-img', runner.temp) }}" });
	assert.match(ociTools.img.assets["linux-amd64"].url, /\/v0\.3\.22\/img_linux_amd64$/);
	assert.match(ociTools.img.assets["linux-amd64"].sha256, /^[0-9a-f]{64}$/);
});

test("native OCI qualification covers both architectures with pinned tools and bundled rules", async () => {
	const step = jobs.rust_self_host.steps.find((candidate) => "uses" in candidate && candidate.uses === "./.github/actions/oci/cache");
	assert.ok(step && "uses" in step);
	assert.equal("if" in step, false);
	assert.deepEqual(step.with, { binary: "target/debug/bsmr", img: "${{ format('{0}/oci-img', runner.temp) }}",
		umoci: "${{ format('{0}/oci-umoci', runner.temp) }}", runc: "${{ format('{0}/oci-runc', runner.temp) }}",
		"registry-archive": "${{ format('{0}/oci-registry.tar.gz', runner.temp) }}",
		platform: "${{ matrix.ociPlatform }}", evidence: "${{ format('{0}/oci-evidence', runner.temp) }}" });
	const download = jobs.rust_self_host.steps.find((candidate) => candidate.name === "Download pinned OCI encoder");
	assert.ok(download);
	assert.equal("if" in download, false);
	for (const tool of ["umoci", "runc", "registry fixture archive"]) {
		const steps = jobs.rust_self_host.steps;
		assert.ok(steps.findIndex((candidate) => candidate.name === `Download pinned ${tool}`)
			< steps.findIndex((candidate) => candidate.name === `Verify ${tool}`));
	}
	for (const entry of jobs.rust_self_host.strategy.matrix.include) {
		const key = entry.ociPlatform.replace("/", "-") as "linux-amd64" | "linux-arm64";
		assert.deepEqual({ url: entry.imgUrl, sha256: entry.imgSha256 }, ociTools.img.assets[key]);
		assert.deepEqual({ url: entry.umociUrl, sha256: entry.umociSha256 }, ociTools.umoci.assets[key]);
		assert.deepEqual({ url: entry.runcUrl, sha256: entry.runcSha256 }, ociTools.runc.assets[key]);
		assert.deepEqual({ url: entry.registryUrl, sha256: entry.registrySha256 }, registryFixture.assets[key]);
	}
	const failure = new Error("OCI qualification failed");
	await assert.rejects(runAction(ociCache, {
		with: { binary: "test path/bsmr", img: "test path/img", umoci: "umoci", runc: "runc", registryArchive: "registry.tar.gz", evidence: "evidence", platform: "linux/amd64" },
		exec: async (file, args) => {
			assert.equal(file, "node");
			assert.deepEqual(args, ["test/oci/cache.ts", "test path/bsmr", "test path/img", "prelude", "prelude/oci/operations.mjs",
				"--platform", "linux/amd64", "--engine-version", releaseVersion(process.cwd()), "--bundled-prelude"]);
			throw failure;
		},
		fs: { readText: async () => assert.fail("action delegates to the real fixture") },
		runner: { uidGid: "1000:1000" },
	}), failure);
});

test("native qualification cannot silently skip runtime tests or replace the bundled prelude", () => {
	const commands = nativeCommands({ binary: "bsmr", img: "img", umoci: "umoci", runc: "runc", platform: "linux/amd64",
		base: "base", lock: "lock", evidence: "evidence", registryArchive: "registry.tar.gz", bind: "10.0.0.2" });
	assert.equal(commands.length, 3);
	for (const command of commands) {
		assert.equal(command.file, "sudo");
		assert.deepEqual(command.args.slice(0, 2), ["-n", "env"]);
	}
	assert.deepEqual(commands[0]?.args, ["-n", "env", "BSMR_OCI_REQUIRE_NATIVE=1", "BSMR_OCI_PLATFORM=linux/amd64",
		"BSMR_OCI_RUN_BASE=base", "BSMR_OCI_UMOCI=umoci", "BSMR_OCI_RUNC=runc", process.execPath, "--test", "test/oci/run.test.mjs"]);
	assert.deepEqual(commands[1]?.args.slice(-5), ["--platform", "linux/amd64", "--engine-version", releaseVersion(process.cwd()), "--bundled-prelude"]);
	assert.ok(commands[2]?.args.includes("test/oci/registry.mjs"));
	assert.deepEqual(commands[2]?.args.slice(-9), ["--bind", "10.0.0.2", "--evidence", "evidence", "--platform", "linux/amd64",
		"--engine-version", releaseVersion(process.cwd()), "--bundled-prelude"]);
});

test("OCI action reaches bundled compact and native qualification after Go composition", async () => {
	const calls: { file: string; args: readonly string[] }[] = [];
	await runAction(ociCache, {
		with: { binary: "bsmr", img: "img", umoci: "umoci", runc: "runc", registryArchive: "registry.tar.gz", evidence: "evidence", platform: "linux/arm64" },
		exec: async (file, args) => { calls.push({ file, args }); return { exitCode: 0, stdout: "", stderr: "" }; },
		fs: { readText: async () => assert.fail("action delegates to real fixtures") }, runner: { uidGid: "1000:1000" },
	});
	assert.equal(calls.length, 3);
	assert.deepEqual(calls[1], { file: "node", args: ["test/oci/graph.ts", "bsmr", "img", "prelude", "--platform", "linux/arm64",
		"--engine-version", releaseVersion(process.cwd()), "--bundled-prelude", "--artifacts", "evidence"] });
	assert.deepEqual(calls[2], { file: "node", args: ["ci/oci/native.ts", "bsmr", "img", "umoci", "runc", "registry.tar.gz", "linux/arm64", "evidence"] });
});

test("TypeScript cache uses its nested action route and propagates failures", async () => {
	const step = jobs.rust_self_host?.steps.find(
		(step) => "uses" in step && step.uses === "./.github/actions/typescript/cache",
	);
	assert.ok(step && "uses" in step);
	assert.deepEqual(step.with, { binary: "target/debug/bsmr" });
	const failure = new Error("cache verification failed");
	await assert.rejects(runAction(typescriptCache, {
		with: { binary: "test path/bsmr" },
		exec: async (file, args) => {
			assert.equal(file, "node");
			assert.deepEqual(args, ["test/typescript/cache.ts", "test path/bsmr"]);
			throw failure;
		},
		fs: { readText: async () => assert.fail("action must delegate to the fixture") },
		runner: { uidGid: "1000:1000" },
	}), failure);
});
const rustLanes = [
	"rust_audit",
	"rust_quality",
	"rust_tests",
	"rust_self_host",
	"rust_sandbox",
] as const;
const trustedCiRun =
	"github.repository == 'dedalus-labs/bsmr' && (github.event_name != 'pull_request' || github.event.pull_request.head.repo.full_name == github.repository)";

function unsafeScript(name: string): string {
	const step = jobs.rust_sandbox?.steps.find((candidate) => candidate.name === name);
	assert.ok(step !== undefined && "run" in step);
	assert.equal(step.run.kind, "unsafe-shell");
	return step.run.kind === "unsafe-shell" ? step.run.script : "";
}

test("Rust aggregates completed runs without surviving workflow cancellation", () => {
	assert.equal(jobs.rust?.name, "Rust");
	assert.equal(jobs.rust?.["runs-on"], "ubuntu-24.04");
	assert.equal(jobs.rust?.if, `\${{ !cancelled() && ${trustedCiRun} }}`);
	assert.deepEqual(jobs.rust?.needs, ["affected", ...rustLanes]);
	const steps = jobs.rust?.steps ?? [];
	assert.deepEqual(steps.map((step) => ("run" in step ? step.run : null)), [
		command({ file: "true", args: [] }),
		command({ file: "false", args: [] }),
	]);
	assert.match(steps[1]?.if ?? "", /!\(needs\.affected\.result == 'success'/);
});

test("Rust lanes run only for trusted affected changes", () => {
	assert.equal(jobs.affected?.["runs-on"], "ubuntu-24.04");
	assert.equal(jobs.affected?.if, `\${{ ${trustedCiRun} }}`);
	assert.equal(jobs.affected?.outputs?.rust, "${{ steps.check.outputs.rust }}");
	const checkout = jobs.affected?.steps[0];
	assert.ok(checkout !== undefined && "with" in checkout);
	assert.deepEqual(checkout.with, {
		"persist-credentials": false,
		"fetch-depth": 0,
		"sparse-checkout": ".github/actions/ci/rust-affected",
	});
	const check = jobs.affected?.steps.at(-1);
	assert.ok(check !== undefined && "uses" in check);
	assert.equal(check.uses, "./.github/actions/ci/rust-affected");
	assert.equal(check.with?.["merge-group-base-sha"], "${{ github.event.merge_group.base_sha }}");
	assert.equal(check.with?.["head-sha"], "${{ github.event.pull_request.head.sha || github.sha }}");
	for (const id of rustLanes) {
		assert.equal(jobs[id]?.needs, "affected");
		assert.equal(
			jobs[id]?.if,
			`\${{ ${trustedCiRun} && needs.affected.outputs.rust == 'true' }}`,
		);
	}
});

test("Rust affected paths fail closed", () => {
	assert.equal(rustAffected([".github/dependabot.yml"]), false);
	assert.equal(rustAffected(["docs/users/getting_started.md"]), false);
	assert.equal(rustAffected(["README.md", ".github/CODEOWNERS"]), false);
	assert.equal(rustAffected(["Cargo.lock"]), true);
	assert.equal(rustAffected(["app/bsmr/src/main.rs"]), true);
	assert.equal(rustAffected(["app/bsmr_core/src/pattern/target_pattern.md"]), true);
	assert.equal(rustAffected(["prelude/rust/rust_binary.bzl"]), true);
	assert.equal(rustAffected(["ci/ci.ts"]), true);
	assert.equal(rustAffected([]), true);
});

test("Pull request paths preserve both sides of a rename", async () => {
	const base = "a".repeat(40);
	const head = "b".repeat(40);
	const mergeBase = "c".repeat(40);
	const calls: string[][] = [];
	const exec: ScriptExec = async (file, args) => {
		calls.push([file, ...args]);
		return {
			exitCode: 0,
			stderr: "",
			stdout:
				args[0] === "merge-base"
					? mergeBase
					: "app/bsmr/src/renamed.rs\0docs/renamed.md\0",
		};
	};
	const files = await pullRequestFiles(exec, base, head);
	assert.equal(rustAffected(files), true);
	assert.deepEqual(calls, [
		["git", "merge-base", base, head],
		["git", "diff", "--name-only", "--no-renames", "-z", mergeBase, head],
	]);
});

test("Pull request paths require immutable commit IDs", async () => {
	const fail: ScriptExec = async () => assert.fail("exec must not run");
	await assert.rejects(pullRequestFiles(fail, "main", "b".repeat(40)), /base SHA/);
});

test("Merge-group paths classify the exact candidate against its base", async () => {
	const base = "a".repeat(40);
	const head = "b".repeat(40);
	const calls: string[][] = [];
	const exec: ScriptExec = async (file, args) => {
		calls.push([file, ...args]);
		return { exitCode: 0, stderr: "", stdout: "docs/users/getting_started.md\0" };
	};
	assert.equal(
		await rustAffectedForEvent(exec, {
			eventName: "merge_group",
			baseSha: "",
			mergeGroupBaseSha: base,
			headSha: head,
		}),
		false,
	);
	assert.deepEqual(calls, [["git", "diff", "--name-only", "--no-renames", "-z", base, head]]);
});

test("Rust compilation uses sized Blacksmith runners", () => {
	assert.equal(jobs.rust_quality?.["runs-on"], "blacksmith-8vcpu-ubuntu-2404");
	for (const id of ["rust_tests", "rust_self_host"] as const) {
		assert.equal(jobs[id].strategy?.["fail-fast"], false);
		const matrix = jobs[id].strategy?.matrix;
		assert.ok(matrix !== undefined && typeof matrix === "object");
		assert.deepEqual(matrix.include?.map(({ architecture, testRunner, selfHostRunner }) => ({ architecture, testRunner, selfHostRunner })), [
			{ architecture: "x64", testRunner: "blacksmith-16vcpu-ubuntu-2404", selfHostRunner: "blacksmith-8vcpu-ubuntu-2404" },
			{ architecture: "arm64", testRunner: "blacksmith-16vcpu-ubuntu-2404-arm", selfHostRunner: "blacksmith-8vcpu-ubuntu-2404-arm" },
		]);
	}
	assert.equal(jobs.rust_tests?.["runs-on"], "${{ matrix.testRunner }}");
	assert.equal(jobs.rust_self_host?.["runs-on"], "${{ matrix.selfHostRunner }}");
	assert.equal(jobs.rust_sandbox?.["runs-on"], "ubuntu-24.04");
	const kvm = unsafeScript("Initialize nested KVM");
	assert.match(kvm, /if ! test -c \/dev\/kvm/);
	assert.match(kvm, /setfacl -m "u:\$\(id -un\):rw" \/dev\/kvm/);
	assert.match(kvm, /sudo modprobe kvm_intel/);
	assert.match(kvm, /sudo modprobe kvm_amd/);
	assert.match(kvm, /test -w \/dev\/kvm/);
	assert.match(
		unsafeScript("Build sandbox components"),
		/sandbox_probe\.rs -o "\$RUNNER_TEMP\/sandbox-probe"/,
	);
	assert.ok(
		jobs.rust_self_host?.steps.some(
			(step) =>
				"run" in step &&
				step.run.kind === "command" &&
				step.run.args.includes("--lint-starlark-only"),
		),
	);
});

test("Firecracker cleanup is gated by acquired resources", () => {
	const steps = jobs.rust_sandbox?.steps ?? [];
	const stop = steps.find((step) => step.name === "Stop isolated launcher");
	const verify = steps.find(
		(step) => step.name === "Verify complete sandbox cleanup",
	);
	const remove = steps.find((step) => step.name === "Remove host sentinel");

	assert.match(stop?.if ?? "", /steps\.sandbox_launcher\.outputs\.started == 'true'/);
	assert.equal(stop?.if, verify?.if);
	assert.match(remove?.if ?? "", /steps\.host_sentinel\.outputs\.created == 'true'/);
});

test("Firecracker installs its bundle beneath the immutable system prefix", () => {
	const steps = jobs.rust_sandbox?.steps ?? [];
	const assemble = steps.find(
		(step) => step.name === "Assemble pinned execution bundle",
	);
	const launcher = steps.find((step) => step.name === "Start isolated launcher");

	assert.ok(assemble !== undefined && "run" in assemble);
	assert.equal(assemble.run.kind, "unsafe-shell");
	assert.match(unsafeScript("Assemble pinned execution bundle"), /\/usr\/local\/share\/bsmr\/firecracker/);
	assert.ok(launcher !== undefined && "run" in launcher);
	assert.equal(launcher.run.kind, "unsafe-shell");
	assert.match(unsafeScript("Start isolated launcher"), /\/usr\/local\/share\/bsmr\/firecracker/);
});

test("self-hosting keeps the CLI reference derived from clap", () => {
	assert.ok(
		jobs.rust_self_host?.steps.some(
			(step) =>
				"uses" in step && step.uses === "./.github/actions/ci/cli-reference",
		),
	);
});

test("workflow checks retain the provenance boundary", () => {
	const workflowCheckout = jobs.workflows?.steps[0];
	assert.ok(workflowCheckout !== undefined && "with" in workflowCheckout);
	assert.ok("fetch-depth" in workflowCheckout.with);
	assert.equal(workflowCheckout.with["fetch-depth"], 0);
});

test("public workflows cannot receive repository administration credentials", () => {
	const workflows = globSync(".github/workflows/*.yml")
		.map((path) => readFileSync(path, "utf8"))
		.join("\n");

	assert.doesNotMatch(workflows, /CIND_BOT_APP_PRIVATE_KEY/);
	assert.doesNotMatch(workflows, /permission-administration:\s*write/);
});

test("each Rust profile has one trusted cache writer", () => {
	for (const id of ["rust_quality", "rust_tests", "rust_self_host"] as const) {
		const cache = jobs[id].steps.find(
			(step) => "uses" in step && step.uses.startsWith("Swatinem/rust-cache@"),
		);

		assert.ok(cache !== undefined);
		assert.ok("with" in cache);
		assert.ok("shared-key" in cache.with);
		assert.equal(cache.with["shared-key"], id === "rust_self_host" ? "engine" : "rust");
		assert.equal(
			cache.with["save-if"],
			id !== "rust_quality"
				? "${{ github.event_name == 'push' && github.ref == 'refs/heads/main' }}"
				: false,
		);
	}
});

test("engine compiler settings cannot change consumer qualification", () => {
	assert.ok(!Object.hasOwn(jobs.rust_self_host.env, "CARGO_PROFILE_DEV_DEBUG"));
	for (const step of jobs.rust_self_host.steps) {
		const engine = step.name === "Build BSMR" || step.name === "Restore Rust cache";
		const environment = "env" in step ? step.env : undefined;
		const profile = Object.entries(environment ?? {}).filter(([key]) => key.startsWith("CARGO_PROFILE_"));
		assert.deepEqual(profile, engine ? [["CARGO_PROFILE_DEV_DEBUG", "0"]] : []);
	}
});

test("the Cargo planner has a main-owned cache for its compiler and output directory", () => {
	const steps = jobs.rust_self_host.steps;
	const cacheIndex = steps.findIndex((step) => step.name === "Restore Cargo planner cache");
	assert.ok(cacheIndex > steps.findIndex((step) => step.name === "Install Cargo planner compiler"));
	const cache = steps[cacheIndex];
	assert.deepEqual(cache, {
		name: "Restore Cargo planner cache",
		uses: "Swatinem/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32",
		env: { RUSTUP_TOOLCHAIN: "1.98.0" },
		with: {
			"prefix-key": "bsmr-v1", "shared-key": "planner", workspaces: "tools/cargo -> target",
			"save-if": "${{ github.event_name == 'push' && github.ref == 'refs/heads/main' }}",
		},
	});
	const build = steps[cacheIndex + 1];
	const install = steps[cacheIndex + 2];
	assert.ok(build && "run" in build && install && "run" in install);
	assert.deepEqual(build.run, command({
		file: "rustup",
		args: ["run", "1.98.0", "cargo", "build", "--locked", "--manifest-path", "tools/cargo/Cargo.toml", "--target-dir", "tools/cargo/target", "-j", "2"],
	}));
	assert.deepEqual(install.run, command({
		file: "cp", args: ["tools/cargo/target/debug/bsmr-cargo", "target/debug/bsmr-cargo"],
	}));
});

test("docs deploy only from a trusted main build", () => {
	assert.deepEqual(docs.on.pull_request, {
		branches: ["main"],
		paths: [
			".github/workflows/docs.yml",
			"ci/docs.test.ts",
			"ci/docs.ts",
			"docs/**",
			"mkdocs.yml",
			"README.md",
		],
	});
	assert.equal(docs.jobs.build?.if, `\${{ ${trustedCiRun} }}`);
	assert.equal(
		docs.jobs.deploy?.if,
		"github.event_name == 'push' && github.ref == 'refs/heads/main'",
	);
	assert.equal(docs.jobs.deploy?.needs, "build");
	assert.deepEqual(docs.jobs.deploy?.permissions, {
		pages: "write",
		"id-token": "write",
	});
});
