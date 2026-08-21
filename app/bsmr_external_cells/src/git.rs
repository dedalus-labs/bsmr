//===----------------------------------------------------------------------===//
// Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc
// Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::hash_map;
use std::process::Command;
use std::process::ExitStatus;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::sync::OnceLock;

use bsmr_build_api::actions::artifact::get_artifact_fs::GetArtifactFs;
use bsmr_common::dice::data::HasIoProvider;
use bsmr_common::file_ops::delegate::FileOpsDelegate;
use bsmr_common::file_ops::metadata::FileDigestConfig;
use bsmr_common::file_ops::metadata::RawDirEntry;
use bsmr_common::file_ops::metadata::RawPathMetadata;
use bsmr_common::io::IoProvider;
use bsmr_common::io::fs::FsIoProvider;
use bsmr_core::cells::cell_path::CellPath;
use bsmr_core::cells::external::ExternalCellOrigin;
use bsmr_core::cells::external::GitCellSetup;
use bsmr_core::cells::name::CellName;
use bsmr_core::cells::paths::CellRelativePath;
use bsmr_core::fs::output_path::OutputPathResolver;
use bsmr_core::fs::project_rel_path::ProjectRelativePath;
use bsmr_core::fs::project_rel_path::ProjectRelativePathBuf;
use bsmr_directory::directory::directory::Directory;
use bsmr_error::BsmrErrorContext;
use bsmr_error::internal_error;
use bsmr_execute::artifact_value::ArtifactValue;
use bsmr_execute::digest_config::HasDigestConfig;
use bsmr_execute::directory::INTERNER;
use bsmr_execute::entry::build_entry_from_disk;
use bsmr_execute::execute::blocking::HasBlockingExecutor;
use bsmr_execute::execute::blocking::IoRequest;
use bsmr_execute::execute::clean_output_paths::CleanOutputPaths;
use bsmr_execute::materialize::materializer::DeclareArtifactPayload;
use bsmr_execute::materialize::materializer::HasMaterializer;
use bsmr_execute::materialize::materializer::Materializer;
use bsmr_fs::fs_util;
use bsmr_fs::paths::abs_norm_path::AbsNormPath;
use bsmr_fs::paths::forward_rel_path::ForwardRelativePath;
use bsmr_hash::StdBsmrHashMap;
use bsmr_util::process::background_command;
use cmp_any::PartialEqAny;
use dice::CancellationContext;
use dice::DiceComputations;
use dice::Key;
use dice::OkPagableValueSerialize;
use dice::ValueSerialize;
use dupe::Dupe;
use pagable::Pagable;
use pagable::pagable_typetag;
use tokio::sync::Semaphore;

#[derive(bsmr_error::Error, Debug)]
#[bsmr(tag = Tier0)]
enum GitError {
    #[error("Error fetching external cell with git, exit code: {exit_code:?}, stderr:\n{stderr}")]
    Unsuccessful {
        exit_code: ExitStatus,
        stderr: String,
    },
    #[error("Expected git to create a directory at the checkout location")]
    NoDirectory,
}

struct GitFetchIoRequest {
    setup: GitCellSetup,
    path: ProjectRelativePathBuf,
}

