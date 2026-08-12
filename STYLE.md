<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Defines Bessemer's source and engineering conventions. -->
# Bessemer style

Bessemer code should be obvious to inspect, operate, and extend.

## Source documentation

- Start every new Dedalus-owned source file with the repository preamble below.
- State the file's responsibility in one sentence below the legal header, after one blank line.
- Preserve existing Meta and third-party notices exactly. Claim Dedalus copyright only for Dedalus-authored work.
- Keep byte-identical Buck2 descendants with a file-local legal notice unchanged. Mark modified descendants with the Dedalus modifications copyright and Apache-2.0 SPDX identifier; never replace the original notice. Record the initial fork point once in `NOTICE`, not in every file.
- Treat source added after the Buck2 fork point as Dedalus-owned unless it retains a Meta copyright notice.
- Use the Dedalus-owned preamble for a wholly rewritten file only when no upstream expression or notice remains.
- Emit the same preamble from generators; never patch generated output by hand.
- Exclude behavioral inputs in `fixtures` and `*_data` directories and `*.golden` test outputs because comments may change the tested bytes.
- Run `pnpm run ci check license` after adding, renaming, or changing source files.
- Document every named function in new or materially modified code with the language's native documentation syntax.
- In TypeScript, use JSDoc with a summary and `@param`, `@returns`, and `@throws` when they clarify the contract.
- Anonymous callbacks and self-explanatory test bodies do not need narration.
- Comments explain invariants, policy, or non-obvious tradeoffs. Delete comments that merely restate the code.
- Reserve the boxed separator for file preambles; use normal headings for sections.

```text
//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Describe this file's single responsibility.
```

## Engineering

- Prefer the smallest correct change; deletions and reuse beat new abstractions.
- Keep control flow explicit, typed, and fail-fast. Do not add silent fallbacks.
- Test behavior and invariants, not implementation details.
- Preserve established local patterns unless this guide intentionally supersedes them.

## Documentation

- Lead with the user outcome and the shortest working example.
- Use short sentences, active voice, and direct technical English.
- Keep native package paths in beginner examples. Introduce labels and Starlark only in advanced reference pages.
- Put one concept on each page. Link to details instead of repeating them.
- State support and hermeticity boundaries exactly. Do not advertise planned behavior as available.
- Keep CLI reference derived from the real parser so commands, flags, and defaults cannot drift.
