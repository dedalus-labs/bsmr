<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

# @generated
# To regenerate, run:
# ```
# STARLARK_RUST_REGENERATE_GOLDEN_TESTS=1 cargo test -p starlark --lib
# ```

# Magic

<pre class="language-python"><code>def Magic(a1: int, a2: int = ..., step: int = 1, /) -> str</code></pre>

A function with only positional arguments.

And a slightly longer description. With some example code:

```python
Magic(1)
```

And some assertions:

```rust
1 == 1
```
