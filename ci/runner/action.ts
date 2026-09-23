//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Owns approved runner dispatches and propagates each payload result exactly once.

import { action, choiceInput, stringInput, stringOutput } from "@dedalus-labs/hollywood/action-runtime";
import { z } from "zod";
import { RunnerApi, revision, providers } from "./api.ts";
import { cancelActiveRun, waitForRun } from "./lifecycle.ts";

export const runnerAction = action({
	name: "Own Rust build runner",
	description: "Authorize, dispatch, observe, and clean up one approved build attempt.",
	localActionPath: "runner/build",
	inputs: {
		operation: choiceInput({ description: "Run lifecycle operation.", options: ["authorize", "dispatch", "wait", "cleanup", "complete"] as const }),
		provider: choiceInput({ description: "Requested provider.", options: ["auto", ...providers] as const, default: "auto" }),
		source: stringInput({ description: "Exact source revision approved for this build.", default: "" }),
		definition: stringInput({ description: "Expected workflow definition revision.", default: "" }),
		actualDefinition: stringInput({ description: "Actual GitHub workflow revision.", default: "" }),
		repository: stringInput({ description: "GitHub repository receiving the dispatch.", default: "" }),
		event: stringInput({ description: "GitHub event name.", default: "" }),
		ref: stringInput({ description: "GitHub workflow ref.", default: "" }),
		actor: stringInput({ description: "GitHub triggering actor.", default: "" }),
		parent: stringInput({ description: "Owning workflow run ID.", default: "" }),
		run: stringInput({ description: "Child run ID returned by dispatch.", default: "" }),
		result: stringInput({ description: "Last provider result.", default: "" }),
	},
	outputs: {
		id: stringOutput({ description: "Owned child workflow run ID." }),
		url: stringOutput({ description: "Owned child workflow URL." }),
		result: stringOutput({ description: "Passed, or unassigned after confirmed cancellation." }),
	},
	run: async ({ input, log }) => {
		const api = new RunnerApi(process.env["GH_TOKEN"]);
		const empty = { id: "", url: "", result: "" };
		switch (input.operation) {
			case "authorize": {
				if (input.repository !== "dedalus-labs/bsmr" || !["workflow_dispatch", "push"].includes(input.event) || input.ref !== "refs/heads/main")
					throw new Error("runner builds require the reviewed main workflow");
				const source = revision.parse(input.source);
				const definition = revision.parse(input.actualDefinition);
				if (input.event === "push" && (source !== definition || input.provider !== "auto" || input.parent !== ""))
					throw new Error("a main push builds only its own revision");
				if (input.parent !== "") {
					if (input.provider === "auto" || revision.parse(input.definition) !== definition)
						throw new Error("child workflow definition changed before dispatch");
					await api.parent({ id: input.parent, definition, source });
				} else {
					await api.administrator(input.actor);
				}
				const commit = z.object({ sha: revision }).parse(await api.call(`commits/${source}`));
				if (commit.sha !== source) throw new Error("build source revision changed");
				return empty;
			}
			case "dispatch": {
				if (input.provider === "auto") throw new Error("dispatch requires one concrete provider");
				const run = await api.dispatch({ provider: input.provider, source: input.source, definition: input.definition, parent: input.parent });
				log.info(`Build attempt: ${run.url}`);
				return { ...empty, id: run.id, url: run.url };
			}
			case "wait": {
				const id = z.coerce.number().int().positive().parse(input.run);
				const result = await waitForRun(id, api.io(), { queueSeconds: input.provider === "github" ? 300 : 60, totalSeconds: 3600 });
				return { ...empty, id: input.run, result };
			}
			case "cleanup":
				if (input.run !== "") await cancelActiveRun(z.coerce.number().int().positive().parse(input.run), api.io());
				return empty;
			case "complete":
				if (input.result !== "passed") throw new Error("all eligible runner providers were unavailable");
				return empty;
		}
	},
});
