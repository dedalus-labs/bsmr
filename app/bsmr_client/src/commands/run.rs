/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fs::File;
use std::io::Write;

use async_trait::async_trait;
use bsmr_cli_proto::BuildRequest;
use bsmr_cli_proto::build_request::BuildProviders;
use bsmr_cli_proto::build_request::Materializations;
use bsmr_cli_proto::build_request::ResponseOptions;
use bsmr_cli_proto::build_request::Uploads;
use bsmr_cli_proto::build_request::build_providers;
use bsmr_client_ctx::client_ctx::ClientCommandContext;
use bsmr_client_ctx::command_outcome::CommandOutcome;
use bsmr_client_ctx::common::BuckArgMatches;
use bsmr_client_ctx::common::CommonBuildConfigurationOptions;
use bsmr_client_ctx::common::CommonCommandOptions;
use bsmr_client_ctx::common::CommonEventLogOptions;
use bsmr_client_ctx::common::CommonStarlarkOptions;
use bsmr_client_ctx::common::build::CommonBuildOptions;
use bsmr_client_ctx::common::target_cfg::TargetCfgWithUniverseOptions;
use bsmr_client_ctx::common::ui::CommonConsoleOptions;
use bsmr_client_ctx::daemon::client::BuckdClientConnector;
use bsmr_client_ctx::daemon::client::NoPartialResultHandler;
use bsmr_client_ctx::events_ctx::EventsCtx;
use bsmr_client_ctx::exit_result::ExitResult;
use bsmr_client_ctx::path_arg::PathArg;
use bsmr_client_ctx::streaming::StreamingCommand;
use bsmr_common::argv::Argv;
use bsmr_common::argv::SanitizedArgv;
use bsmr_error::BuckErrorContext;
use bsmr_error::conversion::from_any_with_tag;
use bsmr_hash::StdBuckHashMap;
use bsmr_hash::StdBuckHashSet;
use bsmr_wrapper_common::BSMR_WRAPPER_ENV_VAR;
use bsmr_wrapper_common::BUCK_WRAPPER_START_TIME_ENV_VAR;
use bsmr_wrapper_common::BUCK_WRAPPER_UUID_ENV_VAR;
use serde::Serialize;

use crate::commands::build::print_buck_ui_and_rating;
use crate::commands::build::print_build_failed;
use crate::commands::build::print_build_result;
use crate::commands::build::print_build_succeeded;

/// Build and run the selected target.
///
/// Use `--` to separate arguments to the target from arguments to bsmr:
///
/// bsmr run //my/target -- --arg1 --arg2
///
/// The Build ID for the underlying build execution is made available to the target in
/// the `BUCK_RUN_BUILD_ID` environment variable.
#[derive(Debug, clap::Parser)]
// FIXME(JakobDegen): Remove usage override once soft error is removed
#[clap(
    name = "run",
    trailing_var_arg = true,
    override_usage = "bsmr run [OPTIONS] <TARGET> [-- <TARGET_ARGS>...]"
)]
pub struct RunCommand {
    #[clap(
        long = "command-args-file",
        help = "Write the command to a file instead of executing it.",
        group = "exec_options"
    )]
    command_args_file: Option<String>,

    #[clap(
        long = "chdir",
        help = "Set the current working directory of the executable being run",
        group = "exec_options"
    )]
    chdir: Option<PathArg>,

    /// Instead of running the command, print out the command
    /// formatted for shell interpolation, use as: $(bsmr run --emit-shell ...)
    #[clap(long, group = "exec_options")]
    emit_shell: bool,

    #[clap(name = "TARGET", help = "Target to build and run", value_hint = clap::ValueHint::Other)]
    target: String,

    #[clap(
        name = "TARGET_ARGS",
        help = "Additional arguments passed to the target when running it"
    )]
    extra_run_args: Vec<String>,

    #[clap(flatten)]
    build_opts: CommonBuildOptions,

    #[clap(flatten)]
    target_cfg: TargetCfgWithUniverseOptions,

    #[clap(flatten)]
    common_opts: CommonCommandOptions,
}

#[async_trait(?Send)]
impl StreamingCommand for RunCommand {
    const COMMAND_NAME: &'static str = "run";

