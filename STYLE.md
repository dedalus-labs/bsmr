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
- Keep byte-identical Buck2 descendants with a file-local legal notice unchanged. Add the canonical provenance header to descendants without one. Mark modified descendants with the upstream commit, Dedalus modifications copyright, and Apache-2.0 SPDX identifier; never replace the original notice.
- Treat source added after the Buck2 fork point as Dedalus-owned unless it retains a Meta copyright notice.
- Emit the same preamble from generators; never patch generated output by hand.
- Exclude behavioral inputs in `fixtures` and `*_data` directories because comments may change the tested bytes.
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
