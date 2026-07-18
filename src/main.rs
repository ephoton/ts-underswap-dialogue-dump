// Copyright (C) 2026 photon.
//
// This file is part of tsus_dialoguedump.
//
// tsus_dialoguedump is free software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version 3 of the License
// only.
//
// tsus_dialoguedump is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even
// the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General
// Public License for more details.
//
// You should have received a copy of the GNU General Public License along with tsus_dialoguedump. If not, see
// <https://www.gnu.org/licenses/>.

use std::{fs, path::PathBuf};

use argh::FromArgs;
use logos::Logos;
use serde::Serialize;
use snafu::{ResultExt, Whatever};
use walkdir_minimal::WalkDir;

#[derive(Logos, Debug, Clone, Copy, PartialEq, Eq)]
#[logos(skip(r"([ \r\t\n\f]+|;[^\n]*)", allow_greedy = true))]
enum TokenType {
    #[token("pushs")]
    PushString,
    #[regex("[_a-zA-Z][_a-zA-Z0-9]*")]
    OtherIdent,
    #[token(".")]
    Dot,
    #[regex(r"(-)?[0-9]+")]
    IntLiteral,
    #[regex(r#""([^"\\\x00-\x1F]|\\(["\\bnfrt/]|u[a-fA-F0-9]{4}))*""#)]
    StringLiteral,
}

/// Extract strings from decompiled Diannex.
#[derive(FromArgs)]
struct Opts {
    /// folder to recursively search for .asm and .def files.
    #[argh(positional)]
    folder: PathBuf,
}

#[derive(Debug, Serialize)]
struct DialogueSegment {
    name: String,
    children: Vec<String>,
}

fn process_line(line: &str) -> String {
    let mut result = String::new();
    let mut ignore = false;
    for c in line.chars() {
        if c == '`' {
            ignore = !ignore;
        } else if !ignore {
            result.push(c);
        }
    }
    result
        .replace("\\\\", "")
        .replace("\\\"", "\"")
        // .replace("\\?", "?")
        .replace("#", "\n")
}

fn extract_strings(asm: &str) -> Vec<String> {
    let mut lexer = TokenType::lexer(asm);
    let mut result = vec![];
    let mut previous = None;
    while let Some(token_ty_result) = lexer.next() {
        let Ok(token_ty) = token_ty_result else {
            panic!("Lexer cannot handle some files");
        };
        if matches!(
            (previous, token_ty),
            (Some(TokenType::PushString), TokenType::StringLiteral)
        ) {
            // eprintln!("{}", lexer.slice());
            result.push(process_line(&lexer.slice()[..lexer.span().len() - 1][1..]));
        }
        previous = Some(token_ty);
    }
    result
}

#[snafu::report]
fn main() -> Result<(), Whatever> {
    let opts: Opts = argh::from_env();

    let mut segments = vec![];

    for entry in WalkDir::new(opts.folder).whatever_context("Failed to access requested folder")? {
        let entry = entry.whatever_context("Failed to access entry of folder")?;
        if !entry.file_type().is_ok_and(|file_type| file_type.is_file()) {
            continue;
        }
        eprintln!("Processing {}", entry.path().display());
        let contents = fs::read_to_string(entry.path())
            .whatever_context(format!("Failed to read {}", entry.path().display()))?;

        match entry.path().extension().and_then(|s| s.to_str()) {
            Some("def") => {
                let name = entry
                    .path()
                    .file_stem()
                    .expect("idk")
                    .to_string_lossy()
                    .to_string();
                segments.push(DialogueSegment {
                    name,
                    children: vec![contents],
                });
            }
            Some("asm") => {
                let name = entry
                    .path()
                    .file_stem()
                    .expect("idk")
                    .to_string_lossy()
                    .to_string();
                segments.push(DialogueSegment {
                    name,
                    children: extract_strings(&contents),
                });
            }
            _ => {}
        }
    }

    let json = serde_json::to_string_pretty(&segments).whatever_context("Failed to serialize")?;

    println!("{json}");

    Ok(())
}
