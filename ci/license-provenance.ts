//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Derives source ownership from Bessemer's immutable Buck2 fork boundary.

import { basename, extname } from "node:path";

const forkCommit = /facebook\/buck2 commit\s+([0-9a-f]{40})/g;

const sourceExtensions = new Set([
	".bash", ".bat", ".bsmr", ".bxl", ".bzl", ".c", ".cc", ".cjs", ".cpp",
	".erl", ".fish", ".go", ".h", ".hpp", ".hrl", ".hs", ".html", ".java", ".js", ".jsx",
	".kt", ".kts", ".m", ".md", ".mjs", ".mk", ".ml", ".mli", ".mll", ".mly", ".nix", ".proto", ".ps1",
	".py", ".pyi", ".rs", ".s", ".sh", ".star", ".ts", ".tsx", ".zsh",
]);
const sourceNames = new Set([".envrc", "BUILD.bsmr", "Dockerfile", "Makefile", "PACKAGE", "TARGETS"]);
const fixture = /(?:^|\/)(?:fixtures|golden|[^/]+_data)(?:\/|$)/;
const golden = /\.golden(?:\.|$)/;
const comment = "(?:\\/\\/|#|--|%|\\/\\*|\\*|<!--|@REM)";
const upstreamCopyright = new RegExp(`^\\s*(?:${comment}\\s*)?Copyright[^\\n]*(?:Meta Platforms|Facebook)`, "im");
const dedalusCopyright = new RegExp(`^\\s*(?:${comment}\\s*)?Copyright \\(c\\) 2026 Dedalus Labs, Inc\\. and its contributors`, "im");
const modifiedCopyright = new RegExp(`^\\s*(?:${comment}\\s*)?Modifications Copyright \\(c\\) 2026 Dedalus Labs, Inc\\. and its contributors`, "im");
const legalNotice = new RegExp(`^\\s*(?:${comment}\\s*)?(?:Copyright|SPDX-License-Identifier|Licensed under|This source code[^\\n]*licens)`, "im");

export type Provenance = "dedalus" | "upstream" | "upstream-modified";
export type GitChange = Readonly<{ oldPath?: string; status: string }>;

/** Read the repository's one canonical Buck2 fork commit from NOTICE. */
export function parseForkPoint(notice: string): string {
	const matches = [...notice.matchAll(forkCommit)];
	if (matches.length !== 1 || matches[0]?.[1] === undefined) {
		throw new Error("NOTICE must record exactly one Buck2 fork commit");
	}
	return matches[0][1];
}

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
	return sourceExtensions.has(extname(path)) || path.endsWith(".bsmr") || sourceNames.has(basename(path));
}

/** Return whether a path is byte-identical to its upstream origin. */
export function isExact(change?: GitChange): boolean {
	return change === undefined || /^(?:R|C)100$/.test(change.status);
}

/** Derive the file's ownership boundary from the immutable Buck2 fork point. */
export function classify(text: string, change: GitChange | undefined, existedAtFork: boolean): Provenance {
	const header = text.slice(0, 4096);
	if (dedalusCopyright.test(header) && !upstreamCopyright.test(header)) return "dedalus";
	if (change?.status === "A" && !existedAtFork && !upstreamCopyright.test(header) && !modifiedCopyright.test(header)) return "dedalus";
	if (isExact(change)) return legalNotice.test(header) ? "upstream" : "upstream-modified";
	return "upstream-modified";
}
