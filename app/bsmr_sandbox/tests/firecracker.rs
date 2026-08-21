//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies the launcher, jailer, VMM, guest, and cleanup boundary on KVM.

#![cfg(target_os = "linux")]

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs::File;
use std::io::IoSlice;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use bsmr_sandbox::BundleTrust;
use bsmr_sandbox::GuestAction;
use bsmr_sandbox::GuestOutput;
use bsmr_sandbox::GuestResultEnvelope;
use bsmr_sandbox::LauncherRequest;
use bsmr_sandbox::LauncherResponse;
use bsmr_sandbox::LauncherStatus;
use bsmr_sandbox::MAX_OUTPUT_BYTES;
use bsmr_sandbox::MEMORY_MIB;
use bsmr_sandbox::PROTOCOL_VERSION;
use bsmr_sandbox::VCPU_COUNT;
use bsmr_sandbox::VerifiedBundle;
use nix::sys::socket::ControlMessage;
use nix::sys::socket::MsgFlags;
use nix::sys::socket::sendmsg;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

const ACTION_BYTES: u64 = 64 * 1024;
const BENCHMARK_SAMPLES: usize = 30;

/// Executes the conformance corpus only in the mandatory nested-KVM CI lane.
#[test]
#[ignore = "requires root-installed Firecracker bundle and KVM launcher"]
fn firecracker_conformance() {
    let bundle_path = required_path("BSMR_SANDBOX_BUNDLE");
    let socket = required_path("BSMR_SANDBOX_SOCKET");
    let probe = required_path("BSMR_SANDBOX_PROBE");
    let bundle =
        VerifiedBundle::load(&bundle_path, std::env::consts::ARCH, BundleTrust::Content).unwrap();

    let completed = execute(&socket, &bundle, &probe, &[], Some(5_000), false);
    assert_eq!(
        completed.response.status,
        LauncherStatus::Completed,
        "{:#?}",
        completed.response
    );
    assert!(completed.response.cleanup_complete);
    let archive = read_archive(completed.output);
    let result: GuestResultEnvelope =
        serde_json::from_slice(&archive.files[".bsmr/result.json"]).unwrap();
    assert_eq!(result.exit_code, 0);
    assert!(!result.timed_out);
    assert_eq!(archive.files[".bsmr/stdout"], b"sandbox probe passed\n");
    assert_eq!(archive.files[".bsmr/stderr"], b"sandbox probe stderr\n");
    assert_eq!(archive.files["outputs/out/result"], b"isolated\n");
    assert_eq!(archive.files["outputs/out/unicode-\u{2603}"], b"snow\n");
    assert_eq!(archive.modes["outputs/out/executable"] & 0o111, 0o111);
    assert!(archive.directories.contains("outputs/out/empty"));
    assert_eq!(archive.symlinks["outputs/out/link"], Path::new("result"));
    assert!(!archive.files.contains_key("outputs/undeclared"));

    let repeated = execute(&socket, &bundle, &probe, &[], Some(5_000), false);
    assert_eq!(repeated.response.status, LauncherStatus::Completed);
    assert_eq!(read_archive(repeated.output), archive);

    let first_random = execute(&socket, &bundle, &probe, &["random"], Some(5_000), false);
    let second_random = execute(&socket, &bundle, &probe, &["random"], Some(5_000), false);
    assert_eq!(first_random.response.status, LauncherStatus::Completed);
    assert_eq!(second_random.response.status, LauncherStatus::Completed);
    let first_random = read_archive(first_random.output);
    let second_random = read_archive(second_random.output);
    assert_ne!(
        first_random.files["outputs/out/random"],
        second_random.files["outputs/out/random"]
    );

    std::thread::scope(|scope| {
        let first = scope.spawn(|| execute(&socket, &bundle, &probe, &[], Some(5_000), false));
        let second = scope.spawn(|| execute(&socket, &bundle, &probe, &[], Some(5_000), false));
        for execution in [first.join().unwrap(), second.join().unwrap()] {
            assert_eq!(execution.response.status, LauncherStatus::Completed);
            assert_eq!(read_archive(execution.output), archive);
        }
    });

    let failed = execute(&socket, &bundle, &probe, &["exit7"], Some(5_000), false);
    assert_eq!(failed.response.status, LauncherStatus::Completed);
    let failed = read_archive(failed.output);
    let result: GuestResultEnvelope =
        serde_json::from_slice(&failed.files[".bsmr/result.json"]).unwrap();
    assert_eq!(result.exit_code, 7);
    assert_eq!(failed.files[".bsmr/stderr"], b"requested failure\n");

    let timed_out = execute(&socket, &bundle, &probe, &["hang"], Some(250), false);
    assert_eq!(timed_out.response.status, LauncherStatus::Completed);
    let result: GuestResultEnvelope =
        serde_json::from_slice(&read_archive(timed_out.output).files[".bsmr/result.json"]).unwrap();
    assert!(result.timed_out);

    let cancelled = execute(&socket, &bundle, &probe, &["hang"], None, true);
    assert_eq!(cancelled.response.status, LauncherStatus::Cancelled);
    assert!(cancelled.response.cleanup_complete);
}

