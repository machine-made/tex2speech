// SPDX-License-Identifier: Apache-2.0
//! Readings that were wrong or poor on real papers, each checked on a small general input: the rule behind the
//! reading, not one equation. No text of a paper is copied here.

use tex2speech::{Document, Format, Options};

fn latex(src: &str) -> String {
    Document::parse_str(src, Format::Latex, &Options::new()).to_text()
}

fn markdown(src: &str) -> String {
    Document::parse_str(src, Format::Markdown, &Options::new()).to_text()
}

fn eq(src: &str) -> String {
    latex(&format!("${src}$")).trim_end().to_string()
}

#[test]
fn a_pair_is_not_an_interval() {
    for src in [r"(a, b)", r"(R, Re_D)", r"(x, y) = (1, 2)"] {
        let t = eq(src);
        assert!(!t.contains("interval"), "{src}: {t}");
    }
    assert_eq!(eq(r"(a, b)"), "a, b.");
    assert_eq!(eq(r"(x, y) = (1, 2)"), "x, y equal to one, two.");
}

#[test]
fn an_interval_follows_a_set_relation() {
    assert_eq!(
        eq(r"x \in (0, 1)"),
        "x in the open interval from zero to one."
    );
    assert_eq!(
        eq(r"x \notin (a, b)"),
        "x not in the open interval from a to b."
    );
    assert_eq!(
        eq(r"(a, b) \subset S"),
        "The open interval from a to b is a subset of capital S."
    );
    assert_eq!(
        eq(r"x \in [a, b)"),
        "x in the half-open interval from a to b, including a."
    );
    assert_eq!(
        eq(r"x \in (a, b]"),
        "x in the half-open interval from a to b, including b."
    );
}

#[test]
fn brackets_after_a_function_hold_arguments() {
    assert_eq!(eq(r"E[a, b]"), "Capital E of a, b.");
    assert_eq!(eq(r"x \in [a, b]"), "x in the closed interval from a to b.");
}

#[test]
fn bare_maths_commands_read_as_maths() {
    assert_eq!(
        latex(r"\beta = 43000").trim_end(),
        "Beta equal to forty-three thousand."
    );
    assert_eq!(
        latex("We take \\beta = 43000 here.").trim_end(),
        "We take beta equal to forty-three thousand here."
    );
    assert_eq!(
        latex(r"Let x = \alpha + 2, then \frac{a}{b} > 1.").trim_end(),
        "Let x equal to alpha plus two, then a over b greater than one."
    );
    assert_eq!(
        markdown(r"\beta = 43000").trim_end(),
        "Beta equal to forty-three thousand."
    );
    // plain words around a lone symbol stay prose
    assert_eq!(
        latex("It was x = 1 today.").trim_end(),
        "It was x = one today."
    );
}

#[test]
fn a_document_keeps_its_text_mode() {
    let t = latex(
        "\\documentclass{article}\n\\begin{document}\nThe command \\textbackslash beta = 1 here.\n\\end{document}\n",
    );
    assert!(!t.contains("equal to"), "{t}");
}

#[test]
fn accents_read_as_words_on_any_base() {
    assert_eq!(eq(r"\vec{v}"), "Vector v.");
    assert_eq!(eq(r"\dot{C_y}"), "Capital C y dot.");
    assert_eq!(eq(r"\dot{\bar{u}}"), "u bar dot.");
    assert_eq!(eq(r"\hat{\theta}"), "Theta hat.");
    assert_eq!(eq(r"\vec{k}_{\parallel}"), "Vector k, parallel.");
}

#[test]
fn labelled_arrows_have_no_empty_label() {
    let t = eq(r"X_n \xrightarrow{\mathrm{d}} X");
    assert!(!t.contains("empty") && !t.contains("above above"), "{t}");
    assert!(t.contains("labelled"), "{t}");
}

#[test]
fn cases_read_as_conditions() {
    assert_eq!(
        eq(r"\begin{cases} a & x > 0 \\ b & \text{otherwise} \end{cases}"),
        "a if x greater than zero, b otherwise."
    );
    assert_eq!(
        eq(r"f(x) = \begin{cases} 1 & \text{if } x > 0 \\ 0 & \text{else} \end{cases}"),
        "f of x, equal to, one if x greater than zero, zero else."
    );
    assert_eq!(
        eq(r"\begin{cases} x^2, & x \ge 0 \\ -x, & x < 0 \end{cases}"),
        "x squared, if x greater than or equal to zero, negative x, if x less than zero."
    );
}

