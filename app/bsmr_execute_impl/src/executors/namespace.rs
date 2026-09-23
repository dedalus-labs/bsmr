//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Stages declared actions for execution in a verified Linux namespace runtime.

pub(crate) mod runtime;

use std::ffi::OsString;
use std::fs;
use std::io::Seek;
use std::path::Path;
use std::path::PathBuf;

use bsmr_core::fs::artifact_path_resolver::ArtifactFs;
use bsmr_directory::directory::directory::Directory;
use bsmr_directory::directory::directory_iterator::DirectoryIterator;
use bsmr_directory::directory::entry::DirectoryEntry;
use bsmr_execute::digest_config::DigestConfig;
use bsmr_execute::directory::ActionDirectoryMember;
use bsmr_execute::execute::prepared::PreparedAction;
use bsmr_execute::execute::request::CommandExecutionRequest;
use bsmr_sandbox::GuestOutput;
use remote_execution as RE;

use self::runtime::Runtime;
use super::firecracker;

/// Owns the verified runtime shared by one command's isolated local actions.
pub struct NamespaceExecutor {
    runtime: Runtime,
}

/// Holds private action trees until execution, descendant cleanup and output import finish.
pub(crate) struct NamespaceAction {
    directory: tempfile::TempDir,
    outputs: Vec<GuestOutput>,
    pub(crate) arguments: Vec<OsString>,
}

#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Input)]
enum NamespaceError {
    #[error("namespace execution requires a supported Linux launcher, host is {0}/{1}")]
    UnsupportedHost(&'static str, &'static str),
    #[error("namespace writable path {parent:?} overlaps declared input {input:?}")]
    InputOutputOverlap { parent: PathBuf, input: PathBuf },
    #[error("namespace input symlink shares a writable output parent: {0:?}")]
    WritableSymlink(PathBuf),
}

impl NamespaceExecutor {
    /// Load a runtime whose launcher matches the independent platform catalog.
    pub fn new(manifest: &Path) -> bsmr_error::Result<Self> {
        // Ubuntu bubblewrap 0.9.0-1ubuntu0.3, independently pinned before reading project data.
        let launcher = match (std::env::consts::OS, std::env::consts::ARCH) {
            ("linux", "aarch64") => {
                "cd6b143283b464baf078ab09ca19dadc5bcc2423b833c4f9dd7e83f9ba1b640b"
            }
            ("linux", "x86_64") => {
                "e318903862396f96de3df57264e0158682b952fd3fb53ac23d876413e7b30f71"
            }
            (os, arch) => return Err(NamespaceError::UnsupportedHost(os, arch).into()),
        };
        Ok(Self {
            runtime: Runtime::load(manifest, launcher)?,
        })
    }

    /// Bind runtime bytes and the execution policy into the action identity before cache lookup.
    pub fn platform(&self) -> RE::Platform {
        RE::Platform {
            properties: [
                ("bsmr.sandbox.backend", "namespace"),
                ("bsmr.sandbox.environment", self.runtime.digest()),
                ("bsmr.sandbox.profile", "declared-inputs-v2"),
            ]
            .into_iter()
            .map(|(name, value)| RE::Property {
                name: name.into(),
                value: value.into(),
            })
            .collect(),
        }
    }

    /// Return the launcher copied and verified before any project command is prepared.
    pub(crate) fn launcher(&self) -> PathBuf {
        self.runtime.launcher()
    }

