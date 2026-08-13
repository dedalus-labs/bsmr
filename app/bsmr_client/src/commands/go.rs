//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Orchestrates native Go toolchain acquisition and package-graph synchronization.

//! Defines the boundary between Go's selected graph and Bessemer-owned build IR.
//!
//! The selected official SDK remains the semantic authority for packages, build
//! constraints, tests, and embeds. This module runs that SDK without ambient Go
//! state or network access, then passes its metadata through `GoGraph` into
//! frontend-owned manifests.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use bsmr_client_ctx::client_ctx::ClientCommandContext;
use bsmr_client_ctx::common::BuckArgMatches;
use bsmr_common::argv::Argv;
use bsmr_common::argv::SanitizedArgv;
use bsmr_common::legacy_configs::cells::BsmrConfigBasedCells;
use bsmr_common::legacy_configs::key::BsmrconfigKeyRef;

use crate::commands::go_graph::GoGraph;
use crate::commands::go_manifest::SyncMode;
use crate::commands::go_manifest::sync_manifests;
use crate::commands::go_toolchain;

/// Native Go ecosystem commands.
#[derive(Debug, clap::Parser)]
#[clap(name = "go", about = "Build Go package graphs with Bessemer")]
pub struct GoCommand {
    #[clap(subcommand)]
    command: GoSubcommand,
}

#[derive(Debug, clap::Subcommand)]
enum GoSubcommand {
    /// Pin or verify the exact official Go SDK used by Bessemer.
    Toolchain(GoToolchainCommand),

    /// Synchronize Go SDK metadata into generated Bessemer manifests.
    Sync(GoSyncCommand),
}

/// Options controlling exact official Go SDK selection.
#[derive(Debug, clap::Parser)]
struct GoToolchainCommand {
    /// Exact stable release to select and acquire.
    #[clap(long)]
    version: Option<String>,

    /// Update an existing lock to the latest stable release from go.dev.
    #[clap(long, conflicts_with_all = ["check", "version"])]
    update: bool,

    /// Verify the committed lock and generated toolchain without network access.
    #[clap(long)]
    check: bool,
}

/// Options controlling deterministic Go package-graph synchronization.
#[derive(Debug, clap::Parser)]
struct GoSyncCommand {
    /// Verify generated manifests and their ownership index without changing files.
    #[clap(long)]
    check: bool,

    /// Override the configured build-file name generated in each Go package.
    #[clap(long)]
    buildfile: Option<String>,

    /// Allowlisted build tags used for both SDK selection and action identity.
    #[clap(long, value_delimiter = ',')]
    tags: Vec<String>,

    /// Include cgo-selected files using the host-native C/C++ toolchain.
    #[clap(long)]
    cgo: bool,
}

impl GoCommand {
    /// Executes a native Go command without starting the Bessemer daemon.
    pub fn exec(
        self,
        _matches: BuckArgMatches<'_>,
        ctx: ClientCommandContext<'_>,
    ) -> bsmr_error::Result<()> {
        ctx.with_runtime(|ctx| async move {
            match self.command {
                GoSubcommand::Toolchain(command) => configure_toolchain(command, &ctx).await,
                GoSubcommand::Sync(command) => sync(command, &ctx),
            }
        })
    }

    /// Declares that native Go arguments contain no credential-bearing values.
    pub fn sanitize_argv(&self, argv: Argv) -> SanitizedArgv {
        argv.no_need_to_sanitize()
    }
}

/// Pins or verifies the generated toolchain at the Bessemer project root.
async fn configure_toolchain(
    command: GoToolchainCommand,
    ctx: &ClientCommandContext<'_>,
) -> bsmr_error::Result<()> {
    let root = ctx.paths()?.project_root().root().as_path();
    let lock = go_toolchain::configure(
        root,
        command.version.as_deref(),
        command.update,
        command.check,
    )
    .await?;
    if command.check {
        go_toolchain::acquired_go(root, &lock)?;
    } else {
        go_toolchain::prepare_acquisition(root, &lock)?;
        let sdk = materialize_sdk_archive(root)?;
        go_toolchain::install_sdk(root, &sdk, &lock)?;
    }
    bsmr_client_ctx::println!(
        "Go SDK {}: {}",
        if command.check { "verified" } else { "pinned" },
        lock.version()
    )?;
    Ok(())
}

