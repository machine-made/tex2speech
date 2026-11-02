// SPDX-License-Identifier: Apache-2.0
//! The LaTeX reader: tolerant, keeps byte positions, never stops on an error.
//!
//! It reads the source text directly rather than building a tree. `run` walks a range of a source,
//! speaks text, and hands commands and environments to handlers. Macro uses expand into a new source
//! that is run in turn; its positions point back at the use.

mod defs;
mod envs;
mod scan;
mod text;

use crate::Options;
use crate::math::defer;
use crate::raw::{
    Block, BlockKind, Ctx, FileInfo, Hole, LabelInfo, Loc, MATH_CLOSE, MATH_OPEN, Raw, RefStyle,
};
use crate::units;
use scan::{
    control_name, find_dollar_end, find_escaped_end, next_char, read_arg, read_braced,
    read_bracket, skip_comment, skip_ws, strip_comments,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// The braced group at the start of `s`: (inner start, inner end, index after).
pub(crate) fn brace_group(s: &str) -> Option<(usize, usize, usize)> {
    read_braced(s, 0, s.len())
}

/// Commands a document may not redefine: the reader's structure depends on them.
pub(super) const PROTECTED: &[&str] = &[
    "begin",
    "end",
    "item",
    "label",
    "caption",
    "part",
    "chapter",
    "section",
    "subsection",
    "subsubsection",
    "title",
    "author",
    "maketitle",
    "input",
    "include",
    "par",
    "footnote",
    "appendix",
    "newcommand",
    "renewcommand",
    "def",
    "let",
    "usepackage",
    "documentclass",
];

const MAX_DEPTH: u32 = 32;
const MAX_EXPANSIONS: usize = 200_000;
/// Bytes that macro, environment and `\input` expansions may produce in one run. A few small definitions that
/// nest can otherwise write gigabytes within the expansion count.
const MAX_EXPANDED_BYTES: usize = 16 * 1024 * 1024;
const MAX_INCLUDE_DEPTH: usize = 24;
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// A source text. Text made by a macro expansion has `over` set to the place of the use.
pub(super) struct Src {
    pub text: String,
    pub file: u32,
    pub over: Option<(usize, usize)>,
}

#[derive(Clone, Debug)]
pub(super) struct Macro {
    pub nargs: usize,
    /// Default of a first optional argument.
    pub opt: Option<String>,
    pub body: String,
}

#[derive(Clone, Debug)]
pub(super) struct EnvDef {
    pub nargs: usize,
    pub opt: Option<String>,
    pub begin: String,
    pub end: String,
}

#[derive(Clone, Debug)]
pub(super) struct Thm {
    pub title: String,
    /// Key of the counter; shared counters use the key of the theorem they follow.
    pub counter: String,
    pub numbered: bool,
}

/// What a label will point to.
#[derive(Clone, Debug, Default)]
pub(super) struct Anchor {
    pub kind: String,
    pub number: String,
    pub title: String,
}

#[derive(Clone, Debug)]
pub(super) struct Float {
    pub kind: String,
    /// Number given to the float, when it has a caption.
    pub number: Option<String>,
    pub captions: u32,
    pub sub: u32,
    /// Number of the enclosing subfigure, for example "3a".
    pub sub_number: Option<String>,
}

const LEVEL_NAMES: [&str; 7] = [
    "Part",
    "Chapter",
    "Section",
    "Subsection",
    "Subsubsection",
    "Paragraph",
    "Subparagraph",
];

#[allow(clippy::struct_excessive_bools)] // independent parser flags
pub(super) struct Lx<'a> {
    pub cx: &'a mut Ctx,
    pub root_dir: Option<PathBuf>,
    pub stack: Vec<PathBuf>,
    pub loaded: HashMap<PathBuf, Rc<Src>>,
    /// Files already scanned for definitions: (file, read as a class or package) to the shallowest depth it was
    /// scanned from. A file included again is not scanned again, so repeated `\input` costs nothing in the scan.
    pub scanned: HashMap<(u32, bool), usize>,
    pub macros: HashMap<String, Macro>,
    pub envs: HashMap<String, EnvDef>,
    pub thms: HashMap<String, Thm>,
    pub flags: HashMap<String, bool>,
    pub aliases: HashMap<String, String>,
    /// Aliases (`\let\new\old`) made while `\old` was a built-in command: they keep reaching the built-in one
    /// when `\old` is defined again later.
    pub frozen: HashSet<String>,
    // output
    pub cur: Raw,
    pub cur_new_para: bool,
    pub next_new_para: bool,
    pub cont_next: bool,
    pub cur_label: String,
    pub pending_label: String,
    pub capturing: u32,
    pub mute: u32,
    pub depth: u32,
    pub expansions: usize,
    pub expanded: usize,
    pub stop: bool,
    // document state
    pub chapters: bool,
    pub secnumdepth: i32,
    pub sec: [u32; 7],
    pub in_appendix: bool,
    pub appendix_level: usize,
    pub kind_counters: HashMap<String, u32>,
    pub within: HashMap<String, usize>,
    pub anchor: Option<Anchor>,
    pub float: Option<Float>,
    pub subeq: Option<(String, u32)>,
    /// Number given by `\refstepcounter{equation}` to the next displayed equation.
    pub pending_eq: Option<String>,
    /// A title block was written.
    pub title_done: bool,
    /// Blocks of an abstract written before `\maketitle`; they follow the title.
    pub deferred: Vec<Block>,
    pub title: Option<(Rc<Src>, usize, usize)>,
    pub authors: Vec<(Rc<Src>, usize, usize)>,
    pub item_counters: Vec<u32>,
    pub desc: Vec<bool>,
    pub warned: HashSet<String>,
    /// Reading a class or package file: it may not redefine commands the reader handles itself.
    pub in_class: bool,
    /// The text has no `\documentclass` and no document body: maths commands outside `$` read as maths.
    fragment: bool,
}

/// Entry point.
pub(crate) fn read(src: &str, name: &str, opts: &Options, cx: &mut Ctx) {
    let mut lx = Lx {
        cx,
        root_dir: opts.base_dir.clone(),
        stack: Vec::new(),
        loaded: HashMap::new(),
        scanned: HashMap::new(),
        macros: HashMap::new(),
        envs: HashMap::new(),
        thms: HashMap::new(),
        flags: HashMap::new(),
        aliases: HashMap::new(),
        frozen: HashSet::new(),
        cur: Raw::default(),
        cur_new_para: true,
        next_new_para: true,
        cont_next: false,
        cur_label: String::new(),
        pending_label: String::new(),
        capturing: 0,
        mute: 0,
        depth: 0,
        expansions: 0,
        expanded: 0,
        stop: false,
        chapters: false,
        secnumdepth: 3,
        sec: [0; 7],
        in_appendix: false,
        appendix_level: 2,
        kind_counters: HashMap::new(),
        within: HashMap::new(),
        anchor: None,
        float: None,
        subeq: None,
        pending_eq: None,
        title_done: false,
        deferred: Vec::new(),
        title: None,
        authors: Vec::new(),
        item_counters: Vec::new(),
        desc: Vec::new(),
        warned: HashSet::new(),
        in_class: false,
        fragment: !src.contains("\\documentclass") && find_begin_document(src).is_none(),
    };
    for k in ["ifpdf", "iflatex"] {
        lx.flags.insert(k.to_string(), true);
    }
    let root = lx.add_source(name, src.to_string());
    lx.detect_class(&root.text);
    lx.prescan(&root, 0);
    lx.run_file(&root);
    lx.flush();
    let held = std::mem::take(&mut lx.deferred);
    lx.cx.blocks.extend(held);
}

fn letter(n: u32) -> String {
    let c = char::from_u32('A' as u32 + (n.max(1) - 1) % 26).unwrap_or('A');
    c.to_string()
}

