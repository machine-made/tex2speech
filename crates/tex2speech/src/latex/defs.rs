// SPDX-License-Identifier: Apache-2.0
//! Definitions: `\newcommand`, `\def`, `\let`, `\newenvironment`, `\newtheorem` and the scan that
//! collects them from every position before the document is read.

use super::scan::{control_name, read_arg, read_braced, read_bracket, skip_comment, skip_ws};
use super::{EnvDef, Lx, Macro, Src, Thm};
use crate::raw::Loc;
use std::rc::Rc;

const DEF_NAMES: &[&str] = &[
    "newcommand",
    "renewcommand",
    "providecommand",
    "DeclareRobustCommand",
    "def",
    "gdef",
    "edef",
    "xdef",
    "let",
    "NewDocumentCommand",
    "RenewDocumentCommand",
    "DeclareDocumentCommand",
    "ProvideDocumentCommand",
    "DeclareMathOperator",
    "newenvironment",
    "renewenvironment",
    "NewDocumentEnvironment",
    "DeclareDocumentEnvironment",
    "newtheorem",
    "DeclarePairedDelimiter",
    "newif",
    "setcounter",
    "numberwithin",
];

fn cmd_name(s: &str) -> String {
    s.trim().trim_start_matches('\\').to_string()
}

fn level_of(name: &str) -> Option<usize> {
    Some(match name.trim() {
        "part" => 0,
        "chapter" => 1,
        "section" => 2,
        "subsection" => 3,
        "subsubsection" => 4,
        _ => return None,
    })
}

