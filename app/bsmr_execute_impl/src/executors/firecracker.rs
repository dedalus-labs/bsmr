//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Runs declared actions inside one-action Firecracker microVMs.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::IoSlice;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use bsmr_common::cas_digest::DigestAlgorithmFamily;
use bsmr_common::cas_digest::Digester;
use bsmr_common::file_ops::metadata::FileDigest;
use bsmr_common::file_ops::metadata::FileDigestKind;
use bsmr_common::liveliness_observer::LivelinessObserver;
use bsmr_directory::directory::directory::Directory;
use bsmr_directory::directory::directory_iterator::DirectoryIterator;
use bsmr_directory::directory::entry::DirectoryEntry;
use bsmr_execute::digest::CasDigestFromReExt;
use bsmr_execute::digest_config::DigestConfig;
use bsmr_execute::directory::ActionDirectoryMember;
use bsmr_execute::directory::ActionImmutableDirectory;
use bsmr_execute::execute::prepared::PreparedAction;
use bsmr_execute::execute::request::CommandExecutionRequest;
use bsmr_execute::execute::request::NetworkAccess;
use bsmr_execute::execute::request::OutputType;
use bsmr_execute_local::CommandResult;
use bsmr_execute_local::GatherOutputStatus;
use bsmr_sandbox::BundleTrust;
use bsmr_sandbox::GuestAction;
pub use bsmr_sandbox::GuestOutput;
pub use bsmr_sandbox::GuestOutputKind;
use bsmr_sandbox::GuestResultEnvelope;
use bsmr_sandbox::LauncherRequest;
pub use bsmr_sandbox::LauncherResponse;
pub use bsmr_sandbox::LauncherStatus;
use bsmr_sandbox::MAX_INPUT_BYTES;
use bsmr_sandbox::MAX_OUTPUT_ARCHIVE_BYTES;
use bsmr_sandbox::MAX_OUTPUT_BYTES;
use bsmr_sandbox::MAX_TIMEOUT_MS;
use bsmr_sandbox::MEMORY_MIB;
use bsmr_sandbox::PROTOCOL_VERSION;
use bsmr_sandbox::VCPU_COUNT;
use bsmr_sandbox::VerifiedBundle;
use prost::Message;
use remote_execution as RE;

const SANDBOX_PROFILE: &str = "untrusted-v1";
const SANDBOX_PROTOCOL: &str = "1";

#[derive(Debug, bsmr_error::Error)]
#[bsmr(input)]
enum FirecrackerSandboxError {
    #[error("Firecracker sandboxing requires Linux; this host is `{0}`")]
    UnsupportedOs(String),
    #[error("Firecracker sandboxing currently requires x86_64; this host is `{0}`")]
    UnsupportedArchitecture(String),
    #[error("Firecracker sandboxing requires cgroup v2")]
    CgroupV2Unavailable,
    #[error("Firecracker sandbox actions cannot inherit the host environment")]
    InheritedEnvironment,
    #[error("Firecracker sandbox actions cannot use persistent workers")]
    PersistentWorker,
    #[error("Firecracker sandbox protocol v1 requires network_access = none")]
    NetworkAccess,
    #[error("invalid Firecracker execution bundle: {0}")]
    Bundle(String),
    #[error("Firecracker guest path must be normalized and project-relative: `{0:?}`")]
    GuestPath(PathBuf),
    #[error("Firecracker guest produced undeclared output `{0:?}`")]
    UndeclaredOutput(PathBuf),
    #[error("Firecracker guest output `{path:?}` is below file output `{root:?}`")]
    OutputBelowFile { path: PathBuf, root: PathBuf },
    #[error("Firecracker guest symlink `{path:?}` -> `{target:?}` escapes `{root:?}`")]
    OutputSymlinkEscape {
        path: PathBuf,
        target: PathBuf,
        root: PathBuf,
    },
    #[error("Firecracker launcher protocol must be 1, got {0}")]
    LauncherProtocol(u32),
    #[error("Firecracker launcher reported incomplete cleanup")]
    IncompleteCleanup,
    #[error("failed to read Firecracker guest output archive: {0}")]
    ReadOutputArchive(std::io::Error),
    #[error("Firecracker guest output archive path is invalid: {0}")]
    OutputArchivePath(std::io::Error),
    #[error("Firecracker guest output archive contains duplicate path `{0:?}`")]
    DuplicateOutput(PathBuf),
    #[error("Firecracker guest output archive entry has forbidden type at `{0:?}`")]
    OutputEntryType(PathBuf),
    #[error("Firecracker guest output `{path:?}` has type {actual}, expected {expected}")]
    OutputType {
        path: PathBuf,
        actual: &'static str,
        expected: &'static str,
    },
    #[error("Firecracker guest output archive exceeds the {kind} limit of {limit} bytes")]
    OutputLimit { kind: &'static str, limit: u64 },
    #[error("Firecracker guest output archive exceeds the node limit of {0}")]
    OutputNodeLimit(usize),
    #[error("Firecracker guest output archive path exceeds the depth limit of {0}")]
    OutputDepthLimit(usize),
    #[error("Firecracker guest output archive is missing `{0}`")]
    MissingEnvelope(&'static str),
    #[error("invalid Firecracker guest result: {0}")]
    ParseGuestResult(serde_json::Error),
    #[error("Firecracker guest protocol must be 1, got {0}")]
    GuestProtocol(u32),
    #[error("Firecracker output staging directory must be empty: `{0:?}`")]
    StagingNotEmpty(PathBuf),
    #[error("failed to materialize Firecracker guest output `{0:?}`: {1}")]
    WriteOutput(PathBuf, std::io::Error),
    #[error("failed to read Firecracker input `{0:?}`: {1}")]
    ReadInput(PathBuf, std::io::Error),
    #[error(
        "Firecracker input `{path:?}` changed after analysis: expected {expected}, got {actual}"
    )]
    InputMutation {
        path: PathBuf,
        expected: String,
        actual: String,
    },
    #[error("Firecracker input `{path:?}` is an external symlink to `{target:?}`")]
    ExternalInputSymlink { path: PathBuf, target: PathBuf },
    #[error("Firecracker input symlink `{path:?}` -> `{target:?}` escapes the action root")]
    InputSymlinkEscape { path: PathBuf, target: PathBuf },
    #[error("failed to write Firecracker input archive: {0}")]
    WriteInputArchive(std::io::Error),
    #[error("Firecracker action input exceeds the node limit of {0}")]
    InputNodeLimit(usize),
    #[error("Firecracker action input path exceeds the depth limit of {0}")]
    InputDepthLimit(usize),
    #[error("Firecracker launcher socket must be an absolute Unix socket: `{0:?}`")]
    LauncherSocket(PathBuf),
    #[error("Firecracker launcher I/O failed: {0}")]
    LauncherIo(#[source] std::io::Error),
    #[error("Firecracker launcher socket failed: {0}")]
    LauncherSocketIo(#[source] nix::Error),
    #[error("Firecracker launcher failed: {0}")]
    LauncherFailure(String),
    #[error("Firecracker launcher status and error payload disagree")]
    LauncherStatusShape,
    #[error("Firecracker launcher returned timeout without an action deadline")]
    MissingTimeout,
    #[error("Firecracker action timeout exceeds u64 milliseconds")]
    TimeoutOverflow,
    #[error("Firecracker action timeout exceeds {MAX_TIMEOUT_MS} ms")]
    TimeoutLimit,
    #[error("Firecracker guest action exceeds its fixed request device")]
    ActionTooLarge,
    #[error("Firecracker launcher request exceeds its protocol limit")]
    LauncherRequestTooLarge,
    #[error("Firecracker launcher response exceeds its protocol limit")]
    LauncherResponseTooLarge,
    #[error("Firecracker action metadata is missing its {0} blob")]
    MissingActionBlob(&'static str),
    #[error("failed to decode Firecracker {kind} blob: {error}")]
    DecodeActionBlob {
        kind: &'static str,
        error: prost::DecodeError,
    },
    #[error("Firecracker action command must contain an executable")]
    MissingExecutable,
    #[error("Firecracker protocol v1 requires a project-relative executable, got `{0}`")]
    AbsoluteExecutable(String),
    #[error("Firecracker protocol v1 does not support required local resources")]
    LocalResources,
    #[error("Firecracker protocol v1 does not support incremental output state")]
    IncrementalOutputs,
    #[error("Firecracker protocol v1 does not support actions that outlive their command")]
    DetachedProcess,
    #[error("failed to create Firecracker transport file: {0}")]
    CreateTransport(std::io::Error),
    #[error("failed to serialize Firecracker protocol message: {0}")]
    SerializeProtocol(serde_json::Error),
    #[error("failed to import Firecracker output `{0:?}`: {1}")]
    ImportOutput(PathBuf, std::io::Error),
    #[error("Firecracker output destination was not cleaned before execution: `{0:?}`")]
    OutputDestinationExists(PathBuf),
    #[error("Firecracker output declarations overlap: `{parent:?}` contains `{child:?}`")]
    OverlappingOutputs { parent: PathBuf, child: PathBuf },
}

/// A fully verified bundle and the identity injected into every action key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirecrackerBundle {
    verified: VerifiedBundle,
}

impl FirecrackerBundle {
    /// Loads a manifest, verifies every required file, and calculates its semantic identity.
    pub fn load(path: &Path, host_architecture: &str) -> bsmr_error::Result<Self> {
        let verified = VerifiedBundle::load(path, host_architecture, BundleTrust::Content)
            .map_err(|error| FirecrackerSandboxError::Bundle(error.to_string()))?;
        Ok(Self { verified })
    }

    /// Returns the digest that separates action caches across execution environments.
    #[must_use]
    pub fn environment_digest(&self) -> &str {
        self.verified.environment_digest()
    }
}

/// Host facts used by fail-closed preflight and its platform-independent tests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostCapabilities<'a> {
    pub os: &'a str,
    pub architecture: &'a str,
    pub cgroup_v2: bool,
}

impl HostCapabilities<'static> {
    /// Detects the minimum host capabilities required by Firecracker protocol v1.
    #[must_use]
    pub fn detect() -> Self {
        Self {
            os: std::env::consts::OS,
            architecture: std::env::consts::ARCH,
            cgroup_v2: Path::new("/sys/fs/cgroup/cgroup.controllers").is_file(),
        }
    }
}

