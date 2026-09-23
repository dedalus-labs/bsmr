//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Moves one complete SHA-256 local action cache across ephemeral execution roots.

use std::path::Path;
use std::path::PathBuf;

use bsmr_common::cas_digest::DigestAlgorithm;
use bsmr_execute::digest_config::DigestConfig;
use bsmr_execute::execute::local_cache::LocalActionCache;
use bsmr_execute::execute::local_cache::transport::sha256_file;

/// Offline local-cache transport commands.
#[derive(Debug, clap::Parser)]
#[clap(
    name = "cache",
    about = "Move verified SHA-256 local cache state between roots"
)]
pub(crate) struct CacheCommand {
    #[clap(subcommand)]
    command: CacheSubcommand,
}

#[derive(Debug, clap::Subcommand)]
enum CacheSubcommand {
    /// Export the configured local cache into a new package directory.
    Export {
        /// New directory to publish atomically.
        #[clap(long)]
        output: PathBuf,
    },
    /// Import a verified package into the absent configured local cache root.
    Import {
        /// Package directory created by `bsmr cache export`.
        #[clap(long)]
        input: PathBuf,
    },
}

impl CacheCommand {
    /// Executes without a project or daemon and binds the package to this executable.
    pub(crate) fn exec(self) -> bsmr_error::Result<()> {
        let executable = std::env::current_exe()?;
        let engine_sha256 = sha256_file(&executable)?;
        let digest_config = DigestConfig::leak_new(vec![DigestAlgorithm::Sha256], None)?;
        let cache = LocalActionCache::open()?;
        match self.command {
            CacheSubcommand::Export { output } => {
                let output = absolute(&output)?;
                let manifest = cache.export_package(&output, &engine_sha256, digest_config)?;
                bsmr_client_ctx::println!(
                    "Exported {} cache files to {} with engine sha256:{}",
                    manifest.files.len(),
                    output.display(),
                    engine_sha256
                )?;
            }
            CacheSubcommand::Import { input } => {
                let input = absolute(&input)?;
                let manifest = cache.import_package(&input, &engine_sha256, digest_config)?;
                bsmr_client_ctx::println!(
                    "Imported {} cache files from {} with engine sha256:{}",
                    manifest.files.len(),
                    input.display(),
                    engine_sha256
                )?;
            }
        }
        Ok(())
    }
}

fn absolute(path: &Path) -> std::io::Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}
