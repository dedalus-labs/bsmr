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

# globals

## MAGIC

<pre class="language-python"><code>MAGIC: int</code></pre>

---

## func1

<pre class="language-python"><code>def func1(foo: str) -> str</code></pre>

Docs for func1

#### Parameters

* `foo`: (required)

  Docs for foo



#### Returns

The string 'func1'

---

## func2

<pre class="language-python"><code>def func2() -> str</code></pre>

---

## pos\_either\_named

<pre class="language-python"><code>def pos_either_named(
    a: int,
    /,
    b: int,
    \*,
    c: int,
) -> <a to="/path/to/Magic">magic</a></code></pre>

---

## with\_defaults

<pre class="language-python"><code>def with_defaults(
    explicit_default: list[str] = [],
    hidden_default: list[str] = ...,
    string_default: str = "my_default",
) -> None</code></pre>
