<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Defines the lifecycle, numbering, and review contract for Bessemer RFCs. -->

# Requests for Comments

RFCs record consequential Bessemer decisions: public interfaces, architecture,
security boundaries, build semantics, testing strategy, and project process.
They preserve the alternatives and evidence behind a direction, not merely its
eventual implementation.

New RFCs use a stable `NNNN-title.md` path copied from
[`template.md`](template.md). Existing unnumbered documents predate this process
and may remain in place until they are revised.

## States

| State | Meaning |
| --- | --- |
| `ideation` | The problem is scoped; the direction is not yet under review. |
| `discussion` | A pull request is open and linked in `discussion`. |
| `accepted` | The direction is approved; implementation may be incomplete. |
| `implemented` | The validation contract is proven and the RFC describes reality. |
| `abandoned` | The direction will not be pursued. |
| `superseded` | Another linked RFC replaces this one. |

Use `ideation` while drafting. Move an RFC to `discussion` when its pull request
opens, to `accepted` before that pull request merges, and to `implemented` only
with the evidence named in its validation section.

RFC pull requests use `.github/PULL_REQUEST_TEMPLATE/rfc.md`; the numbered RFC
remains the source of truth rather than duplicating its design in the PR body.

## Numbering

Choose the smallest unreserved four-digit number. A pull request containing the
new document reserves it. RFC 0001 remains reserved for the package-lockfile
design; its source must be recovered rather than reconstructed from discussion.

## Required content

Every RFC names its owner, state, discussion, and searchable labels. Its body
must:

1. state the problem and evidence;
2. bound goals and non-goals;
3. make the determination explicit;
4. compare viable alternatives and their tradeoffs;
5. cover user, compatibility, security, performance, operational, and economic
   consequences where relevant; and
6. define the proof and migration required to call the work implemented.

Open questions include attempted answers. A question without an attempted
answer is research still to do, not a decision for reviewers to make blindly.

## Writing

Use the shortest text that preserves the complete argument. Every paragraph
must add evidence, a constraint, a determination, a tradeoff, or validation.
Delete throat-clearing, repeated summaries, generic aspirations, and history
that does not explain a present constraint.

State one claim per sentence with concrete nouns and verbs. Distinguish measured
facts, assumptions, and proposed behavior. Define unfamiliar terms at first use
and place sources beside the claims they support. Prefer a table, contract, or
small example when prose would obscure an exact comparison.

Sparse writing may not omit failure behavior, viable alternatives, consequences,
or the evidence needed to change the RFC's state. There is no target length.

The originating pull request remains the discussion record after merge. Later
material changes use new pull requests against the same stable RFC path.

This process follows [Oxide's RFD lifecycle](https://rfd.shared.oxide.computer/rfd/0001)
while using state names that distinguish an accepted direction from implemented
software.
