//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Launches and owns one jailed Firecracker microVM per authenticated action.

#[cfg(target_os = "linux")]
mod linux {
    use std::collections::BTreeMap;
    use std::fs;
    use std::fs::File;
    use std::io::ErrorKind;
    use std::io::IoSliceMut;
    use std::io::Read;
    use std::io::Seek;
    use std::io::SeekFrom;
    use std::io::Write;
    use std::mem::MaybeUninit;
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::FileExt;
    use std::os::unix::fs::FileTypeExt;
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;
    use std::os::unix::net::UnixStream;
    use std::os::unix::process::CommandExt;
    use std::path::Path;
    use std::path::PathBuf;
    use std::process::Child;
    use std::process::Command;
    use std::process::ExitStatus;
    use std::process::Stdio;
    use std::sync::Arc;
    use std::sync::Condvar;
    use std::sync::LazyLock;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;
    use std::time::Duration;
    use std::time::Instant;

    use bsmr_sandbox::BundleTrust;
    use bsmr_sandbox::LauncherRequest;
    use bsmr_sandbox::LauncherResponse;
    use bsmr_sandbox::LauncherStatus;
    use bsmr_sandbox::MAX_ACTION_BYTES;
    use bsmr_sandbox::MAX_INPUT_BYTES;
    use bsmr_sandbox::MAX_OUTPUT_BYTES;
    use bsmr_sandbox::MAX_TIMEOUT_MS;
    use bsmr_sandbox::MEMORY_MIB;
    use bsmr_sandbox::PROTOCOL_VERSION;
    use bsmr_sandbox::VCPU_COUNT;
    use bsmr_sandbox::VerifiedBundle;
    use bsmr_sandbox::firecracker::ApiClient;
    use bsmr_sandbox::snapshot::GUEST_READY_BYTE;
    use bsmr_sandbox::snapshot::READY_MARKER;
    use bsmr_sandbox::snapshot::WAKE_BYTE;
    use bsmr_sandbox::snapshot::WAKE_PORT;
    use clap::Parser;
    use clap::ValueEnum;
    use nix::errno::Errno;
    use nix::fcntl::FcntlArg;
    use nix::fcntl::OFlag;
    use nix::fcntl::fcntl;
    use nix::sched::CloneFlags;
    use nix::sched::unshare;
    use nix::sys::signal::kill;
    use nix::sys::socket::MsgFlags;
    use nix::sys::socket::getsockopt;
    use nix::sys::socket::recv;
    use nix::sys::socket::sockopt::PeerCredentials;
    use nix::sys::wait::WaitStatus;
    use nix::sys::wait::waitpid;
    use nix::unistd::Gid;
    use nix::unistd::Pid;
    use nix::unistd::Uid;
    use nix::unistd::chown;
    use rustix::fs::Mode;
    use rustix::fs::OFlags;
    use serde::Serialize;
    use signal_hook::consts::SIGINT;
    use signal_hook::consts::SIGTERM;
    use thiserror::Error;

