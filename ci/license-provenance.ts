//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Derives source ownership from Bessemer's immutable Buck2 fork boundary.

import { basename, extname } from "node:path";

export const forkPoint = "1560aca2002865cd73d7cafb22c705cfb640b2bc";
export const upstreamSource = `Upstream-Source: facebook/buck2@${forkPoint}`;

const sourceExtensions = new Set([
	".bash", ".bat", ".bsmrconfig", ".bxl", ".bzl", ".c", ".cc", ".cjs", ".cpp",
	".erl", ".fish", ".go", ".h", ".hpp", ".hrl", ".hs", ".html", ".java", ".js", ".jsx",
	".kt", ".kts", ".m", ".md", ".mjs", ".mk", ".ml", ".mli", ".mll", ".mly", ".nix", ".proto", ".ps1",
	".py", ".pyi", ".rs", ".s", ".sh", ".star", ".ts", ".tsx", ".zsh",
]);
const sourceNames = new Set([".envrc", "BUILD.bsmr", "Dockerfile", "Makefile", "PACKAGE", "TARGETS"]);
const fixture = /(?:^|\/)(?:fixtures|golden|[^/]+_data)(?:\/|$)/;
const golden = /\.golden(?:\.|$)/;
const comment = "(?:\\/\\/|#|--|%|\\/\\*|\\*|<!--|@REM)";
const upstreamCopyright = new RegExp(`^\\s*(?:${comment}\\s*)?Copyright[^\\n]*(?:Meta Platforms|Facebook)`, "im");
const legalNotice = new RegExp(`^\\s*(?:${comment}\\s*)?(?:Copyright|SPDX-License-Identifier|Licensed under|This source code[^\\n]*licens)`, "im");
const upstreamMarker = new RegExp(`^(?://|#|--|%|\\(\\*|/\\*|<!--|@REM) ${upstreamSource}(?: \\*\\/| -->)?$`, "m");

export type Provenance = "dedalus" | "upstream" | "upstream-modified";
export type GitChange = Readonly<{ oldPath?: string; status: string }>;

/** Parse NUL-delimited `git diff --name-status` output by destination path. */
export function parseChanges(output: string): ReadonlyMap<string, GitChange> {
	const fields = output.split("\0");
	const changes = new Map<string, GitChange>();
	for (let index = 0; index < fields.length - 1;) {
		const status = fields[index++];
		if (status === undefined) throw new Error("git diff omitted a status");
		const oldPath = /^[RC]/.test(status) ? fields[index++] : undefined;
		const path = fields[index++];
		if (path === undefined) throw new Error(`git diff omitted the path after ${status}`);
		changes.set(path, oldPath === undefined ? { status } : { oldPath, status });
	}
	return changes;
}

/** Return whether a tracked path is source rather than fixture or data. */
export function isSource(path: string): boolean {
	// Golden outputs are compared byte-for-byte by test harnesses, so a preamble changes the tested bytes.
	if (fixture.test(path) || golden.test(basename(path))) return false;
	return sourceExtensions.has(extname(path)) || path.endsWith(".bsmrconfig") || sourceNames.has(basename(path));
}

/** Return whether a path is byte-identical to its upstream origin. */
export function isExact(change?: GitChange): boolean {
	return change === undefined || /^(?:R|C)100$/.test(change.status);
}

/** Derive the file's ownership boundary from the immutable Buck2 fork point. */
export function classify(text: string, change?: GitChange): Provenance {
	const header = text.slice(0, 4096);
	if (upstreamMarker.test(header)) return "upstream-modified";
	if (change?.status === "A" && !upstreamCopyright.test(header)) return "dedalus";
	if (isExact(change)) return legalNotice.test(header) ? "upstream" : "upstream-modified";
	return "upstream-modified";
}
