// SPDX-License-Identifier: Apache-2.0
//! Input as real machines produce it: Windows line endings, byte-order marks, 8-bit encodings, Windows paths, empty
//! and binary files. Every test builds its files in code, so that line endings stay as written whatever git does.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use tex2speech::{Document, Format, Options};

const LATEX: &str = "\\documentclass{article}\n\\newcommand{\\vel}[1]{\\mathbf{%\n  #1}}\n\\title{A Title}\n\\begin{document}\n\\maketitle\n\\section{Intro}\nFirst paragraph with $x = 1$ and a\nbroken line.\n\nSecond paragraph after a blank line.\n\\begin{verbatim}\ncode line one\n\ncode line two\n\\end{verbatim}\nDisplay:\n\\[\n  \\vel{u} \\cdot \\nabla p = 0,\n\\]\n\\begin{align}\na &= b \\\\\nc &= d\n\\end{align}\n% a comment line\nText after comment, and a note\\footnote{Note\ntext.} done.\n\\begin{itemize}\n\\item one\n\\item two\n\\end{itemize}\n\\end{document}\n";

const MARKDOWN: &str = "# Title\n\nText with `code` and $a = b$ here.\n\n```python\nx = 1\n\ny = 2\n```\n\nAfter the fence.\n\n$$\nx = 1\n$$\n\n- one\n- two\n";

fn crlf(s: &str) -> String {
    s.replace('\n', "\r\n")
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tex2speech-input-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("temp folder");
    d
}

fn lines(d: &Document) -> Vec<(usize, usize)> {
    d.segments()
        .iter()
        .map(|s| (s.span.line, s.span.end_line))
        .collect()
}

#[test]
fn windows_line_endings_read_like_unix_ones() {
    for (src, fmt) in [(LATEX, Format::Latex), (MARKDOWN, Format::Markdown)] {
        let lf = Document::parse_bytes(src.as_bytes(), fmt, &Options::new());
        let cr = Document::parse_bytes(crlf(src).as_bytes(), fmt, &Options::new());
        assert_eq!(lf.to_text(), cr.to_text(), "{fmt:?}");
        assert_eq!(lines(&lf), lines(&cr), "{fmt:?}");
        assert_eq!(lf.warning_count(), cr.warning_count(), "{fmt:?}");
        assert!(!lf.to_text().contains("code line"), "verbatim is skipped");
        assert!(!cr.to_text().contains('\r'), "no carriage return is spoken");
    }
}

#[test]
fn old_mac_line_endings_do_not_stop_the_reader() {
    let d = Document::parse_bytes(
        LATEX.replace('\n', "\r").as_bytes(),
        Format::Latex,
        &Options::new(),
    );
    assert!(!d.to_text().contains('\r'));
}

#[test]
fn every_construct_corpus_file_reads_the_same_with_windows_line_endings() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus");
    for sub in ["latex", "markdown"] {
        for e in std::fs::read_dir(dir.join(sub)).expect("folder") {
            let p = e.expect("folder entry").path();
            if !p.is_file() {
                continue;
            }
            let fmt = Format::from_path(&p);
            let bytes = std::fs::read(&p).expect("reads");
            let text = String::from_utf8(bytes).expect("corpus is UTF-8");
            let o = Options::new().with_base_dir(p.parent().expect("folder"));
            let a = Document::parse_str(&text, fmt, &o).to_text();
            let b = Document::parse_str(&crlf(&text), fmt, &o).to_text();
            assert_eq!(a, b, "{}", p.display());
        }
    }
}

#[test]
fn a_byte_order_mark_is_not_part_of_the_text_and_offsets_stay_in_the_file() {
    for (src, fmt) in [(LATEX, Format::Latex), (MARKDOWN, Format::Markdown)] {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(src.as_bytes());
        let plain = Document::parse_bytes(src.as_bytes(), fmt, &Options::new());
        let marked = Document::parse_bytes(&bytes, fmt, &Options::new());
        assert_eq!(plain.to_text(), marked.to_text(), "{fmt:?}");
        assert_eq!(lines(&plain), lines(&marked), "{fmt:?}");
        for (p, m) in plain.segments().iter().zip(marked.segments()) {
            assert_eq!(p.span.start + 3, m.span.start, "{fmt:?}");
            assert_eq!(p.span.end + 3, m.span.end, "{fmt:?}");
        }
        // the same when the caller already turned the bytes into a string
        let s = String::from_utf8(bytes).expect("utf8");
        let again = Document::parse_str(&s, fmt, &Options::new());
        assert_eq!(plain.to_text(), again.to_text(), "{fmt:?}");
    }
}

