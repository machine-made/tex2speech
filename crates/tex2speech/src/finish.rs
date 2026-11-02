// SPDX-License-Identifier: Apache-2.0
//! The last stage: blocks become segments. Sentences are split, abbreviations and numbers become
//! words, holes (references, citations) are filled, and markup that slipped through is removed.

use crate::raw::{
    Block, BlockKind, Ctx, HOLE_CLOSE, HOLE_OPEN, Hole, LabelInfo, Loc, MATH_CLOSE, MATH_OPEN,
    RefStyle,
};
use crate::words::{capitalize, expand_abbreviations, normalize_numbers, speak_symbols, tidy};
use crate::{Kind, Segment, Span};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Arc;

const PAUSE_TITLE: u32 = 800;
const PAUSE_HEADING: u32 = 600;
const PAUSE_PARAGRAPH: u32 = 500;
const PAUSE_SENTENCE: u32 = 250;
const PAUSE_EQUATION: u32 = 400;
const PAUSE_CAPTION: u32 = 400;
const PAUSE_NOTE: u32 = 300;

/// Words that join references in a series: "Figures 1 and 2".
const JOINERS: &[&str] = &["and", "or", "to", "through", "see", "also"];

/// The kind a type word names: "fig" and "figures" both give "figure".
fn type_word_kind(w: &str) -> Option<&'static str> {
    Some(match w {
        "figure" | "figures" | "fig" | "figs" => "figure",
        "table" | "tables" | "tab" => "table",
        "section" | "sections" | "sec" | "secs" => "section",
        "subsection" | "subsections" => "subsection",
        "chapter" | "chapters" | "chap" => "chapter",
        "appendix" | "appendices" => "appendix",
        "equation" | "equations" | "eq" | "eqs" => "equation",
        "theorem" | "theorems" | "thm" => "theorem",
        "lemma" | "lemmas" => "lemma",
        "proposition" | "propositions" => "proposition",
        "corollary" | "corollaries" => "corollary",
        "definition" | "definitions" => "definition",
        "remark" | "remarks" => "remark",
        "example" | "examples" => "example",
        "assumption" | "assumptions" => "assumption",
        "claim" | "claims" => "claim",
        "conjecture" | "conjectures" => "conjecture",
        "hypothesis" | "hypotheses" => "hypothesis",
        "property" | "properties" => "property",
        "condition" | "conditions" => "condition",
        "observation" | "observations" => "observation",
        "axiom" | "axioms" => "axiom",
        "problem" | "problems" => "problem",
        "fact" | "facts" => "fact",
        "algorithm" | "algorithms" => "algorithm",
        "part" | "parts" => "part",
        "item" | "items" | "step" | "steps" | "line" | "lines" => "item",
        _ => return None,
    })
}

/// The kind named by the nearest type word before the end of `out`, looking back over numbers and joiners.
fn type_before(out: &str) -> Option<&'static str> {
    let words: Vec<&str> = out
        .split(|c: char| c.is_whitespace() || c == ',' || c == '(')
        .filter(|w| !w.is_empty())
        .collect();
    for w in words.iter().rev().take(8) {
        let w = w
            .trim_matches(|c: char| !c.is_alphanumeric() && c != '.')
            .to_lowercase();
        let w = w.trim_end_matches('.');
        if w.starts_with(|c: char| c.is_ascii_digit()) || JOINERS.contains(&w) {
            continue;
        }
        return type_word_kind(w);
    }
    None
}

/// Characters that must not reach the output.
const FORBIDDEN: &[char] = &['\\', '$', '{', '}', '_', '^'];

struct Finisher {
    out: Vec<Segment>,
    files: Vec<(Arc<str>, Vec<usize>)>,
    cite_numbers: HashMap<String, usize>,
    next_cite: usize,
    /// Whether the text spoken last ended a sentence.
    last_terminal: bool,
}

