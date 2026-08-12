//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Renders and validates provenance-aware source license preambles.

import { basename, dirname, extname } from "node:path";

import type { Provenance } from "./license-provenance.ts";

const copyright = "Copyright (c) 2026 Dedalus Labs, Inc. and its contributors";
const apacheSpdx = "SPDX-License-Identifier: Apache-2.0";

type CommentStyle = Readonly<{ prefix: string; suffix: string }>;
export type Source = Readonly<{ path: string; provenance: Provenance; text: string }>;

/** Return the language-native single-line comment syntax for a source path. */
function commentStyle(path: string): CommentStyle {
	const extension = extname(path);
	if ([".ml", ".mli", ".mll"].includes(extension)) return { prefix: "(* ", suffix: " *)" };
	if (extension === ".mly") return { prefix: "/* ", suffix: " */" };
	if ([".html", ".md"].includes(extension)) return { prefix: "<!-- ", suffix: " -->" };
	if (extension === ".hs") return { prefix: "-- ", suffix: "" };
	if ([".erl", ".hrl"].includes(extension)) return { prefix: "% ", suffix: "" };
	if (extension === ".bat") return { prefix: "@REM ", suffix: "" };
	if ([".c", ".cc", ".cjs", ".cpp", ".go", ".h", ".hpp", ".java", ".js", ".jsx", ".kt", ".kts", ".m", ".mjs", ".proto", ".rs", ".ts", ".tsx"].includes(extension)) {
		return { prefix: "// ", suffix: "" };
	}
	return { prefix: "# ", suffix: "" };
}

/** Describe source that lacked a first-party responsibility comment. */
function brief(path: string): string {
	if (basename(path) === "BUILD.bsmr") return `Defines build targets for ${dirname(path) === "." ? "the root" : dirname(path)}.`;
	const known = new Map([
		[".bsmr", "Configures the root Bessemer cell."],
		[".github/actions/ci/rust-affected/dist/index.js", "Runs the generated Rust affected-paths action."],
		[".github/actions/ci/rust-affected/src/index.ts", "Implements the Rust affected-paths action."],
		[".github/pull_request_template.md", "Defines the repository pull request template."],
		["AGENTS.md", "Directs coding agents working in Bessemer."],
		["CLA.md", "Defines the contributor license agreement."],
		["SECURITY.md", "Documents the repository security policy."],
		["STYLE.md", "Defines Bessemer's source and engineering conventions."],
		["ci/affected.ts", "Determines whether a change requires Rust CI."],
		["ci/ci.test.ts", "Verifies the generated CI workflow contract."],
		["ci/ci.ts", "Defines Bessemer's generated CI workflow."],
		["ci/cli.test.ts", "Verifies CLI command selection, ordering, and fail-fast execution."],
		["ci/cli.ts", "Routes repository CI commands through Hollywood's typed process executor."],
		["ci/contributors.ts", "Defines the generated contributor-validation workflow."],
		["ci/license-preamble.test.ts", "Verifies native comment rendering and parser-directive preservation."],
		["ci/license-preamble.ts", "Renders and validates provenance-aware source license preambles."],
		["ci/license-provenance.test.ts", "Verifies source selection and fork-boundary provenance classification."],
		["ci/license-provenance.ts", "Derives source ownership from Bessemer's immutable Buck2 fork boundary."],
		["ci/license.test.ts", "Verifies the license policy against an isolated source inventory."],
		["ci/license.ts", "Audits source provenance, legal preambles, and package license metadata."],
		["docs/getting_started/quickstart.md", "Shows the shortest path from project initialization to a successful build."],
		["docs/getting_started/what_is_bsmr.md", "Introduces Bessemer and its core capabilities."],
		["docs/reference/configuration.md", "Documents optional project configuration after the beginner workflow."],
		["docs/reference/index.md", "Directs readers to Bessemer's complete technical reference."],
		["prelude/toolchains/pnpm/runner.mjs", "Runs the generated hermetic pnpm install adapter."],
		["prelude/typescript/runner.mjs", "Runs the generated hermetic TypeScript action adapter."],
		["README.md", "Introduces Bessemer's interface, supported ecosystems, and development workflow."],
		["rolldown.config.ts", "Bundles Bessemer's local GitHub Actions."],
		["test/contributors.test.ts", "Verifies the vouched-contributor trust policy."],
		["tools/build/README.md", "Documents the self-hosted build cell."],
		["tools/rust-project/README.md", "Documents Rust project generation."],
		["UPSTREAM_CHANGELOG.md", "Records later upstream Buck2 integrations after Bessemer's initial fork."],
	]);
	const responsibility = known.get(path);
	if (responsibility === undefined) throw new Error(`missing source responsibility for ${path}`);
	return responsibility;
}

