//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Renders the toolchain targets that native frontends add to a package's own rules.

//! Native frontends contribute toolchains to a package without owning it.
//!
//! The package keeps whatever defines it (a Starlark build file or a native
//! manifest); the targets rendered here are appended after its own rules.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use bsmr_core::fs::project_rel_path::ProjectRelativePath;
use bsmr_core::package::PackageLabel;
use bsmr_core::package::package_relative_path::PackageRelativePath;
use bsmr_fs::paths::abs_norm_path::AbsNormPathBuf;
use bsmr_fs::paths::file_name::FileName;
use dice::DiceComputations;
use dice::InjectedKey;
use dice::PagableValueSerialize;
use dice::ValueSerialize;
use dupe::Dupe;
use pagable::Pagable;
use pagable::pagable_typetag;
use serde::Deserialize;

use crate::dice::cells::HasCellResolver;
use crate::file_ops::dice::DiceFileComputations;
use crate::invocation_roots::home_bsmr_dir;
use crate::package_listing::PackageBuildSource;
use crate::package_listing::listing::PackageListing;

/// The directory, beside the `toolchains//` package, where earlier releases copied the host SDK.
pub const GO_SDK_DIRECTORY: &str = ".bsmr-go-sdk";

/// Reports whether a directory name is an earlier release's Go acquisition, which is host state.
pub fn is_go_acquisition(name: &str) -> bool {
    name == GO_SDK_DIRECTORY
}

/// Verified Go SDK metadata files under the user root, by file name, read once per command.
///
/// `bsmr go toolchain` writes them outside every checkout, so the file watcher never sees a
/// change; injecting them per command lets a running daemon pick up a new acquisition.
#[derive(
    Clone,
    Dupe,
    Debug,
    Eq,
    PartialEq,
    Hash,
    Allocative,
    Pagable,
    derive_more::Display
)]
#[display("GoSdkMetadata")]
#[pagable_typetag(dice::DiceKeyDyn)]
pub struct GoSdkMetadata;

impl InjectedKey for GoSdkMetadata {
    type Value = Arc<BTreeMap<String, String>>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
        PagableValueSerialize::<Self::Value>::new()
    }
}

/// Returns the user-root directory holding one verified metadata file per Go SDK version.
pub fn go_sdk_metadata_directory() -> bsmr_error::Result<AbsNormPathBuf> {
    Ok(home_bsmr_dir()?
        .join(FileName::new("bootstrap")?)
        .join(FileName::new("go")?))
}

/// Reads every `go<version>.json` metadata file; an absent directory holds none.
pub fn capture_go_sdk_metadata() -> bsmr_error::Result<BTreeMap<String, String>> {
    let directory = go_sdk_metadata_directory()?;
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => return Err(error.into()),
    };
    let mut metadata = BTreeMap::new();
    for entry in entries {
        let entry = entry?;
        if let Some(name) = entry.file_name().to_str()
            && name.starts_with("go")
            && name.ends_with(".json")
        {
            metadata.insert(name.to_owned(), fs::read_to_string(entry.path())?);
        }
    }
    Ok(metadata)
}

/// Comparable `(major, minor, patch)` key for Go 1.x versions.
type VersionKey = (u64, u64, u64);

/// Syntax or directive errors raised while reading a Go module or workspace pin.
#[derive(Debug, Eq, PartialEq)]
pub enum GoPinParseError {
    /// A `go` or `toolchain` directive is malformed, duplicated, block-form, or contradictory.
    InvalidPin,
    /// The file violates Go's modfile lexical or block syntax.
    Syntax(String),
}

/// Extracts an exact stable Go pin from `go.mod` or `go.work` text.
///
/// An explicit `toolchain go1.X.Y` wins; without one, an exact `go 1.X.Y` line pins
/// that release because Go deletes `toolchain` when both match. Returns `Ok(None)`
/// when the file has no `toolchain` line and no exact 3-part `go` line (for example
/// `go 1.26`). Invalid directives or syntax fail closed.
pub fn parse_go_pin(source: &str) -> Result<Option<&str>, GoPinParseError> {
    let (mut go_line, mut tc_line): (Option<Vec<&str>>, Option<Vec<&str>>) = (None, None);
    let mut in_block = false;
    for raw_line in source.split('\n') {
        let tokens = lex_go_mod_line(raw_line).map_err(GoPinParseError::Syntax)?;
        if tokens.is_empty() {
            continue;
        }
        if in_block {
            if tokens[0] == ")" {
                if tokens.len() > 1 {
                    return Err(GoPinParseError::Syntax(
                        "expected newline after closing paren".to_owned(),
                    ));
                }
                in_block = false;
            }
            continue;
        }
        let is_block = if tokens.last() == Some(&"(") {
            in_block = true;
            true
        } else {
            tokens.len() >= 2 && tokens[tokens.len() - 2..] == ["(", ")"]
        };
        let slot = match tokens[0] {
            "toolchain" => &mut tc_line,
            "go" => &mut go_line,
            _ => continue,
        };
        if is_block || slot.is_some() {
            return Err(GoPinParseError::InvalidPin);
        }
        *slot = Some(tokens);
    }
    if in_block {
        return Err(GoPinParseError::Syntax("unterminated block".to_owned()));
    }
    let go_ver = match go_line.as_deref() {
        Some([_, v]) => Some(parse_go_ver(v).ok_or(GoPinParseError::InvalidPin)?),
        Some(_) => return Err(GoPinParseError::InvalidPin),
        None => None,
    };
    if let Some(tokens) = tc_line {
        let [_, raw] = tokens.as_slice() else {
            return Err(GoPinParseError::InvalidPin);
        };
        let (ver, key) = raw
            .strip_prefix("go")
            .and_then(exact_go_release)
            .ok_or(GoPinParseError::InvalidPin)?;
        if go_ver.is_some_and(|(go_key, _)| key < go_key) {
            return Err(GoPinParseError::InvalidPin);
        }
        return Ok(Some(ver));
    }
    Ok(go_ver.and_then(|(_, exact)| exact))
}

