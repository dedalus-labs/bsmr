//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Canonicalizes installed Python environments for differential conformance.

import { createHash } from "node:crypto";
import { lstatSync, readFileSync, readdirSync } from "node:fs";
import { join, relative, sep } from "node:path";

interface DistributionIdentity {
	entryPoints: readonly string[];
	name: string;
	tags: readonly string[];
	version: string;
}

interface FileIdentity {
	digest: string;
	executable: boolean;
}

export interface EnvironmentSnapshot {
	distributions: readonly DistributionIdentity[];
	files: Readonly<Record<string, FileIdentity>>;
}

/** Returns one canonical distribution name under the Python packaging contract. */
const normalizeName = (name: string): string => {
	const normalized = name.toLowerCase().replaceAll(/[-_.]+/g, "-");
	if (!/^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/.test(normalized)) throw new Error(`invalid distribution name '${name}'`);
	return normalized;
};

/** Reads one required single-line core-metadata field. */
const metadataField = (metadata: string, name: string): string => {
	const prefix = `${name}:`;
	const headers = metadata.split(/\r?\n\r?\n/, 1)[0]!;
	const values = headers.split("\n").filter((line) => line.startsWith(prefix)).map((line) => line.slice(prefix.length).trim());
	if (values.length !== 1 || values[0] === "") throw new Error(`metadata must contain exactly one ${name}`);
	return values[0]!;
};

/** Parses entry-point declarations into an order-independent identity. */
const entryPoints = (contents: string): readonly string[] => {
	let group: string | undefined;
	const entries: string[] = [];
	for (const source of contents.split("\n")) {
		const line = source.trim();
		if (line === "" || line.startsWith("#")) continue;
		const heading = /^\[([^\]]+)\]$/.exec(line);
		if (heading) {
			group = heading[1]!;
			continue;
		}
		const separator = line.indexOf("=");
		if (!group || separator < 1 || separator === line.length - 1) throw new Error(`invalid entry point '${line}'`);
		entries.push(`${group}:${line.slice(0, separator).trim()}=${line.slice(separator + 1).trim()}`);
	}
	return entries.sort();
};

/** Removes only the absolute interpreter prefix that BSMR intentionally relocates. */
const normalizedPayload = (path: string, contents: Buffer): Buffer => {
	if (!path.startsWith("bin/") || !contents.subarray(0, 2).equals(Buffer.from("#!"))) return contents;
	const newline = contents.indexOf(0x0a);
	if (newline < 0) throw new Error(`entry point '${path}' has an unterminated shebang`);
	let body = contents.subarray(newline + 1);
	if (body.subarray(0, 9).equals(Buffer.from("'''exec' "))) {
		const terminator = Buffer.from("\n' '''\n");
		const end = body.indexOf(terminator);
		if (end < 0) throw new Error(`entry point '${path}' has an unterminated uv trampoline`);
		body = body.subarray(end + terminator.length);
	}
	return Buffer.concat([Buffer.from("#!/usr/bin/env python3\n"), body]);
};

/** Returns whether installer state is irrelevant to the imported artifact. */
const isInstallerState = (path: string): boolean =>
	path === ".lock"
	|| path.endsWith(".pyc")
	|| path.split("/").includes("__pycache__")
	|| /\.dist-info\/(?:INSTALLER|RECORD|REQUESTED|direct_url\.json|uv_build\.json|uv_cache\.json)$/.test(path);

/** Walks an installation tree while rejecting symlinks and special files. */
const installedFiles = (root: string): readonly string[] => {
	const files: string[] = [];
	const visit = (directory: string): void => {
		for (const name of readdirSync(directory).sort()) {
			const absolute = join(directory, name);
			const path = relative(root, absolute).split(sep).join("/");
			const metadata = lstatSync(absolute);
			if (metadata.isSymbolicLink()) throw new Error(`unsupported symbolic link '${path}'`);
			if (metadata.isDirectory()) visit(absolute);
			else if (metadata.isFile()) files.push(path);
			else throw new Error(`unsupported filesystem object '${path}'`);
		}
	};
	visit(root);
	return files;
};

/** Describes each installed distribution using standardized wheel metadata. */
const distributions = (root: string): readonly DistributionIdentity[] => readdirSync(root)
	.filter((name) => name.endsWith(".dist-info") && lstatSync(join(root, name)).isDirectory())
	.map((directory) => {
		const metadata = readFileSync(join(root, directory, "METADATA"), "utf8");
		const wheel = readFileSync(join(root, directory, "WHEEL"), "utf8");
		const entryPointPath = join(root, directory, "entry_points.txt");
		let declaredEntryPoints: readonly string[] = [];
		try {
			declaredEntryPoints = entryPoints(readFileSync(entryPointPath, "utf8"));
		} catch (error) {
			if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
		}
		return {
			entryPoints: declaredEntryPoints,
			name: normalizeName(metadataField(metadata, "Name")),
			tags: wheel.split("\n").filter((line) => line.startsWith("Tag:")).map((line) => line.slice(4).trim()).sort(),
			version: metadataField(metadata, "Version"),
		};
	})
	.sort((left, right) => left.name.localeCompare(right.name) || left.version.localeCompare(right.version));

/** Produces the semantic, content-addressed identity of one Python installation. */
export const snapshotEnvironment = (root: string): EnvironmentSnapshot => {
	const files: Record<string, FileIdentity> = {};
	for (const path of installedFiles(root)) {
		if (isInstallerState(path)) continue;
		const absolute = join(root, ...path.split("/"));
		const contents = normalizedPayload(path, readFileSync(absolute));
		files[path] = {
			digest: createHash("sha256").update(contents).digest("hex"),
			executable: (lstatSync(absolute).mode & 0o111) !== 0,
		};
	}
	return { distributions: distributions(root), files };
};

/** Returns stable leaf paths whose values differ between two JSON-like values. */
const differingPaths = (left: unknown, right: unknown, path: string): readonly string[] => {
	if (Object.is(left, right)) return [];
	if (typeof left !== "object" || left === null || typeof right !== "object" || right === null) return [path];
	const leftRecord = left as Record<string, unknown>;
	const rightRecord = right as Record<string, unknown>;
	const keys = [...new Set([...Object.keys(leftRecord), ...Object.keys(rightRecord)])].sort();
	return keys.flatMap((key) => differingPaths(leftRecord[key], rightRecord[key], path ? `${path}.${key}` : key));
};

/** Reports every semantic mismatch between uv and BSMR environment snapshots. */
export const compareSnapshots = (uv: EnvironmentSnapshot, bsmr: EnvironmentSnapshot): readonly string[] =>
	differingPaths(uv, bsmr, "");