pub(crate) fn finish(cx: &mut Ctx) -> Vec<Segment> {
    let blocks = std::mem::take(&mut cx.blocks);
    let mut f = Finisher {
        out: Vec::new(),
        files: cx
            .files
            .iter()
            .map(|fi| (fi.name.clone(), fi.line_starts.clone()))
            .collect(),
        cite_numbers: HashMap::new(),
        next_cite: 1,
        last_terminal: true,
    };
    for (i, k) in cx.bib_order.iter().enumerate() {
        f.cite_numbers.entry(k.clone()).or_insert(i + 1);
    }
    f.next_cite = cx.bib_order.len() + 1;
    // After a display equation that ended without a full stop, the next text continues the sentence.
    let mut continues = false;
    for (i, b) in blocks.iter().enumerate() {
        let next = blocks.get(i + 1);
        match b.kind {
            BlockKind::Title => f.heading(cx, b, Kind::Title, PAUSE_TITLE),
            BlockKind::Heading => f.heading(cx, b, Kind::Heading, PAUSE_HEADING),
            BlockKind::Caption => f.heading(cx, b, Kind::Caption, PAUSE_CAPTION),
            BlockKind::Label => f.heading(cx, b, Kind::Label, PAUSE_NOTE),
            BlockKind::Para => {
                let lower = continues && b.cont;
                f.para(cx, b, Kind::Sentence, lower, next);
                continues = false;
            }
            BlockKind::Display => {
                continues = f.display(cx, b, next);
            }
        }
    }
    f.out
}

impl Finisher {
    fn span(&self, loc: Loc) -> Span {
        let (name, starts) = self
            .files
            .get(loc.file as usize)
            .cloned()
            .unwrap_or_else(|| (Arc::from(""), vec![0]));
        let line_of = |off: usize| match starts.binary_search(&off) {
            Ok(i) => i + 1,
            Err(i) => i,
        };
        Span {
            file: name,
            start: loc.start,
            end: loc.end,
            line: line_of(loc.start),
            end_line: line_of(loc.end.max(loc.start + 1) - 1),
        }
    }

    fn push(
        &mut self,
        b: &Block,
        kind: Kind,
        text: String,
        pause: u32,
        loc: Loc,
        new_paragraph: bool,
    ) {
        self.out.push(Segment {
            kind,
            text,
            pause_ms: pause,
            span: self.span(loc),
            section: b.section.clone(),
            new_paragraph,
        });
    }

    fn heading(&mut self, cx: &mut Ctx, b: &Block, kind: Kind, pause: u32) {
        let body = self.speak(cx, &b.raw.text, false);
        let mut text = b.label.clone();
        if !body.is_empty() {
            if !text.is_empty() {
                text.push_str(": ");
            }
            text.push_str(&body);
        }
        if text.is_empty() {
            return;
        }
        if !text.ends_with(['.', '!', '?']) {
            text.push('.');
        }
        let text = scrub(cx, &text, b.loc);
        self.last_terminal = true;
        let loc = b.raw.loc_of(0, b.raw.text.len()).unwrap_or(b.loc);
        self.push(b, kind, text, pause, loc, b.new_para);
    }

    /// Sentences of a paragraph-like block, each followed by its footnotes.
    fn para(
        &mut self,
        cx: &mut Ctx,
        b: &Block,
        kind: Kind,
        lower_first: bool,
        next: Option<&Block>,
    ) {
        let ranges = sentence_ranges(&b.raw.text);
        let n = ranges.len();
        let mut first = true;
        for (k, (a, z)) in ranges.iter().enumerate() {
            let slice = &b.raw.text[*a..*z];
            let mut text = self.speak(cx, slice, !(first && lower_first));
            if text.is_empty() {
                continue;
            }
            if first && !b.label.is_empty() {
                text = format!("{}: {text}", b.label);
            }
            let last = k + 1 == n;
            // The last sentence of a block ends with a full stop unless the text goes on after a displayed equation.
            let open_ended = last && next.is_some_and(|nb| nb.cont);
            if last && !open_ended && !text.ends_with(['.', '!', '?', ':', ';', ',']) {
                text.push('.');
            }
            let text = scrub(cx, &text, b.loc);
            let loc = b.raw.loc_of(*a, *z).unwrap_or(b.loc);
            let ends_para = last && next.is_none_or(|nb| nb.new_para);
            let pause = if ends_para {
                PAUSE_PARAGRAPH
            } else {
                PAUSE_SENTENCE
            };
            self.last_terminal = text.ends_with(['.', '!', '?']);
            self.push(b, kind, text, pause, loc, first && b.new_para);
            first = false;
            // footnotes that follow this sentence
            let lo = *a;
            let hi = if last { usize::MAX } else { ranges[k + 1].0 };
            let notes: Vec<usize> = b
                .raw
                .notes
                .iter()
                .filter(|(o, _)| (*o > lo && *o <= hi) || (k == 0 && *o <= lo))
                .map(|(_, h)| *h)
                .collect();
            for h in notes {
                self.footnote(cx, b, h);
            }
        }
        // footnotes of a block with no sentences are dropped
    }