/// Rejects hosts that cannot provide the promised Firecracker boundary.
pub fn validate_host(host: &HostCapabilities<'_>) -> bsmr_error::Result<()> {
    if host.os != "linux" {
        return Err(FirecrackerSandboxError::UnsupportedOs(host.os.to_owned()).into());
    }
    if host.architecture != "x86_64" {
        return Err(
            FirecrackerSandboxError::UnsupportedArchitecture(host.architecture.to_owned()).into(),
        );
    }
    if !host.cgroup_v2 {
        return Err(FirecrackerSandboxError::CgroupV2Unavailable.into());
    }
    Ok(())
}

/// Requires the configured launcher endpoint to be an existing absolute Unix socket.
fn validate_launcher_socket(path: &Path) -> bsmr_error::Result<()> {
    if !path.is_absolute() {
        return Err(FirecrackerSandboxError::LauncherSocket(path.to_owned()).into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;

        let metadata = fs::symlink_metadata(path).map_err(FirecrackerSandboxError::LauncherIo)?;
        if !metadata.file_type().is_socket() {
            return Err(FirecrackerSandboxError::LauncherSocket(path.to_owned()).into());
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        Err(FirecrackerSandboxError::UnsupportedOs(std::env::consts::OS.to_owned()).into())
    }
}

/// Network states understood by the Firecracker action-policy validator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkPolicy {
    None,
    All,
}

/// Host-environment behavior admitted by the Firecracker profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvironmentPolicy {
    Explicit,
    Inherited,
}

/// Worker behavior admitted by the Firecracker profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerPolicy {
    None,
    Persistent,
}

/// A proof that an action uses only semantics implemented by protocol v1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FirecrackerActionPolicy;

impl FirecrackerActionPolicy {
    /// Rejects host environment inheritance, workers, and network access.
    pub fn new(
        environment: EnvironmentPolicy,
        worker: WorkerPolicy,
        network: NetworkPolicy,
    ) -> bsmr_error::Result<Self> {
        if environment == EnvironmentPolicy::Inherited {
            return Err(FirecrackerSandboxError::InheritedEnvironment.into());
        }
        if worker == WorkerPolicy::Persistent {
            return Err(FirecrackerSandboxError::PersistentWorker.into());
        }
        if network != NetworkPolicy::None {
            return Err(FirecrackerSandboxError::NetworkAccess.into());
        }
        Ok(Self)
    }
}

/// A fail-closed client for the privileged Firecracker launcher.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirecrackerExecutor {
    bundle: FirecrackerBundle,
    launcher_socket: PathBuf,
}

impl FirecrackerExecutor {
    /// Verifies the host, launcher socket, and immutable execution bundle up front.
    pub fn new(bundle_manifest: &Path, launcher_socket: &Path) -> bsmr_error::Result<Self> {
        let host = HostCapabilities::detect();
        validate_host(&host)?;
        validate_launcher_socket(launcher_socket)?;
        let bundle = FirecrackerBundle::load(bundle_manifest, host.architecture)?;
        Ok(Self {
            bundle,
            launcher_socket: launcher_socket.to_owned(),
        })
    }

    /// Returns the environment digest injected into the canonical action key.
    #[must_use]
    pub fn environment_digest(&self) -> &str {
        self.bundle.environment_digest()
    }

    /// Executes one already-prepared action through the privileged launcher.
    pub async fn execute(
        &self,
        prepared: &PreparedAction,
        request: &CommandExecutionRequest,
        project_root: &Path,
        digest_config: DigestConfig,
        liveliness: &dyn LivelinessObserver,
    ) -> bsmr_error::Result<CommandResult> {
        validate_action_policy(prepared, request)?;
        let command = decode_re_command(prepared, digest_config)?;
        let action = guest_action(&command, request)?;

        let mut input = tempfile::tempfile().map_err(FirecrackerSandboxError::CreateTransport)?;
        write_input_archive(
            &mut input,
            project_root,
            request.paths().input_directory(),
            digest_config,
        )?;
        input
            .seek(SeekFrom::Start(0))
            .map_err(FirecrackerSandboxError::CreateTransport)?;
        let input_bytes = input
            .metadata()
            .map_err(FirecrackerSandboxError::CreateTransport)?
            .len();

        let action_file = write_guest_action(&action)?;
        let mut output = tempfile::tempfile().map_err(FirecrackerSandboxError::CreateTransport)?;
        output
            .set_len(MAX_OUTPUT_BYTES)
            .map_err(FirecrackerSandboxError::CreateTransport)?;

        let launcher_request = LauncherRequest {
            protocol: PROTOCOL_VERSION,
            action_id: uuid::Uuid::new_v4().to_string(),
            environment_digest: self.environment_digest().to_owned(),
            input_bytes,
            output_bytes: MAX_OUTPUT_BYTES,
            vcpu_count: VCPU_COUNT,
            memory_mib: MEMORY_MIB,
            timeout_ms: action.timeout_ms,
        };
        let response = launch(
            &self.launcher_socket,
            &launcher_request,
            &action_file,
            &input,
            &output,
            liveliness,
        )
        .await?;
        validate_launcher_response(&response)?;

        let status = match response.status {
            LauncherStatus::Completed => {
                output
                    .seek(SeekFrom::Start(0))
                    .map_err(FirecrackerSandboxError::CreateTransport)?;
                let staging = tempfile::Builder::new()
                    .prefix(".bsmr-firecracker-")
                    .tempdir_in(project_root)
                    .map_err(FirecrackerSandboxError::CreateTransport)?;
                let result = extract_guest_outputs(&mut output, staging.path(), &action.outputs)?;
                import_outputs(staging.path(), project_root, &action.outputs)?;
                let status = if result.timed_out {
                    GatherOutputStatus::TimedOut(action_timeout(&action)?)
                } else {
                    GatherOutputStatus::Finished {
                        exit_code: result.exit_code,
                        execution_stats: None,
                    }
                };
                return Ok(CommandResult {
                    status,
                    stdout: result.stdout,
                    stderr: result.stderr,
                    cgroup_result: None,
                    orphan_processes: Vec::new(),
                });
            }
            LauncherStatus::TimedOut => GatherOutputStatus::TimedOut(action_timeout(&action)?),
            LauncherStatus::Cancelled => GatherOutputStatus::Cancelled,
            LauncherStatus::Failed => {
                let error = response
                    .error
                    .ok_or(FirecrackerSandboxError::LauncherStatusShape)?;
                return Err(FirecrackerSandboxError::LauncherFailure(error).into());
            }
        };
        Ok(CommandResult {
            status,
            stdout: Vec::new(),
            stderr: Vec::new(),
            cgroup_result: None,
            orphan_processes: Vec::new(),
        })
    }
}

/// Recovers the required duration for a result that claims timeout.
fn action_timeout(action: &GuestAction) -> bsmr_error::Result<Duration> {
    action
        .timeout_ms
        .map(Duration::from_millis)
        .ok_or_else(|| FirecrackerSandboxError::MissingTimeout.into())
}

