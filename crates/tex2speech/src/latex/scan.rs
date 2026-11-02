// SPDX-License-Identifier: Apache-2.0
//! Scanners over LaTeX source text. Every function is bounded by `end` and never panics on odd input.

/// Next index after `i` that starts a character.
pub(super) fn next_char(t: &str, i: usize) -> usize {
    let mut j = i + 1;
    while j < t.len() && !t.is_char_boundary(j) {
        j += 1;
    }
    j.min(t.len())
}

fn is_comment_start(t: &[u8], i: usize) -> bool {
    t[i] == b'%'
}

/// Skip to just after the end of the line that holds a comment starting at `i`.
pub(super) fn skip_comment(t: &str, i: usize, end: usize) -> usize {
    match t.as_bytes()[i..end].iter().position(|b| *b == b'\n') {
        Some(p) => i + p + 1,
        None => end,
    }
}

/// Skip white space, including single line breaks and comments.
pub(super) fn skip_ws(t: &str, mut i: usize, end: usize) -> usize {
    let b = t.as_bytes();
    let mut newlines = 0;
    while i < end {
        match b[i] {
            b' ' | b'\t' | b'\r' => i += 1,
            b'\n' => {
                newlines += 1;
                if newlines > 1 {
                    break;
                }
                i += 1;
            }
            b'%' => {
                i = skip_comment(t, i, end);
                newlines = 0;
            }
            _ => break,
        }
    }
    i
}

/// `{...}` starting at `i`: (inner start, inner end, index after). Braces must balance.
pub(super) fn read_braced(t: &str, i: usize, end: usize) -> Option<(usize, usize, usize)> {
    let b = t.as_bytes();
    if i >= end || b[i] != b'{' {
        return None;
    }
    let mut depth = 0usize;
    let mut j = i;
    while j < end {
        match b[j] {
            b'\\' => j += 1,
            b'%' if is_comment_start(b, j) => {
                j = skip_comment(t, j, end);
                continue;
            }
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((i + 1, j, j + 1));
                }
            }
            _ => {}
        }
        j += 1;
    }
    // unbalanced: take the rest
    Some((i + 1, end, end))
}

/// `[...]` starting at `i`, with balanced braces inside.
pub(super) fn read_bracket(t: &str, i: usize, end: usize) -> Option<(usize, usize, usize)> {
    let b = t.as_bytes();
    if i >= end || b[i] != b'[' {
        return None;
    }
    let mut depth = 0i32;
    let mut j = i + 1;
    while j < end {
        match b[j] {
            b'\\' => j += 1,
            b'{' => depth += 1,
            b'}' => depth -= 1,
            b']' if depth <= 0 => return Some((i + 1, j, j + 1)),
            b'\n' if j + 1 < end && b[j + 1] == b'\n' => return None,
            _ => {}
        }
        j += 1;
    }
    None
}

/// A command argument: a braced group, or one token. Returns (start, end, index after).
pub(super) fn read_arg(t: &str, i: usize, end: usize) -> Option<(usize, usize, usize)> {
    let i = skip_ws(t, i, end);
    if i >= end {
        return None;
    }
    let b = t.as_bytes();
    match b[i] {
        b'{' => read_braced(t, i, end),
        b'\\' => {
            let mut j = i + 1;
            if j < end && b[j].is_ascii_alphabetic() {
                while j < end && b[j].is_ascii_alphabetic() {
                    j += 1;
                }
            } else if j < end {
                j = next_char(t, j);
            }
            Some((i, j, j))
        }
        b'}' => None,
        _ => {
            let j = next_char(t, i);
            Some((i, j, j))
        }
    }
}

/// The name of the control sequence at `i` (`t[i]` is a backslash): (name, index after). A control symbol
/// has a one-character name.
pub(super) fn control_name(t: &str, i: usize, end: usize) -> (&str, usize) {
    let b = t.as_bytes();
    let mut j = i + 1;
    if j >= end {
        return ("", end);
    }
    if b[j].is_ascii_alphabetic() {
        while j < end && b[j].is_ascii_alphabetic() {
            j += 1;
        }
        (&t[i + 1..j], j)
    } else {
        let k = next_char(t, j);
        (&t[j..k], k)
    }
}

/// Find the end of environment `name` that begins just before `from`: (body end, index after `\end{name}`).
pub(super) fn find_env_end(
    t: &str,
    from: usize,
    name: &str,
    end: usize,
    openers: &[String],
    closers: &[String],
) -> Option<(usize, usize)> {
    let b = t.as_bytes();
    let begin = format!("\\begin{{{name}}}");
    let fin = format!("\\end{{{name}}}");
    let mut depth = 1usize;
    let mut j = from;
    while j < end {
        match b[j] {
            b'\\' => {
                if t[j..end].starts_with(&fin) {
                    depth -= 1;
                    if depth == 0 {
                        return Some((j, j + fin.len()));
                    }
                    j += fin.len();
                    continue;
                }
                if t[j..end].starts_with(&begin) {
                    depth += 1;
                    j += begin.len();
                    continue;
                }
                if !openers.is_empty() || !closers.is_empty() {
                    let (word, after) = control_name(t, j, end);
                    if closers.iter().any(|c| c == word) {
                        depth -= 1;
                        if depth == 0 {
                            return Some((j, after));
                        }
                        j = after;
                        continue;
                    }
                    if openers.iter().any(|c| c == word) {
                        depth += 1;
                        j = after;
                        continue;
                    }
                }
                j += 2;
                continue;
            }
            b'%' => {
                j = skip_comment(t, j, end);
                continue;
            }
            _ => {}
        }
        j += 1;
    }
    None
}