impl IoRequest for GitFetchIoRequest {
    fn execute(
        self: Box<Self>,
        project_fs: &bsmr_core::fs::project::ProjectRoot,
    ) -> bsmr_error::Result<()> {
        let path = project_fs.resolve(&self.path);
        fs_util::create_dir_all(path.clone())?;

        /// Remove some `GIT_` environment variables exposed by `git`.
        ///
        /// From `prek`. MIT-licensed, Copyright (c) 2024 j178.
        ///
        /// See: <https://github.com/j178/prek/blob/7780f1149565ff430b86be1f688dce7f680c6760/crates/prek/src/git.rs#L49-L77>
        static GIT_ENV_TO_REMOVE: LazyLock<Vec<String>> = LazyLock::new(|| {
            let keep = &[
                "GIT_EXEC_PATH",
                "GIT_SSH",
                "GIT_SSH_COMMAND",
                "GIT_SSL_CAINFO",
                "GIT_SSL_NO_VERIFY",
                "GIT_CONFIG_COUNT",
                "GIT_CONFIG_PARAMETERS",
                "GIT_HTTP_PROXY_AUTHMETHOD",
                "GIT_ALLOW_PROTOCOL",
                "GIT_ASKPASS",
            ];

            std::env::vars()
                .map(|(key, _value)| key)
                .filter(|key| {
                    key.starts_with("GIT_")
                        && !key.starts_with("GIT_CONFIG_KEY_")
                        && !key.starts_with("GIT_CONFIG_VALUE_")
                        && !keep.contains(&key.as_str())
                })
                .collect()
        });

        // FIXME(JakobDegen): Ideally we'd use libgit2 directly here instead of shelling out, but
        // unfortunately the third party situation for that library in fbsource isn't great, so
        // let's do this for now
        fn run_git(cwd: &AbsNormPath, f: impl FnOnce(&mut Command)) -> bsmr_error::Result<()> {
            let mut cmd = background_command("git");
            f(&mut cmd);
            // If the user has Git environment variables set, they can cause this Git command to
            // operate on the wrong repo.
            for name in &*GIT_ENV_TO_REMOVE {
                cmd.env_remove(name);
            }

            let output = cmd
                .current_dir(cwd)
                .stderr(Stdio::piped())
                .stdout(Stdio::null())
                .output()
                .bsmr_error_context("Could not run git to fetch external cell")?;

            if !output.status.success() {
                return Err(GitError::Unsuccessful {
                    exit_code: output.status,
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                }
                .into());
            }

            Ok(())
        }

        run_git(&path, |c| {
            match &self.setup.object_format {
                None => c.arg("init"),
                Some(object_format) => c
                    .arg("init")
                    .arg("--object-format")
                    .arg(object_format.to_string()),
            };
        })?;

        run_git(&path, |c| {
            c.arg("fetch")
                .arg(self.setup.git_origin.as_ref())
                .arg(self.setup.commit.as_ref());
        })?;

        run_git(&path, |c| {
            c.arg("reset").arg("--hard").arg("FETCH_HEAD");
        })?;

        Ok(())
    }
}

async fn download_impl(
    ctx: &mut DiceComputations<'_>,
    setup: &GitCellSetup,
    path: &ProjectRelativePath,
    materializer: &dyn Materializer,
    cancellations: &CancellationContext,
) -> bsmr_error::Result<()> {
    let io = ctx.get_blocking_executor();
    io.execute_io(
        Box::new(CleanOutputPaths {
            paths: vec![path.to_owned()],
        }),
        cancellations,
    )
    .await?;

    io.execute_io(
        Box::new(GitFetchIoRequest {
            setup: setup.dupe(),
            path: path.to_owned(),
        }),
        cancellations,
    )
    .await?;

    // Unfortunately, there's no way to ask git not to create this, but it's important that we
    // delete it so that we don't use it or waste cycles hashing it.
    io.execute_io(
        Box::new(CleanOutputPaths {
            paths: vec![path.join(ForwardRelativePath::new(".git").unwrap())],
        }),
        cancellations,
    )
    .await?;

    // Read and hash the contents. We have to do this because the materializer requires an artifact
    // value. This work is kind of duplicated with the reading in the fileops, but only the first
    // time the contents are downloaded. On subsequent invocations of the daemon, we won't rerun
    // this however, so that case will still avoid doing unnecessary work.
    let io_prov = ctx.global_data().get_io_provider();
    let proj_root = io_prov.project_root().root();
    let abs_path = proj_root.join(path);
    let digest_config = ctx.global_data().get_digest_config();
    let file_digest_config = FileDigestConfig::build(digest_config.cas_digest_config());
    let entry = build_entry_from_disk(abs_path, file_digest_config, &*io, proj_root)
        .await?
        .0
        .ok_or(GitError::NoDirectory)?;
    let entry = entry.map_dir(|d| {
        d.to_builder()
            .fingerprint(digest_config.as_directory_serializer())
            .shared(&*INTERNER)
    });

    materializer
        .declare_existing(vec![DeclareArtifactPayload {
            path: path.to_owned(),
            artifact: ArtifactValue::new(entry, None),
            configuration_path: None,
        }])
        .await?;

    Ok(())
}

