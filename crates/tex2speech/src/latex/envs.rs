// SPDX-License-Identifier: Apache-2.0
//! Environments, displayed equations, captions and theorems.

use super::scan::{
    find_env_end, find_literal, read_arg, read_braced, read_bracket, skip_ws, split_rows,
    strip_comments,
};
use super::{Float, Lx, Src, skip_opt};
use crate::math::{defer, split_trailing_punct};
use crate::raw::{Block, BlockKind, Loc, MATH_CLOSE, MATH_OPEN, Raw};
use std::rc::Rc;

/// Environments whose content is code or hidden: skipped up to the literal `\end`.
const LITERAL: &[&str] = &[
    "verbatim",
    "verbatim*",
    "Verbatim",
    "Verbatim*",
    "BVerbatim",
    "LVerbatim",
    "lstlisting",
    "minted",
    "comment",
    "filecontents",
    "filecontents*",
    "alltt",
    "listing",
    "codeblock",
    "lstlisting*",
];

/// Environments skipped whole: tables, pictures, algorithms bodies.
const SKIPPED: &[&str] = &[
    "tabular",
    "tabular*",
    "tabularx",
    "tabulary",
    "array",
    "supertabular",
    "xtabular",
    "NiceTabular",
    "tabu",
    "longtabu",
    "tikzpicture",
    "pgfpicture",
    "picture",
    "axis",
    "tikzcd",
    "circuitikz",
    "forest",
    "pspicture",
    "overpic",
    "algorithmic",
    "algorithmicx",
    "algpseudocode",
    "pseudocode",
    "thebibliography_",
    "ignore",
    "tcolorbox_",
    "asy",
    "gnuplot",
    "matrix_",
];

const FLOATS: &[&str] = &[
    "figure",
    "figure*",
    "table",
    "table*",
    "sidewaysfigure",
    "sidewaystable",
    "wrapfigure",
    "wraptable",
    "algorithm",
    "algorithm*",
    "figurewide",
    "SCfigure",
    "longtable",
    "listing_",
];

const LISTS: &[&str] = &[
    "itemize",
    "enumerate",
    "description",
    "compactitem",
    "compactenum",
    "compactdesc",
    "inparaenum",
    "inparaitem",
    "asparaitem",
    "asparaenum",
    "itemize*",
    "enumerate*",
    "description*",
    "thmlist",
];

const DISPLAYS: &[&str] = &[
    "equation",
    "displaymath",
    "align",
    "gather",
    "multline",
    "eqnarray",
    "flalign",
    "alignat",
    "dmath",
    "IEEEeqnarray",
    "dgroup",
];

const DEFAULT_THEOREMS: &[&str] = &[
    "theorem",
    "lemma",
    "proposition",
    "corollary",
    "definition",
    "remark",
    "example",
    "conjecture",
    "claim",
    "assumption",
    "fact",
    "hypothesis",
    "notation",
    "observation",
    "problem",
    "question",
    "exercise",
    "note",
    "property",
    "criterion",
    "condition",
    "axiom",
    "principle",
    "proposition",
    "lem",
    "thm",
    "prop",
    "cor",
    "defn",
    "rem",
    "conj",
    "assump",
];

fn title_case(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or_else(String::new, |f| {
        f.to_uppercase().collect::<String>() + c.as_str()
    })
}

fn strip_star(s: &str) -> &str {
    s.strip_suffix('*').unwrap_or(s)
}