/// Measures Firecracker environment start independently from action execution.
#[test]
#[ignore = "requires root-installed Firecracker bundle and KVM launcher"]
fn firecracker_startup_benchmark() {
    let bundle_path = required_path("BSMR_SANDBOX_BUNDLE");
    let socket = required_path("BSMR_SANDBOX_SOCKET");
    let probe = required_path("BSMR_SANDBOX_PROBE");
    let output = required_path("BSMR_SANDBOX_BENCHMARK_OUT");
    let mode = std::env::var("BSMR_SANDBOX_MODE").expect("BSMR_SANDBOX_MODE must be set");
    let bundle =
        VerifiedBundle::load(&bundle_path, std::env::consts::ARCH, BundleTrust::Content).unwrap();
    let mut environment_samples = Vec::with_capacity(BENCHMARK_SAMPLES);
    let mut roundtrip_samples = Vec::with_capacity(BENCHMARK_SAMPLES);
    for _ in 0..BENCHMARK_SAMPLES {
        let roundtrip = Instant::now();
        let execution = execute(&socket, &bundle, &probe, &[], Some(5_000), false);
        let roundtrip_us = u64::try_from(roundtrip.elapsed().as_micros()).unwrap();
        assert_eq!(execution.response.status, LauncherStatus::Completed);
        environment_samples.push(
            execution
                .response
                .environment_start_us
                .expect("successful execution must report environment start"),
        );
        roundtrip_samples.push(roundtrip_us);
    }
    let benchmark = StartupBenchmark::from_samples(
        mode,
        bundle.environment_digest().to_owned(),
        bsmr_sandbox::host_fingerprint().unwrap(),
        environment_samples,
        roundtrip_samples,
    );
    std::fs::write(&output, serde_json::to_vec_pretty(&benchmark).unwrap()).unwrap();
    eprintln!("{benchmark:#?}");
}

/// Enforces the accepted snapshot speedup against the fresh-boot oracle.
#[test]
#[ignore = "requires benchmark artifacts from both KVM launch modes"]
fn firecracker_snapshot_speedup() {
    let fresh: StartupBenchmark = serde_json::from_slice(
        &std::fs::read(required_path("BSMR_SANDBOX_FRESH_BENCHMARK")).unwrap(),
    )
    .unwrap();
    let snapshot: StartupBenchmark = serde_json::from_slice(
        &std::fs::read(required_path("BSMR_SANDBOX_SNAPSHOT_BENCHMARK")).unwrap(),
    )
    .unwrap();
    assert_eq!(fresh.mode, "fresh");
    assert_eq!(snapshot.mode, "snapshot");
    assert_eq!(fresh.environment_digest, snapshot.environment_digest);
    assert_eq!(fresh.host_fingerprint, snapshot.host_fingerprint);
    assert_eq!(fresh.environment_samples_us.len(), BENCHMARK_SAMPLES);
    assert_eq!(snapshot.environment_samples_us.len(), BENCHMARK_SAMPLES);
    assert_eq!(fresh.roundtrip_samples_us.len(), BENCHMARK_SAMPLES);
    assert_eq!(snapshot.roundtrip_samples_us.len(), BENCHMARK_SAMPLES);
    assert!(
        snapshot.environment_p50_us.saturating_mul(4) <= fresh.environment_p50_us,
        "snapshot p50 {} us is not 4x faster than fresh p50 {} us",
        snapshot.environment_p50_us,
        fresh.environment_p50_us
    );
    assert!(
        snapshot.environment_p95_us.saturating_mul(4) <= fresh.environment_p95_us,
        "snapshot p95 {} us is not 4x faster than fresh p95 {} us",
        snapshot.environment_p95_us,
        fresh.environment_p95_us
    );
}

