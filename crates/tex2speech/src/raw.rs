// SPDX-License-Identifier: Apache-2.0
//! The structures both readers fill and the final stage reads.
//!
//! A reader produces [`Block`]s. A block holds [`Raw`] text: spoken words with markers for inline maths
//! and holes for things only known at the end of the document (cross-references, citations, footnotes).

use std::collections::HashMap;
use std::sync::Arc;

/// Opens a hole in raw text. The digits between the markers are an index into [`Ctx::holes`].
pub(crate) const HOLE_OPEN: char = '\u{e000}';
/// Closes a hole.
pub(crate) const HOLE_CLOSE: char = '\u{e001}';
/// Opens inline maths. The spoken words of an equation sit between the markers.
pub(crate) const MATH_OPEN: char = '\u{e010}';
/// Closes inline maths.
pub(crate) const MATH_CLOSE: char = '\u{e011}';

/// A place in a source file: file index and byte range.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Loc {
    pub file: u32,
    pub start: usize,
    pub end: usize,
}

/// Where a stretch of raw text came from.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Mark {
    pub off: usize,
    pub end: usize,
    pub loc: Loc,
}

/// Raw text with source marks and the footnotes it carries.
#[derive(Clone, Debug, Default)]
pub(crate) struct Raw {
    pub text: String,
    pub marks: Vec<Mark>,
    /// (byte offset in `text`, hole index of a footnote). The footnote follows the sentence holding the offset.
    pub notes: Vec<(usize, usize)>,
}

impl Raw {
    pub(crate) fn is_blank(&self) -> bool {
        self.text.chars().all(char::is_whitespace)
    }

    pub(crate) fn push(&mut self, s: &str, loc: Loc) {
        if s.is_empty() {
            return;
        }
        let off = self.text.len();
        self.text.push_str(s);
        self.marks.push(Mark {
            off,
            end: self.text.len(),
            loc,
        });
    }

    /// Push a space that stands between two paragraphs. It sits at the end of the text before it, with no width,
    /// so that a range holding it does not reach back to the top of the file.
    pub(crate) fn push_separator(&mut self, file: u32) {
        let at = self.marks.last().map_or(0, |m| m.loc.end);
        self.push(
            " ",
            Loc {
                file: self.marks.last().map_or(file, |m| m.loc.file),
                start: at,
                end: at,
            },
        );
    }

    /// Push one space unless the text is empty or already ends with white space.
    pub(crate) fn push_space(&mut self, loc: Loc) {
        if self.text.is_empty() || self.text.ends_with(char::is_whitespace) {
            return;
        }
        self.push(" ", loc);
    }

    pub(crate) fn push_raw(&mut self, other: &Raw) {
        let base = self.text.len();
        self.text.push_str(&other.text);
        for m in &other.marks {
            self.marks.push(Mark {
                off: m.off + base,
                end: m.end + base,
                loc: m.loc,
            });
        }
        for (o, h) in &other.notes {
            self.notes.push((o + base, *h));
        }
    }

    /// Source location covering the raw range `[a, b)`.
    pub(crate) fn loc_of(&self, a: usize, b: usize) -> Option<Loc> {
        let mut first: Option<&Mark> = None;
        let mut last: Option<&Mark> = None;
        for m in &self.marks {
            if m.end <= a || m.off >= b {
                continue;
            }
            if first.is_none() {
                first = Some(m);
            }
            if first.is_some_and(|f| f.loc.file == m.loc.file) {
                last = Some(m);
            }
        }
        let (f, l) = (first?, last?);
        Some(Loc {
            file: f.loc.file,
            start: f.loc.start.min(l.loc.start),
            end: f.loc.end.max(l.loc.end),
        })
    }

