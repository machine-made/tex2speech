// SPDX-License-Identifier: Apache-2.0
//! The Markdown reader, built on `pulldown-cmark`.

use crate::Options;
use crate::math::{defer, speak_code, split_trailing_punct};
use crate::raw::{Block, BlockKind, Ctx, FileInfo, Hole, Loc, MATH_CLOSE, MATH_OPEN, Raw};
use pulldown_cmark::{
    CodeBlockKind, Event, HeadingLevel, LinkType, Options as MdOptions, Parser, Tag, TagEnd,
};

const LEVEL_NAMES: [&str; 6] = [
    "Chapter",
    "Section",
    "Subsection",
    "Subsubsection",
    "Paragraph",
    "Subparagraph",
];

/// Replace `\(..\)` and `\[..\]` by `$..$` and `$$..$$` without changing any byte offset.
fn prepare_math_delimiters(src: &str) -> String {
    let b = src.as_bytes();
    let mut out = b.to_vec();
    let mut i = 0;
    let mut line_start = true;
    while i < b.len() {
        let c = b[i];
        if line_start && (src[i..].starts_with("```") || src[i..].starts_with("~~~")) {
            // the closing line has at least as many of the same character as the opening one
            let ch = b[i];
            let open_len = b[i..].iter().take_while(|c| **c == ch).count();
            let mut j = i + open_len;
            let mut found = b.len();
            while j < b.len() {
                if b[j] == b'\n' {
                    let after = src[j + 1..].trim_start_matches(' ');
                    let run = after.bytes().take_while(|c| *c == ch).count();
                    if run >= open_len {
                        found = b.len() - after.len() + run;
                        break;
                    }
                }
                j += 1;
            }
            i = found;
            line_start = false;
            continue;
        }
        line_start = c == b'\n';
        match c {
            b'`' => {
                let mut n = 0;
                while i + n < b.len() && b[i + n] == b'`' {
                    n += 1;
                }
                let ticks = &src[i..i + n];
                match src[i + n..].find(ticks) {
                    Some(p) => i += n + p + n,
                    None => i += n,
                }
            }
            b'\\' if i + 1 < b.len() => {
                match b[i + 1] {
                    b'(' => {
                        out[i] = b' ';
                        out[i + 1] = b'$';
                    }
                    b')' => {
                        out[i] = b'$';
                        out[i + 1] = b' ';
                    }
                    b'[' | b']' => {
                        out[i] = b'$';
                        out[i + 1] = b'$';
                    }
                    _ => {}
                }
                i += 2;
            }
            _ => i += 1,
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| src.to_string())
}

/// Text of an HTML block without tags and comments.
fn strip_html(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(a) = rest.find('<') {
        out.push_str(&rest[..a]);
        let after = &rest[a..];
        if let Some(r) = after.strip_prefix("<!--") {
            rest = r.find("-->").map_or("", |e| &r[e + 3..]);
        } else {
            let lower = after.to_ascii_lowercase();
            let skip_to = ["script", "style"]
                .iter()
                .find(|t| lower.starts_with(&format!("<{t}")))
                .and_then(|t| lower.find(&format!("</{t}>")).map(|e| e + t.len() + 3));
            match skip_to {
                Some(e) => rest = &after[e..],
                None => rest = after.find('>').map_or("", |e| &after[e + 1..]),
            }
        }
    }
    out.push_str(rest);
    out
}

fn leading_number(text: &str) -> Option<(String, &str)> {
    let t = text.trim_start();
    let end = t
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(t.len());
    let num = t[..end].trim_end_matches('.');
    if num.is_empty() || !num.starts_with(|c: char| c.is_ascii_digit()) || num.contains("..") {
        return None;
    }
    let rest = &t[end..];
    if !(rest.is_empty() || rest.starts_with(char::is_whitespace)) {
        return None;
    }
    Some((num.to_string(), rest.trim_start()))
}

#[allow(clippy::struct_excessive_bools)] // independent parser flags
struct Reader<'a> {
    cx: &'a mut Ctx,
    file: u32,
    cur: Raw,
    cur_new_para: bool,
    next_new_para: bool,
    cur_label: String,
    pending_quote: bool,
    counters: [u32; 6],
    single_title: bool,
    skip: u32,
    heading: Option<HeadingLevel>,
    image: Option<Raw>,
    footnote_def: Option<String>,
    autolink: bool,
    html: Option<String>,
    in_metadata: bool,
}