#[derive(Debug, Deserialize, Serialize)]
struct StartupBenchmark {
    mode: String,
    environment_digest: String,
    host_fingerprint: String,
    environment_samples_us: Vec<u64>,
    environment_p50_us: u64,
    environment_p95_us: u64,
    environment_p99_us: u64,
    roundtrip_samples_us: Vec<u64>,
    roundtrip_p50_us: u64,
    roundtrip_p95_us: u64,
    roundtrip_p99_us: u64,
}

impl StartupBenchmark {
    /// Sorts the complete sample set and reports nearest-rank percentiles.
    fn from_samples(
        mode: String,
        environment_digest: String,
        host_fingerprint: String,
        environment_samples_us: Vec<u64>,
        roundtrip_samples_us: Vec<u64>,
    ) -> Self {
        assert_eq!(environment_samples_us.len(), BENCHMARK_SAMPLES);
        assert_eq!(roundtrip_samples_us.len(), BENCHMARK_SAMPLES);
        let mut environment = environment_samples_us.clone();
        let mut roundtrip = roundtrip_samples_us.clone();
        environment.sort_unstable();
        roundtrip.sort_unstable();
        Self {
            mode,
            environment_digest,
            host_fingerprint,
            environment_samples_us,
            environment_p50_us: percentile(&environment, 50),
            environment_p95_us: percentile(&environment, 95),
            environment_p99_us: percentile(&environment, 99),
            roundtrip_samples_us,
            roundtrip_p50_us: percentile(&roundtrip, 50),
            roundtrip_p95_us: percentile(&roundtrip, 95),
            roundtrip_p99_us: percentile(&roundtrip, 99),
        }
    }
}

/// Returns one nearest-rank percentile from a non-empty sorted sample.
fn percentile(samples: &[u64], percentile: usize) -> u64 {
    assert!(!samples.is_empty());
    assert!((1..=100).contains(&percentile));
    let rank = samples.len().saturating_mul(percentile).div_ceil(100);
    samples[rank.saturating_sub(1)]
}

/// Nearest-rank selection remains stable for the mandatory sample count.
#[test]
fn startup_percentiles_are_exact() {
    let samples = (1..=BENCHMARK_SAMPLES as u64).collect::<Vec<_>>();
    assert_eq!(percentile(&samples, 50), 15);
    assert_eq!(percentile(&samples, 95), 29);
    assert_eq!(percentile(&samples, 99), 30);
}

struct Execution {
    response: LauncherResponse,
    output: File,
}

#[derive(Debug, Eq, PartialEq)]
struct ArchiveContents {
    files: BTreeMap<String, Vec<u8>>,
    directories: BTreeSet<String>,
    symlinks: BTreeMap<String, PathBuf>,
    modes: BTreeMap<String, u32>,
}

/// Sends one exact action to the launcher and optionally disconnects its write half.
fn execute(
    socket: &Path,
    bundle: &VerifiedBundle,
    probe: &Path,
    arguments: &[&str],
    timeout_ms: Option<u64>,
    cancel: bool,
) -> Execution {
    let mut command = vec!["./probe".to_owned()];
    command.extend(arguments.iter().map(ToString::to_string));
    let action = GuestAction {
        protocol: PROTOCOL_VERSION,
        arguments: command,
        environment: BTreeMap::from([("BSMR_PROBE".to_owned(), "exact".to_owned())]),
        working_directory: PathBuf::new(),
        outputs: vec![GuestOutput::directory("out")],
        timeout_ms,
    };
    let mut action_file = tempfile::tempfile().unwrap();
    let action_bytes = serde_json::to_vec(&action).unwrap();
    action_file
        .write_all(&(action_bytes.len() as u32).to_be_bytes())
        .unwrap();
    action_file.write_all(&action_bytes).unwrap();
    action_file.set_len(ACTION_BYTES).unwrap();
    action_file.seek(SeekFrom::Start(0)).unwrap();
    let mut input = tempfile::tempfile().unwrap();
    write_input(&mut input, probe);
    let input_bytes = input.metadata().unwrap().len();
    input.seek(SeekFrom::Start(0)).unwrap();
    let action_sha256 = transport_sha256(&mut action_file);
    let input_sha256 = transport_sha256(&mut input);
    let mut output = tempfile::tempfile().unwrap();
    output.set_len(MAX_OUTPUT_BYTES).unwrap();
    let request = LauncherRequest {
        protocol: PROTOCOL_VERSION,
        action_id: uuid::Uuid::new_v4().to_string(),
        environment_digest: bundle.environment_digest().to_owned(),
        input_bytes,
        input_sha256,
        output_bytes: MAX_OUTPUT_BYTES,
        action_sha256,
        vcpu_count: VCPU_COUNT,
        memory_mib: MEMORY_MIB,
        timeout_ms,
    };
    let mut stream = connect_socket(socket);
    send_request(&mut stream, &request, [&action_file, &input, &output]);
    if cancel {
        std::thread::sleep(Duration::from_millis(100));
        stream.shutdown(std::net::Shutdown::Write).unwrap();
    }
    let response = read_response(&mut stream);
    output.seek(SeekFrom::Start(0)).unwrap();
    Execution { response, output }
}