    /// Replace every equation slot by the text `fill` gives for it, keeping marks and notes in place.
    pub(crate) fn replace_text(&mut self, fill: &dyn Fn(&str) -> String) {
        use crate::math::{SLOT_CLOSE, SLOT_OPEN};
        if !self.text.contains(SLOT_OPEN) {
            return;
        }
        // (old start, old end, new start, new end) for each slot
        let mut pts: Vec<(usize, usize, usize, usize)> = Vec::new();
        let mut out = String::with_capacity(self.text.len());
        let mut last = 0;
        let t = std::mem::take(&mut self.text);
        for (a, _) in t.match_indices(SLOT_OPEN) {
            if a < last {
                continue;
            }
            let Some(z) = t[a..]
                .find(SLOT_CLOSE)
                .map(|z| a + z + SLOT_CLOSE.len_utf8())
            else {
                break;
            };
            out.push_str(&t[last..a]);
            let ns = out.len();
            let mut words = fill(&t[a..z]);
            // `$\Lambda$CDM` and `deg$^2$` touch the words around them in the source, not in speech
            let before = out.chars().rev().find(|&c| c != MATH_OPEN);
            let after = t[z..].chars().find(|&c| c != MATH_CLOSE);
            let run: String = t[z..]
                .chars()
                .filter(|&c| c != MATH_CLOSE)
                .take_while(|c| c.is_alphanumeric())
                .collect();
            let alnum = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
            // `$(N-2)$th-order`: the parentheses only tie the suffix to the equation, so the suffix is a word
            let ordinal = matches!(run.as_str(), "th" | "st" | "nd" | "rd");
            let mut spaced = false;
            if ordinal
                && let Some(inner) = words
                    .strip_prefix("open paren ")
                    .and_then(|w| w.strip_suffix(" close paren"))
                && !inner.contains("paren")
            {
                words = inner.to_string();
                spaced = true;
            }
            if alnum(before) && alnum(words.chars().next()) {
                out.push(' ');
            }
            out.push_str(&words);
            // a suffix of the equation's last symbol stays attached: `$n$th`, `$x$s`
            if alnum(words.chars().next_back())
                && alnum(after)
                && (spaced || !matches!(run.as_str(), "th" | "st" | "nd" | "rd" | "s"))
            {
                out.push(' ');
            }
            pts.push((a, z, ns, out.len()));
            last = z;
        }
        out.push_str(&t[last..]);
        let map = |pos: usize, is_end: bool| -> usize {
            let k = pts.partition_point(|p| p.1 <= pos);
            let delta = pts[..k]
                .last()
                .map_or(0isize, |p| p.3.cast_signed() - p.1.cast_signed());
            if let Some(p) = pts.get(k)
                && p.0 < pos
            {
                return if is_end { p.3 } else { p.2 };
            }
            (pos.cast_signed() + delta).cast_unsigned()
        };
        for m in &mut self.marks {
            m.off = map(m.off, false);
            m.end = map(m.end, true);
        }
        for n in &mut self.notes {
            n.0 = map(n.0, false);
        }
        self.text = out;
    }
}

/// How a reference is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RefStyle {
    /// `\ref`: the number only, with the type word added when the text before has none.
    Plain,
    /// `\eqref`
    Equation,
    /// `\autoref`, `\cref`: the type word and the number.
    Named,
    /// `\Cref`, `\autoref`: as `Named`, with a capital letter.
    NamedCap,
    /// `\nameref`: the title of the target.
    Title,
    /// `\pageref`: nothing.
    Page,
}

/// A thing whose text is only known when the whole document has been read.
#[derive(Clone, Debug)]
pub(crate) enum Hole {
    Ref {
        style: RefStyle,
        labels: Vec<String>,
    },
    /// `textual` is `\citet`: no comma before the words.
    Cite { textual: bool, keys: Vec<String> },
    /// A footnote with its text, or the label of a Markdown footnote defined elsewhere.
    Footnote {
        raw: Option<Box<Raw>>,
        label: Option<String>,
    },
}

