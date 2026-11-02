// SPDX-License-Identifier: Apache-2.0
//! Checks on the benchmark texts in `bench/` and on the program's exit codes.

use std::path::PathBuf;
use std::process::Command;

fn bench(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../bench")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

#[test]
fn sources_contain_the_hidden_word() {
    for name in ["bench.tex", "bench.md"] {
        assert!(bench(name).contains("HIDDENWORD"), "{name} lost its marker");
    }
}

#[test]
fn expected_output_hides_the_hidden_word() {
    for name in ["bench.tex.expected.txt", "bench.md.expected.txt"] {
        assert!(
            !bench(name).contains("HIDDENWORD"),
            "{name} speaks hidden text"
        );
    }
}

#[test]
fn expected_output_has_no_markup() {
    for name in ["bench.tex.expected.txt", "bench.md.expected.txt"] {
        let text = bench(name);
        for c in ['\\', '$', '{', '_', '^'] {
            assert!(!text.contains(c), "{name} contains {c:?}");
        }
    }
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_tex2speech"))
        .args(args)
        .output()
        .expect("program starts")
}

#[test]
fn version_flag_prints_version_and_exits_0() {
    let out = run(&["--version"]);
    assert_eq!(out.status.code(), Some(0));
    let expected = format!("tex2speech {}\n", tex2speech::version());
    assert_eq!(String::from_utf8_lossy(&out.stdout), expected);
}

#[test]
fn other_arguments_print_usage_to_stderr_and_exit_2() {
    for args in [&[][..], &["--help"][..], &["--version", "x"][..]] {
        let out = run(args);
        assert_eq!(out.status.code(), Some(2), "args {args:?}");
        assert_eq!(out.stdout.len(), 0);
        assert!(String::from_utf8_lossy(&out.stderr).starts_with("usage:"));
    }
}

// ----- the benchmark transcripts -----

fn bench_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../bench")
        .join(name)
}

fn translate(name: &str) -> String {
    let doc = tex2speech::Document::parse_file(bench_path(name), &tex2speech::Options::new())
        .expect("benchmark file reads");
    doc.to_text()
}

#[test]
fn latex_benchmark_matches_the_expected_text() {
    assert_eq!(translate("bench.tex"), bench("bench.tex.expected.txt"));
}

#[test]
fn markdown_benchmark_matches_the_expected_text() {
    assert_eq!(translate("bench.md"), bench("bench.md.expected.txt"));
}

#[test]
fn hidden_word_never_appears_in_any_output() {
    for name in ["bench.tex", "bench.md"] {
        let doc = tex2speech::Document::parse_file(bench_path(name), &tex2speech::Options::new())
            .expect("reads");
        assert!(!doc.to_text().contains("HIDDENWORD"), "{name} text");
        assert!(!doc.to_json().contains("HIDDENWORD"), "{name} json");
    }
}

#[test]
fn program_prints_the_transcript() {
    for name in ["bench.tex", "bench.md"] {
        let out = run(&[bench_path(name).to_str().expect("utf-8 path")]);
        assert_eq!(out.status.code(), Some(0), "{name}");
        assert_eq!(
            String::from_utf8_lossy(&out.stdout),
            bench(&format!("{name}.expected.txt")),
            "{name}"
        );
        assert!(out.stderr.is_empty(), "the program writes only its output");
    }
}

#[test]
fn program_reads_standard_input_with_a_format() {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new(env!("CARGO_BIN_EXE_tex2speech"))
        .args(["--format", "md", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("starts");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"## Part\n\nWe have $x = 1$.\n")
        .expect("writes");
    let out = child.wait_with_output().expect("finishes");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "Section 1: Part.\nWe have x equal to one.\n"
    );
}

#[test]
fn json_output_has_the_documented_fields() {
    let out = run(&[
        "--json",
        bench_path("bench.md").to_str().expect("utf-8 path"),
    ]);
    assert_eq!(out.status.code(), Some(0));
    let json = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(json.starts_with("{\"segments\": ["));
    for field in [
        "\"kind\": \"heading\"",
        "\"text\": \"Chapter 1.\"",
        "\"pause_ms\":",
        "\"span\":",
        "\"line\":",
        "\"section\":",
        "\"warnings\":",
    ] {
        assert!(json.contains(field), "missing {field}");
    }
}

#[test]
fn missing_file_exits_1_and_names_the_file() {
    let out = run(&["/nonexistent/file.tex"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(out.stdout, [] as [u8; 0]);
    assert!(String::from_utf8_lossy(&out.stderr).contains("/nonexistent/file.tex"));
}

#[test]
fn select_returns_the_segments_of_a_line_range() {
    let doc =
        tex2speech::Document::parse_file(bench_path("bench.tex"), &tex2speech::Options::new())
            .expect("reads");
    let hit = doc.select("bench.tex", 14, 14);
    assert!(
        hit.iter().any(|s| s.text.starts_with("We set beta")),
        "{hit:?}"
    );
    assert!(
        hit.iter()
            .all(|s| s.span.line <= 14 && s.span.end_line >= 14)
    );
    assert_eq!(
        doc.select("other.tex", 1, 100),
        [] as [&tex2speech::Segment; 0]
    );
}