#[test]
fn latin1_and_windows_1252_files_are_decoded() {
    // declared: no warning
    let declared = b"\\documentclass{article}\n\\usepackage[latin1]{inputenc}\n\\begin{document}\nCaf\xE9 \x93quoted\x94 na\xEFve \x96 dash.\n\\end{document}\n";
    let d = Document::parse_bytes(declared, Format::Latex, &Options::new());
    assert_eq!(d.to_text(), "Caf\u{e9} quoted na\u{ef}ve - dash.\n");
    assert!(d.warnings().is_empty(), "{:?}", d.warnings());
    // not declared: the guess is reported
    let guessed = b"\\documentclass{article}\n\\begin{document}\nCaf\xE9.\n\\end{document}\n";
    let d = Document::parse_bytes(guessed, Format::Latex, &Options::new());
    assert_eq!(d.to_text(), "Caf\u{e9}.\n");
    assert_eq!(d.warnings().len(), 1);
    assert_eq!(d.warnings()[0].construct, "encoding");
    assert_eq!(d.warnings()[0].line, 1);
    // Latin-9 has the ligature OE where Latin-1 has a fraction
    let l9 = b"\\usepackage[latin9]{inputenc}\n\\begin{document}\n\xBCuvre.\n\\end{document}\n";
    let d = Document::parse_bytes(l9, Format::Latex, &Options::new());
    assert_eq!(d.to_text(), "\u{152}uvre.\n");
    // an encoding with no table is read as Windows-1252, with a warning
    let l2 = b"\\usepackage[latin2]{inputenc}\n\\begin{document}\nx \xE8.\n\\end{document}\n";
    let d = Document::parse_bytes(l2, Format::Latex, &Options::new());
    assert!(d.warnings().iter().any(|w| w.construct == "encoding"));
}

#[test]
fn a_file_on_disk_in_latin1_reads_through_input_and_parse_file() {
    let dir = tmp("latin1");
    std::fs::write(
        dir.join("main.tex"),
        b"\\documentclass{article}\n\\begin{document}\nMain.\n\\input{part}\n\\end{document}\n",
    )
    .expect("writes");
    std::fs::write(dir.join("part.tex"), b"Part: caf\xE9 na\xEFve.\n").expect("writes");
    let d = Document::parse_file(dir.join("main.tex"), &Options::new()).expect("reads");
    assert!(
        d.to_text().contains("caf\u{e9} na\u{ef}ve"),
        "{}",
        d.to_text()
    );
    assert!(
        d.warnings()
            .iter()
            .any(|w| w.construct == "encoding" && w.file.ends_with("part.tex"))
    );
}

#[test]
fn utf16_with_a_mark_is_read() {
    let mut bytes = vec![0xFF, 0xFE];
    for u in
        "\\documentclass{article}\n\\begin{document}\nHello $x$.\n\\end{document}\n".encode_utf16()
    {
        bytes.extend_from_slice(&u.to_le_bytes());
    }
    let d = Document::parse_bytes(&bytes, Format::Latex, &Options::new());
    assert_eq!(d.to_text(), "Hello x.\n");
}

#[test]
fn invalid_utf8_never_stops_a_run() {
    // every byte value, in runs, in both formats
    let mut bytes = Vec::new();
    for round in 0..4u8 {
        for b in 0..=255u8 {
            bytes.push(b.wrapping_add(round));
            if b % 16 == 0 {
                bytes.extend_from_slice(b" word $x$ \n\n");
            }
        }
    }
    for fmt in [Format::Latex, Format::Markdown] {
        let d = Document::parse_bytes(&bytes, fmt, &Options::new());
        let _ = d.to_text();
        let _ = d.to_json();
    }
}

#[test]
fn an_empty_file_a_preamble_only_file_and_a_binary_file_give_no_text_and_no_crash() {
    let empty = Document::parse_bytes(b"", Format::Latex, &Options::new());
    assert_eq!(empty.to_text(), "");
    assert_eq!(empty.warnings().len(), 0);
    let empty_md = Document::parse_bytes(b"", Format::Markdown, &Options::new());
    assert_eq!(empty_md.to_text(), "");
    let pre = Document::parse_bytes(
        b"\\documentclass{article}\n\\usepackage{amsmath}\n\\newcommand{\\a}{HIDDENWORD}\n",
        Format::Latex,
        &Options::new(),
    );
    assert_eq!(pre.to_text(), "");
    let mut binary = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n1 0 obj\n<< /Length 12 >>\nstream\n".to_vec();
    binary.extend((0..=255u8).cycle().take(4000));
    let bin = Document::parse_bytes(&binary, Format::Latex, &Options::new());
    assert_eq!(bin.to_text(), "");
    assert_eq!(bin.warnings().len(), 1);
    assert_eq!(bin.warnings()[0].construct, "encoding");
    let nul = Document::parse_bytes(b"text\0more text\n", Format::Markdown, &Options::new());
    assert_eq!(nul.to_text(), "");
}

