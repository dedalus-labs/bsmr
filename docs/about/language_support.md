---
id: language_support
title: Language / Ecosystem Support
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->


# Language / Ecosystem Support

This page provides an overview of the programming languages supported by Bessemer.

| Language / Ecosystem | Prelude Available | Usability     | Documented | Maintainers                   |
| -------------------- | ----------------- | ------------- | ---------- | ----------------------------- |
| C/C++ (not windows)  | ✅                | Complex Setup | ❌         |                               |
| C/C++ (windows)      | ✅                | Complex Setup | ❌         |                               |
| C#                   | ✅                | Unavailable   | ❌         |                               |
| Erlang               | ✅                | Easy Setup    | ❌         | GitHub: michalmuskala         |
| Go                   | ✅                | Native Sync   | ✅         | Dedalus Labs                  |
| Haskell              | ✅                | Easy Setup    | ❌         |                               |
| Java                 | ✅                | Complex Setup | ❌         |                               |
| Java (Mobile)        | ✅                | Complex Setup | ❌         | GitHub: NavidQar & IanChilds  |
| Kotlin               | ✅                | Complex Setup | ❌         |                               |
| Kotlin (Mobile)      | ✅                | Complex Setup | ❌         | GitHub: NavidQar & siaojiecai |
| Objective-C          | ✅                | Unavailable   | ❌         |                               |
| OCaml                | ✅                | Easy Setup    | ❌         |                               |
| Python               | ✅                | Easy Setup    | ❌         | GitHub: zsol                  |
| Rust                 | ✅                | Easy Setup    | ❌         | GitHub: jakobdegen            |
| Swift                | ✅                | Unavailable   | ❌         |                               |

## Understanding the Table

- **Prelude Available**: Indicates whether Bessemer's prelude includes built-in
  rules for this language.
- **Usability**: Indicates whether this language is possible or degree of setup
  required.
  - **Easy Setup**: Basic installation required, usually searching the path for
    tools
  - **Complex Setup**: Requires additional setup beyond simply installation
  - **Native Sync**: Native ecosystem metadata generates Bessemer targets
  - **Unavailable**: Rules are using tools that are not available
- **Documented**: Indicates the level of documentation available for using this
  language with Bessemer.
- **Maintainers**: Teams or individuals responsible for maintaining support for
  this language.

## Adding Support for New Languages

Bessemer is designed to be extensible, allowing you to add support for additional
programming languages. To add support for a new language, you typically need to:

1. Define appropriate build rules in a `.bzl` file
2. Create toolchain definitions for the language
3. Write documentation for how to use the language with Bessemer

For more information on creating custom rules and toolchains, see:

- [Writing Rules](../rule_authors/writing_rules.md)
- [Writing Toolchains](../rule_authors/writing_toolchains.md)