impl Lx<'_> {
    /// Read a definition command at `j` (just after its name). Returns the index after it.
    #[allow(clippy::too_many_lines)] // one flat table of commands; not split
    pub(super) fn parse_def(
        &mut self,
        src: &Rc<Src>,
        name: &str,
        star: bool,
        j: usize,
        end: usize,
        prescan: bool,
    ) -> usize {
        let t: &str = &src.text;
        let b = t.as_bytes();
        let loc = self.loc(src, j, j);
        match name {
            "newcommand" | "renewcommand" | "providecommand" | "DeclareRobustCommand" => {
                let Some((a, z, k)) = read_arg(t, j, end) else {
                    return j;
                };
                let cmd = cmd_name(&t[a..z]);
                let mut k = skip_ws(t, k, end);
                let mut nargs = 0;
                if let Some((a2, z2, after)) = read_bracket(t, k, end) {
                    nargs = t[a2..z2].trim().parse::<usize>().unwrap_or(0).min(9);
                    k = skip_ws(t, after, end);
                }
                let mut opt = None;
                if let Some((a2, z2, after)) = read_bracket(t, k, end) {
                    opt = Some(t[a2..z2].to_string());
                    k = after;
                }
                let Some((ba, bz, kend)) = read_arg(t, k, end) else {
                    return k;
                };
                self.define(
                    cmd,
                    Macro {
                        nargs,
                        opt,
                        body: t[ba..bz].to_string(),
                    },
                    prescan,
                    name == "providecommand",
                );
                kend
            }
            "def" | "gdef" | "edef" | "xdef" => {
                let Some((a, z, k)) = read_arg(t, j, end) else {
                    return j;
                };
                let cmd = cmd_name(&t[a..z]);
                // parameter text up to the opening brace
                let mut p = k;
                let mut nargs = 0;
                let mut delimited = false;
                while p < end && b[p] != b'{' && p - k < 200 {
                    if b[p] == b'#' {
                        nargs += 1;
                        p += 2;
                        continue;
                    }
                    if !b[p].is_ascii_whitespace() {
                        delimited = true;
                    }
                    p += 1;
                }
                // `#` skips a byte after it, and the 200-byte limit stops anywhere: land on a character boundary
                let mut p = p.min(end);
                while !t.is_char_boundary(p) {
                    p += 1;
                }
                let Some((ba, bz, kend)) = read_braced(t, p, end) else {
                    return p;
                };
                if delimited && !self.in_class && !cmd.contains('@') {
                    self.warn_once("def", &format!("\\{cmd} with delimited arguments"), loc);
                }
                self.define(
                    cmd,
                    Macro {
                        nargs: nargs.min(9),
                        opt: None,
                        body: t[ba..bz].to_string(),
                    },
                    prescan,
                    false,
                );
                kend
            }
            "let" => {
                let Some((a, z, mut k)) = read_arg(t, j, end) else {
                    return j;
                };
                let new = cmd_name(&t[a..z]);
                k = skip_ws(t, k, end);
                if b.get(k) == Some(&b'=') {
                    k += 1;
                }
                let Some((a2, z2, k2)) = read_arg(t, k, end) else {
                    return k;
                };
                let old = cmd_name(&t[a2..z2]);
                if self.frozen.contains(&new) {
                    // the first reading of the file has fixed what `\new` means
                    return k2;
                }
                if let Some(m) = self.macros.get(&old).cloned() {
                    self.define(new, m, prescan, false);
                } else if !prescan || !self.aliases.contains_key(&new) {
                    self.aliases.insert(new, format!("\\{old}"));
                }
                k2
            }
            "NewDocumentCommand"
            | "RenewDocumentCommand"
            | "DeclareDocumentCommand"
            | "ProvideDocumentCommand" => {
                let Some((a, z, k)) = read_arg(t, j, end) else {
                    return j;
                };
                let cmd = cmd_name(&t[a..z]);
                let Some((sa, sz, k)) = read_arg(t, k, end) else {
                    return k;
                };
                let spec = &t[sa..sz];
                let opt_first = spec.trim_start().starts_with('O');
                let nargs = spec
                    .chars()
                    .filter(|c| matches!(c, 'm' | 'o' | 'O'))
                    .count()
                    .min(9);
                let opt = if opt_first {
                    let rest = spec.trim_start()[1..].trim_start();
                    read_braced(rest, 0, rest.len())
                        .map(|(a, z, _)| rest[a..z].to_string())
                        .or(Some(String::new()))
                } else {
                    None
                };
                let Some((ba, bz, kend)) = read_arg(t, k, end) else {
                    return k;
                };
                self.define(
                    cmd,
                    Macro {
                        nargs,
                        opt,
                        body: t[ba..bz].to_string(),
                    },
                    prescan,
                    name.starts_with("Provide"),
                );
                kend
            }
            "DeclareMathOperator" => {
                let Some((a, z, k)) = read_arg(t, j, end) else {
                    return j;
                };
                let cmd = cmd_name(&t[a..z]);
                let Some((ba, bz, kend)) = read_arg(t, k, end) else {
                    return k;
                };
                let word = &t[ba..bz];
                let body = format!("\\operatorname{{{word}}}");
                self.define(
                    cmd,
                    Macro {
                        nargs: 0,
                        opt: None,
                        body,
                    },
                    prescan,
                    false,
                );
                let _ = star;
                kend
            }
            "DeclarePairedDelimiter" => {
                let Some((a, z, k)) = read_arg(t, j, end) else {
                    return j;
                };
                let cmd = cmd_name(&t[a..z]);
                let (la, lz, k) = read_arg(t, k, end).unwrap_or((k, k, k));
                let (ra, rz, k) = read_arg(t, k, end).unwrap_or((k, k, k));
                let body = format!("{} #1 {}", &t[la..lz], &t[ra..rz]);
                self.define(
                    cmd,
                    Macro {
                        nargs: 1,
                        opt: None,
                        body,
                    },
                    prescan,
                    false,
                );
                k
            }
            "newenvironment"
            | "renewenvironment"
            | "NewDocumentEnvironment"
            | "DeclareDocumentEnvironment" => {
                let Some((a, z, mut k)) = read_arg(t, j, end) else {
                    return j;
                };
                let env = t[a..z].trim().to_string();
                let mut nargs = 0;
                let mut opt = None;
                if name.contains("Document") {
                    if let Some((sa, sz, k2)) = read_arg(t, k, end) {
                        nargs = t[sa..sz]
                            .chars()
                            .filter(|c| matches!(c, 'm' | 'o' | 'O'))
                            .count()
                            .min(9);
                        k = k2;
                    }
                } else {
                    k = skip_ws(t, k, end);
                    if let Some((a2, z2, after)) = read_bracket(t, k, end) {
                        nargs = t[a2..z2].trim().parse::<usize>().unwrap_or(0).min(9);
                        k = skip_ws(t, after, end);
                        if let Some((a3, z3, after3)) = read_bracket(t, k, end) {
                            opt = Some(t[a3..z3].to_string());
                            k = after3;
                        }
                    }
                }
                let Some((ba, bz, k)) = read_arg(t, k, end) else {
                    return k;
                };
                let Some((ea, ez, k)) = read_arg(t, k, end) else {
                    return k;
                };
                let def = EnvDef {
                    nargs,
                    opt,
                    begin: t[ba..bz].to_string(),
                    end: t[ea..ez].to_string(),
                };
                // TeX internals (`\@ifstar`, `\global`, `\halign`) cannot be read as text: the environment keeps
                // its built-in reading when it has one, and is otherwise read as plain text
                if !(low_level(&def.begin)
                    || low_level(&def.end)
                    || (prescan && self.envs.contains_key(&env)))
                {
                    self.envs.insert(env, def);
                }
                k
            }
            "newtheorem" => {
                let Some((a, z, mut k)) = read_arg(t, j, end) else {
                    return j;
                };
                let env = t[a..z].trim().to_string();
                k = skip_ws(t, k, end);
                let mut shared = None;
                if let Some((a2, z2, after)) = read_bracket(t, k, end) {
                    shared = Some(t[a2..z2].trim().to_string());
                    k = after;
                }
                let Some((ta, tz, mut k)) = read_arg(t, k, end) else {
                    return k;
                };
                let title = t[ta..tz].to_string();
                k = skip_ws(t, k, end);
                let mut within = None;
                if let Some((a3, z3, after)) = read_bracket(t, k, end) {
                    within = level_of(&t[a3..z3]);
                    k = after;
                }
                let counter = shared.map_or_else(
                    || env.clone(),
                    |s| self.thms.get(&s).map_or(s.clone(), |th| th.counter.clone()),
                );
                if let Some(l) = within {
                    self.within.insert(format!("thm:{counter}"), l);
                }
                self.thms.insert(
                    env,
                    Thm {
                        title,
                        counter,
                        numbered: !star,
                    },
                );
                k
            }
            "newif" => {
                if let Some((a, z, k)) = read_arg(t, j, end) {
                    self.flags.entry(cmd_name(&t[a..z])).or_insert(false);
                    return k;
                }
                j
            }
            "setcounter" => {
                let Some((a, z, k)) = read_arg(t, j, end) else {
                    return j;
                };
                let Some((va, vz, k)) = read_arg(t, k, end) else {
                    return k;
                };
                if t[a..z].trim() == "secnumdepth"
                    && let Ok(n) = t[va..vz].trim().parse::<i32>()
                {
                    self.secnumdepth = n;
                }
                k
            }
            "numberwithin" => {
                let Some((a, z, k)) = read_arg(t, j, end) else {
                    return j;
                };
                let Some((la, lz, k)) = read_arg(t, k, end) else {
                    return k;
                };
                if let Some(l) = level_of(&t[la..lz]) {
                    self.within.insert(t[a..z].trim().to_string(), l);
                }
                k
            }
            _ => j,
        }
    }

    fn define(&mut self, name: String, m: Macro, prescan: bool, only_if_new: bool) {
        if name.is_empty()
            || ((self.in_class || super::PROTECTED.contains(&name.as_str()) || low_level(&m.body))
                && super::is_builtin(&name))
        {
            return;
        }
        if (prescan || only_if_new) && self.macros.contains_key(&name) {
            return;
        }
        // `\renewcommand{\Re}{\mathrm{Re}}` only changes the typeface: the built-in real part says it better than
        // the name of a lexicon term ("Reynolds number")
        if matches!(name.as_str(), "Re" | "Im") && is_part_name(&name, &m.body) {
            return;
        }
        if super::is_builtin(&name) {
            let target = format!("\\{name}");
            for (alias, to) in &self.aliases {
                if *to == target {
                    self.frozen.insert(alias.clone());
                }
            }
        }
        self.aliases.remove(&name);
        self.macros.insert(name, m);
    }

    /// Collect the definitions from every position in the file and the files it includes.
    pub(super) fn prescan(&mut self, src: &Rc<Src>, depth: usize) {
        // A second scan of the same file finds nothing new (the first definition wins), unless the first reached
        // it from so deep that its own includes were cut off.
        let seen = self
            .scanned
            .entry((src.file, self.in_class))
            .or_insert(usize::MAX);
        if *seen <= depth {
            return;
        }
        *seen = depth;
        let t: &str = &src.text;
        let b = t.as_bytes();
        let end = t.len();
        let mut i = 0;
        while i < end {
            match b[i] {
                b'%' => i = skip_comment(t, i, end),
                b'\\' => {
                    let (name, mut j) = control_name(t, i, end);
                    if name == "verb" {
                        // skip the delimited text
                        if let Some(d) = t[j..].chars().next() {
                            let from = j + d.len_utf8();
                            j = t[from..].find(d).map_or(end, |p| from + p + d.len_utf8());
                        }
                        i = j;
                        continue;
                    }
                    if DEF_NAMES.contains(&name) {
                        let star = b.get(j) == Some(&b'*');
                        if star {
                            j += 1;
                        }
                        i = self.parse_def(src, name, star, j, end, true).max(j);
                        continue;
                    }
                    if matches!(name, "input" | "include" | "subfile")
                        && depth < 8
                        && let Some((a, z, after)) = read_arg(t, j, end)
                    {
                        if let Some(path) = self.resolve_for_scan(&t[a..z], src)
                            && let Some(inc) = self.load(&path)
                        {
                            let key = path.canonicalize().unwrap_or(path);
                            if !self.stack.contains(&key) {
                                self.stack.push(key);
                                self.prescan(&inc, depth + 1);
                                self.stack.pop();
                            }
                        }
                        i = after;
                        continue;
                    }
                    i = j.max(i + 1);
                }
                _ => i += 1,
            }
        }
    }

    fn resolve_for_scan(&self, name: &str, from: &Src) -> Option<std::path::PathBuf> {
        match self.resolve(name, from) {
            super::Resolved::Found(p) => Some(p),
            _ => None,
        }
    }

    /// Replace the arguments of a macro use in its body. Returns the new text and the index after the use.
    #[allow(clippy::unused_self)] // keeps the same call shape as its sibling methods
    pub(super) fn apply_macro(
        &mut self,
        m: &Macro,
        t: &str,
        j: usize,
        end: usize,
    ) -> (String, usize) {
        let mut k = j;
        let mut args: Vec<String> = Vec::new();
        let mut n = m.nargs;
        if let Some(def) = &m.opt {
            n = n.saturating_sub(1);
            let k2 = skip_ws(t, k, end);
            match read_bracket(t, k2, end) {
                Some((a, z, after)) => {
                    args.push(t[a..z].to_string());
                    k = after;
                }
                None => args.push(def.clone()),
            }
        }
        for _ in 0..n {
            match read_arg(t, k, end) {
                Some((a, z, after)) => {
                    args.push(t[a..z].to_string());
                    k = after;
                }
                None => args.push(String::new()),
            }
        }
        (substitute(&m.body, &args), k)
    }

    /// Expand user macros inside maths (the maths library sees no definitions).
    pub(super) fn expand_math(&mut self, s: &str, depth: u32) -> String {
        // spacing commands inside an equation say nothing, and a `\renewcommand{\arraystretch}` there would
        // otherwise define a macro that rewrites the equation
        let stripped;
        let s = if depth == 0
            && (s.contains("\\setlength")
                || s.contains("\\addtolength")
                || s.contains("\\renewcommand"))
        {
            stripped = crate::math::strip_layout(s);
            stripped.as_str()
        } else {
            s
        };
        if (self.macros.is_empty() && self.aliases.is_empty()) || depth > 24 || !s.contains('\\') {
            return s.to_string();
        }
        let b = s.as_bytes();
        let mut out = String::with_capacity(s.len());
        let mut i = 0;
        let mut last = 0;
        while i < b.len() {
            if b[i] != b'\\' {
                i += 1;
                continue;
            }
            let (name, j) = control_name(s, i, s.len());
            let Some(m) = self.macros.get(name).cloned().or_else(|| {
                self.aliases.get(name).map(|a| Macro {
                    nargs: 0,
                    opt: None,
                    body: a.clone(),
                })
            }) else {
                i = j.max(i + 1);
                continue;
            };
            self.expansions += 1;
            if self.expansions > super::MAX_EXPANSIONS {
                break;
            }
            out.push_str(&s[last..i]);
            let (body, after) = self.apply_macro(&m, s, j, s.len());
            if !self.charge(body.len(), Loc::default()) {
                break;
            }
            out.push_str(&self.expand_math(&body, depth + 1));
            i = after.max(j);
            last = i;
        }
        out.push_str(&s[last.min(s.len())..]);
        out
    }
}

