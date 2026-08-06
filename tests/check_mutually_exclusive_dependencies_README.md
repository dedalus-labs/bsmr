<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

# check_mutually_exclusive_dependencies_test

This test is particularly useful for enforcing rules like:

1. **Preventing mixing of library variants**: Ensure targets use only one of: `//third-party/volk:volk`, `//third-party/volk:volk-header`, or `//third-party/toolchains:vulkan`
2. **Any scenario** where multiple dependencies should never coexist in the same transitive dependency tree

## How to Use

### Basic Example

Add this to your BUILD.bsmr file:

```python
load(
    "@upstream//tools/build_defs:check_dependencies_test.bzl",
    "check_mutually_exclusive_dependencies_test",
)

check_mutually_exclusive_dependencies_test(
    name = "no_conflicting_volk_deps",
    target = "upstream//your/target:name",
    contacts = ["your-oncall@xmail.facebook.com"],
    mutually_exclusive_group = [
        # Only one of these should be present in the dependency tree
        "upstream//third-party/volk:volk",
        "upstream//third-party/volk:volk-header",
        "upstream//third-party/toolchains:vulkan",
    ],
)
```

### Using Regex Patterns

Each pattern in the group can be a specific target or a regex pattern:

```python
check_mutually_exclusive_dependencies_test(
    name = "no_mixed_dependencies",
    target = "upstream//your/target:name",
    contacts = ["your-oncall@xmail.facebook.com"],
    mutually_exclusive_group = [
        # Match specific targets
        "upstream//third-party/lib-v1:specific_target",
        # Use regex to match multiple targets
        "upstream//third-party/lib-v2:.*",
        # Another regex pattern
        "upstream//third-party/lib-v3/.*",
    ],
)
```

## Parameters

- **name** (required): Name of the test target
- **target** (required): The target whose dependencies should be checked
- **contacts** (required): List of contacts responsible for the test
- **mutually_exclusive_group** (required): List of dependency patterns where only one should be present. Each pattern can be a specific target (e.g., "//foo/bar:baz") or a regex pattern (e.g., "//foo/.*")
- **labels** (optional): Additional labels for the test (default: [])
- **target_deps** (optional): If True, only check target_deps() (default: True)
- **expect_failure_msg** (optional): Regex pattern for expected failure message (for testing)
- **deps** (optional): Additional dependencies for the test
