//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Keeps compiler probes and native actions within their declared input contracts.

use std::collections::BTreeMap;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use anyhow::ensure;
use cargo::GlobalContext;
use cargo::util::context::StringList;
use serde::Deserialize;

/// Options whose arguments are compiler values, not filesystem inputs.
const VALUE_FLAGS: &[&str] = &[
    "--cfg",
    "--check-cfg",
    "--cap-lints",
    "--allow",
    "--warn",
    "--deny",
    "--forbid",
    "-A",
    "-W",
    "-D",
    "-F",
];
/// Code generation settings that do not alter dependency or tool ownership.
const SCALAR_CODEGEN: &[&str] = &[
    "opt-level",
    "debuginfo",
    "codegen-units",
    "debug-assertions",
    "overflow-checks",
];
/// Probes inspect these settings without linking or executing a compiler extension.
const PROBE_CODEGEN: &[&str] = &["target-feature"];

/// Compiler probes never link. Native compilation needs declared linker inputs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Phase {
    Probe,
    Compile,
}

#[derive(Deserialize, Default)]
struct Config {
    /// Cargo expands array and whitespace-separated forms through the same type.
    rustflags: Option<StringList>,
    /// Cargo resolves the driver separately through its target configuration.
    #[serde(rename = "linker")]
    _linker: Option<String>,
}

/// Reject response files and executable compiler extensions before Cargo probes rustc.
pub(crate) fn configure(context: &GlobalContext) -> Result<()> {
    for (key, _) in std::env::vars_os() {
        let key = key.to_string_lossy();
        ensure!(
            !key.ends_with("RUSTFLAGS"),
            "unsupported compiler environment: {key}"
        );
    }
    let build: Config = context.get::<Option<Config>>("build")?.unwrap_or_default();
    let targets: BTreeMap<String, Config> = context
        .get::<Option<BTreeMap<String, Config>>>("target")?
        .unwrap_or_default();
    for config in std::iter::once(&build).chain(targets.values()) {
        if let Some(flags) = &config.rustflags {
            validate(flags.as_slice(), Phase::Probe)?;
        }
    }
    Ok(())
}

/// Admit scalar and linker arguments. Native analysis owns linker execution policy.
pub(crate) fn validate(flags: &[String], phase: Phase) -> Result<()> {
    ensure!(
        flags.iter().all(|flag| !flag.starts_with('@')),
        "unsupported compiler flag: response files require declared inputs"
    );
    let mut arguments = flags.iter();
    while let Some(flag) = arguments.next() {
        let mut rejected = flag.as_str();
        let (name, inline) = flag
            .split_once('=')
            .map_or((flag.as_str(), None), |(name, value)| (name, Some(value)));
        if VALUE_FLAGS.contains(&name) {
            let value = inline
                .or_else(|| arguments.next().map(String::as_str))
                .context("compiler flag requires a value")?;
            ensure!(
                !value.is_empty() && !value.starts_with('-'),
                "compiler flag requires a scalar value"
            );
            continue;
        }
        if ["-A", "-W", "-D", "-F"]
            .iter()
            .any(|prefix| flag.starts_with(prefix) && flag.len() > 2)
        {
            continue;
        }
        let codegen = flag
            .strip_prefix("-C")
            .or_else(|| flag.strip_prefix("--codegen="));
        if let Some(codegen) = codegen {
            let codegen = if codegen.is_empty() {
                arguments.next().context("codegen flag requires a value")?
            } else {
                codegen
            };
            let key = codegen
                .split('=')
                .next()
                .expect("split returns the first component");
            if SCALAR_CODEGEN.contains(&key)
                || key == "link-arg"
                || (phase == Phase::Probe && PROBE_CODEGEN.contains(&key))
            {
                continue;
            }
            rejected = codegen;
        }
        bail!(
            "unsupported compiler flag `{rejected}` during {phase:?}: requires declared inputs or a qualified execution contract"
        );
    }
    Ok(())
}
