// SPDX-License-Identifier: Apache-2.0
//! Maths written without `$`: in a fragment such as `\beta = 43000`, a run of symbols around a maths command is read as
//! an equation. TeX would stop with an error on such input; a person who types it means maths.

/// Commands that occur only in maths. A run of tokens counts as an equation when it holds one of them.
const MATH_ONLY: &[&str] = &[
    "alpha",
    "beta",
    "gamma",
    "delta",
    "epsilon",
    "varepsilon",
    "zeta",
    "eta",
    "theta",
    "vartheta",
    "iota",
    "kappa",
    "lambda",
    "mu",
    "nu",
    "xi",
    "pi",
    "varpi",
    "rho",
    "varrho",
    "sigma",
    "varsigma",
    "tau",
    "upsilon",
    "phi",
    "varphi",
    "chi",
    "psi",
    "omega",
    "Gamma",
    "Delta",
    "Theta",
    "Lambda",
    "Xi",
    "Pi",
    "Sigma",
    "Upsilon",
    "Phi",
    "Psi",
    "Omega",
    "frac",
    "dfrac",
    "tfrac",
    "sqrt",
    "sum",
    "prod",
    "int",
    "iint",
    "oint",
    "partial",
    "nabla",
    "infty",
    "cdot",
    "cdots",
    "times",
    "div",
    "pm",
    "mp",
    "leq",
    "geq",
    "le",
    "ge",
    "neq",
    "ne",
    "approx",
    "sim",
    "simeq",
    "equiv",
    "propto",
    "ll",
    "gg",
    "in",
    "notin",
    "subset",
    "subseteq",
    "supset",
    "cup",
    "cap",
    "to",
    "rightarrow",
    "leftarrow",
    "Rightarrow",
    "Leftarrow",
    "leftrightarrow",
    "mapsto",
    "forall",
    "exists",
    "lim",
    "sin",
    "cos",
    "tan",
    "exp",
    "ln",
    "log",
    "min",
    "max",
    "sup",
    "inf",
    "det",
    "mathbf",
    "mathrm",
    "mathcal",
    "mathbb",
    "boldsymbol",
    "vec",
    "hat",
    "bar",
    "tilde",
    "dot",
    "ddot",
    "overline",
    "underline",
    "left",
    "right",
    "langle",
    "rangle",
    "ell",
    "hbar",
    "circ",
    "bullet",
    "star",
    "ast",
    "perp",
    "parallel",
    "angle",
    "emptyset",
    "operatorname",
];

/// Commands that may appear inside an equation but do not make a run an equation.
const MATH_NEUTRAL: &[&str] = &[
    "text", "mathit", "mathsf", "mathtt", "quad", "qquad", "ldots", "dots", "lbrace", "rbrace",
];

/// Characters of an equation that are not letters or digits.
const OPERATORS: &str = "=+-*/<>^_()[]|:;!'.,";

/// Most tokens in one run.
const MAX_TOKENS: usize = 80;

/// How far the token at `from` goes, and whether it can belong to an equation. `math` is set when it holds a
/// command from [`MATH_ONLY`].
fn token(t: &str, from: usize, end: usize, math: &mut bool) -> Option<usize> {
    let b = t.as_bytes();
    let mut i = from;
    let mut letters = 0usize;
    while i < end {
        let c = b[i];
        match c {
            b' ' | b'\t' | b'\n' | b'\r' => break,
            b'\\' => {
                letters = 0;
                let s = i + 1;
                let mut j = s;
                while j < end && b[j].is_ascii_alphabetic() {
                    j += 1;
                }
                if j == s {
                    // \, \; \! \| and the like
                    if s < end && matches!(b[s], b',' | b';' | b'!' | b'|' | b':' | b'{' | b'}') {
                        i = s + 1;
                        continue;
                    }
                    return None;
                }
                let name = &t[s..j];
                if MATH_ONLY.contains(&name) {
                    *math = true;
                } else if !MATH_NEUTRAL.contains(&name) {
                    return None;
                }
                i = j;
            }
            b'{' => {
                letters = 0;
                let mut depth = 0usize;
                let mut j = i;
                while j < end {
                    match b[j] {
                        b'{' => depth += 1,
                        b'}' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        b'$' | b'%' => return None,
                        _ => {}
                    }
                    j += 1;
                }
                if j >= end {
                    return None;
                }
                i = j + 1;
            }
            c if c.is_ascii_alphabetic() => {
                letters += 1;
                if letters > 1 {
                    return None;
                }
                i += 1;
            }
            c if c.is_ascii_digit() => {
                letters = 0;
                i += 1;
            }
            c if OPERATORS.as_bytes().contains(&c) => {
                letters = 0;
                i += 1;
            }
            _ => return None,
        }
    }
    Some(i)
}

/// The end of the equation that starts at byte `from` of `t[..end]`, when the text from there on is a run of
/// symbols that holds a maths command. The caller calls this at the start of a word. Trailing punctuation is not
/// part of the equation.
pub(crate) fn run_at(t: &str, from: usize, end: usize) -> Option<usize> {
    let b = t.as_bytes();
    let mut i = from;
    let mut stop = from;
    let mut math = false;
    let mut tokens = 0usize;
    while i < end && tokens < MAX_TOKENS {
        let mut m = false;
        let Some(z) = token(t, i, end, &mut m) else {
            break;
        };
        if z == i {
            break;
        }
        math |= m;
        stop = z;
        tokens += 1;
        i = z;
        // blanks within one line, or one line break, join tokens
        let mut newlines = 0;
        while i < end && matches!(b[i], b' ' | b'\t' | b'\n' | b'\r') {
            if b[i] == b'\n' {
                newlines += 1;
            }
            i += 1;
        }
        if newlines > 1 {
            break;
        }
    }
    if !math {
        return None;
    }
    while stop > from && matches!(b[stop - 1], b'.' | b',' | b';' | b':') {
        stop -= 1;
    }
    (stop > from).then_some(stop)
}

/// True when byte `i` of `t` starts a word: the byte before it is not part of a word.
pub(crate) fn at_word_start(t: &str, i: usize) -> bool {
    i == 0
        || t.as_bytes().get(i - 1).is_some_and(|p| {
            !(p.is_ascii_alphanumeric() || *p >= 0x80 || matches!(p, b'\\' | b'{' | b'$'))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(s: &str) -> Option<&str> {
        run_at(s, 0, s.len()).map(|z| &s[..z])
    }

    #[test]
    fn a_bare_equation_is_found() {
        assert_eq!(run(r"\beta = 43000"), Some(r"\beta = 43000"));
        assert_eq!(run(r"\beta = 43000."), Some(r"\beta = 43000"));
        assert_eq!(run(r"x = \alpha + 2 and so on"), Some(r"x = \alpha + 2"));
        assert_eq!(run(r"\frac{a}{b}, then"), Some(r"\frac{a}{b}"));
    }

    #[test]
    fn plain_words_are_not_maths() {
        assert_eq!(run("take the value"), None);
        assert_eq!(run("I take it"), None);
        assert_eq!(run(r"\emph{word} = 1"), None);
        assert_eq!(run("x = 1"), None);
    }
}
