//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Qualifies explicit publication and authenticated acquisition against a real private TLS registry.

import assert from "node:assert/strict";
import { execFile, execFileSync, spawn } from "node:child_process";
import { createHash, randomBytes } from "node:crypto";
import { constants } from "node:fs";
import { appendFile, chmod, copyFile, cp, lstat, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { request } from "node:https";
import { createServer, isIPv4 } from "node:net";
import { machine, networkInterfaces, release, tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { parseArgs, promisify } from "node:util";
import { importLayout } from "../../prelude/oci/closure.mjs";
import { verifyBundledPrelude } from "./bundled.ts";

const execute = promisify(execFile);
const harnessSha256 = createHash("sha256").update(await readFile(import.meta.filename)).digest("hex");
const { values } = parseArgs({ options: Object.fromEntries([
	"bsmr", "img", "registry-archive", "layout", "prelude", "bind", "evidence", "platform", "engine-version",
].map((name) => [name, { type: "string" }]).concat([["bundled-prelude", { type: "boolean", default: false }]])) });
for (const field of ["bsmr", "img", "registry-archive", "layout", "prelude", "bind", "evidence"]) assert.ok(values[field], `--${field} is required`);
assert.equal(process.platform, "linux", "registry qualification requires an owned Linux worker");
assert.equal(process.getuid?.(), 0, "registry qualification temporarily trusts its fixture CA on an owned worker");
const platform = values.platform ?? `linux/${process.arch === "x64" ? "amd64" : process.arch}`;
assert.equal(platform, `linux/${process.arch === "x64" ? "amd64" : process.arch}`);
assert.equal(machine(), process.arch === "x64" ? "x86_64" : "aarch64");
assert.ok(isIPv4(values.bind) && Object.values(networkInterfaces()).flat().some((entry) => entry?.address === values.bind && !entry.internal),
	"--bind must name this worker's non-loopback IPv4 address");
const pins = JSON.parse(await readFile(new URL("registry.json", import.meta.url), "utf8"));
const asset = pins.assets[platform.replace("/", "-")];
assert.ok(asset, "registry fixture has no archive pin for this platform");
const archive = resolve(values["registry-archive"]), engine = resolve(values.bsmr), img = resolve(values.img);
assert.equal(createHash("sha256").update(await readFile(archive)).digest("hex"), asset.sha256, "registry release archive digest");
await mkdir(resolve(values.evidence), { recursive: true });
const evidence = await mkdtemp(join(resolve(values.evidence), "registry-"));
const root = await mkdtemp(join(tmpdir(), "bsmr-oci-registry-"));
const workspace = join(root, "workspace");
const hostname = `bsmr-registry-${randomBytes(8).toString("hex")}.test`;
await mkdir(workspace);
const controller = new AbortController();
const cancel = () => controller.abort();
process.on("SIGINT", cancel);
process.on("SIGTERM", cancel);
const env = { PATH: process.env.PATH, LANG: "C", TZ: "UTC", BSMR_LOCAL_CACHE_DIR: join(root, "cache") };
const user = `fixture-${randomBytes(8).toString("hex")}`, password = randomBytes(24).toString("hex");
const badPassword = randomBytes(24).toString("hex");
const credentialValues = [user, password, badPassword, ...[password, badPassword].map((value) => Buffer.from(`${user}:${value}`).toString("base64"))];
const installedCA = `/usr/local/share/ca-certificates/${basename(root)}.crt`;
let registry, registryClosed, logs = "", trusted = false, initialized = false, complete = false, hostsEntry;
let port, authenticated, ca, originalTrust;

/** Run one bounded executable without placing credentials on its command line. */
async function run(binary, args, overrides = {}, cancellable = true) {
	return execute(binary, args, { cwd: workspace, env, timeout: 180_000, maxBuffer: 16 * 1024 * 1024,
		signal: cancellable ? controller.signal : undefined, killSignal: "SIGKILL", ...overrides });
}

/** Inspect helper logs before saving them; redaction must never turn a leak into a passing test. */
async function saveOutput(phase, result) {
	const output = (result.stdout ?? "") + (result.stderr ?? "");
	for (const credential of credentialValues) assert.equal(output.includes(credential), false, `${phase} leaked a fixture credential`);
	await writeFile(join(evidence, `${phase}.log`), output);
}

/** Count unique requests, including denied authentication, from Distribution's structured log. */
function requests() {
	return new Set(logs.split("\n").map((line) => {
		try { return JSON.parse(line)["http.request.id"]; } catch { return undefined; }
	}).filter(Boolean)).size;
}

/** Use verified TLS directly for readiness, manifest readback, and fixture-only deletion. */
function http(method, path, credentials = false) {
	return new Promise((resolveResult, reject) => {
		const req = request({ hostname, port, method, path, ca,
			headers: { Accept: "application/vnd.oci.image.manifest.v1+json",
				...(credentials ? { Authorization: `Basic ${Buffer.from(`${user}:${password}`).toString("base64")}` } : {}) } }, (response) => {
			const chunks = [];
			response.on("data", (chunk) => chunks.push(chunk));
			response.on("end", () => resolveResult({ status: response.statusCode, body: Buffer.concat(chunks), headers: response.headers }));
		});
		req.setTimeout(2000, () => req.destroy(new Error("fixture HTTPS request timed out")));
		req.on("error", reject);
		req.end();
	});
}

/** Prepare private keys, bcrypt credentials, and one filesystem-backed Distribution instance. */
async function startRegistry() {
	const reserved = createServer();
	await new Promise((accept, reject) => { reserved.once("error", reject); reserved.listen(0, values.bind, accept); });
	port = reserved.address().port;
	await new Promise((accept) => reserved.close(accept));
	await run("/bin/tar", ["-xzf", archive, "-C", root, "registry"]);
	const binary = join(root, "registry");
	assert.equal((await lstat(binary)).isFile(), true);
	await chmod(binary, 0o755);
	const caPath = join(root, "ca.crt"), caKey = join(root, "ca.key"), key = join(root, "server.key"), cert = join(root, "server.crt");
	const hosts = await readFile("/etc/hosts", "utf8");
	hostsEntry = `${hosts.endsWith("\n") ? "" : "\n"}${values.bind} ${hostname} # ${basename(root)}\n`;
	await appendFile("/etc/hosts", hostsEntry);
	await run("/usr/bin/openssl", ["req", "-x509", "-newkey", "rsa:2048", "-sha256", "-days", "1", "-nodes",
		"-keyout", caKey, "-out", caPath, "-subj", "/CN=BSMR fixture CA", "-addext", "basicConstraints=critical,CA:TRUE"]);
	await run("/usr/bin/openssl", ["req", "-newkey", "rsa:2048", "-nodes", "-keyout", key, "-out", join(root, "server.csr"), "-subj", `/CN=${hostname}`]);
	await writeFile(join(root, "extensions"), `subjectAltName=DNS:${hostname}\nextendedKeyUsage=serverAuth\nbasicConstraints=critical,CA:FALSE\n`);
	await run("/usr/bin/openssl", ["x509", "-req", "-in", join(root, "server.csr"), "-CA", caPath, "-CAkey", caKey,
		"-CAcreateserial", "-out", cert, "-days", "1", "-sha256", "-extfile", join(root, "extensions")]);
	const bcrypt = execFileSync("/usr/bin/htpasswd", ["-nBi", user], { input: password + "\n", encoding: "utf8", timeout: 10_000 });
	await writeFile(join(root, "htpasswd"), bcrypt, { mode: 0o600 });
	ca = await readFile(caPath);
	originalTrust = createHash("sha256").update(await readFile("/etc/ssl/certs/ca-certificates.crt")).digest("hex");
	await copyFile(caPath, installedCA, constants.COPYFILE_EXCL);
	trusted = true;
	await run("/usr/sbin/update-ca-certificates", []);
	const config = { version: "0.1", log: { level: "info", formatter: "json", accesslog: { disabled: true } },
		storage: { filesystem: { rootdirectory: join(root, "storage") }, delete: { enabled: true }, maintenance: { uploadpurging: { enabled: false } } },
		auth: { htpasswd: { realm: "BSMR fixture", path: join(root, "htpasswd") } },
		http: { addr: `${values.bind}:${port}`, host: `https://${hostname}:${port}`, secret: randomBytes(32).toString("hex"),
			draintimeout: "2s", tls: { certificate: cert, key, minimumtls: "tls1.2" } } };
	await writeFile(join(root, "registry.json"), JSON.stringify(config), { mode: 0o600 });
	registry = spawn(binary, ["serve", join(root, "registry.json")], { env: { ...env, OTEL_TRACES_EXPORTER: "none" }, stdio: ["ignore", "pipe", "pipe"] });
	registry.stdout.on("data", (chunk) => { logs += chunk; });
	registry.stderr.on("data", (chunk) => { logs += chunk; });
	registryClosed = new Promise((accept) => registry.once("close", accept));
	for (let attempt = 0; attempt < 60; attempt++) {
		if (registry.exitCode !== null) throw new Error("Distribution exited before readiness");
		try { if ((await http("GET", "/v2/")).status === 401) break; } catch (error) {
			if (attempt === 59) throw error;
		}
		await delay(50, undefined, { signal: controller.signal });
	}
	assert.equal((await http("GET", "/v2/")).status, 401);
	authenticated = { ...env, IMG_REGISTRY_AUTH_HOST: `${hostname}:${port}`, IMG_REGISTRY_AUTH_USERNAME: user,
		IMG_REGISTRY_AUTH_PASSWORD: password, SSL_CERT_FILE: caPath };
	await writeFile(join(evidence, "registry-process.json"), JSON.stringify({ pid: registry.pid, address: `${values.bind}:${port}`, hostname, version: pins.version }));
}

/** Exercise the actual language target and save its separate preparation-action receipt. */
async function graph(phase, target, runArgs, credentials = env) {
	const report = join(evidence, `${phase}-build.json`);
	const result = await run(engine, [runArgs === undefined ? "build" : "run", target, "--build-report", report,
		"--console", "simple", ...(runArgs === undefined ? [] : ["--", ...runArgs])], { env: credentials });
	await saveOutput(phase, result);
	const reportText = await readFile(report, "utf8");
	for (const credential of credentialValues) assert.equal(reportText.includes(credential), false, `${phase} build report leaked a fixture credential`);
	const build = JSON.parse(reportText);
	assert.equal(build.success, true);
	const logged = await run(engine, ["log", "what-ran", "--trace-id", build.trace_id, "--format", "json"]);
	for (const credential of credentialValues) assert.equal(logged.stdout.includes(credential), false, `${phase} action log leaked a fixture credential`);
	const actions = logged.stdout.trim() ? logged.stdout.trim().split("\n").map((line) => JSON.parse(line)) : [];
	await writeFile(join(evidence, `${phase}-actions.json`), JSON.stringify(actions, null, 2));
	if (runArgs !== undefined) return { actions };
	return { actions, output: resolve(workspace, build.results[`root${target}`].outputs.DEFAULT[0]) };
}

/** Denied requests must preserve an absent destination and keep all credential values out of logs. */
async function denied(phase, target, runArgs, credentials, expected) {
	let failure;
	try { await graph(phase, target, runArgs, credentials); } catch (error) { failure = error; }
	assert.ok(failure, `${phase} unexpectedly succeeded`);
	await saveOutput(phase, failure);
	assert.match(failure.stderr ?? failure.message, expected);
	if (runArgs?.includes("--output")) await assert.rejects(lstat(runArgs.at(-1)), { code: "ENOENT" });
}

/** Validate downloaded content through the production closure checker with independent metadata outputs. */
async function metadata(layout, phase) {
	return importLayout({ platform, layout, manifest: join(evidence, `${phase}-manifest.json`),
		config: join(evidence, `${phase}-config.json`), descriptor: join(evidence, `${phase}-descriptor.json`) });
}

try {
	await startRegistry();
	assert.equal((await run(engine, ["--version"])).stdout.trim(), `bsmr ${values["engine-version"] ?? "0.0.9"}`);
	await run(engine, ["init"]);
	initialized = true;
	if (values["bundled-prelude"]) await verifyBundledPrelude(engine, resolve(values.prelude), workspace,
		async (file, args) => ({ ...await run(file, args), exitCode: 0 }), evidence);
	else {
		await cp(resolve(values.prelude), join(workspace, "prelude"), { recursive: true });
		await writeFile(join(workspace, ".bsmr.local"), "[external_cells]\nprelude = disabled\n");
	}
	for (const [name, source] of Object.entries({ img, node: process.execPath })) await copyFile(source, join(workspace, name));
	await cp(resolve(values.layout), join(workspace, "base"), { recursive: true });
	await copyFile(new URL("fixtures/artifact.bzl", import.meta.url), join(workspace, "artifact.bzl"));
	const original = await metadata(join(workspace, "base"), "original");
	const repository = "fixture/image", image = `${hostname}:${port}/${repository}:proof`;
	await mkdir(join(workspace, "acquire"));
	await mkdir(join(workspace, "images"));
	await writeFile(join(workspace, "acquire/image.lock.json"), JSON.stringify({ version: 1, image, platform, manifest_digest: original.descriptor.digest }));
	await writeFile(join(workspace, "BUILD.bsmr"), `load("@prelude//oci:defs.bzl", "oci_import", "oci_push")
load("@prelude//oci:toolchain.bzl", "oci_toolchain")
load(":artifact.bzl", "artifact")
artifact(name = "encoder", binary = "img")
artifact(name = "runtime", binary = "node")
oci_toolchain(name = "oci", img = ":encoder", node = ":runtime", visibility = ["PUBLIC"])
oci_import(name = "base", layout = "base", platform = "${platform}", toolchain = ":oci")
oci_push(name = "publish", image = ":base", repository = "${hostname}:${port}/${repository}", tags = ["proof"], toolchain = ":oci")
`);
	await writeFile(join(workspace, "acquire/BUILD.bsmr"), `load("@prelude//oci:defs.bzl", "oci_fetch", "oci_pull")
oci_fetch(name = "base", image = "${image}", platform = "${platform}", lock = "image.lock.json", toolchain = "//:oci")
oci_pull(name = "anonymous", image = "${image}", platform = "${platform}", lock = "image.lock.json", toolchain = "//:oci")
`);
	await writeFile(join(workspace, "images/BUILD.bsmr"), `load("@prelude//oci:defs.bzl", "oci_import", "oci_layout")
oci_import(name = "base", layout = "base", platform = "${platform}", toolchain = "//:oci")
oci_layout(name = "roundtrip", image = ":base", toolchain = "//:oci")
`);
	const beforePrepare = requests();
	await graph("fetch-prepared", "//acquire:base");
	await graph("publish-prepared", "//:publish");
	assert.equal(requests(), beforePrepare, "build preparation contacted the registry");
	assert.equal((await graph("publish-warm", "//:publish")).actions.length, 0);
	await graph("published", "//:publish", [], authenticated);
	const manifestPath = `/v2/${repository}/manifests/${original.descriptor.digest}`;
	assert.equal((await http("GET", manifestPath, true)).status, 200);
	assert.equal(`sha256:${createHash("sha256").update((await http("GET", `/v2/${repository}/manifests/proof`, true)).body).digest("hex")}`,
		original.descriptor.digest, "publication must assign the requested tag to the exact manifest");
	assert.equal((await http("DELETE", manifestPath, true)).status, 202);
	assert.equal((await http("GET", manifestPath, true)).status, 404);
	await run(engine, ["clean"]);
	const beforeRestore = requests();
	const restored = await graph("prepare-restored", "//:publish");
	assert.ok(restored.actions.some((action) => action.identity.includes("oci_push_metadata") && action.reproducer.executor === "Cache"));
	assert.equal(requests(), beforeRestore, "cached preparation contacted the registry");
	await graph("republished", "//:publish", [], authenticated);
	assert.equal((await http("GET", manifestPath, true)).status, 200, "explicit publication was skipped after preparation cache restoration");
	assert.equal(`sha256:${createHash("sha256").update((await http("GET", `/v2/${repository}/manifests/proof`, true)).body).digest("hex")}`,
		original.descriptor.digest, "republication must restore the requested tag");
	await denied("missing-fetch-auth", "//acquire:base", ["--output", join(workspace, "missing")], env, /OCI_REGISTRY_AUTH/u);
	await denied("wrong-fetch-auth", "//acquire:base", ["--output", join(workspace, "wrong")], { ...authenticated, IMG_REGISTRY_AUTH_PASSWORD: badPassword }, /OCI_PULL_FAILED/u);
	await denied("missing-push-auth", "//:publish", [], env, /OCI_PUSH_FAILED/u);
	await denied("wrong-push-auth", "//:publish", [], { ...authenticated, IMG_REGISTRY_AUTH_PASSWORD: badPassword }, /OCI_PUSH_FAILED/u);
	await graph("fetched", "//acquire:base", ["--output", join(workspace, "images/base")], authenticated);
	const fetched = await metadata(join(workspace, "images/base"), "fetched");
	assert.equal(fetched.descriptor.digest, original.descriptor.digest);
	assert.deepEqual(fetched.config, original.config);
	assert.deepEqual(fetched.manifest, original.manifest);
	const imported = await graph("imported", "//images:roundtrip");
	assert.equal((await metadata(imported.output, "roundtrip")).descriptor.digest, original.descriptor.digest);
	const beforeAnonymous = requests();
	await denied("anonymous-private-pull", "//acquire:anonymous", undefined, authenticated, /UNAUTHORIZED: authentication required/u);
	assert.ok(requests() > beforeAnonymous, "anonymous failure did not reach the TLS-trusted private registry");
	await run("/bin/tar", ["-cf", join(evidence, "roundtrip.tar"), "-C", imported.output, "."]);
	await writeFile(join(evidence, "receipt.json"), JSON.stringify({ platform, kernel: release(), registryVersion: pins.version,
		registryArchiveSha256: asset.sha256, imageDigest: original.descriptor.digest, harnessSha256,
		engineVersion: values["engine-version"] ?? "0.0.9",
		tools: Object.fromEntries(await Promise.all(Object.entries({ bsmr: engine, img, node: process.execPath,
			registry: join(root, "registry") }).map(async ([name, path]) => [name, createHash("sha256").update(await readFile(path)).digest("hex")]))),
		sourceHelpers: Object.fromEntries(await Promise.all(["auth.mjs", "client.mjs", "push.mjs", "sources.mjs", "closure.mjs", "push.bzl", "sources.bzl"]
			.map(async (name) => [name, createHash("sha256").update(await readFile(join(values["bundled-prelude"] ? resolve(values.prelude) : join(workspace, "prelude"), "oci", name))).digest("hex")]))),
		bundledPrelude: values["bundled-prelude"], tlsVerified: true, authenticatedRoundtrip: true, fetchPreparationOffline: true,
		republicationAfterCacheRestore: true, requestedTagVerified: true, credentialFailuresRedacted: true, anonymousPrivatePullRejected: true }, null, 2));
	complete = true;
	process.stdout.write(`${JSON.stringify({ evidence, digest: original.descriptor.digest, authenticatedRoundtrip: true })}\n`);
} catch (error) {
	await saveOutput("failure", { stderr: String(error) });
	throw error;
} finally {
	controller.abort();
	if (registry && registry.exitCode === null) {
		registry.kill("SIGTERM");
		const timer = setTimeout(() => registry.kill("SIGKILL"), 5000);
		await registryClosed;
		clearTimeout(timer);
	}
	await writeFile(join(evidence, "registry.log"), credentialValues.reduce((value, secret) => value.replaceAll(secret, "<fixture-credential>"), logs));
	if (initialized) await run(engine, ["kill"], {}, false);
	if (trusted) {
		await rm(installedCA);
		await run("/usr/sbin/update-ca-certificates", [], {}, false);
		assert.equal(createHash("sha256").update(await readFile("/etc/ssl/certs/ca-certificates.crt")).digest("hex"), originalTrust, "fixture CA cleanup changed unrelated trust roots");
	}
	if (hostsEntry) {
		const hosts = await readFile("/etc/hosts", "utf8");
		assert.equal(hosts.split(hostsEntry).length, 2, "the fixture hosts entry must remain unique");
		await writeFile("/etc/hosts", hosts.replace(hostsEntry, ""));
	}
	if (!complete && initialized) {
		const entries = [];
		for (const name of [".bsmr", ".bsmr.local", "BUILD.bsmr", "artifact.bzl", "prelude", "acquire", "images"]) {
			try { await lstat(join(workspace, name)); entries.push(name); } catch (error) { if (error.code !== "ENOENT") throw error; }
		}
		await run("/bin/tar", ["-cf", join(evidence, "failed-workspace.tar"), "-C", workspace, ...entries], {}, false);
	}
	if (complete) await rm(root, { recursive: true, force: true });
	await writeFile(join(evidence, "cleanup.json"), JSON.stringify({ registryExited: registry?.exitCode !== null || registry?.signalCode !== null,
		fixtureCARemoved: trusted, fixtureHostsEntryRemoved: Boolean(hostsEntry), workspaceRemoved: complete, failedWorkspace: complete ? null : root }));
	process.off("SIGINT", cancel);
	process.off("SIGTERM", cancel);
}