async fn download_and_materialize(
    ctx: &mut DiceComputations<'_>,
    path: &ProjectRelativePath,
    setup: &GitCellSetup,
    cancellations: &CancellationContext,
) -> bsmr_error::Result<()> {
    let materializer = ctx.per_transaction_data().get_materializer();

    if materializer.has_artifact_at(path.to_owned()).await? {
        return Ok(());
    }

    // A map of commit hashes to semaphores that are actually condvars which protect access to the
    // directory associated with that commit
    static DIRECTORY_LICENSES: OnceLock<Mutex<StdBsmrHashMap<Arc<str>, Arc<Semaphore>>>> =
        OnceLock::new();

    // We have to write this in a slightly funny way to convince the compiler that there's no
    // `map_guard` being held across an await point
    let semaphore;
    let semaphore_guard;
    'populate: {
        'wait: {
            let mut map_guard = DIRECTORY_LICENSES
                .get_or_init(Default::default)
                .lock()
                .unwrap();
            let entry = map_guard.entry(setup.commit.dupe());

            match entry {
                hash_map::Entry::Occupied(entry) => {
                    // There's another key simultaneously populating this directory. Just wait for
                    // it to finish and then return. We don't need to check the contents of the
                    // directory, since we assume that the commit hash uniquely identifies those.
                    semaphore = entry.get().dupe();
                    break 'wait;
                }
                hash_map::Entry::Vacant(entry) => {
                    // It's on us to populate this directory. Make a condvar so that we block other accesses
                    semaphore = Arc::new(Semaphore::new(1));
                    semaphore_guard = semaphore.try_acquire().unwrap(); // we know there's a permit available
                    entry.insert(semaphore.dupe());
                    break 'populate;
                }
            }
        }

        drop(semaphore.acquire().await.unwrap());
        return Ok(());
    }

    // Don't allow the actual download step to be cancelled. In principle it might be possible to
    // properly clean up after a cancellation within the execution of this key, but we'd also have
    // to deal with another key that might be waiting on this download to finish, which would be
    // pretty complicated to deal with.
    let res = cancellations
        .critical_section(|| download_impl(ctx, setup, path, &*materializer, cancellations))
        .await;

    // Give up our lock
    drop(semaphore_guard);
    DIRECTORY_LICENSES
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .remove(&setup.commit)
        .unwrap();

    res
}

#[derive(allocative::Allocative, Pagable)]
pub(crate) struct GitFileOpsDelegate {
    output_resolver: OutputPathResolver,
    cell: CellName,
    setup: GitCellSetup,
    // The fs accesses in this code are sort of a mix between source file accesses and bsmr-out
    // accesses. Unconditionally using an `FsIoProvider` turns out to give all the right behavior
    io: FsIoProvider,
}

impl GitFileOpsDelegate {
    fn resolve(&self, path: &CellRelativePath) -> ProjectRelativePathBuf {
        self.output_resolver
            .resolve_external_cell_source(path, ExternalCellOrigin::Git(self.setup.dupe()))
    }

    fn get_base_path(&self) -> ProjectRelativePathBuf {
        self.resolve(CellRelativePath::empty())
    }
}

#[pagable_typetag]
#[async_trait::async_trait]
impl FileOpsDelegate for GitFileOpsDelegate {
    async fn read_file_if_exists(
        &self,
        _ctx: &mut DiceComputations<'_>,
        path: &'async_trait CellRelativePath,
    ) -> bsmr_error::Result<Option<String>> {
        let project_path = self.resolve(path);
        (&self.io as &dyn IoProvider)
            .read_file_if_exists(project_path)
            .await
    }

