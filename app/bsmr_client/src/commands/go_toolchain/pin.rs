//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Reads the exact SDK pin from a Go module or workspace file.

//! Finds the one top-level `toolchain` directive the way Go's own `modfile` parser does.
//!
//! The pin selects the SDK, so no acquired `go` can parse it, and an ambient `go` would make
//! selection depend on the host. This module therefore ports the statement structure of
//! `golang.org/x/mod/modfile`: the lexer and parser in `read.go` and the `toolchain` rule in
//! `rule.go`. A block's lines belong to its verb, so `ignore ( toolchain )` names a directory,
//! not a directive. Tokens split only at Go's separators, so `toolchain<U+00A0>go1.26.1` is a
//! syntax error, as it is for Go.
//!
//! One approximation remains. Go rejects identifier runes that `unicode.IsPrint` rejects; Rust's
//! standard library has no general-category lookup, so this reader rejects whitespace and control
//! runes, the ones that can split or join tokens, and accepts other non-printing runes. Such a
//! rune cannot occur in `toolchain` or in an accepted version, so it never produces a pin that
//! Go would read differently.

use std::fs;
use std::iter::Peekable;
use std::path::Path;
use std::vec::IntoIter;

use super::GoToolchainError;

/// One lexical token of a `go.mod` or `go.work` file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Token<'a> {
    /// Ends a statement: a newline, or a comment together with its newline.
    End,
    /// Opens a block when it ends its line.
    Open,
    /// Closes a block when it starts its line.
    Close,
    /// An identifier, a quoted string with its quotes, or other punctuation.
    Word(&'a str),
}

impl<'a> Token<'a> {
    /// Returns the text Go records for a token inside a statement.
    fn text(self) -> &'a str {
        match self {
            Token::End => "",
            Token::Open => "(",
            Token::Close => ")",
            Token::Word(word) => word,
        }
    }
}

/// One parsed statement.
enum Statement<'a> {
    /// A top-level line; its first token is the verb.
    Line(Vec<&'a str>),
    /// A block header; its lines belong to the header's verb.
    Block(Vec<&'a str>),
}

/// Reads the exact stable pin from `go.work` when it exists, otherwise from `go.mod`.
///
/// Go reads the workspace file's `toolchain` line in workspace mode and ignores the members'
/// lines, and synchronization hands Go the root `go.work` whenever one exists.
pub(crate) fn module_pin(root: &Path) -> Result<String, GoToolchainError> {
    let workspace = root.join("go.work");
    let path = if workspace.is_file() {
        workspace
    } else {
        root.join("go.mod")
    };
    let source = fs::read_to_string(&path).map_err(|error| GoToolchainError::Read {
        path: path.clone(),
        message: error.to_string(),
    })?;
    let statements = parse(&source).map_err(|message| GoToolchainError::ModuleSyntax {
        path: path.clone(),
        message,
    })?;
    let mut pins = Vec::new();
    for statement in statements {
        match statement {
            Statement::Line(tokens) if tokens[0] == "toolchain" => pins.push(tokens),
            Statement::Block(header) if header[0] == "toolchain" => {
                return Err(GoToolchainError::ModulePin(path));
            }
            Statement::Line(_) | Statement::Block(_) => {}
        }
    }
    let [pin] = pins.as_slice() else {
        return Err(GoToolchainError::ModulePin(path));
    };
    let [_, name] = pin.as_slice() else {
        return Err(GoToolchainError::ModulePin(path));
    };
    canonical_release(name).ok_or(GoToolchainError::ModulePin(path))
}

/// Returns `1.X.Y` for a canonical `go1.X.Y` name, the only form the pin policy accepts.
fn canonical_release(name: &str) -> Option<String> {
    let version = name.strip_prefix("go")?;
    let components = version.split('.').collect::<Vec<_>>();
    let canonical = components.len() == 3
        && components[0] == "1"
        && components.iter().all(|component| {
            component
                .parse::<u64>()
                .is_ok_and(|number| number.to_string() == *component)
        });
    canonical.then(|| version.to_owned())
}

/// Groups tokens into statements as `parseStmt`, `parseLineBlock` and `parseLine` do.
fn parse(source: &str) -> Result<Vec<Statement<'_>>, String> {
    let mut tokens = lex(source)?.into_iter().peekable();
    let mut statements = Vec::new();
    while let Some(first) = tokens.next() {
        if first == Token::End {
            continue;
        }
        let mut words = vec![first.text()];
        loop {
            match tokens.next() {
                None | Some(Token::End) => {
                    statements.push(Statement::Line(words));
                    break;
                }
                Some(Token::Open) if at_end(&mut tokens) => {
                    tokens.next();
                    parse_block(&mut tokens)?;
                    statements.push(Statement::Block(words));
                    break;
                }
                Some(Token::Open) if tokens.peek() == Some(&Token::Close) => {
                    tokens.next();
                    if at_end(&mut tokens) {
                        tokens.next();
                        statements.push(Statement::Block(words));
                        break;
                    }
                    words.extend(["(", ")"]);
                }
                Some(token) => words.push(token.text()),
            }
        }
    }
    Ok(statements)
}

