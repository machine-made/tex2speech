# tex2speech

Turns LaTeX and Markdown documents into text that a speech model can read aloud. Maths is spoken the way a
person would say it. Preamble, comments, code blocks, table cells and the bibliography are not spoken.

Status: the English engine works. `tex2speech FILE` turns a `.tex` or `.md` file into one sentence per line, `--json` gives segments with their source positions, and `-` with `--format tex|md` reads standard input. The benchmark transcripts in `bench/` match exactly. Portuguese and French are not read yet. `tex2speech --version` prints the version.

## Planned use

- A Rust library that other programs link. It takes a whole file, a paragraph or a sentence.
- A program, `tex2speech`, that prints plain text or JSON.
- No network access and no configuration for the default run.

## Build

Rust 1.99 or later.

```
cargo build --release
cargo test
```

## Licence

Apache-2.0. See `LICENSE` and `NOTICE`.
