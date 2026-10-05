//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Qualifies OCI rules consuming native Go outputs with shared cache restoration.

import assert from "node:assert/strict";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { timedExec } from "./exec.ts";
import artifactRule from "./fixtures/artifact.bzl";
import probeSource from "./fixtures/cache/cmd/probe/main.go";
import goModule from "./fixtures/cache/go.mod";
import imagesBuild from "./fixtures/cache/images/recipe.bsmr";
import missingBuild from "./fixtures/cache/images/missing/recipe.bsmr";
import invalidSource from "./fixtures/cache/invalid.go";
import toolsBuild from "./fixtures/cache/tools/recipe.bsmr";
import verifyScript from "./verify.py";

const { positionals, values } = parseArgs({ allowPositionals: true, options: {
	platform: { type: "string", default: "linux/arm64" },
	"engine-version": { type: "string", default: "0.0.9" },
	"bundled-prelude": { type: "boolean", default: false },
} });
const [binary, img, prelude, operations, goVersion = "1.26.7"] = positionals;
const platform = values.platform;
assert.ok(platform === "linux/arm64" || platform === "linux/amd64", "test platform must be linux/arm64 or linux/amd64");
const architecture = platform === "linux/amd64" ? "amd64" : "arm64";
const cpu = architecture === "amd64" ? "x86_64" : "arm64";
const otherPlatform = architecture === "amd64" ? "linux/arm64" : "linux/amd64";
assert.ok(binary && img && prelude && operations, "pass BSMR, img, checkout prelude, and OCI operations.mjs");
assert.ok(positionals.length <= 5, "only an optional Go version may follow OCI operations.mjs");
const operationsPath = realpathSync(operations);
assert.equal(operationsPath, realpathSync(join(prelude, "oci/operations.mjs")), "operations must belong to the selected source prelude");
const executable = resolve(binary);
const run = timedExec(300);
const resumed = process.env["BSMR_OCI_TEST_RESUME"];
const root = realpathSync(resumed ?? mkdtempSync(join(tmpdir(), "bsmr-oci-cache-")));
assert.ok(basename(root).startsWith("bsmr-oci-cache-"), "resume must name an owned OCI fixture root");
const cwd = join(root, "workspace");
if (resumed === undefined) {
	mkdirSync(join(root, "cache"));
	assert.deepEqual(readdirSync(join(root, "cache")), [], "cold action cache must be explicitly empty");
}
if (resumed !== undefined) {
	assert.match(readFileSync(join(cwd, "go.mod"), "utf8"), /^module example\.com\/oci-probe\n/);
	assert.ok(existsSync(join(cwd, ".bsmr-go-toolchain.json")) && existsSync(join(cwd, ".bsmr-go-sdk")) && existsSync(join(root, "cache")));
	if (values["bundled-prelude"]) assert.equal(existsSync(join(cwd, ".bsmr.local")), false, "bundled qualification must not resume a source overlay");
}
const workspaces: string[] = [];
const env = { BSMR_LOCAL_CACHE_DIR: join(root, "cache") };
const options = { cwd, env };
const evidenceRoot = process.env["BSMR_OCI_TEST_EVIDENCE"];
if (evidenceRoot !== undefined) mkdirSync(resolve(evidenceRoot), { recursive: true });
const evidence = evidenceRoot === undefined ? undefined : mkdtempSync(join(resolve(evidenceRoot), "cache-"));
let complete = false;
type Action = { identity: string; reproducer: { executor: string } };
type Image = {
	digest: string;
	config: { os: string; architecture: string; config: { Entrypoint: string[]; Cmd?: string[]; Env: string[]; User: string; WorkingDir: string } };
	layers: { digest: string; size: number }[];
	files: Record<string, { type: string; mode: number; uid: number; gid: number; target?: string; sha256?: string }>;
};

/** Verify the real exported layout using a separate standard-library tar reader. */
async function verify(path: string): Promise<Image> {
	return JSON.parse((await run("python3", ["-c", verifyScript, path], options)).stdout);
}

