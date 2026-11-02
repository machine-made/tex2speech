// SPDX-License-Identifier: Apache-2.0
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::if_same_then_else
)] // a report tool: counts travel as text between processes
//! Run the engine on folders of documents and report crashes, hangs, leaks and warnings by construct.
//!
//! Usage: `corpus_sweep [--top N] [--timeout SECONDS] FOLDER...`. Each folder is one document, or a folder of
//! documents: every `.tex` file holding `\begin{document}` and every `.md` file is a root. A root runs in a
//! child process (`corpus_sweep --one FILE`) so that a crash or a hang cannot stop the sweep. The engine reads
//! the documents and nothing is executed.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tex2speech::{Document, Options};

fn roots(folder: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    let rd = match std::fs::read_dir(folder) {
        Ok(rd) => rd,
        Err(e) => {
            eprintln!("cannot read folder {}: {e}", folder.display());
            return;
        }
    };
    let mut entries: Vec<(PathBuf, bool)> = Vec::new();
    for e in rd {
        match e.and_then(|e| e.file_type().map(|t| (e.path(), t))) {
            Ok((path, t)) if !t.is_symlink() => entries.push((path, t.is_dir())),
            Ok(_) => {}
            Err(e) => eprintln!("cannot read an entry of {}: {e}", folder.display()),
        }
    }
    entries.sort();
    for (p, is_dir) in entries {
        if is_dir && depth < 3 {
            roots(&p, out, depth + 1);
        } else if p.extension().is_some_and(|e| e == "md") {
            out.push(p);
        } else if p.extension().is_some_and(|e| e == "tex") {
            match std::fs::read(&p) {
                Ok(b) if String::from_utf8_lossy(&b).contains("\\begin{document}") => out.push(p),
                Ok(_) => {}
                Err(e) => eprintln!("cannot read {}: {e}", p.display()),
            }
        }
    }
}

/// Phrases that mark a poor reading of an equation, in the order of the report.
const POOR: [&str; 24] = [
    "embellishment",
    "quantity",
    "with * above",
    "array of",
    "open paren",
    "close paren",
    "the fraction with numerator",
    "open brace",
    "not including",
    // the word for a pause or a mark that a person does not say
    "comma",
    "semicolon",
    "back slash",
    "varies with",
    "divides",
    // a power-of reading of a function space, an index or a momentum
    "power of,",
    "permutations of",
    "raised to the exponent",
    "double struck",
    "circled dot operator",
    "nahblah",
    // readings of a subscript, an underline, an implication and an array after a relation that a person does not say
    "end sub",
    "line below",
    "right double arrow",
    "line one",
];

/// Start offsets of `needle` in `hay` that do not touch a letter or digit on a side where `needle` itself
/// ends in one, so that "comma" is not found in "command".
fn bounded_matches(hay: &str, needle: &str) -> Vec<usize> {
    let first_alnum = needle.chars().next().is_some_and(char::is_alphanumeric);
    let last_alnum = needle
        .chars()
        .next_back()
        .is_some_and(char::is_alphanumeric);
    hay.match_indices(needle)
        .map(|(i, _)| i)
        .filter(|&i| {
            let before_ok = !first_alnum
                || !hay[..i]
                    .chars()
                    .next_back()
                    .is_some_and(char::is_alphanumeric);
            let after_ok = !last_alnum
                || !hay[i + needle.len()..]
                    .chars()
                    .next()
                    .is_some_and(char::is_alphanumeric);
            before_ok && after_ok
        })
        .collect()
}

/// How often `phrase` occurs in `text`, as whole words. A `*` stands for one to six words.
fn count_phrase(text: &str, phrase: &str) -> usize {
    let Some((head, tail)) = phrase.split_once(" * ") else {
        return bounded_matches(text, phrase).len();
    };
    let mut n = 0;
    let mut rest = text;
    while let Some(a) = bounded_matches(rest, head).first().copied() {
        let after = &rest[a + head.len()..];
        let window: String = after
            .split_whitespace()
            .take(7)
            .collect::<Vec<_>>()
            .join(" ");
        let wanted = format!(" {tail}");
        if !bounded_matches(&window, &wanted).is_empty()
            || (window.starts_with(tail) && !bounded_matches(&window, tail).is_empty())
        {
            n += 1;
        }
        rest = after;
    }
    n
}

