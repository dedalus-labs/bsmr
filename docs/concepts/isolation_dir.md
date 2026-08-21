---
id: isolation_dir
title: Isolation Directory
---
<!-- ===----------------------------------------------------------------------=== -->
<!-- Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc -->
<!-- Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- ===----------------------------------------------------------------------=== -->


# Isolation Directory

## What is an Isolation Directory?

An isolation directory is a core mechanism in Bessemer that enables multiple
independent daemon instances to run concurrently. Each Bessemer daemon operates
within its own isolation directory, creating a completely separated environment
for build processes.

The isolation directory serves as a fundamental boundary that:

- Separates cached artifacts between different daemon instances
- Provides independent build environments with no shared state
- Allows multiple Bessemer commands to run in parallel

## How Isolation Directories Work

### Physical Structure

The isolation directory exists as a subdirectory within the `bsmr-out` folder:

```
project_root/
└── bsmr-out/
    ├── default/       # Default isolation directory
    │   ├── artifacts/
    │   ├── cache/
    │   └── ...
    ├── custom_name/   # Custom isolation directory
    │   ├── artifacts/
    │   ├── cache/
    │   └── ...
    └── ...
```

By default, Bessemer uses an isolation directory named `default`, creating all build
outputs and metadata within `$PROJECT_ROOT/bsmr-out/default`.

### Important Characteristics

1. **Independent Caching**:
   - Each isolation directory maintains its own separate cache
   - No cached artifacts or memory cache is shared between different isolation
     directories

2. **Command Execution Isolation**:
   - A single Bessemer daemon can generally execute only one command at a time
   - Different daemons with different isolation directories can execute commands
     concurrently

3. **Resource Implications**:
   - Using multiple isolation directories requires additional system resources
   - Each directory may duplicate build artifacts, consuming more disk space,
     memory, and potentially network bandwidth

:::warning **Resource Usage Warning**: Using multiple isolation directories can
significantly increase resource consumption due to duplicated caches and
artifacts. Each isolation directory requires its own memory, disk space, and
potentially network usage. :::

## When to Use Different Isolation Directories

Isolation directories are particularly useful in the following scenarios:

### 1. Developer Environment Tooling

Background services like Language Server Protocols (LSPs) can run in their own
isolation directory without interfering with manually triggered builds.

```sh
# Running LSP in its own isolation directory
$ bsmr --isolation-dir lsp lsp
```

### 2. Recursive Invocations

When Bessemer needs to be called from within another Bessemer process, using different
isolation directories prevents deadlocks and conflicts.

```sh
# Initial build
$ bsmr build //some:target

# Within this build, Bessemer might make another call using a different isolation dir
$ bsmr --isolation-dir recursive_dir //dependency:target
```

### 3. Parallel Workflows

When you need to run multiple independent build tasks simultaneously:

```sh
# Building the application in one terminal
$ bsmr build //app:binary

# Running tests in another terminal simultaneously
$ bsmr --isolation-dir test_dir test //app:tests
```

## How to Set the Isolation Directory

There are two ways to specify which isolation directory to use:

### 1. Command Line Argument

```sh
$ bsmr --isolation-dir DIRECTORY_NAME COMMAND [ARGS]
```

**Important**: The `--isolation-dir` argument must always appear immediately
after `bsmr`. For example, `bsmr build --isolation-dir experiment target` is not
valid.

### 2. Environment Variable

```sh
$ BSMR_ISOLATION_DIR=DIRECTORY_NAME bsmr COMMAND [ARGS]
```

If not specified, the default isolation directory name is `default`.

## Command Scope and Isolation Directories

Most Bessemer commands only operate within their specified isolation directory. For
example:

- `bsmr build` only builds using the specified isolation directory
- `bsmr clean` only cleans the specified isolation directory
- `bsmr kill` only kills the daemon associated with the specified isolation
  directory

There are exceptions, such as `bsmr killall`, which affects all Bessemer processes
regardless of their isolation directories.

## Example Use Cases

### Typical Development Workflow

```sh
# Using the default isolation directory
$ bsmr build //app:binary
$ bsmr run //app:binary
```

### Running Background Analysis Services

```sh
# Start a language server in a dedicated isolation directory
$ bsmr --isolation-dir ide lsp &

# Continue with regular builds in the default isolation directory
$ bsmr build //app:binary
```

### Comparing Different Build Configurations

```sh
# Build with one set of configurations
$ bsmr --isolation-dir config1 build //app:binary

# Build with different configurations in a separate isolation directory
$ bsmr --isolation-dir config2 build //app:binary
```