impl Lx<'_> {
    // ----- sources and files -----

    pub(super) fn add_source(&mut self, name: &str, text: String) -> Rc<Src> {
        let file = u32::try_from(self.cx.files.len()).unwrap_or(u32::MAX);
        self.cx.files.push(FileInfo::new(name, &text));
        Rc::new(Src {
            text,
            file,
            over: None,
        })
    }

    #[allow(clippy::unused_self)] // keeps the same call shape as its sibling methods
    pub(super) fn loc(&self, src: &Src, a: usize, b: usize) -> Loc {
        match src.over {
            Some((s, e)) => Loc {
                file: src.file,
                start: s,
                end: e,
            },
            None => Loc {
                file: src.file,
                start: a,
                end: b.max(a),
            },
        }
    }

    pub(super) fn warn(&mut self, construct: &str, message: String, loc: Loc) {
        self.cx.warn(construct, message, loc);
    }

    /// Count `n` more expanded bytes. False once the run has passed the budget; the caller then skips the text.
    pub(super) fn charge(&mut self, n: usize, loc: Loc) -> bool {
        let before = self.expanded;
        self.expanded = self.expanded.saturating_add(n);
        if self.expanded <= MAX_EXPANDED_BYTES {
            return true;
        }
        if before <= MAX_EXPANDED_BYTES {
            self.warn(
                "macro",
                format!(
                    "expansions produced more than {} MiB; the rest is skipped",
                    MAX_EXPANDED_BYTES >> 20
                ),
                loc,
            );
        }
        false
    }

    /// Warn once per distinct message in the run, to keep one construct from filling the list.
    pub(super) fn warn_once(&mut self, construct: &str, name: &str, loc: Loc) {
        let key = format!("{construct}:{name}");
        if self.warned.insert(key) {
            self.cx.warn(
                construct,
                format!("{construct} {name} is not understood"),
                loc,
            );
        } else {
            self.cx.warning_total += 1;
        }
    }

    fn detect_class(&mut self, t: &str) {
        let Some(p) = t.find("\\documentclass") else {
            return;
        };
        let end = (p + 400..=t.len())
            .find(|e| t.is_char_boundary(*e))
            .unwrap_or(t.len());
        let seg = &t[p..end];
        if let Some(a) = seg.find('{')
            && let Some(z) = seg[a..].find('}')
        {
            let class = &seg[a + 1..a + z];
            if matches!(
                class,
                "report"
                    | "book"
                    | "scrbook"
                    | "scrreprt"
                    | "memoir"
                    | "extbook"
                    | "extreport"
                    | "ctexbook"
                    | "ctexrep"
            ) {
                self.chapters = true;
                self.secnumdepth = 2;
                self.appendix_level = 1;
                for k in ["equation", "figure", "table"] {
                    self.within.insert(k.to_string(), 1);
                }
            }
        }
    }

    /// Find a file for `\input{name}`. Only files inside the root folder (`Options::base_dir`) are read: an absolute
    /// name, a name that reaches outside by `..` or by a link, and every name when there is no root folder, are
    /// refused.
    fn resolve(&self, name: &str, from: &Src) -> Resolved {
        // Windows writers use a backslash in `\input{sub\file}`; TeX itself wants a slash
        let name = name.trim().replace('\\', "/");
        let name = name.as_str();
        if name.is_empty() {
            return Resolved::Missing;
        }
        let Some(root) = self.root_dir.as_ref().and_then(|r| fs_root(r)) else {
            return Resolved::Outside;
        };
        if name.starts_with('/') || name.starts_with('~') || name.as_bytes().get(1) == Some(&b':') {
            return Resolved::Outside;
        }
        let mut dirs: Vec<PathBuf> = Vec::new();
        if let Some(r) = &self.root_dir {
            dirs.push(r.clone());
        }
        if let Some(f) = self.cx.files.get(from.file as usize)
            && let Some(p) = Path::new(&*f.name).parent()
            && !p.as_os_str().is_empty()
        {
            dirs.push(p.to_path_buf());
        }
        let mut outside = false;
        for d in dirs {
            for ext in ["", ".tex", ".ltx", ".bbl"] {
                let p = d.join(format!("{name}{ext}"));
                if p.is_file() {
                    if within(&root, &p) {
                        return Resolved::Found(p);
                    }
                    outside = true;
                }
            }
        }
        if outside {
            Resolved::Outside
        } else {
            Resolved::Missing
        }
    }

    fn load(&mut self, path: &Path) -> Option<Rc<Src>> {
        let key = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if let Some(s) = self.loaded.get(&key) {
            return Some(s.clone());
        }
        let meta = std::fs::metadata(path).ok()?;
        if meta.len() > MAX_FILE_BYTES {
            return None;
        }
        let decoded = crate::encoding::decode(&std::fs::read(path).ok()?);
        let src = self.add_source(&path.to_string_lossy(), decoded.text);
        if let Some(note) = decoded.note {
            let loc = Loc {
                file: src.file,
                start: 0,
                end: 0,
            };
            self.warn("encoding", note, loc);
        }
        self.loaded.insert(key, src.clone());
        Some(src)
    }

    fn include(&mut self, name: &str, src: &Rc<Src>, at: usize, end: usize) {
        let loc = self.loc(src, at, end);
        let path = match self.resolve(name, src) {
            Resolved::Found(p) => p,
            Resolved::Missing => {
                self.warn("input", format!("file {name} not found"), loc);
                return;
            }
            Resolved::Outside => {
                self.warn(
                    "input",
                    format!("file {name} is outside the document's folder and was not read"),
                    loc,
                );
                return;
            }
        };
        let key = path.canonicalize().unwrap_or_else(|_| path.clone());
        if self.stack.contains(&key) || self.stack.len() >= MAX_INCLUDE_DEPTH {
            self.warn(
                "input",
                format!("file {name} includes itself or is nested too deep"),
                loc,
            );
            return;
        }
        let Some(inc) = self.load(&path) else {
            self.warn("input", format!("file {name} cannot be read"), loc);
            return;
        };
        if !self.charge(inc.text.len(), loc) {
            return;
        }
        self.stack.push(key);
        self.run_file(&inc);
        self.stack.pop();
        self.stop = false;
    }

    /// Run a file: the part before `\begin{document}` is read silently.
    fn run_file(&mut self, src: &Rc<Src>) {
        let t = &src.text;
        if let Some(p) = find_begin_document(t) {
            self.mute += 1;
            self.run(src, 0, p);
            self.mute -= 1;
            let after = p + "\\begin{document}".len();
            self.run(src, after, t.len());
        } else {
            self.run(src, 0, t.len());
        }
    }

    // ----- output -----

    pub(super) fn muted(&self) -> bool {
        self.mute > 0
    }

    pub(super) fn text(&mut self, src: &Src, a: usize, b: usize, s: &str) {
        if self.muted() || s.is_empty() {
            return;
        }
        if self.cur.text.is_empty() && s.trim().is_empty() {
            return;
        }
        if self.cur.text.is_empty() {
            self.cur_new_para = self.next_new_para;
            if !self.pending_label.is_empty() {
                self.cur_label = std::mem::take(&mut self.pending_label);
            }
        }
        let loc = self.loc(src, a, b);
        self.cur.push(s, loc);
    }

    pub(super) fn space(&mut self, src: &Src, a: usize) {
        if self.muted() {
            return;
        }
        let loc = self.loc(src, a, a + 1);
        self.cur.push_space(loc);
    }

    /// Emit the current paragraph as a block.
    pub(super) fn flush(&mut self) {
        if self.capturing > 0 {
            return;
        }
        if self.cur.is_blank() {
            self.cur = Raw::default();
            return;
        }
        let raw = std::mem::take(&mut self.cur);
        let loc = raw.loc_of(0, raw.text.len()).unwrap_or_default();
        let sec = self.section_path();
        let mut b = Block::new(BlockKind::Para, raw, &sec, loc);
        b.new_para = self.cur_new_para;
        b.cont = self.cont_next && !self.cur_new_para;
        b.label = std::mem::take(&mut self.cur_label);
        self.cx.blocks.push(b);
        self.next_new_para = false;
    }

    /// End the paragraph.
    pub(super) fn par(&mut self, src: &Src, at: usize) {
        if self.capturing > 0 {
            self.space(src, at);
            return;
        }
        self.flush();
        self.next_new_para = true;
        self.cont_next = false;
    }

    /// Run `f` with output going to a fresh `Raw`, and return it.
    pub(super) fn capture(&mut self, f: impl FnOnce(&mut Self)) -> Raw {
        let saved = std::mem::take(&mut self.cur);
        let saved_np = self.cur_new_para;
        let saved_label = std::mem::take(&mut self.cur_label);
        self.capturing += 1;
        f(self);
        self.capturing -= 1;
        let out = std::mem::replace(&mut self.cur, saved);
        self.cur_new_para = saved_np;
        self.cur_label = saved_label;
        out
    }

    /// Capture the rendering of `src[a..b]` with the output switched on, whatever the mute state.
    pub(super) fn capture_range(&mut self, src: &Rc<Src>, a: usize, b: usize, unmute: bool) -> Raw {
        let saved_mute = self.mute;
        if unmute {
            self.mute = 0;
        }
        let s = src.clone();
        let r = self.capture(|lx| lx.run(&s, a, b));
        self.mute = saved_mute;
        r
    }

    pub(super) fn section_path(&self) -> Vec<u32> {
        let from = if self.chapters { 1 } else { 2 };
        let n = self.sec.iter().rposition(|c| *c > 0).map_or(0, |p| p + 1);
        if n <= from {
            return Vec::new();
        }
        self.sec[from..n].to_vec()
    }

    // ----- the main loop -----

    pub(super) fn run(&mut self, src: &Rc<Src>, start: usize, end: usize) {
        if self.depth >= MAX_DEPTH {
            let loc = self.loc(src, start, end);
            self.warn_once("nesting", "too deep", loc);
            return;
        }
        self.depth += 1;
        let t: &str = &src.text;
        let b = t.as_bytes();
        let end = end.min(b.len());
        let mut i = start;
        while i < end && !self.stop {
            if self.fragment
                && !self.muted()
                && matches!(b[i], b'\\' | b'(' | b'[' | b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z')
                && crate::loose::at_word_start(t, i)
                && let Some(z) = crate::loose::run_at(t, i, end)
            {
                self.inline_math(src, i, z, i, z);
                i = z;
                continue;
            }
            match b[i] {
                b'\\' => i = self.command(src, i, end),
                b'%' => {
                    i = skip_comment(t, i, end);
                    while i < end && (b[i] == b' ' || b[i] == b'\t') {
                        i += 1;
                    }
                }
                b'$' => i = self.dollar(src, i, end),
                b'{' => {
                    let (a, z, after) = read_braced(t, i, end).unwrap_or((i + 1, end, end));
                    self.run(src, a, z);
                    i = after;
                }
                b'}' | b'&' | b'#' | b'^' | b'_' => i += 1,
                b' ' | b'\t' | b'\r' | b'\n' => {
                    let mut j = i;
                    let mut nl = 0;
                    while j < end && matches!(b[j], b' ' | b'\t' | b'\r' | b'\n') {
                        if b[j] == b'\n' {
                            nl += 1;
                        }
                        j += 1;
                    }
                    if nl >= 2 {
                        self.par(src, i);
                    } else {
                        self.space(src, i);
                    }
                    i = j;
                }
                b'~' => {
                    self.space(src, i);
                    i += 1;
                }
                b'-' if t[i..end].starts_with("--") => {
                    let three = t[i..end].starts_with("---");
                    let n = if three { 3 } else { 2 };
                    let prev_digit = self.cur.text.ends_with(|c: char| c.is_ascii_digit());
                    let next_digit = b.get(i + n).is_some_and(u8::is_ascii_digit);
                    let s = if three {
                        ", "
                    } else if prev_digit && next_digit {
                        " to "
                    } else {
                        "-"
                    };
                    self.text(src, i, i + n, s);
                    i += n;
                }
                b'`' => {
                    i += if b.get(i + 1) == Some(&b'`') { 2 } else { 1 };
                }
                b'\'' if b.get(i + 1) == Some(&b'\'') => i += 2,
                _ => {
                    let mut j = i;
                    while j < end {
                        match b[j] {
                            b'\\' | b'%' | b'$' | b'{' | b'}' | b'&' | b'#' | b'^' | b'_'
                            | b' ' | b'\t' | b'\r' | b'\n' | b'~' | b'`' => break,
                            b'-' if t[j..end].starts_with("--") => break,
                            b'\'' if b.get(j + 1) == Some(&b'\'') => break,
                            _ => j += 1,
                        }
                    }
                    if j == i {
                        j = next_char(t, i);
                    }
                    let j = (j..=end).find(|k| t.is_char_boundary(*k)).unwrap_or(end);
                    self.text(src, i, j, &t[i..j]);
                    i = j;
                }
            }
        }
        self.depth -= 1;
    }

    fn dollar(&mut self, src: &Rc<Src>, i: usize, end: usize) -> usize {
        let t: &str = &src.text;
        let double = t.as_bytes().get(i + 1) == Some(&b'$') && i + 1 < end;
        let open = if double { 2 } else { 1 };
        if let Some(z) = find_dollar_end(t, i + open, end, double) {
            if double {
                self.display(src, i + open, z, "displaymath", true, i, z + 2);
                z + 2
            } else {
                self.inline_math(src, i + 1, z, i, z + 1);
                z + 1
            }
        } else {
            let loc = self.loc(src, i, end);
            self.warn("math syntax", "unclosed $".to_string(), loc);
            i + open
        }
    }

    pub(super) fn inline_math(&mut self, src: &Rc<Src>, a: usize, b: usize, la: usize, lb: usize) {
        if self.muted() {
            return;
        }
        let latex = strip_comments(&src.text[a..b]);
        let latex = self.expand_math(&latex, 0);
        let loc = self.loc(src, la, lb);
        let slot = defer(self.cx, &latex, loc);
        let wrapped = format!("{MATH_OPEN}{slot}{MATH_CLOSE}");
        self.text(src, la, lb, &wrapped);
    }

    // ----- commands -----

    /// Handle the command at `i`; return the index after it and its arguments.
    #[allow(clippy::too_many_lines)] // one pass over the kinds of control sequence
    fn command(&mut self, src: &Rc<Src>, i: usize, end: usize) -> usize {
        let t: &str = &src.text;
        let (name, mut j) = control_name(t, i, end);
        if name.is_empty() {
            return end.min(i + 1);
        }
        let b = t.as_bytes();
        // text accents: \'e \"o \c{c} \v s
        if text::is_accent(name)
            && (!name.as_bytes()[0].is_ascii_alphabetic() || text::is_letter_accent(name))
        {
            return self.accent(src, name, j, end, i);
        }
        let mut star = false;
        if j < end && b[j] == b'*' && starrable(name) {
            star = true;
            j += 1;
        }
        // user definitions take precedence
        if let Some(m) = self.macros.get(name).cloned() {
            return self.use_macro(src, &m, i, j, end);
        }
        if let Some(alias) = self.aliases.get(name).cloned() {
            if self.frozen.contains(name) {
                let old = alias.trim_start_matches('\\').to_string();
                return self.dispatch(src, &old, false, i, j, end);
            }
            let s2 = Rc::new(Src {
                text: alias,
                file: src.file,
                over: Some(self.over_of(src, i, j)),
            });
            let len = s2.text.len();
            self.run(&s2, 0, len);
            return j;
        }
        if name == "(" || name == "[" {
            let close = if name == "(" { b')' } else { b']' };
            return match find_escaped_end(t, j, end, close) {
                Some(z) if name == "(" => {
                    self.inline_math(src, j, z, i, z + 2);
                    z + 2
                }
                Some(z) => {
                    self.display(src, j, z, "displaymath", true, i, z + 2);
                    z + 2
                }
                None => {
                    let loc = self.loc(src, i, j);
                    self.warn("math syntax", "unclosed \\(".to_string(), loc);
                    j
                }
            };
        }
        if name == "$" {
            // \$5 reads "5 dollars"
            let digits = t[j..end]
                .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ','))
                .unwrap_or(end - j);
            let num = t[j..j + digits].trim_end_matches(['.', ',']);
            if !num.is_empty() {
                let s = format!(" {num} dollars ");
                let stop = j + num.len();
                self.text(src, i, stop, &s);
                return stop;
            }
        }
        if name == ")" || name == "]" {
            return j;
        }
        if let Some(sym) = text::symbol(name) {
            // letters such as \ss swallow the space after the name, as TeX does
            if matches!(
                name,
                "ss" | "o"
                    | "O"
                    | "ae"
                    | "AE"
                    | "oe"
                    | "OE"
                    | "aa"
                    | "AA"
                    | "l"
                    | "L"
                    | "i"
                    | "j"
                    | "dh"
                    | "DH"
                    | "th"
                    | "TH"
                    | "ng"
            ) {
                let mut k = j;
                while k < end && matches!(src.text.as_bytes()[k], b' ' | b'\t') {
                    k += 1;
                }
                self.text(src, i, k, sym);
                return k;
            }
            // symbols that follow a control word keep the text going
            if sym == "\n\n" {
                self.par(src, i);
            } else {
                self.text(src, i, j, sym);
            }
            return j;
        }
        if name.starts_with("if") && name != "iff" && name != "ifthenelse" {
            return self.conditional(src, name, i, j, end);
        }
        if let Some(base) = name
            .strip_suffix("true")
            .or_else(|| name.strip_suffix("false"))
            && self.flags.contains_key(&format!("if{base}"))
        {
            self.flags
                .insert(format!("if{base}"), name.ends_with("true"));
            return j;
        }
        self.dispatch(src, name, star, i, j, end)
    }

    #[allow(clippy::unused_self)] // keeps the same call shape as its sibling methods
    fn over_of(&self, src: &Src, a: usize, b: usize) -> (usize, usize) {
        src.over.unwrap_or((a, b))
    }

    fn accent(&mut self, src: &Rc<Src>, name: &str, j: usize, end: usize, i: usize) -> usize {
        let t: &str = &src.text;
        let mut k = j;
        if name.as_bytes()[0].is_ascii_alphabetic() {
            // \c{c} or \c c
            k = skip_ws(t, k, end);
        }
        let Some((a, z, after)) = read_arg(t, k, end) else {
            return j;
        };
        let inner = t[a..z].trim();
        let base = match inner {
            "\\i" => 'i',
            "\\j" => 'j',
            s => s
                .chars()
                .next()
                .filter(|c| c.is_alphabetic())
                .unwrap_or(' '),
        };
        let out = if base == ' ' {
            String::new()
        } else {
            text::accent(name, base).unwrap_or(base).to_string()
        };
        let rest = if inner.chars().count() > 1 && !inner.starts_with('\\') {
            &inner[inner.chars().next().map_or(0, char::len_utf8)..]
        } else {
            ""
        };
        self.text(src, i, after, &format!("{out}{rest}"));
        after
    }

    fn use_macro(&mut self, src: &Rc<Src>, m: &Macro, i: usize, j: usize, end: usize) -> usize {
        self.expansions += 1;
        let loc = self.loc(src, i, j);
        if self.expansions > MAX_EXPANSIONS {
            if self.expansions == MAX_EXPANSIONS + 1 {
                self.warn(
                    "macro",
                    "too many macro expansions; the rest is skipped".to_string(),
                    loc,
                );
            }
            return j;
        }
        // a macro that only opens an environment: the end is further on in this source
        let b = m.body.trim();
        if m.nargs == 0
            && let Some(env) = b.strip_prefix("\\begin{").and_then(|r| r.strip_suffix('}'))
            && !env.contains(['{', '}', '\\'])
        {
            let env = env.to_string();
            return self.env_from(src, i, j, end, &env);
        }
        let (body, after) = self.apply_macro(m, &src.text, j, end);
        if !self.charge(body.len(), loc) {
            return after;
        }
        let over = self.over_of(src, i, after);
        let s2 = Rc::new(Src {
            text: body,
            file: src.file,
            over: Some(over),
        });
        let len = s2.text.len();
        self.run(&s2, 0, len);
        after
    }

    fn conditional(&mut self, src: &Rc<Src>, name: &str, i: usize, j: usize, end: usize) -> usize {
        let t: &str = &src.text;
        let Some((else_pos, fi, after)) = scan::find_fi(t, j, end) else {
            let loc = self.loc(src, i, j);
            self.warn("conditional", format!("\\{name} has no \\fi"), loc);
            return j;
        };
        let loc = self.loc(src, i, j);
        let mut body_start = j;
        let value = match name {
            "iftrue" | "ifhmode" => true,
            "iffalse" | "ifmmode" | "ifinner" | "ifvmode" => false,
            "ifx" => {
                let (a1, e1, n1) = read_arg(t, j, fi).unwrap_or((j, j, j));
                let (a2, e2, n2) = read_arg(t, n1, fi).unwrap_or((n1, n1, n1));
                body_start = n2;
                let (x, y) = (t[a1..e1].trim(), t[a2..e2].trim());
                let defined = |s: &str| {
                    let n = s.trim_start_matches('\\');
                    self.macros.contains_key(n) || self.aliases.contains_key(n)
                };
                x == y || (matches!(y, "\\undefined" | "\\relax" | "\\@undefined") && !defined(x))
            }
            // `\ifnum 2>1`: integers written out; a counter or a register is unknown and takes the else branch
            "ifnum" => {
                let parsed = self.number_at(t, j, fi).and_then(|(x, k)| {
                    let k = skip_ws_same_line(t, k, fi);
                    let rel = *t.as_bytes().get(k)?;
                    let (y, k2) = self.number_at(t, k + 1, fi)?;
                    Some((x, rel, y, k2))
                });
                if let Some((x, rel, y, k)) = parsed
                    && matches!(rel, b'<' | b'>' | b'=')
                {
                    body_start = k;
                    match rel {
                        b'<' => x < y,
                        b'>' => x > y,
                        _ => x == y,
                    }
                } else {
                    self.warn_once("conditional", "\\ifnum", loc);
                    false
                }
            }
            "ifodd" => {
                if let Some((x, k)) = self.number_at(t, j, fi) {
                    body_start = k;
                    x % 2 != 0
                } else {
                    self.warn_once("conditional", "\\ifodd", loc);
                    false
                }
            }
            "ifdefined" | "ifundefined" | "ifcsname" => {
                let (a1, e1, n1) = read_arg(t, j, fi).unwrap_or((j, j, j));
                body_start = n1;
                let d = self
                    .macros
                    .contains_key(t[a1..e1].trim().trim_start_matches('\\'));
                if name == "ifundefined" { !d } else { d }
            }
            n => {
                let key = n.to_string();
                if let Some(v) = self.flags.get(&key) {
                    *v
                } else {
                    self.warn_once("conditional", &format!("\\{n}"), loc);
                    false
                }
            }
        };
        match (value, else_pos) {
            (true, Some((e, _))) => self.run(src, body_start, e),
            (true, None) => self.run(src, body_start, fi),
            (false, Some((_, ea))) => self.run(src, ea, fi),
            (false, None) => {}
        }
        after
    }

    // ----- numbers -----

    /// A number at `j`: digits, or a macro without arguments whose text is a number (`\newcommand\flag{1}`).
    fn number_at(&self, t: &str, j: usize, end: usize) -> Option<(i64, usize)> {
        let k = skip_ws_same_line(t, j, end);
        if t.as_bytes().get(k) == Some(&b'\\') {
            let (name, after) = control_name(t, k, end);
            let m = self.macros.get(name).filter(|m| m.nargs == 0)?;
            let v = m.body.trim().parse::<i64>().ok()?;
            return Some((v, after));
        }
        int_at(t, j, end)
    }

    fn level_number(&self, level: usize) -> String {
        // dotted number of the heading at `level`
        let top = if self.chapters { 1 } else { 2 };
        let mut parts = Vec::new();
        for l in top..=level {
            let n = self.sec[l];
            if l == top && self.in_appendix {
                parts.push(letter(n));
            } else {
                parts.push(n.to_string());
            }
        }
        parts.join(".")
    }

    /// Prefix of a counter that restarts within a heading level, for example "2." or "A.".
    fn within_prefix(&self, kind: &str) -> String {
        match self.within.get(kind) {
            Some(&lvl) => {
                let top = if self.chapters { 1 } else { 2 };
                if lvl < top {
                    return String::new();
                }
                format!("{}.", self.level_number(lvl))
            }
            None => String::new(),
        }
    }

    /// Step the counter of a kind (equation, figure, table, a theorem key) and give its number as text.
    pub(super) fn step_counter(&mut self, kind: &str) -> String {
        let c = self.kind_counters.entry(kind.to_string()).or_insert(0);
        *c += 1;
        let n = *c;
        format!("{}{}", self.within_prefix(kind), n)
    }

    fn step_heading(&mut self, level: usize) {
        self.sec[level] += 1;
        if level > 0 {
            for l in level + 1..7 {
                self.sec[l] = 0;
            }
        }
        let keys: Vec<(String, usize)> = self.within.iter().map(|(k, v)| (k.clone(), *v)).collect();
        for (k, w) in keys {
            if level <= w {
                self.kind_counters.insert(k, 0);
            }
        }
        for (k, v) in self.thm_within() {
            if level <= v {
                self.kind_counters.insert(k, 0);
            }
        }
    }

    fn thm_within(&self) -> Vec<(String, usize)> {
        self.within
            .iter()
            .filter(|(k, _)| k.starts_with("thm:"))
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }

    pub(super) fn set_anchor(&mut self, kind: &str, number: &str, title: &str) {
        self.anchor = Some(Anchor {
            kind: kind.to_string(),
            number: number.to_string(),
            title: title.to_string(),
        });
    }

    pub(super) fn register_label(&mut self, label: &str) {
        let label = label.trim().to_string();
        if label.is_empty() {
            return;
        }
        if let Some(a) = &self.anchor {
            self.cx.labels.insert(
                label,
                LabelInfo {
                    kind: a.kind.clone(),
                    number: a.number.clone(),
                    title: a.title.clone(),
                },
            );
        }
    }

    // ----- sectioning -----

    fn section_command(
        &mut self,
        src: &Rc<Src>,
        level: usize,
        star: bool,
        i: usize,
        j: usize,
        end: usize,
    ) -> usize {
        let t: &str = &src.text;
        let mut k = skip_ws(t, j, end);
        if let Some((_, _, after)) = read_bracket(t, k, end) {
            k = after;
        }
        let Some((a, z, after)) = read_arg(t, k, end) else {
            return j;
        };
        if self.muted() {
            return after;
        }
        self.flush();
        let numbered = !star && (i32::try_from(level).unwrap_or(i32::MAX) - 1) <= self.secnumdepth;
        let top = if self.chapters { 1 } else { 2 };
        let mut label = String::new();
        let mut number = String::new();
        if numbered {
            self.step_heading(level);
            if level == 0 {
                number = self.sec[0].to_string();
                label = format!("Part {number}");
            } else if self.in_appendix && level == self.appendix_level {
                number = letter(self.sec[level]);
                label = format!("Appendix {number}");
            } else {
                number = self.level_number(level);
                label = format!("{} {}", LEVEL_NAMES[level], self.sec[level]);
            }
            let _ = top;
        } else if !star || level != 0 {
            // unnumbered headings do not step counters but still end the paragraph
        }
        let kind = if self.in_appendix && level == self.appendix_level {
            "appendix".to_string()
        } else {
            LEVEL_NAMES[level].to_lowercase()
        };
        self.set_anchor(&kind, &number, "");
        let raw = self.capture_range(src, a, z, false);
        let title = raw.text.replace([MATH_OPEN, MATH_CLOSE], "");
        if let Some(an) = self.anchor.as_mut() {
            an.title = title.trim().to_string();
        }
        // labels written inside the title were registered before the title was known
        let loc = self.loc(src, i, after);
        let sec = self.section_path();
        let kind = if level >= 5 {
            BlockKind::Label
        } else {
            BlockKind::Heading
        };
        let mut blk = Block::new(kind, raw, &sec, loc);
        blk.label = label;
        self.cx.blocks.push(blk);
        self.next_new_para = true;
        self.cont_next = false;
        after
    }

    fn heading_text(&mut self, src: &Rc<Src>, text: &str, at: usize, kind: BlockKind) {
        if self.muted() {
            return;
        }
        self.flush();
        let mut raw = Raw::default();
        let loc = self.loc(src, at, at + 1);
        raw.push(text, loc);
        let sec = self.section_path();
        self.cx.blocks.push(Block::new(kind, raw, &sec, loc));
        self.next_new_para = true;
        self.cont_next = false;
    }

    // ----- dispatch -----

    #[allow(clippy::too_many_lines)]
    fn dispatch(
        &mut self,
        src: &Rc<Src>,
        name: &str,
        star: bool,
        i: usize,
        j: usize,
        end: usize,
    ) -> usize {
        let t: &str = &src.text;
        match name {
            // definitions
            "newcommand"
            | "renewcommand"
            | "providecommand"
            | "DeclareRobustCommand"
            | "def"
            | "gdef"
            | "edef"
            | "xdef"
            | "let"
            | "NewDocumentCommand"
            | "RenewDocumentCommand"
            | "DeclareDocumentCommand"
            | "ProvideDocumentCommand"
            | "DeclareMathOperator"
            | "newenvironment"
            | "renewenvironment"
            | "NewDocumentEnvironment"
            | "DeclareDocumentEnvironment"
            | "newtheorem"
            | "DeclarePairedDelimiter"
            | "DeclarePairedDelimiterX"
            | "newif"
            | "setcounter"
            | "numberwithin"
            | "renewcommand*" => self.parse_def(src, name, star, j, end, false),
            "usepackage" | "RequirePackage" | "documentclass" | "LoadClass" => {
                self.use_package(src, name, i, j, end)
            }
            "begin" => self.begin_env(src, i, j, end),
            "end" => {
                if let Some((a, z, after)) = read_braced(t, j, end) {
                    if &t[a..z] == "document" {
                        self.stop = true;
                    } else if src.over.is_some() {
                        self.open_env_end(src, t[a..z].trim(), i);
                    }
                    return after;
                }
                j
            }
            "input" | "include" | "subfile" | "InputIfFileExists" => {
                if let Some((a, z, after)) = read_arg(t, j, end) {
                    if !self.muted() || name == "input" {
                        self.flush_for_include();
                        self.include(&t[a..z], src, i, after);
                    }
                    return after;
                }
                j
            }
            "import" => {
                // \import{dir}{file}
                let (a1, z1, n1) = read_arg(t, j, end).unwrap_or((j, j, j));
                let (a2, z2, n2) = read_arg(t, n1, end).unwrap_or((n1, n1, n1));
                let joined = format!("{}/{}", t[a1..z1].trim_end_matches('/'), &t[a2..z2]);
                self.include(&joined, src, i, n2);
                n2
            }
            "par" => {
                self.par(src, i);
                j
            }
            "item" => self.item(src, i, j, end),
            "appendix" => {
                self.in_appendix = true;
                self.sec[self.appendix_level] = 0;
                for l in self.appendix_level + 1..7 {
                    self.sec[l] = 0;
                }
                j
            }
            "part" => self.section_command(src, 0, star, i, j, end),
            "chapter" => self.section_command(src, 1, star, i, j, end),
            "section" | "addsec" => {
                self.section_command(src, 2, star || name == "addsec", i, j, end)
            }
            "subsection" => self.section_command(src, 3, star, i, j, end),
            "subsubsection" => self.section_command(src, 4, star, i, j, end),
            "paragraph" => self.section_command(src, 5, star, i, j, end),
            "subparagraph" => self.section_command(src, 6, star, i, j, end),
            "title" => {
                let k = skip_opt(t, j, end);
                if let Some((a, z, after)) = read_arg(t, k, end) {
                    self.title = Some((src.clone(), a, z));
                    return after;
                }
                j
            }
            "author" => {
                let k = skip_opt(t, j, end);
                if let Some((a, z, after)) = read_arg(t, k, end) {
                    self.authors.push((src.clone(), a, z));
                    return after;
                }
                j
            }
            "usetheme" | "usecolortheme" | "usefonttheme" | "useinnertheme" | "useoutertheme"
            | "setbeamertemplate" | "setbeamercolor" | "setbeamerfont" | "subtitle" | "logo"
            | "titlegraphic" | "AtBeginSection" | "AtBeginSubsection" | "institute" => {
                self.skip_command(t, j, end, 1, true)
            }
            "sep" => {
                self.text(src, i, j, ", ");
                j
            }
            "frametitle" => {
                let k = skip_opt(t, j, end);
                if let Some((a, z, after)) = read_arg(t, k, end) {
                    let raw = self.capture_range(src, a, z, false);
                    if !raw.is_blank() && !self.muted() {
                        self.flush();
                        let loc = self.loc(src, i, after);
                        let sec = self.section_path();
                        self.cx
                            .blocks
                            .push(Block::new(BlockKind::Heading, raw, &sec, loc));
                        self.next_new_para = true;
                    }
                    return after;
                }
                j
            }
            "maketitle" | "makeatitle" | "titlepage" => {
                self.make_title(src, i);
                j
            }
            "keywords" | "Keywords" => {
                let k = skip_opt(t, j, end);
                if let Some((a, z, after)) = read_arg(t, k, end) {
                    if !self.muted() {
                        self.flush();
                        let raw = self.capture_range(src, a, z, false);
                        let loc = self.loc(src, i, after);
                        let sec = self.section_path();
                        let mut b = Block::new(BlockKind::Para, raw, &sec, loc);
                        b.label = "Keywords".to_string();
                        self.cx.blocks.push(b);
                        self.next_new_para = true;
                    }
                    return after;
                }
                j
            }
            "acknowledgments" | "acknowledgements" | "acknowledgment" | "acknowledgement" => {
                self.heading_text(src, "Acknowledgements", i, BlockKind::Heading);
                j
            }
            "label" => {
                if let Some((a, z, after)) = read_braced(t, j, end) {
                    self.register_label(&t[a..z]);
                    return after;
                }
                j
            }
            "caption" | "captionof" | "subcaption" | "legend" => {
                self.caption(src, name, star, i, j, end)
            }
            "subfloat" | "subfigure" => self.subfloat(src, i, j, end),
            "footnote" | "footnotetext" | "endnote" | "thanks" => {
                let k = skip_opt(t, j, end);
                if let Some((a, z, after)) = read_arg(t, k, end) {
                    if name != "thanks" && !self.muted() {
                        let raw = self.capture_range(src, a, z, false);
                        if !raw.is_blank() {
                            let h = self.cx.hole(Hole::Footnote {
                                raw: Some(Box::new(raw)),
                                label: None,
                            });
                            let off = self.cur.text.len();
                            self.cur.notes.push((off, h));
                        }
                    }
                    return after;
                }
                j
            }
            "footnotemark" => skip_opt(t, j, end),
            "cite" | "citep" | "citet" | "citealp" | "citealt" | "citeauthor" | "citeyear"
            | "citeyearpar" | "Cite" | "Citep" | "Citet" | "parencite" | "textcite"
            | "autocite" | "supercite" | "citenum" | "fullcite" | "Parencite" | "Textcite"
            | "footcite" | "cites" | "Cites" => self.cite(src, name, j, end),
            "refstepcounter" | "stepcounter" => {
                if let Some((a, z, after)) = read_braced(t, j, end) {
                    if t[a..z].trim() == "equation" && !self.muted() {
                        let n = self.step_counter("equation");
                        self.pending_eq = Some(n);
                    }
                    return after;
                }
                j
            }
            "nocite" => read_arg(t, j, end).map_or(j, |(_, _, a)| a),
            "ref" | "Ref" | "subref" | "eqref" | "autoref" | "Autoref" | "cref" | "Cref"
            | "vref" | "Vref" | "nameref" | "pageref" | "cpageref" | "labelcref" | "crefrange"
            | "Crefrange" => self.reference(src, name, j, end),
            "hyperref" => {
                let k = skip_ws(t, j, end);
                let mut k2 = k;
                let mut labels = String::new();
                if let Some((a, z, after)) = read_bracket(t, k, end) {
                    labels = t[a..z].to_string();
                    k2 = after;
                }
                if let Some((a, z, after)) = read_arg(t, k2, end) {
                    self.run(src, a, z);
                    let _ = labels;
                    return after;
                }
                k2
            }
            "verb" | "lstinline" | "mintinline" => self.verb(src, name, i, j, end),
            "SI" | "qty" | "SIrange" | "qtyrange" | "num" | "numrange" | "si" | "unit" | "ang"
            | "SIlist" | "numlist" | "qtylist" | "sisetup" | "DeclareSIUnit" | "tablenum"
            | "SIlist*" => self.siunitx(src, name, i, j, end),
            "ensuremath" => {
                if let Some((a, z, after)) = read_arg(t, j, end) {
                    self.inline_math(src, a, z, i, after);
                    return after;
                }
                j
            }
            "MakeUppercase" | "uppercase" | "MakeLowercase" | "lowercase" | "textuppercase"
            | "textlowercase" => {
                if let Some((a, z, after)) = read_arg(t, j, end) {
                    let upper = name.to_lowercase().contains("upper");
                    let raw = self.capture_range(src, a, z, false);
                    let s = if upper {
                        raw.text.to_uppercase()
                    } else {
                        raw.text.to_lowercase()
                    };
                    self.text(src, i, after, &s);
                    return after;
                }
                j
            }
            "texorpdfstring" => {
                let (a1, z1, n1) = read_arg(t, j, end).unwrap_or((j, j, j));
                let (_, _, n2) = read_arg(t, n1, end).unwrap_or((n1, n1, n1));
                self.run(src, a1, z1);
                n2
            }
            "bibliography" | "addbibresource" => {
                if let Some((a, z, after)) = read_braced(t, j, end) {
                    self.bibliography(src, &t[a..z]);
                    return after;
                }
                j
            }
            "bibitem" => {
                let k = skip_opt(t, j, end);
                read_arg(t, k, end).map_or(j, |(_, _, a)| a)
            }
            "vskip"
            | "hskip"
            | "kern"
            | "mskip"
            | "moveleft"
            | "moveright"
            | "raise"
            | "lower"
            | "emergencystretch"
            | "looseness"
            | "displaywidowpenalty"
            | "tolerance"
            | "hbadness"
            | "vbadness"
            | "hfuzz"
            | "vfuzz" => skip_dimension(t, j, end),
            "pause" | "qedhere" | "qed" | "linewidth" | "textwidth" | "columnwidth"
            | "textheight" | "paperwidth" | "baselineskip" | "parindent" | "parskip"
            | "arraystretch" => j,
            "hspace" | "vspace" | "hphantom" | "vphantom" | "phantom" | "rule" => {
                // one or two braced arguments
                let k = skip_opt(t, j, end);
                let (_, _, n1) = read_arg(t, k, end).unwrap_or((k, k, k));
                if name == "rule" {
                    return read_arg(t, n1, end).map_or(n1, |(_, _, a)| a);
                }
                n1
            }
            _ => {
                if let Some((n, opt)) = skip_args(name) {
                    return self.skip_command(t, j, end, n, opt);
                }
                if let Some((before, opt)) = transparent(name) {
                    return self.transparent_command(src, t, j, end, before, opt);
                }
                self.unknown_command(src, name, i, j, end)
            }
        }
    }

    /// `\usepackage` and `\documentclass`: read the definitions of a class or package file that sits next to the
    /// document, and note the language of `babel`.
    fn use_package(&mut self, src: &Rc<Src>, name: &str, i: usize, j: usize, end: usize) -> usize {
        let t: &str = &src.text;
        let k = skip_ws(t, j, end);
        let (opts, k) = match read_bracket(t, k, end) {
            Some((a, z, af)) => (t[a..z].to_string(), af),
            None => (String::new(), j),
        };
        let Some((a, z, after)) = read_arg(t, k, end) else {
            return j;
        };
        let names = t[a..z].to_string();
        let loc = self.loc(src, i, after);
        for n in names.split(',').map(str::trim).filter(|n| !n.is_empty()) {
            if n == "babel" || n == "polyglossia" {
                for lang in opts.split(',').map(str::trim) {
                    if matches!(
                        lang,
                        "french"
                            | "francais"
                            | "portuguese"
                            | "brazil"
                            | "brazilian"
                            | "german"
                            | "ngerman"
                            | "spanish"
                            | "italian"
                            | "russian"
                            | "dutch"
                    ) {
                        self.warn(
                            "language",
                            format!(
                                "language {lang} is not supported; the prose is read as English"
                            ),
                            loc,
                        );
                    }
                }
            }
            let ext = if name == "documentclass" || name == "LoadClass" {
                ".cls"
            } else {
                ".sty"
            };
            if let Some(dir) = self.root_dir.clone() {
                let p = dir.join(format!("{n}{ext}"));
                if p.is_file()
                    && fs_root(&dir).is_some_and(|r| within(&r, &p))
                    && self.stack.len() < MAX_INCLUDE_DEPTH
                    && let Some(inc) = self.load(&p)
                {
                    let key = p.canonicalize().unwrap_or(p);
                    if !self.stack.contains(&key) {
                        self.stack.push(key);
                        self.in_class = true;
                        self.prescan(&inc, 1);
                        self.in_class = false;
                        self.stack.pop();
                    }
                }
            }
        }
        after
    }

    #[allow(clippy::unused_self)] // keeps the same call shape as its sibling methods
    fn flush_for_include(&mut self) {
        // an included file may start a new paragraph; keep the text flow of the current one
    }

    #[allow(clippy::unused_self)] // keeps the same call shape as its sibling methods
    fn skip_command(&mut self, t: &str, j: usize, end: usize, n: usize, opt: bool) -> usize {
        let mut k = j;
        if opt {
            k = skip_opt(t, k, end);
        }
        for _ in 0..n {
            match read_arg(t, k, end) {
                Some((_, _, after)) => k = after,
                None => break,
            }
            if opt {
                k = skip_opt(t, k, end);
            }
        }
        k
    }

    /// Speak the last argument; skip `before` arguments first.
    fn transparent_command(
        &mut self,
        src: &Rc<Src>,
        t: &str,
        j: usize,
        end: usize,
        before: usize,
        opt: bool,
    ) -> usize {
        let mut k = j;
        if opt {
            k = skip_opt(t, k, end);
        }
        for _ in 0..before {
            match read_arg(t, k, end) {
                Some((_, _, after)) => k = after,
                None => return k,
            }
            if opt {
                k = skip_opt(t, k, end);
            }
        }
        match read_arg(t, k, end) {
            Some((a, z, after)) => {
                if t.as_bytes()[a..z].first() == Some(&b'\\')
                    || t.as_bytes().get(a.wrapping_sub(1)) == Some(&b'{')
                {
                    // a braced group, or a single command
                }
                self.run(src, a, z);
                after
            }
            None => k,
        }
    }

    fn unknown_command(
        &mut self,
        src: &Rc<Src>,
        name: &str,
        i: usize,
        j: usize,
        end: usize,
    ) -> usize {
        let loc = self.loc(src, i, j);
        self.warn_once("command", &format!("\\{name}"), loc);
        let t: &str = &src.text;
        // speak the braced arguments that follow
        let mut k = skip_opt(t, j, end);
        let mut any = false;
        while let Some((a, z, after)) = read_braced(t, skip_ws_same_line(t, k, end), end) {
            self.run(src, a, z);
            any = true;
            k = skip_opt(t, after, end);
            if k >= end || t.as_bytes()[skip_ws_same_line(t, k, end).min(end - 1)] != b'{' {
                break;
            }
        }
        if any { k } else { j }
    }

    fn item(&mut self, src: &Rc<Src>, i: usize, j: usize, end: usize) -> usize {
        let t: &str = &src.text;
        self.flush();
        self.next_new_para = false;
        self.cont_next = false;
        if let Some(c) = self.item_counters.last_mut() {
            *c += 1;
            let n = *c;
            self.set_anchor("item", &n.to_string(), "");
        }
        let k = skip_ws(t, j, end);
        if let Some((a, z, after)) = read_bracket(t, k, end) {
            if self.desc.last().copied().unwrap_or(false) && !self.muted() {
                let raw = self.capture_range(src, a, z, false);
                if !raw.is_blank() {
                    let sp = self.loc(src, i, after);
                    self.cur_new_para = false;
                    self.cur.push_raw(&raw);
                    self.cur.push(": ", sp);
                }
            }
            return after;
        }
        j
    }

    fn make_title(&mut self, src: &Rc<Src>, at: usize) {
        if self.muted() {
            return;
        }
        self.flush();
        self.title_done = true;
        if let Some((s, a, z)) = self.title.clone() {
            let raw = self.capture_range(&s, a, z, false);
            if !raw.is_blank() {
                let loc = self.loc(src, at, at + 1);
                let loc = raw.loc_of(0, raw.text.len()).unwrap_or(loc);
                let sec = self.section_path();
                self.cx
                    .blocks
                    .push(Block::new(BlockKind::Title, raw, &sec, loc));
            }
        }
        let authors = std::mem::take(&mut self.authors);
        let here = self.loc(src, at, at + 1);
        let mut names = Raw::default();
        for (s, a, z) in &authors {
            let raw = self.capture_range(s, *a, *z, false);
            if raw.is_blank() {
                continue;
            }
            if !names.is_blank() {
                names.push(", ", here);
            }
            names.push_raw(&raw);
        }
        if !names.is_blank() {
            let mut prefixed = Raw::default();
            prefixed.push("By ", here);
            prefixed.push_raw(&names);
            let loc = prefixed.loc_of(0, prefixed.text.len()).unwrap_or(here);
            let sec = self.section_path();
            self.cx
                .blocks
                .push(Block::new(BlockKind::Para, prefixed, &sec, loc));
        }
        let held = std::mem::take(&mut self.deferred);
        self.cx.blocks.extend(held);
        self.next_new_para = true;
    }

    // ----- references -----

    fn cite(&mut self, src: &Rc<Src>, name: &str, j: usize, end: usize) -> usize {
        let t: &str = &src.text;
        let mut k = skip_opt(t, j, end);
        if name == "cites" || name == "Cites" {
            // \cites{a}{b}
            let mut keys = Vec::new();
            while let Some((a, z, after)) = read_braced(t, skip_ws(t, k, end), end) {
                keys.extend(t[a..z].split(',').map(|s| s.trim().to_string()));
                k = skip_opt(t, after, end);
            }
            self.push_cite(src, j, k, false, keys);
            return k;
        }
        let Some((a, z, after)) = read_arg(t, k, end) else {
            return j;
        };
        let keys: Vec<String> = t[a..z]
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if name == "citeyear" || name == "citeyearpar" {
            return after;
        }
        let textual = matches!(
            name,
            "citet" | "textcite" | "Citet" | "Textcite" | "citealt" | "citeauthor"
        );
        self.push_cite(src, j, after, textual, keys);
        after
    }

    fn push_cite(&mut self, src: &Src, a: usize, b: usize, textual: bool, keys: Vec<String>) {
        if self.muted() || keys.is_empty() {
            return;
        }
        let h = self.cx.hole(Hole::Cite { textual, keys });
        let s = Ctx::hole_text(h);
        self.text(src, a, b, &s);
    }

    fn reference(&mut self, src: &Rc<Src>, name: &str, j: usize, end: usize) -> usize {
        let t: &str = &src.text;
        let k = skip_opt(t, j, end);
        let Some((a, z, mut after)) = read_arg(t, k, end) else {
            return j;
        };
        let mut labels: Vec<String> = t[a..z]
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if name.ends_with("range")
            && let Some((a2, z2, n2)) = read_arg(t, after, end)
        {
            labels.push(t[a2..z2].trim().to_string());
            after = n2;
        }
        if self.muted() || labels.is_empty() {
            return after;
        }
        let style = match name {
            "ref" | "Ref" | "subref" => RefStyle::Plain,
            "eqref" => RefStyle::Equation,
            "nameref" => RefStyle::Title,
            "pageref" | "cpageref" => RefStyle::Page,
            "Cref" | "Crefrange" | "autoref" | "Autoref" | "Vref" => RefStyle::NamedCap,
            _ => RefStyle::Named,
        };
        let h = self.cx.hole(Hole::Ref { style, labels });
        let s = Ctx::hole_text(h);
        self.text(src, j, after, &s);
        after
    }

    fn bibliography(&mut self, src: &Rc<Src>, names: &str) {
        // The .bbl file next to the document lists the entries in print order.
        let _ = src;
        let Some(root) = self.root_dir.clone() else {
            return;
        };
        for n in names.split(',') {
            let stem = n.trim().trim_end_matches(".bib");
            let p = root.join(format!("{stem}.bbl"));
            if p.is_file()
                && fs_root(&root).is_some_and(|r| within(&r, &p))
                && let Ok(text) = crate::read_file(&p)
            {
                self.collect_bibitems(&text);
            }
        }
        // the job name is the root file name
        if let Some(name) = self.cx.files.first().map(|f| f.name.clone()) {
            let p = Path::new(&*name).with_extension("bbl");
            if p.is_file()
                && self.cx.bib_order.is_empty()
                && let Ok(text) = crate::read_file(&p)
            {
                self.collect_bibitems(&text);
            }
        }
    }

    pub(super) fn collect_bibitems(&mut self, text: &str) {
        let mut rest = text;
        while let Some(p) = rest.find("\\bibitem") {
            let after = &rest[p + 8..];
            let k = skip_opt(after, 0, after.len());
            if let Some((a, z, nx)) = read_arg(after, k, after.len()) {
                let key = after[a..z].trim().to_string();
                if !key.is_empty() && !self.cx.bib_order.contains(&key) {
                    self.cx.bib_order.push(key);
                }
                rest = &after[nx..];
            } else {
                rest = after;
            }
        }
    }

    fn verb(&mut self, src: &Rc<Src>, name: &str, i: usize, j: usize, end: usize) -> usize {
        let t: &str = &src.text;
        let mut k = j;
        if name == "mintinline" {
            k = skip_opt(t, k, end);
            k = read_arg(t, k, end).map_or(k, |(_, _, a)| a);
        } else if name == "verb" && t.as_bytes().get(k) == Some(&b'*') {
            k += 1;
        }
        let k = if name == "lstinline" {
            skip_opt(t, k, end)
        } else {
            k
        };
        let Some(delim) = t[k.min(end)..end].chars().next() else {
            return k;
        };
        let (a, z, after) = if delim == '{' {
            match read_braced(t, k, end) {
                Some(x) => x,
                None => return k,
            }
        } else {
            let a = k + delim.len_utf8();
            match t[a..end].find(delim) {
                Some(p) => (a, a + p, a + p + delim.len_utf8()),
                None => return a,
            }
        };
        let code = crate::math::speak_code(&t[a..z]);
        self.text(src, i, after, &format!(" {code} "));
        after
    }

    fn siunitx(&mut self, src: &Rc<Src>, name: &str, i: usize, j: usize, end: usize) -> usize {
        let t: &str = &src.text;
        let mut k = skip_opt(t, j, end);
        let mut args: Vec<String> = Vec::new();
        let want = match name {
            "SI" | "qty" | "SIlist" | "qtylist" | "numrange" | "DeclareSIUnit" => 2,
            "SIrange" | "qtyrange" => 3,
            _ => 1,
        };
        for _ in 0..want {
            k = skip_opt(t, k, end);
            match read_arg(t, k, end) {
                Some((a, z, after)) => {
                    args.push(t[a..z].to_string());
                    k = after;
                }
                None => break,
            }
        }
        if name == "sisetup" || name == "DeclareSIUnit" {
            return k;
        }
        let a = |n: usize| args.get(n).map_or("", String::as_str);
        let s = match name {
            "SI" | "qty" => units::speak_quantity(a(0), a(1)),
            "SIrange" | "qtyrange" => units::speak_range(a(0), a(1), a(2)),
            "num" | "tablenum" => units::speak_number(a(0)),
            "numrange" => format!(
                "{} to {}",
                units::speak_number(a(0)),
                units::speak_number(a(1))
            ),
            "si" | "unit" => units::speak_unit(a(0), true),
            "ang" => units::speak_angle(a(0)),
            "SIlist" | "qtylist" | "numlist" => {
                let nums: Vec<String> = a(0).split(';').map(units::speak_number).collect();
                let unit = if want == 2 {
                    units::speak_unit(a(1), true)
                } else {
                    String::new()
                };
                let mut list = match nums.len() {
                    0 | 1 => nums.join(""),
                    n => format!("{} and {}", nums[..n - 1].join(", "), nums[n - 1]),
                };
                if !unit.is_empty() {
                    list = format!("{list} {unit}");
                }
                list
            }
            _ => String::new(),
        };
        self.text(src, i, k, &format!(" {s} "));
        k
    }
}

