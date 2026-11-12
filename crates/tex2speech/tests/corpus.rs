// SPDX-License-Identifier: Apache-2.0
//! The construct corpus: one small source per construct in `tests/corpus/`, each with a reviewed transcript in
//! `tests/corpus/expected/`. A missing transcript fails the test and prints what the engine says; a transcript
//! is written only by a person who has read it (set `T2S_BLESS=1` to write the missing ones).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use tex2speech::{Document, Kind, Options, unspeakable_chars};

fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/corpus")
}

fn sources() -> Vec<PathBuf> {
    let mut v = Vec::new();
    for dir in ["latex", "markdown"] {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(corpus().join(dir))
            .expect("corpus folder")
            .map(|e| e.expect("corpus entry is readable").path())
            .filter(|p| p.extension().is_some_and(|e| e == "tex" || e == "md"))
            .collect();
        entries.sort();
        v.extend(entries);
    }
    v
}

/// Problems in the output: markup, hidden text, symbols a voice cannot say. (`MathCAT` replaces the panic hook,
/// so tests collect messages and print them before they assert.)
fn problems(name: &str, doc: &Document) -> Vec<String> {
    let mut out = Vec::new();
    for s in doc.segments() {
        if s.text.contains("HIDDENWORD") {
            out.push(format!("{name}: hidden text spoken: {}", s.text));
        }
        for c in ['\\', '$', '{', '}', '_', '^'] {
            if s.text.contains(c) {
                out.push(format!("{name}: {c:?} leaked into {:?}", s.text));
            }
        }
        let bad = unspeakable_chars(&s.text);
        if !bad.is_empty() {
            out.push(format!("{name}: unspeakable {bad:?} in {:?}", s.text));
        }
        if s.text.trim().is_empty() {
            out.push(format!("{name}: empty segment"));
        }
        // numbers from references and labels stay digits in LaTeX; Markdown prose has none
        if matches!(s.kind, Kind::Sentence)
            && Path::new(name).extension().is_some_and(|e| e == "md")
            && s.text.contains(|c: char| c.is_ascii_digit())
        {
            out.push(format!("{name}: digits left in prose: {:?}", s.text));
        }
    }
    out
}

fn report(failures: &str) {
    if !failures.is_empty() {
        eprintln!("{failures}");
    }
    assert!(failures.is_empty(), "{failures}");
}

#[test]
fn every_corpus_file_gives_its_reviewed_transcript() {
    let bless = std::env::var("T2S_BLESS").is_ok_and(|value| value == "1");
    let mut failures = String::new();
    for path in sources() {
        let name = path
            .file_name()
            .expect("name")
            .to_string_lossy()
            .to_string();
        let doc = Document::parse_file(&path, &Options::new()).expect("reads");
        for p in problems(&name, &doc) {
            let _ = write!(failures, "\n{p}");
        }
        let got = doc.to_text();
        let expected_path = corpus().join("expected").join(format!("{name}.txt"));
        match std::fs::read_to_string(&expected_path) {
            Ok(want) => {
                if want != got {
                    let _ = write!(failures, "\n--- {name}\nexpected:\n{want}got:\n{got}");
                }
            }
            Err(_) if bless => {
                std::fs::create_dir_all(expected_path.parent().expect("dir")).expect("creates");
                std::fs::write(&expected_path, &got).expect("writes");
            }
            Err(_) => {
                let _ = write!(
                    failures,
                    "\n--- {name}: no transcript yet; the engine says:\n{got}"
                );
            }
        }
    }
    report(&failures);
}

#[test]
fn corpus_output_does_not_depend_on_the_thread_count_or_the_run() {
    for path in sources() {
        let a = Document::parse_file(&path, &Options::new().with_threads(1))
            .expect("reads")
            .to_json();
        let b = Document::parse_file(&path, &Options::new().with_threads(4))
            .expect("reads")
            .to_json();
        assert_eq!(a, b, "{}", path.display());
    }
}

