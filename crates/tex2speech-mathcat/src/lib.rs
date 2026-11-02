// The modules below are upstream MathCAT, kept as they are so that its changes can be merged by hand; they are
// not formatted with this workspace's rustfmt settings (rustfmt::skip on each module declaration).
// This is a fork of upstream MathCAT, kept close to upstream so its changes can be merged by hand.
// Upstream source does not follow this workspace's pedantic clippy profile, and restyling it would
// make every merge conflict. The allow stays at crate level for that reason only; new code here
// follows the workspace lints. `unsafe_code` stays denied.
#![allow(clippy::all, clippy::pedantic)]

//! A library for generating speech and braille from MathML
//!
//! Typical usage is:
//! 1. Set the rules directory [`set_rules_dir`]
//! 2. Set whatever preferences are need with repeated calls to [`set_preference`].
//! 3. Set MathML via [`set_mathml`]
//!    A string representing the cleaned up MathML along with `id`s on each node is returned for highlighting if desired
//! 4. Get the speech [`get_spoken_text`] or (Unicode) braille [`get_braille`].
//!
//! The expression can be navigated also.
//! This is done in one of two ways:
//! 1. Pass key strokes to allow a user to navigate the MathML by calling [`do_navigate_keypress`]; the speech is returned.
//! 2. Pass the MathCAT navigation command directory by called [`do_navigate_command`]; the speech is return returned.
//!
//! To get the MathML associated with the current navigation node, call [`get_navigation_mathml`].
//! To just get the `id` and offset from the id of the current navigation node, call [`get_navigation_mathml_id`].
///
/// This module re-exports anyhow types. Use `bail!` for early returns and
/// `context()`/`with_context()` on Result to add context (replacing old `chain_err()`).
pub mod errors {
    pub use anyhow::{Context, Error, Result, anyhow, bail};
}

#[rustfmt::skip]
pub mod interface;
#[cfg(feature = "include-zip")]
pub use shim_filesystem::ZIPPED_RULE_FILES;

#[rustfmt::skip]
mod canonicalize;
#[rustfmt::skip]
mod infer_intent;
#[rustfmt::skip]
pub mod speech;
#[rustfmt::skip]
mod braille;
#[rustfmt::skip]
mod navigate;
#[rustfmt::skip]
mod prefs;
#[rustfmt::skip]
mod tts;
#[rustfmt::skip]
mod xpath_functions;
#[rustfmt::skip]
mod definitions;
#[rustfmt::skip]
pub mod pretty_print;
#[rustfmt::skip]
mod chemistry;

#[rustfmt::skip]
pub mod shim_filesystem; // really just for override_file_for_debugging_rules, but the config seems to throw it off
use crate::errors::{Result, bail};
pub use interface::*;

#[cfg(test)]
pub fn init_logger() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .is_test(true)
        .format_timestamp(None)
        .format_module_path(false)
        .format_indent(None)
        .format_level(false)
        .init();
}

/// Build Absolute path to rules dir for testing
pub fn abs_rules_dir_path() -> String {
    cfg_if::cfg_if! {
    if #[cfg(feature = "include-zip")] {
          return "Rules".to_string();
    } else {
        // Package root (see tests/common/mod.rs `abs_rules_dir_path` for rationale).
        return std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("Rules")
            .to_str()
            .expect("CARGO_MANIFEST_DIR and Rules path must be UTF-8")
            .to_string();
        }
    }
}

pub fn are_strs_canonically_equal_with_locale(
    test: &str,
    target: &str,
    ignore_attrs: &[&str],
    block_separators: &str,
    decimal_separators: &str,
) -> Result<()> {
    use crate::canonicalize::canonicalize;
    use crate::{interface::*, pretty_print::mml_to_string};
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use sxd_document_no_unsafe::parser;

    crate::interface::init_panic_handler();
    let result = catch_unwind(AssertUnwindSafe(|| {
        // this forces initialization
        crate::interface::set_rules_dir(abs_rules_dir_path()).unwrap();
        set_preference("Language", "en").unwrap();
        set_preference("BlockSeparators", block_separators).unwrap();
        set_preference("DecimalSeparators", decimal_separators).unwrap();
        crate::speech::SPEECH_RULES.with(|rules| rules.borrow_mut().read_files().unwrap());

        let package1 = &parser::parse(test).expect("Failed to parse test input");
        let mathml = get_element(package1);
        trim_element(mathml, false);
        let mathml_test = canonicalize(mathml).unwrap();

        let package2 = &parser::parse(target).expect("Failed to parse target input");
        let mathml_target = get_element(package2);
        trim_element(mathml_target, false);

        match is_same_element(mathml_test, mathml_target, ignore_attrs) {
            Ok(_) => Ok(()),
            Err(e) => {
                bail!(
                    "{}\nResult:\n{}\nTarget:\n{}",
                    e,
                    mml_to_string(mathml_test),
                    mml_to_string(mathml_target)
                );
            }
        }
    }));
    match crate::interface::report_any_panic(result) {
        Ok(()) => Ok(()),
        Err(e) => Err(e),
    }
}

/// sets locale to be US standard
pub fn are_strs_canonically_equal(test: &str, target: &str, ignore_attrs: &[&str]) -> bool {
    are_strs_canonically_equal_with_locale(test, target, ignore_attrs, ", \u{00A0}\u{202F}", ".").is_ok()
}

/// Like `are_strs_canonically_equal` but returns `Result` for use in `#[test]` functions that return `Result<()>`.
pub fn are_strs_canonically_equal_result(test: &str, target: &str, ignore_attrs: &[&str]) -> Result<()> {
    are_strs_canonically_equal_with_locale(test, target, ignore_attrs, ", \u{00A0}\u{202F}", ".")
}
