//! The equation table of the tex2speech benchmark, read by the fork and by upstream `MathCAT`.
//!
//! `ROWS` holds the readings that rule edits produce. The `Should say` text is never changed to fit
//! the output, and nothing is patched after `MathCAT`. A row that no rule edit reaches would go in a
//! separate list holding its current output, so that a later change shows up; none is in that state.

use math_core::{LatexToMathML, MathCoreConfig, MathDisplay};
use std::sync::Mutex;

/// `MathCAT` keeps its state in thread-local storage and tests run on separate threads. The mutex keeps
/// the two tests from loading rules at the same time.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// (LaTeX, should say)
const ROWS: &[(&str, &str)] = &[
    (r"\beta = 43000", "beta equal to 43000"),
    (r"\beta = 43{,}000", "beta equal to 43000"),
    (
        r"Re = \frac{U D}{\nu}",
        "Re equal to capital U capital D over nu",
    ),
    (
        r"\frac{\partial \mathbf{u}}{\partial t} + (\mathbf{u}\cdot\nabla)\mathbf{u} = -\nabla p + \nu \nabla^2 \mathbf{u}",
        "partial u, partial t, plus u dot grad u, equal to, minus grad p, plus nu laplacian u",
    ),
    (r"\frac{\partial u}{\partial t}", "partial u, partial t"),
    (r"\nabla \cdot \mathbf{u} = 0", "divergence of u equal to 0"),
    (
        r"\int_0^\infty e^{-x^2}\,dx = \frac{\sqrt{\pi}}{2}",
        "integral from 0 to infinity of e to the minus x squared d x, equal to, square root of pi over 2",
    ),
    (
        r"\sum_{i=0}^{N} x_i^2",
        "sum from i equal to 0 to capital N of x i squared",
    ),
    (
        r"\lambda_{\max} = 1.3 \times 10^{-3}",
        "lambda max equal to 1.3 times 10 to the minus 3",
    ),
    (r"f(x+1)", "f of x plus 1"),
    (r"a(x+1)", "a times, x plus 1"),
    (r"\mathcal{O}(h^2)", "order h squared"),
    (r"u_\tau^+", "u tau plus"),
    (
        r"\dot{m} = \rho A U",
        "m dot equal to rho capital A capital U",
    ),
    (r"E_{kin}", "capital E kin"),
    (
        r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}",
        "the 2 by 2 matrix with rows a, b, and c, d",
    ),
    (r"\hat{u}", "u hat"),
    (
        r"\|x\|_2 \le \epsilon",
        "two norm of x less than or equal to epsilon",
    ),
    (r"x \in \mathbb{R}^n", "x in R n"),
];

fn mathml(latex: &str) -> String {
    let converter =
        LatexToMathML::new(MathCoreConfig::default(), vec![]).expect("math-core config");
    converter
        .convert_with_local_state(latex, MathDisplay::Inline)
        .unwrap_or_else(|e| panic!("math-core failed on {latex}: {e:?}"))
        .mathml
}

fn rules_dir() -> String {
    if cfg!(feature = "include-zip") {
        "Rules".to_string() // the rules are inside the library; the name is not used
    } else {
        format!("{}/Rules", env!("CARGO_MANIFEST_DIR"))
    }
}

fn fork_reading(latex: &str) -> String {
    use tex2speech_mathcat as fork;
    fork::set_mathml(mathml(latex)).unwrap_or_else(|e| panic!("{}", fork::errors_to_string(&e)));
    fork::get_spoken_text().unwrap_or_else(|e| panic!("{}", fork::errors_to_string(&e)))
}

fn fork_setup() {
    use tex2speech_mathcat as fork;
    fork::set_rules_dir(rules_dir()).unwrap();
    fork::set_preference("Language", "en").unwrap();
    fork::set_preference("SpeechStyle", "ClearSpeak").unwrap();
    fork::set_preference("TTS", "None").unwrap();
}

#[test]
fn bench_equations() {
    let _guard = ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    fork_setup();
    let mut failures = Vec::new();
    for (latex, should) in ROWS {
        let got = fork_reading(latex);
        if got != *should {
            failures.push(format!(
                "{latex}\n   should say: {should}\n   says:       {got}"
            ));
        }
    }
    // MathCAT replaces the panic hook, so the assert message alone is not shown.
    let report = format!(
        "{} of {} rows fail:\n{}",
        failures.len(),
        ROWS.len(),
        failures.join("\n")
    );
    if !failures.is_empty() {
        eprintln!("{report}");
    }
    assert!(failures.is_empty(), "{report}");
}

/// Prints the fork and upstream readings side by side. Run with `--nocapture` to see them.
#[test]
fn fork_against_upstream() {
    let _guard = ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    fork_setup();
    libmathcat::set_rules_dir("Rules").unwrap(); // upstream is built with include-zip
    libmathcat::set_preference("Language", "en").unwrap();
    libmathcat::set_preference("SpeechStyle", "ClearSpeak").unwrap();
    libmathcat::set_preference("TTS", "None").unwrap();
    for (latex, _) in ROWS {
        libmathcat::set_mathml(mathml(latex)).unwrap();
        let upstream = libmathcat::get_spoken_text().unwrap();
        println!(
            "{latex}\n   upstream: {upstream}\n   fork:     {}",
            fork_reading(latex)
        );
    }
}