/// Proves that an action uses only semantics implemented by protocol v1.
pub(crate) fn validate_action_policy(
    prepared: &PreparedAction,
    request: &CommandExecutionRequest,
) -> bsmr_error::Result<FirecrackerActionPolicy> {
    let network = match prepared.network_access {
        None | Some(NetworkAccess::None) if !request.disable_local_network_isolation() => {
            NetworkPolicy::None
        }
        _ => NetworkPolicy::All,
    };
    let policy = FirecrackerActionPolicy::new(
        if request.local_environment_inheritance().is_some() {
            EnvironmentPolicy::Inherited
        } else {
            EnvironmentPolicy::Explicit
        },
        if request.worker().is_some() || request.remote_worker().is_some() {
            WorkerPolicy::Persistent
        } else {
            WorkerPolicy::None
        },
        network,
    )?;
    if !request.required_local_resources().is_empty() {
        return Err(FirecrackerSandboxError::LocalResources.into());
    }
    if !request.outputs_cleanup() {
        return Err(FirecrackerSandboxError::IncrementalOutputs.into());
    }
    if request.skip_resource_control() {
        return Err(FirecrackerSandboxError::DetachedProcess.into());
    }
    Ok(policy)
}

/// Recovers the canonical RE command already bound into the prepared action.
fn decode_re_command(
    prepared: &PreparedAction,
    digest_config: DigestConfig,
) -> bsmr_error::Result<RE::Command> {
    let action_digest: FileDigest = prepared.digest().coerce();
    let action_blob = prepared
        .action_and_blobs
        .blobs
        .iter()
        .find_map(|(digest, blob)| (digest.data() == &action_digest).then_some(&blob.0))
        .ok_or(FirecrackerSandboxError::MissingActionBlob("action"))?;
    let action = RE::Action::decode(action_blob.as_slice()).map_err(|error| {
        FirecrackerSandboxError::DecodeActionBlob {
            kind: "action",
            error,
        }
    })?;
    let command_digest = action
        .command_digest
        .as_ref()
        .ok_or(FirecrackerSandboxError::MissingActionBlob("command digest"))?;
    let command_digest = FileDigest::from_grpc(command_digest, digest_config)?;
    let command_blob = prepared
        .action_and_blobs
        .blobs
        .iter()
        .find_map(|(digest, blob)| (digest.data() == &command_digest).then_some(&blob.0))
        .ok_or(FirecrackerSandboxError::MissingActionBlob("command"))?;
    RE::Command::decode(command_blob.as_slice()).map_err(|error| {
        FirecrackerSandboxError::DecodeActionBlob {
            kind: "command",
            error,
        }
        .into()
    })
}

/// Converts one canonical command into the bounded guest protocol.
fn guest_action(
    command: &RE::Command,
    request: &CommandExecutionRequest,
) -> bsmr_error::Result<GuestAction> {
    let executable = command
        .arguments
        .first()
        .ok_or(FirecrackerSandboxError::MissingExecutable)?;
    let executable_path = Path::new(executable);
    if executable_path.is_absolute()
        || executable.contains('\\')
        || executable_path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(FirecrackerSandboxError::AbsoluteExecutable(executable.clone()).into());
    }
    let working_directory = PathBuf::from(&command.working_directory);
    if !working_directory.as_os_str().is_empty() {
        validate_guest_path(&working_directory)?;
    }
    let mut outputs = request
        .paths()
        .output_paths()
        .iter()
        .map(|(path, kind)| {
            let path = PathBuf::from(path.as_str());
            validate_guest_path(&path)?;
            let kind = match kind {
                OutputType::File => GuestOutputKind::File,
                OutputType::Directory => GuestOutputKind::Directory,
                OutputType::FileOrDirectory => GuestOutputKind::FileOrDirectory,
            };
            bsmr_error::Ok(GuestOutput { path, kind })
        })
        .collect::<bsmr_error::Result<Vec<_>>>()?;
    validate_output_declarations(&mut outputs)?;
    let timeout_ms = request
        .timeout()
        .map(|timeout| u64::try_from(timeout.as_millis()))
        .transpose()
        .map_err(|_| FirecrackerSandboxError::TimeoutOverflow)?;
    if timeout_ms.is_some_and(|timeout| timeout > MAX_TIMEOUT_MS) {
        return Err(FirecrackerSandboxError::TimeoutLimit.into());
    }
    Ok(GuestAction {
        protocol: PROTOCOL_VERSION,
        arguments: command.arguments.clone(),
        environment: command
            .environment_variables
            .iter()
            .map(|variable| (variable.name.clone(), variable.value.clone()))
            .collect(),
        working_directory,
        outputs,
        timeout_ms,
    })
}

/// Sorts output roots and rejects ambiguous overlapping declarations.
fn validate_output_declarations(outputs: &mut [GuestOutput]) -> bsmr_error::Result<()> {
    outputs.sort_by(|left, right| left.path.cmp(&right.path));
    for adjacent in outputs.windows(2) {
        if adjacent[1].path.starts_with(&adjacent[0].path) {
            return Err(FirecrackerSandboxError::OverlappingOutputs {
                parent: adjacent[0].path.clone(),
                child: adjacent[1].path.clone(),
            }
            .into());
        }
    }
    Ok(())
}

/// Serializes one action into its fixed-size read-only guest device.
fn write_guest_action(action: &GuestAction) -> bsmr_error::Result<File> {
    const REQUEST_DEVICE_BYTES: u64 = bsmr_sandbox::MAX_ACTION_BYTES;
    let bytes = serde_json::to_vec(action).map_err(FirecrackerSandboxError::SerializeProtocol)?;
    let size = u32::try_from(bytes.len()).map_err(|_| FirecrackerSandboxError::ActionTooLarge)?;
    if u64::from(size) + 4 > REQUEST_DEVICE_BYTES {
        return Err(FirecrackerSandboxError::ActionTooLarge.into());
    }
    let mut file = tempfile::tempfile().map_err(FirecrackerSandboxError::CreateTransport)?;
    file.write_all(&size.to_be_bytes())
        .and_then(|()| file.write_all(&bytes))
        .and_then(|()| file.set_len(REQUEST_DEVICE_BYTES))
        .and_then(|()| file.seek(SeekFrom::Start(0)).map(|_| ()))
        .map_err(FirecrackerSandboxError::CreateTransport)?;
    Ok(file)
}

/// Moves completely validated output roots from private staging into the project.
fn import_outputs(
    staging: &Path,
    project_root: &Path,
    outputs: &[GuestOutput],
) -> bsmr_error::Result<()> {
    let mut ready = Vec::new();
    for output in outputs {
        let source = staging.join(&output.path);
        match fs::symlink_metadata(&source) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(FirecrackerSandboxError::ImportOutput(source, error).into());
            }
        }
        let destination = project_root.join(&output.path);
        match fs::symlink_metadata(&destination) {
            Ok(_) => {
                return Err(FirecrackerSandboxError::OutputDestinationExists(destination).into());
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(FirecrackerSandboxError::ImportOutput(destination, error).into());
            }
        }
        ready.push((source, destination));
    }
    for (source, destination) in ready {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| FirecrackerSandboxError::ImportOutput(parent.to_owned(), error))?;
        }
        fs::rename(&source, &destination)
            .map_err(|error| FirecrackerSandboxError::ImportOutput(destination.clone(), error))?;
    }
    Ok(())
}

#[cfg(unix)]
/// Transfers one action to the launcher and converts disconnect into cancellation.
async fn launch(
    socket: &Path,
    request: &LauncherRequest,
    action: &File,
    input: &File,
    output: &File,
    liveliness: &dyn LivelinessObserver,
) -> bsmr_error::Result<LauncherResponse> {
    use std::os::fd::AsRawFd;
    use std::os::unix::net::UnixStream;

    use nix::sys::socket::ControlMessage;
    use nix::sys::socket::MsgFlags;
    use nix::sys::socket::sendmsg;
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;

    const MAX_LAUNCHER_MESSAGE_BYTES: usize = 64 * 1024;
    let payload =
        serde_json::to_vec(request).map_err(FirecrackerSandboxError::SerializeProtocol)?;
    if payload.len() > MAX_LAUNCHER_MESSAGE_BYTES {
        return Err(FirecrackerSandboxError::LauncherRequestTooLarge.into());
    }
    let mut frame = Vec::with_capacity(payload.len() + 4);
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(&payload);

    let mut stream = UnixStream::connect(socket).map_err(FirecrackerSandboxError::LauncherIo)?;
    let fds = [action.as_raw_fd(), input.as_raw_fd(), output.as_raw_fd()];
    let written = sendmsg::<()>(
        stream.as_raw_fd(),
        &[IoSlice::new(&frame)],
        &[ControlMessage::ScmRights(&fds)],
        MsgFlags::empty(),
        None,
    )
    .map_err(FirecrackerSandboxError::LauncherSocketIo)?;
    stream
        .write_all(&frame[written..])
        .map_err(FirecrackerSandboxError::LauncherIo)?;
    let cancel_stream = stream
        .try_clone()
        .map_err(FirecrackerSandboxError::LauncherIo)?;
    stream
        .set_nonblocking(true)
        .map_err(FirecrackerSandboxError::LauncherIo)?;
    cancel_stream
        .set_nonblocking(true)
        .map_err(FirecrackerSandboxError::LauncherIo)?;
    let mut stream =
        tokio::net::UnixStream::from_std(stream).map_err(FirecrackerSandboxError::LauncherIo)?;
    let mut cancel_stream = tokio::net::UnixStream::from_std(cancel_stream)
        .map_err(FirecrackerSandboxError::LauncherIo)?;

    let response = async {
        let size = stream
            .read_u32()
            .await
            .map_err(FirecrackerSandboxError::LauncherIo)? as usize;
        if size > MAX_LAUNCHER_MESSAGE_BYTES {
            return Err(FirecrackerSandboxError::LauncherResponseTooLarge.into());
        }
        let mut payload = vec![0; size];
        stream
            .read_exact(&mut payload)
            .await
            .map_err(FirecrackerSandboxError::LauncherIo)?;
        serde_json::from_slice(&payload)
            .map_err(FirecrackerSandboxError::SerializeProtocol)
            .map_err(Into::into)
    };
    tokio::pin!(response);
    tokio::select! {
        response = &mut response => response,
        () = liveliness.while_alive() => {
            cancel_stream
                .shutdown()
                .await
                .map_err(FirecrackerSandboxError::LauncherIo)?;
            response.await
        }
    }
}