    fn footnote(&mut self, cx: &mut Ctx, b: &Block, hole: usize) {
        let raw = match cx.holes.get(hole) {
            Some(Hole::Footnote { raw: Some(r), .. }) => Some((**r).clone()),
            Some(Hole::Footnote { label: Some(l), .. }) => cx.footnote_defs.get(l).cloned(),
            _ => None,
        };
        let Some(raw) = raw else { return };
        let mut nb = Block::new(BlockKind::Para, raw, &b.section, b.loc);
        nb.label = "Footnote".to_string();
        nb.new_para = false;
        let ranges = sentence_ranges(&nb.raw.text);
        let mut first = true;
        for (a, z) in ranges {
            let mut text = self.speak(cx, &nb.raw.text[a..z], true);
            if text.is_empty() {
                continue;
            }
            if !text.ends_with(['.', '!', '?']) {
                text.push('.');
            }
            if first {
                text = format!("Footnote: {text}");
            }
            let text = scrub(cx, &text, b.loc);
            let loc = nb.raw.loc_of(a, z).unwrap_or(b.loc);
            self.push(&nb, Kind::Footnote, text, PAUSE_NOTE, loc, false);
            first = false;
        }
    }

    /// Returns whether the sentence continues after the equation.
    fn display(&mut self, cx: &mut Ctx, b: &Block, next: Option<&Block>) -> bool {
        let starts_sentence = self.last_terminal;
        let mut text = self.speak(cx, &b.raw.text, starts_sentence);
        while text.ends_with([',', '.', ';', ' ']) {
            text.pop();
        }
        if text.is_empty() {
            return false;
        }
        let more_follows = next
            .is_some_and(|nb| nb.cont && matches!(nb.kind, BlockKind::Para | BlockKind::Display));
        // text that goes on with a capital letter starts a new sentence
        let capital_next = next.is_some_and(|nb| {
            nb.kind == BlockKind::Para
                && nb
                    .raw
                    .text
                    .chars()
                    .find(|c| !c.is_whitespace())
                    .is_some_and(char::is_uppercase)
        });
        let punct = match b.punct {
            Some(p) => p,
            None if more_follows && !capital_next => ',',
            None => '.',
        };
        text.push(punct);
        let loc = b.raw.loc_of(0, b.raw.text.len()).unwrap_or(b.loc);
        let text = scrub(cx, &text, b.loc);
        self.push(
            b,
            Kind::Equation,
            text,
            PAUSE_EQUATION,
            loc,
            b.new_para && starts_sentence,
        );
        if let Some(num) = &b.number {
            self.push(
                b,
                Kind::EquationNumber,
                format!("{num}{punct}"),
                PAUSE_SENTENCE,
                loc,
                false,
            );
        }
        self.last_terminal = punct == '.';
        punct != '.'
    }

    /// Turn one stretch of raw text into spoken words.
    fn speak(&mut self, cx: &mut Ctx, raw: &str, capital: bool) -> String {
        let s = comma_before_and(raw);
        // Abbreviations apply to prose only, not to the words of an equation.
        let mut prose = String::with_capacity(s.len());
        let mut piece = String::new();
        let mut in_math = false;
        for c in s.chars() {
            if c == MATH_OPEN && !in_math {
                prose.push_str(&expand_abbreviations(&piece, false));
                piece.clear();
                in_math = true;
            } else if c == MATH_CLOSE && in_math {
                prose.push_str(&piece);
                piece.clear();
                in_math = false;
            } else {
                piece.push(c);
            }
        }
        if in_math {
            prose.push_str(&piece);
        } else {
            prose.push_str(&expand_abbreviations(&piece, true));
        }
        let mut t = tidy(&normalize_numbers(&speak_symbols(&prose)));
        if t.contains(HOLE_OPEN) {
            t = tidy(&self.resolve(cx, &t));
        }
        t = crate::math::collapse_terms(&t);
        t = tidy_joins(&t);
        if capital {
            t = capitalize(&t);
        }
        t
    }

