//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Runs one declared action as PID 1 inside a Firecracker microVM.

#[cfg(target_os = "linux")]
mod linux {
    use std::fs;
    use std::fs::File;
    use std::io::Read;
    use std::io::Write;
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;
    use std::os::unix::process::ExitStatusExt;
    use std::path::Component;
    use std::path::Path;
    use std::path::PathBuf;
    use std::process::Command;
    use std::process::ExitStatus;
    use std::process::Stdio;
    use std::time::Duration;
    use std::time::Instant;

    use bsmr_sandbox::GuestAction;
    use bsmr_sandbox::GuestResultEnvelope;
    use bsmr_sandbox::MAX_ACTION_BYTES;
    use bsmr_sandbox::MAX_STREAM_BYTES;
    use bsmr_sandbox::MAX_TIMEOUT_MS;
    use bsmr_sandbox::PROTOCOL_VERSION;
    use bsmr_sandbox::snapshot::GUEST_READY_BYTE;
    use bsmr_sandbox::snapshot::READY_MARKER;
    use bsmr_sandbox::snapshot::WAKE_BYTE;
    use bsmr_sandbox::snapshot::WAKE_PORT;
    use nix::errno::Errno;
    use nix::mount::MsFlags;
    use nix::mount::mount;
    use nix::sys::reboot::RebootMode;
    use nix::sys::reboot::reboot;
    use nix::sys::signal::Signal;
    use nix::sys::signal::kill;
    use nix::sys::socket::AddressFamily;
    use nix::sys::socket::Backlog;
    use nix::sys::socket::SockFlag;
    use nix::sys::socket::SockType;
    use nix::sys::socket::VsockAddr;
    use nix::sys::socket::bind;
    use nix::sys::socket::listen;
    use nix::sys::socket::socket;
    use nix::sys::wait::WaitStatus;
    use nix::sys::wait::waitpid;
    use nix::unistd::Gid;
    use nix::unistd::Pid;
    use nix::unistd::Uid;
    use nix::unistd::chown;
    use thiserror::Error;

    const WORKSPACE: &str = "/workspace";
    const INPUT_DEVICE: &str = "/dev/vdb";
    const OUTPUT_DEVICE: &str = "/dev/vdc";
    const ACTION_DEVICE: &str = "/dev/vdd";
    const ACTION_DEVICE_BYTES: usize = MAX_ACTION_BYTES as usize;

