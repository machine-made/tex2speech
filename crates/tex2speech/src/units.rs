// SPDX-License-Identifier: Apache-2.0
//! `siunitx` quantities spoken in words: numbers, units, ranges, lists and angles.

const UNITS: &[(&str, &str)] = &[
    ("metre", "metre"),
    ("meter", "metre"),
    ("gram", "gram"),
    ("second", "second"),
    ("ampere", "ampere"),
    ("kelvin", "kelvin"),
    ("mole", "mole"),
    ("candela", "candela"),
    ("radian", "radian"),
    ("steradian", "steradian"),
    ("hertz", "hertz"),
    ("newton", "newton"),
    ("pascal", "pascal"),
    ("joule", "joule"),
    ("watt", "watt"),
    ("coulomb", "coulomb"),
    ("volt", "volt"),
    ("farad", "farad"),
    ("ohm", "ohm"),
    ("siemens", "siemens"),
    ("weber", "weber"),
    ("tesla", "tesla"),
    ("henry", "henry"),
    ("degreeCelsius", "degree Celsius"),
    ("celsius", "degree Celsius"),
    ("lumen", "lumen"),
    ("lux", "lux"),
    ("becquerel", "becquerel"),
    ("gray", "gray"),
    ("sievert", "sievert"),
    ("katal", "katal"),
    ("minute", "minute"),
    ("hour", "hour"),
    ("day", "day"),
    ("degree", "degree"),
    ("arcminute", "arcminute"),
    ("arcsecond", "arcsecond"),
    ("hectare", "hectare"),
    ("litre", "litre"),
    ("liter", "litre"),
    ("tonne", "tonne"),
    ("electronvolt", "electronvolt"),
    ("dalton", "dalton"),
    ("astronomicalunit", "astronomical unit"),
    ("neper", "neper"),
    ("bel", "bel"),
    ("decibel", "decibel"),
    ("angstrom", "angstrom"),
    ("bar", "bar"),
    ("percent", "percent"),
    ("knot", "knot"),
    ("mmHg", "millimetre of mercury"),
    ("atmosphere", "atmosphere"),
    ("bit", "bit"),
    ("byte", "byte"),
    ("rpm", "revolution per minute"),
];

const SHORT_UNITS: &[(&str, &str)] = &[
    ("m", "metre"),
    ("g", "gram"),
    ("s", "second"),
    ("A", "ampere"),
    ("K", "kelvin"),
    ("mol", "mole"),
    ("cd", "candela"),
    ("rad", "radian"),
    ("sr", "steradian"),
    ("Hz", "hertz"),
    ("N", "newton"),
    ("Pa", "pascal"),
    ("J", "joule"),
    ("W", "watt"),
    ("C", "coulomb"),
    ("V", "volt"),
    ("F", "farad"),
    ("Ω", "ohm"),
    ("S", "siemens"),
    ("Wb", "weber"),
    ("T", "tesla"),
    ("H", "henry"),
    ("°C", "degree Celsius"),
    ("lm", "lumen"),
    ("lx", "lux"),
    ("Bq", "becquerel"),
    ("Gy", "gray"),
    ("Sv", "sievert"),
    ("kat", "katal"),
    ("min", "minute"),
    ("h", "hour"),
    ("d", "day"),
    ("L", "litre"),
    ("l", "litre"),
    ("t", "tonne"),
    ("eV", "electronvolt"),
    ("Da", "dalton"),
    ("au", "astronomical unit"),
    ("dB", "decibel"),
    ("bar", "bar"),
    ("%", "percent"),
    ("°", "degree"),
    ("Å", "angstrom"),
    ("rpm", "revolution per minute"),
];