    async fn read_dir(
        &self,
        _ctx: &mut DiceComputations<'_>,
        path: &'async_trait CellRelativePath,
    ) -> bsmr_error::Result<Arc<[RawDirEntry]>> {
        let project_path = self.resolve(path);
        let mut entries = (&self.io as &dyn IoProvider)
            .read_dir(project_path)
            .await
            .with_bsmr_error_context(|| format!("Error listing dir `{path}`"))?
            .into_entries();

        // Make sure entries are deterministic, since read_dir isn't.
        entries.sort_by(|a, b| a.file_name.cmp(&b.file_name));

        Ok(entries.into())
    }

    async fn read_path_metadata_if_exists(
        &self,
        _ctx: &mut DiceComputations<'_>,
        path: &'async_trait CellRelativePath,
    ) -> bsmr_error::Result<Option<RawPathMetadata>> {
        let project_path = self.resolve(path);

        let Some(metadata) = (&self.io as &dyn IoProvider)
            .read_path_metadata_if_exists(project_path)
            .await
            .with_bsmr_error_context(|| format!("Error accessing metadata for path `{path}`"))?
        else {
            return Ok(None);
        };
        Ok(Some(metadata.try_map(
            |path| match path.strip_prefix_opt(self.get_base_path()) {
                Some(path) => Ok(Arc::new(CellPath::new(self.cell, path.to_owned().into()))),
                None => Err(internal_error!(
                    "Non-cell internal symlink at `{}` in cell `{}`",
                    path,
                    self.cell
                )),
            },
        )?))
    }

    fn eq_token(&self) -> PartialEqAny<'_> {
        PartialEqAny::always_false()
    }
}

pub(crate) async fn get_file_ops_delegate(
    ctx: &mut DiceComputations<'_>,
    cell: CellName,
    setup: GitCellSetup,
) -> bsmr_error::Result<Arc<GitFileOpsDelegate>> {
    #[derive(
        dupe::Dupe,
        Clone,
        Debug,
        derive_more::Display,
        PartialEq,
        Eq,
        Hash,
        allocative::Allocative,
        Pagable
    )]
    #[display("({}, {})", _0, _1)]
    #[pagable_typetag(dice::DiceKeyDyn)]
    struct GitFileOpsDelegateKey(CellName, GitCellSetup);

    #[async_trait::async_trait]
    impl Key for GitFileOpsDelegateKey {
        type Value = bsmr_error::Result<Arc<GitFileOpsDelegate>>;

        async fn compute(
            &self,
            ctx: &mut DiceComputations,
            cancellations: &CancellationContext,
        ) -> Self::Value {
            let artifact_fs = ctx.get_artifact_fs().await?;
            let ops = GitFileOpsDelegate {
                output_resolver: artifact_fs.output_path_resolver().clone(),
                cell: self.0,
                setup: self.1.dupe(),
                io: FsIoProvider::new(
                    artifact_fs.fs().dupe(),
                    ctx.global_data().get_digest_config().cas_digest_config(),
                    false,
                ),
            };
            download_and_materialize(ctx, &ops.get_base_path(), &self.1, cancellations).await?;
            Ok(Arc::new(ops))
        }

        fn equality(_x: &Self::Value, _y: &Self::Value) -> bool {
            false
        }

        fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
            OkPagableValueSerialize::<Self::Value>::new()
        }
    }

    ctx.compute(&GitFileOpsDelegateKey(cell, setup)).await?
}

pub(crate) async fn materialize_all(
    ctx: &mut DiceComputations<'_>,
    cell: CellName,
    setup: GitCellSetup,
) -> bsmr_error::Result<ProjectRelativePathBuf> {
    // Get the `GitFileOpsDelegate` instance to make sure all the data is materialized.
    let ops = get_file_ops_delegate(ctx, cell, setup.dupe()).await?;
    Ok(ops.get_base_path())
}
