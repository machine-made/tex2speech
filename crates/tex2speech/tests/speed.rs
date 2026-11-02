// SPDX-License-Identifier: Apache-2.0
//! Speed on a generated 50-page paper with 500 equations. The limit is checked in release
//! builds only; the numbers are printed with `--nocapture`.

use std::fmt::Write as _;
use std::time::Instant;
use tex2speech::{Document, Format, Options};

fn paper() -> String {
    let mut s = String::from(
        "\\documentclass{article}\n\\usepackage{amsmath,siunitx}\n\\newcommand{\\vel}[1]{\\mathbf{#1}}\n\\begin{document}\n\\maketitle\n",
    );
    let templates = [
        "\\frac{\\partial \\vel{u}}{\\partial t} + (\\vel{u}\\cdot\\nabla)\\vel{u} = -\\nabla p + \\nu_{K} \\nabla^2 \\vel{u}",
        "\\int_0^{K} e^{-x^2}\\,dx = \\frac{\\sqrt{\\pi}}{2}",
        "\\sum_{i=0}^{K} x_i^2 \\le C_{K}",
        "\\lambda_{K} = 1.3 \\times 10^{-3}",
        "\\|u_{K}\\|_2 \\le \\epsilon",
        "\\begin{pmatrix} a_{K} & b \\\\ c & d \\end{pmatrix}",
        "Re_{K} = \\frac{U D}{\\nu}",
        "\\hat{u}_{K}(\\omega) = \\int u(t) e^{-i\\omega t}\\,dt",
    ];
    // 50 pages of about 500 words: 25 000 words
    let mut eq = 0;
    for section in 0..25 {
        let _ = writeln!(
            s,
            "\\section{{Section number {section}}}\\label{{sec:{section}}}"
        );
        for para in 0..8 {
            let _ = write!(
                s,
                "In Section~\\ref{{sec:{section}}} the flow past a body is described by Williamson et al.~\\cite{{k{para}}}. "
            );
            for _ in 0..4 {
                s.push_str("The wake of the cylinder is unsteady and the measured drag agrees with the simulation within a few percent, as shown before. ");
            }
            for _ in 0..3 {
                if eq < 500 {
                    let t = templates[eq % templates.len()].replace('K', &eq.to_string());
                    if eq % 3 == 0 {
                        let _ = write!(
                            s,
                            "\\begin{{equation}}\\label{{eq:{eq}}}\n{t}\n\\end{{equation}}\nEquation~\\eqref{{eq:{eq}}} holds. "
                        );
                    } else {
                        let _ = write!(
                            s,
                            "We have ${t}$ at \\SI{{{}.5}}{{\\metre\\per\\second}}. ",
                            eq % 17
                        );
                    }
                    eq += 1;
                }
            }
            s.push_str("\n\n");
        }
    }
    assert_eq!(eq, 500, "the paper holds the 500 equations the test names");
    s.push_str("\\end{document}\n");
    s
}

#[test]
fn fifty_pages_with_five_hundred_equations() {
    let src = paper();
    let words = src.split_whitespace().count();
    let t = Instant::now();
    let doc = Document::parse_str(&src, Format::Latex, &Options::new());
    let first = t.elapsed();
    let t = Instant::now();
    let again = Document::parse_str(&src, Format::Latex, &Options::new());
    let second = t.elapsed();
    eprintln!(
        "{words} words, {} segments: first run {first:?}, second run {second:?}",
        doc.segments().len()
    );
    assert_eq!(doc.to_text(), again.to_text());
    assert!(doc.segments().len() > 1000);
    if !cfg!(debug_assertions) {
        assert!(first.as_millis() < 1000, "first run took {first:?}");
        assert!(second.as_millis() < 1000, "second run took {second:?}");
    }
}

#[test]
fn a_sentence_without_maths_is_fast() {
    let t = Instant::now();
    let doc = Document::parse_str(
        "One plain sentence about a cylinder.",
        Format::Latex,
        &Options::new(),
    );
    let d = t.elapsed();
    eprintln!("no maths: {d:?}");
    assert_eq!(doc.to_text(), "One plain sentence about a cylinder.\n");
    if !cfg!(debug_assertions) {
        assert!(d.as_millis() < 5, "{d:?}");
    }
}

#[test]
fn lookup_in_a_parsed_file_is_fast() {
    let src = paper();
    let doc = Document::parse_str(&src, Format::Latex, &Options::new().with_name("paper.tex"));
    let t = Instant::now();
    let n: u32 = 1000;
    let mut found = 0;
    for i in 0..n {
        found += doc
            .select(
                "paper.tex",
                50 + (i % 400) as usize,
                52 + (i % 400) as usize,
            )
            .len();
    }
    let per = t.elapsed() / n;
    eprintln!(
        "select over {} segments: {per:?} per lookup, {found} hits",
        doc.segments().len()
    );
    assert!(found > 0);
    if !cfg!(debug_assertions) {
        assert!(per.as_micros() < 100, "{per:?}");
    }
}
