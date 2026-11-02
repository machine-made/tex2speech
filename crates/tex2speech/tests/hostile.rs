// SPDX-License-Identifier: Apache-2.0
#![allow(clippy::cast_possible_truncation)] // the generator takes remainders below a small length
//! Hostile input with a bound on time and on memory: a small file must not write gigabytes, run for minutes or make
//! the reader panic. Memory is the peak of the process, read from the system after each case.

use std::fmt::Write as _;
use std::time::{Duration, Instant};
use tex2speech::{Document, Format, Options};

/// The most memory the process has held so far, in bytes (Linux: `VmHWM`), or `None` where the system gives no such
/// figure; there only the time is bounded, and the log line says so. It never falls, so a case that goes over the
/// bound fails, and so does every later one.
fn peak_bytes() -> Option<usize> {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("VmHWM:"))
                .and_then(|v| v.split_whitespace().next()?.parse::<usize>().ok())
        })
        .map(|kib| kib * 1024)
}

/// Longest time one case may take and the largest memory the process may reach. The test profile is unoptimised, so
/// the time is loose.
const MAX_TIME: Duration = Duration::from_secs(60);
const MAX_BYTES: usize = 1536 * 1024 * 1024;

/// Run one case; fail when it panics inside the reader, takes too long or the process passes the memory bound.
fn bounded(name: &str, src: &str, format: Format, opts: &Options) -> Document {
    let start = Instant::now();
    let doc = Document::parse_str(src, format, opts);
    let took = start.elapsed();
    let peak = peak_bytes();
    // on Linux a missing figure is a failure of the check, not a pass
    assert!(
        peak.is_some() || !cfg!(target_os = "linux"),
        "{name}: /proc/self/status gives no VmHWM"
    );
    let peak_text = peak.map_or_else(
        || "memory NOT MEASURED on this system".to_string(),
        |b| format!("process peak {} MiB", b >> 20),
    );
    eprintln!(
        "{name}: {:.2} s, {peak_text}, {} segments",
        took.as_secs_f64(),
        doc.segments().len()
    );
    assert!(
        doc.warnings().iter().all(|w| w.construct != "internal"),
        "{name}: the reader failed: {:?}",
        doc.warnings().iter().find(|w| w.construct == "internal")
    );
    assert!(took < MAX_TIME, "{name}: took {took:?}");
    if let Some(peak) = peak {
        assert!(
            peak < MAX_BYTES,
            "{name}: the process held {} MiB",
            peak >> 20
        );
    }
    doc
}

fn tex(name: &str, src: &str) -> Document {
    bounded(name, src, Format::Latex, &Options::new())
}

fn md(name: &str, src: &str) -> Document {
    bounded(name, src, Format::Markdown, &Options::new())
}

#[test]
fn nested_macro_levels_stay_inside_the_byte_budget() {
    let body = "word ".repeat(20_000);
    let mut src = format!("\\newcommand{{\\mA}}{{{body}}}\n");
    for (lo, hi) in [("mA", "mB"), ("mB", "mC"), ("mC", "mD"), ("mD", "mE")] {
        let _ = writeln!(
            src,
            "\\newcommand{{\\{hi}}}{{{}}}",
            format!("\\{lo}").repeat(100)
        );
    }
    src.push_str("\\begin{document}\n\\mE\n\\end{document}\n");
    let d = tex("macro levels", &src);
    assert!(d.warnings().iter().any(|w| w.construct == "macro"));
}

#[test]
fn nested_environments_and_math_macros_stay_inside_the_byte_budget() {
    let big = "x + ".repeat(20_000);
    let mut src = format!("\\newcommand{{\\mA}}{{{big} y}}\n");
    for (lo, hi) in [("mA", "mB"), ("mB", "mC"), ("mC", "mD")] {
        let _ = writeln!(
            src,
            "\\newcommand{{\\{hi}}}{{{}}}",
            format!("\\{lo} + ").repeat(100)
        );
    }
    src.push_str("\\newenvironment{ee}{\\mA}{\\mA}\n");
    src.push_str("\\begin{document}\n$\\mD$\n\\begin{ee}text\\end{ee}\n\\end{document}\n");
    tex("math and environment macros", &src);
}