    /// Snapshot analyzed inputs and construct mounts without exposing the host project tree.
    pub(crate) fn prepare(
        &self,
        prepared: &PreparedAction,
        request: &CommandExecutionRequest,
        artifact_fs: &ArtifactFs,
        digest_config: DigestConfig,
    ) -> bsmr_error::Result<NamespaceAction> {
        firecracker::validate_action_policy(prepared, request)?;
        let project = artifact_fs.fs().root().as_path();
        let command = firecracker::decode_re_command(prepared, digest_config)?;
        let action = firecracker::sandbox_action(&command, request)?;
        let staging = artifact_fs
            .fs()
            .resolve(artifact_fs.output_path_resolver().root());
        fs::create_dir_all(&staging)?;
        let directory = tempfile::Builder::new()
            .prefix(".bsmr-namespace-")
            .tempdir_in(staging)?;
        let inputs = directory.path().join("inputs");
        let outputs = directory.path().join("outputs");
        fs::create_dir(&inputs)?;
        fs::create_dir(&outputs)?;
        let mut archive = tempfile::tempfile()?;
        firecracker::write_input_archive(
            &mut archive,
            project,
            request.paths().input_directory(),
            digest_config,
        )?;
        archive.rewind()?;
        tar::Archive::new(archive).unpack(&inputs)?;

        let mut parents = Vec::<PathBuf>::new();
        let scratch = action.environment.get("BSMR_SCRATCH_PATH").map(Path::new);
        if let Some(scratch) = scratch {
            firecracker::validate_guest_path(scratch)?;
        }
        for parent in action
            .outputs
            .iter()
            .map(|output| output.path.parent().expect("validated output has a parent"))
            .chain(scratch)
        {
            if !parents.iter().any(|root| parent.starts_with(root)) {
                parents.retain(|root| !root.starts_with(parent));
                parents.push(parent.to_owned());
            }
        }
        let mut readonly_roots = Vec::<PathBuf>::new();
        for (input, entry) in request
            .paths()
            .input_directory()
            .ordered_walk()
            .with_paths()
        {
            let input = Path::new(input.as_str());
            let leaf = matches!(entry, DirectoryEntry::Leaf(_));
            for parent in parents
                .iter()
                .chain(action.outputs.iter().map(|output| &output.path))
            {
                if leaf && parent.starts_with(input) {
                    return Err(NamespaceError::InputOutputOverlap {
                        parent: parent.clone(),
                        input: input.to_owned(),
                    }
                    .into());
                }
            }
            if let Some(output) = action
                .outputs
                .iter()
                .find(|output| input.starts_with(&output.path))
            {
                return Err(NamespaceError::InputOutputOverlap {
                    parent: output.path.clone(),
                    input: input.to_owned(),
                }
                .into());
            }
            let structural_directory = !leaf
                && (action
                    .outputs
                    .iter()
                    .any(|output| output.path.starts_with(input))
                    || parents.iter().any(|parent| parent.starts_with(input)));
            if structural_directory
                || !parents.iter().any(|parent| input.starts_with(parent))
                || readonly_roots
                    .last()
                    .is_some_and(|root| input.starts_with(root))
            {
                continue;
            }
            let destination = outputs.join(input);
            fs::create_dir_all(destination.parent().expect("input has a parent"))?;
            match entry {
                DirectoryEntry::Dir(_) => fs::create_dir(&destination)?,
                DirectoryEntry::Leaf(ActionDirectoryMember::File(_)) => {
                    fs::File::create(destination)?;
                }
                DirectoryEntry::Leaf(_) => {
                    return Err(NamespaceError::WritableSymlink(input.to_owned()).into());
                }
            }
            readonly_roots.push(input.to_owned());
        }

        let mut arguments = [
            "--unshare-user",
            "--unshare-net",
            "--unshare-pid",
            "--unshare-ipc",
            "--unshare-uts",
            "--hostname",
            "bsmr-action",
            "--disable-userns",
            "--die-with-parent",
            "--new-session",
            "--cap-drop",
            "ALL",
            "--uid",
            "0",
            "--gid",
            "0",
            "--clearenv",
            "--ro-bind",
        ]
        .map(OsString::from)
        .to_vec();
        arguments.extend([
            self.runtime.root().into(),
            "/".into(),
            "--ro-bind".into(),
            inputs.clone().into(),
            "/workspace".into(),
        ]);
        for parent in parents {
            fs::create_dir_all(inputs.join(&parent))?;
            fs::create_dir_all(outputs.join(&parent))?;
            arguments.extend([
                "--bind".into(),
                outputs.join(&parent).into(),
                Path::new("/workspace").join(parent).into(),
            ]);
        }
        for input in readonly_roots {
            arguments.extend([
                "--ro-bind".into(),
                inputs.join(&input).into(),
                Path::new("/workspace").join(input).into(),
            ]);
        }
        // The linker resolves its executable through procfs in this private PID namespace.
        arguments.extend(
            [
                "--tmpfs",
                "/tmp",
                "--dev",
                "/dev",
                "--proc",
                "/proc",
                "--remount-ro",
                "/proc",
            ]
            .map(OsString::from),
        );
        for (name, value) in [
            ("PATH", "/usr/bin:/bin"),
            ("HOME", "/tmp"),
            ("TMPDIR", "/tmp"),
            ("BSMR_SCRATCH_PATH", "/tmp"),
        ] {
            arguments.extend(["--setenv".into(), name.into(), value.into()]);
        }
        for (name, value) in action.environment {
            arguments.extend(["--setenv".into(), name.into(), value.into()]);
        }
        let mut cwd = PathBuf::from("/workspace");
        cwd.extend(action.working_directory.components());
        arguments.extend([
            "--setenv".into(),
            "PWD".into(),
            cwd.clone().into(),
            "--chdir".into(),
            cwd.into(),
            "--".into(),
        ]);
        arguments.extend(action.arguments.into_iter().map(OsString::from));
        Ok(NamespaceAction {
            directory,
            outputs: action.outputs,
            arguments,
        })
    }
}

impl NamespaceAction {
    /// Import only validated declared outputs after the namespace's complete process tree exits.
    pub(crate) fn import_outputs(&self, project: &Path) -> bsmr_error::Result<()> {
        let outputs = self.directory.path().join("outputs");
        firecracker::validate_local_outputs(&outputs, &self.outputs)?;
        firecracker::import_outputs(&outputs, project, &self.outputs)
    }
}
