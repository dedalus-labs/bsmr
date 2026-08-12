---
name: writing
description: >-
  Write and revise Bessemer technical content in plain, direct English. Use for
  documentation, issues, pull requests, release notes, CLI text, error messages,
  and other public technical content.
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Defines the required writing style for Bessemer technical content. -->

# Writing

Write for a developer who needs an exact answer.

## Required style

- Use US English.
- Use active voice.
- Use present tense for current behavior.
- Use imperative verbs for instructions.
- Use sentence case for titles and headings.
- Put the main point in the first sentence.
- Put one idea in each sentence.
- Use short paragraphs. Start a new paragraph when the subject changes.
- Use simple words when they preserve the technical meaning.
- Use [ASD-STE100 Simplified Technical English](https://www.asd-ste100.org/)
  as additional guidance. Keep necessary technical terms. Define each uncommon
  term at first use and use it consistently.
- State the actor for each requirement.
- Use `must` for requirements, `can` for options, and `might` for possible outcomes.
- Use lists for parallel requirements, steps, and results.
- Make list items grammatically parallel.
- End complete sentences and list items with periods.
- Use exact commands, file names, versions, measurements, and error text when
  they are public and relevant.
- Distinguish measured results, expected results, and future work.

## Prohibited style

- Do not use aphorisms, metaphors, analogies, slogans, or rhetorical questions.
- Do not use humor, slang, idioms, or cultural references.
- Do not use marketing claims or superlatives.
- Do not use exclamation marks.
- Do not use emoji.
- Do not use `obviously`, `clearly`, `simply`, `easy`, `just`, or `very` to describe a task.
- Do not use `we believe`, `we feel`, `we hope`, or similar statements about intent.
- Do not use `let's` in instructions.
- Do not use filler such as `note that`, `it is worth noting`, `at this time`, or `in order to`.
- Do not use an em dash when a period or comma works.
- Do not repeat a requirement in the summary, body, and acceptance criteria.
- Do not describe internal implementation history unless readers need it to make a decision.

## Claims

- State a measured claim only when the method and result are available.
- State the test environment when it affects the result.
- Report all relevant failures and limitations.
- Do not generalize from one workload.
- Cite the primary source for an upstream status or behavior.
- Label an estimate as an estimate.
- Do not describe planned work as available behavior.

## Public content

- Treat names and data from private systems as confidential unless the user
  confirms that they are public.
- Do not publish private repository names, commit hashes, branch names, package
  names, service names, customer names, host names, dependency relationships, or
  incident details.
- Describe a private test input by its relevant technical properties.
- Publish the minimum evidence needed to support the conclusion.
- Use a public fixture when a reproducible example is required.
- Review links, code blocks, logs, and metadata for private identifiers before publishing.

## Structure

Use the repository template when one exists. Keep only sections that provide required information.

For a feature issue, use this order:

1. `Summary`: State the requested outcome and current status.
2. `Evidence`: State verified behavior, measurements, and limitations.
3. `Requirements`: State implementation requirements once.
4. `Acceptance criteria`: State observable completion conditions.
5. `Non-goals`: State nearby work that the issue excludes.
6. `References`: Link primary sources and public project records.

For a procedure, use this order:

1. State what the procedure does.
2. State prerequisites.
3. Provide numbered steps.
4. State the expected result.
5. Provide troubleshooting only for known failures.

## Revision process

1. Identify the document's single purpose.
2. Delete text that does not support that purpose.
3. Move the conclusion or requested action to the first paragraph.
4. Replace vague nouns and weak verbs with specific terms.
5. Split sentences that contain multiple requirements.
6. Remove repeated requirements and evidence.
7. Remove private identifiers from public content.
8. Verify every number, status statement, link, command, and version.
9. Check the prohibited-style list.
10. Read the result once for ambiguity.

## References

- [Google developer documentation style guide](https://developers.google.com/style/)
- [Google guidance for active voice](https://developers.google.com/style/voice)
- [Google guidance for voice and tone](https://developers.google.com/style/tone)
- [Google guidance for global audiences](https://developers.google.com/style/translation)
- [Google guidance for headings](https://developers.google.com/style/headings)
- [ASD-STE100 Simplified Technical English, Issue 9](https://www.asd-ste100.org/assets/files/ASD-STE100_ISSUE9.pdf)