/// Whether a body is only the letters `Re` or `Im` in some wrapper, as `\mathrm{Re}`.
fn is_part_name(name: &str, body: &str) -> bool {
    let body: String = body.chars().filter(|c| !c.is_whitespace()).collect();
    let body = body.trim_end_matches("\\,").trim_end_matches("\\;");
    let inner = [
        "mathrm",
        "mathit",
        "mathsf",
        "operatorname",
        "text",
        "textrm",
        "mbox",
    ]
    .iter()
    .find_map(|w| body.strip_prefix(&format!("\\{w}{{")))
    .and_then(|r| r.strip_suffix('}'))
    .unwrap_or(body);
    inner == name
}

fn substitute(body: &str, args: &[String]) -> String {
    if !body.contains('#') {
        return body.to_string();
    }
    let mut out = String::with_capacity(body.len() + 16);
    let mut it = body.chars().peekable();
    while let Some(c) = it.next() {
        if c != '#' {
            out.push(c);
            continue;
        }
        match it.peek().copied() {
            Some('#') => {
                it.next();
                out.push('#');
            }
            Some(d) if d.is_ascii_digit() && d != '0' => {
                it.next();
                let idx = d as usize - '1' as usize;
                if let Some(a) = args.get(idx) {
                    // `\lVert#1` with #1 = A must stay two tokens
                    let after_word = {
                        let letters = out
                            .chars()
                            .rev()
                            .take_while(char::is_ascii_alphabetic)
                            .count();
                        letters > 0
                            && out[..out.len() - letters].ends_with('\\')
                            && !out[..out.len() - letters].ends_with("\\\\")
                    };
                    if after_word && a.starts_with(|c: char| c.is_ascii_alphabetic()) {
                        out.push(' ');
                    }
                    out.push_str(a);
                }
            }
            _ => out.push('#'),
        }
    }
    out
}

/// True when the code uses TeX internals: a control word with `@`, `\global`, `\halign`, `\everycr` or `$$`.
fn low_level(code: &str) -> bool {
    code.contains("$$")
        || code.contains("\\global")
        || code.contains("\\halign")
        || code.contains("\\everycr")
        || code.match_indices('\\').any(|(i, _)| {
            code[i + 1..]
                .chars()
                .take_while(|c| c.is_ascii_alphabetic() || *c == '@')
                .any(|c| c == '@')
        })
}

#[cfg(test)]
mod tests {
    use super::substitute;

    #[test]
    fn substitutes_arguments() {
        assert_eq!(
            substitute(r"\mathbf{#1}_{#2}", &["u".into(), "i".into()]),
            r"\mathbf{u}_{i}"
        );
        assert_eq!(substitute("##1", &[]), "#1");
    }
}
