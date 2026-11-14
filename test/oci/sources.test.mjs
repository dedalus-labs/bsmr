//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies pinned acquisition, explicit runtime authentication, and image normalization.

import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync } from "node:fs";
import { chmod, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";
import { promisify } from "node:util";
import { gzipSync } from "node:zlib";
import { imageMetadata } from "../../prelude/oci/closure.mjs";
import { lockedImage } from "../../prelude/oci/sources.mjs";

const execute = promisify(execFile);
const operation = resolve(import.meta.dirname, "../../prelude/oci/sources.mjs");
const pin = { version: 1, image: "docker.io/library/debian:bookworm-slim", platform: "linux/arm64",
	manifest_digest: "sha256:0c8bbb8e987a035fe1d9704eb2e571b7e9a836e1caa46345290674b45b69e417" };

/** Own every local artifact used by one pull recipe. */
async function fixture(t, selected = pin) {
	const root = await mkdtemp(join(tmpdir(), "bsmr-oci-pull-"));
	t.after(() => rm(root, { recursive: true, force: true }));
	const lock = join(root, "image.lock.json");
	const values = { img: process.env.BSMR_OCI_IMG ?? join(root, "absent-img"), spec: join(root, "spec.json"),
		output: join(root, "layout"), manifest: join(root, "manifest.json"), config: join(root, "config.json"), descriptor: join(root, "descriptor.json") };
	await writeFile(lock, JSON.stringify(selected));
	await writeFile(values.spec, JSON.stringify({ lock, image: selected.image, platform: selected.platform }));
	return { root, lock, values };
}

/** Invoke the production helper with controlled parent-process environment. */
async function invoke(values, env = process.env, command) {
	return execute(process.execPath, [operation, ...(command ? [command] : []), ...Object.entries(values).flatMap(([key, value]) => [`--${key}`, value])],
		{ env, timeout: 180_000, maxBuffer: 4 * 1024 * 1024 });
}

test("invariant pull locks bind the requested image and platform to one direct manifest digest", () => {
	assert.deepEqual(lockedImage(pin, pin.image, pin.platform), {
		registry: "docker.io", repository: "library/debian", digest: pin.manifest_digest, platform: pin.platform,
	});
	for (const patch of [{ version: 2 }, { image: "docker.io/library/debian:other" }, { platform: "linux/amd64" },
		{ manifest_digest: "sha256:bad" }, { manifest_digest: pin.manifest_digest.toUpperCase() }, { extra: true }]) {
		assert.throws(() => lockedImage({ ...pin, ...patch }, pin.image, pin.platform), (error) => error.code.startsWith("OCI_"));
	}
});

test("invariant invalid registry references fail without normalization to another destination", () => {
	for (const image of ["docker.io/library/debian", "https://docker.io/library/debian:tag", "name@docker.io/library/debian:tag",
		"docker.io/../debian:tag", "docker.io/library//debian:tag", "docker.io/library/debian:bad/tag", "docker.io/library/debian:@tag",
		"registry.example:99999/image:tag", "registry.example?/image:tag", "docker.io/Library/debian:tag"]) {
		assert.throws(() => lockedImage({ ...pin, image }, image, pin.platform), { code: "OCI_INVALID_IMAGE" });
	}
});

test("invariant qualified cloud and private registry references keep their declared identity", () => {
	for (const registry of ["gcr.io", "us-docker.pkg.dev", "public.ecr.aws", "123.dkr.ecr.us-east-1.amazonaws.com", "registry.example:5443"]) {
		const image = `${registry}/example/image:tag`;
		assert.equal(lockedImage({ ...pin, image }, image, pin.platform).registry, registry);
	}
});

test("invariant invalid locks neither contact a registry nor create outputs", async (t) => {
	const f = await fixture(t);
	await writeFile(f.lock, JSON.stringify({ ...pin, platform: "linux/amd64" }));
	await assert.rejects(invoke({ ...f.values, img: join(f.root, "absent-img") }), (error) => /OCI_LOCK_MISMATCH/u.test(error.stderr));
	assert.equal(existsSync(f.values.output), false);
	assert.equal(existsSync(f.values.manifest), false);
});

test("invariant registry acquisition receives isolated anonymous credentials and no ambient environment", async (t) => {
	const f = await fixture(t);
	const capture = join(f.root, "captured.json");
	const fake = join(f.root, "img");
	await writeFile(fake, `#!${process.execPath}\nconst fs = require('node:fs'); fs.writeFileSync(${JSON.stringify(capture)}, JSON.stringify({cwd:process.cwd(), configDir:fs.realpathSync(process.env.DOCKER_CONFIG), env:process.env, config:fs.readFileSync(process.env.DOCKER_CONFIG+'/config.json','utf8')})); process.exit(42);\n`);
	await chmod(fake, 0o755);
	await assert.rejects(invoke({ ...f.values, img: fake }, { ...process.env, IMG_REGISTRY_AUTH_PASSWORD: "fixture-password",
		GOOGLE_APPLICATION_CREDENTIALS: "/fixture/secret", AWS_SECRET_ACCESS_KEY: "fixture-key", IMG_INSECURE: "1" }),
	(error) => /OCI_PULL_FAILED/u.test(error.stderr));
	const captured = JSON.parse(await readFile(capture, "utf8"));
	assert.equal(captured.env.IMG_REGISTRY_AUTH_PASSWORD, undefined);
	assert.equal(captured.env.AWS_SECRET_ACCESS_KEY, undefined);
	assert.equal(captured.env.GOOGLE_APPLICATION_CREDENTIALS, "/dev/null");
	assert.equal(captured.env.AWS_EC2_METADATA_DISABLED, "true");
	assert.equal(captured.env.AWS_ECR_DISABLE_CACHE, "true");
	assert.equal(captured.env.IMG_INSECURE, "0");
	assert.deepEqual(JSON.parse(captured.config), {});
	assert.equal(captured.cwd, captured.configDir);
	assert.equal(existsSync(captured.env.DOCKER_CONFIG), false);
	assert.equal(existsSync(f.values.output), false);
});

/** Serve deterministic registry bytes while delegating conversion to the real pinned encoder. */
async function imageFixture(t, { docker = false, corruptConfig = false, corruptLayer = false, extraManifest = {} } = {}) {
	const f = await fixture(t);
	const hash = (bytes) => `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
	const tar = Buffer.alloc(1024), compressed = gzipSync(tar, { level: 1 });
	const config = Buffer.from(JSON.stringify({ os: "linux", architecture: "arm64", rootfs: { type: "layers", diff_ids: [hash(tar)] },
		config: { Env: ["A=B"], Healthcheck: { Test: ["CMD", "true"] }, Extension: { retained: true } }, history: [{ created_by: "fixture" }] }));
	const manifest = { schemaVersion: 2,
		mediaType: docker ? "application/vnd.docker.distribution.manifest.v2+json" : "application/vnd.oci.image.manifest.v1+json",
		config: { mediaType: docker ? "application/vnd.docker.container.image.v1+json" : "application/vnd.oci.image.config.v1+json", digest: hash(config), size: config.length },
		layers: [{ mediaType: docker ? "application/vnd.docker.image.rootfs.diff.tar.gzip" : "application/vnd.oci.image.layer.v1.tar+gzip", digest: hash(compressed), size: compressed.length }], ...extraManifest };
	const bytes = Buffer.from(JSON.stringify(manifest));
	const source = { ...pin, manifest_digest: hash(bytes) };
	await writeFile(f.lock, JSON.stringify(source));
	const fake = join(f.root, "img");
	const blobs = { [manifest.config.digest.slice(7)]: (corruptConfig ? Buffer.from("{}") : config).toString("base64"),
		[manifest.layers[0].digest.slice(7)]: (corruptLayer ? Buffer.from("bad layer") : compressed).toString("base64") };
	await writeFile(fake, `#!${process.execPath}\nconst fs=require('node:fs'), path=require('node:path'), cp=require('node:child_process');
const command=process.argv[2], output=process.argv[process.argv.indexOf('--output')+1];
if(command==='manifest') { const result=cp.spawnSync(${JSON.stringify(process.env.BSMR_OCI_IMG ?? "missing-encoder")},process.argv.slice(2),{stdio:'inherit'}); process.exit(result.status??1); }
if(command==='download-manifest') { fs.mkdirSync(path.dirname(output),{recursive:true}); fs.writeFileSync(output,Buffer.from(${JSON.stringify(bytes.toString("base64"))},'base64')); }
else if(command==='pull') { const dir=path.join(output,'blobs/sha256'); fs.mkdirSync(dir,{recursive:true}); for(const [name,data] of Object.entries(${JSON.stringify(blobs)})) fs.writeFileSync(path.join(dir,name),Buffer.from(data,'base64')); }
else process.exit(99);\n`);
	await chmod(fake, 0o755);
	return { ...f, values: { ...f.values, img: fake }, source, manifest, config, compressed };
}

test("invariant Docker schema 2 normalization preserves exact config and compressed content identities", { skip: !process.env.BSMR_OCI_IMG }, async (t) => {
	const f = await imageFixture(t, { docker: true });
	await invoke(f.values);
	const metadata = await imageMetadata({ platform: pin.platform, ...f.values });
	assert.notEqual(metadata.descriptor.digest, f.source.manifest_digest);
	assert.equal(metadata.manifest.mediaType, "application/vnd.oci.image.manifest.v1+json");
	assert.equal(metadata.manifest.config.mediaType, "application/vnd.oci.image.config.v1+json");
	assert.equal(metadata.manifest.config.digest, f.manifest.config.digest);
	assert.deepEqual(await readFile(f.values.config), f.config);
	assert.equal(metadata.manifest.layers[0].digest, f.manifest.layers[0].digest);
	assert.equal(metadata.manifest.layers[0].mediaType, "application/vnd.oci.image.layer.v1.tar+gzip");
	assert.deepEqual(await readFile(join(f.values.output, "blobs/sha256", f.manifest.layers[0].digest.slice(7))), f.compressed);
});

test("invariant corrupt Docker content and unsupported metadata cannot produce an imported image", { skip: !process.env.BSMR_OCI_IMG }, async (t) => {
	for (const [options, code] of [[{ corruptConfig: true }, "OCI_DIGEST_MISMATCH"],
		[{ corruptLayer: true }, "OCI_INVALID_LAYER"], [{ extraManifest: { subject: {} } }, "OCI_INVALID_LOCK"]]) {
		const f = await imageFixture(t, { docker: true, ...options });
		await assert.rejects(invoke(f.values), (error) => error.stderr.startsWith(code + ":"));
		assert.equal(existsSync(f.values.output), false);
		assert.equal(existsSync(f.values.descriptor), false);
	}
});

test("invariant a pull never overwrites an existing output directory", async (t) => {
	const f = await fixture(t);
	await mkdir(f.values.output);
	await writeFile(join(f.values.output, "owned"), "existing\n");
	await assert.rejects(invoke(f.values), (error) => /EEXIST/u.test(error.stderr));
	assert.equal(await readFile(join(f.values.output, "owned"), "utf8"), "existing\n");
});

test("invariant unverified manifests and index pins never reach layer acquisition", async (t) => {
	for (const mismatch of [false, true]) {
		const f = await fixture(t);
		const bytes = JSON.stringify({ schemaVersion: 2, mediaType: "application/vnd.oci.image.index.v1+json", manifests: [] });
		const digest = `sha256:${createHash("sha256").update(bytes).digest("hex")}`;
		await writeFile(f.lock, JSON.stringify({ ...pin, manifest_digest: mismatch ? pin.manifest_digest : digest }));
		const fake = join(f.root, "img");
		const count = join(f.root, "calls");
		await writeFile(fake, `#!${process.execPath}\nconst fs=require('node:fs'), path=require('node:path'); const output=process.argv[process.argv.indexOf('--output')+1]; fs.appendFileSync(${JSON.stringify(count)},process.argv[2]+'\\n'); fs.mkdirSync(path.dirname(output),{recursive:true}); fs.writeFileSync(output,${JSON.stringify(bytes)});\n`);
		await chmod(fake, 0o755);
		await assert.rejects(invoke({ ...f.values, img: fake }), (error) => error.stderr.startsWith(mismatch ? "OCI_DIGEST_MISMATCH:" : "OCI_DIRECT_MANIFEST_REQUIRED:"));
		assert.equal(await readFile(count, "utf8"), "download-manifest\n");
		assert.equal(existsSync(f.values.output), false);
	}
});