/// A small JSON checker: the text must be one valid value.
#[allow(clippy::too_many_lines)] // one grammar, four small helpers
fn valid_json(s: &str) -> Result<(), String> {
    fn ws(b: &[u8], i: &mut usize) {
        while *i < b.len() && matches!(b[*i], b' ' | b'\n' | b'\r' | b'\t') {
            *i += 1;
        }
    }
    fn string(b: &[u8], i: &mut usize) -> Result<(), String> {
        *i += 1;
        while *i < b.len() {
            match b[*i] {
                b'"' => {
                    *i += 1;
                    return Ok(());
                }
                b'\\' => match b.get(*i + 1) {
                    Some(b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't') => *i += 2,
                    Some(b'u')
                        if b.len() > *i + 5
                            && b[*i + 2..*i + 6].iter().all(u8::is_ascii_hexdigit) =>
                    {
                        *i += 6;
                    }
                    _ => return Err(format!("bad escape at {i}")),
                },
                c if c < 0x20 => return Err(format!("control character at {i}")),
                _ => *i += 1,
            }
        }
        Err("unterminated string".into())
    }
    fn value(b: &[u8], i: &mut usize) -> Result<(), String> {
        ws(b, i);
        match b.get(*i) {
            Some(b'{') => {
                *i += 1;
                ws(b, i);
                if b.get(*i) == Some(&b'}') {
                    *i += 1;
                    return Ok(());
                }
                loop {
                    ws(b, i);
                    if b.get(*i) != Some(&b'"') {
                        return Err(format!("key expected at {i}"));
                    }
                    string(b, i)?;
                    ws(b, i);
                    if b.get(*i) != Some(&b':') {
                        return Err(format!("colon expected at {i}"));
                    }
                    *i += 1;
                    value(b, i)?;
                    ws(b, i);
                    match b.get(*i) {
                        Some(b',') => *i += 1,
                        Some(b'}') => {
                            *i += 1;
                            return Ok(());
                        }
                        _ => return Err(format!("comma or brace expected at {i}")),
                    }
                }
            }
            Some(b'[') => {
                *i += 1;
                ws(b, i);
                if b.get(*i) == Some(&b']') {
                    *i += 1;
                    return Ok(());
                }
                loop {
                    value(b, i)?;
                    ws(b, i);
                    match b.get(*i) {
                        Some(b',') => *i += 1,
                        Some(b']') => {
                            *i += 1;
                            return Ok(());
                        }
                        _ => return Err(format!("comma or bracket expected at {i}")),
                    }
                }
            }
            Some(b'"') => string(b, i),
            Some(b't') if b[*i..].starts_with(b"true") => {
                *i += 4;
                Ok(())
            }
            Some(b'f') if b[*i..].starts_with(b"false") => {
                *i += 5;
                Ok(())
            }
            Some(b'n') if b[*i..].starts_with(b"null") => {
                *i += 4;
                Ok(())
            }
            Some(c) if c.is_ascii_digit() || *c == b'-' => {
                while *i < b.len() && (b[*i].is_ascii_digit() || b"-+.eE".contains(&b[*i])) {
                    *i += 1;
                }
                Ok(())
            }
            _ => Err(format!("value expected at {i}")),
        }
    }
    let b = s.as_bytes();
    let mut i = 0;
    value(b, &mut i)?;
    ws(b, &mut i);
    if i == b.len() {
        Ok(())
    } else {
        Err(format!("text after the value at {i}"))
    }
}

#[test]
fn windows_paths_give_valid_json() {
    for name in [
        r"C:\Users\Ana María\papers\main.tex",
        r"\\wsl.localhost\Ubuntu\home\ana\main.tex",
        r"\\?\D:\data\a b\main.tex",
        "relative\\sub folder\\ünï.tex",
        "quote\"and\ttab.tex",
    ] {
        let d = Document::parse_str(
            "A line. $x$ here.",
            Format::Latex,
            &Options::new().with_name(name),
        );
        let j = d.to_json();
        valid_json(&j).unwrap_or_else(|e| panic!("{name}: {e}\n{j}"));
        assert!(
            j.contains(&format!("\"file\": {}", json_quote(name))),
            "{name}: {j}"
        );
    }
}