const PREFIXES: &[(&str, &str, &str)] = &[
    ("yocto", "y", "yocto"),
    ("zepto", "z", "zepto"),
    ("atto", "a", "atto"),
    ("femto", "f", "femto"),
    ("pico", "p", "pico"),
    ("nano", "n", "nano"),
    ("micro", "µ", "micro"),
    ("micro", "u", "micro"),
    ("milli", "m", "milli"),
    ("centi", "c", "centi"),
    ("deci", "d", "deci"),
    ("deca", "da", "deca"),
    ("hecto", "h", "hecto"),
    ("kilo", "k", "kilo"),
    ("mega", "M", "mega"),
    ("giga", "G", "giga"),
    ("tera", "T", "tera"),
    ("peta", "P", "peta"),
    ("exa", "E", "exa"),
    ("zetta", "Z", "zetta"),
    ("yotta", "Y", "yotta"),
    ("kibi", "Ki", "kibi"),
    ("mebi", "Mi", "mebi"),
    ("gibi", "Gi", "gibi"),
];

/// A unit factor under construction.
struct Factor {
    prefix: String,
    unit: String,
    power: String,
    /// "square" or "cubic" written before the unit
    lead: &'static str,
}

fn plural(unit: &str) -> String {
    match unit {
        "hertz" | "siemens" | "lux" => unit.to_string(),
        "degree Celsius" => "degrees Celsius".to_string(),
        "henry" => "henries".to_string(),
        "revolution per minute" => "revolutions per minute".to_string(),
        "millimetre of mercury" => "millimetres of mercury".to_string(),
        u => format!("{u}s"),
    }
}

fn power_words(p: &str) -> String {
    match p.trim() {
        "2" => "squared".to_string(),
        "3" => "cubed".to_string(),
        "" => String::new(),
        "-1" => "to the minus 1".to_string(),
        n => format!("to the {}", n.replace('-', "minus ")),
    }
}

fn lookup_short(tok: &str) -> Option<(String, String)> {
    if let Some((_, name)) = SHORT_UNITS.iter().find(|(s, _)| *s == tok) {
        return Some((String::new(), (*name).to_string()));
    }
    let mut best = None;
    for (_, sym, name) in PREFIXES {
        if let Some(rest) = tok.strip_prefix(sym)
            && let Some((_, unit)) = SHORT_UNITS.iter().find(|(s, _)| *s == rest)
            && best
                .as_ref()
                .is_none_or(|(p, _): &(String, String)| p.len() < name.len())
        {
            best = Some(((*name).to_string(), (*unit).to_string()));
        }
    }
    best
}

fn read_group(s: &[char], i: usize) -> (String, usize) {
    // s[i] == '{'
    let mut depth = 0;
    let mut j = i;
    let mut out = String::new();
    while j < s.len() {
        match s[j] {
            '{' => {
                depth += 1;
                if depth > 1 {
                    out.push('{');
                }
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return (out, j + 1);
                }
                out.push('}');
            }
            c => out.push(c),
        }
        j += 1;
    }
    (out, j)
}