#[test]
fn spans_point_into_the_source() {
    for path in sources() {
        let doc = Document::parse_file(&path, &Options::new()).expect("reads");
        for s in doc.segments() {
            if Path::new(&*s.span.file).exists() {
                let len = std::fs::metadata(&*s.span.file).expect("meta").len();
                assert!(
                    s.span.start <= s.span.end && s.span.end as u64 <= len,
                    "{}: {:?}",
                    path.display(),
                    s.span
                );
                assert!(
                    s.span.line >= 1 && s.span.end_line >= s.span.line,
                    "{}: {:?}",
                    path.display(),
                    s.span
                );
            }
        }
    }
}

/// Every command and environment that math-core accepts gives words with no raw symbol.
/// The list `math_core_commands.txt` comes from the `COMMANDS` and `FUNCTIONS` tables and the `ENVIRONMENTS`
/// table in `math-core-0.9.0/src/{commands,environments}.rs` (lines of the form `"name" =>`).
#[test]
fn every_math_core_command_gives_words() {
    let list = std::fs::read_to_string(corpus().join("math_core_commands.txt")).expect("list");
    let forms = |kind: &str, n: &str| -> Vec<String> {
        if kind == "env" {
            let body = if n.contains("cases")
                || n.contains("matrix")
                || n.contains("array")
                || n.contains("align")
                || n.contains("gather")
                || n.contains("multline")
                || n.contains("equation")
            {
                "a & b \\\\ c & d"
            } else {
                "a"
            };
            let arg = if n == "array" || n == "darray" {
                "{cc}"
            } else if n == "subarray" {
                "{c}"
            } else {
                ""
            };
            return vec![format!("\\begin{{{n}}}{arg} {body} \\end{{{n}}}")];
        }
        vec![
            format!("\\{n}"),
            format!("\\{n}{{a}}"),
            format!("\\{n}{{a}}{{b}}"),
            format!("x \\{n} y"),
            format!("\\{n}{{a}}{{b}}{{c}}"),
        ]
    };
    let mut src = String::new();
    let mut jobs = Vec::new();
    for (i, line) in list.lines().enumerate() {
        let Some((kind, name)) = line.split_once(' ') else {
            continue;
        };
        for (k, f) in forms(kind, name).into_iter().enumerate() {
            let tag: String = format!("{i:04}{k}")
                .bytes()
                .map(|d| char::from(b'a' + (d - b'0')))
                .collect();
            let _ = write!(src, "Q{tag} $ {f} $ Z{tag}.\n\n");
            jobs.push((tag, line.to_string(), f));
        }
    }
    let doc = Document::parse_str(&src, tex2speech::Format::Latex, &Options::new());
    // For each command, at least one of its forms must read cleanly.
    let mut ok: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
    let mut worst: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for (tag, line, form) in &jobs {
        let text = doc
            .segments()
            .iter()
            .find(|s| s.text.starts_with(&format!("Q{tag} ")))
            .map_or_default(|s| s.text.clone());
        let clean = !text.is_empty()
            && !text.contains(['\\', '{', '}', '$', '^', '_'])
            && unspeakable_chars(&text).is_empty()
            && text.split_whitespace().count() > 2
            && !text.starts_with(&format!("Q{tag} Z{tag}"));
        let e = ok.entry(line.clone()).or_insert(false);
        *e |= clean;
        if !clean {
            worst.insert(line.clone(), format!("{form} -> {text}"));
        }
    }
    let mut bad: Vec<String> = ok
        .iter()
        .filter(|(_, v)| !**v)
        .map(|(k, _)| format!("{k}: {}", worst[k]))
        .collect();
    bad.sort();
    assert!(
        bad.is_empty(),
        "{} of {} commands do not read cleanly:\n{}",
        bad.len(),
        ok.len(),
        bad.join("\n")
    );
}