/// Synchronizes the SDK graph into frontend-owned build manifests.
fn sync(mut command: GoSyncCommand, ctx: &ClientCommandContext<'_>) -> bsmr_error::Result<()> {
    command.tags.sort();
    command.tags.dedup();
    let buildfile = resolve_buildfile(command.buildfile.as_deref(), &command.tags, ctx)?;
    let working_dir = ctx.working_dir.path().as_path();
    let root = std::fs::canonicalize(working_dir).map_err(|error| GoCommandError::Io {
        operation: "canonicalize synchronization root",
        path: working_dir.to_owned(),
        message: error.to_string(),
    })?;
    if !root.join("go.mod").is_file() && !root.join("go.work").is_file() {
        return Err(GoCommandError::NoModule(root).into());
    }
    let project_root = ctx.paths()?.project_root().root().as_path();
    let lock = go_toolchain::read_lock(project_root)?;
    let go = go_toolchain::acquired_go(project_root, &lock)?;
    let output = run_go_list(&command, &root, &go)?;
    let graph = GoGraph::from_go_list(&output, &root)?;
    if graph.packages().is_empty() {
        return Err(GoCommandError::NoPackages(root).into());
    }
    let mode = if command.check {
        SyncMode::Check
    } else {
        SyncMode::Write
    };
    let report = sync_manifests(&root, &graph, &buildfile, &command.tags, command.cgo, mode)?;
    bsmr_client_ctx::println!(
        "Go graph: {} packages, {} manifests written, {} removed",
        graph.packages().len(),
        report.written(),
        report.removed()
    )?;
    Ok(())
}

/// Resolves the generator destination from the active cell configuration.
fn resolve_buildfile(
    override_name: Option<&str>,
    build_tags: &[String],
    ctx: &ClientCommandContext<'_>,
) -> bsmr_error::Result<String> {
    let paths = ctx.paths()?;
    let cells = futures::executor::block_on(BsmrConfigBasedCells::parse_with_config_args(
        paths.project_root(),
        &[],
    ))?;
    let cell = cells.cell_resolver.find(&paths.roots.cwd);
    let config = futures::executor::block_on(cells.parse_single_cell(cell, paths.project_root()))?;
    let allowed_tags = config
        .parse_list::<String>(BsmrconfigKeyRef {
            section: "go",
            property: "allowed_build_tags",
        })?
        .unwrap_or_default();
    validate_build_tags(build_tags, &allowed_tags)?;
    if let Some(buildfile) = override_name {
        validate_buildfile(buildfile)?;
        return Ok(buildfile.to_owned());
    }
    Ok(select_buildfile(
        config.parse_list::<String>(BsmrconfigKeyRef {
            section: "buildfile",
            property: "name_v2",
        })?,
        config.parse_list::<String>(BsmrconfigKeyRef {
            section: "buildfile",
            property: "name",
        })?,
    )?)
}

/// Requires every graph-selection tag to exist in Bessemer's configuration space.
pub(super) fn validate_build_tags(
    requested: &[String],
    allowed: &[String],
) -> Result<(), GoCommandError> {
    let missing = requested
        .iter()
        .filter(|tag| !allowed.contains(tag))
        .cloned()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(GoCommandError::UnconfiguredBuildTags(missing))
    }
}

/// Selects one configured build-file name or requires an explicit override.
pub(super) fn select_buildfile(
    name_v2: Option<Vec<String>>,
    name: Option<Vec<String>>,
) -> Result<String, GoCommandError> {
    let configured = name_v2
        .or(name)
        .unwrap_or_else(|| vec!["BUILD.bsmr".to_owned()]);
    if configured.len() != 1 {
        return Err(GoCommandError::AmbiguousBuildfiles(configured));
    }
    let buildfile = configured
        .into_iter()
        .next()
        .ok_or(GoCommandError::AmbiguousBuildfiles(Vec::new()))?;
    validate_buildfile(&buildfile)?;
    Ok(buildfile)
}