impl Lx<'_> {
    pub(super) fn begin_env(&mut self, src: &Rc<Src>, i: usize, j: usize, end: usize) -> usize {
        let t: &str = &src.text;
        let Some((a, z, after_name)) = read_braced(t, j, end) else {
            return j;
        };
        let name = t[a..z].trim().to_string();
        self.env_from(src, i, after_name, end, &name)
    }

    /// The environment `name`, whose opening ends at `after_name`.
    pub(super) fn env_from(
        &mut self,
        src: &Rc<Src>,
        i: usize,
        after_name: usize,
        end: usize,
        name: &str,
    ) -> usize {
        let t: &str = &src.text;
        let name = name.to_string();
        let loc = self.loc(src, i, after_name);
        if LITERAL.contains(&name.as_str()) {
            let fin = format!("\\end{{{name}}}");
            return if let Some(p) = find_literal(t, after_name, &fin, end) {
                p + fin.len()
            } else {
                self.warn("environment", format!("{name} is not closed"), loc);
                end
            };
        }
        let (openers, closers) = self.env_macros(&name);
        let Some((body_end, after)) = find_env_end(t, after_name, &name, end, &openers, &closers)
        else {
            if src.over.is_some() {
                // text made by a macro: `\newenvironment{x}{\begin{quote}}{\end{quote}}` closes in another text
                self.open_env_begin(src, &name, i);
            } else {
                self.warn("environment", format!("{name} is not closed"), loc);
                let (be, af) = (end, end);
                self.env_body(src, &name, i, after_name, be, af, loc);
            }
            return end;
        };
        self.env_body(src, &name, i, after_name, body_end, after, loc);
        after
    }

    /// The beginning of an environment whose end is in another text.
    fn open_env_begin(&mut self, src: &Rc<Src>, name: &str, i: usize) {
        let base = strip_star(name);
        if LISTS.contains(&name) {
            self.par(src, i);
            self.item_counters.push(0);
            self.desc.push(base.contains("desc"));
        } else if matches!(base, "quote" | "quotation" | "verse") {
            self.par(src, i);
            self.pending_label = "Quote".to_string();
        } else {
            self.par(src, i);
        }
    }

    /// A stray `\end{name}`: the end of an environment opened in another text.
    pub(super) fn open_env_end(&mut self, src: &Rc<Src>, name: &str, i: usize) {
        if LISTS.contains(&name) && !self.item_counters.is_empty() {
            self.flush();
            self.item_counters.pop();
            self.desc.pop();
        }
        self.par(src, i);
    }

    #[allow(clippy::too_many_arguments)]
    fn env_body(
        &mut self,
        src: &Rc<Src>,
        name: &str,
        i: usize,
        after_name: usize,
        body_end: usize,
        after: usize,
        loc: Loc,
    ) -> usize {
        let t: &str = &src.text;
        let name = name.to_string();
        if SKIPPED.contains(&name.as_str()) {
            return after;
        }
        self.flush_before_env(&name);
        // user environments
        if let Some(def) = self.envs.get(&name).cloned() {
            self.expansions += 1;
            if self.expansions > super::MAX_EXPANSIONS {
                return after;
            }
            let mb = super::Macro {
                nargs: def.nargs,
                opt: def.opt.clone(),
                body: def.begin.clone(),
            };
            let me = super::Macro {
                body: def.end.clone(),
                ..mb.clone()
            };
            let (begin_text, k) = self.apply_macro(&mb, t, after_name, body_end);
            let (end_text, _) = self.apply_macro(&me, t, after_name, body_end);
            if !self.charge(begin_text.len() + end_text.len(), loc) {
                return after;
            }
            let over = self.over_of(src, i, after);
            let b_src = Rc::new(Src {
                text: begin_text,
                file: src.file,
                over: Some(over),
            });
            let e_src = Rc::new(Src {
                text: end_text,
                file: src.file,
                over: Some(over),
            });
            let (bl, el) = (b_src.text.len(), e_src.text.len());
            self.run(&b_src, 0, bl);
            self.run(src, k.min(body_end), body_end);
            self.run(&e_src, 0, el);
            return after;
        }
        let base = strip_star(&name).to_string();
        let k = after_name;
        if base == "subeqnarray" {
            let n = self.step_counter("equation");
            let saved = self.subeq.replace((n, 0));
            self.display(src, k, body_end, "eqnarray", false, i, after);
            self.subeq = saved;
            return after;
        }
        if DISPLAYS.contains(&base.as_str()) || base == "math" {
            let mut k = k;
            if (base == "alignat" || base == "xalignat")
                && let Some((_, _, af)) = read_braced(t, skip_ws(t, k, body_end), body_end)
            {
                k = af;
            }
            if base == "math" {
                self.inline_math(src, k, body_end, i, after);
            } else {
                self.display(src, k, body_end, &base, name.ends_with('*'), i, after);
            }
            return after;
        }
        if FLOATS.contains(&name.as_str()) {
            self.float_env(src, &name, k, body_end);
            return after;
        }
        if LISTS.contains(&name.as_str()) {
            self.par(src, i);
            let k = skip_opt(t, k, body_end);
            self.item_counters.push(0);
            self.desc.push(base.contains("desc"));
            self.run(src, k, body_end);
            self.flush();
            self.item_counters.pop();
            self.desc.pop();
            self.next_new_para = true;
            self.cont_next = false;
            return after;
        }
        self.other_env(src, &name, &base, i, k, body_end, after);
        after
    }

    /// Macros that only open or only close the environment `name`, such as `\newcommand{\be}{\begin{equation}}`.
    fn env_macros(&self, name: &str) -> (Vec<String>, Vec<String>) {
        let (open, close) = (format!("\\begin{{{name}}}"), format!("\\end{{{name}}}"));
        let mut o = Vec::new();
        let mut c = Vec::new();
        for (k, m) in &self.macros {
            if m.nargs == 0 {
                let b = m.body.trim();
                if b == open {
                    o.push(k.clone());
                } else if b == close {
                    c.push(k.clone());
                }
            }
        }
        (o, c)
    }

    #[allow(clippy::unused_self)] // keeps the same call shape as its sibling methods
    fn flush_before_env(&mut self, name: &str) {
        // inline environments keep the paragraph going; blocks end it
        let _ = name;
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_lines)] // one flat table of commands; not split
    fn other_env(
        &mut self,
        src: &Rc<Src>,
        name: &str,
        base: &str,
        i: usize,
        k: usize,
        body_end: usize,
        after: usize,
    ) {
        let t: &str = &src.text;
        let loc = self.loc(src, i, after);
        match base {
            "subfigure" | "subtable" => {
                if let Some(f) = self.float.as_mut() {
                    f.sub += 1;
                    let letter = char::from_u32('a' as u32 + (f.sub - 1) % 26).unwrap_or('a');
                    f.sub_number =
                        Some(format!("{}{letter}", f.number.clone().unwrap_or_default()));
                }
                self.run(src, k, body_end);
                if let Some(f) = self.float.as_mut() {
                    f.sub_number = None;
                }
            }
            "frontmatter" => {
                let start = self.cx.blocks.len();
                self.run(src, k, body_end);
                self.flush();
                if !self.title_done {
                    let mid = self.cx.blocks.len();
                    self.make_title(src, after);
                    // the title comes first
                    self.cx.blocks[start..].rotate_left(mid - start);
                }
            }
            "document" => {
                self.run(src, k, body_end);
                self.stop = true;
            }
            "abstract" => {
                // Before \maketitle the abstract is typeset after the title.
                let hold = !self.title_done && t[after..].contains("\\maketitle");
                let start = self.cx.blocks.len();
                self.heading_text(src, "Abstract", i, BlockKind::Heading);
                self.run(src, k, body_end);
                self.par(src, after);
                if hold {
                    let moved: Vec<Block> = self.cx.blocks.drain(start..).collect();
                    self.deferred.extend(moved);
                }
            }
            "acknowledgments" | "acknowledgements" | "acknowledgment" | "acknowledgement" => {
                self.heading_text(src, "Acknowledgements", i, BlockKind::Heading);
                self.run(src, k, body_end);
                self.par(src, after);
            }
            "keywords" | "keyword" | "IEEEkeywords" => {
                self.par(src, i);
                self.cur_label = String::new();
                self.pending_label = "Keywords".to_string();
                self.run(src, k, body_end);
                self.par(src, after);
            }
            "quote" | "quotation" | "verse" | "displayquote" => {
                self.par(src, i);
                self.pending_label = "Quote".to_string();
                self.run(src, k, body_end);
                self.par(src, after);
            }
            "proof" => {
                self.par(src, i);
                let kk = skip_ws(t, k, body_end);
                let (custom, kk) = match read_bracket(t, kk, body_end) {
                    Some((a, z, af)) => (Some((a, z)), af),
                    None => (None, k),
                };
                let raw = match custom {
                    Some((a, z)) => self.capture_range(src, a, z, false),
                    None => Raw::default(),
                };
                let mut b = Block::new(BlockKind::Label, raw, &self.section_path(), loc);
                if custom.is_none() {
                    b.label = "Proof".to_string();
                }
                if !self.muted() {
                    self.cx.blocks.push(b);
                }
                self.run(src, kk, body_end);
                self.par(src, after);
            }
            "appendix" | "appendices" => {
                self.in_appendix = true;
                self.sec[self.appendix_level] = 0;
                self.run(src, k, body_end);
                self.in_appendix = false;
            }
            "thebibliography" => {
                let text = t[k..body_end].to_string();
                self.collect_bibitems(&text);
            }
            "frame" => {
                self.par(src, i);
                let mut kk = skip_opt(t, k, body_end);
                if let Some((a, z, af)) = read_braced(t, skip_ws(t, kk, body_end), body_end) {
                    let raw = self.capture_range(src, a, z, false);
                    if !raw.is_blank() && !self.muted() {
                        let sec = self.section_path();
                        self.cx
                            .blocks
                            .push(Block::new(BlockKind::Heading, raw, &sec, loc));
                    }
                    kk = af;
                }
                self.run(src, kk, body_end);
                self.par(src, after);
            }
            "block" | "alertblock" | "exampleblock" => {
                self.par(src, i);
                let kk = skip_opt(t, k, body_end);
                let mut kk2 = kk;
                if let Some((a, z, af)) = read_braced(t, skip_ws(t, kk, body_end), body_end) {
                    let raw = self.capture_range(src, a, z, false);
                    if !raw.is_blank() && !self.muted() {
                        let sec = self.section_path();
                        self.cx
                            .blocks
                            .push(Block::new(BlockKind::Label, raw, &sec, loc));
                    }
                    kk2 = af;
                }
                self.run(src, kk2, body_end);
                self.par(src, after);
            }
            "minipage" | "multicols" | "adjustbox" | "column" | "spacing" | "list"
            | "tcolorbox" | "mdframed" | "framed" | "textblock" | "wrapfigure_" => {
                self.par(src, i);
                let mut kk = skip_opt(t, k, body_end);
                let nargs = if base == "list" { 2 } else { 1 };
                for _ in 0..nargs {
                    if base == "tcolorbox" || base == "mdframed" || base == "framed" {
                        break;
                    }
                    if let Some((_, _, af)) = read_braced(t, skip_ws(t, kk, body_end), body_end) {
                        kk = skip_opt(t, af, body_end);
                    }
                }
                self.run(src, kk, body_end);
                self.par(src, after);
            }
            _ => {
                if let Some(th) = self.theorem_info(name, base) {
                    self.theorem_env(src, &th, k, body_end, i, after);
                    return;
                }
                if !matches!(
                    base,
                    "center"
                        | "flushleft"
                        | "flushright"
                        | "titlepage"
                        | "frontmatter_"
                        | "sloppypar"
                        | "samepage"
                        | "singlespace"
                        | "doublespace"
                        | "onehalfspace"
                        | "landscape"
                        | "small"
                        | "footnotesize"
                        | "large"
                        | "Large"
                        | "columns"
                        | "raggedright"
                        | "raggedleft"
                        | "center*"
                        | "otherlanguage"
                        | "sidewaystext"
                        | "subequations"
                        | "paragraph"
                        | "adjustwidth"
                        | "quote*"
                        | "refsection"
                        | "bibunit"
                        | "appendixpage"
                        | "scriptsize"
                        | "tiny"
                        | "normalsize"
                        | "huge"
                        | "Huge"
                ) {
                    self.warn_once("environment", name, loc);
                }
                if base == "subequations" {
                    let n = self.step_counter("equation");
                    let saved = self.subeq.replace((n, 0));
                    self.run(src, k, body_end);
                    self.subeq = saved;
                    return;
                }
                self.par(src, i);
                let kk = skip_opt(t, k, body_end);
                self.run(src, kk, body_end);
                self.par(src, after);
            }
        }
    }

    fn theorem_info(&self, name: &str, base: &str) -> Option<super::Thm> {
        if let Some(th) = self.thms.get(name).or_else(|| self.thms.get(base)) {
            let mut th = th.clone();
            if name.ends_with('*') {
                th.numbered = false;
            }
            return Some(th);
        }
        if DEFAULT_THEOREMS.contains(&base) {
            let title = match base {
                "thm" => "Theorem",
                "lem" => "Lemma",
                "prop" => "Proposition",
                "cor" => "Corollary",
                "defn" => "Definition",
                "rem" => "Remark",
                "conj" => "Conjecture",
                "assump" => "Assumption",
                b => {
                    return Some(super::Thm {
                        title: title_case(b),
                        counter: b.to_string(),
                        numbered: !name.ends_with('*'),
                    });
                }
            };
            return Some(super::Thm {
                title: title.to_string(),
                counter: base.to_string(),
                numbered: !name.ends_with('*'),
            });
        }
        None
    }

    fn theorem_env(
        &mut self,
        src: &Rc<Src>,
        th: &super::Thm,
        k: usize,
        body_end: usize,
        i: usize,
        after: usize,
    ) {
        let t: &str = &src.text;
        self.par(src, i);
        let kk = skip_ws(t, k, body_end);
        let (custom, kk) = match read_bracket(t, kk, body_end) {
            Some((a, z, af)) => (Some((a, z)), af),
            None => (None, k),
        };
        let number = if th.numbered {
            Some(self.step_counter(&format!("thm:{}", th.counter)))
        } else {
            None
        };
        let label = match &number {
            Some(n) => format!("{} {n}", th.title),
            None => th.title.clone(),
        };
        self.set_anchor(
            &th.title.to_lowercase(),
            number.as_deref().unwrap_or(""),
            "",
        );
        let raw = match custom {
            Some((a, z)) => self.capture_range(src, a, z, false),
            None => Raw::default(),
        };
        if !self.muted() {
            let loc = self.loc(src, i, after);
            let mut b = Block::new(BlockKind::Label, raw, &self.section_path(), loc);
            b.label = label;
            self.cx.blocks.push(b);
            self.next_new_para = true;
        }
        self.run(src, kk, body_end);
        self.par(src, after);
    }

    fn float_env(&mut self, src: &Rc<Src>, name: &str, k: usize, body_end: usize) {
        let body = &src.text[k..body_end];
        let kind = if name.contains("algorithm") {
            "algorithm"
        } else if name.contains("table") {
            "table"
        } else {
            "figure"
        };
        self.flush();
        let numbered = body.contains("\\caption") && !body.contains("\\caption*");
        let number = if numbered {
            Some(self.step_counter(kind))
        } else {
            None
        };
        if let Some(n) = &number {
            self.set_anchor(kind, n, "");
        }
        let prev = self.float.replace(Float {
            kind: kind.to_string(),
            number,
            captions: 0,
            sub: 0,
            sub_number: None,
        });
        self.mute += 1;
        self.run(src, k, body_end);
        self.mute -= 1;
        self.float = prev;
        self.next_new_para = true;
        self.cont_next = false;
    }

    pub(super) fn caption(
        &mut self,
        src: &Rc<Src>,
        name: &str,
        star: bool,
        i: usize,
        j: usize,
        end: usize,
    ) -> usize {
        let t: &str = &src.text;
        let mut k = j;
        let mut kind_override: Option<String> = None;
        if name == "captionof"
            && let Some((a, z, after)) = read_arg(t, k, end)
        {
            kind_override = Some(t[a..z].trim().trim_end_matches('*').to_string());
            k = after;
        }
        k = skip_opt(t, k, end);
        let Some((a, z, after)) = read_arg(t, k, end) else {
            return j;
        };
        if self.capturing > 0 {
            return after;
        }
        let kind = kind_override
            .or_else(|| self.float.as_ref().map(|f| f.kind.clone()))
            .unwrap_or_else(|| "figure".to_string());
        let sub = self.float.as_ref().and_then(|f| f.sub_number.clone());
        let number: Option<String> = if star {
            None
        } else if let Some(s) = sub {
            Some(s)
        } else {
            let pre = self.float.as_mut().and_then(|f| {
                f.captions += 1;
                if f.captions == 1 {
                    f.number.clone()
                } else {
                    None
                }
            });
            Some(pre.unwrap_or_else(|| self.step_counter(&kind)))
        };
        if let Some(n) = &number {
            self.set_anchor(&kind, n, "");
        }
        self.flush();
        let raw = self.capture_range(src, a, z, true);
        let mut label = title_case(&kind);
        if let Some(n) = &number {
            label = format!("{label} {n}");
        }
        let loc = self.loc(src, i, after);
        let sec = self.section_path();
        let mut b = Block::new(BlockKind::Caption, raw, &sec, loc);
        b.label = label;
        self.cx.blocks.push(b);
        self.next_new_para = true;
        after
    }

    /// `\subfloat[caption]{content}` and `\subfigure[caption]{content}`.
    pub(super) fn subfloat(&mut self, src: &Rc<Src>, i: usize, j: usize, end: usize) -> usize {
        let t: &str = &src.text;
        let k = skip_ws(t, j, end);
        let (cap, k) = match read_bracket(t, k, end) {
            Some((a, z, af)) => (Some((a, z)), af),
            None => (None, j),
        };
        let after = match read_arg(t, k, end) {
            Some((_, _, af)) => af,
            None => k,
        };
        if let (Some((a, z)), true) = (cap, self.float.is_some()) {
            let (kind, number) = {
                let f = self.float.as_mut();
                match f {
                    Some(f) => {
                        f.sub += 1;
                        let parent = f.number.clone().unwrap_or_default();
                        let letter = char::from_u32('a' as u32 + (f.sub - 1) % 26).unwrap_or('a');
                        (f.kind.clone(), format!("{parent}{letter}"))
                    }
                    None => return after,
                }
            };
            self.set_anchor(&kind, &number, "");
            self.flush();
            let raw = self.capture_range(src, a, z, true);
            let loc = self.loc(src, i, after);
            let sec = self.section_path();
            let mut b = Block::new(BlockKind::Caption, raw, &sec, loc);
            b.label = format!("{} {number}", title_case(&kind));
            self.cx.blocks.push(b);
            self.next_new_para = true;
        }
        after
    }

    // ----- displayed equations -----

    fn eq_number(&mut self) -> String {
        if let Some((parent, n)) = self.subeq.as_mut() {
            *n += 1;
            let l = char::from_u32('a' as u32 + (*n - 1) % 26).unwrap_or('a');
            return format!("{parent}{l}");
        }
        self.step_counter("equation")
    }

    /// A displayed equation or a group of them.
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_lines)] // one flat table of commands; not split
    pub(super) fn display(
        &mut self,
        src: &Rc<Src>,
        a: usize,
        b: usize,
        env: &str,
        starred: bool,
        la: usize,
        lb: usize,
    ) {
        if self.muted() {
            return;
        }
        let loc = self.loc(src, la, lb);
        let body = strip_comments(&src.text[a..b]);
        let body = self.expand_math(&body, 0);
        let multi = matches!(
            env,
            "align" | "gather" | "flalign" | "alignat" | "eqnarray" | "IEEEeqnarray" | "dgroup"
        );
        let numbered_env = !starred && !matches!(env, "displaymath");
        let mut rows: Vec<String> = if multi {
            split_rows(&body).into_iter().map(str::to_string).collect()
        } else if env == "multline" {
            vec![split_rows(&body).join(" ")]
        } else {
            vec![body.clone()]
        };
        let mut single_number = !multi;
        if !multi
            && rows.len() == 1
            && let Some(inner) = unwrap_aligned(&rows[0])
        {
            rows = split_rows(&inner).into_iter().map(str::to_string).collect();
            single_number = true;
        }
        let infos: Vec<RowInfo> = rows.iter().map(|r| analyze_row(r)).collect();
        let last_idx = infos
            .iter()
            .rposition(|r| !r.text.trim().is_empty() || r.intertext.is_some());
        let any_nonumber = infos.iter().any(|r| r.nonumber);
        if self.capturing > 0 {
            // inside a footnote or caption: speak the equation inline
            for r in &infos {
                let latex = r.text.trim();
                if latex.is_empty() {
                    continue;
                }
                let slot = defer(self.cx, latex, loc);
                let w = format!(" {MATH_OPEN}{slot}{MATH_CLOSE} ");
                self.text(src, la, lb, &w);
            }
            return;
        }
        let cont = !self.cur.is_blank() || self.cont_next;
        self.flush();
        let sec = self.section_path();
        let mut last_number = self.kind_counters.get("equation").map(|_| String::new());
        let _ = &mut last_number;
        for (idx, info) in infos.iter().enumerate() {
            if let Some(it) = &info.intertext {
                let s2 = Rc::new(Src {
                    text: it.clone(),
                    file: src.file,
                    over: Some(self.over_of(src, la, lb)),
                });
                let raw = self.capture(|lx| {
                    let n = s2.text.len();
                    lx.run(&s2, 0, n);
                });
                if !raw.is_blank() {
                    let mut blk = Block::new(BlockKind::Para, raw, &sec, loc);
                    blk.new_para = true;
                    self.cx.blocks.push(blk);
                }
                continue;
            }
            let latex = info.text.trim();
            if latex.is_empty() {
                continue;
            }
            // number
            let tag = info.tag.as_deref().and_then(clean_tag);
            let pending = if info.tag.is_some() || numbered_env {
                self.pending_eq.take()
            } else {
                None
            };
            let numbered = if let Some(tag) = tag {
                Some(tag)
            } else if let Some(p) = pending {
                Some(p)
            } else if info.tag.is_some() || !numbered_env {
                // a `\tag` replaces the number even when its text cannot be spoken, and takes no number from the counter
                None
            } else if single_number {
                if Some(idx) == last_idx && !any_nonumber {
                    Some(self.eq_number())
                } else {
                    None
                }
            } else if info.nonumber {
                None
            } else {
                Some(self.eq_number())
            };
            if let Some(n) = &numbered {
                self.set_anchor("equation", n, "");
            }
            for l in &info.labels {
                self.register_label(l);
            }
            let (body_latex, punct) = split_trailing_punct(latex);
            let slot = defer(self.cx, body_latex, loc);
            let mut raw = Raw::default();
            raw.push(&slot, loc);
            let mut blk = Block::new(BlockKind::Display, raw, &sec, loc);
            blk.punct = punct;
            blk.cont = cont || idx > 0;
            blk.new_para = !blk.cont;
            blk.number = numbered.map(|n| format!("equation {n}"));
            self.cx.blocks.push(blk);
        }
        self.cont_next = true;
        self.next_new_para = false;
        self.cur_new_para = false;
    }
}

