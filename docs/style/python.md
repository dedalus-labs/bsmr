---
title: "Python Style"
description: "Code patterns, naming, typing, and error handling conventions for Python at Dedalus"
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Define Python structure, typing and documentation conventions. -->

# Python Style

## BSMR scope

Apply this guide to new and changed Python. Existing upstream code keeps its legal
notices. This import does not claim that untouched files already comply.

The source is the Dedalus Python guide at revision
`30f7b97c59660bb5c8d49819ba4cbb40d7ea778d`. Its conventions are retained. Local
adaptations cover tool commands, interpreter compatibility and application imports.

Developer programs use Python 3.14 or newer. Native build helpers must also run on
their declared executor interpreter. The current Linux fixture runtime uses Python
3.12, so the shared lint and type-check configuration targets that version.

BSMR does not depend on the monorepo's application `Io` type. Define the narrow I/O
capability the module needs. Methods on an `Io` owner use that receiver as the
capability. Pure values keep their behavior on their own types.

We use **Ruff** for formatting and linting, **ty** for type checking. Ruff replaces Black,
isort, and most of Flake8. Run the formatter, accept its output.

Use Python 3.14 or newer. Annotations are deferred natively, so do not import
`annotations` from `__future__`. Pin ty through the repository dependency configuration.

Use `pnpm run ci check` for repository tooling checks. Python checks use the
pinned tools in `tools/python`, as shown below. They are separate commands.

## Formatting

| Setting | Value |
|---------|-------|
| Indentation | 4 spaces |
| Line width | 100 characters |
| Quotes | Double (`"`) |
| Trailing commas | Yes |

### Ruff Config

In `tools/python/pyproject.toml`:

```toml
[tool.ruff]
line-length = 100
target-version = "py312"

[tool.ruff.format]
quote-style = "double"
indent-style = "space"
docstring-code-format = true

[tool.ruff.lint]
select = ["E", "F", "W", "I", "UP", "B", "SIM", "RUF", "ANN", "RET"]
ignore = ["RET504"]  # Preserve the named return values required below.

```

### Editor Setup

