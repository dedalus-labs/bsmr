//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Reads the exact Go SDK pin from a module or workspace file.

//! Extracts one exact stable Go release pin from `go.work` or `go.mod` using
//! the Go Modules Reference lexical grammar, before any Go SDK is materialized.

use std::fs;
use std::path::Path;

use super::GoToolchainError;

/// One exact stable Go release pin and the root file that declared it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ModulePin {
    /// Canonical `1.X.Y` release version without Go's `go` prefix.
    version: String,
    /// Root file name (`go.work` or `go.mod`) that supplied the pin.
    file: &'static str,
}

impl ModulePin {
    /// Returns the canonical `1.X.Y` release version.
    pub(crate) fn version(&self) -> &str {
        &self.version
    }

    /// Returns the root file name (`go.work` or `go.mod`) that supplied the pin.
    pub(crate) fn file(&self) -> &'static str {
        self.file
    }
}

/// Returns the Go pin file name (`go.work` when present, otherwise `go.mod`).
pub(crate) fn pin_file_name(root: &Path) -> &'static str {
    if root.join("go.work").is_file() {
        "go.work"
    } else {
        "go.mod"
    }
}

/// Reads the exact stable pin from `go.work` when present, otherwise `go.mod`.
///
/// An explicit `toolchain go1.X.Y` wins; without one, an exact `go 1.X.Y` line
/// pins that release because Go deletes `toolchain` when both match.
pub(crate) fn module_pin(root: &Path) -> Result<ModulePin, GoToolchainError> {
    let file = pin_file_name(root);
    let path = root.join(file);
    let source = fs::read_to_string(&path).map_err(|error| GoToolchainError::Read {
        path: path.clone(),
        message: error.to_string(),
    })?;
    let version = parse_module_pin(&source, &path)?.ok_or(GoToolchainError::ModulePin(path))?;
    Ok(ModulePin {
        version: version.to_owned(),
        file,
    })
}

/// Extracts an exact stable Go pin from `go.mod` or `go.work` text.
///
/// Returns `Ok(None)` when the file has no `toolchain` line and no exact 3-part
/// `go` line (for example `go 1.26`). Invalid directives or syntax fail closed.
pub(crate) fn parse_module_pin<'a>(
    source: &'a str,
    path: &Path,
) -> Result<Option<&'a str>, GoToolchainError> {
    let pin_err = || GoToolchainError::ModulePin(path.to_owned());
    let syntax_err = |msg: &str| GoToolchainError::ModuleSyntax {
        path: path.to_owned(),
        message: msg.to_owned(),
    };
    let (mut go_line, mut tc_line): (Option<Vec<&str>>, Option<Vec<&str>>) = (None, None);
    let mut in_block = false;
    for raw_line in source.split('\n') {
        let tokens = lex_line(raw_line).map_err(|m| syntax_err(&m))?;
        if tokens.is_empty() {
            continue;
        }
        if in_block {
            if tokens[0] == ")" {
                if tokens.len() > 1 {
                    return Err(syntax_err("expected newline after closing paren"));
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
            return Err(pin_err());
        }
        *slot = Some(tokens);
    }
    if in_block {
        return Err(syntax_err("unterminated block"));
    }
    let go_ver = match go_line.as_deref() {
        Some([_, v]) => Some(parse_go_ver(v).ok_or_else(pin_err)?),
        Some(_) => return Err(pin_err()),
        None => None,
    };
    if let Some(tokens) = tc_line {
        let [_, raw] = tokens.as_slice() else {
            return Err(pin_err());
        };
        let (ver, key) = raw
            .strip_prefix("go")
            .and_then(exact_release)
            .ok_or_else(pin_err)?;
        if go_ver.is_some_and(|(go_key, _)| key < go_key) {
            return Err(pin_err());
        }
        return Ok(Some(ver));
    }
    Ok(go_ver.and_then(|(_, exact)| exact))
}

/// Comparable `(major, minor, patch)` key for Go 1.x versions.
type VersionKey = (u64, u64, u64);

/// Parses a `go` directive version into its comparable key and optional exact `1.X.Y` pin.
fn parse_go_ver(raw: &str) -> Option<(VersionKey, Option<&str>)> {
    if let Some((ver, key)) = exact_release(raw) {
        return Some((key, Some(ver)));
    }
    let (major, minor) = raw.split_once('.')?;
    (major == "1").then(|| Some(((1, canonical_u64(minor)?, 0), None)))?
}

/// Parses a canonical 3-part stable Go release `1.X.Y`.
fn exact_release(raw: &str) -> Option<(&str, VersionKey)> {
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
fn lex_line(mut rest: &str) -> Result<Vec<&str>, String> {
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
            ch if is_ident(ch) => ident_len(rest)?,
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
        if !is_ident(ch) || tail.starts_with("//") {
            return Ok(idx);
        }
    }
    Ok(rest.len())
}

/// Reports whether a character may appear in an unquoted Go modfile identifier.
fn is_ident(ch: char) -> bool {
    !matches!(ch, '(' | ')' | '[' | ']' | '{' | '}' | ',')
        && !ch.is_whitespace()
        && !ch.is_control()
}
