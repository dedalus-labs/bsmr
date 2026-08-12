<!-- ===----------------------------------------------------------------------=== -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

# @generated
# To regenerate, run:
# ```
# STARLARK_RUST_REGENERATE_GOLDEN_TESTS=1 cargo test -p starlark --lib
# ```

error: Parse error: unexpected identifier 'baz', expected f-string expression end '}'
 --> assert.bzl:1:15
  |
1 | f'foo \n {bar baz}'
  |               ^^^
  |