/** Render the legal block without a file-responsibility comment. */
function renderLegalPreamble(path: string, provenance: Exclude<Provenance, "upstream">): string {
	const style = commentStyle(path);
	const line = (value: string) => `${style.prefix}${value}${style.suffix}`;
	const separator = style.prefix === "// " ? "//===----------------------------------------------------------------------===//" : line("===----------------------------------------------------------------------===");
	const legal = provenance === "dedalus"
		? [copyright, apacheSpdx]
		: [`Modifications ${copyright}`, apacheSpdx];
	return [separator, ...legal.map(line), separator].join("\n");
}

/** Render one canonical preamble without altering any inherited notice. */
export function renderPreamble(path: string, provenance: Exclude<Provenance, "upstream">): string {
	const legal = renderLegalPreamble(path, provenance);
	if (provenance === "upstream-modified") return `${legal}\n\n`;
	const style = commentStyle(path);
	return `${legal}\n\n${style.prefix}${brief(path)}${style.suffix}\n\n`;
}

/** Preserve syntax that a parser requires before comments. */
function insertionOffset(path: string, text: string): number {
	if (text.startsWith("#!") || text.startsWith("# shellcheck") || text.startsWith("// @generated by Hollywood")) {
		return text.indexOf("\n") + 1;
	}
	if (extname(path) === ".html" && /^<!doctype html>\n/i.test(text)) return text.indexOf("\n") + 1;
	if (extname(path) === ".md" && text.startsWith("---\n")) {
		const closing = text.indexOf("\n---\n", 4);
		if (closing === -1) throw new Error(`${path}: unterminated Markdown frontmatter`);
		return closing + 5;
	}
	return 0;
}

/** Return a precise policy error for a source file, if any. */
export function validateSource(source: Source): string | undefined {
	if (source.provenance === "upstream") return undefined;
	const header = source.text.slice(insertionOffset(source.path, source.text), 4096);
	const legal = renderLegalPreamble(source.path, source.provenance);
	if (!header.startsWith(legal)) return `${source.path}: missing canonical ${source.provenance} preamble`;
	if (source.provenance === "dedalus") {
		const style = commentStyle(source.path);
		const responsibility = header.slice(legal.length + 2).split("\n", 1)[0] ?? "";
		const description = responsibility
			.slice(style.prefix.length, responsibility.length - style.suffix.length)
			.trim();
		if (!responsibility.startsWith(style.prefix) || !responsibility.endsWith(style.suffix) || description === "") {
			return `${source.path}: missing source responsibility`;
		}
	}
	return undefined;
}

/** Insert a preamble while preserving every pre-existing byte. */
export function insertPreamble(source: Source): string {
	if (source.provenance === "upstream" || validateSource(source) === undefined) return source.text;
	const offset = insertionOffset(source.path, source.text);
	const rendered = renderPreamble(source.path, source.provenance);
	const preamble = offset === source.text.length ? `${rendered.trimEnd()}\n` : rendered;
	return `${source.text.slice(0, offset)}${preamble}${source.text.slice(offset)}`;
}