/// Runs the exact Go SDK in offline, non-auto-upgrading mode.
fn run_go_list(command: &GoSyncCommand, root: &Path, go: &Path) -> Result<Vec<u8>, GoCommandError> {
    let scratch = tempfile::Builder::new()
        .prefix("bsmr-go-list-")
        .tempdir()
        .map_err(|error| GoCommandError::Io {
            operation: "create isolated Go metadata state",
            path: std::env::temp_dir(),
            message: error.to_string(),
        })?;
    let mut process = Command::new(go);
    process.args(["list", "-deps", "-json", "-test"]);
    process.arg(if root.join("vendor/modules.txt").is_file() {
        "-mod=vendor"
    } else {
        "-mod=readonly"
    });
    if !command.tags.is_empty() {
        process.args(["-tags", &command.tags.join(",")]);
    }
    process.args(discover_patterns(root)?);
    process
        .current_dir(root)
        .env_clear()
        .env("CGO_ENABLED", if command.cgo { "1" } else { "0" })
        .env("GOCACHE", scratch.path().join("cache"))
        .env("GOENV", "off")
        .env("GOFLAGS", "")
        .env("GOMODCACHE", scratch.path().join("modules"))
        .env("GOPATH", scratch.path().join("gopath"))
        .env("GOPROXY", "off")
        .env("GOSUMDB", "off")
        .env("GOTOOLCHAIN", "local")
        .env("GOWORK", go_work(root))
        .env("TMPDIR", scratch.path());
    let output = process.output().map_err(|error| GoCommandError::Spawn {
        executable: go.to_owned(),
        message: error.to_string(),
    })?;
    if !output.status.success() {
        return Err(GoCommandError::GoList {
            executable: go.to_owned(),
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(output.stdout)
}

/// Materializes the selected official archive during the explicit acquisition command.
fn materialize_sdk_archive(root: &Path) -> Result<PathBuf, GoCommandError> {
    let executable =
        std::env::current_exe().map_err(|error| GoCommandError::CurrentExecutable {
            message: error.to_string(),
        })?;
    let output = Command::new(&executable)
        .args([
            "build",
            "toolchains//:go_sdk_archive",
            "--show-full-json-output",
            "--console=none",
        ])
        .current_dir(root)
        .output()
        .map_err(|error| GoCommandError::Spawn {
            executable: executable.clone(),
            message: error.to_string(),
        })?;
    if !output.status.success() {
        return Err(GoCommandError::SdkBuild {
            status: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).trim().to_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    parse_sdk_output(&output.stdout)
}

/// Extracts the single SDK directory from Bessemer's full output map.
pub(super) fn parse_sdk_output(stdout: &[u8]) -> Result<PathBuf, GoCommandError> {
    let stdout = String::from_utf8_lossy(stdout);
    let line = stdout
        .lines()
        .rev()
        .find(|line| line.starts_with('{'))
        .ok_or(GoCommandError::SdkOutput)?;
    let outputs = serde_json::from_str::<BTreeMap<String, String>>(line)
        .map_err(|_| GoCommandError::SdkOutput)?;
    if outputs.len() != 1 {
        return Err(GoCommandError::SdkOutput);
    }
    outputs
        .into_values()
        .next()
        .map(PathBuf::from)
        .ok_or(GoCommandError::SdkOutput)
}

/// Discovers recursive module roots without traversing Bessemer output trees.
pub(super) fn discover_patterns(root: &Path) -> Result<Vec<String>, GoCommandError> {
    let mut patterns = Vec::new();
    if contains_go_source(root)? {
        patterns.push(".".to_owned());
    }
    let entries = fs::read_dir(root).map_err(|error| GoCommandError::Io {
        operation: "read synchronization root",
        path: root.to_owned(),
        message: error.to_string(),
    })?;
    let mut directories = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| GoCommandError::Io {
            operation: "read synchronization root entry",
            path: root.to_owned(),
            message: error.to_string(),
        })?;
        let file_type = entry.file_type().map_err(|error| GoCommandError::Io {
            operation: "inspect synchronization root entry",
            path: entry.path(),
            message: error.to_string(),
        })?;
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| GoCommandError::NonUtf8Entry(entry.path()))?;
        if file_type.is_dir() && !ignored_root_directory(name) {
            directories.push(name.to_owned());
        }
    }
    directories.sort();
    patterns.extend(directories.into_iter().map(|name| format!("./{name}/...")));
    if patterns.is_empty() {
        return Err(GoCommandError::NoPackages(root.to_owned()));
    }
    Ok(patterns)
}

/// Reports whether the module root itself contains a Go source file.
fn contains_go_source(root: &Path) -> Result<bool, GoCommandError> {
    for entry in fs::read_dir(root).map_err(|error| GoCommandError::Io {
        operation: "read synchronization root",
        path: root.to_owned(),
        message: error.to_string(),
    })? {
        let entry = entry.map_err(|error| GoCommandError::Io {
            operation: "read synchronization root entry",
            path: root.to_owned(),
            message: error.to_string(),
        })?;
        if entry
            .file_type()
            .map_err(|error| GoCommandError::Io {
                operation: "inspect synchronization root entry",
                path: entry.path(),
                message: error.to_string(),
            })?
            .is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "go")
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Mirrors Go wildcard exclusions and adds Bessemer's materialization root.
fn ignored_root_directory(name: &str) -> bool {
    name == "buck-out"
        || name == "testdata"
        || name == "vendor"
        || name.starts_with('.')
        || name.starts_with('_')
}

/// Selects an explicit workspace file or disables ambient workspace discovery.
fn go_work(root: &Path) -> OsString {
    let workspace = root.join("go.work");
    if workspace.is_file() {
        workspace.into_os_string()
    } else {
        OsString::from("off")
    }
}

/// Rejects build-file names that could escape a package directory.
fn validate_buildfile(buildfile: &str) -> Result<(), GoCommandError> {
    let path = Path::new(buildfile);
    if buildfile.is_empty() || path.file_name() != Some(path.as_os_str()) {
        return Err(GoCommandError::InvalidBuildfile(buildfile.to_owned()));
    }
    Ok(())
}

/// Fail-closed native Go command errors.
#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Input)]
pub(super) enum GoCommandError {
    #[error("Go synchronization root `{0:?}` has neither go.mod nor go.work")]
    NoModule(PathBuf),
    #[error("Go synchronization root `{0:?}` contains no package roots")]
    NoPackages(PathBuf),
    #[error("Go synchronization root contains a non-UTF-8 entry `{0:?}`")]
    NonUtf8Entry(PathBuf),
    #[error("Go build-file name must be a single non-empty path component, got `{0}`")]
    InvalidBuildfile(String),
    #[error(
        "multiple Bessemer build-file names are configured ({0:?}); select one with `--buildfile`"
    )]
    AmbiguousBuildfiles(Vec<String>),
    #[error("Go build tags {0:?} are not declared in `.bsmr` under `go.allowed_build_tags`")]
    UnconfiguredBuildTags(Vec<String>),
    #[error("failed to {operation} at `{path:?}`: {message}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        message: String,
    },
    #[error("failed to execute Go SDK `{executable:?}`: {message}")]
    Spawn {
        executable: PathBuf,
        message: String,
    },
    #[error("failed to resolve the Bessemer executable: {message}")]
    CurrentExecutable { message: String },
    #[error("failed to materialize the locked Go SDK with status {status:?}: {stdout} {stderr}")]
    SdkBuild {
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error("Bessemer emitted no unique output for `toolchains//:go_sdk_archive`")]
    SdkOutput,
    #[error("`{executable:?} list` failed with status {status:?}: {stderr}")]
    GoList {
        executable: PathBuf,
        status: Option<i32>,
        stderr: String,
    },
}
