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

use std::io::ErrorKind;
use std::io::Write;

use bsmr_client_ctx::client_ctx::ClientCommandContext;
use bsmr_client_ctx::common::BuckArgMatches;
use bsmr_client_ctx::common::ui::CommonConsoleOptions;
use bsmr_client_ctx::exit_result::ExitResult;
use bsmr_client_ctx::final_console::FinalConsole;
use bsmr_client_ctx::path_arg::PathArg;
use bsmr_common::argv::Argv;
use bsmr_common::argv::SanitizedArgv;
use bsmr_error::BuckErrorContext;
use bsmr_error::ErrorTag;
use bsmr_error::bsmr_error;
use bsmr_fs::error::IoResultExt;
use bsmr_fs::fs_util;
use bsmr_fs::paths::abs_path::AbsPath;
use bsmr_util::process::background_command;

/// Initializes a bsmr project at the provided path.
#[derive(Debug, clap::Parser)]
#[clap(name = "init", about = "Initialize a bsmr project")]
pub struct InitCommand {
    /// The path to initialize the project in. The folder does not need to exist.
    #[clap(default_value = ".")]
    path: PathArg,

    /// Don't include the standard prelude.
    #[clap(long)]
    no_prelude: bool,

    /// Initialize the project even if the git repo at \[PATH\] has uncommitted changes.
    #[clap(long)]
    allow_dirty: bool,

    /// Also initialize a git repository at the given path, and set up an appropriate `.gitignore`
    /// file.
    #[clap(long)]
    git: bool,

    #[clap(flatten)]
    console_opts: CommonConsoleOptions,
}

impl InitCommand {
    pub fn exec(self, _matches: BuckArgMatches<'_>, ctx: ClientCommandContext<'_>) -> ExitResult {
        let console = self.console_opts.final_console();

        match exec_impl(self, ctx, &console) {
            Ok(_) => ExitResult::success(),
            Err(e) => {
                // include the backtrace with the error output
                // (same behaviour as returning the Error from main)
                bsmr_error!(ErrorTag::Tier0, "{:?}", e).into()
            }
        }
    }

    pub fn sanitize_argv(&self, argv: Argv) -> SanitizedArgv {
        argv.no_need_to_sanitize()
    }
}

fn exec_impl(
    cmd: InitCommand,
    ctx: ClientCommandContext<'_>,
    console: &FinalConsole,
) -> bsmr_error::Result<()> {
    let path = cmd.path.resolve(&ctx.working_dir);
    fs_util::create_dir_all(&path)?;
    let absolute = fs_util::canonicalize(&path).categorize_internal()?;
    let git = cmd.git;

    if absolute.is_file() {
        return Err(bsmr_error!(
            bsmr_error::ErrorTag::Input,
            "Target path {} cannot be an existing file",
            absolute.display()
        ));
    }

    if git {
        let status = match background_command("git")
            .args(["status", "--porcelain"])
            .current_dir(&absolute)
            .output()
        {
            Err(e) if e.kind().eq(&ErrorKind::NotFound) => {
                console.print_error(
                    "Warning: no git found on path, can't check for dirty repo. Proceeding anyway.",
                )?;
                None
            }
            r => Some(r.buck_error_context("Couldn't detect dirty status of folder.")?),
        };

        let changes = status.filter(|o| o.status.success()).map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .lines()
                .any(|l| !l.starts_with("??"))
        });

        if let (Some(true), false) = (changes, cmd.allow_dirty) {
            return Err(bsmr_error!(
                bsmr_error::ErrorTag::Input,
                "Refusing to initialize in a dirty repo. Stash your changes or use `--allow-dirty` to override."
            ));
        }
    }

    set_up_project(&absolute, git, !cmd.no_prelude)
}

