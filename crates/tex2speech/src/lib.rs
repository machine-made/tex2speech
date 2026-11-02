// SPDX-License-Identifier: Apache-2.0
#![allow(clippy::many_single_char_names)] // byte-offset scanners use i, j, k, a, z the way their loops read best
//! Turn LaTeX and Markdown documents into text that a speech model reads aloud.
//!
//! The library reads a whole document once and gives back a list of [`Segment`]s: sentences, headings,
//! equations, captions and footnotes, each with the place in the source it came from. A caller that has
//! the line numbers of a selection asks for those segments with [`Document::select`].
//!
//! ```
//! use tex2speech::{Document, Format, Options};
//!
//! let doc = Document::parse_str(r"We set $\beta = 43000$.", Format::Latex, &Options::new());
//! assert_eq!(doc.to_text(), "We set beta equal to forty-three thousand.\n");
//! ```
//!
//! # Contract
//!
//! The full contract is in `CONTRACT.md`. In short:
//!
//! - Purpose: `.tex` or `.md` in, spoken text out. English only.
//! - Invariants: only what a reader sees is spoken; no markup reaches the output.
//! - Error model: bad input gives a [`Warning`] and a fallback reading, not an error. Only a file that
//!   cannot be read is an [`Error`].
//! - Determinism: bit-stable.
//! - Unsafe code: none, denied at workspace level.
//! - Not proven: the correctness of any reading beyond the checked-in benchmark and construct corpus.

mod encoding;
mod finish;
mod latex;
mod loose;
mod markdown;
mod math;
mod raw;
mod units;
mod words;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Return the crate version, for example `"0.0.1"`.
///
/// # Examples
///
/// ```
/// assert!(!tex2speech::version().is_empty());
/// ```
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// The format of a source text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Format {
    /// LaTeX source.
    #[default]
    Latex,
    /// Markdown (`CommonMark` with tables, footnotes, task lists, maths and front matter).
    Markdown,
}

impl Format {
    /// The format a file name suggests: `.md`, `.markdown` and `.mdown` are Markdown, everything else LaTeX.
    #[must_use]
    pub fn from_path(path: &Path) -> Self {
        match path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("md" | "markdown" | "mdown" | "mkd") => Self::Markdown,
            _ => Self::Latex,
        }
    }
}

/// The language of the prose. Only English is read so far.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Language {
    /// English.
    #[default]
    English,
}

/// What kind of text a segment holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Kind {
    /// The title of the document.
    Title,
    /// A part, chapter, section or similar heading, or the abstract.
    Heading,
    /// One sentence of running text.
    Sentence,
    /// A displayed equation.
    Equation,
    /// The number of a displayed equation, for example "equation 1,".
    EquationNumber,
    /// A figure or table caption.
    Caption,
    /// A footnote.
    Footnote,
    /// A short label such as "Theorem 2." or "Proof.".
    Label,
}

impl Kind {
    /// Name used in the JSON output.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Heading => "heading",
            Self::Sentence => "sentence",
            Self::Equation => "equation",
            Self::EquationNumber => "equation_number",
            Self::Caption => "caption",
            Self::Footnote => "footnote",
            Self::Label => "label",
        }
    }
}

/// Where a segment comes from in the source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    /// The file, as given to the reader (or found through `\input`).
    pub file: Arc<str>,
    /// First byte.
    pub start: usize,
    /// One past the last byte.
    pub end: usize,
    /// First line, counting from 1.
    pub line: usize,
    /// Last line, counting from 1.
    pub end_line: usize,
}

/// One piece of speech: a voice reads `text`, then waits `pause_ms`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    /// What the text is.
    pub kind: Kind,
    /// The words to speak.
    pub text: String,
    /// Silence after the segment, in milliseconds.
    pub pause_ms: u32,
    /// The source it came from.
    pub span: Span,
    /// Counters of the enclosing headings, for example `[2, 1]` inside section 2, subsection 1.
    pub section: Vec<u32>,
    /// The segment opens a new paragraph.
    pub new_paragraph: bool,
}

