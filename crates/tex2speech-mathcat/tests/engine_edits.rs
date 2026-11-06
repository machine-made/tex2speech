//! Rule edits made with the English engine and for reading quality (`RULE_EDITS.md`, rows 27 to 29, 34 to 62 and 74 to 97).

use math_core::{LatexToMathML, MathCoreConfig, MathDisplay};
use std::sync::Mutex;
use tex2speech_mathcat as fork;

static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

const ROWS: &[(&str, &str)] = &[
    (r"U D / \nu", "capital U capital D over nu"),
    (r"a \div b", "a divided by b"),
    (r"a \approx b", "a approximately equal to b"),
    (r"a \ll b", "a much less than b"),
    (r"R_{c,1}", "capital R c, 1"),
    // a pair is not an interval; a set relation makes it one
    (r"(a, b)", "a, b"),
    (r"x \in (a, b)", "x in the open interval from a to b"),
    (r"x \notin (a, b)", "x not in the open interval from a to b"),
    (r"x \in [a, b]", "x in the closed interval from a to b"),
    (
        r"x \in [a, b)",
        "x in the half-open interval from a to b, including a",
    ),
    (
        r"(a, b) \subset S",
        "the open interval from a to b is a subset of capital S",
    ),
    // accents on any base, vectors, norms
    (r"\vec{v}", "vector v"),
    (r"\vec{k}_{\parallel}", "vector k, parallel"),
    (r"\dot{C_y}", "capital C y dot"),
    (r"\dot{\bar{u}}", "u bar dot"),
    (r"\hat{a+b}", "a plus b hat"),
    (r"\|A\|", "norm of capital A"),
    (r"\xrightarrow{d}", "right arrow labelled d"),
    // brackets around one symbol, indices, stars, transposes
    (r"(a)", "a"),
    (r"x^{(j)}", "x superscript j"),
    (r"y^{(m+1)}", "y superscript m plus 1 end superscript"),
    (r"x^*", "x superscript star"),
    (r"\delta_*", "delta star"),
    (r"A^\top", "capital A transpose"),
    (r"u_h^T", "u h transpose"),
    // names, relations and marks met in the corpus (rows 62 to 72)
    (r"x \sim y", "x tilde y"),
    (r"\sim 5", "about 5"),
    (r"\mathbb{Z}_2", "Z 2"),
    (r"G \rtimes H", "capital G semidirect product capital H"),
    (r"H^1(x)", "capital H 1 of x"),
    (r"W^{1,p}(x)", "capital W 1, p of x"),
    (r"f^{-1}(x)", "f inverse of x"),
    (r"y^\circ", "y circle"),
    (r"90^\circ", "90 degrees"),
    (r"M_\odot", "capital M sun"),
    (r"\ell", "ell"),
    (r"P_1^\mu", "capital P 1 to the mu-th power"),
    (r"\mathcal{O}(eB)", "order e capital B"),
    // groups, fractions, subscripts and marks that a listener hears (rows 77 to 90)
    (r"(a+b)(c+d)", "a plus b, times, c plus d"),
    (r"(a+b)c", "a plus b, times c"),
    (r"2(x+1)", "2 times, x plus 1"),
    (r"(a+b)^2", "a plus b, all squared"),
    (r"(a+b)^3", "a plus b, all cubed"),
    (r"(2\pi h)^4", "2 pi h, all to the fourth power"),
    (r"a-(b+c)", "a minus open paren b plus c close paren"),
    (r"\frac{a+b}{c+d}", "a plus b, over, c plus d"),
    (r"x_{n+1}", "x sub n plus 1"),
    (r"W_+", "capital W sub plus"),
    (r"\underline{\mu}", "mu underlined"),
    (r"A \Rightarrow B", "capital A implies capital B"),
    (r"{f}|_{x=0}", "f at x equal to 0"),
    (
        r"\{x \mid x>0\}",
        "the set of all x such that x greater than 0",
    ),
    (
        r"\{ t : x(t) \in S, t>0 \}",
        "the set of all t such that x of t in capital S, t greater than 0",
    ),
    (r"x | y", "x vertical line y"),
    (r"3 | 6", "3 divides 6"),
    (
        r"x = \begin{array}{cc} a & b \\ c & d \end{array}",
        "x equal to the 2 by 2 matrix with rows a, b, and c, d",
    ),
    (r"M_{200\mathrm{m}}", "capital M sub 200 m"),
    (r"\lfloor x \rfloor", "floor of x"),
    (r"\lceil x \rceil + 1", "ceiling of x, plus 1"),
    // cases, matrices, arrays
    (
        r"\begin{cases} a & x>0 \\ b & \text{otherwise} \end{cases}",
        "a if x greater than 0, b otherwise",
    ),
    (
        r"\begin{cases} 1, & x>0 \\ 0, & \text{otherwise} \end{cases}",
        "1, if x greater than 0, 0, otherwise",
    ),
    (
        r"\begin{matrix} a & b & c \\ d & e & f \\ g & h & i \end{matrix}",
        "the 3 by 3 matrix with rows a, b, c, and d, e, f, and g, h, i",
    ),
    (
        r"\begin{array}{cc} a & b \\ c & d \end{array}",
        "the 2 by 2 matrix with rows a, b, and c, d",
    ),
    (
        r"\begin{pmatrix} a_{11} & a_{12} \\ a_{21} & a_{22} \end{pmatrix}",
        "the 2 by 2 matrix with rows a 11, a 12, and a 21, a 22",
    ),
    (r"\begin{array}{cc} 1 & 2 \end{array}", "1, 2"),
    // separators are pauses, not words (rows 51 to 53)
    (
        r"(U_\infty, 0, 0) = (1, 0, 0)",
        "capital U infinity, 0, 0, equal to, 1, 0, 0",
    ),
    (r"f(x; y)", "f of x; y"),
    (r"A \setminus B", "capital A set minus capital B"),
    // expectation, probability, conditions (rows 54 to 56)
    (r"\mathbb{E}[X]", "expectation of capital X"),
    (r"\mathbb{P}(A)", "probability of capital A"),
    (r"\mathbb{E}_x[f(x)]", "expectation over x of f of x"),
    (r"\mathbb{E}", "capital E"),
    (r"p(x \mid y)", "p of x given y"),
    (r"a \mid b", "a divides b"),
    // perpendicular scripts, braces, determinants (rows 57 to 62)
    (r"k_\perp", "k perpendicular"),
    (r"A^\perp", "capital A perp"),
    (r"\overbrace{a+b}^{n}", "a plus b, labelled n"),
    (r"\underbrace{a+b}", "a plus b"),
    (r"|S|", "the absolute value of capital S"),
    (r"\mathrm{det} A", "determinant of capital A"),
    // a sign before a bracketed sum, braces round one comparison, a subscript before a relation, a definition (rows 91 to 94)
    (r"y = -(a+b)", "y equal to minus, a plus b"),
    (
        r"y = -(a+b) + c",
        "y equal to minus open paren a plus b close paren plus c",
    ),
    (r"\{a > b\}", "the event a greater than b"),
    (r"P(\{a>b\})", "capital P of the event a greater than b"),
    (r"P(\left\{X>t\right\})", "capital P of the event capital X greater than t"),
    (r"P(\{a\})", "capital P of the set a"),
    (r"\{a, b\}", "the set a, b"),
    (r"x_{n^2} = 1", "x sub n squared, equal to 1"),
    (r"\frac{a_{n^2}}{b}", "a sub n squared end sub, over b"),
    (r"x \coloneqq 1", "x defined as 1"),
    // a pause after a fraction that a sum or a product goes on from (row 96)
    (r"\frac{a}{b} + c", "a over b, plus c"),
    (r"\frac{a}{b} - c", "a over b, minus c"),
    (r"\frac{a}{b} c", "a over b, c"),
    (r"\frac{a}{b} \cdot c", "a over b, times c"),
    (r"\frac{a}{b} + \frac{c}{d}", "a over b, plus c over d"),
    (r"\frac{a+b}{c} + d", "a plus b, over, c, plus d"),
    (r"c + \frac{a}{b}", "c plus a over b"),
    (r"x = \frac{a}{b}", "x equal to a over b"),
    (
        r"\begin{vmatrix} a & b \\ c & d \end{vmatrix}",
        "the 2 by 2 determinant with rows a, b, and c, d",
    ),
    (
        r"\sum_{\begin{subarray}{c} i < n \\ j > m \end{subarray}} x",
        "sum over i less than n, j greater than m of x",
    ),
];

#[test]
fn engine_edits() {
    let _guard = ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    fork::set_rules_dir("Rules").unwrap();
    fork::set_preference("Language", "en").unwrap();
    fork::set_preference("SpeechStyle", "ClearSpeak").unwrap();
    fork::set_preference("TTS", "None").unwrap();
    let converter = LatexToMathML::new(MathCoreConfig::default(), vec![]).unwrap();
    let mut failures = Vec::new();
    for (latex, should) in ROWS {
        let ml = converter
            .convert_with_local_state(latex, MathDisplay::Inline)
            .unwrap()
            .mathml;
        fork::set_mathml(ml).unwrap();
        let got = fork::get_spoken_text().unwrap();
        if got != *should {
            failures.push(format!("{latex}: should say {should:?}, says {got:?}"));
        }
    }
    if !failures.is_empty() {
        eprintln!("{}", failures.join("\n"));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
