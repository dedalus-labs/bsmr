//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Acquires locked Cargo sources with process-local Git configuration ownership.

use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use cargo::GlobalContext;
use cargo::util::auth::RegistryConfig;
use cargo::util::auth::RegistryConfigExtended;
use cargo::util::context::PathAndArgs;
use git2::ConfigLevel;

use crate::types::Request;

/// Hold the private source home exclusively until the response is flushed or the process exits.
#[cfg(unix)]
pub(crate) fn lease(home: &Path) -> Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    ensure!(home.is_absolute(), "cargo_home must be absolute");
    if !directory(home)? {
        std::fs::create_dir_all(home)?;
    }
    let lease = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .custom_flags(libc::O_NOFOLLOW)
        .mode(0o600)
        .open(home.join("plan.lock"))?;
    ensure!(
        lease.metadata()?.is_file(),
        "source-home lease must be a regular file"
    );
    lease.lock()?;
    Ok(lease)
}

/// Reject hosts that cannot provide the owned no-follow source-home lease.
#[cfg(not(unix))]
pub(crate) fn lease(_home: &Path) -> Result<File> {
    anyhow::bail!("source-home leases require a Unix host")
}

/// Reject executable configuration before sources or compiler probes can use it.
pub(crate) fn isolate(request: &Request, gctx: &GlobalContext) -> Result<tempfile::TempDir> {
    for key in std::env::vars_os()
        .map(|(key, _)| key)
        .chain(gctx.env_config()?.keys().map(Into::into))
    {
        let key = key.to_str().context("non-UTF-8 environment key")?;
        ensure!(
            !key.starts_with("GIT_")
                && key != "SSH_ASKPASS"
                && !(key.starts_with("CARGO_") && key.contains("CREDENTIAL")),
            "unsupported acquisition environment: {key}"
        );
    }
    ensure!(
        gctx.net_config()?.git_fetch_with_cli != Some(true),
        "unsupported acquisition configuration: net.git-fetch-with-cli"
    );
    let providers: Option<Vec<String>> = gctx.get("registry.global-credential-providers")?;
    ensure!(
        providers
            .iter()
            .flatten()
            .all(|provider| provider == "cargo:token"),
        "unsupported acquisition configuration: registry.global-credential-providers"
    );
    let registry: Option<RegistryConfigExtended> = gctx.get("registry")?;
    ensure!(
        registry.is_none_or(|registry| registry.credential_provider.is_none()),
        "unsupported acquisition configuration: registry.credential-provider"
    );
    let registries: Option<BTreeMap<String, RegistryConfig>> = gctx.get("registries")?;
    for (name, registry) in registries.into_iter().flatten() {
        ensure!(
            registry.credential_provider.is_none(),
            "unsupported acquisition configuration: registries.{name}.credential-provider"
        );
    }
    let aliases: Option<BTreeMap<String, PathAndArgs>> = gctx.get("credential-alias")?;
    ensure!(
        aliases.is_none_or(|aliases| aliases.is_empty()),
        "unsupported acquisition configuration: credential-alias"
    );
    isolate_git(&request.cargo_home)
}

/// Replace inherited libgit2 config paths before creating any Cargo source objects.
fn isolate_git(home: &Path) -> Result<tempfile::TempDir> {
    if !directory(home)? {
        std::fs::create_dir_all(home)?;
    }
    let isolated = tempfile::tempdir_in(home)?;
    for level in [
        ConfigLevel::System,
        ConfigLevel::Global,
        ConfigLevel::XDG,
        ConfigLevel::ProgramData,
    ] {
        // SAFETY: This single-request process sets global paths before any concurrent libgit2 work.
        unsafe {
            git2::opts::set_search_path(level, isolated.path())?;
        }
    }

    let git = home.join("git");
    if directory(&git)? {
        for repository in children(&git.join("db"))? {
            repository_config(&repository)?;
        }
        for repository in children(&git.join("checkouts"))? {
            for checkout in children(&repository)? {
                repository_config(&checkout.join(".git"))?;
            }
        }
    }
    Ok(isolated)
}

/// Inspect only owned directories and fail on links before following them.
fn directory(path: &Path) -> Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            ensure!(
                metadata.is_dir(),
                "Cargo cache directory is not owned: {}",
                path.display()
            );
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

