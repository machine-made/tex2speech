//! Builds `rules.zip` from the `Rules` folder when the `include-zip` feature is on.
//! The library embeds the archive, so no rule file is read at run time.
// rules_archive.rs is upstream code (included below); it does not follow the workspace pedantic profile, and
// restyling it would make every upstream merge conflict.
#![allow(clippy::all, clippy::pedantic)]
#![allow(dead_code)] // include! of rules_archive.rs pulls in helpers this build does not call

include!("src/rules_archive.rs");

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=src/rules_archive.rs");
    println!("cargo::rerun-if-changed=Rules");

    if std::env::var("CARGO_FEATURE_INCLUDE_ZIP").is_ok() {
        let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        let rules_dir =
            PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("Rules");
        let staging = out_dir.join("rules_src");
        if staging.exists() {
            let _ = std::fs::remove_dir_all(&staging);
        }
        if let Err(e) = copy_rules_tree(&rules_dir, &staging, true) {
            panic!("build.rs failed to stage minimized Rules: {}", e);
        }
        let archive_path = out_dir.join("rules.zip");
        if let Err(e) = write_rules_archive(&staging, &archive_path, include_zip_compression()) {
            panic!("build.rs failed to write {}: {}", archive_path.display(), e);
        }
    }
}