#[cfg(not(unix))]
/// Fails launcher use on hosts without Unix descriptor passing.
async fn launch(
    _socket: &Path,
    _request: &LauncherRequest,
    _action: &File,
    _input: &File,
    _output: &File,
    _liveliness: &dyn LivelinessObserver,
) -> bsmr_error::Result<LauncherResponse> {
    Err(FirecrackerSandboxError::UnsupportedOs(std::env::consts::OS.to_owned()).into())
}

/// Validated process result and streams extracted from the guest archive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestExecutionResult {
    pub exit_code: i32,
    pub timed_out: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ValidatedOutputType {
    File(u32),
    Directory,
    Symlink(PathBuf),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ValidatedOutput {
    path: PathBuf,
    kind: ValidatedOutputType,
}

const OUTPUT_BYTES_LIMIT: u64 = MAX_OUTPUT_ARCHIVE_BYTES;
const STREAM_BYTES_LIMIT: u64 = bsmr_sandbox::MAX_STREAM_BYTES;
const ARCHIVE_NODE_LIMIT: usize = 100_000;
const ARCHIVE_PATH_DEPTH_LIMIT: usize = 128;
const RESULT_PATH: &str = ".bsmr/result.json";
const STDOUT_PATH: &str = ".bsmr/stdout";
const STDERR_PATH: &str = ".bsmr/stderr";
const OUTPUT_PREFIX: &str = "outputs";

/// Validates the security-relevant portion of a launcher response.
pub fn validate_launcher_response(response: &LauncherResponse) -> bsmr_error::Result<()> {
    if response.protocol != PROTOCOL_VERSION {
        return Err(FirecrackerSandboxError::LauncherProtocol(response.protocol).into());
    }
    if !response.cleanup_complete {
        return Err(FirecrackerSandboxError::IncompleteCleanup.into());
    }
    let error_shape_is_valid = match response.status {
        LauncherStatus::Failed => response.error.is_some(),
        LauncherStatus::Completed | LauncherStatus::TimedOut | LauncherStatus::Cancelled => {
            response.error.is_none()
        }
    };
    if !error_shape_is_valid {
        return Err(FirecrackerSandboxError::LauncherStatusShape.into());
    }
    Ok(())
}

/// Accepts only non-empty normalized project-relative guest paths.
fn validate_guest_path(path: &Path) -> bsmr_error::Result<()> {
    let valid = !path.as_os_str().is_empty()
        && !path.as_os_str().to_string_lossy().contains('\\')
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
    if !valid {
        return Err(FirecrackerSandboxError::GuestPath(path.to_owned()).into());
    }
    Ok(())
}

/// Resolves one archive path to its exact enclosing output declaration.
fn admit_output<'a>(
    path: &Path,
    declared: &'a [GuestOutput],
) -> bsmr_error::Result<&'a GuestOutput> {
    validate_guest_path(path)?;
    if let Some(output) = declared.iter().find(|output| path == output.path) {
        return Ok(output);
    }
    for output in declared {
        if path.starts_with(&output.path) {
            return match output.kind {
                GuestOutputKind::Directory | GuestOutputKind::FileOrDirectory => Ok(output),
                GuestOutputKind::File => Err(FirecrackerSandboxError::OutputBelowFile {
                    path: path.to_owned(),
                    root: output.path.clone(),
                }
                .into()),
            };
        }
    }
    Err(FirecrackerSandboxError::UndeclaredOutput(path.to_owned()).into())
}

struct DigestingReader<R> {
    inner: R,
    digester: Digester<FileDigestKind>,
}

impl<R> DigestingReader<R> {
    /// Wraps one input reader with the analyzed digest algorithm.
    fn new(inner: R, algorithm: bsmr_common::cas_digest::DigestAlgorithm) -> Self {
        Self {
            inner,
            digester: FileDigest::digester_for_algorithm(algorithm),
        }
    }

    /// Finalizes the digest of the exact bytes read by the tar encoder.
    fn finish(self) -> FileDigest {
        self.digester.finalize()
    }
}

impl<R: Read> Read for DigestingReader<R> {
    /// Hashes exactly the bytes returned to the archive encoder.
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let read = self.inner.read(buffer)?;
        self.digester.update(&buffer[..read]);
        Ok(read)
    }
}

/// Rejects a guest-produced symlink that escapes its declared output root.
fn validate_output_symlink(path: &Path, target: &Path, root: &Path) -> bsmr_error::Result<()> {
    validate_guest_path(path)?;
    if !relative_symlink_stays_within(path, target, root) {
        return Err(FirecrackerSandboxError::OutputSymlinkEscape {
            path: path.to_owned(),
            target: target.to_owned(),
            root: root.to_owned(),
        }
        .into());
    }

    Ok(())
}

/// Resolves a relative symlink lexically and proves it remains below `root`.
fn relative_symlink_stays_within(path: &Path, target: &Path, root: &Path) -> bool {
    if target.is_absolute() || target.as_os_str().to_string_lossy().contains('\\') {
        return false;
    }
    let mut resolved = path.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
    for component in target.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(name) => resolved.push(name),
            Component::ParentDir if resolved.pop() => {}
            _ => return false,
        }
    }
    path.starts_with(root) && resolved.starts_with(root)
}

/// Encodes exactly the analyzed action input tree into a deterministic guest archive.
pub fn write_input_archive<W: Write>(
    writer: W,
    project_root: &Path,
    input_directory: &ActionImmutableDirectory,
    digest_config: DigestConfig,
) -> bsmr_error::Result<()> {
    write_input_archive_with_limits(
        writer,
        project_root,
        input_directory,
        digest_config,
        MAX_INPUT_BYTES,
        ARCHIVE_NODE_LIMIT,
        ARCHIVE_PATH_DEPTH_LIMIT,
    )
}

struct BoundedWriter<W> {
    inner: W,
    remaining: u64,
}

impl<W> BoundedWriter<W> {
    /// Wraps an archive sink with a hard byte ceiling.
    fn new(inner: W, limit: u64) -> Self {
        Self {
            inner,
            remaining: limit,
        }
    }
}

impl<W: Write> Write for BoundedWriter<W> {
    /// Rejects an entire write before it would cross the byte ceiling.
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        if buffer.len() as u64 > self.remaining {
            return Err(std::io::Error::new(
                std::io::ErrorKind::FileTooLarge,
                "Firecracker action input archive exceeds its byte limit",
            ));
        }
        let written = self.inner.write(buffer)?;
        self.remaining -= written as u64;
        Ok(written)
    }

    /// Flushes the bounded writer's underlying sink.
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// Builds an input archive under explicit limits used by production and tests.
fn write_input_archive_with_limits<W: Write>(
    writer: W,
    project_root: &Path,
    input_directory: &ActionImmutableDirectory,
    digest_config: DigestConfig,
    max_bytes: u64,
    max_nodes: usize,
    max_depth: usize,
) -> bsmr_error::Result<()> {
    let mut archive = tar::Builder::new(BoundedWriter::new(writer, max_bytes));
    archive.follow_symlinks(false);

    for (nodes, (path, entry)) in input_directory.ordered_walk().with_paths().enumerate() {
        let path = PathBuf::from(path.as_str());
        validate_guest_path(&path)?;
        if nodes == max_nodes {
            return Err(FirecrackerSandboxError::InputNodeLimit(max_nodes).into());
        }
        if path.components().count() > max_depth {
            return Err(FirecrackerSandboxError::InputDepthLimit(max_depth).into());
        }
        match entry {
            DirectoryEntry::Dir(_) => append_input_directory(&mut archive, &path)?,
            DirectoryEntry::Leaf(ActionDirectoryMember::File(metadata)) => {
                append_input_file(&mut archive, project_root, &path, metadata, digest_config)?;
            }
            DirectoryEntry::Leaf(ActionDirectoryMember::Symlink(symlink)) => {
                append_input_symlink(&mut archive, &path, Path::new(symlink.target().as_str()))?;
            }
            DirectoryEntry::Leaf(ActionDirectoryMember::ExternalSymlink(symlink)) => {
                return Err(FirecrackerSandboxError::ExternalInputSymlink {
                    path,
                    target: symlink.to_path_buf(),
                }
                .into());
            }
        }
    }
    archive
        .finish()
        .map_err(FirecrackerSandboxError::WriteInputArchive)?;
    Ok(())
}