/// A construct the reader did not understand or had to repair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warning {
    /// The kind of construct, for example `command`, `environment`, `math command` or `leak`.
    pub construct: String,
    /// What happened.
    pub message: String,
    /// The file.
    pub file: Arc<str>,
    /// The line, counting from 1.
    pub line: usize,
}

/// Why a file could not be read.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The file could not be read.
    Io {
        /// The path.
        path: PathBuf,
        /// The cause.
        source: std::io::Error,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "cannot read {}: {source}", path.display()),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
        }
    }
}

/// Settings for a run. Build with [`Options::new`] and the `with_` methods.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct Options {
    /// The language of the prose.
    pub language: Language,
    /// Name used in spans for text that has no file. Default `<input>`.
    pub name: Option<String>,
    /// Folder where `\input` and `\include` look for files. Not set for text without a file.
    pub base_dir: Option<PathBuf>,
    /// Contents of a user lexicon: lines of `term = spoken form`.
    pub lexicon: Option<String>,
    /// Number of threads that read equations, 1 to 8. Not set: one more thread per 80 distinct equations, up to
    /// the number of cores. The output does not depend on it.
    pub threads: Option<usize>,
}

impl Options {
    /// Default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the name used in spans.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the folder for `\input` and `\include`.
    #[must_use]
    pub fn with_base_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.base_dir = Some(dir.into());
        self
    }

    /// Set the user lexicon text.
    #[must_use]
    pub fn with_lexicon(mut self, text: impl Into<String>) -> Self {
        self.lexicon = Some(text.into());
        self
    }

    /// Set the number of threads that read equations.
    #[must_use]
    pub fn with_threads(mut self, threads: usize) -> Self {
        self.threads = Some(threads);
        self
    }
}

/// A translated document.
#[derive(Clone, Debug, Default)]
pub struct Document {
    segments: Vec<Segment>,
    warnings: Vec<Warning>,
    warning_count: usize,
}

/// Read a text file for `\input` and the like: decoded as `encoding::decode` does, the note dropped.
pub(crate) fn read_file(path: &Path) -> std::io::Result<String> {
    std::fs::read(path).map(|b| encoding::decode(&b).text)
}

impl Document {
    /// Translate source text. Never fails: problems become warnings.
    #[must_use]
    pub fn parse_str(src: &str, format: Format, opts: &Options) -> Self {
        let name = opts.name.clone().unwrap_or_else(|| "<input>".to_string());
        Self::run(src, &name, format, opts, None)
    }

    /// Translate the bytes of a source file. The bytes may be UTF-8 (with or without a byte-order mark), UTF-16 with
    /// a mark, or an 8-bit encoding of older TeX files (Latin-1, Latin-9, Windows-1252; `\usepackage[latin1]{inputenc}`
    /// is honoured). Never fails: a guess, a repair or a file that is not text becomes a warning (construct
    /// `encoding`). Byte offsets in the spans index the decoded text, which is the file itself when the file is
    /// UTF-8 or starts with a UTF-8 byte-order mark.
    #[must_use]
    pub fn parse_bytes(bytes: &[u8], format: Format, opts: &Options) -> Self {
        let name = opts.name.clone().unwrap_or_else(|| "<input>".to_string());
        let d = encoding::decode(bytes);
        Self::run(&d.text, &name, format, opts, d.note)
    }

    /// Read a file and translate it. The format follows the extension; `\input` is resolved from the file's folder.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] when the file cannot be read.
    pub fn parse_file(path: impl AsRef<Path>, opts: &Options) -> Result<Self, Error> {
        let path = path.as_ref();
        Self::parse_file_as(path, Format::from_path(path), opts)
    }