/// The text of a `\tag`: letters, digits and punctuation only. A tag made of commands gives `None`.
fn clean_tag(tag: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = tag.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                while chars.peek().is_some_and(char::is_ascii_alphabetic) {
                    chars.next();
                }
            }
            '{' | '}' | '$' | '^' | '_' | '@' => {}
            c => out.push(c),
        }
    }
    let t = out
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim()
        .to_string();
    if t.is_empty() || t.contains(',') {
        None
    } else {
        Some(t)
    }
}

/// `\begin{aligned} ... \end{aligned}` filling the whole of `s`: its inside.
fn unwrap_aligned(s: &str) -> Option<String> {
    let s = s.trim();
    for env in ["aligned", "split", "gathered", "alignedat"] {
        let open = format!("\\begin{{{env}}}");
        let close = format!("\\end{{{env}}}");
        if let Some(rest) = s.strip_prefix(&open)
            && let Some(inner) = rest.strip_suffix(&close)
            && !inner.contains(&open)
        {
            let mut inner = inner.to_string();
            if env == "alignedat"
                && let Some((_, _, af)) =
                    read_braced(&inner, skip_ws(&inner, 0, inner.len()), inner.len())
            {
                inner = inner[af..].to_string();
            }
            return Some(inner);
        }
    }
    None
}