**VS Code / Cursor:** Install the [Ruff extension](https://marketplace.visualstudio.com/items?itemName=charliermarsh.ruff). Add to settings:

```json
{
  "editor.formatOnSave": true,
  "[python]": {
    "editor.defaultFormatter": "charliermarsh.ruff"
  }
}
```

**Neovim:** Use `ruff` via `conform.nvim` or LSP.

## Hard Limits

Functions stay under **70 lines**. Files stay under **500** (not including inlined tests). Nesting
stays under **3 levels**. Arguments stay under **5**.

If you're hitting these limits, slow down. A function that doesn't fit on one screen is too long.
A file over 500 lines has too many concerns. Deep nesting means you're not using early returns.
Too many arguments means you need a config object.

## Explicit I/O

Functions that do I/O take `io: Io` as their first parameter. Pure functions don't.

```python
async def fetch_user(io: Io, user_id: str) -> User | None:
    """io: = this function does I/O."""
    result = await io.db.table("users").select("*").eq("id", user_id).single().execute()
    return User(**result.data) if result.data else None


def compute_discount(user: User, cart: Cart) -> Decimal:
    """No io: = pure computation."""
    return cart.total * user.discount_rate
```

The convention is simple: `io:` in the signature means it talks to the outside world. No `io:`
means it's pure. You can tell at a glance, without tracing call stacks or wondering if something
secretly hits the database.

Testing is straightforward. Inject a `MockIo` and you control all the I/O:

```python
@test
async def returns_none_for_missing_user():
    io = MockIo(db=MockDb(responses={"users": []}))
    result = await fetch_user(io, "nonexistent")
    assert result is None
```

The `Io` type bundles capabilities: `io.db` for database, `io.stripe` for payments, `io.http` for
external APIs, `io.auth` for authentication. Route handlers create `Io` at the boundary and pass
it down:

```python
@router.post("/orders")
async def create_order(request: Request) -> OrderResponse:
    io, auth = await with_clerk_auth(request)
    order = await process_order(io, auth.org_id, request.items)
    return OrderResponse(order=order)
```

Three benefits:

1. **Visibility**: `io:` in signature = does I/O, no `io:` = pure
2. **Testability**: Inject `MockIo` with canned responses, no `patch()` gymnastics
3. **No ambient state**: No `get_db_client()` calls hidden in the call stack

## Functions

Functions validate their inputs. If `user_id` must be non-empty, check it:

```python
async def get_user(user_id: str) -> User | None:
    if not user_id:
        raise ValueError("user_id must be non-empty")
    # ...
```

Use early returns instead of nesting:

```python
# Nested (bad)
async def process(request: Request) -> Response:
    if request.is_valid():
        if request.user:
            if request.user.is_active:
                return handle(request)
    return error_response()

# Flat (good)
async def process(request: Request) -> Response:
    if not request.is_valid():
        return error_response("invalid request")
    if not request.user:
        return error_response("no user")
    if not request.user.is_active:
        return error_response("inactive user")
    return handle(request)
```

Don't combine conditions with `or` in error checks. When you throw, you need to know which
clause failed:

```python
# Bad: which field was missing?
if credits is None or debits is None:
    raise BalanceQueryError(org_id, "malformed response")

# Good: specific error for each failure
if credits is None:
    raise BalanceQueryError(org_id, "missing credits_posted")
if debits is None:
    raise BalanceQueryError(org_id, "missing debits_posted")
```

Functions do one thing. If you need "and" to describe what a function does, it's two functions.
`validate_and_save_user` should be `validate_user_data` and `save_user`.

The return statement should be saved to its own variable so you can debug print it instead of
having to delete the return, re-assign to a variable, print, and so on.

### Put Behavior on the Owning Type

Avoid loose module-level functions that cluster around the same state. They grow without structure
and force callers to know which pieces of data belong together. If a few functions keep taking the
same object, config, or small set of fields, make that relationship explicit with a class and put
the behavior on it.

```python
# Bad: related behavior floats beside the state it depends on.
if is_ready(worker):
    result = run_job(worker, job)

# Good: the type owns its state and the behavior that depends on it.
if worker.ready:
    result = worker.run(job)
```

Module-level functions are fine for pure, stateless transformations. Prefer methods when the
behavior depends on object state or when several functions form an informal API around the same
data.

### Named Parameters

Use named parameters at call sites. If a default changes, positional calls silently break.

```python
# Bad: if default changes, this silently does the wrong thing
credits_available = await check_mcp_credits_available(io, org_id)

# Good: explicit, survives refactoring
credits_available = await check_mcp_credits_available(io=io, org_id=org_id, required_credits=1)
```

Exceptions: trivial calls where meaning is obvious (`print(x)`, `len(items)`, CLI flags).

## Naming

Names are precise. `get_usr(uid)` tells you nothing. `get_user_by_id(user_id)` tells you exactly
what it does. Abbreviations save keystrokes and cost clarity.

| Element | Convention | Example |
|---------|------------|---------|
| Functions | snake_case | `fetch_user`, `compute_discount` |
| Variables | snake_case | `user_id`, `is_active` |
| Classes | PascalCase | `User`, `BillingError` |
| Constants | SCREAMING_SNAKE | `MAX_RETRIES`, `DEFAULT_TIMEOUT` |
| Modules | snake_case | `user_service.py`, `billing.py` |
| Test files | inline or `.test.py` | `billing.py` (inline), `test_billing.py` |

Put units and qualifiers last: `timeout_ms`, `latency_ms_max`, `latency_ms_p99`. They sort
together and align in columns. `max_timeout_ms` and `p99_latency_ms` scatter related variables.

### Terse Locals (Unix Style)

Mechanical variables (temporaries, return values, loop counters, API responses) use 2-4 character
names in the Unix tradition. Domain variables keep descriptive names.

```python
# Mechanical (terse)
res = await io.db.table("users").select("*").execute()
ret = parse_config(raw)
tmp = sorted(items)
err = validate(data)
buf = io.read(1024)
ctx = get_context()
cfg = load_config()

# Domain (descriptive)
user = res.data[0]
amount = request.amount
org_id = auth.org_id
balance = credits - debits
```

The rule: if swapping the variable for another of the same type wouldn't change meaning, use a
terse name. If the variable represents a domain concept, name the concept.

## Typing

No raw dicts. If you're passing around `dict[str, Any]`, you're passing around a bag of maybes.
Use typed structures: a dataclass, a TypedDict, a Pydantic model. Something with a shape the IDE
can see and the reader can understand.

```python
# This tells you nothing
def process(data: dict[str, Any]) -> dict[str, Any]: ...

# This tells you everything
def process(request: ProcessRequest) -> ProcessResult: ...
```

Be judicious about Pydantic. It's powerful (runtime validation, serialization, schema
generation), but it has a cost. Every `model_dump()` and `model_validate()` takes CPU cycles. For
mission-critical validation at API boundaries (request/response schemas, config loading), that
cost is worth it. For internal data passing, a frozen dataclass or plain type hints are enough.