fn find_begin_document(t: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(p) = t[from..].find("\\begin{document}") {
        let pos = from + p;
        // not inside a comment
        let line_start = t[..pos].rfind('\n').map_or(0, |n| n + 1);
        let before = &t[line_start..pos];
        if !before.contains('%') {
            return Some(pos);
        }
        from = pos + 1;
    }
    None
}

/// Skip any number of `[...]` groups (with white space before them).
pub(super) fn skip_opt(t: &str, mut k: usize, end: usize) -> usize {
    loop {
        let n = skip_ws(t, k, end);
        match read_bracket(t, n, end) {
            Some((_, _, after)) => k = after,
            None => return k,
        }
    }
}

fn skip_ws_same_line(t: &str, mut k: usize, end: usize) -> usize {
    let b = t.as_bytes();
    while k < end && matches!(b[k], b' ' | b'\t' | b'\r') {
        k += 1;
    }
    if k < end && b[k] == b'\n' {
        return skip_ws(t, k, end);
    }
    k
}

/// The integer at `j` (blanks and a sign allowed) and the index after it.
fn int_at(t: &str, j: usize, end: usize) -> Option<(i64, usize)> {
    let b = t.as_bytes();
    let mut k = skip_ws_same_line(t, j, end);
    let start = k;
    if k < end && matches!(b[k], b'-' | b'+') {
        k += 1;
    }
    let digits = k;
    while k < end && b[k].is_ascii_digit() {
        k += 1;
    }
    if k == digits {
        return None;
    }
    t[start..k].parse().ok().map(|v| (v, k))
}