    async fn exec_impl(
        self,
        buckd: &mut BuckdClientConnector,
        matches: BuckArgMatches<'_>,
        ctx: &mut ClientCommandContext<'_>,
        events_ctx: &mut EventsCtx,
    ) -> ExitResult {
        let run_args_missing_separator =
            // We will soon require a separator before the start of the runs args.
            // Check that the expanded argv has a separator (so we catch them in @ files), 
            // and if not print a warning.
            !self.extra_run_args.is_empty() && !ctx.expanded_argv_has_separator();

        let context = ctx.client_context(matches, &self)?;
        let has_target_universe = !self.target_cfg.target_universe.is_empty();
        // TODO(rafaelc): fail fast on the daemon if the target doesn't have RunInfo
        let response = buckd
            .with_flushing()
            .build(
                BuildRequest {
                    context: Some(context),
                    // TODO(wendyy): glob patterns should be prohibited, and command should fail before the build event happens.
                    target_patterns: vec![self.target.clone()],
                    target_cfg: Some(self.target_cfg.target_cfg.target_cfg()),
                    build_providers: Some(BuildProviders {
                        default_info: build_providers::Action::Skip as i32,
                        run_info: build_providers::Action::Build as i32,
                        test_info: build_providers::Action::Skip as i32,
                    }),
                    // `buck run` execs the target, so it needs the resolved run command line.
                    response_options: Some(ResponseOptions {
                        return_outputs: false,
                        return_run_args: true,
                    }),
                    build_opts: Some(self.build_opts.to_proto()),
                    final_artifact_materializations: Materializations::Materialize as i32,
                    final_artifact_uploads: Uploads::Never as i32,
                    target_universe: self.target_cfg.target_universe,
                    timeout: None, // TODO: maybe it shouild be supported here?
                    run_args_missing_separator,
                },
                events_ctx,
                ctx.console_interaction_stream(&self.common_opts.console_opts),
                &mut NoPartialResultHandler,
            )
            .await;

        let console = self.common_opts.console_opts.final_console();
        let success = match &response {
            Ok(CommandOutcome::Success(response)) => response.errors.is_empty(),
            Ok(CommandOutcome::Failure(_)) => false,
            Err(_) => false,
        };
        if !success {
            print_build_failed(&console)?;
        }
        let response = response??;
        print_build_result(&console, &response.errors)?;

        if !success {
            return ExitResult::from_command_result_errors(response.errors);
        }

        if response.build_targets.len() > 1 {
            return ExitResult::err(RunCommandError::MultipleTargets.into());
        }

        if has_target_universe && response.build_targets.is_empty() {
            return ExitResult::err(
                RunCommandError::TargetNotFoundInTargetUniverse(self.target).into(),
            );
        }

        // TODO(rafaelc): use absolute paths for artifacts in the cli
        //      we should run the command from the current dir, not the project root
        if response.build_targets.is_empty() || response.build_targets[0].run_args.is_empty() {
            return ExitResult::err(RunCommandError::NonBinaryRule(self.target).into());
        }
        let mut run_args = response.build_targets[0].run_args.clone();
        run_args.extend(self.extra_run_args);

        let extra = if !self.emit_shell {
            Some(" - starting your binary")
        } else {
            None
        };

        print_buck_ui_and_rating(&console, ctx, events_ctx.used_superconsole)?;
        print_build_succeeded(&console, ctx, extra)?;

        // Special case for recursive invocations of buck; `BSMR_WRAPPER` is set by wrapper scripts that execute
        // Bessemer. We're not a wrapper script, so we unset it to prevent `run` from inheriting it.
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(BSMR_WRAPPER_ENV_VAR) };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(BUCK_WRAPPER_UUID_ENV_VAR) };
        // TODO: Audit that the environment access only happens in single-threaded code.
        unsafe { std::env::remove_var(BUCK_WRAPPER_START_TIME_ENV_VAR) };

        if let Some(file_path) = self.command_args_file {
            let mut output = File::create(&file_path).with_buck_error_context(|| {
                format!("Failed to create/open `{file_path}` to print command")
            })?;

            let command = CommandArgsFile {
                path: run_args[0].clone(),
                argv: run_args,
                envp: std::env::vars().collect(),
                is_fix_script: false,
                print_command: false,
            };
            let serialized = serde_json::to_string(&command)
                .buck_error_context("Failed to serialize command")?;
            output
                .write_all(serialized.as_bytes())
                .buck_error_context("Failed to write command")?;

            return ExitResult::success();
        }

        if self.emit_shell {
            if cfg!(unix) {
                bsmr_client_ctx::println!(
                    "{}",
                    shlex::try_join(run_args.iter().map(|a| a.as_str()))
                        .map_err(|e| from_any_with_tag(e, bsmr_error::ErrorTag::Tier0))?
                )?;
                return ExitResult::success();
            } else {
                return ExitResult::err(RunCommandError::EmitShellNotSupportedOnWindows.into());
            }
        }

        let chdir = self.chdir.map(|chdir| chdir.resolve(&ctx.working_dir));

        ExitResult::exec(
            run_args[0].clone().into(),
            run_args.into_iter().map(|arg| arg.into()).collect(),
            chdir,
            vec![("BUCK_RUN_BUILD_ID".to_owned(), ctx.trace_id.to_string())],
        )
    }

    fn console_opts(&self) -> &CommonConsoleOptions {
        &self.common_opts.console_opts
    }

    fn event_log_opts(&self) -> &CommonEventLogOptions {
        &self.common_opts.event_log_opts
    }

    fn build_config_opts(&self) -> &CommonBuildConfigurationOptions {
        &self.common_opts.config_opts
    }

    fn starlark_opts(&self) -> &CommonStarlarkOptions {
        &self.common_opts.starlark_opts
    }

    fn sanitize_argv(&self, argv: Argv) -> SanitizedArgv {
        let to_redact: StdBuckHashSet<_> = self.extra_run_args.iter().collect();
        argv.redacted(to_redact)
    }
}

#[derive(Serialize)]
struct CommandArgsFile {
    path: String,
    argv: Vec<String>,
    envp: StdBuckHashMap<String, String>,
    // Not used. For buck_v1 back compatibility only.
    is_fix_script: bool,
    // Not used. For buck_v1 back compatibility only.
    print_command: bool,
}

#[derive(bsmr_error::Error, Debug)]
#[bsmr(tag = Input)]
pub enum RunCommandError {
    #[error("Target `{0}` is not a binary rule (only binary rules can be `run`)")]
    NonBinaryRule(String),
    #[error("`--emit-shell` is not supported on Windows")]
    EmitShellNotSupportedOnWindows,
    #[error("`bsmr run` only supports a single target, but multiple targets were requested.")]
    MultipleTargets,
    #[error("Target `{0}` is not found in the specified target universe")]
    TargetNotFoundInTargetUniverse(String),
    #[error(
        "`bsmr run` will require a `--` separator before target arguments in the future. \
         Please use `bsmr run <target> -- <args>` instead of `bsmr run <target> <args>`"
    )]
    MissingSeparator,
}
