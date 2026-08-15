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

The built-in catalog currently pins CPython 3.14.7, uv 0.12.5, Ruff 0.16.3,
and ty 0.0.72 by platform artifact digest. BSMR never consults an activated
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
First-party wheel targets require an explicit PEP 517 `[build-system]`. BSMR
validates that every `[build-system].requires` and
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

A PEP 751 `packages.directory` entry must identify the root project or one of
those selected workspace members by normalized distribution name and exact
root-relative path. BSMR reuses that project's declared wheel action; it does
not create a second editable installation of the same mutable tree. Absolute,
parent-relative, host-specific, marker-varying, and undeclared local paths fail
during analysis. The lock's `editable` flag records authoring intent but does
not alter the immutable production graph. Local directory build requirements
are rejected because making a first-party wheel part of its own shared build
environment would introduce a cycle; publish or vendor a wheel or source
archive instead.

## Configuration

BSMR deliberately uses each tool's native configuration rather than defining a
second Python DSL:

- PEP 517 backend settings come from uv's native
  [`[tool.uv].config-settings`](https://docs.astral.sh/uv/reference/settings/#config-settings)
  and are forwarded directly as typed, repeated hook `config-settings`;
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
BSMR-specific scope. Ruff receives the complete declared project tree so native
`include` and `extend-include` patterns can select non-Python extensions; ty
receives only Python sources and its configuration files.

Ruff runs with its local cache disabled because BSMR caches the complete action
by tool, configuration, and project-source digest. ty receives independently
cacheable dependency and first-party wheel layers. First-party PEP 517 builds
invoke the declared backend directly inside the pinned interpreter and frozen
build environment; they do not start uv or rediscover backend requirements.
The remaining third-party compatibility path gives uv the same interpreter,
isolated home and cache directories, disabled Python downloads, and no ambient
user configuration.

All settings above are explicit action inputs. Global settings apply to each
build; package-specific settings and environment variables apply only to their
normalized distribution. BSMR presents them directly to first-party backends
and writes an isolated uv configuration only for third-party builds that still
use uv's compatibility path. Changes invalidate only the actions whose
declared settings changed.

A backend that derives a dynamic version from Git must declare Git as native uv
cache state. BSMR treats the same declaration as an explicit build input:

```toml title="pyproject.toml"
[project]
dynamic = ["version"]

[tool.uv]
cache-keys = [{ file = "pyproject.toml" }, { git = { commit = true } }]
```

The wheel action then consumes the repository's read-only commit database and
invalidates when that identity changes. BSMR keeps only `.git/HEAD`, packed and
loose refs, shallow state, and the object database visible to the file watcher;
mutable Git state such as the index and logs remains ignored. A custom
dynamic-version function that reads Git without this declaration is undeclared
behavior; BSMR deliberately does not infer it by executing arbitrary backend
code during analysis.

The current Git adapter requires an ordinary checkout whose `.git/HEAD`, refs,
and object database are project-local files. A linked worktree whose `.git` is
an external indirection file fails before backend execution; declaring that
external Git database hermetically remains a release gate.

First-party pre-PEP 517 projects fail analysis because their backend contract is
not explicit. A locked third-party sdist may still use uv's legacy PEP 517
adapter with an explicit build closure. That compatibility path is discouraged:
new projects should declare `[build-system]`, and existing projects should
migrate rather than add more undeclared `setup.py` behavior.

## Correctness and caching

Dependency environments, first-party wheels, lint, typecheck, and tests are
separate actions. Source-controlled virtual environments are detected by
`pyvenv.cfg` and pruned before package discovery. Generated caches, lockfiles,
build outputs, and virtual environments never become first-party source inputs.

For already-selected wheels, BSMR's native installer validates normalized
archive paths, regular filesystem kinds, a single coherent release identity,
filename-consistent compatibility tags and build identity, complete strong
RECORD hashes and sizes, install schemes, and entry points before recording the
immutable result in BSMR's CAS. It does not start a resolver or installer
subprocess. Ambiguous wheel sets and source artifacts use `uv pip sync
--strict` against one canonical single-package PEP 751 fragment, normalize
console-script shebangs, reject symlinks and special files, and record the same
immutable result shape.
BSMR partitions complete size- and SHA-256-pinned wheel metadata by configured
Python line, execution OS, and CPU using Astral's wheel filename and platform
tag libraries. Pinned uv then evaluates the lock's markers and versions in an
offline, binary-only dry run. The selected exact requirement is installed from
only those local candidates with index, dependency, build, and network access
disabled. A source result does not consume or download wheel candidates.

An unconditional package may bypass uv's dry-run selection when BSMR can prove
one unique best wheel for every supported Python and execution platform. BSMR
uses Astral's wheel-tag priority followed by the wheel build tag, which is the
same ordering uv applies. A single `py3-none-any` or
`py2.py3-none-any` wheel is the simplest instance of this proof. Equal best
priorities, incomplete platform coverage, package-level `requires-python`,
and environment-dependent variants remain delegated to pinned uv.

Package-marker proofs use Astral's canonical PEP 508 marker algebra; BSMR does
not interpret marker strings with a second parser. Direct acquisition always
requires a credential-free HTTP(S) URL and a wheel filename matching the
locked distribution and version. Wheels with incomplete download metadata and
source distributions continue through the canonical PEP 751 fragment so uv
remains responsible for compatibility selection.

BSMR composes those package trees deterministically, rejects incompatible
import-file collisions, applies uv-compatible first-package precedence to
console scripts, and records all owners when identical files are shared.
First-party wheels are built by their declared PEP 517 hook, verified with the
same wheel invariants, and overlaid as a separate content-addressed layer. Warm
no-op builds and deleted-output restoration therefore require no Python tool,
installer, or backend execution.

Successful Python test commands opt into BSMR's action cache. Repeating the
same target with the same interpreter, environments, sources, command, and
arguments restores the result record without starting Python. Failed tests are
never cached. A changed test input produces a new action identity and executes
the selected command normally.

Digest-pinned HTTP artifacts, including the built-in Python tools, are shared
across repositories in the operating system's user cache under
`bsmr/http-v1`. Set `BSMR_HTTP_CACHE_DIR` to an absolute path to relocate it.
Every restoration revalidates the declared checksum before the artifact enters
an action. `bsmr clean` preserves this repository-independent cache, and BSMR
does not currently evict it automatically; remove the configured directory
while no builds are running to reclaim it in full.

Successful actions are also shared across repositories and worktrees through a
local action cache and content-addressed store under `bsmr/action-v1`. Set
`BSMR_LOCAL_CACHE_DIR` to an absolute path to relocate it. Each action manifest
is published only after its complete file and directory-tree closure. A missing
object is a cache miss; malformed metadata, an unexpected digest family, and a
size or content-digest mismatch fail closed. A cached manifest must also name
exactly the action's declared output set; it cannot redirect restoration into
another project path.

This cache restores exact outputs, stdout, and stderr without re-running uv,
Python, Ruff, ty, an archive extractor, or a PEP 517 backend. Portable
`py*-none-any` first-party wheels may be reused across local worktrees. Native
and platform-tagged wheel builds remain local executions until their compiler
and sysroot become declared toolchains. This distinction is an upload policy,
not a fallback between two build implementations.

BSMR validates the complete tree closure before accepting a hit and verifies
each file's content digest while restoring it. The action cache currently has
no automatic garbage collector. `bsmr clean` preserves it; remove the
configured directory while no builds are running to reclaim it in full.

## BSMR or Bazel?

For a conventional local Python project or uv workspace, BSMR is the recommended
default. It preserves `pyproject.toml`, PEP 751, and the declared PEP 517 backend
as the user-facing contract; pins the Astral tools and interpreter; generates
the action graph without BUILD files; and shares verified action results across
worktrees automatically.

The reproducible Django comparison against Bazel 9.2.0 and rules_python 2.3.0
passed exact source, Git-derived release identity, import, test, entry-point,
wheel metadata, RECORD, payload, incremental, and restoration gates. It used
Bazel's latest stable release, the latest rules_python release, and that
ruleset's pinned CPython 3.14.4 against BSMR's CPython 3.14.7.

| Local Django regime | Result |
| --- | ---: |
| Empty acquisition | BSMR 1.79x faster |
| Provisioned, no action results | BSMR 1.63x faster |
| Shared cache, fresh checkout | BSMR 6.29x faster |
| Resident no-op | BSMR 8.50x faster |
| First test | BSMR 2.31x faster |
| Cached test | BSMR 5.65x faster |
| Source edit, runtime target | BSMR 2.74x faster |
| Source edit, test execution | parity; BSMR 1.02x faster |
| Source edit, full PEP 517 wheel | parity; Bazel 1.08x faster |
| Deleted wheel restoration | BSMR 4.25x faster |

The parity rows execute the same Python test workload or the same setuptools
backend. Bazel's separate archive-only `py_wheel` control was 8.12x faster than
either full PEP 517 build because it skips the backend and duplicates
distribution metadata in BUILD syntax. That is a useful lower bound, not an
equivalent build. BSMR will not silently substitute it for the project contract.

This is also a corpus result, not a Django party trick. The pinned RFC 0004
gate passes NVIDIA Cosmos Cookbook's `uv_build` project, Dedalus Agents
Python's Hatchling project, and Pydantic AI's four-project dynamic-version uv
workspace. Across those checkouts, BSMR and pinned uv agree on 110 runtime
distributions, 7,776 runtime files, 17 build distributions, 418 build files,
first-party wheel payloads, imports, entry points, executable bits, and missing
import failures. Pydantic AI's 7,462-file environment reaches a 36 ms resident
no-op on the reference machine.

For this native local-project contract, choose BSMR. It removes BUILD-file
metadata duplication, preserves the project's actual PEP 517 behavior, and
shares verified results across worktrees while beating the tuned Bazel control
by 1.63x to 8.50x wherever graph construction, invalidation, or caching can
differentiate the systems. Choose Bazel today when its mature remote-execution,
sandboxing, query, or IDE ecosystem is itself a requirement; those surfaces are
outside this benchmark and BSMR does not claim otherwise.

See the [benchmark contract and reproduction
instructions](https://github.com/dedalus-labs/bsmr/blob/main/benchmarks/README.md#python-build-systems).
These claims cover the measured local Python path, not unimplemented product
surfaces.

## Hermeticity boundary

The native local path now delivers the central execution design from
[RFC 0004](https://github.com/dedalus-labs/bsmr/discussions/16): standard
PEP 751 inputs, digest-pinned tools and artifacts, a package-granular graph,
native wheel installation, direct frozen PEP 517 execution, and verified local
CAS reuse. uv remains the lock author and the specialist for compatibility
selection, source builds, and legacy third-party projects; it is no longer on
the selected-wheel or first-party wheel critical path.

Portable offline artifact replay after an action-cache miss is implemented for
complete HTTP(S) wheel, sdist, and archive records. Each compatible candidate
is a separately acquired, size- and checksum-verified action input, and its
repository-independent HTTP cache entry is revalidated on every restoration.
Python and platform selects are part of the action graph, so one compiled
package consumes only the candidates for its exact configured Python line and
execution platform. Credential-free HTTPS Git sources pinned to a full object
ID are acquired as separate source-tree actions. When a source artifact still
requires uv, pinned uv consumes it offline and BSMR verifies the resulting
distribution name and version before admitting it to the package graph.

Local directory sources map to declared first-party wheel targets as described
above. A cold action for a local archive path, local VCS path, unsupported VCS,
marker-varying source, or artifact with incomplete acquisition metadata may
still ask uv to consume the canonical one-package PEP 751 fragment. Declared
PEP 794 import ownership, cross-layer collision validation, native wheel
materialization, and differential conformance gates are implemented. Static
import inference for the smallest sound dependency closure and complete
provenance queries remain RFC work.

Pure-Python PEP 517 builds use the exact interpreter and locked build closure.
The backend receives a scratch copy rather than the declared source artifact,
so setuptools-style `build/` and `*.egg-info` writes cannot mutate another
action's input. Entry points and tests disable bytecode writes before importing
project code for the same reason. BSMR imports the declared backend through its
`backend-path`, stdlib, build environment, and `.pth` files in that order, then
calls `build_wheel` directly. It never calls `get_requires_for_build_wheel`
after graph freeze. Missing dynamic build requirements therefore fail instead
of mutating the resolved graph. `.pth` paths must remain inside the declared
build environment, ambient site packages are excluded, source timestamps are
normalized, and the running interpreter must match the pinned toolchain.

Local execution does not yet prevent a PEP 517 backend from opening its own
network connection. Offline artifact acquisition and isolated process state
make compliant builds reproducible, but kernel-enforced network denial remains
an RFC gate. The current linked-worktree Git adapter and explicit `uv.lock`
compatibility provider also remain incomplete.

Native extensions currently use the local execution platform's C/C++ tools;
those actions run locally and are not uploaded to a remote cache. A declared,
content-addressed native compiler and sysroot are required before BSMR can call
native-extension builds fully hermetic or remotely reusable.

BSMR disables compiler debug sections for wheel builds so absolute action
scratch paths do not make otherwise identical extension modules diverge. A
Dedalus API fixture with Cython-backed `pyiceberg` and `pyroaring` extensions
reproduces byte-for-byte across independent cold uv and BSMR build roots.

The other unfinished RFC surfaces are queryable provenance and dependency
explanations, general named resolves beyond test profiles, remote reuse for
native builds, and mature query, IDE, and coverage integrations. Unsupported
native toolchains, lock schemas, or build closures fail explicitly; BSMR does
not select a second package manager or silently resolve a different dependency
set.