/** Build one image and retain this invocation's machine-readable cache evidence. */
async function build(directory: string, phase: string) {
	const context = { ...options, cwd: directory };
	const report = join(root, `${phase}.json`);
	await run(executable, ["build", "//images:layout", "--build-report", report,
		"--build-report-options", "include-artifact-hash-information", "--console", "simple", "-v=1,stderr,full_failed_command",
		"--target-platforms", "//images:linux_target", "-c", "go.link_mode=internal"], context).catch((error) => {
		if (evidence !== undefined) {
			if (existsSync(report)) cpSync(report, join(evidence, `${phase}-failed-build.json`));
			writeFileSync(join(evidence, `${phase}-stderr.log`), error.stderr ?? error.message);
		}
		throw error;
	});
	const result = JSON.parse(readFileSync(report, "utf8"));
	assert.equal(result.success, true);
	const output = result.results["root//images:layout"].outputs.DEFAULT[0];
	const log = await run(executable, ["log", "what-ran", "--trace-id", result.trace_id,
		"--format", "json", "--no-remote"], context);
	const actions: Action[] = log.stdout.trim() === "" ? [] : log.stdout.trim().split("\n").map((line) => JSON.parse(line));
	const local = actions.filter(({ reproducer }) => reproducer.executor === "Local");
	const cached = actions.filter(({ reproducer }) => reproducer.executor === "Cache");
	assert.equal(actions.length, local.length + cached.length);
	const ociLog = await run(executable, ["log", "what-ran", "--trace-id", result.trace_id,
		"--format", "json", "--no-remote", "--filter-category", "^oci_(layer|image|layout)$"], context);
	const ociActions: Action[] = ociLog.stdout.trim() === "" ? [] : ociLog.stdout.trim().split("\n").map((line) => JSON.parse(line));
	const ociLocal = ociActions.filter(({ reproducer }) => reproducer.executor === "Local");
	if (evidence !== undefined) {
		cpSync(report, join(evidence, `${phase}-build.json`));
		writeFileSync(join(evidence, `${phase}-actions.json`), JSON.stringify(actions, null, 2) + "\n");
	}
	const image = await verify(resolve(directory, output));
	if (evidence !== undefined) {
		writeFileSync(join(evidence, `${phase}-image.json`), JSON.stringify(image, null, 2) + "\n");
	}
	assert.equal(image.config.os, "linux");
	assert.equal(image.config.architecture, architecture);
	assert.deepEqual(image.config.config.Entrypoint, ["/app/current"]);
	assert.equal(image.config.config.User, "65532:65532");
	assert.equal(image.config.config.WorkingDir, "/app");
	const probe = image.files["app/probe"];
	const message = image.files["app/message.txt"];
	const current = image.files["app/current"];
	assert.ok(probe && message && current, "export must contain every declared runtime entry");
	assert.equal(probe.type, "file");
	assert.equal(probe.mode, 0o755);
	assert.equal(probe.uid, 0);
	assert.equal(message.mode, 0o644);
	assert.equal(current.target, "probe");
	process.stdout.write(`${JSON.stringify({ phase, trace: result.trace_id, local: local.length, cached: cached.length,
		digest: image.digest, localActions: local.map(({ identity }) => identity) })}\n`);
	return { image, actions, local, cached, ociLocal, output: resolve(directory, output) };
}

/** Restart a workspace without dropping its independent persistent cache. */
async function clean(directory: string) {
	await run(executable, ["clean"], { ...options, cwd: directory });
}

/** Reject corrupt or absent blob bytes through the actual imported-layout operation. */
async function corruptInputs(output: string, expected: string) {
	const broken = join(root, "broken");
	cpSync(output, broken, { recursive: true });
	const descriptor = JSON.parse(readFileSync(join(broken, "index.json"), "utf8")).manifests[0];
	const manifest = JSON.parse(readFileSync(join(broken, "blobs/sha256", descriptor.digest.split(":")[1]), "utf8"));
	const layer = join(broken, "blobs/sha256", manifest.layers[0].digest.split(":")[1]);
	const original = readFileSync(layer);
	const corrupt = Buffer.from(original);
	const finalByte = corrupt.at(-1);
	assert.ok(finalByte !== undefined, "encoded layer must contain bytes");
	corrupt[corrupt.length - 1] = finalByte ^ 1;
	const imported = join(root, "imported.json");
	const helper = values["bundled-prelude"] ? operationsPath : join(cwd, "prelude/oci/operations.mjs");
	const args = [helper, "import", "--layout", broken, "--platform", platform,
		"--manifest", join(root, "imported-manifest.json"), "--config", join(root, "imported-config.json"), "--descriptor", imported];
	writeFileSync(layer, corrupt);
	await assert.rejects(run(process.execPath, args, options), /digest|hash|checksum/i);
	assert.equal(existsSync(imported), false, "corrupt content must not publish an import descriptor");
	rmSync(layer);
	await assert.rejects(run(process.execPath, args, options), /missing|ENOENT|not found/i);
	assert.equal(existsSync(imported), false, "missing content must not publish an import descriptor");
	writeFileSync(layer, original);
	await run(process.execPath, args, options);
	assert.equal(JSON.parse(readFileSync(imported, "utf8")).digest, expected);
}

