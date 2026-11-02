use math_core::{LatexToMathML, MathCoreConfig, MathDisplay};
use std::collections::{HashMap, VecDeque};
use std::fmt::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Condvar, Mutex, OnceLock};

/// Opens a reference to an equation that is read at the end of the document.
pub(crate) const SLOT_OPEN: char = '\u{e020}';
/// Closes a reference to an equation.
pub(crate) const SLOT_CLOSE: char = '\u{e021}';

/// Equations met while reading. Each distinct equation is converted once, at the end, on as many threads as
/// the amount of work justifies.
#[derive(Default)]
pub(crate) struct MathBatch {
    pub latex: Vec<String>,
    pub locs: Vec<crate::raw::Loc>,
    index: HashMap<String, usize>,
}

/// Note an equation and return the slot that stands for its words in raw text.
pub(crate) fn defer(cx: &mut crate::raw::Ctx, latex: &str, loc: crate::raw::Loc) -> String {
    let b = &mut cx.math;
    let idx = if let Some(i) = b.index.get(latex) {
        *i
    } else {
        b.latex.push(latex.to_string());
        b.locs.push(loc);
        b.index.insert(latex.to_string(), b.latex.len() - 1);
        b.latex.len() - 1
    };
    format!("{SLOT_OPEN}{idx}{SLOT_CLOSE}")
}

struct Job {
    id: usize,
    mathml: String,
    reply: Sender<(usize, Result<String, String>)>,
}

/// The queue shared by the `MathCAT` threads. `MathCAT` keeps its state in thread-local storage, so every
/// thread loads the rules itself, at its first equation.
struct Pool {
    queue: Mutex<VecDeque<Job>>,
    ready: Condvar,
    threads: Mutex<usize>,
}

static POOL: OnceLock<Arc<Pool>> = OnceLock::new();

fn worker_loop(pool: &Pool) {
    use tex2speech_mathcat as mc;
    let mut ready: Option<Result<(), String>> = None;
    loop {
        let job = {
            let mut q = pool
                .queue
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            loop {
                if let Some(j) = q.pop_front() {
                    break j;
                }
                q = pool
                    .ready
                    .wait(q)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
        };
        let init = ready.get_or_insert_with(|| {
            let run = || -> Result<(), String> {
                let e = |e: tex2speech_mathcat::errors::Error| mc::errors_to_string(&e);
                mc::set_rules_dir("Rules").map_err(e)?;
                mc::set_preference("Language", "en").map_err(e)?;
                mc::set_preference("SpeechStyle", "ClearSpeak").map_err(e)?;
                mc::set_preference("TTS", "None").map_err(e)?;
                Ok(())
            };
            catch_unwind(AssertUnwindSafe(run))
                .unwrap_or_else(|_| Err("MathCAT panicked while loading".into()))
        });
        let result = match init {
            Err(e) => Err(e.clone()),
            Ok(()) => catch_unwind(AssertUnwindSafe(|| {
                mc::set_mathml(&job.mathml)
                    .and_then(|_| mc::get_spoken_text())
                    .map_err(|e| mc::errors_to_string(&e))
            }))
            .unwrap_or_else(|_| Err("MathCAT panicked".into())),
        };
        let _ = job.reply.send((job.id, result));
    }
}

/// Equations per thread worth the cost of loading the rules once more.
const JOBS_PER_THREAD: usize = 80;
const MAX_THREADS: usize = 8;

/// Convert `MathML` to words for all `mathml` strings, in the order given.
fn speak_all(mathml: &[String], threads: Option<usize>) -> Vec<Result<String, String>> {
    let n = mathml.len();
    if n == 0 {
        return Vec::new();
    }
    let pool = POOL
        .get_or_init(|| {
            Arc::new(Pool {
                queue: Mutex::new(VecDeque::new()),
                ready: Condvar::new(),
                threads: Mutex::new(0),
            })
        })
        .clone();
    let cores = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let want = threads
        .unwrap_or_else(|| (n / JOBS_PER_THREAD + 1).min(cores))
        .clamp(1, MAX_THREADS);
    {
        let mut have = pool
            .threads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while *have < want {
            let p = pool.clone();
            if std::thread::Builder::new()
                .name("tex2speech-mathcat".into())
                .spawn(move || worker_loop(&p))
                .is_err()
            {
                break;
            }
            *have += 1;
        }
        if *have == 0 {
            return vec![Err("no thread for equations".to_string()); n];
        }
    }
    let (reply_tx, results_rx) = channel();
    // longest first, so that one heavy equation does not end up alone at the back
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|i| std::cmp::Reverse(mathml[*i].len()));
    {
        let mut q = pool
            .queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for i in order {
            q.push_back(Job {
                id: i,
                mathml: mathml[i].clone(),
                reply: reply_tx.clone(),
            });
        }
    }
    drop(reply_tx);
    pool.ready.notify_all();
    let mut out: Vec<Result<String, String>> = vec![Err("equation was not read".to_string()); n];
    while let Ok((id, r)) = results_rx.recv() {
        out[id] = r;
    }
    out
}

