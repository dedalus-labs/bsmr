import { defineConfig } from "rolldown";

export default defineConfig({
	input: "./.github/actions/ci/rust-affected/src/index.ts",
	platform: "node",
	transform: {
		define: { "import.meta.vitest": "undefined" },
		// GitHub Actions executes this bundle with the runtime declared in action.yml.
		target: "node24",
	},
	output: {
		file: ".github/actions/ci/rust-affected/dist/index.js",
		format: "esm",
		minify: true,
	},
});