/// Enumerate one known Cargo cache level without traversing source files.
fn children(path: &Path) -> Result<Vec<PathBuf>> {
    if !directory(path)? {
        return Ok(Vec::new());
    }
    std::fs::read_dir(path)?
        .map(|entry| {
            let entry = entry?;
            ensure!(
                !entry.file_type()?.is_symlink(),
                "Cargo cache link is not owned: {}",
                entry.path().display()
            );
            Ok(entry.file_type()?.is_dir().then(|| entry.path()))
        })
        .filter_map(|entry| entry.transpose())
        .collect()
}

/// Reject local Git command hooks and includes without opening included files.
fn repository_config(path: &Path) -> Result<()> {
    if !directory(path)? {
        return Ok(());
    }
    ensure!(
        !path.join("commondir").try_exists()?,
        "linked Cargo Git repository is not owned: {}",
        path.display()
    );
    let config = path.join("config");
    match std::fs::symlink_metadata(&config) {
        Ok(metadata) => {
            ensure!(
                metadata.is_file(),
                "Cargo Git config is not owned: {}",
                config.display()
            );
            let bytes = std::fs::read(&config)?;
            let parsed = gix_config::File::from_bytes_no_includes(
                &bytes,
                gix_config::file::Metadata::api(),
                Default::default(),
            )?;
            for section in parsed.sections() {
                let name = section.header().name().to_string().to_ascii_lowercase();
                ensure!(
                    !["include", "includeif", "credential", "filter", "url"]
                        .contains(&name.as_str()),
                    "unsupported Cargo Git config section {name}: {}",
                    config.display()
                );
                for key in section.body().value_names() {
                    let key = key.to_string().to_ascii_lowercase();
                    ensure!(
                        (name != "core"
                            || ![
                                "sshcommand",
                                "hookspath",
                                "fsmonitor",
                                "worktree",
                                "gitproxy"
                            ]
                            .contains(&key.as_str()))
                            && !(name == "extensions" && key == "worktreeconfig"),
                        "unsupported Cargo Git config key {name}.{key}: {}",
                        config.display()
                    );
                }
            }
            for module in children(&path.join("modules"))? {
                repository_config(&module)?;
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            for child in children(path)? {
                repository_config(&child)?;
            }
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::fs::symlink;

    use super::*;

    /// A real credential command runs before isolation and cannot run afterward.
    #[test]
    fn invariant_ambient_credential_helpers_cannot_execute() -> Result<()> {
        let root = tempfile::tempdir()?;
        let marker = root.path().join("executed");
        let hook = root.path().join("credential");
        std::fs::write(
            &hook,
            format!(
                "#!/bin/sh\nprintf hit > '{}'\nprintf 'username=test\\npassword=test\\n'\n",
                marker.display()
            ),
        )?;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o700))?;
        std::fs::write(
            root.path().join(".gitconfig"),
            format!("[credential]\nhelper = {}\n", hook.display()),
        )?;
        // SAFETY: This is the only test accessing libgit2's process-global configuration.
        for level in [
            ConfigLevel::System,
            ConfigLevel::Global,
            ConfigLevel::XDG,
            ConfigLevel::ProgramData,
        ] {
            unsafe {
                git2::opts::set_search_path(level, root.path())?;
            }
        }
        git2::Cred::credential_helper(
            &git2::Config::open_default()?,
            "https://example.invalid",
            None,
        )?;
        assert!(marker.exists());
        std::fs::remove_file(&marker)?;
        let _isolation = isolate_git(&root.path().join("cargo"))?;
        assert!(
            git2::Cred::credential_helper(
                &git2::Config::open_default()?,
                "https://example.invalid",
                None
            )
            .is_err()
        );
        assert!(!marker.exists());
        Ok(())
    }

    /// Repository configuration cannot introduce helper commands or escape through links/includes.
    #[test]
    fn invariant_owned_git_configs_do_not_delegate() -> Result<()> {
        let root = tempfile::tempdir()?;
        let config = root.path().join("config");
        for text in [
            "[credential]\nhelper = marker\n",
            "[include]\npath = /missing/ambient\n",
            "[includeIf \"gitdir:/**\"]\npath = /missing/ambient\n",
            "[core]\nsshCommand = marker\n",
        ] {
            std::fs::write(&config, text)?;
            assert!(
                repository_config(root.path())
                    .unwrap_err()
                    .to_string()
                    .contains("unsupported Cargo Git config")
            );
        }
        std::fs::remove_file(&config)?;
        symlink("/missing/ambient", &config)?;
        assert!(
            repository_config(root.path())
                .unwrap_err()
                .to_string()
                .contains("not owned")
        );
        assert!(
            directory(&config)
                .unwrap_err()
                .to_string()
                .contains("not owned")
        );
        Ok(())
    }
}