/// Appends one normalized directory with stable metadata.
fn append_input_directory<W: Write>(
    archive: &mut tar::Builder<W>,
    path: &Path,
) -> bsmr_error::Result<()> {
    let mut header = deterministic_tar_header(tar::EntryType::Directory, 0, 0o755);
    archive
        .append_data(&mut header, path, std::io::empty())
        .map_err(FirecrackerSandboxError::WriteInputArchive)?;
    Ok(())
}

/// Streams and re-hashes one declared file into the deterministic archive.
fn append_input_file<W: Write>(
    archive: &mut tar::Builder<W>,
    project_root: &Path,
    path: &Path,
    metadata: &bsmr_common::file_ops::metadata::FileMetadata,
    digest_config: DigestConfig,
) -> bsmr_error::Result<()> {
    let source = project_root.join(path);
    let inspected = fs::symlink_metadata(&source)
        .map_err(|error| FirecrackerSandboxError::ReadInput(source.clone(), error))?;
    if !inspected.file_type().is_file() {
        return Err(FirecrackerSandboxError::InputMutation {
            path: source,
            expected: metadata.digest.to_string(),
            actual: "non-file".to_owned(),
        }
        .into());
    }

    let file = File::open(&source)
        .map_err(|error| FirecrackerSandboxError::ReadInput(source.clone(), error))?;
    let algorithm = match metadata.digest.raw_digest().algorithm() {
        DigestAlgorithmFamily::Sha1 => digest_config.cas_digest_config().digest160(),
        DigestAlgorithmFamily::Sha256
        | DigestAlgorithmFamily::Blake3
        | DigestAlgorithmFamily::Blake3Keyed => digest_config.cas_digest_config().digest256(),
    }
    .expect("an input digest algorithm must be enabled in the action digest configuration");
    let mut verified = DigestingReader::new(file, algorithm);
    let mode = if metadata.is_executable { 0o755 } else { 0o644 };
    let mut header =
        deterministic_tar_header(tar::EntryType::Regular, metadata.digest.size(), mode);
    archive
        .append_data(&mut header, path, &mut verified)
        .map_err(FirecrackerSandboxError::WriteInputArchive)?;
    let mut remainder = [0u8; 1];
    verified
        .read(&mut remainder)
        .map_err(|error| FirecrackerSandboxError::ReadInput(source.clone(), error))?;
    let actual = verified.finish();
    if &actual != metadata.digest.data() {
        return Err(FirecrackerSandboxError::InputMutation {
            path: source,
            expected: metadata.digest.to_string(),
            actual: actual.to_string(),
        }
        .into());
    }
    Ok(())
}

/// Appends one relative input symlink after proving it cannot escape the action root.
fn append_input_symlink<W: Write>(
    archive: &mut tar::Builder<W>,
    path: &Path,
    target: &Path,
) -> bsmr_error::Result<()> {
    if !relative_symlink_stays_within(path, target, Path::new("")) {
        return Err(FirecrackerSandboxError::InputSymlinkEscape {
            path: path.to_owned(),
            target: target.to_owned(),
        }
        .into());
    }
    let mut header = deterministic_tar_header(tar::EntryType::Symlink, 0, 0o777);
    header
        .set_link_name(target)
        .map_err(FirecrackerSandboxError::WriteInputArchive)?;
    header.set_cksum();
    archive
        .append_data(&mut header, path, std::io::empty())
        .map_err(FirecrackerSandboxError::WriteInputArchive)?;
    Ok(())
}

/// Creates a tar header with normalized ownership and timestamps.
fn deterministic_tar_header(entry_type: tar::EntryType, size: u64, mode: u32) -> tar::Header {
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(entry_type);
    header.set_size(size);
    header.set_mode(mode);
    header.set_mtime(0);
    header.set_uid(0);
    header.set_gid(0);
    header.set_cksum();
    header
}

/// Validates an untrusted guest archive completely before writing any output node.
pub fn extract_guest_outputs<R: Read + Seek>(
    mut reader: R,
    staging: &Path,
    declared: &[GuestOutput],
) -> bsmr_error::Result<GuestExecutionResult> {
    if fs::read_dir(staging)
        .map_err(|error| FirecrackerSandboxError::WriteOutput(staging.to_owned(), error))?
        .next()
        .is_some()
    {
        return Err(FirecrackerSandboxError::StagingNotEmpty(staging.to_owned()).into());
    }

    let mut result = None;
    let mut stdout = None;
    let mut stderr = None;
    let mut outputs = Vec::new();
    let mut seen = BTreeSet::new();
    let mut output_bytes = 0_u64;
    let mut archive = tar::Archive::new(&mut reader);

    let entries = archive
        .entries()
        .map_err(FirecrackerSandboxError::ReadOutputArchive)?;
    for entry in entries {
        let mut entry = entry.map_err(FirecrackerSandboxError::ReadOutputArchive)?;
        let path = entry
            .path()
            .map_err(FirecrackerSandboxError::OutputArchivePath)?
            .into_owned();
        if !seen.insert(path.clone()) {
            return Err(FirecrackerSandboxError::DuplicateOutput(path).into());
        }

        match path.to_str() {
            Some(RESULT_PATH) => {
                let bytes = read_bounded(&mut entry, STREAM_BYTES_LIMIT, "result")?;
                let envelope = serde_json::from_slice::<GuestResultEnvelope>(&bytes)
                    .map_err(FirecrackerSandboxError::ParseGuestResult)?;
                if envelope.protocol != PROTOCOL_VERSION {
                    return Err(FirecrackerSandboxError::GuestProtocol(envelope.protocol).into());
                }
                result = Some(envelope);
            }
            Some(STDOUT_PATH) => {
                stdout = Some(read_bounded(&mut entry, STREAM_BYTES_LIMIT, "stdout")?);
            }
            Some(STDERR_PATH) => {
                stderr = Some(read_bounded(&mut entry, STREAM_BYTES_LIMIT, "stderr")?);
            }
            _ => {
                let output_path = path
                    .strip_prefix(OUTPUT_PREFIX)
                    .map_err(|_| FirecrackerSandboxError::UndeclaredOutput(path.clone()))?;
                validate_guest_path(output_path)?;
                let declaration = admit_output(output_path, declared)?;
                if output_path.components().count() > ARCHIVE_PATH_DEPTH_LIMIT {
                    return Err(FirecrackerSandboxError::OutputDepthLimit(
                        ARCHIVE_PATH_DEPTH_LIMIT,
                    )
                    .into());
                }
                if outputs.len() == ARCHIVE_NODE_LIMIT {
                    return Err(FirecrackerSandboxError::OutputNodeLimit(ARCHIVE_NODE_LIMIT).into());
                }

                let entry_type = entry.header().entry_type();
                let kind = if entry_type.is_file() {
                    let mode = entry
                        .header()
                        .mode()
                        .map_err(FirecrackerSandboxError::ReadOutputArchive)?;
                    validate_declared_node_type(
                        output_path,
                        declaration,
                        ValidatedOutputType::File(mode),
                    )?
                } else if entry_type.is_dir() {
                    validate_declared_node_type(
                        output_path,
                        declaration,
                        ValidatedOutputType::Directory,
                    )?
                } else if entry_type.is_symlink() {
                    let target = entry
                        .link_name()
                        .map_err(FirecrackerSandboxError::ReadOutputArchive)?
                        .ok_or_else(|| {
                            FirecrackerSandboxError::OutputEntryType(output_path.to_owned())
                        })?
                        .into_owned();
                    validate_output_symlink(output_path, &target, &declaration.path)?;
                    ValidatedOutputType::Symlink(target)
                } else {
                    return Err(
                        FirecrackerSandboxError::OutputEntryType(output_path.to_owned()).into(),
                    );
                };

                output_bytes = output_bytes.checked_add(entry.size()).ok_or(
                    FirecrackerSandboxError::OutputLimit {
                        kind: "output",
                        limit: OUTPUT_BYTES_LIMIT,
                    },
                )?;
                if output_bytes > OUTPUT_BYTES_LIMIT {
                    return Err(FirecrackerSandboxError::OutputLimit {
                        kind: "output",
                        limit: OUTPUT_BYTES_LIMIT,
                    }
                    .into());
                }
                outputs.push(ValidatedOutput {
                    path: output_path.to_owned(),
                    kind,
                });
            }
        }
    }

    let result = result.ok_or(FirecrackerSandboxError::MissingEnvelope(RESULT_PATH))?;
    let stdout = stdout.ok_or(FirecrackerSandboxError::MissingEnvelope(STDOUT_PATH))?;
    let stderr = stderr.ok_or(FirecrackerSandboxError::MissingEnvelope(STDERR_PATH))?;

    reader
        .seek(SeekFrom::Start(0))
        .map_err(FirecrackerSandboxError::ReadOutputArchive)?;
    materialize_validated_outputs(&mut reader, staging, &outputs)?;

    Ok(GuestExecutionResult {
        exit_code: result.exit_code,
        timed_out: result.timed_out,
        stdout,
        stderr,
    })
}