/// Spoken form of a unit expression such as `\metre\per\second` or `m.s^{-1}`.
/// `plural_first` makes the last unit before a `per` plural.
#[allow(clippy::too_many_lines)] // one flat table of commands; not split
pub(crate) fn speak_unit(unit: &str, plural_first: bool) -> String {
    let s: Vec<char> = unit.chars().collect();
    let mut num: Vec<Factor> = Vec::new();
    let mut den: Vec<Factor> = Vec::new();
    let mut in_den = false;
    let mut pending_prefix = String::new();
    let mut pending_lead: &'static str = "";
    let mut i = 0;
    while i < s.len() {
        let c = s[i];
        if c == '\\' {
            let mut j = i + 1;
            while j < s.len() && s[j].is_ascii_alphabetic() {
                j += 1;
            }
            let name: String = s[i + 1..j].iter().collect();
            i = j.max(i + 2).min(s.len());
            match name.as_str() {
                "per" => in_den = true,
                "square" => pending_lead = "square",
                "cubic" => pending_lead = "cubic",
                "squared" | "cubed" => {
                    let p = if name == "squared" { "2" } else { "3" };
                    if let Some(f) = if in_den {
                        den.last_mut()
                    } else {
                        num.last_mut()
                    } {
                        f.power = p.to_string();
                    }
                }
                "tothe" | "raiseto" => {
                    if i < s.len() && s[i] == '{' {
                        let (g, nx) = read_group(&s, i);
                        i = nx;
                        if let Some(f) = if in_den {
                            den.last_mut()
                        } else {
                            num.last_mut()
                        } {
                            f.power = g;
                        }
                    }
                }
                "of" | "usk" | "cancel" | "highlight" | "mathrm" | "text" | "ensuremath" | ","
                | " " | "cdot" => {}
                n => {
                    if let Some((_, _, p)) = PREFIXES.iter().find(|(pn, _, _)| *pn == n) {
                        pending_prefix = (*p).to_string();
                    } else if let Some((_, u)) = UNITS.iter().find(|(un, _)| *un == n) {
                        let f = Factor {
                            prefix: std::mem::take(&mut pending_prefix),
                            unit: (*u).to_string(),
                            power: String::new(),
                            lead: std::mem::take(&mut pending_lead),
                        };
                        if in_den { den.push(f) } else { num.push(f) }
                    } else {
                        let f = Factor {
                            prefix: std::mem::take(&mut pending_prefix),
                            unit: n.to_string(),
                            power: String::new(),
                            lead: std::mem::take(&mut pending_lead),
                        };
                        if in_den { den.push(f) } else { num.push(f) }
                    }
                }
            }
            continue;
        }
        if c == '/' {
            in_den = true;
            i += 1;
            continue;
        }
        if c == '^' {
            i += 1;
            let p = if i < s.len() && s[i] == '{' {
                let (g, nx) = read_group(&s, i);
                i = nx;
                g
            } else {
                let mut g = String::new();
                while i < s.len() && (s[i].is_ascii_digit() || s[i] == '-') {
                    g.push(s[i]);
                    i += 1;
                }
                g
            };
            if !in_den && p.starts_with('-') {
                if let Some(mut f) = num.pop() {
                    let q = p.trim_start_matches('-');
                    f.power = if q == "1" {
                        String::new()
                    } else {
                        q.to_string()
                    };
                    den.push(f);
                }
            } else if let Some(f) = if in_den {
                den.last_mut()
            } else {
                num.last_mut()
            } {
                f.power = p;
            }
            continue;
        }
        if c.is_alphabetic() || matches!(c, '°' | '%' | 'Ω' | 'µ' | 'Å') {
            let mut tok = String::new();
            while i < s.len()
                && (s[i].is_alphabetic() || matches!(s[i], '°' | '%' | 'Ω' | 'µ' | 'Å'))
            {
                tok.push(s[i]);
                i += 1;
            }
            let (prefix, name) = lookup_short(&tok).unwrap_or_else(|| (String::new(), tok.clone()));
            let f = Factor {
                prefix,
                unit: name,
                power: String::new(),
                lead: "",
            };
            if in_den {
                den.push(f);
            } else {
                num.push(f);
            }
            continue;
        }
        i += 1;
    }
    let render = |f: &Factor, pl: bool| -> String {
        let mut w = String::new();
        if !f.lead.is_empty() {
            w.push_str(f.lead);
            w.push(' ');
        }
        let unit = if pl { plural(&f.unit) } else { f.unit.clone() };
        w.push_str(&f.prefix);
        w.push_str(&unit);
        let pw = power_words(&f.power);
        if !pw.is_empty() {
            w.push(' ');
            w.push_str(&pw);
        }
        w
    };
    let mut parts: Vec<String> = Vec::new();
    let n = num.len();
    for (k, f) in num.iter().enumerate() {
        parts.push(render(f, plural_first && k + 1 == n));
    }
    if !den.is_empty() {
        parts.push("per".to_string());
        for f in &den {
            parts.push(render(f, false));
        }
    }
    parts.join(" ")
}