/// Parses a `go` directive version into its comparable key and optional exact `1.X.Y` pin.
fn parse_go_ver(raw: &str) -> Option<(VersionKey, Option<&str>)> {
    if let Some((ver, key)) = exact_go_release(raw) {
        return Some((key, Some(ver)));
    }
    let (major, minor) = raw.split_once('.')?;
    (major == "1").then(|| Some(((1, canonical_u64(minor)?, 0), None)))?
}

/// Parses a canonical 3-part stable Go release `1.X.Y`.
fn exact_go_release(raw: &str) -> Option<(&str, VersionKey)> {
    let mut parts = raw.split('.');
    let (m, n, p) = (parts.next()?, parts.next()?, parts.next()?);
    (m == "1" && parts.next().is_none())
        .then(|| Some((raw, (1, canonical_u64(n)?, canonical_u64(p)?))))?
}

/// Parses a decimal integer with no leading zeros (except `"0"`).
fn canonical_u64(part: &str) -> Option<u64> {
    let n = part.parse::<u64>().ok()?;
    (n.to_string() == part).then_some(n)
}

/// Splits one line into Go modfile tokens up to any `//` comment.
fn lex_go_mod_line(mut rest: &str) -> Result<Vec<&str>, String> {
    let mut tokens = Vec::new();
    loop {
        rest = rest.trim_start_matches([' ', '\t', '\r']);
        let Some(first) = rest.chars().next() else {
            return Ok(tokens);
        };
        if rest.starts_with("//") {
            return Ok(tokens);
        }
        if rest.starts_with("/*") {
            return Err("mod files must use // comments (not /* */ comments)".to_owned());
        }
        let len = match first {
            '(' | ')' | '[' | ']' | '{' | '}' | ',' => 1,
            '"' | '`' => quoted_len(rest, first)?,
            ch if is_go_mod_ident(ch) => ident_len(rest)?,
            ch => return Err(format!("unexpected input character {ch:?}")),
        };
        tokens.push(&rest[..len]);
        rest = &rest[len..];
    }
}

/// Returns the byte length of a single-line quoted or raw string token.
fn quoted_len(rest: &str, quote: char) -> Result<usize, String> {
    let mut chars = rest.char_indices().skip(1);
    while let Some((idx, ch)) = chars.next() {
        if ch == quote {
            return Ok(idx + 1);
        }
        if ch == '\\' && quote == '"' && chars.next().is_none() {
            break;
        }
    }
    Err("unterminated string".to_owned())
}

/// Returns the byte length of an identifier token, stopping at `//` or delimiter runes.
fn ident_len(rest: &str) -> Result<usize, String> {
    for (idx, ch) in rest.char_indices() {
        let tail = &rest[idx..];
        if tail.starts_with("/*") {
            return Err("mod files must use // comments (not /* */ comments)".to_owned());
        }
        if !is_go_mod_ident(ch) || tail.starts_with("//") {
            return Ok(idx);
        }
    }
    Ok(rest.len())
}

/// Reports whether a character may appear in an unquoted Go modfile identifier.
fn is_go_mod_ident(ch: char) -> bool {
    !matches!(ch, '(' | ')' | '[' | ']' | '{' | '}' | ',')
        && !ch.is_whitespace()
        && !ch.is_control()
}

/// The fields that bind a metadata file to its writer, format, and pinned version.
#[derive(Deserialize)]
struct MetadataIdentity {
    /// Distinguishes Bessemer's metadata from another file with the same name.
    generated_by: String,
    /// Prevents a newer format from being read with older semantics.
    schema: u32,
    /// Must equal the `go.mod` or `go.work` pin that selected the file.
    version: String,
}

