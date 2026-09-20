//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies publication reads finalized outputs while retaining command output paths.

use bsmr_core::cells::CellResolver;
use bsmr_core::cells::cell_root_path::CellRootPathBuf;
use bsmr_core::cells::name::CellName;
use bsmr_core::configuration::data::ConfigurationData;
use bsmr_core::deferred::base_deferred_key::BaseDeferredKey;
use bsmr_core::fs::output_path::BuildArtifactPath;
use bsmr_core::fs::output_path::OutputPathKind;
use bsmr_core::fs::output_path::OutputPathResolver;
use bsmr_core::fs::project::ProjectRootTemp;
use bsmr_core::fs::project_rel_path::ProjectRelativePath;
use bsmr_core::target::label::label::TargetLabel;
use bsmr_execute::artifact_utils::ArtifactValueBuilder;
use bsmr_execute::execute::action_digest::ActionDigest;
use bsmr_execute::execute::request::OutputType;

use super::*;

#[test]
fn publication_reads_finalized_outputs_with_command_paths() -> bsmr_error::Result<()> {
    for kind in [OutputPathKind::ContentHash, OutputPathKind::Configuration] {
        for directory in [true, false] {
            let project = ProjectRootTemp::new()?;
            let artifact_fs = ArtifactFs::new(
                CellResolver::testing_with_name_and_path(
                    CellName::testing_new("root"),
                    CellRootPathBuf::testing_new(""),
                ),
                OutputPathResolver::new(ProjectRelativePath::new("bsmr-out/default")?.to_owned()),
                project.path().dupe(),
            );
            let digest_config = DigestConfig::testing_default();
            let bytes = b"finalized output";
            let digest = TrackedFileDigest::from_content(bytes, digest_config.cas_digest_config());
            let mut builder = ArtifactValueBuilder::new(artifact_fs.fs(), digest_config);
            builder.add_entry(
                ProjectRelativePath::new(if directory { "output/member" } else { "output" })?
                    .to_owned(),
                DirectoryEntry::Leaf(ActionDirectoryMember::File(FileMetadata {
                    digest: digest.dupe(),
                    is_executable: false,
                })),
            )?;
            let value = builder.build(ProjectRelativePath::new("output")?)?;
            let output = CommandExecutionOutput::BuildArtifact {
                path: BuildArtifactPath::new(
                    BaseDeferredKey::TargetLabel(
                        TargetLabel::testing_parse("root//:archive")
                            .configure(ConfigurationData::testing_new()),
                    ),
                    ForwardRelativePath::new("output")?.to_owned(),
                    kind,
                ),
                output_type: if directory {
                    OutputType::Directory
                } else {
                    OutputType::File
                },
            };
            let finalized = output.as_ref().resolve(
                &artifact_fs,
                output
                    .has_content_based_path()
                    .then(|| value.content_based_path_hash())
                    .as_ref(),
            )?;
            let file = if directory {
                finalized.path().join(ForwardRelativePath::new("member")?)
            } else {
                finalized.path().to_owned()
            };
            artifact_fs.fs().write_file(&file, bytes, false)?;
            let action = ActionDigest::from_content(b"action", digest_config.cas_digest_config());
            let command = output.as_ref().resolve(
                &artifact_fs,
                output
                    .has_content_based_path()
                    .then_some(ContentBasedPathHash::OutputArtifact)
                    .as_ref(),
            )?;
            let command_path = command.path().to_string();
            let outputs = BsmrIndexMap::from_iter([(output, value)]);
            let cache = LocalActionCache::at(
                project
                    .path()
                    .root()
                    .join(ForwardRelativePath::new("cache")?)
                    .into_path_buf(),
            )?;
            publish_result(
                &artifact_fs,
                &cache,
                digest_config,
                &outputs,
                b"",
                b"",
                &action,
            )?;
            let manifest = cache.action_result(&action)?.expect("published result");
            validate_output_paths(std::iter::once(command_path), &manifest)?;
            assert_eq!(
                cache
                    .read_blob(&LocalDigest::from_file(&digest), digest_config)?
                    .unwrap(),
                bytes
            );
        }
    }
    Ok(())
}