/// Terms with a fixed spoken form, written as plain letters in the source.
const BUILTIN_LEXICON: &[(&str, &str)] = &[
    ("Re", "Reynolds number"),
    ("Ma", "Mach number"),
    ("Pr", "Prandtl number"),
    ("Sc", "Schmidt number"),
    ("St", "Strouhal number"),
    ("Fr", "Froude number"),
    ("Ra", "Rayleigh number"),
    ("Pe", "Peclet number"),
    ("We", "Weber number"),
    ("Ro", "Rossby number"),
    ("Ri", "Richardson number"),
    ("Gr", "Grashof number"),
    ("Nu", "Nusselt number"),
    ("Sh", "Sherwood number"),
    ("Bi", "Biot number"),
    ("Le", "Lewis number"),
    ("Ek", "Ekman number"),
    ("Kn", "Knudsen number"),
    ("Eu", "Euler number"),
    ("Bo", "Bond number"),
    ("Oh", "Ohnesorge number"),
    ("Ca", "capillary number"),
    ("Da", "Damkoehler number"),
    ("Wi", "Weissenberg number"),
    ("De", "Deborah number"),
];

/// Remove a built-in term said twice in a row: "Reynolds number `$Re_D$`" is "Reynolds number Reynolds number D" until
/// the second one goes.
pub(crate) fn collapse_terms(text: &str) -> String {
    let mut out = text.to_string();
    for (_, spoken) in BUILTIN_LEXICON {
        let twice = format!("{spoken} {spoken}").to_ascii_lowercase();
        let mut from = 0;
        while let Some(p) = out.to_ascii_lowercase()[from..].find(&twice) {
            let at = from + p;
            let end = at + twice.len();
            let word_start = at == 0 || !out.as_bytes()[at - 1].is_ascii_alphanumeric();
            let word_end = out
                .as_bytes()
                .get(end)
                .is_none_or(|c| !c.is_ascii_alphanumeric());
            if word_start && word_end {
                out.replace_range(at + spoken.len()..end, "");
            }
            from = at + spoken.len();
        }
    }
    out
}

/// Wrapping commands that may hold a term: `\mathit{Re}`, `\text{Re}` and the like.
const WRAPPERS: &[&str] = &[
    "mathit",
    "mathrm",
    "mathsf",
    "mathnormal",
    "mathtt",
    "text",
    "textit",
    "textrm",
    "textsf",
    "textnormal",
    "operatorname",
    "mbox",
];

const MARKER: &str = "Zqlex";

const GREEK_COMMANDS: &[&str] = &[
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
];

/// Whether `rest`, the formula after the letters `Re`, goes on with the thing whose real part it takes: a bracket, or
/// a Greek letter. With `any_symbol` (after `\operatorname`, which names a function) a Latin letter, a digit or a
/// group also counts. A relation, a script, a comma or the end means the Reynolds number.
fn operand_follows(rest: &str, any_symbol: bool) -> bool {
    let mut r = rest.trim_start();
    while let Some(t) = ["\\,", "\\;", "\\:", "\\!", "\\ "]
        .iter()
        .find_map(|c| r.strip_prefix(c))
    {
        r = t.trim_start();
    }
    let r = r.strip_prefix("\\left").unwrap_or(r).trim_start();
    if r.starts_with(['(', '[']) {
        return true;
    }
    if let Some(cmd) = r.strip_prefix('\\') {
        let name: String = cmd.chars().take_while(char::is_ascii_alphabetic).collect();
        return GREEK_COMMANDS.contains(&name.as_str());
    }
    any_symbol && r.starts_with(|c: char| c.is_alphanumeric() || c == '{')
}

/// The lexicon of spoken forms: built in, plus an optional user file of `term = spoken form` lines.
#[derive(Clone, Debug)]
pub(crate) struct Lexicon {
    entries: Vec<(String, String)>,
}

