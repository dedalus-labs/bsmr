import { and, eq, format, github, job, workflow } from "@dedalus-labs/hollywood";

const saveRustCache = and(
	eq(github.eventName, "push"),
	eq(github.ref, "refs/heads/main"),
);
const osvScannerUrl =
	"https://github.com/google/osv-scanner/releases/download/v2.4.0/osv-scanner_linux_amd64";
const osvScannerSha256 =
	"15314940c10d26af9c6649f150b8a47c1262e8fc7e17b1d1029b0e479e8ed8a0";
const dotSlashUrl =
	"https://github.com/facebook/dotslash/releases/download/v0.5.9/dotslash-linux-musl.x86_64.v0.5.9.tar.gz";
const dotSlashSha256 =
	"4c75c6eb7890ae35993b962073f6d9bbe78b42b81a5691303ad70f63bfbf7196";

export const ci = workflow({
	name: "CI",
	on: {
		push: { branches: ["main"] },
		pull_request: {},
		workflow_dispatch: {},
	},
	permissions: {},
	concurrency: {
		group: format("{0}-{1}", github.workflow, github.ref),
		"cancel-in-progress": true,
	},
	env: {
		FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true,
	},
	jobs: {
		dependencies: job({
			name: "Dependency review",
			if: eq(github.eventName, "pull_request"),
			"runs-on": "ubuntu-24.04",
			"timeout-minutes": 10,
			permissions: { contents: "read" },
			steps: [
				{
					name: "Checkout",
					uses: "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
					with: { "persist-credentials": false },
				},
				{
					name: "Review dependencies",
					uses: "actions/dependency-review-action@a1d282b36b6f3519aa1f3fc636f609c47dddb294",
					with: { "fail-on-severity": "high" },
				},
			],
		}),
		workflows: job({
			name: "Generated workflows",
			"runs-on": "ubuntu-24.04",
			"timeout-minutes": 10,
			permissions: { contents: "read" },
			steps: [
				{
					name: "Checkout",
					uses: "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
					with: { "persist-credentials": false },
				},
				{
					name: "Set up Node",
					uses: "actions/setup-node@249970729cb0ef3589644e2896645e5dc5ba9c38",
					with: {
						"node-version": "24.18.0",
					},
				},
				{ name: "Enable pnpm", run: "corepack enable" },
				{
					name: "Install dependencies",
					run: "pnpm install --frozen-lockfile --ignore-scripts",
				},
				{ name: "Audit dependencies", run: "pnpm audit --audit-level high" },
				{ name: "Check workflow source", run: "pnpm run ci:check" },
			],
		}),
		rust: job({
			name: "Rust",
			"runs-on": "ubuntu-24.04",
			"timeout-minutes": 60,
			permissions: { contents: "read" },
			env: {
				CARGO_INCREMENTAL: "0",
			},
			steps: [
				{
					name: "Checkout",
					uses: "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
					with: { "persist-credentials": false },
				},
				{
					name: "Install pinned Rust toolchain",
					run: "rustup toolchain install nightly-2026-04-11 --profile minimal --component clippy --component llvm-tools-preview --component rust-src --no-self-update",
				},
				{
					name: "Restore Rust cache",
					uses: "Swatinem/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32",
					with: {
						"prefix-key": "bsmr-v1",
						"save-if": saveRustCache,
					},
				},
				{
					name: "Install pinned OSV Scanner",
					run: [
						`curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error ${osvScannerUrl} --output "$RUNNER_TEMP/osv-scanner"`,
						`echo "${osvScannerSha256}  $RUNNER_TEMP/osv-scanner" | sha256sum --check`,
						'chmod 500 "$RUNNER_TEMP/osv-scanner"',
					].join("\n"),
				},
				{
					name: "Audit Rust dependencies",
					run: [
						'"$RUNNER_TEMP/osv-scanner" scan source --lockfile Cargo.lock --lockfile tools/build/third-party/rust/Cargo.lock --no-resolve --format json --output-file "$RUNNER_TEMP/osv.json" . || [ "$?" -eq 1 ]',
						"jq -e '[.results[].packages[].vulnerabilities[] | select(any(.affected[]; .database_specific.informational? != \"unmaintained\"))] as $v | if ($v | length) == 0 then true else ($v | map({id, summary})), false end' \"$RUNNER_TEMP/osv.json\"",
					].join("\n"),
				},
				{
					name: "Build Bessemer",
					run: "cargo build --locked --bin bsmr",
				},
				{
					name: "Install pinned DotSlash",
					run: [
						`curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error ${dotSlashUrl} --output "$RUNNER_TEMP/dotslash.tar.gz"`,
						`echo "${dotSlashSha256}  $RUNNER_TEMP/dotslash.tar.gz" | sha256sum --check`,
						'tar -xzf "$RUNNER_TEMP/dotslash.tar.gz" -C "$RUNNER_TEMP"',
						'echo "$RUNNER_TEMP" >> "$GITHUB_PATH"',
					].join("\n"),
				},
				{
					name: "Generate Rust build dependencies",
					run: "./tools/bin/reindeer --third-party-dir tools/build/third-party/rust buckify",
				},
				{
					name: "Run upstream Rust checks",
					run: "python3 test.py --ci --git --bsmr=target/debug/bsmr",
				},
				{
					name: "Validate self-host graph",
					run: "target/debug/bsmr --isolation-dir=ci uquery 'deps(//app/...)'\ntarget/debug/bsmr --isolation-dir=ci targets 'bsmr_build//...'",
				},
			],
		}),
	},
});