fn initialize_bsmrconfig(repo_root: &AbsPath, prelude: bool, git: bool) -> bsmr_error::Result<()> {
    let mut bsmrconfig = std::fs::File::create(repo_root.join(".bsmr"))?;
    writeln!(bsmrconfig, "[project]")?;
    writeln!(bsmrconfig, "  root = .")?;
    if git {
        writeln!(bsmrconfig, "  ignore = .git")?;
    }
    writeln!(bsmrconfig)?;
    writeln!(bsmrconfig, "[cells]")?;
    writeln!(bsmrconfig, "  root = .")?;

    // Add additional configs that depend on prelude / no-prelude mode
    if prelude {
        writeln!(bsmrconfig, "  prelude = prelude")?;
        writeln!(bsmrconfig, "  none = none")?;
        writeln!(bsmrconfig)?;
        writeln!(bsmrconfig, "[cell_aliases]")?;
        writeln!(bsmrconfig, "  config = prelude")?;
        writeln!(bsmrconfig, "  ovr_config = prelude")?;
        writeln!(bsmrconfig, "  upstream = none")?;
        writeln!(bsmrconfig, "  toolchains = root")?;
        writeln!(bsmrconfig)?;
        writeln!(
            bsmrconfig,
            "# Uses a copy of the prelude bundled with the bsmr binary. You can alternatively delete this"
        )?;
        writeln!(
            bsmrconfig,
            "# section and vendor a copy of the prelude to the `prelude` directory of your project."
        )?;
        writeln!(bsmrconfig, "[external_cells]")?;
        writeln!(bsmrconfig, "  prelude = bundled")?;
        writeln!(bsmrconfig)?;
        writeln!(bsmrconfig, "[parser]")?;
        writeln!(
            bsmrconfig,
            "  target_platform_detector_spec = target:root//...->prelude//platforms:default \\
    target:prelude//...->prelude//platforms:default \\
    target:toolchains//...->prelude//platforms:default"
        )?;
        writeln!(bsmrconfig)?;
        writeln!(bsmrconfig, "[build]")?;
        writeln!(
            bsmrconfig,
            "  execution_platforms = prelude//platforms:default"
        )?;
    }

    Ok(())
}

fn set_up_gitignore(repo_root: &AbsPath) -> bsmr_error::Result<()> {
    let gitignore = repo_root.join(".gitignore");
    // If .gitignore is empty or doesn't exist, add in buck-out
    if !gitignore.exists() || fs_util::metadata(&gitignore).categorize_internal()?.len() == 0 {
        fs_util::write(gitignore, "/buck-out\n").categorize_internal()?;
    }
    Ok(())
}

fn set_up_project(repo_root: &AbsPath, git: bool, prelude: bool) -> bsmr_error::Result<()> {
    if git {
        if !background_command("git")
            .arg("init")
            .current_dir(repo_root)
            .status()?
            .success()
        {
            return Err(bsmr_error!(
                bsmr_error::ErrorTag::Tier0,
                "Failure when running `git init`."
            ));
        };
        set_up_gitignore(repo_root)?;
    }

    // If the project already contains a .bsmr, leave it alone.
    if repo_root.join(".bsmr").exists() {
        bsmr_client_ctx::println!(".bsmr already exists, not overwriting")?;
        return Ok(());
    }

    initialize_bsmrconfig(repo_root, prelude, git)
}

#[cfg(test)]
mod tests {
    use bsmr_fs::fs_util::uncategorized as fs_util;
    use bsmr_fs::paths::abs_path::AbsPath;

    use crate::commands::init::initialize_bsmrconfig;
    use crate::commands::init::set_up_gitignore;
    use crate::commands::init::set_up_project;