impl Lexicon {
    pub(crate) fn new(user: Option<&str>) -> Self {
        let mut entries: Vec<(String, String)> = BUILTIN_LEXICON
            .iter()
            .map(|(a, b)| ((*a).to_string(), (*b).to_string()))
            .collect();
        if let Some(text) = user {
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((k, v)) = line.split_once('=') {
                    let (k, v) = (k.trim(), v.trim());
                    if !k.is_empty() && !v.is_empty() && k.chars().all(|c| c.is_ascii_alphabetic())
                    {
                        entries.retain(|(e, _)| e != k);
                        entries.push((k.to_string(), v.to_string()));
                    }
                }
            }
        }
        Self { entries }
    }

    fn find(&self, term: &str) -> Option<usize> {
        self.entries.iter().position(|(k, _)| k == term)
    }

    fn marker(i: usize) -> String {
        let letters: String = i
            .to_string()
            .bytes()
            .map(|d| char::from(b'a' + (d - b'0')))
            .collect();
        format!("{MARKER}{letters}z")
    }

    /// Replace terms in the LaTeX source by marker words.
    fn mark(&self, latex: &str) -> String {
        let b = latex.as_bytes();
        let mut out = String::with_capacity(latex.len());
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'\\' {
                // \wrapper{Term}
                let name_start = i + 1;
                let mut j = name_start;
                while j < b.len() && b[j].is_ascii_alphabetic() {
                    j += 1;
                }
                let name = &latex[name_start..j];
                if WRAPPERS.contains(&name)
                    && b.get(j) == Some(&b'{')
                    && let Some(close) = latex[j..].find('}')
                {
                    let term = latex[j + 1..j + close].trim();
                    // `\mathrm{Re}(\lambda)` is a function of its argument, as `Re(\lambda)` is
                    let applied = operand_follows(&latex[j + close + 1..], name == "operatorname");
                    if let Some(k) = self.find(term) {
                        if applied && term == "Re" {
                            // the engine reads `\operatorname{Re}` as the real part of what follows
                            out.push_str("\\operatorname{Re}");
                        } else if applied {
                            out.push_str(&latex[i..=j + close]);
                        } else {
                            let _ = write!(out, "\\mathrm{{{}}}", Self::marker(k));
                        }
                        i = j + close + 1;
                        continue;
                    }
                }
                if matches!(
                    name,
                    "text"
                        | "textrm"
                        | "textit"
                        | "textsf"
                        | "textnormal"
                        | "mbox"
                        | "label"
                        | "tag"
                ) && b.get(j) == Some(&b'{')
                {
                    // plain words: copy the whole group
                    let mut depth = 0usize;
                    let mut k = j;
                    while k < b.len() {
                        match b[k] {
                            b'{' => depth += 1,
                            b'}' => {
                                depth -= 1;
                                if depth == 0 {
                                    k += 1;
                                    break;
                                }
                            }
                            _ => {}
                        }
                        k += 1;
                    }
                    let k = (k..=b.len())
                        .find(|e| latex.is_char_boundary(*e))
                        .unwrap_or(b.len());
                    out.push_str(&latex[i..k]);
                    i = k;
                    continue;
                }
                // copy the command and one escaped character
                let end = if j > name_start {
                    j
                } else {
                    (i + 2).min(b.len())
                };
                let end = (end..=b.len())
                    .find(|e| latex.is_char_boundary(*e))
                    .unwrap_or(b.len());
                out.push_str(&latex[i..end]);
                i = end;
                continue;
            }
            if b[i].is_ascii_alphabetic() {
                let start = i;
                while i < b.len() && b[i].is_ascii_alphabetic() {
                    i += 1;
                }
                let word = &latex[start..i];
                if word == "Re" && operand_follows(&latex[i..], false) {
                    out.push_str("\\operatorname{Re}");
                } else if !operand_follows(&latex[i..], false)
                    && let Some(k) = self.find(word)
                {
                    let _ = write!(out, "\\mathrm{{{}}}", Self::marker(k));
                } else {
                    out.push_str(word);
                }
                continue;
            }
            let ch = latex[i..].chars().next().unwrap_or(' ');
            out.push(ch);
            i += ch.len_utf8();
        }
        out
    }

    /// Give marked identifiers their `intent` in the `MathML`.
    fn apply(&self, mathml: &str) -> String {
        if !mathml.contains(MARKER) {
            return mathml.to_string();
        }
        let mut out = mathml.to_string();
        for (k, (term, spoken)) in self.entries.iter().enumerate() {
            let m = Self::marker(k);
            while let Some(pos) = out.find(&m) {
                let open = out[..pos].rfind("<mi").unwrap_or(0);
                let close = out[pos..].find("</mi>").map_or(out.len(), |c| pos + c + 5);
                let intent = spoken.replace(' ', "-");
                let rep = format!("<mi intent=\"{intent}\">{term}</mi>");
                out.replace_range(open..close, &rep);
            }
        }
        out
    }
}