/// Skip a TeX dimension such as ` 2pt plus 1fil` after `\vskip`.
fn skip_dimension(t: &str, j: usize, end: usize) -> usize {
    let b = t.as_bytes();
    let mut k = j;
    while k < end
        && (b[k] == b' '
            || b[k].is_ascii_digit()
            || matches!(b[k], b'.' | b'-' | b'+' | b'=')
            || b[k].is_ascii_lowercase())
    {
        k += 1;
    }
    k
}

/// Commands that may carry a star we should consume.
fn starrable(name: &str) -> bool {
    matches!(
        name,
        "part"
            | "chapter"
            | "section"
            | "subsection"
            | "subsubsection"
            | "paragraph"
            | "subparagraph"
            | "newcommand"
            | "renewcommand"
            | "providecommand"
            | "caption"
            | "captionof"
            | "vspace"
            | "hspace"
            | "DeclareMathOperator"
            | "newtheorem"
            | "counterwithin"
            | "counterwithout"
            | "cref"
            | "Cref"
            | "ref"
            | "item"
            | "footnote"
            | "newenvironment"
            | "renewenvironment"
            | "section*"
            | "addsec"
            | "SI"
            | "qty"
            | "num"
            | "NewDocumentCommand"
            | "DeclarePairedDelimiter"
            | "operatorname"
            | "tag"
            | "\\"
    )
}

