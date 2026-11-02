// SPDX-License-Identifier: Apache-2.0
//! Turn the bytes of a source file into text: UTF-8, UTF-16 with a byte-order mark, or the 8-bit encodings of older
//! TeX files (Latin-1, Latin-9, Windows-1252). Never fails; a guess or a repair comes back as a note.

/// Text decoded from bytes, and what the decoder had to guess or repair.
pub(crate) struct Decoded {
    pub text: String,
    pub note: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Declared {
    Utf8,
    Latin1,
    Latin9,
    Cp1252,
    /// A named encoding this reader has no table for.
    Other,
}

/// Windows-1252 for bytes 0x80 to 0x9F; `\0` marks the five bytes it leaves undefined.
const CP1252_HIGH: [char; 32] = [
    '\u{20AC}', '\0', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\0', '\u{017D}', '\0', '\0',
    '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}', '\u{02DC}',
    '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}', '\0', '\u{017E}', '\u{0178}',
];

fn eight_bit(bytes: &[u8], latin9: bool) -> String {
    bytes
        .iter()
        .map(|&b| match b {
            0x80..=0x9F => match CP1252_HIGH[usize::from(b - 0x80)] {
                '\0' => ' ',
                c => c,
            },
            0xA4 if latin9 => '\u{20AC}',
            0xA6 if latin9 => '\u{0160}',
            0xA8 if latin9 => '\u{0161}',
            0xB4 if latin9 => '\u{017D}',
            0xB8 if latin9 => '\u{017E}',
            0xBC if latin9 => '\u{0152}',
            0xBD if latin9 => '\u{0153}',
            0xBE if latin9 => '\u{0178}',
            _ => char::from(b),
        })
        .collect()
}

/// The encoding named by `\usepackage[...]{inputenc}` or `\inputencoding{...}` in the first bytes, with its name.
fn declared(bytes: &[u8]) -> Option<(Declared, String)> {
    let head = &bytes[..bytes.len().min(32 * 1024)];
    let s: String = head
        .iter()
        .map(|&b| if b.is_ascii() { char::from(b) } else { '?' })
        .collect();
    let mut found = None;
    for (key, is_option) in [("{inputenc}", true), ("\\inputencoding{", false)] {
        let mut from = 0;
        while let Some(p) = s[from..].find(key) {
            let at = from + p;
            from = at + key.len();
            let line_start = s[..at].rfind('\n').map_or(0, |i| i + 1);
            if s[line_start..at].contains('%') {
                continue;
            }
            let names = if is_option {
                let before = &s[line_start..at];
                match (before.rfind('['), before.rfind(']')) {
                    (Some(a), Some(z)) if a < z && before[..a].contains("usepackage") => {
                        before[a + 1..z].to_string()
                    }
                    _ => continue,
                }
            } else {
                s[from..].split('}').next().unwrap_or_default().to_string()
            };
            for name in names.split(',') {
                let name = name.trim().to_ascii_lowercase();
                let kind = match name.as_str() {
                    "utf8" | "utf8x" | "utf-8" => Declared::Utf8,
                    "latin1" | "iso-8859-1" | "iso8859-1" => Declared::Latin1,
                    "latin9" | "latin-9" | "iso-8859-15" | "iso8859-15" => Declared::Latin9,
                    "cp1252" | "ansinew" | "windows-1252" => Declared::Cp1252,
                    "latin2" | "latin3" | "latin4" | "latin5" | "latin10" | "cp1250" | "cp1251"
                    | "cp1253" | "cp1254" | "cp437" | "cp850" | "cp852" | "cp858" | "cp865"
                    | "cp866" | "applemac" | "macce" | "next" | "decmulti" | "koi8-r"
                    | "koi8-u" | "cp1257" | "cp437de" | "cp855" | "cp860" | "cp862" | "cp863"
                    | "cp1255" | "cp1256" | "cp1258" | "cp737" | "cp775" | "cp857" | "cp861"
                    | "cp869" | "cp874" => Declared::Other,
                    _ => continue,
                };
                found = Some((kind, name));
            }
        }
    }
    found
}

/// A file that is not text: a NUL byte, or many control characters in the first bytes.
fn looks_binary(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(8192)];
    if head.is_empty() {
        return false;
    }
    if head.contains(&0) {
        return true;
    }
    let control = head
        .iter()
        .filter(|&&b| (b < 0x20 && !matches!(b, b'\t' | b'\n' | b'\r' | 0x0C | 0x1B)) || b == 0x7F)
        .count();
    control * 20 > head.len()
}

fn utf16(bytes: &[u8], little: bool) -> String {
    let (pairs, _) = bytes[2..].as_chunks::<2>();
    let units = pairs.iter().map(|&p| {
        if little {
            u16::from_le_bytes(p)
        } else {
            u16::from_be_bytes(p)
        }
    });
    char::decode_utf16(units)
        .map(|r| r.unwrap_or(' '))
        .collect()
}