/// A numbered thing a label points to.
#[derive(Clone, Debug)]
pub(crate) struct LabelInfo {
    /// "section", "equation", "figure", "table", "theorem", and so on.
    pub kind: String,
    pub number: String,
    pub title: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BlockKind {
    Title,
    Heading,
    Para,
    Display,
    Caption,
    Label,
}

#[derive(Clone, Debug)]
pub(crate) struct Block {
    pub kind: BlockKind,
    /// "Section 1", "Figure 2", "Quote": spoken before the text and not touched by the number stage.
    pub label: String,
    pub raw: Raw,
    pub new_para: bool,
    /// The block continues the sentence of the block before it (text after a displayed equation).
    pub cont: bool,
    pub section: Vec<u32>,
    /// Display maths: the number words, for example "equation 1".
    pub number: Option<String>,
    /// Display maths: punctuation written in the source after the equation.
    pub punct: Option<char>,
    /// Location for blocks with no marked text.
    pub loc: Loc,
}

impl Block {
    pub(crate) fn new(kind: BlockKind, raw: Raw, section: &[u32], loc: Loc) -> Self {
        Self {
            kind,
            label: String::new(),
            raw,
            new_para: true,
            cont: false,
            section: section.to_vec(),
            number: None,
            punct: None,
            loc,
        }
    }
}

/// A file read during a run.
#[derive(Clone, Debug)]
pub(crate) struct FileInfo {
    pub name: Arc<str>,
    pub line_starts: Vec<usize>,
}

impl FileInfo {
    pub(crate) fn new(name: &str, text: &str) -> Self {
        let mut line_starts = vec![0];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        Self {
            name: Arc::from(name),
            line_starts,
        }
    }

    /// One-based line of a byte offset.
    pub(crate) fn line(&self, off: usize) -> usize {
        match self.line_starts.binary_search(&off) {
            Ok(i) => i + 1,
            Err(i) => i,
        }
    }
}

/// A warning with its construct name, for example "command" or "environment".
#[derive(Clone, Debug)]
pub(crate) struct RawWarning {
    pub construct: String,
    pub message: String,
    pub loc: Loc,
}

/// Everything a reader collects.
#[derive(Default)]
pub(crate) struct Ctx {
    pub files: Vec<FileInfo>,
    pub blocks: Vec<Block>,
    pub holes: Vec<Hole>,
    pub labels: HashMap<String, LabelInfo>,
    pub warnings: Vec<RawWarning>,
    pub warning_total: usize,
    pub footnote_defs: HashMap<String, Raw>,
    /// Citation keys in bibliography order, when the document lists them.
    pub bib_order: Vec<String>,
    /// Equations to read when the document is done.
    pub math: crate::math::MathBatch,
}

/// Warnings kept per run; the count goes on.
pub(crate) const MAX_WARNINGS: usize = 5000;

impl Ctx {
    pub(crate) fn warn(&mut self, construct: &str, message: impl Into<String>, loc: Loc) {
        self.warning_total += 1;
        if self.warnings.len() < MAX_WARNINGS {
            self.warnings.push(RawWarning {
                construct: construct.to_string(),
                message: message.into(),
                loc,
            });
        }
    }

    pub(crate) fn hole(&mut self, h: Hole) -> usize {
        self.holes.push(h);
        self.holes.len() - 1
    }

    pub(crate) fn hole_text(idx: usize) -> String {
        format!("{HOLE_OPEN}{idx}{HOLE_CLOSE}")
    }
}

#[cfg(test)]
mod tests {
    use super::{Loc, Raw};

    #[test]
    fn a_separator_does_not_move_the_start_of_a_range_to_the_file_top() {
        let mut r = Raw::default();
        r.push(
            "First",
            Loc {
                file: 1,
                start: 400,
                end: 405,
            },
        );
        r.push_separator(1);
        r.push(
            "Second",
            Loc {
                file: 1,
                start: 420,
                end: 426,
            },
        );
        // a range that begins at the separator
        let l = r.loc_of(5, r.text.len()).expect("covered");
        assert_eq!((l.file, l.start, l.end), (1, 405, 426));
        // a range that ends at it
        let l = r.loc_of(0, 6).expect("covered");
        assert_eq!((l.file, l.start, l.end), (1, 400, 405));
    }
}
