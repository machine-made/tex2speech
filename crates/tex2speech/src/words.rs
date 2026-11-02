// SPDX-License-Identifier: Apache-2.0
//! Numbers to English words, abbreviations, and the clean-up of spoken text.

use crate::raw::{HOLE_CLOSE, HOLE_OPEN};

const ONES: [&str; 20] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];
const TENS: [&str; 10] = [
    "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];
const SCALES: [&str; 5] = ["", "thousand", "million", "billion", "trillion"];

fn below_thousand(n: u64, out: &mut Vec<String>) {
    let h = n / 100;
    let r = n % 100;
    if h > 0 {
        out.push(ONES[h as usize].to_string());
        out.push("hundred".to_string());
    }
    if r >= 20 {
        let t = TENS[(r / 10) as usize];
        if r.is_multiple_of(10) {
            out.push(t.to_string());
        } else {
            out.push(format!("{t}-{}", ONES[(r % 10) as usize]));
        }
    } else if r > 0 {
        out.push(ONES[r as usize].to_string());
    }
}

/// Words for a whole number below 10^15.
pub(crate) fn number_words(n: u64) -> String {
    if n == 0 {
        return "zero".to_string();
    }
    let mut groups = Vec::new();
    let mut m = n;
    while m > 0 {
        groups.push(m % 1000);
        m /= 1000;
    }
    let mut out = Vec::new();
    for (i, g) in groups.iter().enumerate().rev() {
        if *g == 0 {
            continue;
        }
        below_thousand(*g, &mut out);
        if i > 0 {
            out.push(SCALES[i].to_string());
        }
    }
    out.join(" ")
}

fn digit_by_digit(s: &str) -> String {
    s.chars()
        .filter_map(|c| c.to_digit(10))
        .map(|d| ONES[d as usize])
        .collect::<Vec<_>>()
        .join(" ")
}

fn integer_words(digits: &str) -> String {
    let leading_zero = digits.len() > 1 && digits.starts_with('0');
    if leading_zero || digits.len() > 15 {
        return digit_by_digit(digits);
    }
    digits
        .parse::<u64>()
        .map_or_else(|_| digit_by_digit(digits), number_words)
}

fn ordinal_words(digits: &str) -> Option<String> {
    if digits.len() > 15 {
        return None;
    }
    let n: u64 = digits.parse().ok()?;
    if digits.len() > 1 && digits.starts_with('0') {
        return None;
    }
    let cardinal = number_words(n);
    // Replace the last word with its ordinal form.
    let (head, last) = match cardinal.rfind([' ', '-']) {
        Some(i) => (&cardinal[..=i], &cardinal[i + 1..]),
        None => ("", cardinal.as_str()),
    };
    let ord = match last {
        "one" => "first",
        "two" => "second",
        "three" => "third",
        "five" => "fifth",
        "eight" => "eighth",
        "nine" => "ninth",
        "twelve" => "twelfth",
        w if w.ends_with('y') => return Some(format!("{head}{}ieth", &w[..w.len() - 1])),
        w => return Some(format!("{head}{w}th")),
    };
    Some(format!("{head}{ord}"))
}

const MONTHS: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

fn year_words(n: u64) -> String {
    let (hi, lo) = (n / 100, n % 100);
    let hi_words = number_words(hi);
    match lo {
        0 => format!("{hi_words} hundred"),
        1..=9 => format!(
            "{hi_words} oh {}",
            ONES[usize::try_from(lo).unwrap_or(usize::MAX)]
        ),
        _ => format!("{hi_words} {}", number_words(lo)),
    }
}

fn prev_word(s: &str) -> String {
    let t = s.trim_end();
    t.rsplit(|c: char| !c.is_alphabetic())
        .next()
        .unwrap_or("")
        .to_lowercase()
}

fn is_group_sep(c: char) -> bool {
    matches!(c, '\u{2009}' | '\u{202f}' | '\u{a0}')
}

/// Replace every number in `text` by words. Text between hole markers is left alone.
pub(crate) fn normalize_numbers(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 16);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == HOLE_OPEN {
            while i < chars.len() {
                out.push(chars[i]);
                i += 1;
                if chars[i - 1] == HOLE_CLOSE {
                    break;
                }
            }
            continue;
        }
        if c == '%' {
            out.push_str(" percent ");
            i += 1;
            continue;
        }
        if !c.is_ascii_digit() {
            out.push(c);
            i += 1;
            continue;
        }
        // Digit run, with thousands separators when the groups have exactly three digits.
        let mut int = String::new();
        while i < chars.len() && chars[i].is_ascii_digit() {
            int.push(chars[i]);
            i += 1;
        }
        let first_len = int.len();
        while first_len <= 3
            && !int.starts_with('0')
            && i + 3 < chars.len() + 1
            && i < chars.len()
            && (chars[i] == ',' || is_group_sep(chars[i]))
            && chars
                .get(i + 1..i + 4)
                .is_some_and(|g| g.iter().all(char::is_ascii_digit))
            && !chars.get(i + 4).is_some_and(char::is_ascii_digit)
        {
            int.extend(&chars[i + 1..i + 4]);
            i += 4;
        }
        let mut frac = String::new();
        if i + 1 < chars.len() && chars[i] == '.' && chars[i + 1].is_ascii_digit() {
            i += 1;
            while i < chars.len() && chars[i].is_ascii_digit() {
                frac.push(chars[i]);
                i += 1;
            }
        }
        let before_alpha = out.chars().last().is_some_and(char::is_alphabetic);
        let mut words;
        // Ordinal suffix.
        let suffix: String = chars[i..].iter().take(2).collect();
        let after_suffix = chars.get(i + 2);
        if frac.is_empty()
            && matches!(suffix.as_str(), "st" | "nd" | "rd" | "th")
            && !after_suffix.is_some_and(|c| c.is_alphanumeric())
            && ordinal_words(&int).is_some()
        {
            words = ordinal_words(&int).unwrap_or_default();
            i += 2;
        } else if frac.is_empty()
            && int.len() == 4
            && (1800..=2099).contains(&int.parse::<u64>().unwrap_or(0))
            && {
                let w = prev_word(&out);
                w == "in" || w == "since" || w == "year" || MONTHS.contains(&w.as_str())
            }
        {
            words = year_words(int.parse().unwrap_or(0));
        } else {
            words = integer_words(&int);
            if !frac.is_empty() {
                words.push_str(" point ");
                words.push_str(&digit_by_digit(&frac));
            }
        }
        // A minus sign before the number.
        if out.ends_with('-') {
            let pre: Vec<char> = out.chars().collect();
            let before = if pre.len() >= 2 {
                Some(pre[pre.len() - 2])
            } else {
                None
            };
            if before.is_none_or(|c| c.is_whitespace() || c == '(' || c == '[') {
                out.pop();
                out.push_str("minus ");
            }
        }
        if before_alpha {
            out.push(' ');
        }
        out.push_str(&words);
        if chars.get(i).is_some_and(|c| c.is_alphabetic()) {
            out.push(' ');
        }
    }
    out
}

