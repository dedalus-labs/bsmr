import { action, pathInput, runGitHubAction, stringOutput } from "@dedalus-labs/hollywood/action-runtime";
import { value } from "./index.js";

await runGitHubAction(action({
  name: "read-input",
  description: "Read an action input from the execution directory.",
  inputs: { source: pathInput({ description: "Input file path." }) },
  outputs: { value: stringOutput({ description: "Build value and file contents." }) },
  run: async ({ input, fs }) => ({ value: `${value}:${(await fs.readText(input.source)).trim()}` }),
}));