/// Spoken form of a siunitx number such as `1.5e-3`, `3 +- 0.1` or `2x3`.
pub(crate) fn speak_number(num: &str) -> String {
    let mut out = String::new();
    let cs: Vec<char> = num.trim().chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        match c {
            'e' | 'E' | 'd' | 'D'
                if i > 0
                    && cs[i - 1].is_ascii_digit()
                    && cs
                        .get(i + 1)
                        .is_some_and(|n| n.is_ascii_digit() || *n == '-' || *n == '+') =>
            {
                out.push_str(" times 10 to the ");
                i += 1;
                if cs[i] == '+' {
                    i += 1;
                }
            }
            '+' if cs.get(i + 1) == Some(&'-') => {
                out.push_str(" plus or minus ");
                i += 2;
            }
            '\\' => {
                let mut j = i + 1;
                while j < cs.len() && cs[j].is_ascii_alphabetic() {
                    j += 1;
                }
                let name: String = cs[i + 1..j].iter().collect();
                match name.as_str() {
                    "pm" => out.push_str(" plus or minus "),
                    "times" => out.push_str(" times "),
                    _ => {}
                }
                i = j.max(i + 1);
            }
            'x' | '×' => {
                out.push_str(" times ");
                i += 1;
            }
            // '(' is an uncertainty in brackets: 1.234(5)
            '±' | '(' => {
                out.push_str(" plus or minus ");
                i += 1;
            }
            '{' | '}' | '~' | ')' => i += 1,
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whether the number is exactly one, so that the unit stays singular.
pub(crate) fn is_one(num: &str) -> bool {
    let t = num.trim().trim_start_matches('+');
    t.parse::<f64>()
        .is_ok_and(|v| (v - 1.0).abs() < f64::EPSILON)
}

/// `\SI{num}{unit}`
pub(crate) fn speak_quantity(num: &str, unit: &str) -> String {
    let one = is_one(num);
    let n = speak_number(num);
    let u = speak_unit(unit, !one);
    if u.is_empty() { n } else { format!("{n} {u}") }
}

/// `\SIrange{a}{b}{unit}`
pub(crate) fn speak_range(a: &str, b: &str, unit: &str) -> String {
    let u = speak_unit(unit, true);
    let r = format!("{} to {}", speak_number(a), speak_number(b));
    if u.is_empty() { r } else { format!("{r} {u}") }
}

/// `\ang{12.5}` or `\ang{1;2;3}`
pub(crate) fn speak_angle(a: &str) -> String {
    let parts: Vec<&str> = a.split(';').collect();
    let names = [
        ("degree", "degrees"),
        ("arcminute", "arcminutes"),
        ("arcsecond", "arcseconds"),
    ];
    let mut out = Vec::new();
    for (k, p) in parts.iter().enumerate().take(3) {
        let p = p.trim();
        if p.is_empty() {
            continue;
        }
        let n = if k == 0 && parts.len() == 1 {
            names[0]
        } else {
            names[k]
        };
        out.push(format!(
            "{} {}",
            speak_number(p),
            if is_one(p) { n.0 } else { n.1 }
        ));
    }
    out.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metres_per_second() {
        assert_eq!(
            speak_quantity("3", r"\metre\per\second"),
            "3 metres per second"
        );
        assert_eq!(
            speak_quantity("1", r"\metre\per\second"),
            "1 metre per second"
        );
        assert_eq!(
            speak_quantity("2.5", r"\kilo\gram\metre\squared\per\second\squared"),
            "2.5 kilogram metres squared per second squared"
        );
    }

    #[test]
    fn short_form_units() {
        assert_eq!(speak_unit("m.s^{-1}", true), "metres per second");
        assert_eq!(speak_unit("kg/m^3", true), "kilograms per metre cubed");
        assert_eq!(speak_unit("MPa", true), "megapascals");
    }

    #[test]
    fn numbers() {
        assert_eq!(speak_number("1.5e-3"), "1.5 times 10 to the -3");
        assert_eq!(speak_number("3 +- 0.1"), "3 plus or minus 0.1");
        assert_eq!(speak_angle("1;2;3"), "1 degree 2 arcminutes 3 arcseconds");
    }
}