/// Child mode: translate one file and print one line per fact.
fn one(path: &str) {
    let start = Instant::now();
    let doc = match Document::parse_file(path, &Options::new()) {
        Ok(d) => d,
        Err(e) => {
            println!("ERROR {e}");
            return;
        }
    };
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    let mut leaks = 0usize;
    let mut symbols: BTreeMap<char, (usize, String)> = BTreeMap::new();
    let mut digits = 0usize;
    let mut poor = [0usize; POOR.len()];
    for s in doc.segments() {
        for (k, p) in POOR.iter().enumerate() {
            poor[k] += count_phrase(&s.text, p);
        }
        if s.text.contains(['\\', '$', '{', '}', '_', '^']) {
            leaks += 1;
        }
        for c in tex2speech::unspeakable_chars(&s.text) {
            let e = symbols.entry(c).or_insert_with(|| {
                let at = s.text.find(c).unwrap_or(0);
                let from = s.text[..at]
                    .char_indices()
                    .rev()
                    .nth(30)
                    .map_or(0, |(i, _)| i);
                (
                    0,
                    format!(
                        "{}:{} {}",
                        s.span.file.rsplit(['/', '\\']).next().unwrap_or(""),
                        s.span.line,
                        s.text[from..]
                            .chars()
                            .take(70)
                            .collect::<String>()
                            .replace('\n', " ")
                    ),
                )
            });
            e.0 += 1;
        }
        if s.kind == tex2speech::Kind::Sentence && s.text.chars().any(|c| c.is_ascii_digit()) {
            digits += 1;
        }
    }
    println!(
        "OK segments={} ms={ms:.0} warnings={} leaks={leaks} digits={digits}",
        doc.segments().len(),
        doc.warning_count()
    );
    for (c, (n, ex)) in symbols {
        println!("SYMBOL {} {n} {ex}", c as u32);
    }
    for (k, _) in POOR.iter().enumerate() {
        println!("POOR {k} {}", poor[k]);
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for w in doc.warnings() {
        let key = format!("{}: {}", w.construct, group(&w.message));
        *counts.entry(key).or_default() += 1;
    }
    for (k, n) in counts {
        println!("WARN {n} {k}");
    }
}

/// Reduce a message to its construct name, for example "command \foo is not understood" to "\foo".
fn group(msg: &str) -> String {
    if let Some(rest) = msg
        .strip_prefix("command ")
        .or_else(|| msg.strip_prefix("environment "))
        .or_else(|| msg.strip_prefix("conditional "))
    {
        return rest.split(' ').next().unwrap_or(rest).to_string();
    }
    if let Some(rest) = msg.strip_prefix("unknown command ") {
        return rest.split(' ').next().unwrap_or(rest).to_string();
    }
    if msg.starts_with("removed ") {
        return msg.chars().take(11).collect();
    }
    if msg.starts_with("LatexError") {
        return msg.split(',').nth(1).map_or_else(
            || "math error".to_string(),
            |s| s.trim().chars().take(30).collect(),
        );
    }
    msg.chars().take(60).collect()
}

fn usage(why: &str) -> ! {
    eprintln!("corpus_sweep: {why}\nusage: corpus_sweep [--top N] [--timeout SECONDS] FOLDER...");
    std::process::exit(2);
}

#[allow(clippy::too_many_lines)] // one pass: run the children, then print the tables
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() == 2 && args[0] == "--one" {
        one(&args[1]);
        return;
    }
    let mut top = 25usize;
    let mut timeout = 120u64;
    let mut folders = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--top" => match it.next().and_then(|v| v.parse().ok()) {
                Some(v) => top = v,
                None => usage("--top needs a whole number"),
            },
            "--timeout" => match it.next().and_then(|v| v.parse().ok()) {
                Some(v) if v > 0 => timeout = v,
                _ => usage("--timeout needs a whole number of seconds above 0"),
            },
            f if f.starts_with("--") => usage(&format!("unknown option {f}")),
            f => folders.push(PathBuf::from(f)),
        }
    }
    if folders.is_empty() {
        usage("give at least one folder");
    }
    let exe = std::env::current_exe().expect("own path");
    let mut docs: Vec<PathBuf> = Vec::new();
    for f in &folders {
        roots(f, &mut docs, 0);
    }
    let (mut crashes, mut hangs, mut errors) = (Vec::new(), Vec::new(), Vec::new());
    let (mut segments, mut warnings, mut leaks, mut digits) = (0usize, 0usize, 0usize, 0usize);
    let mut by_construct: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_category: BTreeMap<String, usize> = BTreeMap::new();
    let mut symbols: BTreeMap<u32, (usize, String)> = BTreeMap::new();
    let mut slowest = (0.0f64, String::new());
    let mut poor_total = [0usize; POOR.len()];
    let mut poor_docs: Vec<(usize, String)> = Vec::new();
    println!(
        "| document | segments | ms | warnings | leaks | poor | status |\n|---|---|---|---|---|---|---|"
    );
    for d in &docs {
        let mut child = Command::new(&exe)
            .arg("--one")
            .arg(d)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("child starts");
        // read the output while the child runs, or a full pipe would stop it
        let reader = child.stdout.take().map(|mut so| {
            std::thread::spawn(move || {
                let mut out = String::new();
                let _ = so.read_to_string(&mut out);
                out
            })
        });
        let start = Instant::now();
        let mut status = None;
        while start.elapsed() < Duration::from_secs(timeout) {
            if let Ok(Some(s)) = child.try_wait() {
                status = Some(s);
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let name = d.display().to_string();
        let Some(st) = status else {
            let _ = child.kill();
            let _ = child.wait();
            if let Some(h) = reader {
                let _ = h.join();
            }
            hangs.push(name.clone());
            println!("| {name} | | | | | | HANG |");
            continue;
        };
        let out = reader.and_then(|h| h.join().ok()).unwrap_or_default();
        if !st.success() {
            crashes.push(name.clone());
            println!("| {name} | | | | | | CRASH ({st}) |");
            continue;
        }
        let mut line_info = String::new();
        let mut row = String::new();
        let mut doc_poor = 0usize;
        for l in out.lines() {
            if let Some(r) = l.strip_prefix("OK ") {
                line_info = r.to_string();
                let get = |k: &str| {
                    r.split(' ')
                        .find_map(|p| p.strip_prefix(k))
                        .and_then(|v| v.parse::<f64>().ok())
                        .unwrap_or(0.0)
                };
                segments += get("segments=") as usize;
                warnings += get("warnings=") as usize;
                leaks += get("leaks=") as usize;
                digits += get("digits=") as usize;
                let ms = get("ms=");
                if ms > slowest.0 {
                    slowest = (ms, name.clone());
                }
                row = format!(
                    "| {name} | {} | {ms:.0} | {} | {} |",
                    get("segments=") as usize,
                    get("warnings=") as usize,
                    get("leaks=") as usize
                );
            } else if let Some(r) = l.strip_prefix("POOR ") {
                let mut p = r.split(' ').filter_map(|v| v.parse::<usize>().ok());
                if let (Some(k), Some(n)) = (p.next(), p.next())
                    && k < POOR.len()
                {
                    poor_total[k] += n;
                    doc_poor += n;
                }
            } else if let Some(r) = l.strip_prefix("ERROR ") {
                errors.push(format!("{name}: {r}"));
            } else if let Some(r) = l.strip_prefix("SYMBOL ") {
                let mut p = r.splitn(3, ' ');
                if let (Some(c), Some(n)) = (
                    p.next().and_then(|v| v.parse::<u32>().ok()),
                    p.next().and_then(|v| v.parse::<usize>().ok()),
                ) {
                    let e = symbols
                        .entry(c)
                        .or_insert((0, p.next().unwrap_or("").to_string()));
                    e.0 += n;
                }
            } else if let Some(r) = l.strip_prefix("WARN ") {
                let (n, key) = r.split_once(' ').unwrap_or(("0", r));
                let n: usize = n.parse().unwrap_or(0);
                *by_construct.entry(key.to_string()).or_default() += n;
                *by_category
                    .entry(key.split(':').next().unwrap_or("").to_string())
                    .or_default() += n;
            }
        }
        let _ = line_info;
        if !row.is_empty() {
            println!("{row} {doc_poor} | ok |");
            poor_docs.push((doc_poor, name));
        }
    }
    println!(
        "\nDocuments: {}  crashes: {}  hangs: {}  read errors: {}",
        docs.len(),
        crashes.len(),
        hangs.len(),
        errors.len()
    );
    println!(
        "Segments: {segments}  warnings: {warnings}  segments with leaked markup: {leaks}  sentences holding digits: {digits}"
    );
    println!("Slowest: {} ({:.0} ms)", slowest.1, slowest.0);
    println!("\nPoor readings (phrase counts over all documents):");
    for (k, p) in POOR.iter().enumerate() {
        println!("  {:6}  {p}", poor_total[k]);
    }
    println!("  {:6}  total", poor_total.iter().sum::<usize>());
    poor_docs.sort_by_key(|d| std::cmp::Reverse(d.0));
    println!("Poor readings per document, worst first:");
    for (n, name) in poor_docs.iter().take(top) {
        println!("  {n:6}  {name}");
    }
    for c in crashes.iter().chain(&hangs) {
        println!("FAILED: {c}");
    }
    for e in &errors {
        println!("ERROR: {e}");
    }
    println!("\nWarnings by category:");
    for (k, n) in &by_category {
        println!("  {n:6}  {k}");
    }
    let mut v: Vec<_> = by_construct.iter().collect();
    v.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    println!("\nMost frequent constructs:");
    for (k, n) in v.into_iter().take(top) {
        println!("  {n:6}  {k}");
    }
    let mut sy: Vec<_> = symbols.iter().collect();
    sy.sort_by_key(|a| std::cmp::Reverse(a.1.0));
    println!(
        "\nSymbols outside the spoken alphabet in the output (code point, count, first example):"
    );
    for (c, (n, ex)) in sy.into_iter().take(top) {
        println!(
            "  U+{c:04X} {}  {n}  {ex}",
            char::from_u32(*c).unwrap_or('?')
        );
    }
    // a script or CI step sees a crash, a hang, a read error or an empty sweep as a failure
    if docs.is_empty() {
        eprintln!("corpus_sweep: no document found");
    }
    if docs.is_empty() || !crashes.is_empty() || !hangs.is_empty() || !errors.is_empty() {
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::count_phrase;

    #[test]
    fn phrases_are_counted() {
        assert_eq!(
            count_phrase("open paren a close paren open paren", "open paren"),
            2
        );
        assert_eq!(count_phrase("the fraction with numerator a", "array of"), 0);
    }

    #[test]
    fn a_star_stands_for_a_few_words() {
        assert_eq!(count_phrase("x with y above it", "with * above"), 1);
        assert_eq!(
            count_phrase(
                "x with right arrow above embellishment above",
                "with * above"
            ),
            1
        );
        assert_eq!(
            count_phrase("with a b c d e f g h above", "with * above"),
            0
        );
        assert_eq!(count_phrase("above with", "with * above"), 0);
    }

    #[test]
    fn phrases_are_whole_words() {
        assert_eq!(count_phrase("the command", "comma"), 0);
        assert_eq!(count_phrase("a comma, then comma", "comma"), 2);
        assert_eq!(count_phrase("x subdivides y", "divides"), 0);
        assert_eq!(count_phrase("without it aboveground", "with * above"), 0);
        assert_eq!(count_phrase("x with y above", "with * above"), 1);
        assert_eq!(count_phrase("x with y aboveboard", "with * above"), 0);
        assert_eq!(count_phrase("a power of, b", "power of,"), 1);
    }
}
