// SPDX-License-Identifier: Apache-2.0
#![allow(clippy::cast_possible_truncation)] // the generator takes a remainder below a small length
//! Property tests on generated input: no panic, no markup in the output, bounded work on hostile input.
//! The generator is a fixed-seed xorshift, so a failure repeats.

use tex2speech::{Document, Format, Options, unspeakable_chars};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn pick<'a>(&mut self, v: &[&'a str]) -> &'a str {
        v[(self.next() % v.len() as u64) as usize]
    }
}

const LATEX: &[&str] = &[
    "word ",
    "text. ",
    "More text, here; ",
    "\\section{S}",
    "\\subsection*{T}",
    "$x^2 + y_1$",
    "$$a=b$$",
    "\\[ \\frac{a}{b} \\]",
    "\\begin{equation}",
    "\\end{equation}",
    "\\begin{align} a &= b \\\\ c &= d \\end{align}",
    "\\begin{itemize}",
    "\\item ",
    "\\end{itemize}",
    "\\emph{",
    "}",
    "{",
    "\\textbf{",
    "% comment\n",
    "\n\n",
    "\\label{l}",
    "\\ref{l}",
    "\\cite{k}",
    "\\footnote{n}",
    "\\newcommand{\\foo}[1]{#1#1}",
    "\\foo{a}",
    "\\def\\bar#1{[#1]}",
    "\\bar{b}",
    "\\begin{figure}",
    "\\caption{c}",
    "\\end{figure}",
    "\\input{nowhere}",
    "\\unknown{arg}",
    "\\left(",
    "\\right)",
    "\\begin{verbatim}",
    "\\end{verbatim}",
    "\\iffalse ",
    "\\fi ",
    "\\if ",
    "\\SI{3}{\\metre}",
    "\\verb|x|",
    "&",
    "#",
    "^",
    "_",
    "$",
    "\\\\",
    "\\",
    "~",
    "--",
    "```",
    "\\begin{",
    "\\end{",
    "\\'e",
    "\\c{c}",
    "\\begin{tabular}{cc} a & b \\end{tabular}",
    "\\appendix",
    "\\maketitle",
    "\\title{T}",
    "\\begin{document}",
    "\\end{document}",
    "\\begin{theorem}",
    "\\end{theorem}",
    "\\tag{1}",
    "\\nonumber",
    "\\text{",
    "\\mathbb{R}",
    "\\sum_{i=0}^{N}",
    "é",
    "∑",
    "\u{0}",
];

const MARKDOWN: &[&str] = &[
    "# Head\n",
    "## Sub\n",
    "text ",
    "*em* ",
    "**strong** ",
    "`code_x` ",
    "$x_1$ ",
    "$$\na=b\n$$\n",
    "\\(y\\) ",
    "\\[z\\]",
    "- item\n",
    "1. one\n",
    "> quote\n",
    "```\ncode\n```\n",
    "| a | b |\n|---|---|\n| 1 | 2 |\n",
    "[link](http://x) ",
    "![alt](i.png) ",
    "<b>",
    "</b>",
    "<!-- c -->",
    "[^1]",
    "\n[^1]: note\n",
    "---\n",
    "\n\n",
    "Setext\n====\n",
    "- [x] done\n",
    "<script>",
    "$",
    "\\",
    "*",
    "_",
    "`",
    "[",
    "](",
    "é",
];

fn generate(rng: &mut Rng, vocab: &[&str], n: usize) -> String {
    (0..n).map(|_| rng.pick(vocab)).collect()
}

fn check(src: &str, format: Format) {
    let doc = Document::parse_str(src, format, &Options::new());
    for s in doc.segments() {
        for c in ['\\', '$', '{', '}', '_', '^'] {
            if s.text.contains(c) {
                eprintln!("leak {c:?} in {:?} from {src:?}", s.text);
                panic!("markup leaked");
            }
        }
        let bad = unspeakable_chars(&s.text);
        if !bad.is_empty() {
            eprintln!("unspeakable {bad:?} in {:?} from {src:?}", s.text);
            panic!("unspeakable symbol");
        }
        assert!(s.span.start <= s.span.end);
        assert!(s.span.line >= 1);
    }
    // the same input gives the same bytes
    assert_eq!(
        doc.to_json(),
        Document::parse_str(src, format, &Options::new()).to_json()
    );
}

#[test]
fn random_latex_never_panics_or_leaks() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for _ in 0..400 {
        let n = 1 + (rng.next() % 60) as usize;
        let src = generate(&mut rng, LATEX, n);
        check(&src, Format::Latex);
    }
}

#[test]
fn random_markdown_never_panics_or_leaks() {
    let mut rng = Rng(0x1234_5678_9abc_def1);
    for _ in 0..400 {
        let n = 1 + (rng.next() % 50) as usize;
        let src = generate(&mut rng, MARKDOWN, n);
        check(&src, Format::Markdown);
    }
}

#[test]
fn truncated_documents_never_panic() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bench");
    for (name, format) in [("bench.tex", Format::Latex), ("bench.md", Format::Markdown)] {
        let text = std::fs::read_to_string(dir.join(name)).expect("reads");
        for cut in (0..text.len()).step_by(7) {
            if text.is_char_boundary(cut) {
                check(&text[..cut], format);
            }
        }
    }
}

#[test]
fn hostile_nesting_and_repetition_stay_bounded() {
    let deep = "{".repeat(50_000);
    check(&deep, Format::Latex);
    check(
        &format!("\\emph{{{}", "\\emph{".repeat(5_000)),
        Format::Latex,
    );
    check(&"$".repeat(10_001), Format::Latex);
    check(&"\\begin{x}".repeat(2_000), Format::Latex);
    check("\\newcommand{\\a}{\\a\\a}\\a", Format::Latex);
    check("\\def\\a{\\b}\\def\\b{\\a}\\a", Format::Latex);
    check(&"> ".repeat(3_000), Format::Markdown);
    check(&"*".repeat(20_000), Format::Markdown);
    check(&"[".repeat(5_000), Format::Markdown);
}

#[test]
fn non_utf8_latin1_files_are_read() {
    let dir = std::env::temp_dir().join(format!("t2s_latin1_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join("l.tex");
    std::fs::write(&path, b"Caf\xe9 au lait.\n").expect("writes");
    let doc = Document::parse_file(&path, &Options::new()).expect("reads");
    assert_eq!(doc.to_text(), "Café au lait.\n");
    let _ = std::fs::remove_dir_all(&dir);
}