    #[test]
    fn test_set_up_project_with_prelude_no_git() -> bsmr_error::Result<()> {
        let tempdir = tempfile::tempdir()?;
        let tempdir_path = tempdir.path();
        let tempdir_path = AbsPath::new(tempdir_path)?;
        fs_util::create_dir_all(tempdir_path)?;

        // no git, with prelude
        set_up_project(tempdir_path, false, true)?;
        assert!(tempdir_path.join(".bsmr").exists());
        assert!(!tempdir_path.join(".bsmrconfig").exists());
        assert!(!tempdir_path.join(".bsmrroot").exists());
        assert!(!tempdir_path.join("toolchains").exists());
        assert!(!tempdir_path.join("BUILD.bsmr").exists());
        assert!(!tempdir_path.join("BUCK").exists());
        Ok(())
    }

    #[test]
    fn test_default_gitignore() -> bsmr_error::Result<()> {
        let tempdir = tempfile::tempdir()?;
        let tempdir_path = tempdir.path();
        let tempdir_path = AbsPath::new(tempdir_path)?;
        fs_util::create_dir_all(tempdir_path)?;

        // .gitignore does not exist yet
        set_up_gitignore(tempdir_path)?;
        let gitignore_path = tempdir_path.join(".gitignore");
        assert!(gitignore_path.exists());
        let actual = fs_util::read_to_string(&gitignore_path)?;
        let expected = "/buck-out\n";
        assert_eq!(actual, expected);

        // If an empty .gitignore exists (this is the case after running `git init`), add `buck-out`.
        fs_util::write(&gitignore_path, "")?;
        set_up_gitignore(tempdir_path)?;
        assert!(gitignore_path.exists());
        let actual = fs_util::read_to_string(&gitignore_path)?;
        assert_eq!(actual, expected);

        // If a non-empty .gitignore exists, don't touch it.
        fs_util::write(&gitignore_path, "foo\nbar\n")?;
        set_up_gitignore(tempdir_path)?;
        assert!(gitignore_path.exists());
        let actual = fs_util::read_to_string(&gitignore_path)?;
        let expected = "foo\nbar\n";
        assert_eq!(actual, expected);
        Ok(())
    }

    #[test]
    fn test_bsmrconfig_generation_with_prelude() -> bsmr_error::Result<()> {
        let tempdir = tempfile::tempdir()?;
        let tempdir_path = tempdir.path();
        let tempdir_path = AbsPath::new(tempdir_path)?;
        fs_util::create_dir_all(tempdir_path)?;

        let bsmrconfig_path = tempdir_path.join(".bsmr");
        initialize_bsmrconfig(tempdir_path, true, true)?;
        let actual_bsmrconfig = fs_util::read_to_string(bsmrconfig_path)?;
        let expected_bsmrconfig = "[project]
  root = .
  ignore = .git

[cells]
  root = .
  prelude = prelude
  none = none

[cell_aliases]
  config = prelude
  ovr_config = prelude
  upstream = none
  toolchains = root

# Uses a copy of the prelude bundled with the bsmr binary. You can alternatively delete this
# section and vendor a copy of the prelude to the `prelude` directory of your project.
[external_cells]
  prelude = bundled

[parser]
  target_platform_detector_spec = target:root//...->prelude//platforms:default \\
    target:prelude//...->prelude//platforms:default \\
    target:toolchains//...->prelude//platforms:default

[build]
  execution_platforms = prelude//platforms:default
";
        assert_eq!(actual_bsmrconfig, expected_bsmrconfig);
        Ok(())
    }

    #[test]
    fn test_bsmrconfig_generation_without_prelude() -> bsmr_error::Result<()> {
        let tempdir = tempfile::tempdir()?;
        let tempdir_path = tempdir.path();
        let tempdir_path = AbsPath::new(tempdir_path)?;
        fs_util::create_dir_all(tempdir_path)?;

        let bsmrconfig_path = tempdir_path.join(".bsmr");
        initialize_bsmrconfig(tempdir_path, false, false)?;
        let actual_bsmrconfig = fs_util::read_to_string(bsmrconfig_path)?;
        let expected_bsmrconfig = "[project]
  root = .

[cells]
  root = .
";
        assert_eq!(actual_bsmrconfig, expected_bsmrconfig);

        Ok(())
    }
}