/// Abbreviations: (written, spoken). Matched at a word start; a case difference in the first letter is kept.
const ABBREVIATIONS: &[(&str, &str)] = &[
    ("et al.", "and others"),
    ("e.g.", "for example"),
    ("i.e.", "that is"),
    ("cf.", "compare"),
    ("vs.", "versus"),
    ("etc.", "et cetera"),
    ("fig.", "figure"),
    ("figs.", "figures"),
    ("eq.", "equation"),
    ("eqs.", "equations"),
    ("sec.", "section"),
    ("secs.", "sections"),
    ("ref.", "reference"),
    ("refs.", "references"),
    ("tab.", "table"),
    ("thm.", "theorem"),
    ("lem.", "lemma"),
    ("prop.", "proposition"),
    ("cor.", "corollary"),
    ("chap.", "chapter"),
    ("app.", "appendix"),
    ("approx.", "approximately"),
    ("resp.", "respectively"),
    ("ca.", "approximately"),
    ("dr.", "doctor"),
    ("mr.", "mister"),
    ("mrs.", "missus"),
    ("prof.", "professor"),
    ("vol.", "volume"),
    ("pp.", "pages"),
];

/// Expand abbreviations. A period that ends the sentence after an abbreviation is kept; `ends_text` says that
/// nothing follows `s` in the sentence (an equation that follows means the sentence goes on).
pub(crate) fn expand_abbreviations(s: &str, ends_text: bool) -> String {
    let mut out = String::with_capacity(s.len());
    let lower = s.to_lowercase();
    let same_len = lower.len() == s.len();
    let mut i = 0;
    while i < s.len() {
        let at_word_start = out
            .chars()
            .last()
            .is_none_or(|c| !c.is_alphanumeric() && c != '.');
        if at_word_start && same_len && s.is_char_boundary(i) {
            let mut hit = None;
            for (abbr, spoken) in ABBREVIATIONS {
                if lower[i..].starts_with(abbr) {
                    hit = Some((abbr, spoken));
                    break;
                }
            }
            if let Some((abbr, spoken)) = hit {
                let rest = &s[i + abbr.len()..];
                let first_upper = s[i..].chars().next().is_some_and(char::is_uppercase);
                let mut w = (*spoken).to_string();
                if first_upper {
                    let mut cs = w.chars();
                    w = cs
                        .next()
                        .map(|f| f.to_uppercase().collect::<String>() + cs.as_str())
                        .unwrap_or_default();
                }
                out.push_str(&w);
                if ends_text && rest.trim().is_empty() {
                    out.push('.');
                }
                i += abbr.len();
                continue;
            }
        }
        let Some(ch) = s[i..].chars().next() else {
            break;
        };
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Words for symbols that a voice does not say by itself. A symbol with no entry is removed.
fn symbol_words(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{2212}' => "minus",
        '\u{2013}' | '\u{2014}' | '\u{2011}' | '\u{2010}' => "-",
        '@' => "at",
        '|' | '*' | '#' | '~' | '`' | '\u{fe00}' | '\u{fe01}' | '\u{fe0e}' | '\u{fe0f}'
        | '\u{338}' | '\u{2020}' | '\u{2021}' | '\u{2022}' | '\u{2023}' | '\u{b6}' | '\u{200b}'
        | '\u{200c}' | '\u{200d}' | '\u{feff}' | '\u{ad}' | '\u{201c}' | '\u{201d}' => "",
        '\u{a7}' => "section",
        '\u{b0}' => "degrees",
        '\u{b1}' => "plus or minus",
        '\u{d7}' => "times",
        '\u{f7}' => "divided by",
        '\u{b7}' | '\u{22c5}' => "dot",
        '\u{2192}' | '\u{27f6}' => "to",
        '\u{2190}' => "from",
        '\u{2194}' => "left right arrow",
        '\u{21d2}' | '\u{27f9}' => "implies",
        '\u{21d4}' | '\u{27fa}' => "if and only if",
        '\u{2248}' => "approximately equal to",
        '\u{2260}' => "not equal to",
        '\u{2264}' => "less than or equal to",
        '\u{2265}' => "greater than or equal to",
        '\u{226a}' => "much less than",
        '\u{226b}' => "much greater than",
        '\u{2273}' => "greater than or equivalent to",
        '\u{2272}' => "less than or equivalent to",
        '\u{221e}' => "infinity",
        '\u{2205}' => "empty set",
        '\u{2208}' => "in",
        '\u{2209}' => "not in",
        '\u{2032}' => "prime",
        '\u{221a}' => "square root of",
        '\u{222b}' => "integral",
        '\u{2211}' => "sum",
        '\u{220f}' => "product",
        '\u{2202}' => "partial",
        '\u{2207}' => "grad",
        '\u{27e8}' => "left angle bracket",
        '\u{27e9}' => "right angle bracket",
        '\u{2018}' | '\u{2019}' => "'",
        '\u{2026}' => "...",
        '\u{2223}' => "divides",
        '\u{221d}' => "proportional to",
        '\u{2282}' => "subset of",
        '\u{2286}' => "subset of or equal to",
        '\u{222a}' => "union",
        '\u{2229}' => "intersection",
        '\u{2200}' => "for all",
        '\u{2203}' => "there exists",
        '\u{2227}' => "and",
        '\u{2228}' => "or",
        '\u{ac}' => "not",
        _ => return None,
    })
}