#[derive(Default)]
struct RowInfo {
    text: String,
    labels: Vec<String>,
    tag: Option<String>,
    nonumber: bool,
    intertext: Option<String>,
}

/// Take labels, tags and number switches out of a row, and drop its alignment marks.
fn analyze_row(row: &str) -> RowInfo {
    let mut info = RowInfo::default();
    let b = row.as_bytes();
    let mut out = String::with_capacity(row.len());
    let mut i = 0;
    let mut last = 0;
    let mut depth = 0i32;
    let mut env = 0i32;
    while i < b.len() {
        match b[i] {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            b'&' if depth <= 0 && env <= 0 => {
                out.push_str(&row[last..i]);
                out.push(' ');
                last = i + 1;
            }
            b'\\' => {
                let (name, j) = super::scan::control_name(row, i, row.len());
                match name {
                    "begin" => env += 1,
                    "end" => env -= 1,
                    "label" | "tag" | "intertext" | "shortintertext" => {
                        let mut k = j;
                        if row.as_bytes().get(k) == Some(&b'*') {
                            k += 1;
                        }
                        if let Some((a, z, af)) =
                            read_braced(row, skip_ws(row, k, row.len()), row.len())
                        {
                            out.push_str(&row[last..i]);
                            let inner = row[a..z].to_string();
                            match name {
                                "label" => info.labels.push(inner),
                                "tag" => info.tag = Some(inner),
                                _ => info.intertext = Some(inner),
                            }
                            i = af;
                            last = af;
                            continue;
                        }
                    }
                    "eqno" | "leqno" => {
                        out.push_str(&row[last..i]);
                        let tag = row[j..]
                            .trim()
                            .trim_start_matches('(')
                            .trim_end_matches(')')
                            .trim()
                            .to_string();
                        if !tag.is_empty() {
                            info.tag = Some(tag);
                        }
                        i = row.len();
                        last = row.len();
                        continue;
                    }
                    "nonumber" | "notag" => {
                        out.push_str(&row[last..i]);
                        info.nonumber = true;
                        i = j;
                        last = j;
                        continue;
                    }
                    "displaybreak" | "allowdisplaybreaks" | "qedhere" | "hphantom_" => {
                        out.push_str(&row[last..i]);
                        i = j;
                        last = j;
                        continue;
                    }
                    _ => {}
                }
                i = j.max(i + 1);
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    out.push_str(&row[last.min(row.len())..]);
    info.text = out;
    info
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_lose_labels_and_alignment() {
        let r = analyze_row(r"a &= b \label{x} \nonumber");
        assert_eq!(r.labels, ["x"]);
        assert!(r.nonumber);
        assert_eq!(
            r.text.split_whitespace().collect::<Vec<_>>().join(" "),
            "a = b"
        );
    }

    #[test]
    fn matrix_keeps_its_ampersands() {
        let r = analyze_row(r"\begin{pmatrix} a & b \end{pmatrix}");
        assert!(r.text.contains('&'));
    }

    #[test]
    fn aligned_unwraps() {
        assert_eq!(
            unwrap_aligned(r"\begin{aligned} a &= b \end{aligned}").as_deref(),
            Some(" a &= b ")
        );
        assert!(unwrap_aligned("x = 1").is_none());
    }
}