    fn resolve(&mut self, cx: &mut Ctx, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(a) = rest.find(HOLE_OPEN) {
            out.push_str(&rest[..a]);
            let after = &rest[a + HOLE_OPEN.len_utf8()..];
            let Some(z) = after.find(HOLE_CLOSE) else {
                rest = "";
                break;
            };
            let idx: usize = after[..z].parse().unwrap_or(usize::MAX);
            rest = &after[z + HOLE_CLOSE.len_utf8()..];
            let hole = cx.holes.get(idx).cloned();
            match hole {
                Some(Hole::Ref { style, labels }) => {
                    let at = out.len();
                    self.fill_ref(cx, &mut out, style, &labels);
                    if style == RefStyle::NamedCap
                        && let Some(c) = out[at..].chars().next()
                    {
                        let up: String = c.to_uppercase().collect();
                        out.replace_range(at..at + c.len_utf8(), &up);
                    }
                }
                Some(Hole::Cite { textual, keys }) => self.fill_cite(&mut out, textual, &keys),
                _ => {}
            }
        }
        out.push_str(rest);
        out
    }

    #[allow(clippy::unused_self)] // keeps the same call shape as its sibling methods
    fn fill_ref(&self, cx: &mut Ctx, out: &mut String, style: RefStyle, labels: &[String]) {
        if style == RefStyle::Page {
            return;
        }
        let infos: Vec<Option<LabelInfo>> =
            labels.iter().map(|l| cx.labels.get(l).cloned()).collect();
        let known: Vec<&LabelInfo> = infos.iter().flatten().collect();
        if known.is_empty() {
            if type_before(out).is_none() {
                out.push_str("a reference");
            }
            return;
        }
        if style == RefStyle::Title {
            let parts: Vec<String> = known
                .iter()
                .map(|i| {
                    if i.title.is_empty() {
                        i.number.clone()
                    } else {
                        i.title.clone()
                    }
                })
                .collect();
            out.push_str(&parts.join(" and "));
            return;
        }
        let typed = type_before(out);
        let kind0 = known[0].kind.as_str();
        let sectioning = |k: &str| {
            matches!(
                k,
                "section" | "subsection" | "subsubsection" | "appendix" | "chapter" | "paragraph"
            )
        };
        let compatible = typed.is_some_and(|k| k == kind0 || (sectioning(k) && sectioning(kind0)));
        let bare = matches!(style, RefStyle::Plain | RefStyle::Equation) && compatible;
        if bare {
            let nums: Vec<&str> = known.iter().map(|i| i.number.as_str()).collect();
            out.push_str(&join_and(&nums));
            return;
        }
        // same kind: "figures 1 and 2"
        let kind = known[0].kind.clone();
        if known.iter().all(|i| i.kind == kind) {
            let nums: Vec<&str> = known.iter().map(|i| i.number.as_str()).collect();
            if known.len() > 1 {
                let _ = write!(out, "{kind}s {}", join_and(&nums));
            } else {
                let _ = write!(out, "{kind} {}", nums[0]);
            }
        } else {
            let parts: Vec<String> = known
                .iter()
                .map(|i| format!("{} {}", i.kind, i.number))
                .collect();
            let refs: Vec<&str> = parts.iter().map(String::as_str).collect();
            out.push_str(&join_and(&refs));
        }
    }

    fn fill_cite(&mut self, out: &mut String, textual: bool, keys: &[String]) {
        let mut nums = Vec::new();
        for k in keys {
            let n = *self.cite_numbers.entry(k.clone()).or_insert_with(|| {
                let n = self.next_cite;
                self.next_cite += 1;
                n
            });
            nums.push(n);
        }
        if nums.is_empty() {
            return;
        }
        let words = cite_words(&nums);
        let trimmed = out.trim_end().len();
        let starts = trimmed == 0 || out[..trimmed].ends_with(['.', '!', '?']);
        let last_word = out[..trimmed]
            .rsplit(|c: char| !c.is_alphabetic())
            .next()
            .unwrap_or("")
            .to_lowercase();
        let glued = matches!(
            last_word.as_str(),
            "and"
                | "or"
                | "in"
                | "by"
                | "of"
                | "see"
                | "from"
                | "with"
                | "to"
                | "at"
                | "as"
                | "for"
                | "on"
                | "also"
                | "cf"
        ) && out[..trimmed].ends_with(|c: char| c.is_alphabetic());
        if textual || glued || out[..trimmed].ends_with(['(', '[', ',', ':', ';']) {
            out.truncate(trimmed);
            if !out.is_empty() && !out.ends_with(['(', '[']) {
                out.push(' ');
            }
            out.push_str(&words);
        } else if starts {
            out.truncate(trimmed);
            if !out.is_empty() {
                out.push(' ');
            }
            let mut cs = words.chars();
            if let Some(f) = cs.next() {
                out.extend(f.to_uppercase());
                out.push_str(cs.as_str());
            }
        } else {
            out.truncate(trimmed);
            out.push_str(", ");
            out.push_str(&words);
        }
    }
}

