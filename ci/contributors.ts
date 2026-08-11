//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Defines the generated contributor-validation workflow.

import { job, workflow } from "@dedalus-labs/hollywood";

export const checkContributor = String.raw`set -euo pipefail

node <<'NODE'
const fs = require("node:fs");

const author = process.env.PR_AUTHOR;
const check = process.env.CONTRIBUTOR_CHECK;
if (!author || (check !== "CLA" && check !== "Vouch")) {
	console.error("PR_AUTHOR and a valid CONTRIBUTOR_CHECK are required");
	process.exit(1);
}
if (!fs.existsSync("VOUCHED.td")) {
	console.error("VOUCHED.td is not present on the trusted base commit");
	process.exit(1);
}

const authorKey = "github:" + author.toLowerCase();
let vouched = false;
let denounced = null;
for (const rawLine of fs.readFileSync("VOUCHED.td", "utf8").split("\n")) {
	const line = rawLine.replace(/\r$/, "").trim();
	if (line === "" || line.startsWith("#")) {
		continue;
	}

	const [token, ...reasonParts] = line.split(/\s+/);
	const isDenounced = token.startsWith("-");
	const rawHandle = (isDenounced ? token.slice(1) : token).replace(/^@/, "");
	const handle = rawHandle.includes(":")
		? rawHandle.toLowerCase()
		: "github:" + rawHandle.toLowerCase();
	if (handle !== authorKey) {
		continue;
	}
	if (isDenounced) {
		denounced = reasonParts.join(" ") || "no reason recorded";
		break;
	}
	vouched = true;
}

const appendSummary = (body) => {
	if (process.env.GITHUB_STEP_SUMMARY) {
		fs.appendFileSync(process.env.GITHUB_STEP_SUMMARY, body + "\n");
	}
};
if (denounced !== null) {
	const message = "@" + author + " is denounced in VOUCHED.td: " + denounced;
	appendSummary("## " + check + " blocked\n\n" + message);
	console.error(message);
	process.exit(1);
}
if (vouched) {
	const message = check === "CLA"
		? "@" + author + " has accepted CLA.md according to VOUCHED.td"
		: "@" + author + " is listed in VOUCHED.td";
	appendSummary("## " + check + " passed\n\n" + message);
	console.log(message);
	process.exit(0);
}

appendSummary([
	"## " + check + " required",
	"",
	"@" + author + " is not listed in VOUCHED.td.",
	"",
	"Open a Vouch Request issue, accept CLA.md, and wait for a maintainer to add your handle.",
].join("\n"));
console.error("@" + author + " is not listed in VOUCHED.td");
process.exit(1);
NODE`.replaceAll("\t", "  ");

const checkoutTrustedBase = {
	name: "Checkout trusted base",
	uses: "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
	with: {
		ref: "${{ github.event.pull_request.base.sha }}",
		"persist-credentials": false,
	},
} as const;

const contributorJob = (check: "CLA" | "Vouch", needs?: string) =>
	job({
		name: check,
		"runs-on": "ubuntu-24.04",
		...(needs === undefined ? {} : { needs }),
		permissions: { contents: "read" },
		steps: [
			checkoutTrustedBase,
			{
				name: `Check ${check}`,
				env: {
					CONTRIBUTOR_CHECK: check,
					PR_AUTHOR: "${{ github.event.pull_request.user.login }}",
				},
				run: checkContributor,
			},
		],
	});

export const contributors = workflow({
	name: "Contributor Checks",
	on: {
		pull_request: {
			branches: ["main"],
			types: ["opened", "reopened", "synchronize", "ready_for_review"],
		},
	},
	permissions: { contents: "read" },
	jobs: {
		cla: contributorJob("CLA"),
		vouch: contributorJob("Vouch", "cla"),
	},
});
