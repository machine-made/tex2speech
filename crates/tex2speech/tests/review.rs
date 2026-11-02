// SPDX-License-Identifier: Apache-2.0
//! Bugs found by review, each with the input that broke it.

use std::fmt::Write as _;
use std::path::PathBuf;
use tex2speech::{Document, Format, Options};

fn latex(src: &str) -> Document {
    Document::parse_str(src, Format::Latex, &Options::new())
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("tex2speech-review-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("temp folder");
    d
}

#[test]
fn accented_text_near_documentclass_does_not_panic() {
    // the 400th byte after the command falls inside a two-byte letter, for both parities
    for pad in ["", "a"] {
        let src = format!(
            "\\documentclass{{article}}% {pad}{}\n\\begin{{document}}\nHello.\n\\end{{document}}\n",
            "\u{e9}".repeat(300)
        );
        let src2 = format!("\\documentclass{pad}{}", "\u{e9}".repeat(300));
        for s in [src, src2] {
            let d = latex(&s);
            assert!(d.warnings().iter().all(|w| w.construct != "internal"));
        }
    }
}

#[test]
fn every_builtin_term_gets_its_own_reading() {
    for (term, spoken) in [
        ("Ma", "Mach number"),
        ("Ri", "Richardson number"),
        ("Bo", "Bond number"),
        ("Da", "Damkoehler number"),
        ("De", "Deborah number"),
        ("Oh", "Ohnesorge number"),
        ("Eu", "Euler number"),
    ] {
        let t = latex(&format!("We use ${term} = 1$.")).to_text();
        assert!(t.contains(spoken), "{term}: {t}");
    }
}

#[test]
fn a_huge_ordinal_does_not_panic() {
    for n in [
        "10000000000000000",
        "99999999999999999999",
        "1000000000000000",
    ] {
        let d = latex(&format!("The {n}th time and the {n}st one."));
        assert!(
            d.warnings().iter().all(|w| w.construct != "internal"),
            "{n}: {:?}",
            d.warnings()
        );
        assert!(!d.segments().is_empty(), "{n}");
    }
}

#[test]
fn input_cannot_leave_the_document_folder() {
    let dir = tmp("confined");
    let root = dir.join("project");
    std::fs::create_dir_all(root.join("sub")).expect("folder");
    std::fs::write(dir.join("secret.tex"), "SECRETOUT.\n").expect("writes");
    std::fs::write(root.join("sub").join("ok.tex"), "Inside text.\n").expect("writes");
    let abs = dir.join("secret.tex").to_string_lossy().replace('\\', "/");
    std::fs::write(
        root.join("main.tex"),
        format!(
            "\\documentclass{{article}}\n\\begin{{document}}\n\\input{{../secret}}\n\\input{{{abs}}}\n\\input{{sub/../../secret}}\n\\input{{sub/ok}}\nEnd.\n\\end{{document}}\n"
        ),
    )
    .expect("writes");
    let d = Document::parse_file(root.join("main.tex"), &Options::new()).expect("reads");
    let t = d.to_text();
    assert!(!t.contains("SECRETOUT"), "{t}");
    assert!(t.contains("Inside text."), "{t}");
    let outside = d
        .warnings()
        .iter()
        .filter(|w| w.construct == "input" && w.message.contains("outside"))
        .count();
    assert_eq!(outside, 3, "{:?}", d.warnings());
}

#[test]
fn a_link_out_of_the_folder_is_not_followed() {
    #[cfg(unix)]
    {
        let dir = tmp("link");
        let root = dir.join("project");
        std::fs::create_dir_all(&root).expect("folder");
        std::fs::write(dir.join("secret.tex"), "SECRETOUT.\n").expect("writes");
        std::os::unix::fs::symlink(dir.join("secret.tex"), root.join("link.tex")).expect("link");
        std::fs::write(
            root.join("main.tex"),
            "\\documentclass{article}\n\\begin{document}\n\\input{link}\n\\end{document}\n",
        )
        .expect("writes");
        let d = Document::parse_file(root.join("main.tex"), &Options::new()).expect("reads");
        assert!(!d.to_text().contains("SECRETOUT"));
    }
}

#[test]
fn without_a_base_folder_input_reads_nothing() {
    let dir = tmp("nobase");
    let abs = dir.join("secret.tex").to_string_lossy().replace('\\', "/");
    std::fs::write(dir.join("secret.tex"), "SECRETOUT.\n").expect("writes");
    let d = latex(&format!(
        "\\documentclass{{article}}\n\\begin{{document}}\n\\input{{{abs}}}\n\\input{{secret}}\n\\end{{document}}\n"
    ));
    assert!(!d.to_text().contains("SECRETOUT"));
    assert!(d.warnings().iter().any(|w| w.construct == "input"));
}

#[test]
fn a_longer_fence_is_not_closed_by_a_shorter_one() {
    let src = "Before.\n\n````\n```\ninner\n```\n````\n\nAfter $a = b$ and \\(c = d\\) here.\n";
    let t = Document::parse_str(src, Format::Markdown, &Options::new()).to_text();
    assert!(!t.contains("inner"), "{t}");
    assert!(t.contains("c equal to d"), "{t}");
    assert!(t.contains("a equal to b"), "{t}");
}

#[test]
fn user_lexicon_terms_with_two_digit_indices_keep_their_reading() {
    let opts = Options::new().with_lexicon("Aa = alpha thing\nBb = beta thing\n");
    let t = Document::parse_str("We use $Aa + Bb$ and $Pr = 1$.", Format::Latex, &opts).to_text();
    assert!(t.contains("alpha thing") && t.contains("beta thing"), "{t}");
    assert!(t.contains("Prandtl number"), "{t}");
    assert!(
        !t.contains("Prandtl number plus") && !t.contains("Weber"),
        "{t}"
    );
}

// ----- second group -----

#[test]
fn def_with_a_multibyte_letter_after_a_hash_does_not_panic() {
    for src in [
        "\\def\\x#\u{e9} text".to_string(),
        format!("\\def\\x{}", "\u{e9}".repeat(300)),
        format!("\\def\\x #{}", "\u{e9}".repeat(300)),
        // `#` steps over two bytes, so the 200-byte limit then stops inside a letter
        format!("\\def\\x#{}", "\u{e9}".repeat(150)),
        "\\def\\x#".to_string(),
        "\\def\\x#1#\u{20ac}".to_string(),
    ] {
        let d = latex(&format!("Before. {src}\n\nAfter."));
        assert!(
            d.warnings().iter().all(|w| w.construct != "internal"),
            "{src:?}: {:?}",
            d.warnings()
        );
        assert!(d.to_text().contains("Before"), "{src:?}");
    }
}

#[test]
fn nested_macros_cannot_write_gigabytes() {
    // each level repeats the one below 100 times: about 10^6 uses of the 100 kB body inside the use limit
    let body = "word ".repeat(20_000);
    let mut src = format!("\\newcommand{{\\mA}}{{{body}}}\n");
    for (lo, hi) in [("mA", "mB"), ("mB", "mC"), ("mC", "mD")] {
        let _ = writeln!(
            src,
            "\\newcommand{{\\{hi}}}{{{}}}",
            format!("\\{lo}").repeat(100)
        );
    }
    src.push_str("\\begin{document}\n\\mD\n\\end{document}\n");
    let d = latex(&src);
    assert!(
        d.to_text().len() < 100 * 1024 * 1024,
        "{}",
        d.to_text().len()
    );
    assert!(
        d.warnings().iter().any(|w| w.construct == "macro"),
        "{:?}",
        d.warnings()
    );
}

#[test]
fn the_same_file_included_many_times_is_bounded() {
    let dir = tmp("again");
    std::fs::write(dir.join("big.tex"), "word ".repeat(200_000)).expect("writes");
    let src = "\\input{big}\n".repeat(2_000);
    let opts = Options::new().with_base_dir(&dir);
    let d = Document::parse_str(&src, Format::Latex, &opts);
    assert!(d.to_text().len() < 100 * 1024 * 1024);
    assert!(d.warnings().iter().any(|w| w.construct == "macro"));
}

#[test]
fn a_lone_carriage_return_ends_a_line() {
    let src = "\\documentclass{article}\r% a comment\r\\begin{document}\rFirst line.\r\rSecond paragraph.\r\\end{document}\r";
    let d = latex(src);
    let t = d.to_text();
    assert!(
        t.contains("First line.") && t.contains("Second paragraph."),
        "{t}"
    );
    let seg = d
        .segments()
        .iter()
        .find(|s| s.text.contains("Second"))
        .expect("segment");
    assert!(seg.span.line > 3, "line {}", seg.span.line);
    assert!(d.segments().len() >= 2, "{:?}", d.segments());
}

#[test]
fn a_huge_equation_is_read_by_the_fallback() {
    let src = format!("${}$", "x + ".repeat(40_000));
    let d = latex(&src);
    assert!(d.to_text().contains("x plus x") || d.to_text().contains('x'));
    assert!(
        d.warnings()
            .iter()
            .any(|w| w.message.contains("longer than")),
        "{:?}",
        d.warnings()
    );
}