    /// Like [`Document::parse_file`], with the format given instead of taken from the extension.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] when the file cannot be read.
    pub fn parse_file_as(
        path: impl AsRef<Path>,
        format: Format,
        opts: &Options,
    ) -> Result<Self, Error> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let d = encoding::decode(&bytes);
        let mut o = opts.clone();
        if o.base_dir.is_none() {
            o.base_dir = Some(path.parent().map(Path::to_path_buf).unwrap_or_default());
        }
        Ok(Self::run(
            &d.text,
            &path.to_string_lossy(),
            format,
            &o,
            d.note,
        ))
    }

    /// Run the reader. A panic inside it becomes one `internal` warning and an empty document, never a panic in
    /// the caller.
    fn run(src: &str, name: &str, format: Format, opts: &Options, note: Option<String>) -> Self {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Self::run_inner(src, name, format, opts, note)
        }))
        .unwrap_or_else(|_| Self {
            segments: Vec::new(),
            warnings: vec![Warning {
                construct: "internal".to_string(),
                message: "the reader failed on this input; the document is empty".to_string(),
                file: Arc::from(name),
                line: 0,
            }],
            warning_count: 1,
        })
    }

    fn run_inner(
        src: &str,
        name: &str,
        format: Format,
        opts: &Options,
        note: Option<String>,
    ) -> Self {
        #[cfg(test)]
        assert!(!src.contains("\u{1}T2S_PANIC"), "test panic");
        let mut cx = raw::Ctx::default();
        // a byte-order mark that survived (text read by the caller): three spaces keep every offset
        let unmarked;
        let src = match src.strip_prefix('\u{feff}') {
            Some(rest) => {
                unmarked = format!("   {rest}");
                unmarked.as_str()
            }
            None => src,
        };
        let src = &*encoding::lone_cr_to_lf(src);
        match format {
            Format::Latex => latex::read(src, name, opts, &mut cx),
            Format::Markdown => markdown::read(src, name, opts, &mut cx),
        }
        if let Some(n) = note
            && !cx.files.is_empty()
        {
            cx.warn(
                "encoding",
                n,
                raw::Loc {
                    file: 0,
                    start: 0,
                    end: 0,
                },
            );
        }
        math::resolve(
            &mut cx,
            &math::MathEngine::new(opts.lexicon.as_deref(), opts.threads),
        );
        let segments = finish::finish(&mut cx);
        let warnings = cx
            .warnings
            .iter()
            .map(|w| {
                let fi = cx.files.get(w.loc.file as usize);
                Warning {
                    construct: w.construct.clone(),
                    message: w.message.clone(),
                    file: fi.map_or_else(|| Arc::from(""), |f| f.name.clone()),
                    line: fi.map_or(0, |f| f.line(w.loc.start)),
                }
            })
            .collect();
        Self {
            segments,
            warnings,
            warning_count: cx.warning_total,
        }
    }

    /// All segments, in reading order.
    #[must_use]
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// The warnings, at most 5000. See [`Document::warning_count`] for the total.
    #[must_use]
    pub fn warnings(&self) -> &[Warning] {
        &self.warnings
    }

    /// The number of warnings raised, including those not kept.
    #[must_use]
    pub fn warning_count(&self) -> usize {
        self.warning_count
    }

    /// Segments of `file` that touch lines `first_line` to `last_line` (counting from 1, both included).
    /// `file` matches a span's file when the two paths name the same file: they may differ in separators (`/` or
    /// `\\`), in `./` and `..`, in a drive letter or share (case ignored there), or one may be the other's tail.
    #[must_use]
    pub fn select(&self, file: &str, first_line: usize, last_line: usize) -> Vec<&Segment> {
        let (lo, hi) = (first_line.min(last_line), first_line.max(last_line));
        self.segments
            .iter()
            .filter(|s| same_file(&s.span.file, file) && s.span.line <= hi && s.span.end_line >= lo)
            .collect()
    }

    /// Plain text: one segment per line, a blank line before headings and between paragraphs.
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        let mut prev: Option<&Segment> = None;
        for s in &self.segments {
            if let Some(p) = prev {
                let blank = matches!(s.kind, Kind::Title | Kind::Heading)
                    || (s.new_paragraph
                        && matches!(s.kind, Kind::Sentence | Kind::Label)
                        && matches!(
                            p.kind,
                            Kind::Sentence | Kind::Equation | Kind::EquationNumber | Kind::Footnote
                        ));
                if blank {
                    out.push('\n');
                }
            }
            out.push_str(&s.text);
            out.push('\n');
            prev = Some(s);
        }
        out
    }

    /// JSON: `{"segments": [...], "warnings": [...]}`.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut o = String::from("{\"segments\": [");
        for (i, s) in self.segments.iter().enumerate() {
            if i > 0 {
                o.push(',');
            }
            let _ = write!(
                o,
                "\n  {{\"kind\": {}, \"text\": {}, \"pause_ms\": {}, \"span\": {{\"file\": {}, \"start\": {}, \"end\": {}, \"line\": {}, \"end_line\": {}}}, \"section\": [{}], \"new_paragraph\": {}}}",
                json_str(s.kind.name()),
                json_str(&s.text),
                s.pause_ms,
                json_str(&s.span.file),
                s.span.start,
                s.span.end,
                s.span.line,
                s.span.end_line,
                s.section
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
                s.new_paragraph,
            );
        }
        o.push_str("\n], \"warnings\": [");
        for (i, w) in self.warnings.iter().enumerate() {
            if i > 0 {
                o.push(',');
            }
            let _ = write!(
                o,
                "\n  {{\"construct\": {}, \"message\": {}, \"file\": {}, \"line\": {}}}",
                json_str(&w.construct),
                json_str(&w.message),
                json_str(&w.file),
                w.line
            );
        }
        let _ = write!(o, "\n], \"warning_count\": {}}}\n", self.warning_count);
        o
    }
}