/// Commands dropped together with `n` mandatory arguments (and optional arguments when `opt`).
#[allow(clippy::too_many_lines)] // one flat table of command names
fn skip_args(name: &str) -> Option<(usize, bool)> {
    Some(match name {
        "usetikzlibrary"
        | "PassOptionsToPackage"
        | "aff"
        | "corresp"
        | "email"
        | "affiliation"
        | "affil"
        | "address"
        | "inst"
        | "orcid"
        | "cormark"
        | "fnref"
        | "tnoteref"
        | "authorrunning"
        | "titlerunning"
        | "institute"
        | "IEEEauthorblockA"
        | "IEEEauthorblockN_"
        | "date"
        | "pacs"
        | "PACS"
        | "thankstext"
        | "collab"
        | "altaffiliation"
        | "homepage"
        | "preprint"
        | "subjclass"
        | "includegraphics"
        | "includepdf"
        | "includesvg"
        | "lstinputlisting"
        | "verbatiminput"
        | "inputminted" => (1, true),
        "graphicspath"
        | "pagestyle"
        | "thispagestyle"
        | "pagenumbering"
        | "bibliographystyle"
        | "hypersetup"
        | "geometry"
        | "captionsetup"
        | "color"
        | "pagecolor"
        | "newlength"
        | "newcounter"
        | "setlist"
        | "theoremstyle"
        | "urlstyle"
        | "citestyle"
        | "setcitestyle"
        | "lstset"
        | "tikzset"
        | "pgfplotsset"
        | "fancyhf"
        | "markright"
        | "addcontentsline_"
        | "index"
        | "glossary"
        | "printglossary"
        | "nomenclature_"
        | "AtBeginDocument"
        | "AtEndDocument"
        | "selectlanguage"
        | "foreignlanguage_"
        | "linespread"
        | "setmainfont"
        | "setsansfont"
        | "setmonofont"
        | "usetikzlibrary_"
        | "DeclareGraphicsExtensions"
        | "bibpunct"
        | "floatname"
        | "newfloat"
        | "restylefloat"
        | "ProvidesPackage"
        | "ProvidesClass"
        | "NeedsTeXFormat"
        | "label_"
        | "url"
        | "doi"
        | "nolinkurl"
        | "hyphenation"
        | "setstretch"
        | "newpagestyle"
        | "usepgfplotslibrary"
        | "pgfdeclareplotmark"
        | "cleardoublepage_"
        | "DeclareUnicodeCharacter"
        | "DeclareOption"
        | "ProcessOptions_"
        | "ExecuteOptions"
        | "todo"
        | "marginpar"
        | "thispagestyle_"
        | "bibstyle"
        | "bibdata"
        | "citation"
        | "newpage_"
        | "lstdefinestyle"
        | "lstdefinelanguage"
        | "definecolorset"
        | "tcbset"
        | "hypertarget_"
        | "addtocounter"
        | "setlength"
        | "addtolength"
        | "settowidth"
        | "settoheight"
        | "colorlet"
        | "definecolor"
        | "crefname"
        | "Crefname"
        | "labelformat"
        | "numberwithin_"
        | "renewcommand_"
        | "fancyhead"
        | "fancyfoot"
        | "lhead"
        | "chead"
        | "rhead"
        | "lfoot"
        | "cfoot"
        | "rfoot"
        | "markboth"
        | "addcontentsline"
        | "addtocontents"
        | "hypertarget"
        | "hyperlink_"
        | "pdfbookmark"
        | "bookmark"
        | "thispdfpagelabel"
        | "setkeys"
        | "pgfkeys"
        | "tikzstyle"
        | "orcidlink"
        | "Needspace"
        | "setstcolor"
        | "fancypagestyle"
        | "counterwithin"
        | "counterwithout"
        | "fontsize"
        | "DeclareTextFontCommand"
        | "DeclareMathAlphabet"
        | "SetMathAlphabet"
        | "newtheoremstyle"
        | "captionlistentry" => {
            let n = match name {
                "setlength"
                | "addtolength"
                | "settowidth"
                | "settoheight"
                | "colorlet"
                | "crefname"
                | "Crefname"
                | "addtocounter"
                | "markboth"
                | "hypertarget"
                | "labelformat"
                | "DeclareUnicodeCharacter"
                | "lstdefinestyle"
                | "lstdefinelanguage"
                | "definecolorset"
                | "fancypagestyle"
                | "counterwithin"
                | "counterwithout"
                | "fontsize"
                | "DeclareTextFontCommand" => 2,
                "definecolor" | "addcontentsline" | "pdfbookmark" => 3,
                "DeclareMathAlphabet" => 5,
                "SetMathAlphabet" => 6,
                "newtheoremstyle" => 9,
                _ => 1,
            };
            (n, true)
        }
        "begingroup"
        | "endgroup"
        | "allowdisplaybreaks"
        | "unskip"
        | "onecolumngrid"
        | "twocolumngrid"
        | "bgroup"
        | "egroup"
        | "abovedisplayskip"
        | "belowdisplayskip"
        | "abovedisplayshortskip"
        | "belowdisplayshortskip"
        | "global"
        | "floatsep"
        | "noindent"
        | "centering"
        | "raggedright"
        | "raggedleft"
        | "small"
        | "footnotesize"
        | "scriptsize"
        | "tiny"
        | "large"
        | "Large"
        | "LARGE"
        | "huge"
        | "Huge"
        | "normalsize"
        | "normalfont"
        | "bfseries"
        | "itshape"
        | "em"
        | "bf"
        | "it"
        | "rm"
        | "sf"
        | "tt"
        | "sc"
        | "sl"
        | "upshape"
        | "mdseries"
        | "rmfamily"
        | "sffamily"
        | "ttfamily"
        | "scshape"
        | "slshape"
        | "newpage"
        | "clearpage"
        | "cleardoublepage"
        | "pagebreak"
        | "linebreak"
        | "nolinebreak"
        | "nopagebreak"
        | "bigskip"
        | "medskip"
        | "smallskip"
        | "vfill"
        | "hfill"
        | "hrulefill"
        | "dotfill"
        | "indent"
        | "null"
        | "relax"
        | "protect"
        | "ignorespaces"
        | "fussy"
        | "sloppy"
        | "flushbottom"
        | "raggedbottom"
        | "onecolumn"
        | "twocolumn"
        | "tableofcontents"
        | "listoffigures"
        | "listoftables"
        | "printindex"
        | "pause"
        | "qedhere"
        | "makeindex"
        | "makeatletter"
        | "makeatother"
        | "frontmatter"
        | "mainmatter"
        | "backmatter"
        | "allowbreak"
        | "leavevmode"
        | "FloatBarrier"
        | "balance"
        | "hfil"
        | "vfil"
        | "strut"
        | "selectfont"
        | "boldmath"
        | "unboldmath"
        | "displaystyle"
        | "textstyle"
        | "scriptstyle"
        | "hline"
        | "toprule"
        | "midrule"
        | "bottomrule"
        | "cline"
        | "today"
        | "xspace"
        | "noalign"
        | "nobreak"
        | "break"
        | "penalty"
        | "centerline_"
        | "printbibliography"
        | "printglossaries"
        | "printnomenclature"
        | "ifdraft"
        | "pdfoutput"
        | "nonumber"
        | "notag"
        | "qed"
        | "hline_"
        | "tableofcontents_"
        | "titlepage_"
        | "bigbreak"
        | "goodbreak"
        | "samepage"
        | "clubpenalty"
        | "widowpenalty"
        | "hrule"
        | "endgraf"
        | "thinspace"
        | "negthinspace"
        | "hss"
        | "mathversion_"
        | "normalcolor"
        | "reversemarginpar"
        | "normalmarginpar"
        | "pdfpagewidth"
        | "pdfpageheight"
        | "abovecaptionskip"
        | "belowcaptionskip"
        | "captionlistentry_"
        | "listfiles"
        | "IfFileExists_"
        | "csname"
        | "endcsname"
        | "expandafter"
        | "noexpand"
        | "ensuremath_"
        | "limits"
        | "nolimits"
        | "bibliographystyle_"
        | "RaggedRight"
        | "justifying"
        | "doublespacing"
        | "singlespacing"
        | "onehalfspacing"
        | "tracingall"
        | "color_" => (0, false),
        _ => return None,
    })
}

