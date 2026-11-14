//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies publication receives only explicit credentials and never prints denied credentials.

import assert from "node:assert/strict";
import { execFile, spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { chmod, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";
import { setTimeout } from "node:timers/promises";
import { promisify } from "node:util";

const execute = promisify(execFile);
const operation = resolve(import.meta.dirname, "../../prelude/oci/push.mjs");

/** Capture the real wrapper's subprocess boundary without contacting a registry. */
async function fixture(t, exitCode = 0, wait = false) {
	const root = await mkdtemp(join(tmpdir(), "bsmr-oci-publish-"));
	t.after(() => rm(root, { recursive: true, force: true }));
	const request = join(root, "request.json");
	const capture = join(root, "capture.json");
	const img = join(root, "img");
	await writeFile(request, JSON.stringify({ operations: [{ command: "push", registry: "registry.example.com" }] }));
	await writeFile(img, `#!${process.execPath}\nconst fs = require('node:fs'); if (${wait}) process.on('SIGTERM',()=>{}); fs.writeFileSync(${JSON.stringify(capture)}, JSON.stringify({pid:process.pid, ppid:process.ppid, env:process.env, args:process.argv.slice(2), cwd:process.cwd()})); if (${wait}) setInterval(()=>{},1000); else { if (${exitCode}) process.stderr.write(process.env.IMG_REGISTRY_AUTH_PASSWORD); process.exit(${exitCode}); }\n`);
	await chmod(img, 0o755);
	const args = [operation, "--img", img, "--request", request, "--layout", root];
	return { root, args, capture };
}

test("invariant publication credentials stay in the runtime environment for exactly one host", async (t) => {
	const f = await fixture(t);
	await execute(process.execPath, f.args, { env: { ...process.env, IMG_REGISTRY_AUTH_HOST: "registry.example.com",
		IMG_REGISTRY_AUTH_USERNAME: "fixture", IMG_REGISTRY_AUTH_PASSWORD: "fixture-password", AWS_SECRET_ACCESS_KEY: "must-not-escape", IMG_INSECURE: "1" } });
	const captured = JSON.parse(await readFile(f.capture, "utf8"));
	assert.equal(captured.env.IMG_REGISTRY_AUTH_PASSWORD, "fixture-password");
	assert.equal(captured.env.IMG_INSECURE, "0");
	assert.equal(captured.env.AWS_SECRET_ACCESS_KEY, undefined);
	assert.equal(JSON.stringify(captured.args).includes("fixture-password"), false);
	assert.equal(existsSync(captured.cwd), false);
});

test("invariant a credential host mismatch never starts the publisher", async (t) => {
	const f = await fixture(t);
	await assert.rejects(execute(process.execPath, f.args, { env: { ...process.env, IMG_REGISTRY_AUTH_HOST: "another.example.com",
		IMG_REGISTRY_AUTH_USERNAME: "fixture", IMG_REGISTRY_AUTH_PASSWORD: "fixture-password" } }),
		(error) => /OCI_REGISTRY_AUTH/u.test(error.stderr));
	assert.equal(existsSync(f.capture), false);
});

test("invariant registry error bodies cannot expose publication credentials", async (t) => {
	const f = await fixture(t, 1);
	await assert.rejects(execute(process.execPath, f.args, { env: { ...process.env, IMG_REGISTRY_AUTH_HOST: "registry.example.com",
		IMG_REGISTRY_AUTH_USERNAME: "fixture", IMG_REGISTRY_AUTH_PASSWORD: "fixture-password" } }),
		(error) => error.stderr.startsWith("OCI_PUSH_FAILED:") && !error.stderr.includes("fixture-password"));
	const captured = JSON.parse(await readFile(f.capture, "utf8"));
	assert.equal(existsSync(captured.cwd), false);
});

test("invariant cancelling only the publisher reaps its client before removing authentication state", async (t) => {
	const f = await fixture(t, 0, true), existing = join(f.root, "existing-image");
	await writeFile(existing, "retained image\n");
	const helper = spawn(process.execPath, f.args, {
		env: { TMPDIR: f.root, IMG_REGISTRY_AUTH_HOST: "registry.example.com", IMG_REGISTRY_AUTH_USERNAME: "fixture", IMG_REGISTRY_AUTH_PASSWORD: "fixture-password" },
		stdio: ["ignore", "pipe", "pipe"],
	});
	let stderr = "", client;
	helper.stderr.on("data", (chunk) => { stderr += chunk; });
	const closed = new Promise((resolve, reject) => {
		helper.once("error", reject);
		helper.once("close", (code, signal) => resolve({ code, signal }));
	});
	/** Detect the owned client even if a broken publisher orphaned it. */
	function alive(pid) {
		try { process.kill(pid, 0); return true; } catch (error) { if (error.code === "ESRCH") return false; throw error; }
	}
	try {
		for (let i = 0; i < 100 && !existsSync(f.capture); i++) await setTimeout(25);
		assert.equal(existsSync(f.capture), true, "client must be running before cancellation");
		client = JSON.parse(await readFile(f.capture, "utf8"));
		assert.equal(client.ppid, helper.pid);
		helper.kill("SIGTERM");
		const result = await Promise.race([closed, setTimeout(5000, null, { ref: false }).then(() => { throw new Error("publisher did not close"); })]);
		assert.deepEqual(result, { code: 1, signal: null }, stderr);
		assert.match(stderr, /^OCI_PUSH_CANCELLED:/u);
		assert.equal(alive(client.pid), false, "client must be reaped before publisher exit");
		assert.equal(existsSync(client.cwd), false);
		assert.equal(await readFile(existing, "utf8"), "retained image\n");
	} finally {
		if (alive(helper.pid)) helper.kill("SIGKILL");
		await closed;
		if (client && alive(client.pid)) {
			process.kill(client.pid, "SIGKILL");
			for (let i = 0; i < 100 && alive(client.pid); i++) await setTimeout(20);
			assert.equal(alive(client.pid), false, "test must not leave its client running");
		}
	}
});