/// Turn each carriage return that does not start `\r\n` into `\n`, so that old Mac files have lines. Every
/// replacement is one byte, so span offsets stay valid.
pub(crate) fn lone_cr_to_lf(s: &str) -> std::borrow::Cow<'_, str> {
    if !s.contains('\r') {
        return std::borrow::Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        out.push(if c == '\r' && it.peek() != Some(&'\n') {
            '\n'
        } else {
            c
        });
    }
    std::borrow::Cow::Owned(out)
}

/// Decode a source file.
///
/// A UTF-8 byte-order mark becomes three spaces, so that every byte offset still points into the file. A file that
/// is valid UTF-8 is read as UTF-8, whatever it declares. Otherwise the declared 8-bit encoding is used; with no
/// declaration the bytes are read as Windows-1252 (a superset of Latin-1 for printable text) and the note says so.
/// A file that holds mostly valid UTF-8 with a few bad bytes keeps its UTF-8 text and loses the bad bytes.
pub(crate) fn decode(bytes: &[u8]) -> Decoded {
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        return Decoded {
            text: utf16(bytes, bytes[0] == 0xFF),
            note: Some("UTF-16 converted to UTF-8; byte offsets index the converted text".into()),
        };
    }
    if looks_binary(bytes) {
        return Decoded {
            text: String::new(),
            note: Some("the file is not text (binary data); nothing was read".into()),
        };
    }
    let (bom, body) = match bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        Some(rest) => ("   ", rest),
        None => ("", bytes),
    };
    let finish = |text: String, note: Option<String>| Decoded {
        text: format!("{bom}{}", lone_cr_to_lf(&text)),
        note,
    };
    if let Ok(s) = std::str::from_utf8(body) {
        return finish(s.to_string(), None);
    }
    let lossy = String::from_utf8_lossy(body);
    let bad = lossy.matches('\u{FFFD}').count();
    let good = lossy
        .chars()
        .filter(|c| !c.is_ascii() && *c != '\u{FFFD}')
        .count();
    let decl = declared(body);
    match decl {
        Some((Declared::Latin1 | Declared::Cp1252, _)) => finish(eight_bit(body, false), None),
        Some((Declared::Latin9, _)) => finish(eight_bit(body, true), None),
        Some((Declared::Other, name)) => finish(
            eight_bit(body, false),
            Some(format!(
                "inputenc {name} is not supported; the file was read as Windows-1252"
            )),
        ),
        _ if good > bad => finish(
            lossy.replace('\u{FFFD}', " "),
            Some(format!(
                "{bad} invalid UTF-8 byte sequence(s) replaced by spaces"
            )),
        ),
        _ => finish(
            eight_bit(body, false),
            Some(
                "the file is not valid UTF-8 and declares no encoding; guessed Windows-1252".into(),
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_bom_becomes_three_spaces() {
        let d = decode(b"\xEF\xBB\xBFab");
        assert_eq!(d.text, "   ab");
        assert!(d.note.is_none());
    }

    #[test]
    fn declared_latin1_is_used_without_a_note() {
        let d = decode(b"\\usepackage[latin1]{inputenc}\ncaf\xE9 \x93q\x94");
        assert!(d.text.contains("caf\u{e9} \u{201c}q\u{201d}"), "{}", d.text);
        assert!(d.note.is_none());
    }

    #[test]
    fn latin9_has_the_euro() {
        let d = decode(b"\\usepackage[latin9]{inputenc}\n\xA4");
        assert!(d.text.ends_with('\u{20AC}'));
    }

    #[test]
    fn a_guess_is_reported() {
        let d = decode(b"caf\xE9");
        assert_eq!(d.text, "caf\u{e9}");
        assert!(d.note.is_some_and(|n| n.contains("guessed")));
    }

    #[test]
    fn a_commented_declaration_does_not_count() {
        let d = decode(b"% \\usepackage[latin1]{inputenc}\ncaf\xE9");
        assert!(d.note.is_some());
    }

    #[test]
    fn a_few_bad_bytes_keep_the_utf8_text() {
        let mut v = "é è ê ë ï ô ö ü ç à ".as_bytes().to_vec();
        v.push(0xFF);
        let d = decode(&v);
        assert!(d.text.starts_with("é è"));
        assert!(d.note.is_some());
    }

    #[test]
    fn nul_bytes_mean_binary() {
        let d = decode(b"%PDF-1.4\n\x00\x01\x02\xff\xfe");
        assert_eq!(d.text, "");
        assert!(d.note.is_some());
    }

    #[test]
    fn utf16_with_a_mark_is_converted() {
        let d = decode(b"\xFF\xFEa\x00b\x00");
        assert_eq!(d.text, "ab");
    }
}