test("invariant a real public digest pull produces a verified complete layout", { skip: process.env.BSMR_OCI_PULL_TEST !== "1" }, async (t) => {
	const f = await fixture(t);
	await invoke(f.values);
	const metadata = await imageMetadata({ platform: pin.platform, ...f.values });
	assert.equal(metadata.descriptor.digest, pin.manifest_digest);
	assert.equal(metadata.config.architecture, "arm64");
	assert.equal(JSON.parse(await readFile(join(f.values.output, "oci-layout"), "utf8")).imageLayoutVersion, "1.0.0");
	assert.deepEqual(JSON.parse(await readFile(join(f.values.output, "index.json"), "utf8")).manifests, [metadata.descriptor]);
});

test("invariant public ECR and GCR direct manifests acquire without ambient cloud authority", { skip: process.env.BSMR_OCI_PULL_TEST !== "1" }, async (t) => {
	for (const selected of [
		{ ...pin, image: "public.ecr.aws/docker/library/debian:bookworm-slim" },
		{ ...pin, image: "gcr.io/distroless/base-debian12:latest", manifest_digest: "sha256:94d5278016c07e4e328da0a983823b92a2341df326d59de3408b8c6fd50bb4b3" },
	]) {
		const f = await fixture(t, selected);
		await invoke(f.values, { ...process.env, GOOGLE_APPLICATION_CREDENTIALS: "/never-read-google-credentials", AWS_SECRET_ACCESS_KEY: "never-use-aws-authority" });
		const metadata = await imageMetadata({ platform: selected.platform, ...f.values });
		assert.equal(metadata.descriptor.digest, selected.manifest_digest);
	}
});