/// Reads one result stream while enforcing its protocol ceiling.
fn read_bounded(
    reader: &mut impl Read,
    limit: u64,
    kind: &'static str,
) -> bsmr_error::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(FirecrackerSandboxError::ReadOutputArchive)?;
    if bytes.len() as u64 > limit {
        return Err(FirecrackerSandboxError::OutputLimit { kind, limit }.into());
    }
    Ok(bytes)
}

/// Checks the exact output root type while permitting descendants of directories.
fn validate_declared_node_type(
    path: &Path,
    declaration: &GuestOutput,
    actual: ValidatedOutputType,
) -> bsmr_error::Result<ValidatedOutputType> {
    if path != declaration.path {
        return Ok(actual);
    }
    let mismatch = matches!(
        (&declaration.kind, &actual),
        (GuestOutputKind::File, ValidatedOutputType::Directory)
            | (GuestOutputKind::Directory, ValidatedOutputType::File(_))
    );
    if mismatch {
        let actual_name = match actual {
            ValidatedOutputType::File(_) => "file",
            ValidatedOutputType::Directory => "directory",
            ValidatedOutputType::Symlink(_) => "symlink",
        };
        let expected = match declaration.kind {
            GuestOutputKind::File => "file",
            GuestOutputKind::Directory => "directory",
            GuestOutputKind::FileOrDirectory => "file or directory",
        };
        return Err(FirecrackerSandboxError::OutputType {
            path: path.to_owned(),
            actual: actual_name,
            expected,
        }
        .into());
    }
    Ok(actual)
}

/// Replays a previously validated archive into a private empty directory.
fn materialize_validated_outputs(
    reader: &mut (impl Read + Seek),
    staging: &Path,
    outputs: &[ValidatedOutput],
) -> bsmr_error::Result<()> {
    let mut archive = tar::Archive::new(reader);
    let mut output_index = BTreeMap::new();
    for output in outputs {
        output_index.insert(output.path.clone(), output);
    }

    let entries = archive
        .entries()
        .map_err(FirecrackerSandboxError::ReadOutputArchive)?;
    for entry in entries {
        let mut entry = entry.map_err(FirecrackerSandboxError::ReadOutputArchive)?;
        let archive_path = entry
            .path()
            .map_err(FirecrackerSandboxError::OutputArchivePath)?
            .into_owned();
        let Ok(path) = archive_path.strip_prefix(OUTPUT_PREFIX) else {
            continue;
        };
        let output = output_index
            .get(path)
            .expect("validated output archive changed between passes");
        let destination = staging.join(path);
        match output.kind {
            ValidatedOutputType::Directory => {
                fs::create_dir_all(&destination).map_err(|error| {
                    FirecrackerSandboxError::WriteOutput(destination.clone(), error)
                })?
            }
            ValidatedOutputType::File(mode) => {
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent).map_err(|error| {
                        FirecrackerSandboxError::WriteOutput(parent.to_owned(), error)
                    })?;
                }
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&destination)
                    .map_err(|error| {
                        FirecrackerSandboxError::WriteOutput(destination.clone(), error)
                    })?;
                std::io::copy(&mut entry, &mut file).map_err(|error| {
                    FirecrackerSandboxError::WriteOutput(destination.clone(), error)
                })?;
                set_output_executable(&destination, mode)?;
            }
            ValidatedOutputType::Symlink(_) => {}
        }
    }

    for output in outputs {
        if let ValidatedOutputType::Symlink(target) = &output.kind {
            let destination = staging.join(&output.path);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    FirecrackerSandboxError::WriteOutput(parent.to_owned(), error)
                })?;
            }
            create_output_symlink(target, &destination)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
/// Normalizes an output file to executable or non-executable Unix permissions.
fn set_output_executable(path: &Path, mode: u32) -> bsmr_error::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let permissions = fs::Permissions::from_mode(if mode & 0o111 == 0 { 0o644 } else { 0o755 });
    fs::set_permissions(path, permissions)
        .map_err(|error| FirecrackerSandboxError::WriteOutput(path.to_owned(), error).into())
}

#[cfg(not(unix))]
/// Fails permission materialization on unsupported hosts.
fn set_output_executable(_path: &Path, _mode: u32) -> bsmr_error::Result<()> {
    Err(FirecrackerSandboxError::UnsupportedOs(std::env::consts::OS.to_owned()).into())
}

#[cfg(unix)]
/// Creates one already validated relative output symlink.
fn create_output_symlink(target: &Path, destination: &Path) -> bsmr_error::Result<()> {
    std::os::unix::fs::symlink(target, destination)
        .map_err(|error| FirecrackerSandboxError::WriteOutput(destination.to_owned(), error).into())
}

#[cfg(not(unix))]
/// Fails symlink materialization on unsupported hosts.
fn create_output_symlink(_target: &Path, _destination: &Path) -> bsmr_error::Result<()> {
    Err(FirecrackerSandboxError::UnsupportedOs(std::env::consts::OS.to_owned()).into())
}

