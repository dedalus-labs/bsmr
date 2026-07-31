---
rfc: "0002"
title: "First-party Android rules"
authors: ["@windsornguyen"]
state: ideation
discussion: null
labels: [android, rules, toolchains]
---

# First-party Android rules

## Summary

Bessemer will support Android through a first-party `rules/android` module,
versioned independently from the build engine. Its first stable release will
build and test a small Kotlin application from a clean checkout on Linux and
macOS without Meta infrastructure.

The inherited Android prelude is implementation material, not a supported API.
We will migrate only code needed by the public contract and delete the rest.

## Context

An Android application combines JVM compilation, generated code, resources,
manifests, DEX generation, packaging, signing, and tests. It is a useful proof
of Bessemer's action graph, toolchains, caching, and test model.

The repository contains inherited Android rules, toolchains, and an example,
but they reference unavailable Meta targets and unfinished tests. Their
presence does not prove that Bessemer supports Android.

## Goals and non-goals

This RFC establishes a small, hermetic Android rule surface with familiar names
and measurable build behavior. Android remains independently versionable and
does not become an engine dependency.

This RFC does not provide full Bazel compatibility, Maven resolution, NDK
builds, device tests, App Bundles, IDE synchronization, code shrinking, dynamic
delivery, or exopackage. It does not choose Bessemer's build-language filename
extension.

## Determination

The first public surface is:

- `aar_import`
- `android_library`
- `android_binary`
- `android_local_test`

These names follow Bazel's `rules_android`; they are not a rename of Buck2:

| Public rule | Inherited implementation seed |
| --- | --- |
| `aar_import` | `android_prebuilt_aar` |
| `android_library` | `android_library` |
| `android_binary` | `android_binary` |
| `android_local_test` | `robolectric_test` |

Old names will not remain as aliases. Reused internals must be cleaned behind
the public contract.

The stable release includes Java and Kotlin sources, resources, manifest
processing, DEX generation, APK packaging, debug signing, AAR imports, and
Robolectric-backed local tests. JVM rules own language compilation; Android
consumes their public providers.

Common `rules_android` 0.7 attributes are compatible only where their semantics
match. A checked-in matrix identifies each supported attribute. Unknown or
unsupported attributes fail during analysis.

### Toolchains

The Android SDK platform, build tools, JDK, Kotlin compiler, AAPT2, D8,
zipalign, apksigner, and Robolectric runtime are explicit build inputs.
Versions and content digests are declared in source control. Builds do not
search `ANDROID_HOME`, select the newest installed SDK, or fall back to Gradle.
A missing input fails before execution and names that input.

Android consumes declared JAR and AAR targets. The NDK receives a separate
toolchain and rules RFC.

### Repository boundary

```text
rules/android/
  providers/
  rules/
  toolchains/
  tests/
```

Examples live in `examples/android`. Engine tests may depend on this module;
engine production code may not.

The module starts in this repository. A separate repository is justified only
by an independent maintainer or release cadence.

## Alternatives

| Option | Benefit | Why not |
| --- | --- | --- |
| Keep the prelude | No initial move | Couples Android to the engine and preserves private assumptions. |
| Wrap Gradle | Broad existing ecosystem | Hides a second action graph and cache beneath Bessemer. |
| Fork `rules_android` | Familiar implementation | Imports Bazel-specific internals and dependencies wholesale. |
| Remove Android | Lowest maintenance | Gives up a strategic workload and adoption path. |

The determination keeps the useful public familiarity of `rules_android` while
using only implementation that Bessemer can prove and own.

## Consequences

Existing Buck2 Android files will require migration; source familiarity does
not mean drop-in compatibility. The smaller initial surface reduces migration
coverage but gives every supported attribute a testable meaning.

SDK, compiler, and test-runtime artifacts are supply-chain inputs and must be
content-addressed. Stable support includes only debug signing with a
non-production key. Requesting production signing before its credential
boundary is specified must fail.

Keeping Android actions in Bessemer's graph enables normal sandboxing, remote
execution, and cache invalidation. The prototype must report cold toolchain
download size and time, warm build time, and incremental action counts before
this RFC becomes `accepted`.

## Validation and rollout

1. Define the four signatures and one explicit SDK toolchain.
2. Build a valid Kotlin APK on a clean Linux runner.
3. Move only the transitive implementation used by that application.
4. Delete private branches, targets, modes, tests, and documentation as code
   moves.
5. Prove local tests, macOS arm64, and incremental rebuilds.
6. Delete `prelude/android` and `prelude/toolchains/android`.

Stable Android support requires CI evidence that:

- clean Linux and macOS arm64 runners build the example from pinned inputs;
- `aapt2 dump badging` validates the APK;
- `android_local_test` runs a real failing and passing test;
- an unchanged rebuild executes no compile, resource, DEX, or package actions;
- a source edit invalidates only actions that consume it;
- a missing SDK artifact produces a specific analysis error; and
- `rules/android` contains no Meta-only paths, services, modes, or conditional
  export markers.

There is no alias from the old paths. Migration is complete only when the old
directories are gone. Documentation may claim only capabilities proven above.

## Open questions

1. Which `rules_android` 0.7 attributes belong in the first matrix? Start with
   only attributes exercised by the reference application, then add an
   attribute with its contract test.
2. How may pinned Android SDK packages be materialized? The design must require
   explicit license acceptance and verify every downloaded artifact.
3. May a Linux-only preview precede macOS parity? Yes, if it is labeled preview;
   macOS arm64 remains a stable-release gate.

## References

- [Oxide RFD process](https://rfd.shared.oxide.computer/rfd/0001)
- [Bazel Android rules](https://github.com/bazelbuild/rules_android/tree/0bd9b590fbc856b22abc7da1e17c914488889221)
- [Bazel Android public surface](https://github.com/bazelbuild/rules_android/blob/0bd9b590fbc856b22abc7da1e17c914488889221/rules/rules.bzl)
- [Bazel Android NDK rules](https://github.com/bazelbuild/rules_android_ndk/tree/987213e7f982de48696f3b9cd031ff4e3e065c7c)
- [Bazel Kotlin rules](https://github.com/bazelbuild/rules_kotlin/tree/01bf16406ae9fd2252a5cc7b16f6e15aeeaa6c59)
