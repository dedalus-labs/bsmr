//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Audits source provenance, legal preambles, and package license metadata.

import { realpathSync, readFileSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { nodeExec, type ScriptExec } from "@dedalus-labs/hollywood";

import { insertPreamble, type Source, validateSource } from "./license-preamble.ts";
import { classify, forkPoint, type GitChange, isExact, isSource, parseChanges } from "./license-provenance.ts";

type Inventory = Readonly<{ changes: ReadonlyMap<string, GitChange>; sources: readonly Source[]; tracked: readonly string[] }>;
type CargoMetadata = Readonly<{ packages: readonly Readonly<{ license: string | null; manifest_path: string; source: string | null }>[] }>;

/** Build the complete source inventory with one bounded Git diff. */
async function inventory(root: string, exec: ScriptExec): Promise<Inventory> {
	await exec("git", ["cat-file", "-e", `${forkPoint}^{commit}`], { cwd: root });
	const [tracked, diff] = await Promise.all([
		exec("git", ["ls-files", "-z"], { cwd: root }),
		exec("git", ["-c", "diff.renameLimit=999999", "diff", "--name-status", "-z", "-M50%", "-C50%", "--find-copies-harder", `${forkPoint}..HEAD`, "--"], { cwd: root }),
	]);
	const changes = parseChanges(diff.stdout);
	const paths = tracked.stdout.split("\0").filter(Boolean);
	const sources = paths.filter(isSource).map((path) => {
		const text = readFileSync(join(root, path), "utf8");
		return { path, provenance: classify(text, changes.get(path)), text };
	});
	return { changes, sources, tracked: paths };
}

/** Validate canonical license files and every first-party package manifest. */
function validateProject(root: string, inventory_: Inventory, cargo: CargoMetadata): readonly string[] {
	const errors: string[] = [];
	if (readFileSync(join(root, "LICENSE"), "utf8") !== readFileSync(join(root, "LICENSE-APACHE"), "utf8")) {
		errors.push("LICENSE must match LICENSE-APACHE byte-for-byte");
	}
	for (const path of inventory_.tracked.filter((file) => basename(file) === "package.json")) {
		if (isExact(inventory_.changes.get(path))) continue;
		const manifest = JSON.parse(readFileSync(join(root, path), "utf8")) as { license?: string };
		if (manifest.license !== "Apache-2.0") errors.push(`${path}: first-party package license must be Apache-2.0`);
	}
	for (const package_ of cargo.packages.filter(({ source }) => source === null)) {
		if (package_.license === "Apache-2.0") continue;
		if (package_.manifest_path.endsWith("/packages/rust/superconsole/Cargo.toml") && package_.license === "MIT OR Apache-2.0") continue;
		errors.push(`${package_.manifest_path}: first-party crate license must be Apache-2.0`);
	}
	return errors;
}

/** Check or mechanically apply the complete source-license policy. */
export async function runLicensePolicy(mode: "apply" | "check", root: string, exec: ScriptExec): Promise<void> {
	const inventory_ = await inventory(root, exec);
	if (mode === "apply") {
		for (const source of inventory_.sources) {
			const updated = insertPreamble(source);
			if (updated !== source.text) writeFileSync(join(root, source.path), updated);
		}
		return;
	}
	const cargo = await exec("cargo", ["metadata", "--locked", "--no-deps", "--format-version", "1"], { cwd: root });
	const errors = inventory_.sources.map(validateSource).filter((error): error is string => error !== undefined);
	errors.push(...validateProject(root, inventory_, JSON.parse(cargo.stdout) as CargoMetadata));
	if (errors.length !== 0) throw new Error(`source license policy failed (${errors.length}):\n${errors.join("\n")}`);
}

/** Add canonical preambles to Hollywood's generated TypeScript entrypoints. */
export function licenseGeneratedEntrypoints(root: string): void {
	for (const path of [
		".github/actions/ci/cli-reference/src/index.ts",
		".github/actions/ci/osv-audit/src/index.ts",
		".github/actions/ci/release-sync/src/index.ts",
		".github/actions/ci/rust-affected/src/index.ts",
		".github/actions/ci/verify-sha256/src/index.ts",
	]) {
		const text = readFileSync(join(root, path), "utf8");
		writeFileSync(join(root, path), insertPreamble({ path, provenance: "dedalus", text }));
	}
}

/** Execute the requested license-policy mode. */
async function main(arguments_: readonly string[] = process.argv): Promise<number> {
	const mode = arguments_[2];
	if (mode !== "apply" && mode !== "check" && mode !== "generated") throw new Error("usage: node ci/license.ts <apply|check|generated>");
	const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
	if (mode === "generated") licenseGeneratedEntrypoints(root);
	else await runLicensePolicy(mode, root, nodeExec);
	return 0;
}

const invokedPath = process.argv[1];
if (invokedPath !== undefined && realpathSync(invokedPath) === fileURLToPath(import.meta.url)) {
	void main().then(
		(exitCode) => { process.exitCode = exitCode; },
		(error: unknown) => { console.error(error instanceof Error ? error.message : String(error)); process.exitCode = 1; },
	);
}