/// Returns the complete sorted action identity for Firecracker protocol v1.
#[must_use]
pub fn sandbox_platform_properties(environment_digest: &str) -> [(&'static str, &str); 4] {
    [
        ("bsmr.sandbox.backend", "firecracker"),
        ("bsmr.sandbox.environment", environment_digest),
        ("bsmr.sandbox.profile", SANDBOX_PROFILE),
        ("bsmr.sandbox.protocol", SANDBOX_PROTOCOL),
    ]
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Cursor;
    #[cfg(unix)]
    use std::io::Write;
    #[cfg(unix)]
    use std::os::fd::AsRawFd;
    #[cfg(unix)]
    use std::os::unix::net::UnixListener;
    use std::path::Path;

    #[cfg(unix)]
    use bsmr_common::liveliness_observer::NoopLivelinessObserver;
    use bsmr_sandbox::BundleArtifact;
    use bsmr_sandbox::BundleManifest;
    #[cfg(unix)]
    use nix::cmsg_space;
    #[cfg(unix)]
    use nix::sys::socket::ControlMessageOwned;
    #[cfg(unix)]
    use nix::sys::socket::MsgFlags;
    #[cfg(unix)]
    use nix::sys::socket::recvmsg;
    use sha2::Digest;
    use sha2::Sha256;

    use super::*;

    /// A bundle identity binds every executable and guest artifact, not its path.
    #[test]
    fn bundle_identity_changes_with_every_artifact() {
        let temp = tempfile::tempdir().unwrap();
        for name in ["firecracker", "jailer", "kernel", "rootfs"] {
            fs::write(temp.path().join(name), name).unwrap();
        }

        let before = load_test_bundle(temp.path());
        for name in ["firecracker", "jailer", "kernel", "rootfs"] {
            fs::write(temp.path().join(name), format!("changed {name}")).unwrap();
            let after = load_test_bundle(temp.path());
            assert_ne!(before.environment_digest(), after.environment_digest());
            fs::write(temp.path().join(name), name).unwrap();
        }
    }

    /// Constructs one valid synthetic bundle for semantic identity tests.
    fn load_test_bundle(root: &Path) -> FirecrackerBundle {
        let artifacts = ["firecracker", "jailer", "kernel", "rootfs"]
            .into_iter()
            .map(|name| {
                let bytes = fs::read(root.join(name)).unwrap();
                (
                    name.to_owned(),
                    BundleArtifact {
                        path: name.into(),
                        sha256: format!("{:x}", Sha256::digest(bytes)),
                    },
                )
            })
            .collect();
        let manifest = BundleManifest {
            schema: PROTOCOL_VERSION,
            architecture: std::env::consts::ARCH.to_owned(),
            firecracker_version: "test".to_owned(),
            jailer_version: "test".to_owned(),
            artifacts,
        };
        let path = root.join("manifest.json");
        fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        FirecrackerBundle::load(&path, std::env::consts::ARCH).unwrap()
    }

    /// The action key receives a stable, sorted description of the sandbox contract.
    #[test]
    fn platform_properties_bind_the_profile_backend_environment_and_protocol() {
        let properties = sandbox_platform_properties("sha256:environment");

        assert_eq!(
            properties,
            [
                ("bsmr.sandbox.backend", "firecracker"),
                ("bsmr.sandbox.environment", "sha256:environment"),
                ("bsmr.sandbox.profile", "untrusted-v1"),
                ("bsmr.sandbox.protocol", "1"),
            ]
        );
    }

    /// Firecracker v1 admits only networkless, non-worker, explicit-environment actions.
    #[test]
    fn action_policy_rejects_every_semantic_downgrade() {
        assert!(
            FirecrackerActionPolicy::new(
                EnvironmentPolicy::Explicit,
                WorkerPolicy::None,
                NetworkPolicy::None,
            )
            .is_ok()
        );
        for policy in [
            (
                EnvironmentPolicy::Inherited,
                WorkerPolicy::None,
                NetworkPolicy::None,
            ),
            (
                EnvironmentPolicy::Explicit,
                WorkerPolicy::Persistent,
                NetworkPolicy::None,
            ),
            (
                EnvironmentPolicy::Explicit,
                WorkerPolicy::None,
                NetworkPolicy::All,
            ),
        ] {
            assert!(FirecrackerActionPolicy::new(policy.0, policy.1, policy.2).is_err());
        }
    }

    /// Unsupported operating systems fail instead of selecting the host executor.
    #[test]
    fn host_preflight_is_fail_closed() {
        let unsupported = HostCapabilities {
            os: "macos",
            architecture: "aarch64",
            cgroup_v2: false,
        };

        let error = validate_host(&unsupported).unwrap_err().to_string();
        assert!(error.contains("Linux"));
        assert!(error.contains("macos"));

        let unsupported = HostCapabilities {
            os: "linux",
            architecture: "aarch64",
            cgroup_v2: true,
        };
        assert!(
            validate_host(&unsupported)
                .unwrap_err()
                .to_string()
                .contains("x86_64")
        );

        let unsupported = HostCapabilities {
            os: "linux",
            architecture: "x86_64",
            cgroup_v2: false,
        };
        assert!(validate_host(&unsupported).is_err());

        let directory = tempfile::tempdir().unwrap();
        assert!(validate_launcher_socket(&directory.path().join("missing.sock")).is_err());
        assert!(validate_launcher_socket(directory.path()).is_err());
    }

    /// Guest-visible paths are normalized project-relative paths, never host paths.
    #[test]
    fn guest_paths_reject_absolute_parent_and_platform_prefixes() {
        assert!(validate_guest_path(Path::new("packages/api/src.ts")).is_ok());
        assert!(validate_guest_path(Path::new("/etc/passwd")).is_err());
        assert!(validate_guest_path(Path::new("../etc/passwd")).is_err());
        assert!(validate_guest_path(Path::new("C:\\Windows")).is_err());
    }

    /// A file declaration admits exactly one file; a directory admits its descendants.
    #[test]
    fn output_admission_is_declaration_granular() {
        let declared = [
            GuestOutput::file("bsmr-out/app.js"),
            GuestOutput::directory("bsmr-out/assets"),
        ];

        assert!(admit_output(Path::new("bsmr-out/app.js"), &declared).is_ok());
        assert!(admit_output(Path::new("bsmr-out/assets/logo.svg"), &declared).is_ok());
        assert!(admit_output(Path::new("bsmr-out/app.js/map"), &declared).is_err());
        assert!(admit_output(Path::new("bsmr-out/undeclared"), &declared).is_err());
    }

    /// Overlapping roots are rejected because they cannot be imported independently.
    #[test]
    fn output_declarations_must_not_overlap() {
        let mut declared = [
            GuestOutput::file("bsmr-out/assets/logo.svg"),
            GuestOutput::directory("bsmr-out/assets"),
        ];

        assert!(validate_output_declarations(&mut declared).is_err());
    }

    /// Relative symlinks may stay inside one declared output root and nowhere else.
    #[test]
    fn output_symlinks_cannot_escape_their_declared_root() {
        let root = Path::new("bsmr-out/assets");

        assert!(
            validate_output_symlink(
                Path::new("bsmr-out/assets/current"),
                Path::new("images/logo.svg"),
                root,
            )
            .is_ok()
        );
        assert!(
            validate_output_symlink(
                Path::new("bsmr-out/assets/current"),
                Path::new("../../secret"),
                root,
            )
            .is_err()
        );
        assert!(
            validate_output_symlink(
                Path::new("bsmr-out/assets/current"),
                Path::new("/etc/passwd"),
                root,
            )
            .is_err()
        );
    }

    /// Input symlinks may move within the action root but never above it.
    #[test]
    fn input_symlinks_cannot_escape_the_action_root() {
        assert!(relative_symlink_stays_within(
            Path::new("src/link"),
            Path::new("../include/file"),
            Path::new("")
        ));
        assert!(!relative_symlink_stays_within(
            Path::new("src/link"),
            Path::new("../../etc/passwd"),
            Path::new("")
        ));
        assert!(!relative_symlink_stays_within(
            Path::new("src/link"),
            Path::new("/etc/passwd"),
            Path::new("")
        ));
    }

    /// The host accepts a launcher result only after complete resource cleanup.
    #[test]
    fn launcher_response_requires_matching_protocol_and_cleanup() {
        let completed = || LauncherResponse {
            protocol: PROTOCOL_VERSION,
            status: LauncherStatus::Completed,
            cleanup_complete: true,
            error: None,
        };
        assert!(validate_launcher_response(&completed()).is_ok());

        let mut wrong_protocol = completed();
        wrong_protocol.protocol = 2;
        assert!(validate_launcher_response(&wrong_protocol).is_err());

        let mut leaked = completed();
        leaked.cleanup_complete = false;
        assert!(validate_launcher_response(&leaked).is_err());

        let mut ambiguous = completed();
        ambiguous.error = Some("unexpected".to_owned());
        assert!(validate_launcher_response(&ambiguous).is_err());

        let mut failed = completed();
        failed.status = LauncherStatus::Failed;
        assert!(validate_launcher_response(&failed).is_err());
    }

    /// The client transfers exactly three descriptors and parses one bounded response.
    #[cfg(unix)]
    #[tokio::test]
    async fn launcher_transport_round_trips_frame_and_descriptors() {
        let directory = tempfile::tempdir().unwrap();
        let socket = directory.path().join("sandboxd.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut bytes = [0u8; 4096];
            let mut iov = [std::io::IoSliceMut::new(&mut bytes)];
            let mut control = cmsg_space!([i32; 3]);
            let message = recvmsg::<()>(
                stream.as_raw_fd(),
                &mut iov,
                Some(&mut control),
                MsgFlags::empty(),
            )
            .unwrap();
            let mut descriptors = Vec::new();
            for control in message.cmsgs().unwrap() {
                if let ControlMessageOwned::ScmRights(rights) = control {
                    descriptors.extend(rights);
                }
            }
            assert_eq!(descriptors.len(), 3);
            let size = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
            let request: LauncherRequest = serde_json::from_slice(&bytes[4..4 + size]).unwrap();
            assert_eq!(request.action_id, "01234567-89ab-cdef-0123-456789abcdef");
            for descriptor in descriptors {
                nix::unistd::close(descriptor).unwrap();
            }
            let response = LauncherResponse {
                protocol: PROTOCOL_VERSION,
                status: LauncherStatus::Completed,
                cleanup_complete: true,
                error: None,
            };
            let payload = serde_json::to_vec(&response).unwrap();
            stream
                .write_all(&(payload.len() as u32).to_be_bytes())
                .unwrap();
            stream.write_all(&payload).unwrap();
        });
        let mut action = tempfile::tempfile().unwrap();
        action.write_all(b"action").unwrap();
        let mut input = tempfile::tempfile().unwrap();
        input.write_all(b"input").unwrap();
        let output = tempfile::tempfile().unwrap();
        let request = LauncherRequest {
            protocol: PROTOCOL_VERSION,
            action_id: "01234567-89ab-cdef-0123-456789abcdef".to_owned(),
            environment_digest: "sha256:test".to_owned(),
            input_bytes: 5,
            output_bytes: 0,
            vcpu_count: 1,
            memory_mib: 128,
            timeout_ms: None,
        };

        let response = launch(
            &socket,
            &request,
            &action,
            &input,
            &output,
            &NoopLivelinessObserver,
        )
        .await
        .unwrap();

        assert_eq!(response.status, LauncherStatus::Completed);
        server.join().unwrap();
    }

    /// Output extraction accepts the protocol envelope and declared nodes only.
    #[test]
    fn output_archive_round_trips_declared_results() {
        let archive = output_archive(&[
            (".bsmr/result.json", br#"{"protocol":1,"exit_code":0}"#),
            (".bsmr/stdout", b"compiled\n"),
            (".bsmr/stderr", b""),
            ("outputs/bsmr-out/app.js", b"export {};\n"),
        ]);
        let temp = tempfile::tempdir().unwrap();

        let result = extract_guest_outputs(
            Cursor::new(archive),
            temp.path(),
            &[GuestOutput::file("bsmr-out/app.js")],
        )
        .unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout, b"compiled\n");
        assert_eq!(result.stderr, b"");
        assert_eq!(
            fs::read(temp.path().join("bsmr-out/app.js")).unwrap(),
            b"export {};\n"
        );
    }

    /// A syntactically valid archive cannot smuggle an undeclared output onto the host.
    #[test]
    fn output_archive_rejects_undeclared_nodes_before_import() {
        let archive = output_archive(&[
            (".bsmr/result.json", br#"{"protocol":1,"exit_code":0}"#),
            (".bsmr/stdout", b""),
            (".bsmr/stderr", b""),
            ("outputs/.ssh/authorized_keys", b"oops"),
        ]);
        let temp = tempfile::tempdir().unwrap();

        assert!(extract_guest_outputs(Cursor::new(archive), temp.path(), &[]).is_err());
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    }

    /// Special, hard-linked, oversized, and over-deep output nodes are never materialized.
    #[test]
    fn output_archive_rejects_every_bounded_tree_violation() {
        let deep = format!("out/{}", vec!["node"; ARCHIVE_PATH_DEPTH_LIMIT].join("/"));
        let archives = [
            special_output_archive(tar::EntryType::Fifo, "outputs/out/fifo", None, 0),
            special_output_archive(
                tar::EntryType::Link,
                "outputs/out/link",
                Some("outputs/out/file"),
                0,
            ),
            special_output_archive(
                tar::EntryType::Regular,
                "outputs/out/large",
                None,
                OUTPUT_BYTES_LIMIT + 1,
            ),
            output_archive(&[(deep.as_str(), b"deep")]),
        ];

        for archive in archives {
            let temp = tempfile::tempdir().unwrap();
            assert!(
                extract_guest_outputs(
                    Cursor::new(archive),
                    temp.path(),
                    &[GuestOutput::directory("out")],
                )
                .is_err()
            );
            assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
        }
    }

    /// Import never overwrites an output that the ordinary cleanup phase left behind.
    #[test]
    fn output_import_fails_closed_on_existing_destination() {
        let project = tempfile::tempdir().unwrap();
        let staging = tempfile::tempdir().unwrap();
        fs::create_dir_all(project.path().join("bsmr-out")).unwrap();
        fs::create_dir_all(staging.path().join("bsmr-out")).unwrap();
        fs::write(project.path().join("bsmr-out/app.js"), b"existing").unwrap();
        fs::write(staging.path().join("bsmr-out/app.js"), b"new").unwrap();

        assert!(
            import_outputs(
                staging.path(),
                project.path(),
                &[GuestOutput::file("bsmr-out/app.js")],
            )
            .is_err()
        );
        assert_eq!(
            fs::read(project.path().join("bsmr-out/app.js")).unwrap(),
            b"existing"
        );
    }

    /// All destination checks complete before the first staged output is moved.
    #[test]
    fn output_import_preflight_prevents_partial_materialization() {
        let project = tempfile::tempdir().unwrap();
        let staging = tempfile::tempdir().unwrap();
        fs::create_dir_all(project.path().join("bsmr-out")).unwrap();
        fs::create_dir_all(staging.path().join("bsmr-out")).unwrap();
        fs::write(staging.path().join("bsmr-out/a.js"), b"new").unwrap();
        fs::write(staging.path().join("bsmr-out/b.js"), b"new").unwrap();
        fs::write(project.path().join("bsmr-out/b.js"), b"existing").unwrap();

        assert!(
            import_outputs(
                staging.path(),
                project.path(),
                &[
                    GuestOutput::file("bsmr-out/a.js"),
                    GuestOutput::file("bsmr-out/b.js"),
                ],
            )
            .is_err()
        );
        assert!(!project.path().join("bsmr-out/a.js").exists());
        assert!(staging.path().join("bsmr-out/a.js").exists());
    }

    /// Symlink metadata is treated as hostile even when the guest agent emitted it.
    #[test]
    fn output_archive_rejects_escaping_symlinks() {
        let mut archive = tar::Builder::new(Vec::new());
        append_archive_file(
            &mut archive,
            ".bsmr/result.json",
            br#"{"protocol":1,"exit_code":0}"#,
        );
        append_archive_file(&mut archive, ".bsmr/stdout", b"");
        append_archive_file(&mut archive, ".bsmr/stderr", b"");
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        header.set_mode(0o777);
        header.set_mtime(0);
        header.set_uid(0);
        header.set_gid(0);
        header.set_link_name("../../../../etc/passwd").unwrap();
        header.set_cksum();
        archive
            .append_data(&mut header, "outputs/bsmr-out/assets/current", &[][..])
            .unwrap();
        archive.finish().unwrap();
        let temp = tempfile::tempdir().unwrap();

        assert!(
            extract_guest_outputs(
                Cursor::new(archive.into_inner().unwrap()),
                temp.path(),
                &[GuestOutput::directory("bsmr-out/assets")],
            )
            .is_err()
        );
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    }

    /// Input transport is deterministic and refuses bytes that no longer match the action key.
    #[test]
    fn input_archive_is_deterministic_and_detects_mutation() {
        use bsmr_common::file_ops::metadata::FileMetadata;
        use bsmr_common::file_ops::metadata::TrackedFileDigest;
        use bsmr_core::fs::project_rel_path::ProjectRelativePath;
        use bsmr_execute::directory::ActionDirectoryBuilder;
        use bsmr_execute::directory::insert_file;

        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("src")).unwrap();
        fs::write(temp.path().join("src/main.ts"), b"export {};\n").unwrap();
        let digest_config = DigestConfig::testing_default();
        let mut builder = ActionDirectoryBuilder::empty();
        insert_file(
            &mut builder,
            ProjectRelativePath::new("src/main.ts").unwrap().to_buf(),
            FileMetadata {
                digest: TrackedFileDigest::from_content(
                    b"export {};\n",
                    digest_config.cas_digest_config(),
                ),
                is_executable: false,
            },
        )
        .unwrap();
        let directory = builder.fingerprint(digest_config.as_directory_serializer());

        let mut first = Vec::new();
        write_input_archive(&mut first, temp.path(), &directory, digest_config).unwrap();
        let mut second = Vec::new();
        write_input_archive(&mut second, temp.path(), &directory, digest_config).unwrap();
        assert_eq!(first, second);
        assert!(
            write_input_archive_with_limits(
                Vec::new(),
                temp.path(),
                &directory,
                digest_config,
                511,
                ARCHIVE_NODE_LIMIT,
                ARCHIVE_PATH_DEPTH_LIMIT,
            )
            .is_err()
        );
        assert!(
            write_input_archive_with_limits(
                Vec::new(),
                temp.path(),
                &directory,
                digest_config,
                MAX_INPUT_BYTES,
                0,
                ARCHIVE_PATH_DEPTH_LIMIT,
            )
            .is_err()
        );
        assert!(
            write_input_archive_with_limits(
                Vec::new(),
                temp.path(),
                &directory,
                digest_config,
                MAX_INPUT_BYTES,
                ARCHIVE_NODE_LIMIT,
                1,
            )
            .is_err()
        );

        fs::write(temp.path().join("src/main.ts"), b"mutated\n").unwrap();
        assert!(write_input_archive(Vec::new(), temp.path(), &directory, digest_config).is_err());
    }

    /// Hashing includes bytes read past the declared size so growth is never truncated silently.
    #[test]
    fn digesting_reader_detects_a_declared_size_mismatch() {
        let digest_config = DigestConfig::testing_default();
        let algorithm = digest_config.cas_digest_config().preferred_algorithm();
        let expected = FileDigest::from_content_for_algorithm(b"declared", algorithm);
        let mut reader = DigestingReader::new(Cursor::new(b"declaredextra"), algorithm);
        let mut archive = tar::Builder::new(Vec::new());
        let mut header = deterministic_tar_header(tar::EntryType::Regular, 8, 0o644);

        archive
            .append_data(&mut header, "input", &mut reader)
            .unwrap();

        assert_ne!(reader.finish(), expected);
    }

    /// Creates one deterministic output archive from regular-file entries.
    fn output_archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut archive = tar::Builder::new(Vec::new());
        for (path, bytes) in entries {
            append_archive_file(&mut archive, path, bytes);
        }
        archive.finish().unwrap();
        archive.into_inner().unwrap()
    }

    /// Creates one raw archive entry whose metadata is rejected before its body is read.
    fn special_output_archive(
        entry_type: tar::EntryType,
        path: &str,
        link: Option<&str>,
        size: u64,
    ) -> Vec<u8> {
        let mut header = deterministic_tar_header(entry_type, size, 0o644);
        header.set_path(path).unwrap();
        if let Some(link) = link {
            header.set_link_name(link).unwrap();
        }
        header.set_cksum();
        let mut archive = header.as_bytes().to_vec();
        archive.extend_from_slice(&[0; 1024]);
        archive
    }

    /// Appends one regular file with stable metadata to a test archive.
    fn append_archive_file(archive: &mut tar::Builder<Vec<u8>>, path: &str, bytes: &[u8]) {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Regular);
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(0);
        header.set_uid(0);
        header.set_gid(0);
        header.set_cksum();
        archive.append_data(&mut header, path, bytes).unwrap();
    }
}
