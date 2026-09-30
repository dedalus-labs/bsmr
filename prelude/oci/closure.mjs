//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies OCI metadata and exports complete independent blob closures.

import { createHash } from "node:crypto";
import { constants, createWriteStream } from "node:fs";
import { lstat, mkdir, open, rm, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { Transform, Writable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { createGunzip } from "node:zlib";

const manifestType = "application/vnd.oci.image.manifest.v1+json";
const indexType = "application/vnd.oci.image.index.v1+json";
const configType = "application/vnd.oci.image.config.v1+json";
const tarType = "application/vnd.oci.image.layer.v1.tar";
const gzipType = `${tarType}+gzip`;
const digestPattern = /^sha256:[0-9a-f]{64}$/u;
const metadataLimit = 16 * 1024 * 1024;

/** Name the failing contract for callers and structured action diagnostics. */
function fail(code, message) {
	const error = new Error(message);
	error.name = code;
	error.code = code;
	throw error;
}

/** Require a JSON object rather than accepting arrays or null as metadata. */
function object(value, name) {
	if (value === null || typeof value !== "object" || Array.isArray(value)) fail("OCI_INVALID_METADATA", `${name} must be an object`);
	return value;
}

/** Decode metadata while preserving its original bytes for OCI identities. */
function json(bytes, name) {
	try {
		return object(JSON.parse(bytes.toString("utf8")), name);
	} catch (error) {
		if (error instanceof SyntaxError) fail("OCI_INVALID_METADATA", `${name} is not valid JSON`);
		throw error;
	}
}

/** Return a qualified Linux platform with the canonical OCI architecture. */
function platformParts(platform) {
	if (platform === "linux/amd64") return { os: "linux", architecture: "amd64" };
	if (platform === "linux/arm64" || platform === "linux/arm64/v8") return { os: "linux", architecture: "arm64", variant: "v8" };
	fail("OCI_UNSUPPORTED_PLATFORM", "OCI platform must be linux/amd64 or linux/arm64[/v8]");
}

/** Compare an optional descriptor platform with the selected execution-independent target. */
function samePlatform(value, expected) {
	return value !== null && typeof value === "object" && !Array.isArray(value)
		&& value.os === expected.os && value.architecture === expected.architecture
		&& (value.variant === undefined || value.variant === "" || value.variant === expected.variant);
}

/** Reject ambiguous digest paths, unsafe lengths, and unsupported media types. */
function descriptor(value, types, name) {
	object(value, name);
	if (typeof value.digest !== "string" || !digestPattern.test(value.digest)
		|| !Number.isSafeInteger(value.size) || value.size < 0 || !types.includes(value.mediaType)) {
		fail("OCI_INVALID_DESCRIPTOR", `${name} requires a lowercase SHA-256 digest, safe size, and supported media type`);
	}
	return value;
}

/** Bind reads to a regular leaf while allowing build-system ancestor symlinks. */
async function regular(path) {
	let handle;
	try {
		handle = await open(path, constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK);
		if (!(await handle.stat()).isFile()) fail("OCI_INVALID_BLOB", "OCI inputs must be regular files, not directories or symlinks");
		return handle;
	} catch (error) {
		if (handle) await handle.close();
		if (error.code === "ENOENT") fail("OCI_MISSING_BLOB", "a referenced OCI metadata or blob file is missing");
		if (error.code === "ELOOP") fail("OCI_INVALID_BLOB", "OCI metadata and blob leaves must not be symlinks");
		throw error;
	}
}

/** Read one small regular metadata artifact without following its leaf symlink. */
async function bytes(path) {
	const handle = await regular(path);
	try {
		if ((await handle.stat()).size > metadataLimit) fail("OCI_METADATA_TOO_LARGE", "OCI metadata exceeds the qualified 16 MiB limit");
		const chunks = [];
		let length = 0;
		for await (const chunk of handle.createReadStream({ autoClose: false })) {
			length += chunk.length;
			if (length > metadataLimit) fail("OCI_METADATA_TOO_LARGE", "OCI metadata exceeds the qualified 16 MiB limit");
			chunks.push(chunk);
		}
		return Buffer.concat(chunks, length);
	} finally { await handle.close(); }
}

/** Check the exact bytes named by a descriptor before interpreting them. */
function verifyBytes(data, expected, name) {
	if (data.length !== expected.size || `sha256:${createHash("sha256").update(data).digest("hex")}` !== expected.digest) {
		fail("OCI_DIGEST_MISMATCH", `${name} does not match its declared SHA-256 digest and size`);
	}
}

/** Verify image identities and the positional relationship of layers to diffIDs. */
function image(platform, manifestBytes, configBytes, imageDescriptor) {
	const expected = platformParts(platform);
	descriptor(imageDescriptor, [manifestType], "image descriptor");
	verifyBytes(manifestBytes, imageDescriptor, "image manifest");
	const manifest = json(manifestBytes, "image manifest");
	if (manifest.schemaVersion !== 2 || manifest.mediaType !== manifestType || !Array.isArray(manifest.layers)) {
		fail("OCI_INVALID_METADATA", "image manifest must contain schemaVersion 2 and an ordered layers array");
	}
	descriptor(manifest.config, [configType], "image config descriptor");
	verifyBytes(configBytes, manifest.config, "image config");
	const config = json(configBytes, "image config");
	if (!samePlatform(config, expected) || (imageDescriptor.platform !== undefined && !samePlatform(imageDescriptor.platform, expected))) {
		fail("OCI_PLATFORM_MISMATCH", "image config or descriptor does not match the selected OCI platform");
	}
	if (config.rootfs?.type !== "layers" || !Array.isArray(config.rootfs.diff_ids)
		|| config.rootfs.diff_ids.length !== manifest.layers.length) {
		fail("OCI_INVALID_METADATA", "image config must provide one ordered rootfs diffID for every layer");
	}
	for (const [index, layer] of manifest.layers.entries()) {
		descriptor(layer, [tarType, gzipType], "image layer descriptor");
		if (typeof config.rootfs.diff_ids[index] !== "string" || !digestPattern.test(config.rootfs.diff_ids[index])) {
			fail("OCI_INVALID_METADATA", "rootfs diffIDs must be lowercase SHA-256 digests");
		}
	}
	return { manifest, config, descriptor: imageDescriptor };
}

/** Validate the small image artifacts without opening any layer payload. */
export async function imageMetadata({ platform, manifest, config, descriptor: descriptorPath }) {
	const [manifestBytes, configBytes, descriptorBytes] = await Promise.all([bytes(manifest), bytes(config), bytes(descriptorPath)]);
	return image(platform, manifestBytes, configBytes, json(descriptorBytes, "image descriptor"));
}

/** Validate img's layer metadata, retaining its native snake-case diff_id field. */
export async function layerMetadata(path) {
	const metadata = descriptor(json(await bytes(path), "layer metadata"), [tarType, gzipType], "layer metadata");
	if (typeof metadata.diff_id !== "string" || !digestPattern.test(metadata.diff_id)) {
		fail("OCI_INVALID_METADATA", "layer metadata requires a lowercase SHA-256 diff_id");
	}
	return metadata;
}

/** Address only the safe digest suffix already checked by descriptor validation. */
function blobPath(layout, value) {
	return join(layout, "blobs", "sha256", value.digest.slice("sha256:".length));
}

/** Stream and optionally copy exact compressed bytes while verifying the uncompressed diffID. */
async function verifyLayer(path, expected, diffID, destination) {
	const source = await regular(path);
	const compressed = createHash("sha256");
	const uncompressed = createHash("sha256");
	let size = 0;
	const output = destination === undefined ? undefined : createWriteStream(destination, { flags: "wx", mode: 0o644 });
	const tee = new Transform({
		transform(chunk, _encoding, next) {
			compressed.update(chunk);
			size += chunk.length;
			if (output) output.write(chunk, (error) => next(error, chunk));
			else next(null, chunk);
		},
		final(next) {
			if (output) output.end(next);
			else next();
		},
	});
	output?.on("error", (error) => tee.destroy(error));
	const sink = new Writable({ write(chunk, _encoding, next) { uncompressed.update(chunk); next(); } });
	try {
		const streams = [source.createReadStream({ autoClose: false }), tee];
		if (expected.mediaType === gzipType) streams.push(createGunzip());
		streams.push(sink);
		try { await pipeline(streams); } catch (error) {
			if (error.code?.startsWith("Z_")) fail("OCI_INVALID_LAYER", "a gzip layer is invalid or its checksum failed");
			throw error;
		}
		if (size !== expected.size || `sha256:${compressed.digest("hex")}` !== expected.digest) {
			fail("OCI_DIGEST_MISMATCH", "layer bytes do not match the declared SHA-256 digest and size");
		}
		if (`sha256:${uncompressed.digest("hex")}` !== diffID) fail("OCI_DIFFID_MISMATCH", "uncompressed layer bytes do not match the config diffID");
	} finally {
		output?.destroy();
		await source.close();
	}
}

/** Require the standard complete-layout marker before looking up digest files. */
async function layoutMarker(layout) {
	if (json(await bytes(join(layout, "oci-layout")), "OCI layout marker").imageLayoutVersion !== "1.0.0") {
		fail("OCI_INVALID_LAYOUT", "OCI layout requires imageLayoutVersion 1.0.0");
	}
	for (const path of [join(layout, "blobs"), join(layout, "blobs", "sha256")]) {
		let entry;
		try { entry = await lstat(path); } catch (error) {
			if (error.code === "ENOENT") fail("OCI_MISSING_BLOB", "the OCI layout is missing its declared blob directory");
			throw error;
		}
		if (!entry.isDirectory() || entry.isSymbolicLink()) {
			fail("OCI_INVALID_LAYOUT", "OCI blobs and algorithm directories must be real directories inside the layout");
		}
	}
}

/** Import one direct platform image after verifying all compressed and uncompressed content. */
export async function importLayout({ platform, layout, manifest, config, descriptor: descriptorPath }) {
	const expected = platformParts(platform);
	await layoutMarker(layout);
	const index = json(await bytes(join(layout, "index.json")), "OCI layout index");
	if (index.schemaVersion !== 2 || (index.mediaType !== undefined && index.mediaType !== indexType) || !Array.isArray(index.manifests)) {
		fail("OCI_INVALID_LAYOUT", "OCI layout index must have schemaVersion 2 and a manifests array");
	}
	const candidates = [];
	for (const entry of index.manifests) {
		descriptor(entry, [manifestType, indexType], "index entry");
		if (entry.mediaType === indexType) fail("OCI_NESTED_INDEX_UNSUPPORTED", "nested OCI indexes are not qualified");
		if (entry.platform === undefined || samePlatform(entry.platform, expected)) candidates.push(entry);
	}
	if (candidates.length !== 1) fail("OCI_PLATFORM_SELECTION", "OCI layout must contain exactly one unambiguous direct image for the requested platform");
	const selected = candidates[0];
	const manifestBytes = await bytes(blobPath(layout, selected));
	verifyBytes(manifestBytes, selected, "image manifest");
	const manifestValue = json(manifestBytes, "image manifest");
	descriptor(manifestValue.config, [configType], "image config descriptor");
	const configBytes = await bytes(blobPath(layout, manifestValue.config));
	const metadata = image(platform, manifestBytes, configBytes, selected);
	for (const [position, layer] of metadata.manifest.layers.entries()) {
		await verifyLayer(blobPath(layout, layer), layer, metadata.config.rootfs.diff_ids[position]);
	}
	await Promise.all([manifest, config, descriptorPath].map((path) => mkdir(dirname(path), { recursive: true })));
	await Promise.all([
		writeFile(manifest, manifestBytes, { flag: "wx" }),
		writeFile(config, configBytes, { flag: "wx" }),
		writeFile(descriptorPath, JSON.stringify(selected), { flag: "wx" }),
	]);
	return metadata;
}

/** Select a declared digest source; corrupt selected sources never try another provider. */
async function layerSource(value, native, layouts) {
	if (native.has(value.digest)) return native.get(value.digest).blob;
	for (const layout of layouts) {
		const path = blobPath(layout, value);
		try { await lstat(path); return path; } catch (error) {
			if (error.code !== "ENOENT") throw error;
		}
	}
	fail("OCI_MISSING_BLOB", "the complete export has no declared content for a referenced layer");
}

/** Export independently owned standard OCI bytes, rejecting missing or inconsistent content. */
export async function exportLayout({ platform, spec, output }) {
	object(spec, "layout specification");
	if (!Array.isArray(spec.layers) || !Array.isArray(spec.base_layouts)) fail("OCI_INVALID_METADATA", "layout specification requires layers and base_layouts arrays");
	const metadata = await imageMetadata({ platform, manifest: spec.manifest, config: spec.config, descriptor: spec.descriptor });
	const native = new Map();
	for (const placement of spec.layers) {
		object(placement, "layer placement");
		const layer = await layerMetadata(placement.metadata);
		const previous = native.get(layer.digest);
		if (previous && (previous.metadata.size !== layer.size || previous.metadata.mediaType !== layer.mediaType || previous.metadata.diff_id !== layer.diff_id)) {
			fail("OCI_LAYER_METADATA_MISMATCH", "the same digest has conflicting declared layer metadata");
		}
		native.set(layer.digest, { metadata: layer, blob: placement.blob });
	}
	for (const layout of spec.base_layouts) await layoutMarker(layout);
	const referenced = new Set(metadata.manifest.layers.map((layer) => layer.digest));
	for (const digest of native.keys()) if (!referenced.has(digest)) fail("OCI_LAYER_METADATA_MISMATCH", "a declared native layer is absent from the image manifest");
	await mkdir(output);
	try {
		const blobs = join(output, "blobs", "sha256");
		await mkdir(blobs, { recursive: true });
		const exported = new Map();
		for (const [position, layer] of metadata.manifest.layers.entries()) {
			const diffID = metadata.config.rootfs.diff_ids[position];
			const declared = native.get(layer.digest)?.metadata;
			if (declared && (declared.size !== layer.size || declared.mediaType !== layer.mediaType || declared.diff_id !== diffID)) {
				fail("OCI_LAYER_METADATA_MISMATCH", "native layer metadata does not match the image manifest and rootfs diffID");
			}
			if (exported.has(layer.digest)) {
				const previous = exported.get(layer.digest);
				if (previous.diffID !== diffID || previous.mediaType !== layer.mediaType || previous.size !== layer.size) {
					fail("OCI_LAYER_METADATA_MISMATCH", "repeated layer digest has conflicting type, size, or rootfs diffID");
				}
				continue;
			}
			await verifyLayer(await layerSource(layer, native, spec.base_layouts), layer, diffID, blobPath(output, layer));
			exported.set(layer.digest, { diffID, mediaType: layer.mediaType, size: layer.size });
		}
		const [manifestBytes, configBytes] = await Promise.all([bytes(spec.manifest), bytes(spec.config)]);
		verifyBytes(manifestBytes, metadata.descriptor, "image manifest");
		verifyBytes(configBytes, metadata.manifest.config, "image config");
		await Promise.all([
			writeFile(blobPath(output, metadata.descriptor), manifestBytes, { flag: "wx" }),
			writeFile(blobPath(output, metadata.manifest.config), configBytes, { flag: "wx" }),
			writeFile(join(output, "oci-layout"), '{"imageLayoutVersion":"1.0.0"}', { flag: "wx" }),
			writeFile(join(output, "index.json"), JSON.stringify({ schemaVersion: 2, mediaType: indexType, manifests: [metadata.descriptor] }), { flag: "wx" }),
		]);
	} catch (error) {
		await rm(output, { recursive: true, force: true });
		throw error;
	}
}