/// Words that the prose and an equation both say: "a `$7:2$` lock" is "a the ratio of seven to two lock", and
/// "of order `$\mathcal{O}(h)$`" is "of order order h", and "Ref. `\cite{k}`" is "Reference, reference 9", until the
/// second one goes.
fn tidy_joins(t: &str) -> String {
    let mut out = t.to_string();
    for (from, to) in [
        ("of order order ", "of order "),
        ("Reference, reference ", "Reference "),
        ("reference, reference ", "reference "),
        ("References, references ", "References "),
        ("references, references ", "references "),
        ("Of order order ", "Of order "),
        ("a the ratio of", "a ratio of"),
        ("A the ratio of", "A ratio of"),
        ("an the ratio of", "a ratio of"),
        ("An the ratio of", "A ratio of"),
    ] {
        let mut start = 0;
        while let Some(p) = out[start..].find(from) {
            let at = start + p;
            let before_ok = at == 0 || !out.as_bytes()[at - 1].is_ascii_alphanumeric();
            if before_ok {
                out.replace_range(at..at + from.len(), to);
            }
            start = at + to.len();
        }
    }
    out
}

fn cite_words(nums: &[usize]) -> String {
    let mut sorted = nums.to_vec();
    sorted.dedup();
    let strs: Vec<String> = sorted.iter().map(ToString::to_string).collect();
    let refs: Vec<&str> = strs.iter().map(String::as_str).collect();
    if sorted.len() == 1 {
        format!("reference {}", sorted[0])
    } else {
        format!("references {}", join_and(&refs))
    }
}

fn join_and(items: &[&str]) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].to_string(),
        n => format!("{} and {}", items[..n - 1].join(", "), items[n - 1]),
    }
}

/// Insert a comma after a long inline equation that is followed by "and" or "or".
fn comma_before_and(s: &str) -> String {
    if !s.contains(MATH_CLOSE) {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 8);
    let mut math_start = 0;
    let chars = s.char_indices().peekable();
    for (i, c) in chars {
        out.push(c);
        if c == MATH_OPEN {
            math_start = i;
        } else if c == MATH_CLOSE {
            let words = s[math_start..i].split_whitespace().count();
            let tail = &s[i + c.len_utf8()..];
            let t = tail.trim_start();
            let conj = ["and ", "or "].iter().find(|w| t.starts_with(**w));
            if let Some(_w) = conj
                && words >= 3
                && !s[..i].trim_end_matches(MATH_CLOSE).ends_with(',')
            {
                out.push(',');
            }
        }
    }
    out
}

/// Abbreviations after which a period does not end a sentence.
const NO_SPLIT: &[&str] = &[
    "al", "e.g", "i.e", "cf", "vs", "etc", "fig", "figs", "eq", "eqs", "sec", "secs", "ref",
    "refs", "tab", "thm", "lem", "prop", "cor", "chap", "app", "approx", "resp", "ca", "dr", "mr",
    "mrs", "ms", "prof", "vol", "pp", "no", "p", "ed", "eds", "inc", "jr", "sr", "st", "ch", "def",
    "ex", "incl",
];

fn split_here(before: &str, next: char) -> bool {
    // a reference or a citation after a full stop starts a sentence, unless the stop ends an abbreviation
    if next.is_lowercase() {
        return false;
    }
    let token: String = before
        .chars()
        .rev()
        .take_while(|c| c.is_alphabetic() || *c == '.')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let lower = token.to_lowercase();
    if matches!(lower.as_str(), "al" | "etc") {
        return next.is_uppercase() || next == MATH_OPEN;
    }
    if NO_SPLIT.contains(&lower.as_str()) {
        return false;
    }
    let mut cs = token.chars();
    if let (Some(c), None) = (cs.next(), cs.next())
        && c.is_uppercase()
    {
        return false;
    }
    true
}