    #[derive(Debug, Error)]
    enum GuestError {
        #[error("I/O failure at {path:?}: {source}")]
        Io {
            path: PathBuf,
            #[source]
            source: std::io::Error,
        },
        #[error("invalid action request: {0}")]
        Action(#[from] serde_json::Error),
        #[error("guest protocol must be {PROTOCOL_VERSION}, got {0}")]
        Protocol(u32),
        #[error("invalid guest path: {0:?}")]
        Path(PathBuf),
        #[error("action command is empty")]
        EmptyCommand,
        #[error("action request exceeds its {ACTION_DEVICE_BYTES}-byte device")]
        ActionTooLarge,
        #[error("action timeout {0} ms exceeds {MAX_TIMEOUT_MS} ms")]
        Timeout(u64),
        #[error("guest kernel operation failed: {0}")]
        Kernel(#[from] nix::Error),
        #[error("failed to spawn action: {0}")]
        Spawn(#[source] std::io::Error),
        #[error("action stream {path:?} has {actual} bytes, limit {MAX_STREAM_BYTES}")]
        StreamTooLarge { path: PathBuf, actual: u64 },
        #[error("invalid host wake signal")]
        Wake,
    }

    /// Runs the PID 1 lifecycle and reboots through Firecracker's x86 reset exit.
    pub fn main() {
        if let Err(error) = run() {
            eprintln!("bsmr-sandbox-guest: {error}");
        }
        let Err(error) = reboot(RebootMode::RB_AUTOBOOT);
        eprintln!("bsmr-sandbox-guest: reboot failed: {error}");
        std::process::exit(1);
    }

    /// Mounts the private workspace, runs one action, and emits its result archive.
    fn run() -> Result<(), GuestError> {
        mount(
            Some("proc"),
            "/proc",
            Some("proc"),
            MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC,
            None::<&str>,
        )?;
        fs::create_dir_all(WORKSPACE).map_err(|source| io_error(WORKSPACE, source))?;
        mount(
            Some("tmpfs"),
            WORKSPACE,
            Some("tmpfs"),
            MsFlags::MS_NOSUID | MsFlags::MS_NODEV,
            Some("size=1536m,mode=0755"),
        )?;

        wait_for_host()?;

        let action = read_action()?;
        if action.protocol != PROTOCOL_VERSION {
            return Err(GuestError::Protocol(action.protocol));
        }
        validate_action(&action)?;
        let input = File::open(INPUT_DEVICE).map_err(|source| io_error(INPUT_DEVICE, source))?;
        tar::Archive::new(input)
            .unpack(WORKSPACE)
            .map_err(|source| io_error(INPUT_DEVICE, source))?;
        fs::create_dir_all(Path::new(WORKSPACE).join(".tmp"))
            .map_err(|source| io_error(Path::new(WORKSPACE).join(".tmp"), source))?;
        chown_tree(
            Path::new(WORKSPACE),
            Uid::from_raw(1000),
            Gid::from_raw(1000),
        )?;

        let stdout_path = PathBuf::from("/dev/.bsmr-stdout");
        let stderr_path = PathBuf::from("/dev/.bsmr-stderr");
        let stdout = File::create(&stdout_path).map_err(|source| io_error(&stdout_path, source))?;
        let stderr = File::create(&stderr_path).map_err(|source| io_error(&stderr_path, source))?;
        let mut arguments = action.arguments.clone();
        let working_directory = Path::new(WORKSPACE).join(&action.working_directory);
        let executable = arguments.first_mut().ok_or(GuestError::EmptyCommand)?;
        if executable.contains('/') {
            *executable = working_directory
                .join(&*executable)
                .to_string_lossy()
                .into_owned();
        }
        let mut command = Command::new(&arguments[0]);
        command
            .args(&arguments[1..])
            .current_dir(working_directory)
            .env_clear()
            .envs(&action.environment)
            .env("TMPDIR", Path::new(WORKSPACE).join(".tmp"))
            .uid(1000)
            .gid(1000)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
        let mut child = command.spawn().map_err(GuestError::Spawn)?;
        let (status, timed_out) = wait_action(&mut child, action.timeout_ms)?;
        terminate_descendants()?;
        reap_descendants()?;
        let exit_code = match (status.code(), status.signal()) {
            (Some(code), _) => code,
            (None, Some(signal)) => 128 + signal,
            (None, None) => {
                return Err(GuestError::Spawn(std::io::Error::other(
                    "missing exit status",
                )));
            }
        };

        write_result(&action, exit_code, timed_out, &stdout_path, &stderr_path)
    }

    /// Snapshots only after this listener exists, then releases one restored clone.
    fn wait_for_host() -> Result<(), GuestError> {
        let listener = socket(
            AddressFamily::Vsock,
            SockType::Stream,
            SockFlag::SOCK_CLOEXEC,
            None,
        )?;
        bind(
            listener.as_raw_fd(),
            &VsockAddr::new(nix::libc::VMADDR_CID_ANY, WAKE_PORT),
        )?;
        listen(&listener, Backlog::new(1)?)?;
        eprintln!("{READY_MARKER}");
        loop {
            let connection = rustix::net::accept_with(&listener, rustix::net::SocketFlags::CLOEXEC)
                .map_err(|source| io_error("vsock accept", source.into()))?;
            let mut connection = File::from(connection);
            if connection.write_all(&[GUEST_READY_BYTE]).is_err() {
                continue;
            }
            let mut wake = [0u8; 1];
            if connection.read_exact(&mut wake).is_err() {
                continue;
            }
            if wake != [WAKE_BYTE] {
                return Err(GuestError::Wake);
            }
            break;
        }
        reseed_kernel()
    }

    /// Mixes fresh host entropy after every restore and before untrusted code.
    fn reseed_kernel() -> Result<(), GuestError> {
        let mut entropy = [0u8; 32];
        File::open("/dev/hwrng")
            .and_then(|mut source| source.read_exact(&mut entropy))
            .map_err(|source| io_error("/dev/hwrng", source))?;
        File::options()
            .write(true)
            .open("/dev/random")
            .and_then(|mut random| random.write_all(&entropy))
            .map_err(|source| io_error("/dev/random", source))?;
        entropy.fill(0);
        Ok(())
    }

    /// Reads one bounded length-prefixed action from its read-only device.
    fn read_action() -> Result<GuestAction, GuestError> {
        let mut file =
            File::open(ACTION_DEVICE).map_err(|source| io_error(ACTION_DEVICE, source))?;
        let mut size = [0; 4];
        file.read_exact(&mut size)
            .map_err(|source| io_error(ACTION_DEVICE, source))?;
        let size = u32::from_be_bytes(size) as usize;
        if size > ACTION_DEVICE_BYTES - 4 {
            return Err(GuestError::ActionTooLarge);
        }
        let mut bytes = vec![0; size];
        file.read_exact(&mut bytes)
            .map_err(|source| io_error(ACTION_DEVICE, source))?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    /// Revalidates every security-relevant path inside the guest boundary.
    fn validate_action(action: &GuestAction) -> Result<(), GuestError> {
        if !action.working_directory.as_os_str().is_empty() {
            validate_path(&action.working_directory)?;
        }
        for output in &action.outputs {
            validate_path(&output.path)?;
        }
        if let Some(timeout) = action.timeout_ms
            && timeout > MAX_TIMEOUT_MS
        {
            return Err(GuestError::Timeout(timeout));
        }
        let executable = action.arguments.first().ok_or(GuestError::EmptyCommand)?;
        let executable = Path::new(executable);
        if executable.is_absolute()
            || executable
                .components()
                .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
        {
            return Err(GuestError::Path(executable.to_owned()));
        }
        Ok(())
    }

    /// Accepts one non-empty normalized workspace-relative path.
    fn validate_path(path: &Path) -> Result<(), GuestError> {
        if path.as_os_str().is_empty()
            || path.as_os_str().to_string_lossy().contains('\\')
            || !path
                .components()
                .all(|component| matches!(component, Component::Normal(_)))
        {
            return Err(GuestError::Path(path.to_owned()));
        }
        Ok(())
    }

    /// Writes the envelope, streams, and declared outputs to the private result device.
    fn write_result(
        action: &GuestAction,
        exit_code: i32,
        timed_out: bool,
        stdout_path: &Path,
        stderr_path: &Path,
    ) -> Result<(), GuestError> {
        let output = File::options()
            .write(true)
            .open(OUTPUT_DEVICE)
            .map_err(|source| io_error(OUTPUT_DEVICE, source))?;
        let mut archive = tar::Builder::new(output);
        append_bytes(
            &mut archive,
            ".bsmr/result.json",
            &serde_json::to_vec(&GuestResultEnvelope {
                protocol: PROTOCOL_VERSION,
                exit_code,
                timed_out,
            })?,
        )?;
        append_file(&mut archive, ".bsmr/stdout", stdout_path)?;
        append_file(&mut archive, ".bsmr/stderr", stderr_path)?;
        archive.follow_symlinks(false);
        for output in &action.outputs {
            let source = Path::new(WORKSPACE).join(&output.path);
            let metadata = match fs::symlink_metadata(&source) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(io_error(&source, error)),
            };
            let destination = Path::new("outputs").join(&output.path);
            if metadata.file_type().is_dir() {
                archive
                    .append_dir_all(destination, source)
                    .map_err(|source| io_error(OUTPUT_DEVICE, source))?;
            } else {
                archive
                    .append_path_with_name(source, destination)
                    .map_err(|source| io_error(OUTPUT_DEVICE, source))?;
            }
        }
        archive
            .finish()
            .map_err(|source| io_error(OUTPUT_DEVICE, source))?;
        archive
            .into_inner()
            .map_err(|source| io_error(OUTPUT_DEVICE, source))?
            .sync_all()
            .map_err(|source| io_error(OUTPUT_DEVICE, source))?;
        Ok(())
    }

    /// Waits for the action or kills every guest descendant at its deadline.
    fn wait_action(
        child: &mut std::process::Child,
        timeout_ms: Option<u64>,
    ) -> Result<(ExitStatus, bool), GuestError> {
        let Some(timeout_ms) = timeout_ms else {
            return child
                .wait()
                .map(|status| (status, false))
                .map_err(GuestError::Spawn);
        };
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        loop {
            if let Some(status) = child.try_wait().map_err(GuestError::Spawn)? {
                return Ok((status, false));
            }
            if Instant::now() >= deadline {
                terminate_descendants()?;
                return child
                    .wait()
                    .map(|status| (status, true))
                    .map_err(GuestError::Spawn);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Streams one bounded root-owned result file into the output archive.
    fn append_file(
        archive: &mut tar::Builder<File>,
        destination: &str,
        source: &Path,
    ) -> Result<(), GuestError> {
        let size = fs::metadata(source)
            .map_err(|error| io_error(source, error))?
            .len();
        if size > MAX_STREAM_BYTES {
            return Err(GuestError::StreamTooLarge {
                path: source.to_owned(),
                actual: size,
            });
        }
        let file = File::open(source).map_err(|error| io_error(source, error))?;
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Regular);
        header.set_size(size);
        header.set_mode(0o644);
        header.set_mtime(0);
        header.set_uid(0);
        header.set_gid(0);
        header.set_cksum();
        archive
            .append_data(&mut header, destination, file)
            .map_err(|error| io_error(OUTPUT_DEVICE, error))?;
        Ok(())
    }

    /// Appends one in-memory protocol record with deterministic metadata.
    fn append_bytes(
        archive: &mut tar::Builder<File>,
        destination: &str,
        bytes: &[u8],
    ) -> Result<(), GuestError> {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Regular);
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(0);
        header.set_uid(0);
        header.set_gid(0);
        header.set_cksum();
        archive
            .append_data(&mut header, destination, bytes)
            .map_err(|source| io_error(OUTPUT_DEVICE, source))?;
        Ok(())
    }

    /// Gives the unprivileged action ownership of its private workspace without following links.
    fn chown_tree(path: &Path, uid: Uid, gid: Gid) -> Result<(), GuestError> {
        let metadata = fs::symlink_metadata(path).map_err(|source| io_error(path, source))?;
        if !metadata.file_type().is_symlink() {
            chown(path, Some(uid), Some(gid))?;
        }
        if metadata.is_dir() {
            for entry in fs::read_dir(path).map_err(|source| io_error(path, source))? {
                let entry = entry.map_err(|source| io_error(path, source))?;
                chown_tree(&entry.path(), uid, gid)?;
            }
        }
        Ok(())
    }

    /// Delivers an unmaskable signal to every action process still in the guest.
    fn terminate_descendants() -> Result<(), GuestError> {
        match kill(Pid::from_raw(-1), Signal::SIGKILL) {
            Ok(()) | Err(Errno::ESRCH) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// Reaps every remaining child after the guest-wide kill signal.
    fn reap_descendants() -> Result<(), GuestError> {
        loop {
            match waitpid(Pid::from_raw(-1), None) {
                Err(Errno::ECHILD) => return Ok(()),
                Err(Errno::EINTR) | Ok(WaitStatus::StillAlive) => {}
                Err(error) => return Err(error.into()),
                Ok(_) => {}
            }
        }
    }

    /// Attaches one guest path to an I/O failure.
    fn io_error(path: impl Into<PathBuf>, source: std::io::Error) -> GuestError {
        GuestError::Io {
            path: path.into(),
            source,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::GuestError;
        use super::MAX_STREAM_BYTES;
        use super::append_file;

        /// Oversized streams fail before the guest reads or archives their contents.
        #[test]
        fn result_streams_are_bounded_before_reading() {
            let source = tempfile::NamedTempFile::new().unwrap();
            source.as_file().set_len(MAX_STREAM_BYTES + 1).unwrap();
            let output = tempfile::tempfile().unwrap();
            let mut archive = tar::Builder::new(output);

            assert!(matches!(
                append_file(&mut archive, "stdout", source.path()),
                Err(GuestError::StreamTooLarge { .. })
            ));
        }
    }
}

#[cfg(target_os = "linux")]
/// Delegates the executable entry point to the Linux guest implementation.
fn main() {
    linux::main();
}

#[cfg(not(target_os = "linux"))]
/// Fails loudly when the guest binary is invoked on an unsupported host.
fn main() {
    eprintln!("bsmr-sandbox-guest requires Linux");
    std::process::exit(1);
}
