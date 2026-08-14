---
id: python
title: Python, uv, Ruff, and ty
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->

<!-- Documents the native Python project API and its current hermeticity boundary. -->

# Python, uv, Ruff, and ty

BSMR builds conventional Python projects and uv workspaces directly from
`pyproject.toml`. You do not need a `BUCK`, `BUILD.bsmr`, or handwritten
Starlark file for the conventional path. Standard Python metadata stays
authoritative while BSMR lowers it into a private action graph.

The built-in catalog currently pins CPython 3.14.7, uv 0.12.4, Ruff 0.16.3,
and ty 0.0.71 by platform artifact digest. BSMR never consults an activated
virtual environment or ambient Python, uv, Ruff, or ty installation.

CPython 3.14.7 is the latest-stable default. Repositories that require the
previous stable line may select either its minor alias or exact patch in the
root `.bsmr` file:

```toml title=".bsmr"
[python]
version = 3.13
```

The supported values are `3.13`, `3.13.15`, `3.14`, and `3.14.7`; minor
aliases resolve to BSMR's exact catalog pin. An unsupported value fails during
analysis instead of selecting an ambient or approximately compatible Python.

## Project contract

Commit these files at the repository root:

- `pyproject.toml` with `project.name` and `project.requires-python`, or a
  tool-only uv workspace root;