/// Find `needle` from `from` with no comment handling (verbatim material).
pub(super) fn find_literal(t: &str, from: usize, needle: &str, end: usize) -> Option<usize> {
    if from >= end {
        return None;
    }
    t[from..end].find(needle).map(|p| p + from)
}

/// The closing `$` (or `$$`) of math that opens just before `from`. Returns the index of the closing delimiter.
pub(super) fn find_dollar_end(t: &str, from: usize, end: usize, double: bool) -> Option<usize> {
    // A `$` inside braces belongs to maths nested in `\text{..}`; try that reading first.
    find_dollar_end_with(t, from, end, double, true)
        .or_else(|| find_dollar_end_with(t, from, end, double, false))
}

fn find_dollar_end_with(
    t: &str,
    from: usize,
    end: usize,
    double: bool,
    braces: bool,
) -> Option<usize> {
    let b = t.as_bytes();
    let mut depth = 0i32;
    let mut j = from;
    while j < end {
        match b[j] {
            b'\\' => j += 2,
            b'%' => j = skip_comment(t, j, end),
            b'{' if braces => {
                depth += 1;
                j += 1;
            }
            b'}' if braces => {
                depth -= 1;
                j += 1;
            }
            b'$' if depth <= 0 => {
                if !double || (j + 1 < end && b[j + 1] == b'$') {
                    return Some(j);
                }
                j += 1;
            }
            _ => j += 1,
        }
    }
    None
}

/// Find the closing `\)` or `\]`.
pub(super) fn find_escaped_end(t: &str, from: usize, end: usize, close: u8) -> Option<usize> {
    let b = t.as_bytes();
    let mut j = from;
    while j + 1 < end {
        match b[j] {
            b'\\' if b[j + 1] == close => return Some(j),
            b'\\' => j += 2,
            b'%' => j = skip_comment(t, j, end),
            _ => j += 1,
        }
    }
    None
}

/// Remove `%` comments from math source.
pub(super) fn strip_comments(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut last = 0;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'%' => {
                out.push_str(&s[last..i]);
                i = skip_comment(s, i, b.len());
                last = i;
            }
            _ => i += 1,
        }
    }
    if last < s.len() {
        out.push_str(&s[last.min(s.len())..]);
    }
    out
}

/// Positions for a conditional that opens at `from` (just after the `\if...` name): the `\else` (if any) and
/// the `\fi`, as (else start, else end, fi start, fi end).
#[allow(clippy::type_complexity)] // (else range, fi start, fi end) tuple is local to the scanner
pub(super) fn find_fi(
    t: &str,
    from: usize,
    end: usize,
) -> Option<(Option<(usize, usize)>, usize, usize)> {
    let b = t.as_bytes();
    let mut depth = 1usize;
    let mut else_pos = None;
    let mut j = from;
    while j < end {
        match b[j] {
            b'\\' => {
                let (name, after) = control_name(t, j, end);
                if name == "fi" {
                    depth -= 1;
                    if depth == 0 {
                        return Some((else_pos, j, after));
                    }
                } else if name == "else" && depth == 1 && else_pos.is_none() {
                    else_pos = Some((j, after));
                } else if name.starts_with("if") && name != "iff" && name != "ifthenelse" {
                    depth += 1;
                }
                j = after.max(j + 1);
                continue;
            }
            b'%' => {
                j = skip_comment(t, j, end);
                continue;
            }
            _ => {}
        }
        j += 1;
    }
    None
}

/// Split `s` at top-level occurrences of `\\` (outside braces and nested environments).
pub(super) fn split_rows(s: &str) -> Vec<&str> {
    let b = s.as_bytes();
    let mut rows = Vec::new();
    let mut depth = 0i32;
    let mut env = 0i32;
    let mut start = 0;
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            b'\\' => {
                if s[i..].starts_with("\\\\") {
                    if depth <= 0 && env <= 0 {
                        rows.push(&s[start..i]);
                        i += 2;
                        // optional * and [length]
                        if i < b.len() && b[i] == b'*' {
                            i += 1;
                        }
                        let k = skip_ws(s, i, b.len());
                        if k < b.len()
                            && b[k] == b'['
                            && let Some((_, _, after)) = read_bracket(s, k, b.len())
                        {
                            i = after;
                        }
                        start = i;
                        continue;
                    }
                    i += 2;
                    continue;
                }
                if s[i..].starts_with("\\begin{") {
                    env += 1;
                } else if s[i..].starts_with("\\end{") {
                    env -= 1;
                }
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }
    rows.push(&s[start..]);
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn braces_balance_and_escape() {
        let t = r"{a{b}c\}d} tail";
        assert_eq!(read_braced(t, 0, t.len()), Some((1, 9, 10)));
    }

    #[test]
    fn unbalanced_takes_the_rest() {
        let t = "{abc";
        assert_eq!(read_braced(t, 0, t.len()), Some((1, 4, 4)));
    }

    #[test]
    fn env_end_skips_nested_and_comments() {
        let t = r"\begin{a} x \begin{a} y \end{a} % \end{a}
 z \end{a} tail";
        let from = 9;
        let (be, after) = find_env_end(t, from, "a", t.len(), &[], &[]).unwrap();
        assert!(t[after..].starts_with(" tail"));
        assert!(t[..be].ends_with("z "));
    }

    #[test]
    fn rows_split_at_top_level_only() {
        let rows = split_rows(r"a &= b \\ c &= \begin{x} 1 \\ 2 \end{x} \\[2pt] d");
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn comments_are_stripped() {
        assert_eq!(strip_comments("a % hidden\nb \\% c"), "a b \\% c");
    }
}
