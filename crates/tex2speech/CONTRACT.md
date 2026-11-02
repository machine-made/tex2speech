# Contract: tex2speech

Status: G1 for English. Property tests (generated input, truncated documents, hostile nesting) and
differential tests (the MathCAT fork against upstream, rule edits listed in `RULE_EDITS.md`) exist. No sanitizer or
mutation run has been made, so no claim above G1.

## Invariants

- The output holds only what a reader sees in the rendered document. Preamble, packages, macro
  definitions, comments and other source-only content are never spoken.
- Page numbers, running headers, table of contents, table cells, bibliography and code blocks are
  skipped.
- No LaTeX or Markdown markup reaches the output.
- Library code does not touch the network and does not print.

## Input

- Bytes may be UTF-8 (a byte-order mark counts as three spaces, so offsets stay in the file), UTF-16 with a mark, or an
  8-bit file. An 8-bit file is read with the encoding of `\usepackage[...]{inputenc}` when it is Latin-1, Latin-9 or
  Windows-1252, and as Windows-1252 (a guess, with an `encoding` warning) otherwise. A file that is valid UTF-8 is read
  as UTF-8 whatever it declares. A binary file reads as empty text with a warning.
- Line endings may be `\n`, `\r\n` or `\r`. A `\r` that does not start `\r\n` is read as `\n`, one byte for one byte, so span offsets still index the file; lines count `\n`.
- Span offsets index the decoded text: the file itself for UTF-8, with or without a mark; the converted text for
  UTF-16 and 8-bit files.
- Paths: `/` and `\` both separate in `\input{...}`. `Document::select` compares paths by their parts, so `./a.tex`,
  `A.tex` with a drive letter or share in either spelling (case ignored there), `\\?\` prefixes, `..` and repeated
  separators name one file, and a path that is the tail of the other matches. In the JSON a path is escaped.

- Files: `\input`, `\include`, a local class or package and a `.bbl` are read only from inside the root folder
  (`Options::base_dir`; `parse_file` sets it to the folder of the file). A name that is absolute (`/x`, `C:/x`, `~`), or
  that reaches outside by `..` or by a link, is refused with an `input` warning, and the text is not read. Without a
  base folder (`parse_str`, standard input) no file is read. A caller that needs a wider root sets `base_dir` to it.

## Error model

Bad input never stops a run. Unknown or broken constructs produce a warning and a fallback reading.
Errors returned to a caller are typed: only a file that cannot be read is an error. A panic inside the reader gives an
empty document with one `internal` warning, never a panic in the caller.

## Determinism class

Bit-stable. The same input, options and crate version give the same bytes, across runs and threads.

## Unsafe boundary

None. `unsafe_code` is denied at workspace level.

## Threads, stack and environment

- Equations are read by `MathCAT`, which keeps global state, on pool threads started at the first equation (up to 8, one
  more per 80 distinct equations). Output does not depend on the count; `Options::with_threads(n)` fixes it.
- The reader runs on the caller's thread. Nesting of groups, environments and macro calls is limited to 32 levels, and an
  equation to 64 levels of braces, `\left` and `\begin`; a deeper construct gives a `nesting` warning and a fallback
  reading. At these limits the reader needs less than 512 KiB of stack in a release build and less than 1 MiB without
  optimisation (checked on hostile input), so the default 2 MiB thread stack is enough.
- Nothing reads the environment.

## No-claim boundaries

- No claim that a reading is correct beyond the checked-in benchmark texts.
- No claim on PDF input: a PDF is read only through its `.tex` source.
- No claim on unusual TeX such as `\if` and catcode changes: `\newif` flags, `\iftrue`, `\iffalse`, `\ifx`,
  `\ifdefined`, `\ifnum` and `\ifodd` on written-out numbers are evaluated, every other conditional takes its `\else`
  branch and is reported.
- A definition that uses TeX internals (`\@...`, `\global`, `\halign`, `$$`) is not read as text: the command keeps its
  built-in reading, or is read as unknown.
- A text with no `\documentclass` and no `\begin{document}` (a fragment) reads a run of symbols around a maths command
  as an equation (`\beta = 43000`). A word of two letters ends the run. A fragment with maths that holds no command, such
  as `x = 1`, stays prose.
- English prose only.
- No claim for 8-bit encodings other than Latin-1, Latin-9 and Windows-1252: other declared ones read as Windows-1252.
- Windows is tested in CI only; no Windows machine was available to the author.
- No speed claim until baselines exist in CI.
