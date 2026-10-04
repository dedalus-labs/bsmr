//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Acquires digest-locked images without persisting registry credentials in artifacts.

import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { lstat, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { parseArgs, promisify } from "node:util";
import { importLayout } from "./closure.mjs";
import { registryEnvironment } from "./auth.mjs";

const execute = promisify(execFile);
const manifestType = "application/vnd.oci.image.manifest.v1+json";
const indexType = "application/vnd.oci.image.index.v1+json";
const digestPattern = /^sha256:[0-9a-f]{64}$/u;

/** Name the unsupported or mismatched acquisition contract. */
function fail(code, message) {
	throw Object.assign(new Error(message), { code });
}

/** Reject unknown fields instead of silently ignoring future lock semantics. */
function object(value, fields, name) {
	if (value === null || typeof value !== "object" || Array.isArray(value)
		|| Object.keys(value).sort().join(",") !== [...fields].sort().join(",")) {
		fail("OCI_INVALID_LOCK", `${name} has an unsupported schema`);
	}
}

/** Bind an exact image spelling and platform to a direct immutable manifest. */
export function lockedImage(lock, image, platform) {
	object(lock, ["version", "image", "platform", "manifest_digest"], "image lock");
	if (lock.version !== 1 || typeof lock.manifest_digest !== "string" || !digestPattern.test(lock.manifest_digest)) {
		fail("OCI_INVALID_LOCK", "image lock requires version 1 and a lowercase SHA-256 manifest_digest");
	}
	if (!["linux/amd64", "linux/arm64"].includes(platform)) fail("OCI_UNSUPPORTED_PLATFORM", "pull platform must be linux/amd64 or linux/arm64");
	if (lock.image !== image || lock.platform !== platform) fail("OCI_LOCK_MISMATCH", "image lock must exactly match the requested image and platform");
	const reference = typeof image === "string" && /^([^/\s]+)\/([^:@\s]+):([A-Za-z0-9_][A-Za-z0-9_.-]{0,127})$/u.exec(image);
	if (!reference) fail("OCI_INVALID_IMAGE", "image must be fully qualified as registry/repository:tag");
	const [, registry, repository] = reference;
	let authority;
	try { authority = new URL("https://" + registry); } catch { fail("OCI_INVALID_IMAGE", "registry must be a valid host with an optional port"); }
	if (authority.host !== registry || authority.username || authority.password || authority.pathname !== "/" || authority.search || authority.hash
		|| !repository.split("/").every((part) => /^[a-z0-9]+(?:(?:[._]|__|-+)[a-z0-9]+)*$/u.test(part))) {
		fail("OCI_INVALID_IMAGE", "image must contain a canonical registry host and normalized repository");
	}
	return { registry, repository, digest: lock.manifest_digest, platform };
}

/** Bound recipe and manifest reads without accepting special-file inputs. */
async function readMetadata(path) {
	const info = await lstat(path);
	if (!info.isFile() || info.size > 16 * 1024 * 1024) fail("OCI_INVALID_METADATA", "acquisition metadata must be a regular file no larger than 16 MiB");
	return readFile(path);
}

/** Report malformed metadata without echoing registry response bytes. */
function decode(bytes) {
	try { return JSON.parse(bytes); } catch { fail("OCI_INVALID_METADATA", "acquisition metadata must be valid JSON"); }
}

/** Run the maintained client with isolated anonymous registry access. */
async function registryCommand(binary, args, scratch, env) {
	try {
		await execute(resolve(binary), args, { cwd: scratch, timeout: 300_000, maxBuffer: 4 * 1024 * 1024,
			env });
	} catch (error) {
		if (error.stderr) process.stderr.write(error.stderr);
		fail("OCI_PULL_FAILED", "pinned img could not acquire the locked image");
	}
}

/** Materialize a complete verified public image from one immutable manifest. */
async function pull(values) {
	const spec = decode(await readMetadata(values.spec));
	object(spec, ["image", "platform", "lock"], "pull specification");
	if (typeof spec.lock !== "string" || !spec.lock || /[\0\r\n]/u.test(spec.lock)) fail("OCI_INVALID_LOCK", "pull specification requires a lock artifact path");
	const selected = lockedImage(decode(await readMetadata(spec.lock)), spec.image, spec.platform);
	await mkdir(dirname(values.output), { recursive: true });
	const scratch = await mkdtemp(join(tmpdir(), "bsmr-oci-pull-auth-"));
	try {
		await writeFile(join(scratch, "config.json"), "{}", { mode: 0o600, flag: "wx" });
		const env = registryEnvironment(selected.registry, scratch);
		await mkdir(values.output);
		try {
			const manifestPath = join(values.output, "blobs", "sha256", selected.digest.slice(7));
			await registryCommand(values.img, ["download-manifest", "--digest", selected.digest,
				"--source", selected.repository + "@" + selected.registry, "--output", manifestPath], scratch, env);
			const manifestBytes = await readMetadata(manifestPath);
			if (`sha256:${createHash("sha256").update(manifestBytes).digest("hex")}` !== selected.digest) {
				fail("OCI_DIGEST_MISMATCH", "downloaded manifest does not match the locked digest");
			}
			const manifest = decode(manifestBytes);
			if (manifest?.mediaType !== manifestType || manifest.schemaVersion !== 2) {
				fail("OCI_DIRECT_MANIFEST_REQUIRED", "image lock must pin a direct OCI image manifest, not an index");
			}
			await registryCommand(values.img, ["pull", "--registry", selected.registry, "--repository", selected.repository,
				"--reference", selected.digest, "--platform", selected.platform, "--layer-handling", "eager", "--output", values.output], scratch, env);
			const descriptor = { mediaType: manifestType, digest: selected.digest, size: manifestBytes.length };
			await writeFile(join(values.output, "oci-layout"), '{"imageLayoutVersion":"1.0.0"}', { flag: "wx" });
			await writeFile(join(values.output, "index.json"), JSON.stringify({ schemaVersion: 2, mediaType: indexType, manifests: [descriptor] }), { flag: "wx" });
			const metadata = { manifest: values.manifest, config: values.config, descriptor: values.descriptor };
			await importLayout({ platform: selected.platform, layout: values.output, ...metadata });
		} catch (error) {
			await rm(values.output, { recursive: true, force: true });
			throw error;
		}
	} finally {
		await rm(scratch, { recursive: true });
	}
}

/** Require every declared output and tool exactly once before making network requests. */
async function main() {
	const fields = ["img", "spec", "output", "manifest", "config", "descriptor"];
	const { values, tokens } = parseArgs({ tokens: true, options: Object.fromEntries(fields.map((name) => [name, { type: "string" }])) });
	if (tokens.length !== fields.length || fields.some((name) => typeof values[name] !== "string" || !values[name] || /[\0\r\n]/u.test(values[name]))) {
		fail("OCI_INVALID_ARGUMENTS", "each pull tool, input, and output must be specified exactly once");
	}
	await pull(Object.fromEntries(fields.map((name) => [name, resolve(values[name])])));
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
	main().catch((error) => {
		process.stderr.write((error.code ?? error.name) + ": " + error.message + "\n");
		process.exitCode = 1;
	});
}