#[test]
fn matrices_and_arrays_of_any_size_read_by_rows() {
    assert_eq!(
        eq(r"\begin{matrix} a & b & c \\ d & e & f \\ g & h & i \end{matrix}"),
        "The three by three matrix with rows a, b, c, and d, e, f, and g, h, i."
    );
    assert_eq!(
        eq(r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}"),
        "The two by two matrix with rows a, b, and c, d."
    );
    assert_eq!(
        eq(r"\begin{array}{cc} a & b \\ c & d \\ e & f \end{array}"),
        "The three by two matrix with rows a, b, and c, d, and e, f."
    );
    assert_eq!(
        eq(r"\begin{bmatrix} a_{11} & a_{12} \\ a_{21} & a_{22} \end{bmatrix}"),
        "The two by two matrix with rows a eleven, a twelve, and a twenty-one, a twenty-two."
    );
}

#[test]
fn slanted_relations_read_as_the_plain_ones() {
    assert_eq!(eq(r"a \leqslant b"), "a less than or equal to b.");
    assert_eq!(eq(r"a \geqslant b"), "a greater than or equal to b.");
}

#[test]
fn sans_serif_words_in_maths_are_words() {
    assert_eq!(eq(r"\mathsf{err} = 1"), "Err equal to one.");
}

#[test]
fn a_list_numeral_keeps_its_case() {
    assert_eq!(latex("(ii) Second item.").trim_end(), "(ii) Second item.");
}

#[test]
fn a_term_is_not_said_twice() {
    assert_eq!(
        latex("At the Reynolds number $Re_D = 5$ we see it.").trim_end(),
        "At the Reynolds number capital D equal to five we see it."
    );
}

#[test]
fn upright_greek_and_dots_variants_read() {
    assert_eq!(eq(r"\upmu + \uppi"), "Mu plus pi.");
    assert!(!eq(r"a_1 \dotsb a_n").contains("dotsb"));
}

#[test]
fn a_substack_reads_as_its_lines() {
    let t = eq(r"\sum_{\substack{i < n \\ j > m}} x");
    assert!(!t.contains("array of"), "{t}");
    assert_eq!(t, "Sum over i less than n, j greater than m of x.");
}

#[test]
fn a_norm_is_a_norm() {
    assert_eq!(eq(r"\norm{A}"), "Norm of capital A.");
    assert_eq!(eq(r"\|A\|"), "Norm of capital A.");
    assert_eq!(eq(r"\|A\|_2"), "Two norm of capital A.");
    assert_eq!(eq(r"\abs{x}"), "The absolute value of x.");
    assert_eq!(eq(r"\floor{x}"), "Floor of x.");
}

#[test]
fn one_symbol_in_brackets_has_no_brackets() {
    assert_eq!(eq(r"(a)"), "a.");
    assert_eq!(eq(r"(1)"), "One.");
    // brackets that group a sum are still said where nothing else shows the grouping
    assert!(eq(r"a-(b+c)").contains("open paren"));
    assert!(eq(r"(a+(b+c))d").contains("open paren"));
}

#[test]
fn a_parenthesised_superscript_is_an_index() {
    assert_eq!(eq(r"x^{(j)}"), "x superscript j.");
    assert_eq!(
        eq(r"y^{(m+1)}"),
        "y superscript m plus one end superscript."
    );
    assert_eq!(eq(r"x^2"), "x squared.");
}

#[test]
fn a_reference_after_a_full_stop_starts_a_sentence() {
    let t =
        latex("The result is shown in~\\cite{a}. \\citet{b} further showed it. See Fig. \\ref{x}.");
    let lines: Vec<&str> = t.lines().collect();
    assert_eq!(lines[0], "The result is shown in reference 1.");
    assert_eq!(lines[1], "Reference 2 further showed it.");
}

#[test]
fn skipped_commands_leave_no_words() {
    for cmd in [
        r"\orcidlink{0000-0001-2345-6789}",
        r"\DeclareMathAlphabet{\mathpzc}{OT1}{pzc}{m}{it}",
        r"\SetMathAlphabet{\mathsf}{bold}{OT1}{cmss}{bx}{n}",
        r"\fontsize{10}{12}\selectfont",
        r"\counterwithin*{equation}{section}",
        r"\displaywidowpenalty=10000",
        r"\emergencystretch=3em",
        r"\hrule",
        r"\Needspace{3\baselineskip}",
    ] {
        let t = latex(&format!("{cmd} Text."));
        assert_eq!(t.trim_end(), "Text.", "{cmd}: {t}");
    }
}

