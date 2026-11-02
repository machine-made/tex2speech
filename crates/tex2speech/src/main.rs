// SPDX-License-Identifier: Apache-2.0
//! The `tex2speech` program: translate a `.tex` or `.md` file (or standard input) into text a voice can read.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use tex2speech::{Document, Format, Options};

/// Exit code for a file or input that cannot be read.
const EXIT_INPUT: u8 = 1;
/// Exit code for a command line the program does not accept.
const EXIT_USAGE: u8 = 2;
/// Exit code when the output cannot be written.
const EXIT_OUTPUT: u8 = 3;

const USAGE: &str = "usage: tex2speech [--json] [--warnings] [--lang en] [--lexicon FILE] [--threads N] [--format tex|md] [FILE | -]\n       tex2speech --version";

struct Args {
    json: bool,
    warnings: bool,
    format: Option<Format>,
    lexicon: Option<PathBuf>,
    threads: Option<usize>,
    file: Option<PathBuf>,
}

fn parse(args: &[OsString]) -> Result<Args, String> {
    let mut a = Args {
        json: false,
        warnings: false,
        format: None,
        lexicon: None,
        threads: None,
        file: None,
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.to_str() {
            Some("--json") => a.json = true,
            Some("--warnings") => a.warnings = true,
            Some("--lang") => match it.next().and_then(|v| v.to_str()) {
                Some("en") => {}
                _ => return Err("--lang accepts only en".into()),
            },
            Some("--format") => {
                a.format = Some(match it.next().and_then(|v| v.to_str()) {
                    Some("tex" | "latex") => Format::Latex,
                    Some("md" | "markdown") => Format::Markdown,
                    _ => return Err("--format accepts tex or md".into()),
                });
            }
            Some("--lexicon") => {
                a.lexicon = Some(it.next().ok_or("--lexicon needs a file")?.clone().into());
            }
            Some("--threads") => {
                a.threads = Some(
                    it.next()
                        .and_then(|v| v.to_str())
                        .and_then(|v| v.parse::<usize>().ok())
                        .filter(|n| (1..=8).contains(n))
                        .ok_or("--threads needs a number from 1 to 8")?,
                );
            }
            Some(s) if s.starts_with("--") => return Err(format!("unknown option {s}")),
            _ => {
                // a file name may hold bytes that are not UTF-8: keep it as the system gave it
                if a.file.replace(PathBuf::from(arg)).is_some() {
                    return Err("only one input is accepted".into());
                }
            }
        }
    }
    Ok(a)
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--version" {
        println!("tex2speech {}", tex2speech::version());
        return ExitCode::SUCCESS;
    }
    let a = match parse(&args) {
        Ok(a) if a.file.is_some() || a.format.is_some() => a,
        Ok(_) => {
            eprintln!("{USAGE}");
            return ExitCode::from(EXIT_USAGE);
        }
        Err(e) => {
            eprintln!("{USAGE}\ntex2speech: {e}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    let mut opts = Options::new();
    if let Some(n) = a.threads {
        opts = opts.with_threads(n);
    }
    if let Some(path) = &a.lexicon {
        match std::fs::read(path) {
            Ok(b) => opts = opts.with_lexicon(String::from_utf8_lossy(&b).into_owned()),
            Err(e) => {
                eprintln!("tex2speech: cannot read {}: {e}", path.display());
                return ExitCode::from(EXIT_INPUT);
            }
        }
    }
    let doc = match a.file.as_deref() {
        Some(path) if path != std::path::Path::new("-") => {
            let result = match a.format {
                Some(fmt) => Document::parse_file_as(path, fmt, &opts),
                None => Document::parse_file(path, &opts),
            };
            match result {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("tex2speech: {e}");
                    return ExitCode::from(EXIT_INPUT);
                }
            }
        }
        _ => {
            let mut bytes = Vec::new();
            if let Err(e) = std::io::stdin().read_to_end(&mut bytes) {
                eprintln!("tex2speech: cannot read standard input: {e}");
                return ExitCode::from(EXIT_INPUT);
            }
            let fmt = a.format.unwrap_or(Format::Latex);
            Document::parse_bytes(&bytes, fmt, &opts.with_name("<stdin>"))
        }
    };
    if a.warnings {
        for w in doc.warnings() {
            eprintln!(
                "warning: {}:{}: {}: {}",
                w.file, w.line, w.construct, w.message
            );
        }
    }
    let out = if a.json { doc.to_json() } else { doc.to_text() };
    let mut stdout = std::io::stdout().lock();
    if stdout
        .write_all(out.as_bytes())
        .and_then(|()| stdout.flush())
        .is_err()
    {
        return ExitCode::from(EXIT_OUTPUT);
    }
    ExitCode::SUCCESS
}