/// Authenticates a KVM-test transport and rewinds it for descriptor passing.
fn transport_sha256(file: &mut File) -> String {
    file.seek(SeekFrom::Start(0)).unwrap();
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let read = file.read(&mut buffer).unwrap();
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    file.seek(SeekFrom::Start(0)).unwrap();
    format!("{:x}", hasher.finalize())
}

/// Waits only for the configured launcher socket to accept its first connection.
fn connect_socket(path: &Path) -> UnixStream {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match UnixStream::connect(path) {
            Ok(stream) => return stream,
            Err(_) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("launcher socket {path:?} did not become ready: {error}"),
        }
    }
}

/// Creates the deterministic read-only input tar consumed by the guest PID 1.
fn write_input(output: &mut File, probe: &Path) {
    let bytes = std::fs::read(probe).unwrap();
    let mut archive = tar::Builder::new(output);
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Regular);
    header.set_size(bytes.len() as u64);
    header.set_mode(0o755);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    header.set_cksum();
    archive
        .append_data(&mut header, "probe", &bytes[..])
        .unwrap();
    archive.finish().unwrap();
}

/// Transfers one framed request and its three transport descriptors.
fn send_request(stream: &mut UnixStream, request: &LauncherRequest, files: [&File; 3]) {
    let payload = serde_json::to_vec(request).unwrap();
    let mut frame = Vec::with_capacity(payload.len() + 4);
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(&payload);
    let descriptors = files.map(AsRawFd::as_raw_fd);
    let written = sendmsg::<()>(
        stream.as_raw_fd(),
        &[IoSlice::new(&frame)],
        &[ControlMessage::ScmRights(&descriptors)],
        MsgFlags::empty(),
        None,
    )
    .unwrap();
    stream.write_all(&frame[written..]).unwrap();
}

/// Reads one bounded launcher response frame.
fn read_response(stream: &mut UnixStream) -> LauncherResponse {
    let mut size = [0u8; 4];
    stream.read_exact(&mut size).unwrap();
    let size = u32::from_be_bytes(size) as usize;
    assert!(size <= 64 * 1024);
    let mut payload = vec![0; size];
    stream.read_exact(&mut payload).unwrap();
    serde_json::from_slice(&payload).unwrap()
}

/// Reads the complete metadata and contents of one trusted conformance archive.
fn read_archive(mut file: File) -> ArchiveContents {
    file.seek(SeekFrom::Start(0)).unwrap();
    let mut files = BTreeMap::new();
    let mut directories = BTreeSet::new();
    let mut symlinks = BTreeMap::new();
    let mut modes = BTreeMap::new();
    for entry in tar::Archive::new(file).entries().unwrap() {
        let mut entry = entry.unwrap();
        let path = entry.path().unwrap().to_string_lossy().into_owned();
        modes.insert(path.clone(), entry.header().mode().unwrap());
        if entry.header().entry_type().is_file() {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            files.insert(path, bytes);
        } else if entry.header().entry_type().is_dir() {
            directories.insert(path);
        } else if entry.header().entry_type().is_symlink() {
            symlinks.insert(path, entry.link_name().unwrap().unwrap().into_owned());
        }
    }
    ArchiveContents {
        files,
        directories,
        symlinks,
        modes,
    }
}

/// Reads one required CI path and fails loudly outside the configured KVM lane.
fn required_path(name: &str) -> PathBuf {
    std::env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{name} must be set by the Firecracker conformance lane"))
}