/// Consumes one block body through its closing parenthesis and the line end after it.
fn parse_block(tokens: &mut Peekable<IntoIter<Token<'_>>>) -> Result<(), String> {
    loop {
        match tokens.next() {
            None => return Err("syntax error (unterminated block)".to_owned()),
            Some(Token::End) => {}
            Some(Token::Close) => {
                if !at_end(tokens) {
                    return Err("syntax error (expected newline after closing paren)".to_owned());
                }
                tokens.next();
                return Ok(());
            }
            Some(_) => while !matches!(tokens.next(), None | Some(Token::End)) {},
        }
    }
}

/// Reports whether the next token ends the current line.
fn at_end(tokens: &mut Peekable<IntoIter<Token<'_>>>) -> bool {
    matches!(tokens.peek(), None | Some(Token::End))
}

/// Splits a file into Go `modfile` tokens.
fn lex(source: &str) -> Result<Vec<Token<'_>>, String> {
    let mut tokens = Vec::new();
    let mut rest = source;
    loop {
        rest = rest.trim_start_matches([' ', '\t', '\r']);
        let Some(first) = rest.chars().next() else {
            return Ok(tokens);
        };
        if let Some(comment) = rest.strip_prefix("//") {
            rest = comment.split_once('\n').map_or("", |(_, after)| after);
            tokens.push(Token::End);
            continue;
        }
        if rest.starts_with("/*") {
            return Err("mod files must use // comments (not /* */ comments)".to_owned());
        }
        let length = match first {
            '\n' => {
                tokens.push(Token::End);
                1
            }
            '(' => {
                tokens.push(Token::Open);
                1
            }
            ')' => {
                tokens.push(Token::Close);
                1
            }
            '[' | ']' | '{' | '}' | ',' => {
                tokens.push(Token::Word(&rest[..1]));
                1
            }
            '"' | '`' => {
                let length = quoted_length(rest, first)?;
                tokens.push(Token::Word(&rest[..length]));
                length
            }
            first if identifier_rune(first) => {
                let length = identifier_length(rest)?;
                tokens.push(Token::Word(&rest[..length]));
                length
            }
            first => return Err(format!("unexpected input character {first:?}")),
        };
        rest = &rest[length..];
    }
}

/// Returns the byte length of a quoted string, quotes included.
fn quoted_length(rest: &str, quote: char) -> Result<usize, String> {
    let mut characters = rest.char_indices().skip(1);
    while let Some((index, character)) = characters.next() {
        if character == '\n' {
            return Err("unexpected newline in string".to_owned());
        }
        if character == quote {
            return Ok(index + 1);
        }
        if character == '\\' && quote != '`' && characters.next().is_none() {
            break;
        }
    }
    Err("unexpected EOF in string".to_owned())
}

/// Returns the byte length of an identifier, which a `//` comment can end.
fn identifier_length(rest: &str) -> Result<usize, String> {
    for (index, character) in rest.char_indices() {
        let tail = &rest[index..];
        if tail.starts_with("/*") {
            return Err("mod files must use // comments (not /* */ comments)".to_owned());
        }
        if !identifier_rune(character) || tail.starts_with("//") {
            return Ok(index);
        }
    }
    Ok(rest.len())
}

/// Mirrors Go's `isIdent` except for the documented non-printing approximation.
fn identifier_rune(character: char) -> bool {
    !matches!(character, ' ' | '(' | ')' | '[' | ']' | '{' | '}' | ',')
        && !character.is_whitespace()
        && !character.is_control()
}
