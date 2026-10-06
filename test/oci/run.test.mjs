//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Proves recipe boundaries and the real rootful umoci/runc package installation contract.

import assert from "node:assert/strict";
import { execFile, spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { lstat, mkdir, mkdtemp, readFile, readdir, readlink, rm, writeFile } from "node:fs/promises";
import { machine, release, tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";
import { promisify } from "node:util";
import { setTimeout } from "node:timers/promises";
import { importLayout } from "../../prelude/oci/closure.mjs";
import { processConfig, runImage, validateRunSpec } from "../../prelude/oci/run.mjs";

const execute = promisify(execFile);
const umoci = process.env.BSMR_OCI_UMOCI, runc = process.env.BSMR_OCI_RUNC;
const base = process.env.BSMR_OCI_RUN_BASE;
const native = process.env.BSMR_OCI_REQUIRE_NATIVE === "1";
const platform = process.env.BSMR_OCI_PLATFORM ?? `linux/${process.arch === "x64" ? "amd64" : process.arch}`;
if (native) {
	assert.equal(process.platform, "linux", "native qualification requires Linux");
	assert.equal(process.getuid?.(), 0, "native qualification requires root");
	assert.ok(["x64", "arm64"].includes(process.arch), "native qualification requires amd64 or arm64");
	assert.equal(platform, `linux/${process.arch === "x64" ? "amd64" : process.arch}`, "selected platform must match the native Node architecture");
	assert.equal(machine(), process.arch === "x64" ? "x86_64" : "aarch64", "native qualification cannot use architecture emulation");
	assert.ok(umoci && runc && base, "set BSMR_OCI_UMOCI, BSMR_OCI_RUNC and BSMR_OCI_RUN_BASE");
	process.stdout.write(`${JSON.stringify({ platform, kernel: release(), machine: machine(), node: process.version })}\n`);
}

/** State the complete public recipe, leaving execution config inherited by default. */
function recipe(overrides = {}) {
	return { layout: base ?? "/declared/base", platform, inputs: {},
		command: ["/bin/true"], env: {}, user: null, working_dir: null, ...overrides };
}

/** Allocate one test's artifacts with deterministic ownership and cleanup. */
async function fixture(t) {
	const root = await mkdtemp(join(tmpdir(), "bsmr-run-test-"));
	t.after(() => rm(root, { recursive: true, force: true }));
	return root;
}

/** Hash every source-layout byte to detect accidental mutation or inode sharing. */
async function digestDirectory(root) {
	const hash = createHash("sha256");
	/** Visit lexical paths rather than trusting directory enumeration order. */
	async function visit(path) {
		for (const name of (await readdir(path)).sort()) {
			const file = join(path, name);
			hash.update(file.slice(root.length));
			if ((await lstat(file)).isDirectory()) await visit(file);
			else hash.update(await readFile(file));
		}
	}
	await visit(root);
	return hash.digest("hex");
}

/** Validate completed output through the same importer used by the language rule. */
async function inspect(layout, root) {
	return importLayout({ platform, layout, manifest: join(root, "manifest.json"),
		config: join(root, "config.json"), descriptor: join(root, "descriptor.json") });
}

test("invariant_run_recipe_rejects_ambiguous_paths_and_processes", () => {
	for (const name of ["", "/absolute", "../escape", "a/../escape", "a//b", "a\\b"]) {
		assert.throws(() => validateRunSpec(recipe({ inputs: { [name]: "artifact" } })));
	}
	for (const override of [{ command: [] }, { command: ["/bin/sh", "bad\0arg"] }, { user: "root" },
		{ user: "4294967295:0" }, { working_dir: "relative" }, { env: { "BAD=NAME": "x" } },
		{ inputs: { a: "one", "a/b": "two" } }, { platform: "darwin/arm64" }, { unknown: true }]) {
		assert.throws(() => validateRunSpec(recipe(override)));
	}
	assert.deepEqual(validateRunSpec(recipe({ command: ["/bin/printf", "", "line\n"], user: "123:456", working_dir: "/work" })),
		recipe({ command: ["/bin/printf", "", "line\n"], user: "123:456", working_dir: "/work" }));
});

test("invariant_runtime_overrides_keep_namespaces_and_readonly_inputs", () => {
	const config = { process: { args: ["base"], env: ["PATH=/bin", "KEPT=a=b"], cwd: "/base", user: { uid: 1, gid: 2 } },
		root: { path: "rootfs" }, mounts: [], linux: { namespaces: ["mount", "pid", "network", "ipc", "uts", "cgroup"].map((type) => ({ type })) } };
	const result = processConfig(config, recipe({ env: { PATH: "/usr/bin" }, user: "123:456", working_dir: "/work" }), "/private/inputs");
	assert.deepEqual(result.process.user, { uid: 123, gid: 456 });
	assert.deepEqual(result.process.env, ["PATH=/usr/bin", "KEPT=a=b"]);
	assert.equal(result.process.cwd, "/work");
	assert.deepEqual(result.process.capabilities.bounding, []);
	assert.deepEqual(result.mounts[0].options, ["bind", "ro", "nosuid", "nodev"]);
	result.linux.namespaces.find((ns) => ns.type === "network").path = "/proc/1/ns/net";
	assert.throws(() => processConfig(result, recipe(), "/private/inputs"), { code: "OCI_RUN_ISOLATION" });
});

test("invariant_offline_apt_executes_postinst_and_preserves_filesystem_metadata", { skip: !native }, async (t) => {
	const root = await fixture(t), packageRoot = join(root, "package"), output = join(root, "output");
	await mkdir(join(packageRoot, "DEBIAN"), { recursive: true });
	await writeFile(join(packageRoot, "DEBIAN/control"), "Package: bsmr-proof\nVersion: 1.0\nArchitecture: all\nMaintainer: BSMR <test@example.invalid>\nDescription: native installation proof\n");
	await writeFile(join(packageRoot, "DEBIAN/postinst"), "#!/bin/sh\nset -eu\nmkdir /proof\nprintf postinst > /proof/receipt\nln /proof/receipt /proof/hardlink\nln -s /proof/receipt /proof/symlink\nchmod 0751 /proof/receipt\nchown 123:456 /proof/receipt\nrm /etc/debian_version\n", { mode: 0o755 });
	const deb = join(root, "proof.deb");
	await execute("/usr/bin/dpkg-deb", ["--build", packageRoot, deb]);
	const before = await digestDirectory(base), inputBefore = await readFile(deb);
	await runImage({ umoci, runc, output, spec: recipe({ user: "0:0", inputs: {
		"proof.deb": deb, "offline.sh": resolve(import.meta.dirname, "fixtures/offline.sh"),
	}, command: ["/bin/sh", "-exc", "apt-get -o Dir::Etc::sourcelist=/dev/null -o Dir::Etc::sourceparts=- -y --no-install-recommends install /inputs/proof.deb; /bin/bash /inputs/offline.sh; test -e /proc/self/status; ! touch /inputs/forbidden"] }) });
	assert.equal(await digestDirectory(base), before);
	assert.deepEqual(await readFile(deb), inputBefore);
	const original = await inspect(base, join(root, "original")), result = await inspect(output, join(root, "result"));
	assert.deepEqual(result.config.config, original.config.config, "execution overrides must not change image defaults");
	assert.deepEqual(result.manifest.layers.slice(0, -1), original.manifest.layers);
	await execute(umoci, ["unpack", "--image", `${output}:run`, join(root, "unpacked")]);
	const filesystem = join(root, "unpacked/rootfs"), receipt = await lstat(join(filesystem, "proof/receipt"));
	assert.equal(await readFile(join(filesystem, "proof/receipt"), "utf8"), "postinst");
	assert.equal(receipt.mode & 0o7777, 0o751);
	assert.equal(receipt.uid, 123);
	assert.equal(receipt.gid, 456);
	assert.equal(receipt.ino, (await lstat(join(filesystem, "proof/hardlink"))).ino);
	assert.equal(await readlink(join(filesystem, "proof/symlink")), "/proof/receipt");
	await assert.rejects(lstat(join(filesystem, "etc/debian_version")), { code: "ENOENT" });
	await assert.rejects(lstat(join(filesystem, "inputs")), { code: "ENOENT" });
});

test("invariant_failed_command_publishes_no_layout_or_changed_input", { skip: !native }, async (t) => {
	const root = await fixture(t), input = join(root, "immutable"), output = join(root, "output");
	await writeFile(input, "original");
	await assert.rejects(runImage({ umoci, runc, output, spec: recipe({ inputs: { immutable: input },
		command: ["/bin/sh", "-ec", "printf changed > /inputs/immutable"] }) }),
		(error) => error.code === "OCI_RUN_TOOL_FAILED" && /Read-only file system/u.test(error.cause.stderr));
	await assert.rejects(lstat(output), { code: "ENOENT" });
	assert.equal(await readFile(input, "utf8"), "original");
});

test("invariant_concurrent_runs_have_independent_cgroups_and_users", { skip: !native }, async (t) => {
	const root = await fixture(t);
	await Promise.all(["123:456", "321:654"].map(async (user, index) => {
		const output = join(root, `image-${index}`);
		await runImage({ umoci, runc, output, spec: recipe({ user, working_dir: "/tmp", env: { EXPECTED_UID: user.split(":")[0] },
			command: ["/bin/sh", "-ec", 'test "$(id -u)" = "$EXPECTED_UID"; test "$(pwd)" = /tmp; sleep 1; printf owned > receipt'] }) });
		await execute(umoci, ["unpack", "--image", `${output}:run`, join(root, `bundle-${index}`)]);
		assert.equal((await lstat(join(root, `bundle-${index}/rootfs/tmp/receipt`))).uid, Number(user.split(":")[0]));
	}));
});

test("invariant_cancelled_command_removes_its_container_and_work_directory", { skip: !native }, async (t) => {
	const root = await fixture(t), output = join(root, "output"), specification = join(root, "spec.json");
	await writeFile(specification, JSON.stringify(recipe({ command: ["/bin/sh", "-ec", "trap '' TERM; sleep 600 & wait"] })));
	const child = spawn(process.execPath, [resolve(import.meta.dirname, "../../prelude/oci/run.mjs"),
		"--spec", specification, "--umoci", umoci, "--runc", runc, "--output", output], { env: { ...process.env, TMPDIR: root }, stdio: ["ignore", "pipe", "pipe"] });
	let stderr = "";
	child.stderr.on("data", (chunk) => { stderr += chunk; });
	const closed = new Promise((resolve) => child.on("close", (code) => resolve(code)));
	let running = false;
	try {
		for (let attempt = 0; attempt < 100 && !running; attempt++) {
			for (const directory of (await readdir(root)).filter((name) => name.startsWith("bsmr-oci-run-"))) {
				const state = join(root, directory, "state");
				try {
					const result = await execute(runc, ["--root", state, "list", "--format", "json"]);
					running = (JSON.parse(result.stdout) ?? []).some((container) => container.status === "running");
				} catch (error) { if (error.code === undefined) throw error; }
			}
			if (!running) await setTimeout(50);
		}
	} finally {
		child.kill("SIGTERM");
		await closed;
	}
	assert.equal(await closed, 1, stderr);
	assert.equal(running, true, "cancellation must interrupt a real running container");
	assert.match(stderr, /OCI_RUN_CANCELLED/u);
	await assert.rejects(lstat(output), { code: "ENOENT" });
	assert.deepEqual((await readdir(root)).filter((name) => name.startsWith("bsmr-oci-run-")), []);
});