/// Fail-closed Go SDK selection errors during package evaluation.
#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Input)]
pub enum GoSdkMetadataError {
    #[error("{0} requires an exact stable Go pin (`toolchain go1.X.Y` or `go 1.X.Y`)")]
    Pin(&'static str),
    #[error("{file} is not valid Go module syntax: {message}")]
    Syntax { file: &'static str, message: String },
    #[error("Go SDK metadata {0:?} is missing; run `bsmr go toolchain`")]
    Missing(PathBuf),
    #[error("Go SDK metadata {0:?} is invalid; run `bsmr go toolchain`")]
    Invalid(PathBuf),
}

/// Declares `cxx`, `python_bootstrap`, `genrule`, `test`, and `remote_test_execution`.
const NATIVE_TOOLS: &str = "load(\"@prelude//toolchains:native.bzl\", __bsmr_native_tools = \"native_tools\")\n__bsmr_native_tools()\n";

/// Renders the targets native frontends append to `package`.
///
/// The root package carries the shared native tools when a native frontend declares a
/// toolchain from it: a Cargo root beside its compiler, or a pinned `go.work` / `go.mod`.
/// The Go SDK itself joins whichever package `toolchains//` names. `bsmr init` aliases
/// `toolchains` to the root cell, so in its layout every contribution lands in one package, once.
pub async fn render(
    ctx: &mut DiceComputations<'_>,
    package: PackageLabel,
    listing: &PackageListing,
) -> bsmr_error::Result<String> {
    if !package.cell_relative_path().is_empty() {
        return Ok(String::new());
    }
    let mut source = String::new();
    let cargo_root = listing.build_source() == PackageBuildSource::Native
        && listing
            .get_file(PackageRelativePath::new("Cargo.toml")?)
            .is_some();
    let go = go_toolchain(ctx, package).await?;
    // A dedicated toolchain cell owns these names. Injecting them into a Go
    // source root as well collides with ordinary targets such as `:test`.
    let go_tools_here = !go.is_empty()
        && (listing
            .get_file(PackageRelativePath::new("go.work")?)
            .is_some()
            || listing
                .get_file(PackageRelativePath::new("go.mod")?)
                .is_some());
    if cargo_root || go_tools_here {
        source.push_str(NATIVE_TOOLS);
    }
    source.push_str(&go);
    Ok(source)
}

/// Declares the pinned Go SDK when `package` is the one `toolchains//` names.
///
/// The root `go.work` (when present) or `go.mod` pin is read before the `toolchains` alias
/// is resolved, so projects without a pin never depend on that alias. The SDK and the
/// bootstrap wrapper are action outputs.
async fn go_toolchain(
    ctx: &mut DiceComputations<'_>,
    package: PackageLabel,
) -> bsmr_error::Result<String> {
    let cells = ctx.get_cell_resolver().await?;
    let work_path = cells.get_cell_path(ProjectRelativePath::unchecked_new("go.work"));
    let (file, source) = if let Some(work) =
        DiceFileComputations::read_file_if_exists(ctx, work_path.as_ref()).await?
    {
        ("go.work", work)
    } else {
        let mod_path = cells.get_cell_path(ProjectRelativePath::unchecked_new("go.mod"));
        let Some(module) =
            DiceFileComputations::read_file_if_exists(ctx, mod_path.as_ref()).await?
        else {
            return Ok(String::new());
        };
        ("go.mod", module)
    };
    let Some(version) = parse_go_pin(&source).map_err(|error| match error {
        GoPinParseError::InvalidPin => GoSdkMetadataError::Pin(file),
        GoPinParseError::Syntax(message) => GoSdkMetadataError::Syntax { file, message },
    })?
    else {
        return Ok(String::new());
    };
    if package.cell_name()
        != cells
            .root_cell_cell_alias_resolver()
            .resolve("toolchains")?
    {
        return Ok(String::new());
    }
    let name = format!("go{version}.json");
    let path = go_sdk_metadata_directory()?.join(FileName::new(&name)?);
    let metadata = ctx.compute(&GoSdkMetadata).await?;
    let lock = metadata
        .get(&name)
        .ok_or_else(|| GoSdkMetadataError::Missing(path.clone().into_path_buf()))?;
    if !serde_json::from_str::<MetadataIdentity>(lock).is_ok_and(|identity| {
        identity.generated_by == "bsmr go toolchain"
            && identity.schema == 1
            && identity.version == version
    }) {
        return Err(GoSdkMetadataError::Invalid(path.into_path_buf()).into());
    }
    Ok(format!(
        "load(\"@prelude//go/native:toolchain.bzl\", __bsmr_native_go_toolchains = \"native_go_toolchains\")\n__bsmr_native_go_toolchains(lock = {})\n",
        serde_json::to_string(&lock)?,
    ))
}