#[test]
fn numeric_conditionals_are_evaluated() {
    assert_eq!(latex(r"\ifnum 2>1 yes\else no\fi.").trim_end(), "Yes.");
    assert_eq!(latex(r"\ifnum 1>2 yes\else no\fi.").trim_end(), "No.");
    assert_eq!(latex(r"\ifnum 3=3 yes\fi.").trim_end(), "Yes.");
    assert_eq!(latex(r"\ifodd 3 odd\else even\fi.").trim_end(), "Odd.");
}

#[test]
fn class_environments_made_of_tex_internals_keep_their_reading() {
    let src = r"\documentclass{article}
\newenvironment{subeqnarray}{\stepcounter{equation}\global\@eqnswtrue $$\halign{##\cr}{\@@subeqncr $$\global\@ignoretrue}
\begin{document}
\begin{subeqnarray}
a &=& b, \\
c &=& d.
\end{subeqnarray}
\end{document}
";
    let t = latex(src);
    assert!(t.contains("a equal to b"), "{t}");
    assert!(t.contains("c equal to d"), "{t}");
    assert!(!t.contains("ignoretrue") && !t.contains("eqnsw"), "{t}");
}

#[test]
fn a_command_saved_with_let_keeps_its_meaning_when_redefined() {
    let src = r"\documentclass{article}
\let\plainref\ref
\renewcommand{\ref}[1]{Fig.~\plainref{#1}}
\begin{document}
\section{A}\label{s}
See \ref{s}.
\end{document}
";
    let doc = Document::parse_str(src, Format::Latex, &Options::new());
    assert!(
        doc.warnings().iter().all(|w| w.construct != "nesting"),
        "{:?}",
        doc.warnings()
    );
    assert!(doc.to_text().contains("See "), "{}", doc.to_text());
}

#[test]
fn the_thread_option_does_not_change_the_output() {
    let src = "$a = 1$ and $b = 2$ and $\\sum_i x_i$ and $\\frac{a}{b}$.";
    let one = Document::parse_str(src, Format::Latex, &Options::new().with_threads(1));
    let four = Document::parse_str(src, Format::Latex, &Options::new().with_threads(4));
    assert_eq!(one.to_json(), four.to_json());
}

#[test]
fn deep_nesting_warns_and_fits_in_a_small_stack() {
    // half of the default thread stack, in a build without optimisation; the reader needs no larger stack of its own
    let handle = std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let inputs = [
                ("{".repeat(50_000), Format::Latex),
                ("\\begin{itemize}".repeat(2_000), Format::Latex),
                ("\\begin{quote}".repeat(2_000), Format::Latex),
                ("\\textbf{".repeat(2_000), Format::Latex),
                ("\\def\\a{{\\a}}\\a".to_string(), Format::Latex),
                ("> ".repeat(3_000), Format::Markdown),
                ("- ".repeat(3_000), Format::Markdown),
            ];
            let mut warned = false;
            for (text, format) in inputs {
                let doc = Document::parse_str(&text, format, &Options::new());
                warned |= doc.warnings().iter().any(|w| w.construct == "nesting");
            }
            warned
        })
        .expect("thread starts");
    assert!(
        handle.join().expect("no stack overflow"),
        "no nesting warning"
    );
}

#[test]
fn an_equation_nested_too_deeply_is_read_by_the_fallback() {
    let deep = format!("${}x{}$", "{".repeat(5_000), "}".repeat(5_000));
    let doc = Document::parse_str(&deep, Format::Latex, &Options::new());
    assert!(doc.warnings().iter().any(|w| w.construct == "nesting"));
}

#[test]
fn a_single_numbered_heading_is_a_chapter() {
    for (src, want) in [
        ("# 1\n\nText.\n", "Chapter 1.\nText.\n"),
        ("# 1 Intro\n\nText.\n", "Chapter 1: Intro.\nText.\n"),
        ("## 1\n\nText.\n", "Section 1.\nText.\n"),
        ("### 2.3\n\nText.\n", "Subsection 2.3.\nText.\n"),
        ("# Title\n\nText.\n", "Title.\nText.\n"),
    ] {
        assert_eq!(markdown(src), want, "{src:?}");
    }
}

#[test]
fn commas_between_parts_are_pauses_not_words() {
    assert_eq!(eq(r"(R, Re_D)"), "Capital R, Reynolds number capital D.");
    assert_eq!(
        eq(r"(U_\infty, 0, 0) = (1, 0, 0)"),
        "Capital U infinity, zero, zero, equal to, one, zero, zero."
    );
    assert_eq!(eq(r"f(x, y)"), "f of x, y.");
    assert_eq!(
        eq(r"x_1, x_2, \dots, x_n"),
        "x one, x two, dot dot dot, x n."
    );
    for src in [
        r"(a, b, c)",
        r"f(x, y, z)",
        r"\{1, 2, 3\}",
        r"x_{i,j}",
        r"a, b,",
    ] {
        assert!(!eq(src).contains("comma"), "{src}: {}", eq(src));
    }
}

#[test]
fn other_separators_are_not_spoken_as_punctuation() {
    assert_eq!(eq(r"f(x; y)"), "f of x; y.");
    assert_eq!(eq(r"A \setminus B"), "Capital A set minus capital B.");
    assert_eq!(eq(r"A \backslash B"), "Capital A set minus capital B.");
}

#[test]
fn expectation_and_probability_read_as_words() {
    assert_eq!(eq(r"\mathbb{E}[X]"), "Expectation of capital X.");
    assert_eq!(eq(r"\mathbb{P}(A)"), "Probability of capital A.");
    assert_eq!(
        eq(r"\mathbb{P}(X \in A)"),
        "Probability of capital X in capital A."
    );
    assert_eq!(eq(r"\mathbb{E}_x[f(x)]"), "Expectation over x of f of x.");
    assert_eq!(eq(r"\mathbb{E}"), "Capital E.");
    assert_eq!(eq(r"\mathbb{P}"), "Capital P.");
    assert_eq!(eq(r"\mathbb{R}^n"), "R n.");
    assert_eq!(eq(r"x \in \mathbb{R}"), "x in the real numbers.");
    assert_eq!(eq(r"\mathbb{N}"), "The natural numbers.");
    assert_eq!(eq(r"\mathbb{Z}"), "The integers.");
}

#[test]
fn a_bar_inside_brackets_is_a_condition() {
    assert_eq!(eq(r"p(x \mid y)"), "p of x given y.");
    assert_eq!(eq(r"P(A|B)"), "Capital P of capital A given capital B.");
    assert_eq!(
        eq(r"\mathbb{E}[X \mid Y]"),
        "Expectation of capital X given capital Y."
    );
    assert_eq!(eq(r"a \mid b"), "a divides b.");
}

#[test]
fn determinants_braces_and_perpendicular_scripts_read_as_a_person_says_them() {
    assert_eq!(
        eq(r"\begin{vmatrix} a & b \\ c & d \end{vmatrix}"),
        "The two by two determinant with rows a, b, and c, d."
    );
    assert_eq!(eq(r"\det A"), "Determinant of capital A.");
    assert_eq!(eq(r"|S|"), "The absolute value of capital S.");
    assert_eq!(eq(r"\overbrace{a+b}^{n}"), "a plus b, labelled n.");
    assert_eq!(eq(r"\underbrace{a+b}_{m}"), "a plus b, labelled m.");
    assert_eq!(eq(r"\underbrace{a+b}"), "a plus b.");
    assert_eq!(eq(r"k_\perp"), "k perpendicular.");
    assert_eq!(eq(r"A^\perp"), "Capital A perp.");
    assert_eq!(eq(r"a \perp b"), "a is perpendicular to b.");
}

#[test]
fn function_spaces_indices_and_inverses_are_not_powers() {
    assert_eq!(eq(r"H^1(\Omega)"), "Capital H one of capital omega.");
    assert_eq!(eq(r"W^{1,p}(\Omega)"), "Capital W one, p of capital omega.");
    assert_eq!(
        eq(r"L^\infty(\Omega)"),
        "Capital L infinity of capital omega."
    );
    assert_eq!(
        eq(r"H^{-1}(\Omega)"),
        "Capital H minus one of capital omega."
    );
    assert_eq!(
        eq(r"\mathcal{R}^{\mathrm{out}}(u)"),
        "Script capital R out of u."
    );
    assert_eq!(eq(r"\tau^1(x)"), "Tau one of x.");
    assert_eq!(eq(r"f^{-1}(x)"), "f inverse of x.");
    // a power of a lower-case function stays a power
    assert_eq!(eq(r"f^2(x)"), "f squared of x.");
    assert_eq!(eq(r"f^n(x)"), "The n power of, f of x.");
}

#[test]
fn marks_and_names_read_as_a_person_says_them() {
    assert_eq!(eq(r"\mathbb{Z}_2"), "Z two.");
    assert_eq!(eq(r"\mathbb{R}_+"), "R plus.");
    assert_eq!(
        eq(r"G \rtimes H"),
        "Capital G semidirect product capital H."
    );
    assert_eq!(eq(r"\delta_*"), "Delta star.");
    assert_eq!(eq(r"M_\odot"), "Capital M sun.");
    assert_eq!(eq(r"\ell = 80"), "Ell equal to eighty.");
    assert_eq!(eq(r"y^\circ(u)"), "y circle of u.");
    assert_eq!(eq(r"90^\circ"), "Ninety degrees.");
    assert_eq!(eq(r"\mathbb{1}_A"), "Indicator of capital A.");
    assert_eq!(eq(r"\mathbb{S}^{d-1}"), "Capital S to the d minus one.");
    assert_eq!(eq(r"\mathcal{O}(h^2)"), "Order h squared.");
    assert_eq!(eq(r"P_1^\mu"), "Capital P one to the mu-th power.");
    assert_eq!(
        eq(r"N \sim 10^7"),
        "Capital N tilde ten to the seventh power."
    );
    assert_eq!(eq(r"\sim 5"), "About five.");
    assert_eq!(eq(r"\bm{\nabla} p"), "Grad p.");
}

#[test]
fn text_and_equations_that_touch_in_the_source_are_spoken_apart() {
    assert_eq!(
        latex(r"The $\Lambda$CDM model at 5 deg$^2$ and the $n$th term."),
        "The capital lambda CDM model at five deg squared and the nth term.\n"
    );
    // an abbreviation before an equation does not end the sentence
    assert_eq!(
        latex(r"Weak (i.e. $x<1$) and so on, e.g. $y$ too."),
        "Weak (that is x less than one) and so on, for example y too.\n"
    );
    // "Ref." before a citation is not said twice
    assert_eq!(
        latex(
            "See Ref. \\cite{a} and Ref.~\\cite{b}.\n\n\\begin{thebibliography}{9}\\bibitem{a} A\\bibitem{b} B\\end{thebibliography}"
        ),
        "See Reference 1 and Reference 2.\n"
    );
    // the article and "the ratio of" are not said twice
    assert_eq!(
        latex(r"This is of order $\mathcal{O}(h)$ and a $7:2$ lock."),
        "This is of order h and a ratio of seven to two lock.\n"
    );
}

#[test]
fn spacing_commands_inside_an_equation_say_nothing() {
    assert_eq!(
        latex(
            "\\documentclass{article}\\begin{document}\n\\[\n\\setlength{\\arraycolsep}{0pt}\\renewcommand{\\arraystretch}{1.3}\nx = 1\n\\]\n\\end{document}\n"
        ),
        "x equal to one.\n"
    );
}

#[test]
fn a_reference_after_its_type_word_is_a_number() {
    let t = latex(
        "\\documentclass{article}\\newtheorem{assumption}{Assumption}\\begin{document}\n\\begin{assumption}\\label{a}Text.\\end{assumption}\n\nAssumption~\\ref{a} holds.\n\\end{document}\n",
    );
    assert!(t.contains("Assumption 1 holds."), "{t}");
    assert!(!t.contains("ssumption assumption"), "{t}");
}

#[test]
fn a_sum_in_a_subscript_has_no_end_sub() {
    assert_eq!(eq(r"x_{n+1}"), "x sub n plus one.");
    assert_eq!(eq(r"a_{n-1} + b"), "a sub n minus one, plus b.");
}

#[test]
fn parentheses_around_a_sum_give_way_to_pauses_and_words() {
    assert_eq!(eq(r"(a+b)(c+d)"), "a plus b, times, c plus d.");
    assert_eq!(eq(r"(a+b)^2"), "a plus b, all squared.");
    assert_eq!(eq(r"(a+b)^3"), "a plus b, all cubed.");
    assert_eq!(eq(r"(a+b)^n"), "a plus b, all to the n-th power.");
    assert_eq!(eq(r"2(x+1)"), "Two times, x plus one.");
    assert_eq!(eq(r"(a+b)c"), "a plus b, times c.");
    // a sum inside a function call, after a minus, or with brackets inside keeps its words
    assert_eq!(eq(r"f(a+b)"), "f of a plus b.");
    assert!(eq(r"a-(b+c)").contains("open paren"));
    assert!(eq(r"(a+(b+c))d").contains("open paren"));
}

#[test]
fn a_fraction_reads_with_pauses_and_nested_ones_keep_the_long_form() {
    assert_eq!(eq(r"\frac{a+b}{c+d}"), "a plus b, over, c plus d.");
    assert_eq!(eq(r"\frac{a}{b+c}"), "a, over, b plus c.");
    assert!(eq(r"\frac{a}{\frac{b}{c}+1}").contains("fraction with numerator"));
}

#[test]
fn an_ordinal_suffix_after_an_equation_in_parentheses_is_a_word() {
    let t = latex(r"The $(N-2)$th-order scheme and the $(n+1)$st step.");
    assert!(t.contains("capital N minus two th"), "{t}");
    assert!(t.contains("n plus one st step"), "{t}");
    assert!(!t.contains("paren"), "{t}");
    assert!(latex(r"The $n$th root.").contains("nth root"));
}

#[test]
fn an_array_after_a_relation_reads_as_a_matrix() {
    let t = eq(r"x = \begin{array}{cc} a & b \\ c & d \end{array}");
    assert_eq!(
        t,
        "x equal to the two by two matrix with rows a, b, and c, d."
    );
    assert_eq!(
        eq(r"\begin{array}{cc} a & b \\ c & d \end{array}"),
        "The two by two matrix with rows a, b, and c, d."
    );
}

#[test]
fn marks_under_symbols_and_implications_read_as_words() {
    assert_eq!(eq(r"\underline{\mu}"), "Mu underlined.");
    assert_eq!(eq(r"A \Rightarrow B"), "Capital A implies capital B.");
    assert_eq!(eq(r"e^{-i\omega t}"), "e to the negative i omega t.");
}

#[test]
fn re_before_a_bracket_is_the_real_part() {
    assert_eq!(eq(r"\mathrm{Re}(\lambda)"), "The real part of lambda.");
    assert_eq!(eq(r"\mathrm{Im}[\lambda]"), "The imaginary part of lambda.");
    let t = latex(
        r"\renewcommand{\Re}{\mathrm{Re}} $\Re\lambda$ and $\Re(\lambda)$ and the Reynolds number $\mathrm{Re}_D$.",
    );
    assert!(!t.contains("Reynolds number lambda"), "{t}");
    assert_eq!(t.matches("real part of lambda").count(), 2, "{t}");
    assert!(t.contains("Reynolds number capital D"), "{t}");
}

#[test]
fn units_apply_only_outside_subscripts() {
    assert_eq!(eq(r"M_{200\mathrm{m}}"), "Capital M sub two hundred m.");
    assert!(eq(r"L = 5\,\mathrm{m}").contains("metres"));
}

#[test]
fn set_builder_bars_and_colons_read_such_that() {
    assert_eq!(
        eq(r"\{x \mid x>0\}"),
        "The set of all x such that x greater than zero."
    );
    assert_eq!(
        eq(r"\{ t : x(t) \in S, t>0 \}"),
        "The set of all t such that x of t in capital S, t greater than zero."
    );
    assert_eq!(
        eq(r"\{ t_i : i=1,2 \}"),
        "The set of all t i such that i equal to one, two."
    );
}

#[test]
fn a_bar_after_an_empty_left_fence_is_an_evaluation() {
    assert_eq!(eq(r"\left. f \right|_{x=0}"), "f at x equal to zero.");
    assert_eq!(
        eq(r"\left. \frac{df}{dx} \right|_{x=0}"),
        "d f over d x at x equal to zero."
    );
}

#[test]
fn floor_and_ceiling_are_functions() {
    assert_eq!(eq(r"\lfloor x \rfloor"), "Floor of x.");
    assert_eq!(eq(r"\lceil x \rceil + 1"), "Ceiling of x, plus one.");
    assert!(eq(r"2^{\lceil \log_2 h \rceil}").contains("ceiling of"));
}

#[test]
fn the_lines_under_a_big_operator_are_conditions() {
    let t = eq(r"\sum_{\substack{i=1 \\ j>i}} a");
    assert!(!t.contains("line"), "{t}");
    assert!(t.contains(", and j"), "{t}");
}

// ----- readings that were wrong -----

#[test]
fn a_definition_is_read_as_defined_as() {
    assert_eq!(eq(r"x := 1"), "x defined as one.");
    assert_eq!(eq(r"x \coloneqq 1"), "x defined as one.");
    assert_eq!(eq(r"f(t) := t^2"), "f of t defined as t squared.");
}

#[test]
fn a_dot_in_place_of_an_argument_is_named() {
    assert_eq!(eq(r"\|\cdot\|"), "Norm of dot.");
    assert_eq!(eq(r"f(\cdot)"), "f of dot.");
    assert_eq!(eq(r"|\cdot|"), "The absolute value of dot.");
    // a product keeps its sign, and a row of dots is not a placeholder
    assert_eq!(eq(r"a \cdot b"), "a times b.");
    assert_eq!(eq(r"a \cdots b"), "a dot dot dot b.");
}

#[test]
fn braces_around_one_comparison_are_an_event() {
    assert_eq!(eq(r"\{a > b\}"), "The event a greater than b.");
    assert_eq!(
        eq(r"\{X_n \geq t\}"),
        "The event capital X n greater than or equal to t."
    );
    // a list or a condition is still a set
    assert_eq!(eq(r"\{a, b\}"), "The set a, b.");
    assert_eq!(
        eq(r"\{x : x > 0\}"),
        "The set of all x such that x greater than zero."
    );
}

#[test]
fn re_before_a_symbol_is_the_real_part() {
    assert_eq!(eq(r"\operatorname{Re}\lambda"), "The real part of lambda.");
    assert_eq!(eq(r"\operatorname{Re} z"), "The real part of z.");
    assert_eq!(eq(r"\mathrm{Re}\,\lambda"), "The real part of lambda.");
    assert_eq!(
        eq(r"\operatorname{Im}\lambda > 0"),
        "The imaginary part of lambda greater than zero."
    );
    // a relation, a script or the end of the formula after it means the Reynolds number
    assert_eq!(
        eq(r"\mathrm{Re} = 100"),
        "Reynolds number equal to one hundred."
    );
    assert_eq!(eq(r"\mathrm{Re}_\tau"), "Reynolds number tau.");
}

#[test]
fn a_function_name_with_an_index_takes_its_argument() {
    assert_eq!(eq(r"V_1(\nu)"), "Capital V one of nu.");
    assert_eq!(eq(r"f_n(x)"), "f n of x.");
}

#[test]
fn a_sign_before_a_bracketed_sum_that_ends_the_term_is_said_once() {
    assert_eq!(eq(r"-(a+b)"), "Minus, a plus b.");
    assert_eq!(eq(r"y = -(a+b)"), "y equal to minus, a plus b.");
    // more follows, or the sum is a power: the brackets stay or the power says "all"
    assert_eq!(
        eq(r"y = -(a+b) + c"),
        "y equal to minus open paren a plus b close paren plus c."
    );
    assert_eq!(eq(r"-(a+b)^2"), "Negative a plus b, all squared.");
    assert_eq!(eq(r"a - (b+c)"), "a minus open paren b plus c close paren.");
}

#[test]
fn a_subscript_ends_with_a_pause_where_nothing_could_join_it() {
    assert_eq!(eq(r"x_{n^2}"), "x sub n squared.");
    assert_eq!(eq(r"x_{n^2} = 1"), "x sub n squared, equal to one.");
    assert_eq!(eq(r"x_{n^2}, y"), "x sub n squared; y.");
    // a factor, a sum or a fraction can still be taken for part of the subscript
    assert!(eq(r"x_{n^2} y").contains("end sub"));
    assert!(eq(r"x_{n^2} + 1").contains("end sub"));
    assert!(eq(r"\frac{a_{n^2}}{b}").contains("end sub"));
}

#[test]
fn aligned_definitions_are_lines_not_an_array() {
    let t = latex("\\begin{align}\nE(f)&:=a^2,\\\\\nD(f)&:=E(f)-b.\n\\end{align}\n");
    assert!(!t.contains("array") && !t.contains("column"), "{t}");
    assert!(t.contains("defined as"), "{t}");
}
