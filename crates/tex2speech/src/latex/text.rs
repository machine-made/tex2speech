// SPDX-License-Identifier: Apache-2.0
//! Tables for text-mode LaTeX: accents, special letters and symbols.

/// Accent commands and the letters they combine with, as pairs "base, result".
const ACCENTS: &[(&str, &str)] = &[
    ("'", "aáeéiíoóuúyýAÁEÉIÍOÓUÚYÝcćnńsśzźCĆNŃSŚZŹlĺrŕLĹRŔ"),
    ("`", "aàeèiìoòuùAÀEÈIÌOÒUÙ"),
    ("^", "aâeêiîoôuûAÂEÊIÎOÔUÛcĉgĝhĥjĵsŝwŵyŷ"),
    ("\"", "aäeëiïoöuüyÿAÄEËIÏOÖUÜY\u{178}"),
    ("~", "aãnñoõAÃNÑOÕiĩuũIĨUŨ"),
    ("c", "cçsştţCÇSŞTŢ"),
    ("v", "cčdďeěnňrřsštťzžCČDĎEĚNŇRŘSŠTŤZŽ"),
    ("u", "aăgğeĕiĭoŏuŭAĂGĞEĔIĬOŎUŬ"),
    ("=", "aāeēiīoōuūAĀEĒIĪOŌUŪ"),
    (".", "zżcċeėgġZŻCĊEĖGĠI\u{130}"),
    ("r", "aåuůAÅUŮ"),
    ("k", "aąeęiįuųAĄEĘIĮUŲ"),
    ("H", "oőuűOŐUŰ"),
];

/// Letters and symbols written as a bare command in text.
const SYMBOLS: &[(&str, &str)] = &[
    ("ss", "ß"),
    ("o", "ø"),
    ("O", "Ø"),
    ("ae", "æ"),
    ("AE", "Æ"),
    ("oe", "œ"),
    ("OE", "Œ"),
    ("aa", "å"),
    ("AA", "Å"),
    ("l", "ł"),
    ("L", "Ł"),
    ("i", "i"),
    ("j", "j"),
    ("dh", "ð"),
    ("DH", "Ð"),
    ("th", "þ"),
    ("TH", "Þ"),
    ("ng", "ŋ"),
    ("LaTeX", "LaTeX"),
    ("LaTeXe", "LaTeX"),
    ("TeX", "TeX"),
    ("BibTeX", "BibTeX"),
    ("XeTeX", "XeTeX"),
    ("ldots", "\u{2026}"),
    ("dots", "\u{2026}"),
    ("textellipsis", "\u{2026}"),
    ("cdots", "\u{2026}"),
    ("textendash", "-"),
    ("textemdash", ", "),
    ("textquoteleft", "'"),
    ("textquoteright", "'"),
    ("textquotedblleft", ""),
    ("textquotedblright", ""),
    ("lq", "'"),
    ("rq", "'"),
    ("guillemotleft", ""),
    ("guillemotright", ""),
    ("textdegree", " degrees "),
    ("degree", " degrees "),
    ("texttimes", " times "),
    ("textpm", " plus or minus "),
    ("textmu", " micro "),
    ("textless", " less than "),
    ("textgreater", " greater than "),
    ("textbar", " bar "),
    ("textbackslash", " backslash "),
    ("textasciitilde", " tilde "),
    ("textasciicircum", " caret "),
    ("textunderscore", " "),
    ("textregistered", " registered "),
    ("texttrademark", " trademark "),
    ("textcopyright", " copyright "),
    ("copyright", " copyright "),
    ("pounds", " pounds "),
    ("euro", " euros "),
    ("textdollar", " dollars "),
    ("textsection", " section "),
    ("S", " section "),
    ("P", " paragraph "),
    ("textbullet", ""),
    ("dag", ""),
    ("ddag", ""),
    ("textperthousand", " per mille "),
    ("textpercent", " percent "),
    ("&", " and "),
    ("%", " percent "),
    ("#", " number "),
    ("_", " "),
    ("{", " "),
    ("}", " "),
    ("$", " dollars "),
    (" ", " "),
    (",", "\u{2009}"),
    (";", " "),
    (":", " "),
    ("!", ""),
    ("/", ""),
    ("-", ""),
    ("@", ""),
    ("\\", " "),
    ("|", ""),
    ("*", ""),
    ("par", "\n\n"),
    ("and", " and "),
    ("quad", " "),
    ("qquad", " "),
    ("enspace", " "),
    ("space", " "),
    ("newline", " "),
    ("tab", " "),
];

/// Compose an accent with a base letter. Unknown pairs give the base letter.
pub(super) fn accent(acc: &str, base: char) -> Option<char> {
    let (_, table) = ACCENTS.iter().find(|(a, _)| *a == acc)?;
    let cs: Vec<char> = table.chars().collect();
    for pair in cs.chunks(2) {
        if pair.len() == 2 && pair[0] == base {
            return Some(pair[1]);
        }
    }
    Some(base)
}

pub(super) fn is_accent(name: &str) -> bool {
    ACCENTS.iter().any(|(a, _)| *a == name)
}

/// Accents written with a letter name (`\c`, `\v`) rather than a symbol.
pub(super) fn is_letter_accent(name: &str) -> bool {
    matches!(name, "c" | "v" | "u" | "r" | "k" | "H")
}

pub(super) fn symbol(name: &str) -> Option<&'static str> {
    SYMBOLS.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}