try {
	assert.equal((await run(executable, ["--version"], { env })).stdout.trim(), `bsmr ${values["engine-version"]}`);
	mkdirSync(join(cwd, "cmd/probe"), { recursive: true });
	writeFileSync(join(cwd, "go.mod"), goModule);
	writeFileSync(join(cwd, "cmd/probe/main.go"), probeSource);
	if (resumed === undefined) {
		await run(executable, ["init"], options);
		workspaces.push(cwd);
		await run(executable, ["go", "toolchain", "--version", goVersion], options);
		await run(executable, ["go", "sync"], options);
	} else {
		workspaces.push(cwd);
	}
	await run(executable, ["kill"], options);
	if (!values["bundled-prelude"]) {
		cpSync(resolve(prelude), join(cwd, "prelude"), { recursive: true });
		writeFileSync(join(cwd, ".bsmr.local"), "[external_cells]\nprelude = disabled\n");
	}
	mkdirSync(join(cwd, "tools"), { recursive: true });
	cpSync(resolve(img), join(cwd, "tools/img"));
	cpSync(process.execPath, join(cwd, "tools/node"));
	writeFileSync(join(cwd, "tools/defs.bzl"), artifactRule);
	writeFileSync(join(cwd, "tools/BUILD.bsmr"), toolsBuild);
	mkdirSync(join(cwd, "images"), { recursive: true });
	writeFileSync(join(cwd, "images/message.txt"), "fixture\n");
	writeFileSync(join(cwd, "images/BUILD.bsmr"), imagesBuild);
	const fixture = join(cwd, "images/fixture.json");
	const imageConfig = { platform, cpu, otherPlatform, env: { OCI_PROBE: "original", PATH: "/app" } };
	writeFileSync(fixture, JSON.stringify(imageConfig));
	mkdirSync(join(cwd, "images/missing"), { recursive: true });
	writeFileSync(join(cwd, "images/missing/BUILD.bsmr"), missingBuild);
	writeFileSync(join(cwd, "images/missing/fixture.json"), JSON.stringify({ platform }));
	for (const [target, error] of [["//images:wrong_platform", /platform mismatch/i], ["//images:unsafe_path", /normalized/i], ["//images/missing:layer", /missing\.txt/i]] as const) {
		await assert.rejects(run(executable, ["build", target, "--target-platforms", "//images:linux_target", "--console", "none"], options), error);
	}
	const cold = await build(cwd, resumed === undefined ? "cold" : "seed-restored");
	if (resumed === undefined) {
		assert.equal(cold.cached.length, 0, "fresh qualification must not borrow cached compiler or packaging actions");
		assert.ok(cold.local.some(({ identity }) => identity.includes("go_compile")));
		assert.equal(cold.ociLocal.length, 3);
	} else {
		assert.equal(cold.local.length, cold.ociLocal.length, "preserved compiler outputs must restore without recompilation");
	}
	assert.equal((await build(cwd, "warm")).actions.length, 0);
	imageConfig.env.OCI_PROBE = "changed";
	writeFileSync(fixture, JSON.stringify(imageConfig));
	const config = await build(cwd, "config-only");
	assert.notEqual(config.image.digest, cold.image.digest);
	assert.deepEqual(config.image.layers, cold.image.layers);
	assert.equal(config.local.length, 2);
	assert.equal(config.ociLocal.length, 2);
	const source = join(cwd, "cmd/probe/main.go");
	writeFileSync(source, readFileSync(source, "utf8").replace('"v1"', '"v2"'));
	const changed = await build(cwd, "binary-edit");
	const changedLayer = changed.image.layers[0];
	const configuredLayer = config.image.layers[0];
	assert.ok(changedLayer && configuredLayer, "both images must contain the application layer");
	assert.notEqual(changedLayer.digest, configuredLayer.digest);
	assert.ok(changed.local.some(({ identity }) => identity.includes("go_compile")));
	assert.ok(changed.local.some(({ identity }) => identity.includes("oci_layer")));
	await corruptInputs(changed.output, changed.image.digest);
	if (evidence !== undefined) cpSync(changed.output, join(evidence, "native-layout"), { recursive: true });
	await clean(cwd);
	const restored = await build(cwd, "restored");
	assert.equal(restored.local.length, 0);
	assert.ok(restored.cached.length > 0);
	assert.equal(restored.image.digest, changed.image.digest);
	await clean(cwd);
	const second = join(root, "second");
	cpSync(cwd, second, { recursive: true, filter: (path) => basename(path) !== "bsmr-out" });
	workspaces.push(second);
	const fresh = await build(second, "fresh-root");
	assert.equal(fresh.local.length, 0, "source-root paths must not invalidate action identity");
	assert.equal(fresh.image.digest, changed.image.digest);
	const invalid = join(second, "cmd/probe/main.go");
	const validSource = readFileSync(invalid, "utf8");
	writeFileSync(invalid, validSource + invalidSource);
	await assert.rejects(build(second, "failed-build"), /syntax error|non-declaration/i);
	writeFileSync(invalid, validSource);
	assert.equal((await build(second, "failed-retry")).image.digest, fresh.image.digest);
	if (evidence !== undefined) process.stdout.write(`${JSON.stringify({ evidence, platform, engine: values["engine-version"], bundledPrelude: values["bundled-prelude"], cold: resumed === undefined })}\n`);
	process.stdout.write("ok: native Go OCI composition, metadata-only invalidation, shared restoration, rejected inputs, failed retry\n");
	complete = true;
} finally {
	for (const directory of workspaces) {
		await run(executable, ["kill"], { ...options, cwd: directory });
	}
	if (!complete && process.env["BSMR_OCI_TEST_PRESERVE_FAILURE"] === "1") {
		process.stderr.write(`${JSON.stringify({ preservedWorkspace: root, evidence })}\n`);
	} else {
		rmSync(root, { recursive: true });
	}
}