/// Byte ranges of the sentences of raw text, trimmed. It walks the text by byte position: a list of every
/// character with its position would take 16 bytes per character, a quarter of a gigabyte for 16 MiB of text.
pub(crate) fn sentence_ranges(t: &str) -> Vec<(usize, usize)> {
    let at = |p: usize| t[p..].chars().next();
    let mut ranges = Vec::new();
    let mut start = 0usize;
    let mut math = 0usize;
    let mut hole = false;
    let mut pos = 0usize;
    let mut push = |a: usize, z: usize| {
        let s = &t[a..z];
        let trimmed_start = a + (s.len() - s.trim_start().len());
        let trimmed_end = z - (s.len() - s.trim_end().len());
        if trimmed_start < trimmed_end {
            ranges.push((trimmed_start, trimmed_end));
        }
    };
    while let Some(c) = at(pos) {
        match c {
            MATH_OPEN => math += 1,
            MATH_CLOSE => math = math.saturating_sub(1),
            HOLE_OPEN => hole = true,
            HOLE_CLOSE => hole = false,
            _ => {}
        }
        if math == 0 && !hole && matches!(c, '.' | '!' | '?') {
            let mut j = pos + c.len_utf8();
            while let Some(q) =
                at(j).filter(|q| matches!(q, '"' | '\'' | ')' | ']' | '\u{201d}' | '\u{2019}'))
            {
                j += q.len_utf8();
            }
            let end_pos = j;
            if j >= t.len() {
                push(start, end_pos);
                start = end_pos;
            } else if at(j).is_some_and(char::is_whitespace) {
                let mut k = j;
                while let Some(w) = at(k).filter(|w| w.is_whitespace()) {
                    k += w.len_utf8();
                }
                if let Some(n) = at(k) {
                    let split = if c == '.' {
                        split_here(&t[..pos], n)
                    } else {
                        !n.is_lowercase()
                    };
                    if split {
                        push(start, end_pos);
                        start = k;
                    }
                }
            }
            pos = j;
            continue;
        }
        pos += c.len_utf8();
    }
    push(start, t.len());
    ranges
}

/// Remove characters that would leak markup, and report each stretch removed.
fn scrub(cx: &mut Ctx, s: &str, loc: Loc) -> String {
    if !s
        .chars()
        .any(|c| FORBIDDEN.contains(&c) || ('\u{e000}'..='\u{f8ff}').contains(&c))
    {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if FORBIDDEN.contains(&c) {
            let ctx: String = s.chars().take(80).collect();
            cx.warn(
                "leak",
                format!("removed {c:?} from the output near {ctx:?}"),
                loc,
            );
            match c {
                '_' | '^' | '\\' => out.push(' '),
                '$' => out.push_str(" dollars "),
                _ => {}
            }
        } else if ('\u{e000}'..='\u{f8ff}').contains(&c) {
            // private-use marker that was never resolved
        } else {
            out.push(c);
        }
    }
    tidy(&out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_sentences_but_not_abbreviations() {
        let r = sentence_ranges("Smith et al. showed it. See Fig. 3. Then C. Jones left? Yes.");
        let parts: Vec<&str> = r
            .iter()
            .map(|(a, b)| &"Smith et al. showed it. See Fig. 3. Then C. Jones left? Yes."[*a..*b])
            .collect();
        assert_eq!(
            parts,
            [
                "Smith et al. showed it.",
                "See Fig. 3.",
                "Then C. Jones left?",
                "Yes."
            ]
        );
    }

    #[test]
    fn comma_between_long_equations() {
        let s = format!("{MATH_OPEN}a plus b plus c{MATH_CLOSE} and {MATH_OPEN}x{MATH_CLOSE}");
        assert!(comma_before_and(&s).contains(&format!("{MATH_CLOSE},")));
        let t = format!("{MATH_OPEN}x{MATH_CLOSE} and {MATH_OPEN}y{MATH_CLOSE}");
        assert!(!comma_before_and(&t).contains(','));
    }

    #[test]
    fn cite_words_group() {
        assert_eq!(cite_words(&[1]), "reference 1");
        assert_eq!(cite_words(&[1, 2, 3]), "references 1, 2 and 3");
    }
}
