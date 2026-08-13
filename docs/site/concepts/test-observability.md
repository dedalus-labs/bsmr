---
description: Export test attempts, detect flakes, and place retries at the correct boundary.
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Defines the stable test-event contract and its CI integration boundary. -->

# Test observability

Bessemer can export completed test attempts without exposing its internal event
log. The stable stream is intended for CI summaries, flaky-test detection, and
external observability tools.

## Write a build-event stream

Pass an output path to `bsmr test`:

```console
bsmr test --build-event-jsonl build-events.jsonl //path/to:test
```

Bessemer writes one `bsmr.build.v1.BuildEvent` JSON object per line. It creates
the requested file even when no test attempt qualifies, and flushes the file to
durable storage before the command finishes.

The following record is formatted for readability. The artifact itself uses
JSON Lines.

```json
{
  "schema_version": 1,
  "invocation_id": "43be1e94-5a33-4a57-8cd9-4478375827c5",
  "sequence_number": 41,
  "event_time_unix_millis": 1786564418123,
  "payload": {
    "test_attempt_completed": {
      "test": {
        "target": "root//packages/auth:test",
        "configuration": "node26#release",
        "suite": "vitest",
        "case": "refreshes an expired token",
        "variant": null
      },
      "action_digest": "8d33c776f81e6dd5:18432",
      "attempt": 1,
      "outcome": "fail",
      "execution_kind": "remote",
      "duration_millis": 617,
      "message": "expected 200, received 401",
      "details_digest": {
        "algorithm": "blake3",
        "hash": "ab68d0f1b93b9335a04e231023af15e9ec9b5cfa446e150e786f5441c2755e0d",
        "size_bytes": 2841
      },
      "max_memory_used_bytes": 91226112
    }
  }
}
```

The logical test identity groups results by configured target, suite, case, and
variant. The opaque action digest identifies the command and declared inputs
that produced the result. Consumers may compare this digest for equality, but
must not parse its representation.

Verbose output remains in Bessemer's internal event log. The public record uses
a BLAKE3 digest so a consumer can group equivalent failure details without
copying logs into its historical index.

## Classify flakes from facts

A detector compares attempts rather than trusting one final job status:

| Observation | Classification |
| --- | --- |
| Same test and action digest fails, then passes | Strong test-flake signal |
| Failure correlates with `remote` but not `local` | Execution-environment signal |
| `infra_failure` or `timeout` without assertion failure | Infrastructure reliability signal |
| Action digest changes between outcomes | Inputs changed; lower flake confidence |

Within one invocation, increasing attempt numbers establish retry order. Across
invocations, CI metadata supplies the repository, commit, branch, workflow,
job, shard, and runner platform. Bessemer does not infer quarantine policy or
persist historical scores inside the build engine.

## Retry at the test boundary

The built-in open-source runner currently emits one attempt per test execution.
It does not yet retry failed tests, so `attempt` is currently `1` unless an
external runner coordinates retries.

Fine-grained test retries belong in Bessemer's test-runner boundary. Only that
layer can rerun the same test action while preserving logical identity, action
identity, executor provenance, and an increasing attempt number. A future
explicit retry count should default to zero.

Retry count and build verdict are separate policies. An eventual pass proves
that the test can pass; it does not erase the earlier failure. CI may treat that
sequence as a warning or a failure, but the event stream must retain every
attempt either way.

[GitHub workflow reruns](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/re-run-workflows-and-jobs)
remain useful for coarse recovery. They rerun a job or workflow at the same
commit, but they repeat setup and unrelated tests and do not replace native
test-attempt identity.

## GitHub-native public deployment

An open-source repository can operate the first version entirely on GitHub:

1. Run Bessemer on a standard GitHub-hosted runner.
2. Upload `build-events.jsonl` as a workflow artifact.
3. Use a later job or scheduled workflow to fetch recent artifacts through the
   [Actions artifact API](https://docs.github.com/en/rest/actions/artifacts) and
   calculate flake history.
4. Write the current result to the job summary or annotate the existing Actions
   check.
5. Optionally publish a static historical dashboard with
   [GitHub Pages](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages).

[Standard GitHub-hosted Actions runners are free for public repositories](https://docs.github.com/en/billing/concepts/product-billing/github-actions#free-use-of-github-actions).
[Public-repository artifacts can be retained for at most 90 days](https://docs.github.com/en/organizations/managing-organization-settings/configuring-the-retention-period-for-github-actions-artifacts-and-logs-in-your-organization).
Artifacts are files rather than a queryable database, Actions jobs are
ephemeral compute, and Pages is static hosting. This is enough for a bounded
public history without an ECR image, Lambda function, S3 bucket, or DynamoDB
table.

At larger scale, an external backend can index the same vendor-neutral events
for longer retention and efficient queries. That backend is optional and must
not change the Bessemer event contract.

Treat artifacts from forked pull requests as untrusted input. Aggregators must
enforce the protobuf-derived schema and size limits, reject malformed records,
and never execute repository code while holding write credentials.

## Current boundary

Version 1 emits completed test attempts only. Target, action, cache-transfer,
and invocation lifecycle events are future schema additions. JSONL is the first
transport; backend uploads and provider-specific adapters remain separate.

See the implementation and review history in
[pull request 73](https://github.com/dedalus-labs/bsmr/pull/73).