The rule: **Pydantic for the edges** (where data enters and exits), **plain types for the internals**.

### Forbidden Types

Never use `object` as a type annotation. It tells you nothing. If you don't know the type, figure
it out. If the type is genuinely polymorphic, use a `Protocol` or a `Union`.

Same goes for `**kwargs: Any`. Every `Any` is a hole in your type safety. Holes require
justification. "I didn't want to think about it" is not justification.

## Lambdas

Use lambdas sparingly. Appropriate only when genuinely trivial (single expression) and used
exactly once, typically as `key=` for `sorted()` or `max()`:

```python
# Acceptable: single-use, obvious context
sorted(users, key=lambda u: u.created_at)
max(items, key=lambda x: x.priority)
```

For anything else, define a named function. Named functions are debuggable (stack traces say the
name, not `<lambda>`), testable in isolation, and readable.

```python
# Bad: complex logic in lambda
users.filter(lambda u: u.status == "active" and u.org_id == org_id and not u.deleted)

# Good: named function with clear intent
def is_active_member(user: User) -> bool:
    return user.status == "active" and user.org_id == org_id and not user.deleted

users.filter(is_active_member)
```

If you're writing a lambda longer than 40 characters, stop and write a function.

## Documentation

### Module Docstrings

First line states what the module is or does. Follow stdlib conventions:

```python
"""Double-ledger accounting system.

Functions:
  charge_*  -- ledger primitives (debit account)
  bill_*    -- business logic (determine price, then charge)
  get_*     -- queries
"""
```

Rules:

- First line: what it is (noun phrase) or does (verb phrase), period
- Use `name -- description` or `name(args) --> return` for catalogs
- Group related items under plain text headers
- No "This module provides..." preamble

### Function Docstrings

Google-style. Start with imperative mood ("Fetch", not "Fetches"). Args, Returns, Raises sections
follow. Multi-line docstrings with sections end with a blank line before the closing quotes.
One-liners stay compact on a single line.

```python
# One-liner: no trailing blank line
"""Load a checkpoint from a specific step."""

# Multi-line with sections: trailing blank line before closing quotes
"""Fetch user from database by ID.

Args:
    user_id: The unique identifier for the user.

"""
```

Write proper English sentences with periods, not sentence fragments with dashes:

```python
async def fetch_user(io: Io, user_id: str, *, include_deleted: bool = False) -> User | None:
    """Fetch user from database by ID.

    The returned User is a snapshot; mutations won't persist.

    Args:
        io: I/O capability handle.
        user_id: The unique identifier for the user.
        include_deleted: If True, include soft-deleted users.

    Returns:
        The User object if found, None otherwise.

    Raises:
        ValueError: If user_id is empty.
        DatabaseError: On connection failure.

    """
```

Skip sections that add nothing. A parameterless function doesn't need an Args section.
Docstrings explain the **contract**, not the implementation.

### Comments

Comments earn their keep or get deleted. `user = get_user(id)  # Get the user` is noise.

Section headers inside a file use `# --- Label ---` format. One line, not three.

```python
# --- Tests ---

from inline_tests import test

# Good: inside a class
class Parser:
    # --- Entry points ---

    def parse(self, resource): ...

    # --- Schema resolution ---

    def resolve_schema(self, schema, schemas): ...
```

When a structured type has inline field comments, column-align them two spaces after the longest
value expression and wrap with `# fmt: off` / `# fmt: on`:

```python
# fmt: off
class TransferCode(IntEnum):
    USAGE = 1                  # Standard usage charge.
    BALANCE_DEPOSIT = 100      # Stripe payment or balance top-up.
    MCP_SELLER_CLAWBACK = 411  # Reverse posted hold on refund.
# fmt: on
```

## Check Python

Run these from the repository root, replacing `path/to/file.py` with the files you
changed. The tools and their versions are locked in `tools/python`.

```console
uv run --project tools/python --locked ruff format --config tools/python/pyproject.toml path/to/file.py
uv run --project tools/python --locked ruff check --config tools/python/pyproject.toml path/to/file.py
uv run --project tools/python --locked ty check --config-file tools/python/ty.toml path/to/file.py
```

Use `ruff format --check` in a read-only verification step. The repository's
existing CI check does not yet enforce these Python checks across all Python files.