impl Reader<'_> {
    fn loc(&self, r: &std::ops::Range<usize>) -> Loc {
        Loc {
            file: self.file,
            start: r.start,
            end: r.end,
        }
    }

    fn section(&self) -> Vec<u32> {
        let n = self
            .counters
            .iter()
            .rposition(|c| *c > 0)
            .map_or(0, |p| p + 1);
        self.counters[..n].to_vec()
    }

    fn flush(&mut self) {
        if self.cur.is_blank() {
            self.cur = Raw::default();
            self.cur_label.clear();
            return;
        }
        let raw = std::mem::take(&mut self.cur);
        if self.footnote_def.is_some() {
            // footnote text is collected by the End(FootnoteDefinition) event
            self.cur = raw;
            self.cur.push_separator(self.file);
            return;
        }
        let loc = raw.loc_of(0, raw.text.len()).unwrap_or_default();
        let sec = self.section();
        let mut b = Block::new(BlockKind::Para, raw, &sec, loc);
        b.new_para = self.cur_new_para;
        b.label = std::mem::take(&mut self.cur_label);
        self.cx.blocks.push(b);
        self.next_new_para = false;
    }

    fn start_block(&mut self, new_para: bool) {
        self.flush();
        self.cur_new_para = new_para;
    }

    fn push_text(&mut self, s: &str, r: &std::ops::Range<usize>) {
        if self.skip > 0 || self.autolink {
            return;
        }
        let loc = self.loc(r);
        if let Some(img) = self.image.as_mut() {
            img.push(s, loc);
            return;
        }
        if self.cur.is_blank() && !s.trim().is_empty() && self.cur.text.is_empty() {
            self.cur_new_para = self.next_new_para;
        }
        self.cur.push(s, loc);
    }

    /// Text of a paragraph: a run of symbols around a maths command outside `$` is read as an equation.
    fn push_prose(&mut self, s: &str, r: &std::ops::Range<usize>) {
        let mut done = 0;
        let mut i = 0;
        while i < s.len() {
            if matches!(s.as_bytes()[i], b'\\' | b'(' | b'[' | b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z')
                && crate::loose::at_word_start(s, i)
                && let Some(z) = crate::loose::run_at(s, i, s.len())
            {
                self.push_text(&s[done..i], r);
                self.math_inline(&s[i..z], r);
                done = z;
                i = z;
            } else {
                i += s[i..].chars().next().map_or(1, char::len_utf8);
            }
        }
        self.push_text(&s[done..], r);
    }

    fn math_inline(&mut self, latex: &str, r: &std::ops::Range<usize>) {
        if self.skip > 0 {
            return;
        }
        let loc = self.loc(r);
        let slot = defer(self.cx, latex, loc);
        let wrapped = format!("{MATH_OPEN}{slot}{MATH_CLOSE}");
        self.push_text(&wrapped, r);
    }

    fn math_display(&mut self, latex: &str, r: &std::ops::Range<usize>) {
        if self.skip > 0 {
            return;
        }
        let new_para = self.cur.is_blank() && self.next_new_para;
        self.flush();
        let loc = self.loc(r);
        let (body, punct) = split_trailing_punct(latex);
        let slot = defer(self.cx, body, loc);
        let mut raw = Raw::default();
        raw.push(&slot, loc);
        let sec = self.section();
        let mut b = Block::new(BlockKind::Display, raw, &sec, loc);
        b.punct = punct;
        b.new_para = new_para;
        self.cx.blocks.push(b);
        self.next_new_para = false;
        self.cur_new_para = false;
    }

    fn heading(&mut self, level: HeadingLevel, r: &std::ops::Range<usize>) {
        let raw = std::mem::take(&mut self.cur);
        let lvl = level as usize; // H1 = 1
        let loc = raw.loc_of(0, raw.text.len()).unwrap_or(self.loc(r));
        let sec;
        let mut b;
        // A single `#` is the title, unless its text is a number or starts with one: that is a chapter.
        if lvl == 1 && self.single_title && leading_number(&raw.text).is_none() {
            sec = self.section();
            b = Block::new(BlockKind::Title, raw, &sec, loc);
        } else {
            let idx = lvl - 1;
            let name = LEVEL_NAMES[idx];
            let text = raw.text.clone();
            let (label, mut title) = if let Some((num, rest)) = leading_number(&text) {
                let last = num
                    .rsplit('.')
                    .next()
                    .and_then(|x| x.parse::<u32>().ok())
                    .unwrap_or(self.counters[idx] + 1);
                self.counters[idx] = last;
                (format!("{name} {num}"), Some(rest.to_string()))
            } else {
                self.counters[idx] += 1;
                (format!("{name} {}", self.counters[idx]), None)
            };
            for c in &mut self.counters[idx + 1..] {
                *c = 0;
            }
            let mut body = raw;
            if let Some(rest) = title.take() {
                // drop the number from the start of the spoken text
                let cut = text.len() - rest.len();
                let mut r2 = Raw {
                    text: body.text[cut..].to_string(),
                    ..Raw::default()
                };
                r2.marks = body
                    .marks
                    .iter()
                    .filter(|m| m.end > cut)
                    .map(|m| crate::raw::Mark {
                        off: m.off.saturating_sub(cut),
                        end: m.end - cut,
                        loc: m.loc,
                    })
                    .collect();
                body = r2;
            }
            sec = self.section();
            b = Block::new(BlockKind::Heading, body, &sec, loc);
            b.label = label;
        }
        self.cx.blocks.push(b);
        self.next_new_para = true;
    }

    fn event(&mut self, ev: Event<'_>, r: std::ops::Range<usize>) {
        match ev {
            Event::Start(tag) => self.start(tag, &r),
            Event::End(tag) => self.end(tag, &r),
            Event::Text(s) => {
                if self.in_metadata {
                    return;
                }
                if let Some(h) = self.html.as_mut() {
                    h.push_str(&s);
                    return;
                }
                self.push_prose(&s, &r);
            }
            Event::Code(s) => {
                let t = speak_code(&s);
                self.push_text(&format!(" {t} "), &r);
            }
            Event::InlineMath(s) => self.math_inline(&s, &r),
            Event::DisplayMath(s) => {
                if self.heading.is_some() || self.image.is_some() {
                    self.math_inline(&s, &r);
                } else {
                    self.math_display(&s, &r);
                }
            }
            Event::Html(s) | Event::InlineHtml(s) => {
                if let Some(h) = self.html.as_mut() {
                    h.push_str(&s);
                } else {
                    // inline tags carry no words; comments are hidden
                    let _ = s;
                }
            }
            Event::FootnoteReference(label) => {
                if self.skip == 0 {
                    let h = self.cx.hole(Hole::Footnote {
                        raw: None,
                        label: Some(label.to_string()),
                    });
                    let off = self.cur.text.len();
                    self.cur.notes.push((off, h));
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                let loc = self.loc(&r);
                if self.skip == 0 {
                    match self.image.as_mut() {
                        Some(img) => img.push_space(loc),
                        None => self.cur.push_space(loc),
                    }
                }
            }
            Event::Rule => {}
            Event::TaskListMarker(done) => {
                self.cur_label = if done { "Done" } else { "To do" }.to_string();
            }
        }
    }

    fn start(&mut self, tag: Tag<'_>, r: &std::ops::Range<usize>) {
        match tag {
            Tag::Paragraph => {
                if self.skip == 0 {
                    let np = self.next_new_para;
                    self.start_block(np);
                    if self.pending_quote {
                        self.cur_label = "Quote".to_string();
                        self.pending_quote = false;
                    }
                }
            }
            Tag::Heading { level, .. } => {
                self.flush();
                self.heading = Some(level);
            }
            Tag::BlockQuote(_) => {
                self.flush();
                self.pending_quote = true;
                self.next_new_para = true;
            }
            Tag::CodeBlock(kind) => {
                let _ = matches!(kind, CodeBlockKind::Fenced(_));
                self.skip += 1;
            }
            Tag::HtmlBlock => {
                self.flush();
                self.html = Some(String::new());
            }
            Tag::List(_) | Tag::DefinitionList => {
                self.flush();
                self.next_new_para = true;
            }
            Tag::Item | Tag::DefinitionListTitle | Tag::DefinitionListDefinition => {
                self.flush();
                self.next_new_para = false;
                self.cur_new_para = false;
                if self.pending_quote {
                    self.cur_label = "Quote".to_string();
                    self.pending_quote = false;
                }
            }
            Tag::FootnoteDefinition(label) => {
                self.flush();
                self.footnote_def = Some(label.to_string());
                self.cur = Raw::default();
            }
            Tag::Table(_) => {
                self.flush();
                self.skip += 1;
            }
            Tag::Link { link_type, .. } => {
                if matches!(link_type, LinkType::Autolink | LinkType::Email) {
                    self.autolink = true;
                }
            }
            Tag::Image { .. } => {
                if self.skip == 0 {
                    self.flush();
                    self.image = Some(Raw::default());
                }
            }
            Tag::MetadataBlock(_) => self.in_metadata = true,
            _ => {}
        }
        let _ = r;
    }

    fn end(&mut self, tag: TagEnd, r: &std::ops::Range<usize>) {
        match tag {
            TagEnd::Paragraph => {
                if self.skip == 0 {
                    self.flush();
                    self.next_new_para = true;
                }
            }
            TagEnd::Heading(level) => {
                self.heading = None;
                if self.cur.is_blank() {
                    self.cur = Raw::default();
                } else {
                    self.heading(level, r);
                }
            }
            TagEnd::BlockQuote(_) => {
                self.flush();
                self.pending_quote = false;
                self.next_new_para = true;
            }
            TagEnd::CodeBlock => self.skip = self.skip.saturating_sub(1),
            TagEnd::HtmlBlock => {
                if let Some(h) = self.html.take() {
                    let text = strip_html(&h);
                    let loc = self.loc(r);
                    let t = text.split_whitespace().collect::<Vec<_>>().join(" ");
                    if !t.is_empty() {
                        let np = self.next_new_para;
                        self.start_block(np);
                        self.cur.push(&t, loc);
                        self.flush();
                        self.next_new_para = true;
                    }
                }
            }
            TagEnd::List(_) | TagEnd::DefinitionList => {
                self.flush();
                self.next_new_para = true;
            }
            TagEnd::Item | TagEnd::DefinitionListTitle | TagEnd::DefinitionListDefinition => {
                self.flush();
                self.next_new_para = false;
            }
            TagEnd::FootnoteDefinition => {
                if let Some(label) = self.footnote_def.take() {
                    let raw = std::mem::take(&mut self.cur);
                    self.cx.footnote_defs.insert(label, raw);
                }
            }
            TagEnd::Table => {
                self.skip = self.skip.saturating_sub(1);
                self.next_new_para = true;
            }
            TagEnd::Link => self.autolink = false,
            TagEnd::Image => {
                if let Some(alt) = self.image.take() {
                    if !alt.is_blank() {
                        let loc = alt.loc_of(0, alt.text.len()).unwrap_or(self.loc(r));
                        let sec = self.section();
                        let mut b = Block::new(BlockKind::Caption, alt, &sec, loc);
                        b.label = "Figure".to_string();
                        self.cx.blocks.push(b);
                    }
                    self.next_new_para = false;
                    self.cur_new_para = false;
                }
            }
            TagEnd::MetadataBlock(_) => self.in_metadata = false,
            _ => {}
        }
    }
}

pub(crate) fn read(src: &str, name: &str, opts: &Options, cx: &mut Ctx) {
    let _ = opts;
    let file = u32::try_from(cx.files.len()).unwrap_or(u32::MAX);
    cx.files.push(FileInfo::new(name, src));
    let prepared = prepare_math_delimiters(src);
    let mut o = MdOptions::empty();
    o.insert(MdOptions::ENABLE_TABLES);
    o.insert(MdOptions::ENABLE_FOOTNOTES);
    o.insert(MdOptions::ENABLE_STRIKETHROUGH);
    o.insert(MdOptions::ENABLE_TASKLISTS);
    o.insert(MdOptions::ENABLE_MATH);
    o.insert(MdOptions::ENABLE_YAML_STYLE_METADATA_BLOCKS);
    o.insert(MdOptions::ENABLE_DEFINITION_LIST);
    o.insert(MdOptions::ENABLE_HEADING_ATTRIBUTES);
    let events: Vec<(Event<'_>, std::ops::Range<usize>)> =
        Parser::new_ext(&prepared, o).into_offset_iter().collect();
    let h1 = events
        .iter()
        .filter(|(e, _)| {
            matches!(
                e,
                Event::Start(Tag::Heading {
                    level: HeadingLevel::H1,
                    ..
                })
            )
        })
        .count();
    let mut rd = Reader {
        cx,
        file,
        cur: Raw::default(),
        cur_new_para: true,
        next_new_para: true,
        cur_label: String::new(),
        pending_quote: false,
        counters: [0; 6],
        single_title: h1 == 1,
        skip: 0,
        heading: None,
        image: None,
        footnote_def: None,
        autolink: false,
        html: None,
        in_metadata: false,
    };
    for (ev, r) in events {
        rd.event(ev, r);
    }
    rd.flush();
}
