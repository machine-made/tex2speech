# tex2speech-mathcat

MathCAT (MathML to speech) with English rules edited for tex2speech. The library name is
`tex2speech_mathcat`, so it can sit next to upstream `mathcat` (`libmathcat`) in one test.
Upstream source and commit: see `NOTICE`. Licence: MIT, as upstream (`LICENSE`).

- Feature `include-zip` (rules embedded in the library, no file is read at run time) is on by default.
  The safe build of the XML crates is always on; there is no `no-unsafe` feature.
- `RULE_EDITS.md` lists every change to a rule file, with the old and new reading.
- `tests/bench_equations.rs` holds the equation table, the readings that rule edits do not reach, and a
  test that prints the fork and upstream readings side by side.
- MathCAT keeps its state per thread. Call `set_rules_dir` once on the thread that speaks.

## Upstream difference: git tag against crates.io

The source here is git tag v0.7.7, not the crates.io package of the same version; the two differ.

- In the kept `Rules` folders, three files differ: `Rules/Intent/general.yaml` (the tag has a `ratio`
  rule and strips U+2061 in `NoSpacesBase`), `Rules/Intent/linear-algebra.yaml` (the tag has the
  norm rules for a subscript on the closing bar) and `Rules/Languages/en/unicode-full.yaml` (the
  tag has the `·`, `×` and `÷` entries and a few more). `fr`, `pt`, `Nemeth` and the top-level files are the same.
  Rules of removed languages and braille sets differ in 18 more files.
- The crates.io manifest has none of the `lang-*` and `braille-*` features that the tag has.
- The tag's `Cargo.toml` still says `0.7.6-rc.4`.