/// Replace symbols by words, and drop invisible marks. Letters, digits and ordinary punctuation stay.
pub(crate) fn speak_symbols(s: &str) -> String {
    let plain = |c: char| {
        c.is_alphanumeric()
            || c.is_whitespace()
            || matches!(
                c,
                '.' | ','
                    | ';'
                    | ':'
                    | '!'
                    | '?'
                    | '\''
                    | '"'
                    | '('
                    | ')'
                    | '['
                    | ']'
                    | '-'
                    | '/'
                    | '&'
                    | '+'
                    | '='
                    | '<'
                    | '>'
            )
            || ('\u{e000}'..='\u{f8ff}').contains(&c)
    };
    if s.chars().all(plain) {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 16);
    for c in s.chars() {
        if plain(c) {
            out.push(c);
        } else if let Some(w) = symbol_words(c) {
            if !w.is_empty() {
                out.push(' ');
                out.push_str(w);
                out.push(' ');
            }
        } else if c == '\u{a0}' || c == '\u{2009}' || c == '\u{202f}' {
            out.push(' ');
        }
        // anything else is dropped
    }
    out
}

/// Drop a second "the" right after a "the": "The the ratio" comes from "The $7:2$ lock".
fn collapse_the_the(s: &str) -> String {
    let lower = s.to_lowercase();
    if !lower.contains("the the ") || lower.len() != s.len() {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        let at_start = out.chars().last().is_none_or(|c| !c.is_alphanumeric());
        if at_start && lower[i..].starts_with("the the ") {
            out.push_str(&s[i..i + 4]);
            i += 8;
            continue;
        }
        let Some(ch) = s[i..].chars().next() else {
            break;
        };
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Collapse white space, remove the space before closing punctuation, and fix doubled punctuation.
pub(crate) fn tidy(s: &str) -> String {
    let collapsed = collapse_the_the(s);
    let s = collapsed.as_str();
    let mut out = String::with_capacity(s.len());
    let mut pending_space = false;
    for c in s.chars() {
        if c.is_whitespace() || c == '\u{a0}' || c == '\u{2009}' || c == '\u{202f}' {
            pending_space = true;
            continue;
        }
        if matches!(c, ',' | '.' | ';' | ':' | '!' | '?' | ')' | ']') {
            pending_space = false;
            // ",," or ",." collapses to the later one when the earlier is a comma
            if c == ',' && out.ends_with(',') {
                continue;
            }
            if matches!(c, '.' | '!' | '?') && out.ends_with(',') {
                out.pop();
            }
            out.push(c);
            continue;
        }
        if pending_space && !out.is_empty() && !out.ends_with(['(', '[']) {
            out.push(' ');
        }
        pending_space = false;
        out.push(c);
    }
    out
}

/// True for a Roman numeral in brackets that opens a list item: "(ii) ..." stays "(ii) ...".
fn is_list_numeral(s: &str) -> bool {
    let Some(rest) = s.strip_prefix('(') else {
        return false;
    };
    let Some((word, _)) = rest.split_once(')') else {
        return false;
    };
    !word.is_empty() && word.len() <= 5 && word.chars().all(|c| matches!(c, 'i' | 'v' | 'x'))
}

/// Capitalise the first word when it has two or more letters.
pub(crate) fn capitalize(s: &str) -> String {
    if is_list_numeral(s) {
        return s.to_string();
    }
    let mut word_len = 0;
    let mut first = None;
    for (i, c) in s.char_indices() {
        if c.is_alphanumeric() {
            if first.is_none() {
                first = Some(i);
            }
            word_len += 1;
        } else if first.is_some() {
            break;
        } else if c == '(' || c == '"' || c == '\'' || c.is_whitespace() {
            // leading punctuation
        } else {
            return s.to_string();
        }
    }
    match first {
        Some(i) if word_len >= 2 => {
            let c = s[i..].chars().next().unwrap_or(' ');
            if c.is_lowercase() {
                let mut r = String::with_capacity(s.len());
                r.push_str(&s[..i]);
                r.extend(c.to_uppercase());
                r.push_str(&s[i + c.len_utf8()..]);
                r
            } else {
                s.to_string()
            }
        }
        _ => s.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cardinal_numbers() {
        assert_eq!(number_words(0), "zero");
        assert_eq!(number_words(43000), "forty-three thousand");
        assert_eq!(number_words(1200), "one thousand two hundred");
        assert_eq!(number_words(1_000_001), "one million one");
        assert_eq!(number_words(100), "one hundred");
    }

    #[test]
    fn numbers_in_text() {
        assert_eq!(
            normalize_numbers("has 1200 elements"),
            "has one thousand two hundred elements"
        );
        assert_eq!(
            normalize_numbers("1.3 times 10"),
            "one point three times ten"
        );
        assert_eq!(
            normalize_numbers("1,200 and 12,34"),
            "one thousand two hundred and twelve,thirty-four"
        );
        assert_eq!(
            normalize_numbers("the 3rd and 21st"),
            "the third and twenty-first"
        );
        assert_eq!(normalize_numbers("in 1996"), "in nineteen ninety-six");
        assert_eq!(normalize_numbers("50%"), "fifty percent ");
        assert_eq!(normalize_numbers("H2O"), "H two O");
        assert_eq!(normalize_numbers("x is -3"), "x is minus three");
        assert_eq!(normalize_numbers("007"), "zero zero seven");
    }

    #[test]
    fn abbreviations() {
        assert_eq!(
            expand_abbreviations("Smith et al. showed", true),
            "Smith and others showed"
        );
        assert_eq!(
            expand_abbreviations("see Fig. 3, e.g. here", true),
            "see Figure 3, for example here"
        );
        assert_eq!(
            expand_abbreviations("and so on, etc.", true),
            "and so on, et cetera."
        );
    }

    #[test]
    fn capitals() {
        assert_eq!(capitalize("integral from"), "Integral from");
        assert_eq!(capitalize("f of x"), "f of x");
        assert_eq!(capitalize("(sum of"), "(Sum of");
    }
}
