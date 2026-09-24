<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Define Python error handling, testing and import conventions. -->

# Errors and tests

Continue [the Python guide](../python.md) with error handling, constants, tests
and imports. The conventions below come from the same Dedalus guide. Only the
repository commands and test-runner applicability are adapted.

## Error Handling

Fail fast and fail loud. Swallowing exceptions is how bugs hide:

```python
# This is how bugs hide
try:
    result = risky_call()
except Exception:
    result = None  # Swallowed. Good luck debugging.

# Translate the expected failure at the owning boundary.
try:
    result = risky_call()
except SpecificError as exc:
    raise OperationError("required operation failed") from exc
# Unexpected failures propagate.
```

Never catch or raise blind exceptions. `except Exception` catches everything and tells you nothing.
`raise Exception("something went wrong")` is the error message equivalent of a shrug.

Define specific exception types. Namespace them under a base class:

```python
# exceptions.py
class BillingError(Exception): ...
class ChargeError(BillingError): ...
class InsufficientBalanceError(BillingError): ...

# Namespace aliases (bottom of file)
BillingError.ChargeError = ChargeError
BillingError.InsufficientBalanceError = InsufficientBalanceError

# Usage: one import, access via namespace
from .exceptions import BillingError

raise BillingError.ChargeError(org_id, "card declined")
```

### Result Types for Domain Logic

For business logic where you want explicit error handling in the type signature, use
`Result[T, E]`. The function signature tells callers exactly what can fail:

```python
type OrderError = UserNotFoundError | InsufficientBalanceError | DatabaseError

async def create_order(io: Io, user_id: str, items: list[Item]) -> Result[Order, OrderError]:
    """Create an order. Returns typed Result, not exceptions."""
    user = await io.db.get_user(user_id)
    if not user:
        return Err(UserNotFoundError(user_id))

    if user.balance < total(items):
        return Err(InsufficientBalanceError(user.balance, total(items)))

    order = await io.db.insert_order(user_id, items)
    return Ok(order)

# Caller: pattern match for exhaustive handling
match await create_order(io, user_id, items):
    case Ok(order):
        return OrderResponse(order)
    case Err(UserNotFoundError()):
        raise HTTPException(404, "User not found")
    case Err(InsufficientBalanceError()):
        raise HTTPException(402, "Insufficient balance")
```

### When to Use Which

| Situation | Pattern |
| --- | --- |
| API boundaries (routes) | try-catch, convert to HTTP errors |
| Domain logic (services) | `Result[T, DomainError]` for explicit contracts |
| Infrastructure calls (DB, HTTP) | try-catch, convert to domain errors or return `Err` |
| Background operations | Give the task an owner and record failures. Usage accounting must follow its durability contract. |

### No Fallbacks

Fallbacks are an anti-pattern. They're how bugs hide:

```python
# This is a bug waiting to happen
model = request.model or config.default_model or "gpt-4"

# This is explicit
if not request.model:
    raise ValueError("model is required")
```

When code says `a or b or c`, you're admitting you don't know which one will run. Be explicit:

```python
# Explicit precedence (good)
def get_timeout(user: User, org: Org) -> int:
    if user.timeout_override is not None:
        return user.timeout_override
    if org.timeout_policy is not None:
        return org.timeout_policy
    return DEFAULT_TIMEOUT
```

## Constants

Raw dicts are not constants. They're mutable, untyped, and invisible to your IDE. Use enums
and frozen dataclasses:

```python
# This is a bug waiting to happen
TIMEOUTS = {"dev": 60, "staging": 300, "prod": 600}

# This is a constant
class Timeout(int, Enum):
    DEV = 60
    STAGING = 300
    PROD = 600

    @classmethod
    def for_env(cls, env: str) -> int:
        return cls[env.upper()].value
```

### IntFlag Bitmasks

Sometimes a value has several independent boolean properties that combine. A dict of booleans
works, but it does not compose into a single reusable value.

Use `IntFlag` when you want one value that can represent multiple options at once:

```python
from enum import IntFlag


class Permission(IntFlag):
    NONE = 0
    READ = 1 << 0
    WRITE = 1 << 1
    DELETE = 1 << 2
    ADMIN = READ | WRITE | DELETE


def can_write(perms: Permission) -> bool:
    return bool(perms & Permission.WRITE)


editor = Permission.READ | Permission.WRITE
```

Membership checks are bitwise operations (`&`) and are effectively constant-time for normal flag
domains. In CPython, integers are arbitrary precision, so the theoretical cost scales with the
number of machine words. For typical flag usage (dozens of bits), this is still very fast.

Use `IntFlag` when:

- The domain is fixed and small.
- Values are combinable and passed around as one field.

Use `set`/`frozenset` when:

- Values are dynamic/unbounded.
- You only need plain membership and not flag composition.

### Numeric Literals

Use underscores for readability. `1_000_000` is easier to parse than `1000000`:

```python
MILLI_CENTS_PER_DOLLAR = 100_000
MAX_TOKENS = 1_000_000
```

Use underscores for numbers larger than 1000, unless it's a power of 2 (`2048`, `4096`, `65536`
are instantly recognizable).

## Testing

Tests are specifications. They document what the code should do:

```python
@test
async def rejects_empty_clerk_id():
    """Empty Clerk ID should raise ValueError instead of querying the database."""
    with pytest.raises(ValueError, match="non-empty"):
        await get_user_uuid_from_clerk_id("")
```

Test **contracts**, not implementation details. Testing that `get_user` calls `db.table("users")`
is brittle. Testing that `get_user("nonexistent-id")` returns `None` tests actual behavior.

### Inline Tests

We use `inline-tests` for colocated unit tests. Tests live next to the code they verify:

```python
async def fetch_user(user_id: str) -> User | None:
    if not user_id:
        raise ValueError("user_id required")
    # ... implementation


# --- Tests ---

from inline_tests import test


@test
async def rejects_empty_id():
    import pytest  # noqa: PLC0415

    with pytest.raises(ValueError):
        await fetch_user("")
```

Local imports in tests with `# noqa: PLC0415` prevent test dependencies from loading in
production.

**Running:** In packages that install `inline-tests`, use
`uv run pytest path/to/file.py --inline-tests -v` or `itest src/`. BSMR
qualification scripts keep their existing standalone entrypoints.

## Imports

Import what you need at the top of the file. No `__getattr__` lazy import patterns in
`__init__.py`. If a circular import exists, fix the dependency graph.

No import aliases (`import X as Y`) unless there is an immovable naming collision:

```python
# Bad: alias hides the real name
from api.usage.tracking import ResponsesUsage as Usage

# Good: use the real name
from api.usage.tracking import ResponsesUsage
```

### Lint Suppressions

Suppress lint rules at the smallest scope possible.

- Use line-level suppression only (`# noqa: RULE`) for the exact offending line.
- Do not use file-level suppressions (`# ruff: noqa`, top-level `# noqa`, etc.).
- Include only the specific rule IDs you need.

Good:

```python
import pytest  # noqa: PLC0415
```

## Run Everything

Run the [pinned Python checks](../python.md#check-python) on changed Python, then
run the relevant tests. `pnpm run ci check` verifies the existing repository
commands. It does not replace Ruff, ty or the real runtime fixture.

```console
uv run --project tools/python --locked ruff format --config tools/python/pyproject.toml path/to/file.py
uv run --project tools/python --locked ruff check --config tools/python/pyproject.toml path/to/file.py
uv run --project tools/python --locked ty check --config-file tools/python/ty.toml path/to/file.py
pnpm run ci check
```