/// Commands that take text arguments: (arguments to skip first, optional arguments allowed).
fn transparent(name: &str) -> Option<(usize, bool)> {
    Some(match name {
        "emph" | "textbf" | "textit" | "textrm" | "textsf" | "texttt" | "textsc" | "textsl"
        | "textup" | "textmd" | "textnormal" | "underline" | "uline" | "sout" | "ul" | "hl"
        | "mbox" | "hbox" | "fbox" | "framebox" | "makebox" | "textsuperscript"
        | "textsubscript" | "text" | "mathrm" | "mathbf" | "mathit" | "mathsf" | "mathtt"
        | "texttt_" | "boxed" | "centerline" | "lowercase_" | "bm" | "boldsymbol" | "mathcal"
        | "mathbb" | "textcolor_" | "noindent_" | "uppercase_" | "textlf" | "textsmaller"
        | "textlarger" | "vbox" | "vtop" | "hyperlink_" | "url_" | "ac" | "acs" | "gls"
        | "glspl" | "acrshort" | "acrlong" | "acrfull" | "textsc_" | "mintinline_"
        | "lstinline_" | "sectionmark" | "abstract" | "Abstract" | "mathnormal" | "mathrm_"
        | "mbox_" => (0, true),
        "textcolor" | "colorbox" | "href" | "parbox" | "raisebox" | "scalebox" | "rotatebox"
        | "adjustbox" | "reflectbox_" | "hypersetup_" | "fcolorbox_" | "mathcolor" => (1, true),
        "resizebox" | "fcolorbox" => (2, true),
        _ => return None,
    })
}