#[test]
fn a_file_read_over_and_over_stays_inside_the_byte_budget() {
    let dir = std::env::temp_dir().join(format!("tex2speech-hostile-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("folder");
    std::fs::write(dir.join("big.tex"), "word ".repeat(400_000)).expect("writes");
    let opts = Options::new().with_base_dir(&dir);
    let src = "\\input{big}\n".repeat(5_000);
    let d = bounded("input again", &src, Format::Latex, &opts);
    assert!(d.warnings().iter().any(|w| w.construct == "macro"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn deep_and_repeated_structure_is_bounded() {
    tex("braces", &"{".repeat(200_000));
    tex("emph", &"\\emph{".repeat(50_000));
    tex("dollars", &"$".repeat(100_001));
    tex("begin", &"\\begin{x}".repeat(20_000));
    tex(
        "math nesting",
        &format!("${}x{}$", "{\\rightarrow".repeat(5_000), "}".repeat(5_000)),
    );
    tex("left", &format!("$${}x$$", "\\left(".repeat(5_000)));
    tex("def chain", &"\\def\\a#1{\\a{#1#1}}\\a{x}".repeat(100));
    tex(
        "tabular",
        &format!(
            "\\begin{{tabular}}{{c}}{}\\end{{tabular}}",
            "a & ".repeat(100_000)
        ),
    );
}

#[test]
fn markdown_that_scans_ahead_is_bounded() {
    // each backtick run looks for its partner to the end of the text
    let mut ticks = String::new();
    for n in (1..=1500).rev() {
        ticks.push_str(&"`".repeat(n));
        ticks.push(' ');
    }
    md("decreasing backtick runs", &ticks);
    md("brackets", &"[".repeat(200_000));
    md("emphasis", &"*a ".repeat(200_000));
    md("quotes", &"> ".repeat(50_000));
    md("fences", &"```\n".repeat(100_000));
    md("math", &"$a ".repeat(200_000));
    md("links", &"[a](".repeat(100_000));
    md("long line", &"word ".repeat(1_000_000));
}

/// Characters that are one byte long in no encoding: an offset taken from a byte count lands inside them.
const WIDE: [&str; 4] = ["\u{e9}", "\u{20ac}", "\u{1d538}", "\u{2062}"];

#[test]
fn a_wide_character_anywhere_in_a_document_never_panics() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus");
    let mut files = Vec::new();
    for sub in ["latex", "markdown"] {
        for e in std::fs::read_dir(dir.join(sub)).expect("folder") {
            let p = e.expect("entry").path();
            if p.is_file() {
                files.push(p);
            }
        }
    }
    assert!(files.len() > 10);
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    let mut cases = 0usize;
    for p in files {
        let format = Format::from_path(&p);
        let text = std::fs::read_to_string(&p).expect("UTF-8 corpus file");
        let boundaries: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
        // about 120 insertions per file, at places a generator picks, plus one after every backslash
        let mut spots: Vec<usize> = (0..120)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                boundaries[(state % boundaries.len() as u64) as usize]
            })
            .collect();
        spots.extend(text.match_indices('\\').map(|(i, _)| i + 1).step_by(7));
        for at in spots {
            if !text.is_char_boundary(at) {
                continue;
            }
            let wide = WIDE[cases % WIDE.len()];
            let mutated = format!("{}{wide}{}", &text[..at], &text[at..]);
            let doc = Document::parse_str(
                &mutated,
                format,
                &Options::new().with_base_dir(dir.join("latex")),
            );
            if let Some(w) = doc.warnings().iter().find(|w| w.construct == "internal") {
                panic!("{}: {wide:?} at byte {at}: {w:?}", p.display());
            }
            cases += 1;
        }
    }
    eprintln!("{cases} documents with one wide character each");
}
