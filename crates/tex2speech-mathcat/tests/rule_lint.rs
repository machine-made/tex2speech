//! Checks on the rule files that no engine run would show.

use std::path::Path;

fn yaml_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).expect("rules folder").flatten() {
        let p = e.path();
        if p.is_dir() {
            yaml_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "yaml") {
            out.push(p);
        }
    }
}

/// In `XPath` a name without quotes is a child element. `$Verbosity != Terse` compares with an empty node set and
/// is always false, so a preference value must be written in quotes.
#[test]
fn preference_values_are_quoted() {
    let mut files = Vec::new();
    // English and the shared rules. The French and Portuguese `unicode.yaml` still hold `$Verbosity != Terse`; an
    // upstream French test encodes the Terse output that follows from it, so they are left as upstream wrote them.
    let rules = Path::new(env!("CARGO_MANIFEST_DIR")).join("Rules");
    yaml_files(&rules.join("Languages/en"), &mut files);
    yaml_files(&rules.join("Intent"), &mut files);
    assert!(files.len() > 5, "{} rule files", files.len());
    let mut bad = Vec::new();
    for f in files {
        let text = std::fs::read_to_string(&f).expect("reads");
        for (n, line) in text.lines().enumerate() {
            let code = line.split('#').next().unwrap_or("");
            for key in ["$Verbosity", "$SpeechStyle", "$Impairment", "$Language"] {
                let mut rest = code;
                while let Some(p) = rest.find(key) {
                    rest = &rest[p + key.len()..];
                    let after = rest
                        .trim_start()
                        .trim_start_matches(['!', '='])
                        .trim_start();
                    let is_cmp = rest.trim_start().starts_with(['=', '!']);
                    if is_cmp && after.starts_with(|c: char| c.is_ascii_alphabetic()) {
                        bad.push(format!("{}:{}: {}", f.display(), n + 1, line.trim()));
                    }
                }
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