fn json_quote(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\t' => o.push_str("\\t"),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

#[test]
fn select_matches_paths_spelled_differently() {
    let src = "One line.\n\nSecond paragraph.\n";
    // one file, spelled in many ways
    let name = r"C:\Users\Ana María\papers\main.tex";
    let d = Document::parse_str(src, Format::Latex, &Options::new().with_name(name));
    for q in [
        "C:/Users/Ana María/papers/main.tex",
        r"c:\users\ana maría\papers\main.tex",
        r"C:\Users\Ana María\papers\..\papers\.\main.tex",
        r"\\?\C:\Users\Ana María\papers\main.tex",
        "papers/main.tex",
        "papers\\main.tex",
        "main.tex",
        "./main.tex",
        "C:\\Users\\Ana María\\\\papers\\main.tex",
    ] {
        assert!(!d.select(q, 1, 3).is_empty(), "{q}");
    }
    for q in [
        r"C:\Users\Ana María\other\main.tex",
        "other/main.tex",
        "main.text",
        "xmain.tex",
        "",
    ] {
        assert!(d.select(q, 1, 3).is_empty(), "{q}");
    }
    // a WSL path and the Linux path of the same file
    let d = Document::parse_str(
        src,
        Format::Latex,
        &Options::new().with_name("/home/ana/papers/main.tex"),
    );
    for q in [
        r"\\wsl.localhost\Ubuntu\home\ana\papers\main.tex",
        "//wsl$/Ubuntu/home/ana/papers/main.tex",
        "papers/main.tex",
    ] {
        assert!(!d.select(q, 1, 3).is_empty(), "{q}");
    }
    assert_eq!(d.select("/home/bob/papers/main.tex", 1, 3).len(), 0);
    // lines
    assert_eq!(d.select("main.tex", 3, 3).len(), 1);
    assert_eq!(d.select("main.tex", 3, 1).len(), 2);
}

#[test]
fn input_with_a_backslash_reaches_the_file() {
    let dir = tmp("backslash");
    std::fs::create_dir_all(dir.join("sub dir")).expect("folder");
    std::fs::write(
        dir.join("main.tex"),
        "\\documentclass{article}\n\\begin{document}\nStart.\n\\input{sub dir\\part}\n\\include{sub dir\\other}\nEnd.\n\\end{document}\n",
    )
    .expect("writes");
    std::fs::write(dir.join("sub dir").join("part.tex"), "Inside the part.\n").expect("writes");
    std::fs::write(dir.join("sub dir").join("other.tex"), "Inside the other.\n").expect("writes");
    let d = Document::parse_file(dir.join("main.tex"), &Options::new()).expect("reads");
    let t = d.to_text();
    assert!(t.contains("Inside the part."), "{t}");
    assert!(t.contains("Inside the other."), "{t}");
    assert!(d.warnings().is_empty(), "{:?}", d.warnings());
    valid_json(&d.to_json()).expect("valid JSON");
}

#[test]
fn the_program_reads_a_path_with_spaces_and_non_ascii_letters() {
    let dir = tmp("program");
    let sub = dir.join("Ana María — thesis");
    std::fs::create_dir_all(&sub).expect("folder");
    let file = sub.join("tésis principal.tex");
    std::fs::write(
        &file,
        "\\documentclass{article}\r\n\\begin{document}\r\nHello $x = 1$.\r\n\\input{capítulo}\r\n\\end{document}\r\n",
    )
    .expect("writes");
    std::fs::write(sub.join("capítulo.tex"), "Second file.\r\n").expect("writes");
    let exe = env!("CARGO_BIN_EXE_tex2speech");
    let out = std::process::Command::new(exe)
        .arg(&file)
        .output()
        .expect("runs");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).expect("utf8");
    assert_eq!(text, "Hello x equal to one.\nSecond file.\n");
    let out = std::process::Command::new(exe)
        .arg("--json")
        .arg(&file)
        .output()
        .expect("runs");
    let json = String::from_utf8(out.stdout).expect("utf8");
    valid_json(&json).expect("valid JSON");
    assert!(json.contains("capítulo.tex"), "{json}");
    // standard input with bytes that are not UTF-8
    let mut child = std::process::Command::new(exe)
        .args(["--format", "tex"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("runs");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"Caf\xE9 au lait.\n")
        .expect("writes");
    let out = child.wait_with_output().expect("waits");
    assert_eq!(String::from_utf8_lossy(&out.stdout), "Caf\u{e9} au lait.\n");
    // a missing file is an error with a message
    let out = std::process::Command::new(exe)
        .arg(dir.join("no such file.tex"))
        .output()
        .expect("runs");
    assert_eq!(out.status.code(), Some(1));
}