/// A path as a list of its parts, so that spellings of one file compare equal: both separators, `.` and `..`,
/// repeated separators, a `\\?\` prefix, and a Windows drive letter or UNC share (case does not matter there).
fn path_parts(p: &str) -> (Vec<String>, bool) {
    let p = p.replace('\\', "/");
    let p = p.strip_prefix("//?/").unwrap_or(&p);
    let p = p.strip_prefix("file:///").unwrap_or(p);
    let windows = p.as_bytes().get(1) == Some(&b':') && p.as_bytes()[0].is_ascii_alphabetic()
        || p.starts_with("//");
    let mut parts: Vec<String> = Vec::new();
    for part in p.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|l| l != "..") {
                    parts.pop();
                } else {
                    parts.push("..".to_string());
                }
            }
            x => parts.push(x.to_string()),
        }
    }
    (parts, windows)
}

/// The two paths name the same file when one, as a list of parts, ends with the other.
fn same_file(a: &str, b: &str) -> bool {
    let (pa, wa) = path_parts(a);
    let (pb, wb) = path_parts(b);
    let fold = wa || wb;
    let eq = |x: &String, y: &String| {
        if fold {
            x.to_lowercase() == y.to_lowercase()
        } else {
            x == y
        }
    };
    let (long, short) = if pa.len() >= pb.len() {
        (&pa, &pb)
    } else {
        (&pb, &pa)
    };
    !short.is_empty()
        && long[long.len() - short.len()..]
            .iter()
            .zip(short.iter())
            .all(|(x, y)| eq(x, y))
}

fn json_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// Characters of `text` that a voice cannot be expected to say: anything that is not a letter, a digit,
/// white space or ordinary punctuation. Used by tests and by the corpus sweep.
#[must_use]
pub fn unspeakable_chars(text: &str) -> Vec<char> {
    text.chars()
        .filter(|c| {
            !(c.is_alphanumeric()
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
                        | '%'
                        | '+'
                        | '='
                        | '<'
                        | '>'
                        | '\u{2026}'
                        | '\u{2019}'
                        | '\u{201c}'
                        | '\u{201d}'
                        | '\u{2013}'
                        | '\u{2014}'
                ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panic_in_the_reader_gives_an_empty_document_with_a_warning() {
        let d = Document::parse_str("text \u{1}T2S_PANIC", Format::Latex, &Options::new());
        assert_eq!(d.segments().len(), 0);
        assert_eq!(d.warning_count(), 1);
        assert_eq!(d.warnings()[0].construct, "internal");
    }
}