/// `\left. f \right|_{x=0}` becomes `{f}|_{x=0}`: the bar after nothing on its left says "evaluated at", not "divides".
fn rewrite_evaluation_bar(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find("\\left.") {
        let body_start = p + "\\left.".len();
        // find the `\right` that closes this `\left`
        let mut depth = 1usize;
        let mut i = body_start;
        let mut close = None;
        while let Some(q) = rest[i..].find('\\') {
            let at = i + q;
            let tail = &rest[at..];
            if tail.starts_with("\\left")
                && !tail[5..].starts_with(|c: char| c.is_ascii_alphabetic())
            {
                depth += 1;
                i = at + 5;
            } else if tail.starts_with("\\right")
                && !tail[6..].starts_with(|c: char| c.is_ascii_alphabetic())
            {
                depth -= 1;
                if depth == 0 {
                    close = Some(at);
                    break;
                }
                i = at + 6;
            } else {
                i = at + 1;
            }
        }
        let Some(at) = close else { break };
        let after = &rest[at + "\\right".len()..];
        let bar = if after.starts_with('|') {
            Some(1)
        } else if after.starts_with("\\vert")
            && !after[5..].starts_with(|c: char| c.is_ascii_alphabetic())
        {
            Some(5)
        } else {
            None
        };
        out.push_str(&rest[..p]);
        if let Some(n) = bar {
            out.push('{');
            out.push_str(&rest[body_start..at]);
            out.push_str("}|");
            rest = &after[n..];
        } else {
            out.push_str(&rest[p..at + "\\right".len()]);
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// `\substack{a \\ b}` becomes a one-column array.
fn rewrite_substack(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find("\\substack") {
        out.push_str(&rest[..p]);
        let after = &rest[p + 9..];
        let trimmed = after.trim_start();
        if trimmed.starts_with('{')
            && let Some((a, z, nx)) = crate::latex::brace_group(trimmed)
        {
            out.push_str("\\begin{subarray}{c}");
            out.push_str(&trimmed[a..z]);
            out.push_str("\\end{subarray}");
            rest = &trimmed[nx..];
        } else {
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// Greek letters the `upgreek` package writes upright, as `\\upalpha`.
const GREEK: &[&str] = &[
    "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta", "iota", "kappa",
    "lambda", "mu", "nu", "xi", "pi", "rho", "sigma", "tau", "phi", "chi", "psi", "omega",
];

/// `\upmu` becomes `\mu`: the reading does not tell upright from slanted letters.
fn rewrite_upright_greek(s: &str) -> String {
    let mut out = s.to_string();
    for name in GREEK {
        let from = format!("\\up{name}");
        if out.contains(&from) {
            out = out.replace(&from, &format!("\\{name}"));
        }
    }
    out
}

/// Commands of `physics`, `mathtools` and similar packages that wrap one argument in delimiters:
/// `\norm{A}` becomes `\left\| A\right\|`. A star or a bracketed size after the name is dropped.
const PAIRED: &[(&str, &str, &str)] = &[
    ("norm", "\\left\\|", "\\right\\|"),
    ("abs", "\\left|", "\\right|"),
    ("floor", "\\left\\lfloor ", "\\right\\rfloor"),
    ("ceil", "\\left\\lceil ", "\\right\\rceil"),
];

/// Replace the one-argument delimiter commands of [`PAIRED`] that are written with a braced argument.
fn rewrite_paired(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    'scan: while let Some(p) = rest.find('\\') {
        out.push_str(&rest[..p]);
        let after = &rest[p + 1..];
        let name_len = after
            .find(|c: char| !c.is_ascii_alphabetic())
            .unwrap_or(after.len());
        let name = &after[..name_len];
        for (cmd, open, close) in PAIRED {
            if name == *cmd {
                let tail = after[name_len..].trim_start_matches('*');
                let tail = tail.trim_start();
                if tail.starts_with('{')
                    && let Some((a, z, nx)) = crate::latex::brace_group(tail)
                {
                    out.push_str(open);
                    out.push_str(&rewrite_paired(&tail[a..z]));
                    out.push_str(close);
                    rest = &tail[nx..];
                    continue 'scan;
                }
            }
        }
        out.push('\\');
        let keep = if name_len == 0 {
            after.chars().next().map_or(0, char::len_utf8)
        } else {
            name_len
        };
        out.push_str(&after[..keep]);
        rest = &after[keep..];
    }
    out.push_str(rest);
    out
}

/// Replacements for commands that LaTeX papers use and math-core does not know.
/// Remove layout commands that sit inside an equation, such as `\setlength{\arraycolsep}{0pt}` and
/// `\renewcommand{\arraystretch}{1.3}`: they set spacing and say nothing.
pub(crate) fn strip_layout(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    'next: while !rest.is_empty() {
        for (cmd, groups) in [
            ("\\setlength", 2),
            ("\\addtolength", 2),
            ("\\renewcommand", 2),
        ] {
            if let Some(after) = rest.strip_prefix(cmd) {
                let mut tail = after;
                let mut ok = true;
                for _ in 0..groups {
                    let t = tail.trim_start();
                    if let Some((_, _, next)) = crate::latex::brace_group(t) {
                        tail = &t[next..];
                    } else {
                        ok = false;
                        break;
                    }
                }
                // only the spacing commands: a macro definition inside an equation is left to the converter
                let spacing = after.contains("\\array") || cmd != "\\renewcommand";
                if ok && spacing && after.trim_start().starts_with(['{']) {
                    rest = tail;
                    continue 'next;
                }
            }
        }
        let c = rest.chars().next().unwrap_or(' ');
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// Remove scripts with nothing in them (`^{}`, `_{}`), as a macro leaves when its optional exponent is empty.
fn drop_empty_scripts(latex: &str) -> String {
    let mut out = String::with_capacity(latex.len());
    let mut rest = latex;
    while let Some(p) = rest.find(['^', '_']) {
        let (head, tail) = rest.split_at(p);
        out.push_str(head);
        // `\^{}` and `\_{}` are accents and symbols of text mode, not scripts
        let escaped = head.ends_with('\\');
        if !escaped && tail[1..].starts_with("{}") {
            rest = &tail[3..];
        } else {
            out.push_str(&tail[..1]);
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

/// `\cdot` with a bracket, bar, comma or the end of the formula on both sides is a placeholder for an argument, as in
/// `\|\cdot\|` and `f(\cdot)`: it becomes the word "dot", which the maths library cannot place by itself.
fn rewrite_placeholder_dot(latex: &str) -> String {
    const SPACING: [&str; 7] = ["\\,", "\\;", "\\:", "\\!", "\\ ", "\\quad", "\\qquad"];
    const OPENERS: [&str; 7] = ["(", "[", "{", "|", ",", ";", "\\langle"];
    const OPEN_NAMES: [&str; 5] = ["\\lVert", "\\lvert", "\\lfloor", "\\lceil", "\\lbrace"];
    const CLOSERS: [&str; 15] = [
        ")", "]", "}", "|", ",", ";", "\\rangle", "\\rVert", "\\rvert", "\\rfloor", "\\rceil",
        "\\rbrace", "\\right", "\\|", "\\}",
    ];
    fn trim_start(mut s: &str) -> &str {
        loop {
            let t = s.trim_start();
            match SPACING.iter().find_map(|c| t.strip_prefix(c)) {
                Some(r) => s = r,
                None => return t,
            }
        }
    }
    fn trim_end(mut s: &str) -> &str {
        loop {
            let t = s.trim_end();
            match SPACING.iter().find_map(|c| t.strip_suffix(c)) {
                Some(r) => s = r,
                None => return t,
            }
        }
    }
    let mut out = String::with_capacity(latex.len());
    let mut rest = latex;
    while let Some(p) = rest.find("\\cdot") {
        let after = &rest[p + 5..];
        out.push_str(&rest[..p]);
        let before = trim_end(&out);
        let next = trim_start(after);
        let word_continues = after.starts_with(|c: char| c.is_ascii_alphabetic());
        let open = before.is_empty()
            || OPENERS
                .iter()
                .chain(&OPEN_NAMES)
                .any(|o| before.ends_with(o));
        let close = next.is_empty() || CLOSERS.iter().any(|c| next.starts_with(c));
        if !word_continues && open && close {
            out.push_str("\\text{dot}");
        } else {
            out.push_str("\\cdot");
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

fn rewrite_unsupported(latex: &str) -> String {
    let mut s = if latex.contains("\\substack") {
        rewrite_substack(latex)
    } else {
        latex.to_string()
    };
    if s.contains("^{}") || s.contains("_{}") {
        s = drop_empty_scripts(&s);
    }
    if s.contains("\\cdot") {
        s = rewrite_placeholder_dot(&s);
    }
    if s.contains("\\up") {
        s = rewrite_upright_greek(&s);
    }
    if s.contains("\\left.") {
        s = rewrite_evaluation_bar(&s);
    }
    if ["\\norm", "\\abs", "\\floor", "\\ceil"]
        .iter()
        .any(|c| s.contains(c))
    {
        s = rewrite_paired(&s);
    }
    if s.contains("\\setlength") || s.contains("\\addtolength") || s.contains("\\renewcommand") {
        s = strip_layout(&s);
    }
    for (from, to) in [
        (":=", "\\coloneqq "),
        ("\\leqslant", "\\leq"),
        ("\\geqslant", "\\geq"),
        ("\\nleqslant", "\\nleq"),
        ("\\ngeqslant", "\\ngeq"),
        ("\\dotsi", "\\cdots"),
        ("\\dotsb", "\\cdots"),
        ("\\dotsm", "\\cdots"),
        ("\\dotsc", "\\ldots"),
        ("\\dotso", "\\ldots"),
        ("\\begin{split}", "\\begin{aligned}"),
        ("\\end{split}", "\\end{aligned}"),
        ("\\begin{alignedat}", "\\begin{aligned}"),
        ("\\end{alignedat}", "\\end{aligned}"),
        ("\\operatorname*", "\\operatorname"),
        ("\\mathbbm", "\\mathbb"),
        ("\\widetilde", "\\tilde"),
        ("^{}", ""),
        ("_{}", ""),
        ("\\mbox{", "\\text{"),
        ("\\AA", "\u{c5}"),
        ("\\mathds", "\\mathbb"),
        ("\\mathsfbi", "\\mathsf"),
        ("\\@", ""),
        ("\\widebar", "\\overline"),
        ("\\dddot", "\\ddot"),
        ("\\bm{", "\\boldsymbol{"),
        ("\\ensuremath", ""),
        ("\\nonumber", ""),
        ("\\notag", ""),
        ("\\displaybreak", ""),
        ("\\allowbreak", ""),
        ("\\mathstrut", ""),
        ("\\bigstrut", ""),
    ] {
        if s.contains(from) {
            s = s.replace(from, to);
        }
    }
    s
}

/// Deepest nesting of groups, `\left`...`\right` pairs and environments an equation may have. The converter and
/// the speech rules recurse on it, so a deeper equation is read by the fallback instead.
const MAX_MATH_NESTING: usize = 64;

/// Longest equation, in bytes of LaTeX, that goes to the maths library. A longer one is read by the fallback: the
/// library takes time that grows faster than the length, and a macro can build an equation of any size.
const MAX_MATH_BYTES: usize = 64 * 1024;

/// The deepest nesting of `{`, `\left` and `\begin` in `latex`.
fn nesting(latex: &str) -> usize {
    let b = latex.as_bytes();
    let (mut depth, mut deepest) = (0usize, 0usize);
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            b'\\' => {
                let rest = &latex[i + 1..];
                // the whole command name: `\leftarrow` and `\endgroup` do not open or close anything
                let word = rest
                    .split(|c: char| !c.is_ascii_alphabetic())
                    .next()
                    .unwrap_or("");
                if matches!(word, "left" | "begin") {
                    depth += 1;
                } else if matches!(word, "right" | "end") {
                    depth = depth.saturating_sub(1);
                }
                i += 1;
            }
            _ => {}
        }
        deepest = deepest.max(depth);
        i += 1;
    }
    deepest
}

/// A reading with the problems met on the way.
pub(crate) struct Reading {
    pub text: String,
    pub issues: Vec<(String, String)>,
}

/// Converts LaTeX to `MathML` and applies the lexicon.
pub(crate) struct MathEngine {
    converter: LatexToMathML,
    lexicon: Lexicon,
    threads: Option<usize>,
}

/// An equation ready for `MathCAT`, or already read by the fallback.
enum Prepared {
    Mathml {
        mathml: String,
        issues: Vec<(String, String)>,
    },
    Done(Reading),
}

impl MathEngine {
    pub(crate) fn new(user_lexicon: Option<&str>, threads: Option<usize>) -> Self {
        let converter = LatexToMathML::new(MathCoreConfig::default(), vec![])
            .unwrap_or_else(|_| LatexToMathML::default());
        Self {
            converter,
            lexicon: Lexicon::new(user_lexicon),
            threads,
        }
    }

    fn convert(&self, src: &str) -> Result<String, (std::ops::Range<usize>, String)> {
        self.converter
            .convert_with_local_state(src, MathDisplay::Inline)
            .map(|r| r.mathml)
            .map_err(|e| (e.0.clone(), format!("{e:?}")))
    }

    fn prepare(&self, latex: &str) -> Prepared {
        let mut issues = Vec::new();
        if latex.len() > MAX_MATH_BYTES {
            issues.push((
                "size".to_string(),
                format!("equation longer than {MAX_MATH_BYTES} bytes"),
            ));
            return Prepared::Done(Reading {
                text: plain_fallback(latex),
                issues,
            });
        }
        if nesting(latex) > MAX_MATH_NESTING {
            issues.push((
                "nesting".to_string(),
                format!("equation nested deeper than {MAX_MATH_NESTING} levels"),
            ));
            return Prepared::Done(Reading {
                text: plain_fallback(latex),
                issues,
            });
        }
        let mut src = self.lexicon.mark(&rewrite_unsupported(latex));
        let mut mathml = None;
        for _ in 0..24 {
            match self.convert(&src) {
                Ok(m) => {
                    mathml = Some(m);
                    break;
                }
                Err((range, dbg)) => {
                    if dbg.contains("ExpectedDelimiter")
                        && (src.contains("\\left") || src.contains("\\right"))
                    {
                        // \left and \right split over table rows: keep the delimiters, drop the pairing
                        issues.push((
                            "math syntax".to_string(),
                            "unpaired \\left or \\right".to_string(),
                        ));
                        src = src
                            .replace("\\left.", "")
                            .replace("\\right.", "")
                            .replace("\\left", "")
                            .replace("\\right", "");
                        continue;
                    }
                    if dbg.contains("NestedMathMode") && src.contains('$') {
                        issues.push((
                            "math syntax".to_string(),
                            "$ inside an equation".to_string(),
                        ));
                        src = src.replace('$', "");
                        continue;
                    }
                    let fixable = dbg.contains("UnknownCommand")
                        && src
                            .get(range.clone())
                            .is_some_and(|t| t.starts_with('\\') && t.len() > 1);
                    if fixable {
                        let cmd = src[range.clone()].to_string();
                        let name = cmd[1..].to_string();
                        issues.push((
                            "math command".to_string(),
                            format!("unknown command \\{name} in an equation"),
                        ));
                        let word: String = name.chars().filter(|c| c.is_alphabetic()).collect();
                        let has_arg = src[range.end..].trim_start().starts_with('{');
                        let rep = if word.is_empty() || has_arg {
                            String::new()
                        } else {
                            format!("\\mathrm{{{word}}}")
                        };
                        src.replace_range(range, &rep);
                    } else {
                        let near: String = src.chars().take(70).collect();
                        issues.push(("math syntax".to_string(), format!("{dbg} in {near:?}")));
                        break;
                    }
                }
            }
        }
        match mathml {
            Some(m) => Prepared::Mathml {
                mathml: fix_entities(&self.lexicon.apply(&m)),
                issues,
            },
            None => Prepared::Done(Reading {
                text: plain_fallback(latex),
                issues,
            }),
        }
    }

    /// Read a list of equations.
    pub(crate) fn speak_many(&self, latex: &[String]) -> Vec<Reading> {
        let prepared: Vec<Prepared> = latex.iter().map(|l| self.prepare(l)).collect();
        let mut jobs = Vec::new();
        let mut job_of = vec![usize::MAX; prepared.len()];
        for (i, p) in prepared.iter().enumerate() {
            if let Prepared::Mathml { mathml, .. } = p {
                job_of[i] = jobs.len();
                jobs.push(mathml.clone());
            }
        }
        let results = speak_all(&jobs, self.threads);
        prepared
            .into_iter()
            .enumerate()
            .map(|(i, p)| match p {
                Prepared::Done(r) => r,
                Prepared::Mathml {
                    issues: mut iss, ..
                } => match &results[job_of[i]] {
                    Ok(t) => Reading {
                        text: squeeze(t),
                        issues: iss,
                    },
                    Err(e) => {
                        iss.push(("math speech".to_string(), e.clone()));
                        Reading {
                            text: plain_fallback(&latex[i]),
                            issues: iss,
                        }
                    }
                },
            })
            .collect()
    }

    /// Read one equation.
    #[cfg(test)]
    pub(crate) fn speak(&self, latex: &str) -> Reading {
        self.speak_many(&[latex.to_string()])
            .pop()
            .unwrap_or(Reading {
                text: String::new(),
                issues: Vec::new(),
            })
    }
}

/// Read all equations met so far and put their words where the slots stand.
pub(crate) fn resolve(cx: &mut crate::raw::Ctx, engine: &MathEngine) {
    if cx.math.latex.is_empty() {
        return;
    }
    let latex = std::mem::take(&mut cx.math.latex);
    let locs = std::mem::take(&mut cx.math.locs);
    let readings = engine.speak_many(&latex);
    let mut texts = Vec::with_capacity(readings.len());
    for (r, loc) in readings.into_iter().zip(locs) {
        for (construct, msg) in r.issues {
            cx.warn(&construct, msg, loc);
        }
        texts.push(r.text);
    }
    let fill = |s: &str| fill_slots(s, &texts);
    for b in &mut cx.blocks {
        b.raw.replace_text(&fill);
    }
    for h in &mut cx.holes {
        if let crate::raw::Hole::Footnote { raw: Some(r), .. } = h {
            r.replace_text(&fill);
        }
    }
    for r in cx.footnote_defs.values_mut() {
        r.replace_text(&fill);
    }
    for l in cx.labels.values_mut() {
        l.title = fill(&l.title);
    }
}

/// Replace each slot in `s` by the words of its equation.
pub(crate) fn fill_slots(s: &str, texts: &[String]) -> String {
    if !s.contains(SLOT_OPEN) {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(a) = rest.find(SLOT_OPEN) {
        out.push_str(&rest[..a]);
        let after = &rest[a + SLOT_OPEN.len_utf8()..];
        let Some(z) = after.find(SLOT_CLOSE) else {
            rest = "";
            break;
        };
        if let Some(t) = after[..z].parse::<usize>().ok().and_then(|i| texts.get(i)) {
            out.push_str(t);
        }
        rest = &after[z + SLOT_CLOSE.len_utf8()..];
    }
    out.push_str(rest);
    out
}

/// The plain letter or digit for a sans-serif or monospace letter or digit of the Mathematical Alphanumeric Symbols
/// block (`\mathsf{err}` and `\mathtt{x}`); `MathCAT` has no names for them in a word.
fn plain_math_alphanumeric(c: char) -> Option<char> {
    let u = u32::from(c);
    match u {
        0x1D5A0..=0x1D6A3 => {
            let k = (u - 0x1D5A0) % 52;
            char::from_u32(if k < 26 { 0x41 + k } else { 0x61 + k - 26 })
        }
        0x1D7E2..=0x1D7FF => char::from_u32(0x30 + (u - 0x1D7E2) % 10),
        _ => None,
    }
}

/// math-core writes named entities for invisible operators; the XML reader behind `MathCAT` knows only five.
fn fix_entities(mathml: &str) -> String {
    let stripped;
    let mathml = if mathml.contains(['\u{fe00}', '\u{fe01}']) {
        // variation selectors after script and fraktur letters: MathCAT has no names for the pairs
        stripped = mathml.replace(['\u{fe00}', '\u{fe01}'], "");
        stripped.as_str()
    } else {
        mathml
    };
    let plain;
    let mathml = if mathml.chars().any(|c| c >= '\u{1D5A0}') {
        plain = mathml
            .chars()
            .map(|c| plain_math_alphanumeric(c).unwrap_or(c))
            .collect::<String>();
        plain.as_str()
    } else {
        mathml
    };
    if !mathml.contains('&') {
        return mathml.to_string();
    }
    let mut out = String::with_capacity(mathml.len());
    let mut rest = mathml;
    while let Some(a) = rest.find('&') {
        out.push_str(&rest[..a]);
        let after = &rest[a..];
        let Some(z) = after.find(';').filter(|z| *z <= 24) else {
            out.push('&');
            rest = &after[1..];
            continue;
        };
        let name = &after[1..z];
        match name {
            "amp" | "lt" | "gt" | "quot" | "apos" => out.push_str(&after[..=z]),
            "ApplyFunction" | "af" => out.push('\u{2061}'),
            "InvisibleTimes" | "it" => out.push('\u{2062}'),
            "InvisibleComma" | "ic" => out.push('\u{2063}'),
            "nbsp" | "NonBreakingSpace" => out.push('\u{a0}'),
            n if n.starts_with('#') => out.push_str(&after[..=z]),
            _ => {}
        }
        rest = &after[z + 1..];
    }
    out.push_str(rest);
    out
}

fn squeeze(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A rough reading used when an equation cannot be converted: letters and operators only.
pub(crate) fn plain_fallback(latex: &str) -> String {
    let mut out = String::new();
    let b: Vec<char> = latex.chars().collect();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        match c {
            '\\' => {
                let mut j = i + 1;
                while j < b.len() && b[j].is_ascii_alphabetic() {
                    j += 1;
                }
                let name: String = b[i + 1..j].iter().collect();
                out.push(' ');
                match name.as_str() {
                    "cdot" => out.push_str("dot"),
                    "le" | "leq" => out.push_str("less than or equal to"),
                    "ge" | "geq" => out.push_str("greater than or equal to"),
                    "" | "frac" | "left" | "right" | "mathrm" | "mathbf" | "mathit" | "text"
                    | "mathcal" | "mathbb" | "operatorname" | "label" | "tag" | "nonumber"
                    | "notag" | "quad" | "qquad" | "begin" | "end" => {}
                    n => out.push_str(n),
                }
                out.push(' ');
                i = if j == i + 1 { (i + 2).min(b.len()) } else { j };
            }
            '=' => {
                out.push_str(" equal to ");
                i += 1;
            }
            '+' => {
                out.push_str(" plus ");
                i += 1;
            }
            '-' => {
                out.push_str(" minus ");
                i += 1;
            }
            '<' => {
                out.push_str(" less than ");
                i += 1;
            }
            '>' => {
                out.push_str(" greater than ");
                i += 1;
            }
            '/' => {
                out.push_str(" over ");
                i += 1;
            }
            '*' => {
                out.push_str(" times ");
                i += 1;
            }
            '^' => {
                out.push_str(" to the ");
                i += 1;
            }
            c if c.is_alphanumeric() || c == '.' || c == ',' => {
                out.push(c);
                i += 1;
            }
            _ => {
                out.push(' ');
                i += 1;
            }
        }
    }
    squeeze(&out)
}

/// Split punctuation written after an equation (`x = 1,`) from the equation.
pub(crate) fn split_trailing_punct(latex: &str) -> (&str, Option<char>) {
    let mut s = latex.trim_end();
    loop {
        let before = s;
        for tail in ["\\,", "\\;", "\\:", "\\!", "\\ ", "\\quad", "\\qquad", "~"] {
            if let Some(r) = s.strip_suffix(tail) {
                s = r.trim_end();
            }
        }
        if s == before {
            break;
        }
    }
    match s.chars().last() {
        Some(c @ ('.' | ',' | ';')) if !s.ends_with("\\.") => {
            (s[..s.len() - 1].trim_end(), Some(c))
        }
        _ => (s, None),
    }
}

/// Words for inline code: underscores become spaces and symbols that are not words are dropped.
pub(crate) fn speak_code(code: &str) -> String {
    let mut out = String::with_capacity(code.len());
    for c in code.chars() {
        match c {
            '_' | '\\' | '{' | '}' | '$' | '^' | '`' | '#' | '|' | '~' | '*' => out.push(' '),
            '=' => out.push_str(" equals "),
            c => out.push(c),
        }
    }
    squeeze(&out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_an_equation() {
        let m = MathEngine::new(None, None);
        assert_eq!(m.speak(r"\beta = 43000").text, "beta equal to 43000");
    }

    #[test]
    fn lexicon_names_the_reynolds_number() {
        let m = MathEngine::new(None, None);
        assert_eq!(
            m.speak(r"\mathit{Re} = 100").text,
            "Reynolds number equal to 100"
        );
        assert_eq!(m.speak(r"Re = 100").text, "Reynolds number equal to 100");
    }

    #[test]
    fn unknown_command_is_reported_and_read() {
        let m = MathEngine::new(None, None);
        let r = m.speak(r"\foo + 1");
        assert_eq!(r.text, "foo plus 1");
        assert_eq!(r.issues.len(), 1);
    }

    #[test]
    fn fallback_has_no_markup() {
        let t = plain_fallback(r"\frac{a}{b}^{2} = \alpha");
        assert!(!t.contains(['\\', '{', '}', '^']));
    }

    #[test]
    fn nesting_counts_whole_commands_only() {
        // arrows and `\endgroup` begin with `left`, `right` or `end` and open nothing
        let deep = format!("{}x", "{\\rightarrow".repeat(100));
        assert_eq!(nesting(&deep), 100);
        let arrows = "\\leftarrow ".repeat(100) + "\\leftrightarrow\\endgroup";
        assert_eq!(nesting(&arrows), 0);
        assert_eq!(
            nesting("\\left( x \\right) \\begin{matrix} a \\end{matrix}"),
            2
        );
        assert_eq!(nesting("\\left(\\left[ x \\right]\\right)"), 2);
    }

    #[test]
    fn empty_scripts_are_dropped() {
        assert_eq!(drop_empty_scripts(r"\partial^{} u_{}^2"), r"\partial u^2");
        assert_eq!(drop_empty_scripts(r"\_{} \^{} a^{b}"), r"\_{} \^{} a^{b}");
        let m = MathEngine::new(None, None);
        assert_eq!(
            m.speak(r"\frac{\partial^{} u}{\partial t^{}}").text,
            m.speak(r"\frac{\partial u}{\partial t}").text
        );
    }
}