- `pylock.toml`, the complete [PEP 751](https://peps.python.org/pep-0751/)
  runtime installation set; and
- `pylock.build.toml`, the complete PEP 751 build-backend installation set.

Add `pylock.test.toml` for the default pytest closure. Named files such as
`pylock.test-integration.toml` create matching `test-integration` targets.
Each lock is a complete installation set, not a requirements fragment.
Before graph analysis, BSMR rejects unnormalized package names, ambiguous
source forms, artifacts without a location and hash, incomplete VCS identities,
and attestation entries without a kind.

```toml title="pyproject.toml"
[project]
name = "acme-api"
version = "0.1.0"
requires-python = ">=3.14"
dependencies = ["httpx>=0.28"]

[project.scripts]
acme-api = "acme_api.cli:main"

[build-system]
requires = ["hatchling>=1.27"]
build-backend = "hatchling.build"

[tool.ruff]
target-version = "py314"

[tool.ty.environment]
python-version = "3.14"
```

Use uv to author the standard locks. For example:

```console
uv lock
uv export --locked --format pylock.toml --no-default-groups --no-emit-workspace --output-file pylock.toml
uv pip compile pylock.build.in --format pylock.toml --python-version 3.14.7 --output-file pylock.build.toml
uv export --locked --format pylock.toml --all-extras --all-groups --no-emit-workspace --output-file pylock.test.toml
```

`pylock.build.in` is an authoring input listing every build backend and any
explicit compatibility requirement needed by an sdist. It is not a BSMR lock
format. Keep the resulting standard `pylock.build.toml` authoritative.
BSMR validates that every `[build-system].requires` and
`[tool.uv.extra-build-dependencies]` package in the selected first-party
closure exists in that frozen build lock before running a backend. A
`match-runtime = true` requirement must select the same version in the runtime
and build locks.

Third-party sdists must declare their build closure explicitly. Use uv's native
package-scoped setting and place the same requirements in `pylock.build.in`:

```toml title="pyproject.toml"
[tool.uv.extra-build-dependencies]
pyiceberg = ["poetry-core>=1.0.0", "wheel", "cython>=3.0.0", "setuptools"]
flash-attn = [{ requirement = "torch", match-runtime = true }]
```

This is intentional PEP 517 planning, not dependency resolution during a
build. Missing requirements and `match-runtime` version disagreements fail
before the backend executes. The current baseline materializes one shared
build lock, so changing that lock invalidates every sdist action that consumes
it; package-scoped build environments remain a later granularity improvement.

BSMR does not consume `uv.lock` implicitly. uv may use it while authoring a PEP
751 export, but ordinary builds consume only the committed `pylock*.toml`
files. An explicit `uv.lock` compatibility provider remains an RFC milestone.

## Build targets

Initialize the repository once, then address projects by directory:

```console
bsmr init
bsmr build .
bsmr build :lint
bsmr build :typecheck
bsmr test :test
bsmr run :acme-api
```

An installable project gets a wheel target named after its normalized PEP 621
distribution name. BSMR also creates Ruff `lint`, ty `typecheck`, pytest
targets for every test lock, and runnable PEP 621 console scripts.

For uv workspaces, `[tool.uv.workspace].members` and `exclude` select native
members. `[tool.uv] package = false` creates checks and tests without a wheel.
A tool-only root may own the shared toolchains and locks without inventing a
fake root distribution. Package-local environments contain only the transitive
first-party wheels declared by that package; unrelated workspace members do
not invalidate its build or tests.

## Configuration

BSMR deliberately uses each tool's native configuration rather than defining a
second Python DSL:

- uv build backend settings come from
  [`[tool.uv].config-settings`](https://docs.astral.sh/uv/reference/settings/#config-settings)
  and are forwarded as typed, repeated PEP 517 `config-settings`;
- package-specific PEP 517 settings and build variables come from
  [`config-settings-package`](https://docs.astral.sh/uv/reference/settings/#config-settings-package)
  and
  [`extra-build-variables`](https://docs.astral.sh/uv/reference/settings/#extra-build-variables);
- Ruff reads its
  [native configuration](https://docs.astral.sh/ruff/configuration/) from the
  declared source tree; and
- ty reads its [native configuration](https://docs.astral.sh/ty/configuration/)
  and receives the exact interpreter plus the selected third- and first-party
  search paths.

The one build-system-specific exception is test orchestration. Pytest remains
the zero-configuration default. A project that owns another Python runner may
declare shell-free argv without adding Starlark:

```toml title="pyproject.toml"
[tool.bsmr.python]
test-command = ["tests/runtests.py", "--verbosity", "1"]
```

Arguments passed after `bsmr test :test --` are appended to this command. BSMR
executes it through the pinned child interpreter and the selected PEP 751
environment; it does not invoke a shell or search for an ambient executable.

Keep Ruff and ty file selection in `pyproject.toml`. BSMR invokes both tools
from the declared project root without a positional path, so committed native
configuration remains authoritative instead of being overridden by a
BSMR-specific scope.

Ruff runs with its local cache disabled because BSMR caches the complete action
by tool, configuration, and analysis-source digest. ty receives independently
cacheable dependency and first-party wheel layers. uv receives the pinned
interpreter, isolated home and cache directories, disabled Python downloads,
and no ambient user configuration.

All uv settings above are explicit action inputs. Global settings apply to
each sdist; package-specific settings apply only to their normalized package;
build variables are written to an isolated generated uv configuration because
uv does not expose them as command-line flags. Changes invalidate only the
actions whose declared settings changed.

Pre-PEP 517 projects are accepted only where uv can build them through its
legacy PEP 517 adapter with an explicit build closure. This path is discouraged.
New projects should declare `[build-system]`; existing projects should migrate
rather than add more undeclared `setup.py` behavior.

## Correctness and caching

Dependency environments, first-party wheels, lint, typecheck, and tests are
separate actions. Source-controlled virtual environments are detected by
`pyvenv.cfg` and pruned before package discovery. Generated caches, lockfiles,
build outputs, and virtual environments never become first-party source inputs.

Each package action uses `uv pip sync --strict` against one canonical
single-package PEP 751 fragment, normalizes console-script shebangs, rejects
symlinks and special files, and records the immutable result in BSMR's CAS.
BSMR composes those package trees deterministically, rejects incompatible
import-file collisions, applies uv-compatible first-package precedence to
console scripts, and records all owners when identical files are shared.
First-party wheels are built once and overlaid as a separate content-addressed
layer. Warm no-op builds and deleted-output restoration therefore require no
Python tool execution.

Test commands consume those cached environments, but BSMR does not yet cache
their result records. Repeating `bsmr test` reruns the selected command; test
result caching remains a release gate.

Digest-pinned HTTP artifacts, including the built-in Python tools, are shared
across repositories in the operating system's user cache under
`bsmr/http-v1`. Set `BSMR_HTTP_CACHE_DIR` to an absolute path to relocate it.
Every restoration revalidates the declared checksum before the artifact enters
an action. `bsmr clean` preserves this repository-independent cache, and BSMR
does not currently evict it automatically; remove the configured directory
while no builds are running to reclaim it in full.

## Hermeticity boundary

The current implementation is the pinned-uv differential baseline from
[RFC 0004](https://github.com/dedalus-labs/bsmr/discussions/16), not the RFC's
final native materializer.

A cold package action may still ask uv to fetch its exact locked artifact from
an index. Package-granular, hash-addressed environment layers are implemented,
but portable offline artifact replay after an action-cache miss is not: the
distribution archive itself is not yet a separately acquired CAS object.
Native PEP 751 materialization, import-level ownership and closure inference,
and provenance queries remain release gates.

Pure-Python PEP 517 builds use the exact interpreter and locked build closure.
Native extensions currently use the local execution platform's C/C++ tools;
those actions run locally and are not uploaded to a remote cache. A declared,
content-addressed native compiler and sysroot are required before BSMR can call
native-extension builds fully hermetic or remotely reusable.

Until those gates land, describe this surface as a pinned uv, Ruff, and ty
adapter with deterministic BSMR action caching. Unsupported native toolchains,
lock schemas, or build closures should fail explicitly; BSMR does not select a
second package manager or silently resolve a different dependency set.