    const MAX_MESSAGE_BYTES: usize = 64 * 1024;
    const MAX_DIAGNOSTIC_BYTES: u64 = 16 * 1024;
    const MAX_GUEST_READY_LOG_BYTES: u64 = 64 * 1024;
    const BOOT_TIMEOUT: Duration = Duration::from_secs(10);
    const CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);
    const CLIENT_IO_TIMEOUT: Duration = Duration::from_secs(5);
    const POLL_INTERVAL: Duration = Duration::from_millis(5);
    const CGROUP_PARENT: &str = "bsmr";
    static TERMINATE: LazyLock<Arc<AtomicBool>> =
        LazyLock::new(|| Arc::new(AtomicBool::new(false)));

    #[derive(Debug, Parser)]
    #[command(name = "bsmr-sandboxd")]
    struct Args {
        /// Root-owned immutable Firecracker bundle manifest.
        #[arg(
            long,
            default_value = "/usr/local/share/bsmr/firecracker/manifest.json"
        )]
        bundle: PathBuf,
        /// Unix socket used by unprivileged BSMR daemons.
        #[arg(long, default_value = "/run/bsmr/sandboxd.sock")]
        socket: PathBuf,
        /// Root-owned base directory for ephemeral jailer roots.
        #[arg(long, default_value = "/var/lib/bsmr/jailer")]
        jail_root: PathBuf,
        /// Root-owned local content-addressed store on the jail filesystem.
        #[arg(long, default_value = "/var/cache/bsmr/cas")]
        cas_root: PathBuf,
        /// First UID in the operator-reserved per-microVM identity range.
        #[arg(long)]
        uid_base: u32,
        /// First GID in the operator-reserved per-microVM identity range.
        #[arg(long)]
        gid_base: u32,
        /// Group allowed to connect to the launcher socket.
        #[arg(long)]
        socket_gid: u32,
        /// Hard upper bound on concurrent microVMs.
        #[arg(long, default_value_t = 8)]
        max_vms: usize,
        /// Explicit boot path; fresh exists only as the correctness oracle.
        #[arg(long, value_enum, default_value_t = BootMode::Snapshot)]
        boot_mode: BootMode,
    }

    #[derive(Clone, Copy, Debug, ValueEnum)]
    enum BootMode {
        Snapshot,
        Fresh,
    }

    #[derive(Debug, Error)]
    pub(super) enum LauncherError {
        #[error("bsmr-sandboxd must run as root")]
        RootRequired,
        #[error("max-vms must be greater than zero")]
        InvalidConcurrency,
        #[error("the reserved UID/GID range overflows max-vms")]
        IdentityRange,
        #[error("reserved {kind} {id} already belongs to a host account")]
        IdentityInUse { kind: &'static str, id: u32 },
        #[error("unsafe launcher path {0:?}")]
        UnsafePath(PathBuf),
        #[error("bundle, local CAS, and jail root must share a filesystem: {0:?}")]
        CasFilesystem(PathBuf),
        #[error("existing local CAS object does not match its verified source: {0:?}")]
        CasCollision(PathBuf),
        #[error("local CAS object is missing: {0}")]
        CasMissing(&'static str),
        #[error("cgroup v2 is missing required controller {0}")]
        MissingController(&'static str),
        #[error("I/O failure at {path:?}: {source}")]
        Io {
            path: PathBuf,
            #[source]
            source: std::io::Error,
        },
        #[error("invalid bundle: {0}")]
        Bundle(#[from] bsmr_sandbox::BundleError),
        #[error("launcher request is larger than {MAX_MESSAGE_BYTES} bytes")]
        MessageTooLarge,
        #[error("launcher request did not include exactly three file descriptors")]
        FileDescriptors,
        #[error("launcher transport file {index} is not a regular file")]
        FileType { index: usize },
        #[error("launcher transport file {index} is not open with required access")]
        FileAccess { index: usize },
        #[error("launcher transport file {index} has {actual} bytes, expected {expected}")]
        FileSize {
            index: usize,
            actual: u64,
            expected: u64,
        },
        #[error("launcher transport {path:?} changed while it was copied")]
        TransportMutation { path: PathBuf },
        #[error("launcher transport file {index} failed digest authentication: {source}")]
        TransportDigest {
            index: usize,
            #[source]
            source: bsmr_sandbox::BundleError,
        },
        #[error("launcher protocol must be {PROTOCOL_VERSION}, got {0}")]
        Protocol(u32),
        #[error("launcher action ID must be a lowercase UUID")]
        ActionId,
        #[error("launcher execution bundle digest does not match")]
        EnvironmentDigest,
        #[error("launcher input transport must be non-empty")]
        EmptyInput,
        #[error("launcher transport digests must be lowercase SHA-256")]
        TransportDigestShape,
        #[error("launcher output capacity must be {MAX_OUTPUT_BYTES} bytes")]
        OutputCapacity,
        #[error("launcher input exceeds {MAX_INPUT_BYTES} bytes")]
        InputTooLarge,
        #[error("launcher output exceeds {MAX_OUTPUT_BYTES} bytes")]
        OutputTooLarge,
        #[error("launcher timeout exceeds {MAX_TIMEOUT_MS} ms")]
        TimeoutTooLarge,
        #[error("launcher vCPU count must be {VCPU_COUNT}, got {0}")]
        VcpuCount(u8),
        #[error("launcher memory must be {MEMORY_MIB} MiB, got {0}")]
        MemorySize(u32),
        #[error("launcher request frame is truncated")]
        TruncatedFrame,
        #[error("launcher request frame length is invalid")]
        FrameLength,
        #[error("launcher client sent data after its request")]
        UnexpectedClientData,
        #[error("launcher peer disconnected")]
        Cancelled,
        #[error("failed to execute jailer: {0}")]
        Jailer(std::io::Error),
        #[error(
            "{artifact} reported {reported:?} with status {status}, expected {expected:?}; stderr: {stderr:?}"
        )]
        Version {
            artifact: &'static str,
            reported: Option<String>,
            status: ExitStatus,
            expected: String,
            stderr: String,
        },
        #[error("launcher supervisor argument is missing: {0}")]
        SupervisorArgument(&'static str),
        #[error("launcher supervisor exited with {status}: {log}")]
        SupervisorExit { status: ExitStatus, log: String },
        #[error("jailed Firecracker exited as {0:?}")]
        FirecrackerExit(WaitStatus),
        #[error("microVM exceeded its host deadline: {0}")]
        HostDeadline(String),
        #[error("Firecracker API did not become ready")]
        ApiTimeout,
        #[error("Firecracker guest did not reach its pre-action barrier")]
        GuestReadyTimeout,
        #[error("Firecracker guest exceeded its readiness log limit")]
        GuestReadyLogLimit,
        #[error("launcher supervisor is missing after startup")]
        MissingSupervisor,
        #[error("invalid Firecracker PID file")]
        Pid,
        #[error("Firecracker API failure: {0}")]
        Api(#[from] bsmr_sandbox::firecracker::ApiError),
        #[error("failed to release the pristine guest barrier: {0}")]
        WakeIo(#[source] std::io::Error),
        #[error("Firecracker vsock acknowledgement is too large")]
        WakeAcknowledgementTooLarge,
        #[error("Firecracker rejected the guest vsock connection")]
        WakeRejected,
        #[error("Firecracker returned an invalid vsock acknowledgement")]
        WakeAcknowledgement,
        #[error("Firecracker guest did not acknowledge its restored vsock")]
        GuestWakeTimeout,
        #[error("Firecracker guest returned an invalid readiness byte")]
        GuestWakeAcknowledgement,
        #[error("guest output archive is invalid: {0}")]
        Output(std::io::Error),
        #[error("microVM cleanup failed: {0}")]
        Cleanup(String),
        #[error("failed to serialize launcher response: {0}")]
        Serialize(#[from] serde_json::Error),
        #[error("launcher socket failure: {0}")]
        Socket(#[from] nix::Error),
        #[error("kernel-side transport copy failed at {path:?}: {source}")]
        Transfer {
            path: PathBuf,
            #[source]
            source: std::io::Error,
        },
        #[error("kernel-side transport returned an invalid byte count")]
        TransferCount,
        #[error("environment-start duration exceeds u64 microseconds")]
        DurationOverflow,
    }

    struct Transport {
        action: File,
        input: File,
        output: File,
    }

    struct TransportSpec<'a> {
        index: usize,
        bytes: u64,
        capacity: u64,
        sha256: &'a str,
    }

    struct Execution {
        status: LauncherStatus,
        environment_start_us: u64,
    }

    #[derive(Clone, Copy)]
    struct VmContext<'a> {
        bundle: &'a VerifiedBundle,
        cas: &'a CasStore,
        jail_root: &'a Path,
        uid: u32,
        gid: u32,
        boot_mode: BootMode,
    }

    struct CasStore {
        artifacts: BTreeMap<&'static str, PathBuf>,
    }

    impl CasStore {
        /// Materializes verified VM artifacts once beneath their content digests.
        fn prepare(
            bundle: &VerifiedBundle,
            cas_root: &Path,
            jail_root: &Path,
        ) -> Result<Self, LauncherError> {
            let root = cas_root.join("sha256");
            verify_or_create_root_directory(&root, 0o700)?;
            if fs::metadata(&root)
                .map_err(|error| io_error(&root, error))?
                .dev()
                != fs::metadata(jail_root)
                    .map_err(|error| io_error(jail_root, error))?
                    .dev()
            {
                return Err(LauncherError::CasFilesystem(root));
            }
            let mut artifacts = BTreeMap::new();
            for name in ["kernel", "rootfs", "snapshot", "memory"] {
                let source = bundle.artifact(name)?;
                let destination = root.join(bundle.artifact_sha256(name)?);
                match fs::hard_link(&source, &destination) {
                    Ok(()) => {}
                    Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                        verify_cas_object(&source, &destination, bundle.artifact_sha256(name)?)?;
                    }
                    Err(error) if error.raw_os_error() == Some(Errno::EXDEV as i32) => {
                        return Err(LauncherError::CasFilesystem(destination));
                    }
                    Err(error) => return Err(io_error(&destination, error)),
                }
                artifacts.insert(name, destination);
            }
            Ok(Self { artifacts })
        }

        /// Resolves one required immutable object from the prepared local CAS.
        fn artifact(&self, name: &'static str) -> Result<&Path, LauncherError> {
            self.artifacts
                .get(name)
                .map(PathBuf::as_path)
                .ok_or(LauncherError::CasMissing(name))
        }
    }

    struct Jail {
        root: PathBuf,
        cgroup: PathBuf,
        pid: Option<Pid>,
        supervisor: Option<Child>,
    }

    impl Drop for Jail {
        /// Tears down resources that remain after a panic or early return.
        fn drop(&mut self) {
            let action_root_exists = self.root.parent().is_some_and(Path::exists);
            if (self.supervisor.is_some() || self.cgroup.exists() || action_root_exists)
                && let Err(error) = cleanup_jail(self)
            {
                eprintln!("bsmr-sandboxd: drop cleanup failed: {error}");
            }
        }
    }

    struct Permit {
        state: Arc<(Mutex<Vec<bool>>, Condvar)>,
        slot: usize,
    }

    impl Drop for Permit {
        /// Returns one microVM slot to the bounded launcher pool.
        fn drop(&mut self) {
            let (lock, available) = &*self.state;
            let mut active = lock.lock().expect("launcher permit mutex poisoned");
            assert!(active[self.slot], "launcher slot released twice");
            active[self.slot] = false;
            available.notify_one();
        }
    }

    /// Starts the privileged launcher and serves authenticated local clients forever.
    pub fn main() -> Result<(), LauncherError> {
        if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("__supervise")) {
            return supervise();
        }
        let args = Args::parse();
        if !rustix::process::geteuid().is_root() {
            return Err(LauncherError::RootRequired);
        }
        if args.max_vms == 0 {
            return Err(LauncherError::InvalidConcurrency);
        }
        let last_slot =
            u32::try_from(args.max_vms - 1).map_err(|_| LauncherError::IdentityRange)?;
        args.uid_base
            .checked_add(last_slot)
            .ok_or(LauncherError::IdentityRange)?;
        args.gid_base
            .checked_add(last_slot)
            .ok_or(LauncherError::IdentityRange)?;
        for slot in 0..args.max_vms as u32 {
            let uid = Uid::from_raw(args.uid_base + slot);
            if nix::unistd::User::from_uid(uid)?.is_some() {
                return Err(LauncherError::IdentityInUse {
                    kind: "UID",
                    id: uid.as_raw(),
                });
            }
            let gid = Gid::from_raw(args.gid_base + slot);
            if nix::unistd::Group::from_gid(gid)?.is_some() {
                return Err(LauncherError::IdentityInUse {
                    kind: "GID",
                    id: gid.as_raw(),
                });
            }
        }
        verify_or_create_root_directory(&args.jail_root, 0o700)?;
        verify_cgroup_controllers()?;
        verify_kvm_device(Path::new("/dev/kvm"))?;
        let bundle = Arc::new(VerifiedBundle::load(
            &args.bundle,
            std::env::consts::ARCH,
            BundleTrust::RootOwned,
        )?);
        let cas = Arc::new(CasStore::prepare(&bundle, &args.cas_root, &args.jail_root)?);
        verify_reported_versions(&bundle, args.uid_base, args.gid_base)?;
        let listener = bind_socket(&args.socket, args.socket_gid)?;
        listener
            .set_nonblocking(true)
            .map_err(|source| io_error(&args.socket, source))?;
        install_termination_handler()?;
        let capacity = Arc::new((Mutex::new(vec![false; args.max_vms]), Condvar::new()));
        let accept_error = loop {
            if TERMINATE.load(Ordering::Acquire) {
                break None;
            }
            match listener.accept() {
                Ok((stream, _)) => {
                    let permit = acquire(Arc::clone(&capacity));
                    let bundle = Arc::clone(&bundle);
                    let cas = Arc::clone(&cas);
                    let jail_root = args.jail_root.clone();
                    let uid = args.uid_base + permit.slot as u32;
                    let gid = args.gid_base + permit.slot as u32;
                    let boot_mode = args.boot_mode;
                    std::thread::spawn(move || {
                        let _permit = permit;
                        if let Err(error) = handle_connection(
                            stream, &bundle, &cas, &jail_root, uid, gid, boot_mode,
                        ) {
                            eprintln!("bsmr-sandboxd: {error}");
                        }
                    });
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    std::thread::sleep(POLL_INTERVAL);
                }
                Err(error) => break Some(io_error(&args.socket, error)),
            }
        };
        wait_for_workers(&capacity);
        fs::remove_file(&args.socket).map_err(|source| io_error(&args.socket, source))?;
        if let Some(error) = accept_error {
            return Err(error);
        }
        Ok(())
    }

    /// Installs the service stop handlers before accepting the first action.
    fn install_termination_handler() -> Result<(), LauncherError> {
        let termination = Arc::clone(&TERMINATE);
        signal_hook::flag::register(SIGTERM, Arc::clone(&termination))
            .map_err(|source| io_error("SIGTERM handler", source))?;
        signal_hook::flag::register(SIGINT, termination)
            .map_err(|source| io_error("SIGINT handler", source))?;
        Ok(())
    }

    /// Waits until every admitted worker has released its launcher slot.
    fn wait_for_workers(state: &Arc<(Mutex<Vec<bool>>, Condvar)>) {
        let (lock, available) = &**state;
        let mut active = lock.lock().expect("launcher permit mutex poisoned");
        while active.iter().any(|used| *used) {
            active = available
                .wait(active)
                .expect("launcher permit mutex poisoned");
        }
    }

    /// Acquires one bounded microVM slot without admitting unbounded worker threads.
    fn acquire(state: Arc<(Mutex<Vec<bool>>, Condvar)>) -> Permit {
        let (lock, available) = &*state;
        let mut active = lock.lock().expect("launcher permit mutex poisoned");
        while active.iter().all(|used| *used) {
            active = available
                .wait(active)
                .expect("launcher permit mutex poisoned");
        }
        let slot = active
            .iter()
            .position(|used| !used)
            .expect("an available launcher slot must exist");
        active[slot] = true;
        drop(active);
        Permit { state, slot }
    }

    /// Receives one action, owns its VM lifecycle, and responds only after cleanup.
    fn handle_connection(
        mut stream: UnixStream,
        bundle: &VerifiedBundle,
        cas: &CasStore,
        jail_root: &Path,
        uid: u32,
        gid: u32,
        boot_mode: BootMode,
    ) -> Result<(), LauncherError> {
        stream
            .set_read_timeout(Some(CLIENT_IO_TIMEOUT))
            .and_then(|()| stream.set_write_timeout(Some(CLIENT_IO_TIMEOUT)))
            .map_err(|source| io_error("launcher client", source))?;
        let credentials = getsockopt(&stream, PeerCredentials)?;
        let (request, mut transport) = receive_request(&stream)?;
        let context = VmContext {
            bundle,
            cas,
            jail_root,
            uid,
            gid,
            boot_mode,
        };
        let result = execute_request(&stream, &request, &mut transport, context);
        let response = match result {
            Ok(execution) => LauncherResponse {
                protocol: PROTOCOL_VERSION,
                status: execution.status,
                cleanup_complete: true,
                environment_start_us: Some(execution.environment_start_us),
                error: None,
            },
            Err(LauncherError::Cancelled) => LauncherResponse {
                protocol: PROTOCOL_VERSION,
                status: LauncherStatus::Cancelled,
                cleanup_complete: true,
                environment_start_us: None,
                error: None,
            },
            Err(LauncherError::Cleanup(error)) => LauncherResponse {
                protocol: PROTOCOL_VERSION,
                status: LauncherStatus::Failed,
                cleanup_complete: false,
                environment_start_us: None,
                error: Some(error),
            },
            Err(error) => LauncherResponse {
                protocol: PROTOCOL_VERSION,
                status: LauncherStatus::Failed,
                cleanup_complete: true,
                environment_start_us: None,
                error: Some(error.to_string()),
            },
        };
        write_frame(&mut stream, &response)?;
        eprintln!(
            "bsmr-sandboxd: peer_uid={} action={} status={:?}",
            credentials.uid(),
            request.action_id,
            response.status
        );
        Ok(())
    }

    /// Validates a request before any privileged resource is created.
    fn validate_request(
        request: &LauncherRequest,
        bundle: &VerifiedBundle,
    ) -> Result<(), LauncherError> {
        if request.protocol != PROTOCOL_VERSION {
            return Err(LauncherError::Protocol(request.protocol));
        }
        if !valid_action_id(&request.action_id) {
            return Err(LauncherError::ActionId);
        }
        if request.environment_digest != bundle.environment_digest() {
            return Err(LauncherError::EnvironmentDigest);
        }
        if request.input_bytes == 0 {
            return Err(LauncherError::EmptyInput);
        }
        if request.input_bytes > MAX_INPUT_BYTES {
            return Err(LauncherError::InputTooLarge);
        }
        if !valid_sha256(&request.action_sha256) || !valid_sha256(&request.input_sha256) {
            return Err(LauncherError::TransportDigestShape);
        }
        if request.output_bytes > MAX_OUTPUT_BYTES {
            return Err(LauncherError::OutputTooLarge);
        }
        if request.output_bytes != MAX_OUTPUT_BYTES {
            return Err(LauncherError::OutputCapacity);
        }
        validate_machine(request)?;
        if request
            .timeout_ms
            .is_some_and(|timeout| timeout > MAX_TIMEOUT_MS)
        {
            return Err(LauncherError::TimeoutTooLarge);
        }
        Ok(())
    }

    /// Enforces the fixed resource shape of the cache-keyed sandbox profile.
    fn validate_machine(request: &LauncherRequest) -> Result<(), LauncherError> {
        if request.vcpu_count != VCPU_COUNT {
            return Err(LauncherError::VcpuCount(request.vcpu_count));
        }
        if request.memory_mib != MEMORY_MIB {
            return Err(LauncherError::MemorySize(request.memory_mib));
        }
        Ok(())
    }

    /// Receives one bounded JSON request and exactly three close-on-exec descriptors.
    fn receive_request(stream: &UnixStream) -> Result<(LauncherRequest, Transport), LauncherError> {
        let mut bytes = vec![0u8; MAX_MESSAGE_BYTES + 4];
        let mut iov = [IoSliceMut::new(&mut bytes)];
        let mut control_bytes = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(3))];
        let mut control = rustix::net::RecvAncillaryBuffer::new(&mut control_bytes);
        let message = rustix::net::recvmsg(
            stream,
            &mut iov,
            &mut control,
            rustix::net::RecvFlags::CMSG_CLOEXEC,
        )
        .map_err(|source| io_error("launcher request", source.into()))?;
        let received = message.bytes;
        let mut descriptors = Vec::new();
        for message in control.drain() {
            if let rustix::net::RecvAncillaryMessage::ScmRights(rights) = message {
                descriptors.extend(rights);
            }
        }
        if descriptors.len() != 3 {
            return Err(LauncherError::FileDescriptors);
        }
        if message.flags.contains(rustix::net::ReturnFlags::CTRUNC) {
            return Err(LauncherError::FileDescriptors);
        }
        if received < 4 {
            return Err(LauncherError::TruncatedFrame);
        }
        let size = u32::from_be_bytes(bytes[..4].try_into().expect("four-byte frame")) as usize;
        if size > MAX_MESSAGE_BYTES {
            return Err(LauncherError::MessageTooLarge);
        }
        let mut payload = bytes[4..received].to_vec();
        if payload.len() < size {
            let missing = size - payload.len();
            let mut tail = vec![0; missing];
            (&*stream)
                .read_exact(&mut tail)
                .map_err(|source| io_error("launcher request", source))?;
            payload.extend_from_slice(&tail);
        }
        if payload.len() != size {
            return Err(LauncherError::FrameLength);
        }
        let request: LauncherRequest = serde_json::from_slice(&payload)?;
        let mut descriptors = descriptors.into_iter();
        let transport = Transport {
            action: File::from(descriptors.next().expect("three descriptors")),
            input: File::from(descriptors.next().expect("three descriptors")),
            output: File::from(descriptors.next().expect("three descriptors")),
        };
        validate_transport(&request, &transport)?;
        Ok((request, transport))
    }

    /// Verifies descriptor kinds, sizes, and access modes before use.
    fn validate_transport(
        request: &LauncherRequest,
        transport: &Transport,
    ) -> Result<(), LauncherError> {
        let files = [&transport.action, &transport.input, &transport.output];
        for (index, file) in files.iter().enumerate() {
            if !file
                .metadata()
                .map_err(|source| io_error("transport", source))?
                .is_file()
            {
                return Err(LauncherError::FileType { index });
            }
            let flags = OFlag::from_bits_truncate(fcntl(file, FcntlArg::F_GETFL)?);
            let access = flags & OFlag::O_ACCMODE;
            let valid = if index == 2 {
                access == OFlag::O_RDWR
            } else {
                access == OFlag::O_RDONLY || access == OFlag::O_RDWR
            };
            if !valid {
                return Err(LauncherError::FileAccess { index });
            }
        }
        let action_bytes = transport
            .action
            .metadata()
            .map_err(|source| io_error("action", source))?
            .len();
        if action_bytes == 0 || action_bytes > MAX_ACTION_BYTES {
            return Err(LauncherError::FileSize {
                index: 0,
                actual: action_bytes,
                expected: MAX_ACTION_BYTES,
            });
        }
        for (index, actual, expected) in [
            (
                1,
                transport
                    .input
                    .metadata()
                    .map_err(|source| io_error("input", source))?
                    .len(),
                request.input_bytes,
            ),
            (
                2,
                transport
                    .output
                    .metadata()
                    .map_err(|source| io_error("output", source))?
                    .len(),
                request.output_bytes,
            ),
        ] {
            if actual != expected {
                return Err(LauncherError::FileSize {
                    index,
                    actual,
                    expected,
                });
            }
        }
        Ok(())
    }

    /// Runs one VM and guarantees cgroup and jail cleanup on every terminal path.
    fn execute_request(
        stream: &UnixStream,
        request: &LauncherRequest,
        transport: &mut Transport,
        context: VmContext<'_>,
    ) -> Result<Execution, LauncherError> {
        validate_request(request, context.bundle)?;
        let mut jail = prepare_jail(request, transport, context)?;
        let result = run_microvm(stream, request, transport, &mut jail, context);
        let cleanup = cleanup_jail(&mut jail);
        match (result, cleanup) {
            (_, Err(error)) => Err(LauncherError::Cleanup(error.to_string())),
            (result, Ok(())) => result,
        }
    }

    /// Creates a unique jail populated only with declared immutable resources and block files.
    fn prepare_jail(
        request: &LauncherRequest,
        transport: &mut Transport,
        context: VmContext<'_>,
    ) -> Result<Jail, LauncherError> {
        let firecracker = context.bundle.artifact("firecracker")?;
        let executable = firecracker
            .file_name()
            .ok_or_else(|| LauncherError::UnsafePath(firecracker.clone()))?;
        let root = context
            .jail_root
            .join(executable)
            .join(&request.action_id)
            .join("root");
        if root.exists() {
            return Err(LauncherError::UnsafePath(root));
        }
        let mut jail = Jail {
            root,
            cgroup: Path::new("/sys/fs/cgroup")
                .join(CGROUP_PARENT)
                .join(&request.action_id),
            pid: None,
            supervisor: None,
        };
        let populate = (|| {
            fs::create_dir_all(&jail.root).map_err(|source| io_error(&jail.root, source))?;
            link_immutable(context.cas.artifact("kernel")?, jail.root.join("kernel"))?;
            link_immutable(context.cas.artifact("rootfs")?, jail.root.join("rootfs"))?;
            if matches!(context.boot_mode, BootMode::Snapshot) {
                for artifact in ["snapshot", "memory"] {
                    link_immutable(context.cas.artifact(artifact)?, jail.root.join(artifact))?;
                }
            }
            let action_bytes = transport
                .action
                .metadata()
                .map_err(|error| io_error("action transport", error))?
                .len();
            copy_transport(
                &mut transport.action,
                &jail.root.join("action"),
                TransportSpec {
                    index: 0,
                    bytes: action_bytes,
                    capacity: MAX_ACTION_BYTES,
                    sha256: &request.action_sha256,
                },
                context.uid,
                context.gid,
            )?;
            copy_transport(
                &mut transport.input,
                &jail.root.join("input"),
                TransportSpec {
                    index: 1,
                    bytes: request.input_bytes,
                    capacity: MAX_INPUT_BYTES,
                    sha256: &request.input_sha256,
                },
                context.uid,
                context.gid,
            )?;
            let output = jail.root.join("output");
            File::options()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&output)
                .and_then(|file| file.set_len(request.output_bytes))
                .map_err(|source| io_error(&output, source))?;
            chown(
                &output,
                Some(nix::unistd::Uid::from_raw(context.uid)),
                Some(Gid::from_raw(context.gid)),
            )?;
            Ok(())
        })();
        if let Err(error) = populate {
            cleanup_jail(&mut jail)?;
            return Err(error);
        }
        Ok(jail)
    }

    /// Hard-links one verified immutable artifact into an action jail.
    fn link_immutable(
        source: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> Result<(), LauncherError> {
        let destination = destination.as_ref();
        fs::hard_link(source, destination).map_err(|error| io_error(destination, error))
    }

    /// Rejects a pre-existing digest name unless it is the verified source inode.
    fn verify_cas_object(
        source: &Path,
        destination: &Path,
        expected: &str,
    ) -> Result<(), LauncherError> {
        let source_metadata =
            fs::symlink_metadata(source).map_err(|error| io_error(source, error))?;
        let destination_metadata =
            fs::symlink_metadata(destination).map_err(|error| io_error(destination, error))?;
        if !source_metadata.is_file() || !destination_metadata.is_file() {
            return Err(LauncherError::CasCollision(destination.into()));
        }
        if source_metadata.dev() == destination_metadata.dev()
            && source_metadata.ino() == destination_metadata.ino()
        {
            return Ok(());
        }
        bsmr_sandbox::verify_sha256(destination, expected).map_err(Into::into)
    }

    /// Confirms both trusted executables report the manifest's same pinned release.
    fn verify_reported_versions(
        bundle: &VerifiedBundle,
        uid: u32,
        gid: u32,
    ) -> Result<(), LauncherError> {
        for (artifact, product) in [("firecracker", "Firecracker"), ("jailer", "Jailer")] {
            let executable = bundle.artifact(artifact)?;
            let output = Command::new(&executable)
                .arg("--version")
                .env_clear()
                .uid(uid)
                .gid(gid)
                .stdin(Stdio::null())
                .output()
                .map_err(LauncherError::Jailer)?;
            let expected = format!("{product} v{}", bundle.firecracker_version());
            let reported = reported_version(&output.stdout).map(str::to_owned);
            if !output.status.success() || reported.as_deref() != Some(&expected) {
                return Err(LauncherError::Version {
                    artifact,
                    reported,
                    status: output.status,
                    expected,
                    stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                });
            }
        }
        Ok(())
    }

    /// Reads the canonical first line from a trusted release's diagnostic output.
    fn reported_version(stdout: &[u8]) -> Option<&str> {
        std::str::from_utf8(stdout).ok()?.lines().next()
    }

    /// Copies an untrusted transport file from offset zero without inheriting its name.
    fn copy_transport(
        source: &mut File,
        destination: &Path,
        spec: TransportSpec<'_>,
        uid: u32,
        gid: u32,
    ) -> Result<(), LauncherError> {
        let mut output = File::options()
            .write(true)
            .create_new(true)
            .mode(0o400)
            .open(destination)
            .map_err(|error| io_error(destination, error))?;
        copy_exact_transport(source, &mut output, spec.bytes, destination)?;
        bsmr_sandbox::verify_sha256(destination, spec.sha256).map_err(|source| {
            LauncherError::TransportDigest {
                index: spec.index,
                source,
            }
        })?;
        output
            .set_len(spec.capacity)
            .map_err(|error| io_error(destination, error))?;
        output
            .sync_all()
            .map_err(|error| io_error(destination, error))?;
        chown(
            destination,
            Some(nix::unistd::Uid::from_raw(uid)),
            Some(Gid::from_raw(gid)),
        )?;
        Ok(())
    }

    /// Copies exactly the admitted descriptor length and rejects concurrent growth or truncation.
    fn copy_exact_transport(
        source: &mut File,
        destination: &mut File,
        expected: u64,
        path: &Path,
    ) -> Result<(), LauncherError> {
        sendfile_exact(source, destination, expected, path)?;
        let mut extra = [0u8; 1];
        let has_extra = source
            .read_at(&mut extra, expected)
            .map_err(|error| io_error(path, error))?
            != 0;
        if has_extra {
            return Err(LauncherError::TransportMutation {
                path: path.to_owned(),
            });
        }
        Ok(())
    }

    /// Copies a fixed range entirely inside the kernel, without userspace buffers.
    fn sendfile_exact(
        source: &File,
        destination: &File,
        expected: u64,
        path: &Path,
    ) -> Result<(), LauncherError> {
        let mut offset = 0_u64;
        let mut remaining = expected;
        while remaining > 0 {
            let count = usize::try_from(remaining).unwrap_or(usize::MAX);
            let copied = rustix::fs::sendfile(destination, source, Some(&mut offset), count)
                .map_err(|source| LauncherError::Transfer {
                    path: path.to_owned(),
                    source: source.into(),
                })?;
            if copied == 0 {
                return Err(LauncherError::TransportMutation {
                    path: path.to_owned(),
                });
            }
            let copied = u64::try_from(copied).map_err(|_| LauncherError::TransferCount)?;
            remaining -= copied;
        }
        Ok(())
    }

    /// Starts the jailed VMM, configures its API, and monitors exit or cancellation.
    fn run_microvm(
        stream: &UnixStream,
        request: &LauncherRequest,
        transport: &mut Transport,
        jail: &mut Jail,
        context: VmContext<'_>,
    ) -> Result<Execution, LauncherError> {
        let environment_started = Instant::now();
        let jailer = context.bundle.artifact("jailer")?;
        let firecracker = context.bundle.artifact("firecracker")?;
        let cgroup_memory = (u64::from(request.memory_mib) + 128) * 1024 * 1024;
        let jailer_log_path = jail
            .root
            .parent()
            .ok_or_else(|| LauncherError::UnsafePath(jail.root.clone()))?
            .join("jailer.log");
        let jailer_log = File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&jailer_log_path)
            .map_err(|error| io_error(&jailer_log_path, error))?;
        let jail_base = jail
            .root
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .ok_or_else(|| LauncherError::UnsafePath(jail.root.clone()))?;
        let jailer_arguments = [
            "--id".into(),
            request.action_id.clone().into(),
            "--exec-file".into(),
            firecracker.as_os_str().to_owned(),
            "--uid".into(),
            context.uid.to_string().into(),
            "--gid".into(),
            context.gid.to_string().into(),
            "--chroot-base-dir".into(),
            jail_base.as_os_str().to_owned(),
            "--cgroup-version".into(),
            "2".into(),
            "--parent-cgroup".into(),
            CGROUP_PARENT.into(),
            "--cgroup".into(),
            "pids.max=64".into(),
            "--cgroup".into(),
            format!("memory.max={cgroup_memory}").into(),
            "--resource-limit".into(),
            "no-file=256".into(),
            "--resource-limit".into(),
            format!("fsize={MAX_OUTPUT_BYTES}").into(),
            "--".into(),
            "--api-sock".into(),
            "api.socket".into(),
        ];
        let executable = std::env::current_exe().map_err(LauncherError::Jailer)?;
        let mut command = Command::new(executable);
        command
            .arg("__supervise")
            .arg(
                jail.root
                    .parent()
                    .ok_or_else(|| LauncherError::UnsafePath(jail.root.clone()))?
                    .join("firecracker.pid"),
            )
            .arg(jailer)
            .args(jailer_arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::from(
                jailer_log
                    .try_clone()
                    .map_err(|error| io_error(&jailer_log_path, error))?,
            ))
            .stderr(Stdio::from(jailer_log));
        // SAFETY: `arm_parent_death` performs only `getppid(2)` and `prctl(2)` through rustix.
        // It does not allocate, lock, or access shared process state after `fork(2)`.
        unsafe {
            command.pre_exec(arm_parent_death);
        }
        jail.supervisor = Some(command.spawn().map_err(LauncherError::Jailer)?);
        let api = jail.root.join("api.socket");
        let pid_file = jail
            .root
            .parent()
            .ok_or_else(|| LauncherError::UnsafePath(jail.root.clone()))?
            .join("firecracker.pid");
        wait_for_files(jail, &api, &pid_file, &jailer_log_path, BOOT_TIMEOUT)?;
        if !jail.cgroup.join("cgroup.kill").is_file() {
            return Err(LauncherError::Cleanup(format!(
                "microVM cgroup {:?} has no cgroup.kill",
                jail.cgroup
            )));
        }
        let pid = read_pid(&pid_file)?;
        jail.pid = Some(pid);
        let supervisor = jail
            .supervisor
            .as_mut()
            .ok_or(LauncherError::MissingSupervisor)?;
        match context.boot_mode {
            BootMode::Snapshot => load_snapshot(&api)?,
            BootMode::Fresh => {
                configure_fresh(&api, request)?;
                wait_for_guest_ready(supervisor, &jailer_log_path, BOOT_TIMEOUT)?;
            }
        }
        wake_guest(&jail.root.join("vsock.socket"))?;
        let environment_start_us = u64::try_from(environment_started.elapsed().as_micros())
            .map_err(|_| LauncherError::DurationOverflow)?;
        eprintln!(
            "bsmr-sandboxd: action={} boot_mode={:?} environment_start_us={}",
            request.action_id, context.boot_mode, environment_start_us
        );
        let deadline = request
            .timeout_ms
            .map(|timeout| Instant::now() + Duration::from_millis(timeout) + BOOT_TIMEOUT);
        let status = monitor(stream, supervisor, deadline)?;
        if status == LauncherStatus::TimedOut {
            return Err(LauncherError::HostDeadline(read_log_tail(
                &jailer_log_path,
            )?));
        }
        if status == LauncherStatus::Completed {
            copy_guest_output(&jail.root.join("output"), &mut transport.output)?;
        }
        Ok(Execution {
            status,
            environment_start_us,
        })
    }

    /// Reads only the bounded tail of a trusted VMM and guest diagnostic log.
    fn read_log_tail(path: &Path) -> Result<String, LauncherError> {
        let mut file = File::open(path).map_err(|source| io_error(path, source))?;
        let length = file
            .metadata()
            .map_err(|source| io_error(path, source))?
            .len();
        file.seek(SeekFrom::Start(length.saturating_sub(MAX_DIAGNOSTIC_BYTES)))
            .map_err(|source| io_error(path, source))?;
        let mut bytes = Vec::new();
        file.take(MAX_DIAGNOSTIC_BYTES)
            .read_to_end(&mut bytes)
            .map_err(|source| io_error(path, source))?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Waits for the jailed API socket and PID file to appear together.
    fn wait_for_files(
        jail: &mut Jail,
        api: &Path,
        pid: &Path,
        log: &Path,
        timeout: Duration,
    ) -> Result<(), LauncherError> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if api.exists() && pid.exists() {
                return Ok(());
            }
            if let Some(status) = jail
                .supervisor
                .as_mut()
                .expect("supervisor exists while booting")
                .try_wait()
                .map_err(LauncherError::Jailer)?
            {
                let contents = fs::read_to_string(log).map_err(|error| io_error(log, error))?;
                return Err(LauncherError::SupervisorExit {
                    status,
                    log: contents.trim().to_owned(),
                });
            }
            std::thread::sleep(POLL_INTERVAL);
        }
        Err(LauncherError::ApiTimeout)
    }

    /// Waits until fresh guest PID 1 has bound vsock before the host sends `CONNECT`.
    fn wait_for_guest_ready(
        supervisor: &mut Child,
        log: &Path,
        timeout: Duration,
    ) -> Result<(), LauncherError> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            let metadata = fs::metadata(log).map_err(|source| io_error(log, source))?;
            if metadata.len() > MAX_GUEST_READY_LOG_BYTES {
                return Err(LauncherError::GuestReadyLogLimit);
            }
            let contents = fs::read_to_string(log).map_err(|source| io_error(log, source))?;
            if contents.contains(READY_MARKER) {
                return Ok(());
            }
            if let Some(status) = supervisor.try_wait().map_err(LauncherError::Jailer)? {
                return Err(LauncherError::SupervisorExit {
                    status,
                    log: contents.trim().to_owned(),
                });
            }
            std::thread::sleep(POLL_INTERVAL);
        }
        Err(LauncherError::GuestReadyTimeout)
    }

    /// Keeps Firecracker as PID 1 in a child namespace and links its life to sandboxd.
    fn supervise() -> Result<(), LauncherError> {
        arm_parent_death().map_err(LauncherError::Jailer)?;
        let mut arguments = std::env::args_os().skip(2);
        let pid_file = arguments
            .next()
            .ok_or(LauncherError::SupervisorArgument("PID path"))?;
        let jailer = arguments
            .next()
            .ok_or(LauncherError::SupervisorArgument("jailer path"))?;
        let arguments = arguments.collect::<Vec<_>>();
        unshare(CloneFlags::CLONE_NEWPID)?;
        // SAFETY: `__supervise` starts after `exec` with one thread. No allocator or lock state
        // from the multithreaded launcher survives into this process before `fork(2)`.
        match unsafe { nix::unistd::fork() }? {
            nix::unistd::ForkResult::Child => {
                arm_parent_death().map_err(LauncherError::Jailer)?;
                let error = Command::new(jailer).args(arguments).exec();
                Err(LauncherError::Jailer(error))
            }
            nix::unistd::ForkResult::Parent { child } => {
                fs::write(&pid_file, child.as_raw().to_string())
                    .map_err(|error| io_error(PathBuf::from(pid_file), error))?;
                match waitpid(child, None)? {
                    WaitStatus::Exited(_, 0) => Ok(()),
                    status => Err(LauncherError::FirecrackerExit(status)),
                }
            }
        }
    }

    /// Arms an unmaskable parent-death signal and closes the setup race.
    fn arm_parent_death() -> std::io::Result<()> {
        let parent = rustix::process::getppid();
        rustix::process::set_parent_process_death_signal(Some(rustix::process::Signal::KILL))?;
        if rustix::process::getppid() != parent {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "parent exited while arming PR_SET_PDEATHSIG",
            ));
        }
        Ok(())
    }

    /// Reads the jailer's authoritative host PID for the Firecracker process.
    fn read_pid(path: &Path) -> Result<Pid, LauncherError> {
        let value = fs::read_to_string(path).map_err(|source| io_error(path, source))?;
        let pid = value
            .trim()
            .parse::<i32>()
            .map_err(|_| LauncherError::Pid)?;
        if pid <= 1 {
            return Err(LauncherError::Pid);
        }
        Ok(Pid::from_raw(pid))
    }

    /// Configures the fresh-boot correctness oracle with the production device graph.
    fn configure_fresh(api: &Path, request: &LauncherRequest) -> Result<(), LauncherError> {
        let api = ApiClient::new(api, CLIENT_IO_TIMEOUT);
        api.put(
            "/machine-config",
            &serde_json::json!({
                "vcpu_count": request.vcpu_count,
                "mem_size_mib": request.memory_mib,
                "smt": false,
                "track_dirty_pages": false
            }),
        )?;
        api.put(
            "/boot-source",
            &serde_json::json!({
                "kernel_image_path": "kernel",
                "boot_args": "root=/dev/vda ro console=ttyS0 reboot=k panic=1 pci=off init=/sbin/bsmr-sandbox-guest"
            }),
        )?;
        for (id, path, root, read_only) in [
            ("rootfs", "rootfs", true, true),
            ("input", "input", false, true),
            ("output", "output", false, false),
            ("action", "action", false, true),
        ] {
            let endpoint = format!("/drives/{id}");
            api.put(
                &endpoint,
                &serde_json::json!({
                    "drive_id": id,
                    "path_on_host": path,
                    "is_root_device": root,
                    "is_read_only": read_only
                }),
            )?;
        }
        api.put(
            "/vsock",
            &serde_json::json!({
                "vsock_id": "bsmr-control",
                "guest_cid": 3,
                "uds_path": "vsock.socket"
            }),
        )?;
        api.put("/entropy", &serde_json::json!({}))?;
        api.put(
            "/actions",
            &serde_json::json!({"action_type": "InstanceStart"}),
        )?;
        Ok(())
    }

    /// Restores the authenticated state through Firecracker's kernel COW backend.
    fn load_snapshot(api: &Path) -> Result<(), LauncherError> {
        ApiClient::new(api, CLIENT_IO_TIMEOUT).put(
            "/snapshot/load",
            &serde_json::json!({
                "snapshot_path": "snapshot",
                "mem_backend": {
                    "backend_path": "memory",
                    "backend_type": "File"
                },
                "track_dirty_pages": false,
                "resume_vm": true,
                "clock_realtime": true
            }),
        )?;
        Ok(())
    }

    /// Connects to the snapshot-preserved listener and releases exactly one guest.
    fn wake_guest(socket: &Path) -> Result<(), LauncherError> {
        let mut stream = connect_ready_guest(socket, BOOT_TIMEOUT)?;
        stream
            .write_all(&[WAKE_BYTE])
            .map_err(LauncherError::WakeIo)
    }

    /// Retries only connections that fail before the action-release byte is sent.
    fn connect_ready_guest(socket: &Path, timeout: Duration) -> Result<UnixStream, LauncherError> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(stream) = try_connect_ready_guest(socket)? {
                return Ok(stream);
            }
            if Instant::now() >= deadline {
                return Err(LauncherError::GuestWakeTimeout);
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    /// Establishes one connection without releasing guest action state.
    fn try_connect_ready_guest(socket: &Path) -> Result<Option<UnixStream>, LauncherError> {
        let mut stream = match UnixStream::connect(socket) {
            Ok(stream) => stream,
            Err(_) => return Ok(None),
        };
        stream
            .set_read_timeout(Some(CLIENT_IO_TIMEOUT))
            .and_then(|()| stream.set_write_timeout(Some(CLIENT_IO_TIMEOUT)))
            .map_err(LauncherError::WakeIo)?;
        if writeln!(stream, "CONNECT {WAKE_PORT}").is_err() {
            return Ok(None);
        }
        let mut acknowledgement = Vec::new();
        loop {
            let mut byte = [0u8; 1];
            if stream.read_exact(&mut byte).is_err() {
                return Ok(None);
            }
            if byte[0] == b'\n' {
                break;
            }
            if acknowledgement.len() == 63 {
                return Err(LauncherError::WakeAcknowledgementTooLarge);
            }
            acknowledgement.push(byte[0]);
        }
        let host_port = acknowledgement
            .strip_prefix(b"OK ")
            .ok_or(LauncherError::WakeRejected)?;
        if host_port.is_empty() || !host_port.iter().all(u8::is_ascii_digit) {
            return Err(LauncherError::WakeAcknowledgement);
        }
        let mut ready = [0u8; 1];
        if stream.read_exact(&mut ready).is_err() {
            return Ok(None);
        }
        if ready != [GUEST_READY_BYTE] {
            return Err(LauncherError::GuestWakeAcknowledgement);
        }
        Ok(Some(stream))
    }

    /// Monitors Firecracker without trusting guest completion messages.
    fn monitor(
        stream: &UnixStream,
        supervisor: &mut Child,
        deadline: Option<Instant>,
    ) -> Result<LauncherStatus, LauncherError> {
        loop {
            if TERMINATE.load(Ordering::Acquire) {
                return Err(LauncherError::Cancelled);
            }
            if let Some(status) = supervisor.try_wait().map_err(LauncherError::Jailer)? {
                if status.success() {
                    return Ok(LauncherStatus::Completed);
                }
                return Err(LauncherError::SupervisorExit {
                    status,
                    log: String::new(),
                });
            }
            let mut byte = [0u8; 1];
            match recv(
                stream.as_raw_fd(),
                &mut byte,
                MsgFlags::MSG_PEEK | MsgFlags::MSG_DONTWAIT,
            ) {
                Ok(0) => return Err(LauncherError::Cancelled),
                Ok(_) => return Err(LauncherError::UnexpectedClientData),
                Err(Errno::EAGAIN) => {}
                Err(error) => return Err(LauncherError::Socket(error)),
            }
            if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                return Ok(LauncherStatus::TimedOut);
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    /// Copies only the logical tar stream out of the sparse guest block device.
    fn copy_guest_output(source: &Path, destination: &mut File) -> Result<(), LauncherError> {
        let mut input = File::open(source).map_err(|error| io_error(source, error))?;
        let logical_bytes = tar_stream_len(&mut input)?;
        destination
            .set_len(0)
            .map_err(|error| io_error("output transport", error))?;
        destination
            .seek(SeekFrom::Start(0))
            .map_err(|error| io_error("output transport", error))?;
        sendfile_exact(&input, destination, logical_bytes, source)?;
        destination
            .sync_all()
            .map_err(|error| io_error("output transport", error))?;
        Ok(())
    }

    /// Finds the end of a valid tar stream without copying the sparse device tail.
    fn tar_stream_len(file: &mut File) -> Result<u64, LauncherError> {
        file.seek(SeekFrom::Start(0))
            .map_err(LauncherError::Output)?;
        let mut archive = tar::Archive::new(file);
        let entries = archive.entries().map_err(LauncherError::Output)?;
        for entry in entries {
            let mut entry = entry.map_err(LauncherError::Output)?;
            std::io::copy(&mut entry, &mut std::io::sink()).map_err(LauncherError::Output)?;
        }
        archive
            .into_inner()
            .stream_position()
            .map_err(LauncherError::Output)
    }

    /// Kills the entire cgroup, waits for exit, and removes the unique jail.
    fn cleanup_jail(jail: &mut Jail) -> Result<(), LauncherError> {
        let mut failures = Vec::new();
        if jail.pid.is_some() && !jail.cgroup.exists() {
            failures.push(format!("microVM cgroup {:?} is missing", jail.cgroup));
        }
        if jail.cgroup.exists() {
            let kill = jail.cgroup.join("cgroup.kill");
            if let Err(error) = fs::write(&kill, b"1") {
                failures.push(io_error(kill, error).to_string());
            }
        }
        if let Some(mut supervisor) = jail.supervisor.take() {
            let mut reaped = false;
            match supervisor.try_wait() {
                Ok(Some(_)) => reaped = true,
                Ok(None) => {
                    if let Err(error) = supervisor.kill() {
                        failures.push(LauncherError::Jailer(error).to_string());
                    }
                    let deadline = Instant::now() + CLEANUP_TIMEOUT;
                    loop {
                        match supervisor.try_wait() {
                            Ok(Some(_)) => {
                                reaped = true;
                                break;
                            }
                            Ok(None) if Instant::now() < deadline => {
                                std::thread::sleep(POLL_INTERVAL);
                            }
                            Ok(None) => {
                                failures.push("launcher supervisor survived SIGKILL".to_owned());
                                break;
                            }
                            Err(error) => {
                                failures.push(LauncherError::Jailer(error).to_string());
                                break;
                            }
                        }
                    }
                }
                Err(error) => failures.push(LauncherError::Jailer(error).to_string()),
            }
            if reaped {
                jail.pid = None;
            }
        }
        if let Some(pid) = jail.pid
            && !matches!(kill(pid, None), Err(Errno::ESRCH))
        {
            let deadline = Instant::now() + CLEANUP_TIMEOUT;
            while Instant::now() < deadline && !matches!(kill(pid, None), Err(Errno::ESRCH)) {
                std::thread::sleep(POLL_INTERVAL);
            }
            if !matches!(kill(pid, None), Err(Errno::ESRCH)) {
                failures.push(format!("PID {pid} survived launcher termination"));
            }
        }
        if jail.cgroup.exists() {
            let deadline = Instant::now() + CLEANUP_TIMEOUT;
            loop {
                match fs::remove_dir(&jail.cgroup) {
                    Ok(()) => break,
                    Err(error)
                        if error.kind() == ErrorKind::ResourceBusy && Instant::now() < deadline =>
                    {
                        std::thread::sleep(POLL_INTERVAL);
                    }
                    Err(error) => {
                        failures.push(io_error(&jail.cgroup, error).to_string());
                        break;
                    }
                }
            }
        }
        let action_root = jail
            .root
            .parent()
            .ok_or_else(|| LauncherError::UnsafePath(jail.root.clone()))?;
        if action_root.exists()
            && let Err(error) = fs::remove_dir_all(action_root)
        {
            failures.push(io_error(action_root, error).to_string());
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(LauncherError::Cleanup(failures.join("; ")))
        }
    }

    /// Creates or validates one root-owned non-writable service directory.
    fn verify_or_create_root_directory(path: &Path, mode: u32) -> Result<(), LauncherError> {
        match fs::symlink_metadata(path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir_all(path).map_err(|source| io_error(path, source))?;
                fs::set_permissions(path, fs::Permissions::from_mode(mode))
                    .map_err(|source| io_error(path, source))?;
            }
            Err(error) => return Err(io_error(path, error)),
        }
        for ancestor in path.ancestors() {
            let metadata =
                fs::symlink_metadata(ancestor).map_err(|source| io_error(ancestor, source))?;
            if !metadata.is_dir() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
                return Err(LauncherError::UnsafePath(ancestor.to_owned()));
            }
        }
        Ok(())
    }

    /// Requires the controllers used by every action cgroup before accepting clients.
    fn verify_cgroup_controllers() -> Result<(), LauncherError> {
        let path = Path::new("/sys/fs/cgroup/cgroup.controllers");
        let controllers = fs::read_to_string(path).map_err(|source| io_error(path, source))?;
        for required in ["memory", "pids"] {
            if !controllers
                .split_ascii_whitespace()
                .any(|value| value == required)
            {
                return Err(LauncherError::MissingController(required));
            }
        }
        Ok(())
    }

    /// Requires the launcher itself to own a usable Kernel-based Virtual Machine device.
    fn verify_kvm_device(path: &Path) -> Result<(), LauncherError> {
        let metadata = fs::symlink_metadata(path).map_err(|source| io_error(path, source))?;
        if !metadata.file_type().is_char_device() {
            return Err(LauncherError::UnsafePath(path.to_owned()));
        }
        let _device = rustix::fs::open(path, OFlags::RDWR | OFlags::CLOEXEC, Mode::empty())
            .map_err(|source| io_error(path, source.into()))?;
        Ok(())
    }

    /// Binds one group-restricted socket and only removes an owned stale socket.
    fn bind_socket(path: &Path, gid: u32) -> Result<UnixListener, LauncherError> {
        let parent = path
            .parent()
            .ok_or_else(|| LauncherError::UnsafePath(path.to_owned()))?;
        verify_or_create_root_directory(parent, 0o755)?;
        match fs::symlink_metadata(path) {
            Ok(metadata) => {
                if !metadata.file_type().is_socket() || metadata.uid() != 0 {
                    return Err(LauncherError::UnsafePath(path.to_owned()));
                }
                match UnixStream::connect(path) {
                    Ok(_) => return Err(LauncherError::UnsafePath(path.to_owned())),
                    Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => {
                        fs::remove_file(path).map_err(|source| io_error(path, source))?;
                    }
                    Err(error) => return Err(io_error(path, error)),
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(path, error)),
        }
        let listener = UnixListener::bind(path).map_err(|source| io_error(path, source))?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o660))
            .map_err(|source| io_error(path, source))?;
        chown(path, None, Some(Gid::from_raw(gid)))?;
        Ok(listener)
    }

    /// Writes one length-prefixed protocol response.
    fn write_frame<T: Serialize>(stream: &mut UnixStream, value: &T) -> Result<(), LauncherError> {
        let payload = serde_json::to_vec(value)?;
        if payload.len() > MAX_MESSAGE_BYTES {
            return Err(LauncherError::MessageTooLarge);
        }
        stream
            .write_all(&(payload.len() as u32).to_be_bytes())
            .and_then(|_| stream.write_all(&payload))
            .map_err(|source| io_error("launcher response", source))
    }

    /// Checks the canonical lowercase UUID shape used as an untrusted path component.
    fn valid_action_id(value: &str) -> bool {
        value.len() == 36
            && value.bytes().enumerate().all(|(index, byte)| match index {
                8 | 13 | 18 | 23 => byte == b'-',
                _ => byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte),
            })
    }

    /// Checks the canonical lowercase digest shape before any filesystem work.
    fn valid_sha256(value: &str) -> bool {
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }

    /// Attaches one filesystem path to an I/O error.
    fn io_error(path: impl Into<PathBuf>, source: std::io::Error) -> LauncherError {
        LauncherError::Io {
            path: path.into(),
            source,
        }
    }

    #[cfg(test)]
    mod tests {
        use std::fs;
        use std::io::Read;
        use std::io::Write;
        use std::os::unix::net::UnixListener;
        use std::path::Path;
        use std::process::Command;
        use std::thread;
        use std::time::Duration;

        use nix::errno::Errno;
        use nix::sys::signal::kill;
        use nix::unistd::Pid;
        use sha2::Digest;
        use sha2::Sha256;

        use super::GUEST_READY_BYTE;
        use super::Jail;
        use super::LauncherRequest;
        use super::MEMORY_MIB;
        use super::PROTOCOL_VERSION;
        use super::READY_MARKER;
        use super::TransportSpec;
        use super::VCPU_COUNT;
        use super::WAKE_BYTE;
        use super::WAKE_PORT;
        use super::cleanup_jail;
        use super::copy_exact_transport;
        use super::copy_transport;
        use super::link_immutable;
        use super::reported_version;
        use super::valid_action_id;
        use super::valid_sha256;
        use super::validate_machine;
        use super::verify_cas_object;
        use super::verify_kvm_device;
        use super::wait_for_guest_ready;
        use super::wake_guest;

        /// Constructs the smallest request accepted by protocol validation.
        fn request(environment_digest: &str) -> LauncherRequest {
            LauncherRequest {
                protocol: PROTOCOL_VERSION,
                action_id: "01234567-89ab-cdef-0123-456789abcdef".to_owned(),
                environment_digest: environment_digest.to_owned(),
                input_bytes: 1,
                input_sha256: format!("{:x}", Sha256::digest(b"i")),
                output_bytes: 1,
                action_sha256: format!("{:x}", Sha256::digest(b"a")),
                vcpu_count: VCPU_COUNT,
                memory_mib: MEMORY_MIB,
                timeout_ms: None,
            }
        }

        /// The authenticated client cannot enlarge the profile's resource envelope.
        #[test]
        fn machine_shape_is_fixed_by_the_privileged_launcher() {
            assert!(validate_machine(&request("sha256:test")).is_ok());

            let mut oversized = request("sha256:test");
            oversized.memory_mib += 1;
            assert!(validate_machine(&oversized).is_err());

            let mut oversized = request("sha256:test");
            oversized.vcpu_count += 1;
            assert!(validate_machine(&oversized).is_err());
        }

        /// Launcher readiness rejects a path that is not the KVM character device.
        #[test]
        fn invariant_launcher_rejects_non_kvm_device() {
            let file = tempfile::NamedTempFile::new().unwrap();

            assert!(verify_kvm_device(file.path()).is_err());
        }

        /// Fresh boot cannot release an action before guest PID 1 publishes readiness.
        #[test]
        fn invariant_fresh_boot_waits_for_guest_listener() {
            let log = tempfile::NamedTempFile::new().unwrap();
            let path = log.path().to_owned();
            let writer = thread::spawn(move || {
                thread::sleep(Duration::from_millis(20));
                fs::write(path, READY_MARKER).unwrap();
            });
            let mut supervisor = Command::new("sleep").arg("60").spawn().unwrap();

            let result = wait_for_guest_ready(&mut supervisor, log.path(), Duration::from_secs(1));

            writer.join().unwrap();
            supervisor.kill().unwrap();
            supervisor.wait().unwrap();
            assert!(result.is_ok());
        }

        /// A restored transport reset cannot consume the one action-release byte.
        #[test]
        fn invariant_snapshot_wake_waits_for_guest_readiness() {
            let directory = tempfile::tempdir().unwrap();
            let socket = directory.path().join("vsock.socket");
            let listener = UnixListener::bind(&socket).unwrap();
            let server = thread::spawn(move || {
                for attempt in 0..2 {
                    let (mut stream, _) = listener.accept().unwrap();
                    let mut request = Vec::new();
                    loop {
                        let mut byte = [0u8; 1];
                        stream.read_exact(&mut byte).unwrap();
                        request.push(byte[0]);
                        if byte[0] == b'\n' {
                            break;
                        }
                    }
                    assert_eq!(request, format!("CONNECT {WAKE_PORT}\n").as_bytes());
                    stream.write_all(b"OK 1\n").unwrap();
                    if attempt == 0 {
                        continue;
                    }
                    stream.write_all(&[GUEST_READY_BYTE]).unwrap();
                    let mut wake = [0u8; 1];
                    stream.read_exact(&mut wake).unwrap();
                    assert_eq!(wake, [WAKE_BYTE]);
                }
            });

            wake_guest(&socket).unwrap();
            server.join().unwrap();
        }

        #[test]
        /// The action nonce grammar cannot introduce a jail path component.
        fn action_id_is_one_lowercase_uuid() {
            assert!(valid_action_id("01234567-89ab-cdef-0123-456789abcdef"));
            assert!(!valid_action_id("../234567-89ab-cdef-0123-456789abcdef"));
            assert!(!valid_action_id("01234567-89AB-CDEF-0123-456789ABCDEF"));
        }

        /// Transport digests have exactly one canonical wire representation.
        #[test]
        fn transport_digest_is_lowercase_sha256() {
            assert!(valid_sha256(&"a".repeat(64)));
            assert!(!valid_sha256(&"A".repeat(64)));
            assert!(!valid_sha256(&"a".repeat(63)));
            assert!(!valid_sha256(&"g".repeat(64)));
        }

        /// Missing cgroup state is reported only after the process and jail are removed.
        #[test]
        fn cleanup_continues_after_missing_cgroup() {
            let directory = tempfile::tempdir().unwrap();
            let action_root = directory.path().join("firecracker/action");
            let root = action_root.join("root");
            std::fs::create_dir_all(&root).unwrap();
            let child = Command::new("sleep").arg("60").spawn().unwrap();
            let pid = Pid::from_raw(child.id() as i32);
            let mut jail = Jail {
                root,
                cgroup: directory.path().join("missing-cgroup"),
                pid: Some(pid),
                supervisor: Some(child),
            };

            assert!(cleanup_jail(&mut jail).is_err());
            assert!(matches!(kill(pid, None), Err(Errno::ESRCH)));
            assert!(!action_root.exists());
        }

        /// Dropping an owned jail cannot leave its supervisor or filesystem behind.
        #[test]
        fn jail_drop_is_a_cleanup_backstop() {
            let directory = tempfile::tempdir().unwrap();
            let action_root = directory.path().join("firecracker/action");
            let root = action_root.join("root");
            std::fs::create_dir_all(&root).unwrap();
            let child = Command::new("sleep").arg("60").spawn().unwrap();
            let pid = Pid::from_raw(child.id() as i32);

            drop(Jail {
                root,
                cgroup: directory.path().join("missing-cgroup"),
                pid: None,
                supervisor: Some(child),
            });

            assert!(matches!(kill(pid, None), Err(Errno::ESRCH)));
            assert!(!action_root.exists());
        }

        /// Descriptor copies reject both truncation and bytes beyond the admitted length.
        #[test]
        fn transport_copy_is_exact() {
            use std::io::Seek;
            use std::io::SeekFrom;
            use std::io::Write;

            for (bytes, expected) in [(b"short".as_slice(), 6), (b"extra".as_slice(), 4)] {
                let mut source = tempfile::tempfile().unwrap();
                source.write_all(bytes).unwrap();
                source.seek(SeekFrom::Start(0)).unwrap();
                let mut destination = tempfile::tempfile().unwrap();

                assert!(
                    copy_exact_transport(
                        &mut source,
                        &mut destination,
                        expected,
                        Path::new("transport")
                    )
                    .is_err()
                );
            }
        }

        /// Same-size descriptor mutation cannot cross the privileged boundary.
        #[test]
        fn transport_copy_authenticates_content() {
            use std::io::Seek;
            use std::io::SeekFrom;
            use std::io::Write;

            let directory = tempfile::tempdir().unwrap();
            let destination = directory.path().join("destination");
            let mut source = tempfile::tempfile().unwrap();
            source.write_all(b"evil").unwrap();
            source.seek(SeekFrom::Start(0)).unwrap();
            let expected = format!("{:x}", Sha256::digest(b"good"));

            assert!(
                copy_transport(
                    &mut source,
                    &destination,
                    TransportSpec {
                        index: 0,
                        bytes: 4,
                        capacity: 4,
                        sha256: &expected,
                    },
                    0,
                    0,
                )
                .is_err()
            );
        }

        /// Immutable VM artifacts are materialized without copying their data.
        #[test]
        fn immutable_artifacts_are_hardlinked() {
            use std::os::unix::fs::MetadataExt;

            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join("source");
            let destination = directory.path().join("destination");
            std::fs::write(&source, b"immutable").unwrap();

            link_immutable(&source, &destination).unwrap();

            let source = std::fs::metadata(source).unwrap();
            let destination = std::fs::metadata(destination).unwrap();
            assert_eq!(source.dev(), destination.dev());
            assert_eq!(source.ino(), destination.ino());
            assert_eq!(source.nlink(), 2);
        }

        /// A digest name accepts equal content and rejects a collision.
        #[test]
        fn cas_object_collision_is_rejected() {
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join("source");
            let equivalent = directory.path().join("equivalent");
            let collision = directory.path().join("collision");
            std::fs::write(&source, b"same bytes").unwrap();
            std::fs::write(&equivalent, b"same bytes").unwrap();
            std::fs::write(&collision, b"different bytes").unwrap();
            let digest = format!("{:x}", Sha256::digest(b"same bytes"));

            assert!(verify_cas_object(&source, &equivalent, &digest).is_ok());
            assert!(verify_cas_object(&source, &collision, &digest).is_err());
            let linked = directory.path().join("linked");
            std::fs::hard_link(&source, &linked).unwrap();
            assert!(verify_cas_object(&source, &linked, &digest).is_ok());
        }

        /// Firecracker appends a normal exit log after its canonical version line.
        #[test]
        fn release_version_is_the_exact_first_line() {
            let output = b"Firecracker v1.16.1\n\nexit log\n";

            assert_eq!(reported_version(output), Some("Firecracker v1.16.1"));
            assert_eq!(reported_version(b"\xff"), None);
        }
    }
}

#[cfg(target_os = "linux")]
/// Starts the Linux launcher and reports typed startup failures.
fn main() {
    if let Err(error) = linux::main() {
        eprintln!("bsmr-sandboxd: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(target_os = "linux"))]
/// Fails loudly when the launcher is invoked on an unsupported host.
fn main() {
    eprintln!("bsmr-sandboxd requires Linux");
    std::process::exit(1);
}