/// Commands the reader handles itself; a class or package file cannot redefine them.
pub(super) fn is_builtin(name: &str) -> bool {
    skip_args(name).is_some()
        || transparent(name).is_some()
        || text::symbol(name).is_some()
        || matches!(
            name,
            "begin"
                | "end"
                | "item"
                | "label"
                | "ref"
                | "eqref"
                | "cite"
                | "citep"
                | "citet"
                | "caption"
                | "part"
                | "chapter"
                | "section"
                | "subsection"
                | "subsubsection"
                | "paragraph"
                | "subparagraph"
                | "title"
                | "author"
                | "maketitle"
                | "input"
                | "include"
                | "par"
                | "footnote"
                | "appendix"
                | "emph"
                | "textbf"
                | "textit"
                | "text"
                | "verb"
                | "SI"
                | "si"
                | "num"
                | "autoref"
                | "cref"
                | "Cref"
                | "newcommand"
                | "renewcommand"
                | "def"
                | "let"
                | "newenvironment"
                | "newtheorem"
                | "bibliography"
                | "usepackage"
                | "documentclass"
                | "thanks"
        )
}

/// The result of looking for an `\input` file.
enum Resolved {
    Found(PathBuf),
    Missing,
    Outside,
}

/// The root folder with links resolved.
fn fs_root(root: &Path) -> Option<PathBuf> {
    let r = if root.as_os_str().is_empty() {
        Path::new(".")
    } else {
        root
    };
    r.canonicalize().ok()
}

/// Whether `p`, with links resolved, lies inside `root` (already resolved).
fn within(root: &Path, p: &Path) -> bool {
    p.canonicalize().is_ok_and(|c| c.starts_with(root))
}
