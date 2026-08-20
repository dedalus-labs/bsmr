//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Synchronizes the product version across release and Cargo metadata.

import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const canonicalVersion = /^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)$/;

type VersionCarrier = Readonly<{
	path: string;
	pattern: RegExp;
	replacement: (version: string) => string;
}>;

const carriers: readonly VersionCarrier[] = [
	{
		path: "app/bsmr/Cargo.toml",
		pattern: /^name = "bsmr"\nversion = "\d+\.\d+\.\d+"$/m,
		replacement: (version) => `name = "bsmr"\nversion = "${version}"`,
	},
	{
		path: "Cargo.lock",
		pattern: /^\[\[package\]\]\nname = "bsmr"\nversion = "\d+\.\d+\.\d+"$/m,
		replacement: (version) => `[[package]]\nname = "bsmr"\nversion = "${version}"`,
	},
	{
		path: "dist-workspace.toml",
		pattern: /^version = "\d+\.\d+\.\d+"$/m,
		replacement: (version) => `version = "${version}"`,
	},
];

/**
 * Replace one version carrier and reject ambiguous metadata.
 *
 * @param contents - Complete file contents.
 * @param carrier - File-specific version contract.
 * @param version - Canonical product version.
 * @returns Updated file contents.
 */
function replaceVersion(contents: string, carrier: VersionCarrier, version: string): string {
	const matches = contents.match(new RegExp(carrier.pattern.source, carrier.pattern.flags.concat("g")));
	if (matches?.length !== 1) {
		throw new Error(`${carrier.path}: expected exactly one product version, found ${matches?.length ?? 0}`);
	}
	return contents.replace(carrier.pattern, carrier.replacement(version));
}

/**
 * Synchronize every derived product-version carrier with VERSION.
 *
 * @param root - Repository root.
 * @returns Paths changed by synchronization.
 */
export function synchronizeReleaseVersion(root: string): readonly string[] {
	const version = readFileSync(join(root, "VERSION"), "utf8").trim();
	if (!canonicalVersion.test(version)) throw new Error(`VERSION: invalid canonical version '${version}'`);

	const manifest = JSON.parse(readFileSync(join(root, ".release-please-manifest.json"), "utf8")) as Record<
		string,
		unknown
	>;
	if (manifest["."] !== version) {
		throw new Error(`.release-please-manifest.json: expected ${version}, found ${String(manifest["."])}`);
	}

	const changed: string[] = [];
	for (const carrier of carriers) {
		const path = join(root, carrier.path);
		const contents = readFileSync(path, "utf8");
		const updated = replaceVersion(contents, carrier, version);
		if (updated === contents) continue;
		writeFileSync(path, updated);
		changed.push(carrier.path);
	}
	return changed;
}
