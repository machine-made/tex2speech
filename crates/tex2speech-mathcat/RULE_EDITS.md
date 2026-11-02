# Rule edits

Every change to a file under `Rules/` against upstream MathCAT at git tag v0.7.7 (commit 764d24d1).
Settings used for every reading below: language `en`, speech style `ClearSpeak`, `TTS` `None`,
MathML from math-core 0.9. The speech rules edited are English only (`Rules/Languages/en`); `fr` and `pt` speech rules are untouched.
Rows 34 to 50, 54, 55, 59, 60 and 71 also edit `Rules/Intent/general.yaml` and `Rules/Intent/linear-algebra.yaml`, and rows 56, 72 and 73 edit
`Rules/definitions.yaml` and `unicode*.yaml`; row 55 edits `src/canonicalize.rs`. Every language reads these. Rows 54, 55 and 61 change `fr` and `pt`
test strings (listed under the upstream tests changed); row 60 adds an intent name that only `Languages/en/definitions.yaml` defines, so
`\det A` in French or Portuguese is read by the fallback.

Files are CRLF, as upstream. A rule in `ClearSpeak_Rules.yaml` applies to ClearSpeak only. A rule
in `unicode.yaml`, `unicode-full.yaml` or `definitions.yaml` applies to ClearSpeak and SimpleSpeak.

## unicode.yaml

| # | Edit | Old reading | New reading | Why |
|---|---|---|---|---|
| 1 | `=`: "is equal to" becomes "equal to" | beta is equal to 43000 | beta equal to 43000 | table row 1; applies to every equation |
| 2 | Relations lose the leading "is": `<` `>` `≤` `≥` `≠` `≡` `∦` `∝` `∶` `∾` `⊄` `⊅` | x is less than or equal to y | x less than or equal to y | same wording as 1 |
| 3 | "cap" becomes "capital", for Latin and Greek capitals | cap u | capital U | table rows 3, 8, 14, 15 |
| 4 | With `TTS` `None` a capital letter keeps its case (upstream lower-cases it, and says "eigh" for A) | cap eigh | capital A | table rows 3, 14, 15 |
| 5 | The beep branch for capital letters is removed. It left an empty piece that put a second space in the text | rho  cap a | rho capital A | text must have single spaces |
| 6 | `∈` and `∊` outside a set, preference `Auto`: "is a member of" becomes "in" | x is a member of S | x in S | table row 18; the `Member` preference still says "is a member of" |
| 7 | `∂` as a character: "partial derivative" becomes "partial" | partial derivative u | partial u | table rows 4, 5 |

## unicode.yaml, added with the English engine

| # | Edit | Old reading | New reading | Why |
|---|---|---|---|---|
| 74 | `/` (outside literal intent): "divided by" becomes "over" | capital U capital D divided by nu | capital U capital D over nu | `bench/bench.md`, line `$Re = U D / \nu$`; `÷` still says "divided by" |

## Added with the English engine

| # | Edit | Old reading | New reading | Why |
|---|---|---|---|---|
| 75 | unicode-full.yaml: the 72 `$Verbosity!='Terse'` tests that say "is" before relations (`≈`, `≅`, `≪`, ...) are removed | x is approximately equal to y | x approximately equal to y | same wording as 1 and 2 |
| 76 | ClearSpeak `indexed-by-plain`: also for a subscript that is a run of letters, numbers and commas | R sub c comma 1 end sub | R c comma 1 | `Re_{c,1}` |

`cases` was read as an array at this stage; edit 43 fixes it.

## unicode-full.yaml

| # | Edit | Old reading | New reading | Why |
|---|---|---|---|---|
| 8 | The 52 `not($IgnoreBold)` tests that say "bold" before bold letters are removed | bold u | u | table row 4 (`\mathbf{u}`) |
| 9 | `∇`: "nahblah" becomes "grad" | nahblah | grad | table row 4 (`(\mathbf{u}\cdot\nabla)\mathbf{u}`) |

## definitions.yaml

| # | Edit | Old reading | New reading | Why |
|---|---|---|---|---|
| 10 | `KnownWords` gains kin, pot, ref, rms, tot (runs of single-letter identifiers that are words) | E sub k i n | E kin (with rule 25) | table row 15; a vocabulary list is meant to grow |

## ClearSpeak_Rules.yaml: changed rules

| # | Rule | Old reading | New reading | Why |
|---|---|---|---|---|
| 11 | `default` (square-root) and `default` (root): no "the" | the square root of pi | square root of pi | table row 7 |
| 12 | `default` (square-root): no pause after a simple radicand | the square root of pi, over 2 | square root of pi over 2 | table row 7 |
| 13 | `negative_and_positive`: a prefix minus whose parent is a power says "minus" | e raised to the negative x squared power | e to the minus x squared | table rows 7, 9; outside exponents it still says "negative" |
| 14 | `fraction-over-simple`: "over" also for fractions with no sum, difference or nested fraction inside | the fraction with numerator U D and denominator nu | U D over nu | table rows 3, 7; fractions with a sum stay in the long form |
| 15 | `simple-negative-integer`: no "power" | 10 to the negative 3 power | 10 to the minus 3 (with 13) | table row 9 |
| 16 | `default` (power): "to the", no "power" | t raised to the 3 power | t to the 3 | table rows 7, 9 |
| 17 | `ClearSpeak-times`: said also when the next sibling is bracketed and the product is only a guess; after a bracketed group only when the group or the next element is a number; short pause after "times" before a bracket | a, open paren x plus 1, close paren | a times, x plus 1 | table rows 4, 11 |
| 18 | `no-say-parens`: parentheses and brackets are silent unless they hold a sum, a difference or an equation that is not a function argument (chemistry keeps the old condition) | f of, open paren x plus 1, close paren | f of x plus 1 | table rows 4, 10, 11, 12; (a+b)c still says the parentheses |

## ClearSpeak_Rules.yaml: added rules (new names)

| # | Rule | Old reading | New reading | Why |
|---|---|---|---|---|
| 19 | `partial-derivative-fraction` | the fraction with numerator partial u and denominator partial t | partial u, partial t | table rows 4, 5 |
| 20 | `gradient-grad`, `laplacian-plain`: no "of" | gradient of p, laplacian of u | grad p, laplacian u | table row 4 |
| 21 | `dot-product-word` (leaf `dot-product`) | u dot product grad | u dot grad | table row 4 |
| 22 | `divergence-nabla-dot`: `∇ · u` as a divergence | nahblah dot product bold u | divergence of u | table row 6 |
| 23 | `order-symbol`, `order-no-of`: script O applied to an argument | 𝒪 of, open paren h squared close paren | order h squared | table row 12 |
| 24 | `indexed-by-plain`: a subscript that is a letter or a number is said without "sub" | lambda sub max end sub | lambda max | table rows 8, 9; a compound subscript keeps "sub ... end sub" |
| 25 | (with 10) `kin` is one identifier | capital E sub k i n | capital E kin | table row 15 |
| 26 | `say-super-silent`: no "sub" and "super" in a scripted symbol | u sub tau, super plus | u tau plus | table row 13 |
| 27 | `number-set-power`: R^n | x is a member of the real numbers to the n-th power | x in R n | table row 18 |
| 28 | `subscripted-norm-from-magnitude`: the canonical form of the p norm | magnitude of x sub 2 | two norm of x | table row 17; subscripts 1 and 2 are written as words, any other subscript is spoken as it is |
| 29 | `bigop-both-plain`, `bigop-under-plain`: no "the", no pause | the integral from 0, to infinity of | integral from 0 to infinity of | table rows 7, 8 |
| 30 | `matrix-with-rows`, `matrix-row`, `matrix-cell`: a matrix of two rows, simple entries, default `ClearSpeak_Matrix`, no row or column lines | the 2 by 2 matrix; row 1; a, b; row 2; c, d | the 2 by 2 matrix with rows a, b, and c, d | table row 16; other matrices keep the upstream reading |

## ClearSpeak_Rules.yaml: rules that replace an upstream rule of the same name

MathCAT replaces a rule when a later file defines the same name for the same tag. Both rules are at
the end of `ClearSpeak_Rules.yaml`, after the includes, so they replace the upstream rule for
ClearSpeak only.

| # | Rule | Old reading | New reading | Why |
|---|---|---|---|---|
| 31 | `default` (mrow): no automatic pause between the parts | y equal to, U D | y equal to U D | The automatic pause depends on the byte length of the neighbouring phrases, so `y = U D` got a comma and `y = a b` did not. Table rows 3, 9, 14, 18 have no comma. It causes most of the changed ClearSpeak test strings (commas and semicolons) Edit 33 puts pauses back for long equations |
| 32 | `silent-intent`: no pause after a silent intent | m dot, equal to | m dot equal to | table row 14 |
| 33 | `default` (mrow), same rule as 31: for a long equation only, a pause before and after the relation of the outermost mrow, and before each binary plus or minus of the outermost mrow or of its direct children (the sides of the relation) | partial u, partial t plus u dot grad u equal to minus grad p plus nu laplacian u | partial u, partial t, plus u dot grad u, equal to, minus grad p, plus nu laplacian u | table rows 4, 7. "Long" is `LongEquation` = 18: the number of elements inside the outermost mrow must exceed it. Table rows have 3 to 14 elements when short (13 for the largest with a relation) and 23 and 35 for rows 7 and 4. Six upstream test strings in `tests/Languages/en` changed (`hyperbolic_trig_names`, `trig_names`, `test_functions_none_pref`, `explicit_times_none_superscript`, `multiline::case_1`, `shared::ignore_period_and_space`): each is a long equation and gained commas at the relation or at a plus |

## LiteralSpeak_Rules.yaml: added file

`LiteralSpeak` has no rule file in upstream; MathCAT then takes the first `*_Rules.yaml` in the language
folder (`find_any_style_file`), and the listing order changes from run to run. Once ClearSpeak and
SimpleSpeak read differently, the output of `LiteralSpeak` changed from run to run. The file includes
`SimpleSpeak_Rules.yaml`, which fixes the choice.

## Added for reading quality

Every row is a general rule. The readings are `en`, `ClearSpeak`, `TTS` `None`.

| # | File and rule | Old reading | New reading | Why |
|---|---|---|---|---|
| 34 | `Intent/general.yaml`, `interval`: `=` and a following `∈` are no clue; a preceding `∉` is one; a following set relation counts when it is not `∈`, `∊`, `∉` | `(R, Re_D) = (3, 495)`: the interval from R to Re D, not including | R, Re D equal to 3, 495 | a pair is not an interval; `x ∈ (a,b)` and `(a,b) ⊂ S` still are |
| 35 | `ClearSpeak_Rules.yaml`, `ClearSpeak-intervals`: "the open interval from a to b", "the closed interval ...", "the half-open interval ..., including b" | the interval from a to b, not including a or b | the open interval from a to b | shorter, standard words |
| 36 | `Intent/general.yaml`, `modified-var`: any base, not only an identifier | quantity C y with dot above | C y dot | accents on `C_y`, `\bar{u}`, `a+b` |
| 37 | `Intent/general.yaml`, `vector`: `U+20D7` (math-core's `\vec`) on an identifier, or on any base | v with right arrow above embellishment above | vector v | |
| 38 | `SharedRules/default.yaml`: "quantity" removed from `munder`, `mover`, `munderover`; `mover` says "labelled" instead of "with y above" | quantity x plus 1 with y above | x plus 1 labelled y | |
| 39 | `Intent/general.yaml`, `labelled-arrow`, and `ClearSpeak_Rules.yaml`, `labelled` | right arrow with P with empty above above | right arrow labelled P | math-core writes `\xrightarrow{P}` as an arrow over an empty line over P |
| 40 | `Languages/en/definitions.yaml`: `magnitude` says "norm" | magnitude of A | norm of A | `‖A‖`; `|x|` stays "the absolute value of" |
| 41 | `ClearSpeak_Rules.yaml`, `no-say-parens`: a pair or a symbol in parentheses that stands alone has silent parentheses | open paren a close paren | a | panel labels `(a)`; a sum in parentheses still says them |
| 42 | `ClearSpeak_Rules.yaml`, `power-parenthesized-label` | x to the open paren j close paren | x superscript j | `x^{(j)}` is an index |
| 43 | `Intent/general.yaml`, `piecewise-brace-invisible-comma`; `ClearSpeak_Rules.yaml`, `piecewise-plain`, `piecewise-line`, `piecewise-line-end`, `piecewise-value-comma`, `piecewise-cells` | open brace array of; row 1; column 1; a ... | a if x greater than 0, b otherwise | math-core ends `cases` with an invisible comma, so the old rule never matched. A cell that starts with "if", "for", "when", "where", "unless", "otherwise" or "else" gets no second "if". Two upstream tests changed (`multiline::case_1`, `navigate::cases_speech`) |
| 44 | `ClearSpeak_Rules.yaml`, `matrix-with-rows`, `matrix-row`, `matrix-cell`: any number of rows; `array` as well as `matrix`; entries of up to 3 symbols; every row the same length; no labelled row | array of; row 1; column 1; ... (3 rows) | the 3 by 3 matrix with rows a, b, c, and d, e, f, and g, h, i | the 2 by 2 form of the benchmark is unchanged. Determinants keep the long form: the navigation tests read them row by row |
| 45 | `ClearSpeak_Rules.yaml`, `array-one-column` (also `mtable`), `array-one-row` | array of; row 1; column 1; i less than n; row 2; ... | i less than n, j greater than m | the lines of `\substack` |
| 46 | `unicode.yaml`, `∉`: Auto says "not in" | x is not a member of S | x not in S | same as `∈` (edit 6); preference `Member` unchanged |
| 47 | `unicode.yaml`, `*`, and `unicode-full.yaml`, `∗` and `⋆`: "star" in any script | x superscript times | x superscript star | `x^*`, `\delta_*` |
| 48 | `Intent/linear-algebra.yaml`, `transpose`: also `⊤`, `⊺` and sans-serif T; new `transpose-with-subscript` for `u_h^T` | A to the top-th power | A transpose | |
| 49 | `unicode.yaml`, `∥`: "parallel" in a script; `ClearSpeak_Rules.yaml`, `indexed-by-plain`: `∥` and `⊥` are plain subscripts | k sub is parallel to end sub | k parallel | `k_\parallel` |
| 50 | `Intent/general.yaml`, `interval`: not after a function application | E of the closed interval from a to b | E of a, b | `E[a, b]` and `f[a, b]` hold arguments; `[a, b]` alone, or after a relation, is still a closed interval (most uses in the corpus) |

## Added for separators, probability and structure

Every row is a general rule. The readings are `en`, `ClearSpeak`, `TTS` `None`.

| # | File and rule | Old reading | New reading | Why |
|---|---|---|---|---|
| 51 | `unicode.yaml`, `,`: after something, the comma is a short pause and no word. A comma with nothing before it is still "comma" | (R, Re_D): R comma, Re D | R, Re D | a person says a pause. Tuples, arguments, lists, trailing commas. 3783 spoken commas in the corpus fell to 16 (cells of unusual `cases` tables) |
| 52 | `unicode.yaml`, `;`: between two parts a medium pause | f of x semicolon y | f of x; y | arguments and parameters |
| 53 | `unicode.yaml`, `\\` between two parts: "set minus" | A back slash B | A set minus B | `\backslash` and `\setminus` as a set difference |
| 54 | `unicode.yaml` `|` and `unicode-full.yaml` `∣`: a bar between two parts inside parentheses or brackets, not between two numbers, is "given" | p times, x divides y | p of x given y | `p(x | y)`, `E[X \mid Y]` |
| 55 | `src/canonicalize.rs`, `is_function_name`: a single bar inside the parentheses after an identifier makes it a function, before the chemistry test (P is also an element). New `is_given_arg` | P open paren A given B close paren | P of A given B | a condition only occurs inside the arguments of a function. This changes the English, French and Portuguese tests of `P(A|B)` (see below) |
| 56 | `Rules/definitions.yaml`, `LikelyFunctionNames`: double-struck E and P. `ClearSpeak_Rules.yaml`: `expectation-symbol`, `expectation-over` | double struck capital E of X | expectation of X; probability of A; expectation over x of f of x. Alone: capital E, capital P | `\mathbb{E}[X]`, `\mathbb{P}(A)`. R, N, Z, Q, C keep their number-set readings |
| 57 | `unicode-full.yaml`, `⟂⊥` in a script: "perpendicular"; `ClearSpeak_Rules.yaml`, `indexed-by-plain` also for `⟂` (`\perp`; edit 49 listed only `⊥`); new `perp-power` | k sub is perpendicular to end sub; A superscript is perpendicular to | k perpendicular; A perp | `k_\perp`, `A^\perp` |
| 58 | `ClearSpeak_Rules.yaml`, `brace-labelled`, `brace-alone` | a plus b labelled top brace labelled n | a plus b, labelled n | `\overbrace`, `\underbrace`: the brace is not spoken |
| 59 | `Intent/linear-algebra.yaml`: `determinant-scalar` (vertical bars around a capital letter read as a determinant) removed | determinant of capital S | the absolute value of capital S | `|S|` is nearly always a set size in the corpus, and `|A|` of a matrix stays readable as bars. 36 uses in the corpus; the 14 read in a sample are set sizes, counts and norms, none a matrix |
| 60 | `Intent/linear-algebra.yaml`, `determinant-function`, and `Languages/en/definitions.yaml`, entry `determinant-function` | det of A | determinant of A | `\det A`. A separate intent name, since `determinant` is also the tag of a table |
| 61 | `ClearSpeak_Rules.yaml`, `matrix-with-rows`, `matrix-row`, `matrix-cell`: also for `determinant` | the 2 by 2 determinant; row 1; a, b; row 2; c, d | the 2 by 2 determinant with rows a, b, and c, d | `vmatrix`, `\left| matrix \right|` |

## Added after reading the output of real papers

Every row is a general rule. The readings are `en`, `ClearSpeak`, `TTS` `None`.

| # | File and rule | Old reading | New reading | Why |
|---|---|---|---|---|
| 62 | `unicode-full.yaml`, `∼`: first in its group "about", between two parts "tilde" | R varies with O of 1 | R tilde order 1; about 2 percent | 145 uses in the corpus: adjacency (`w ~ u`), distribution (`X ~ N`), order of magnitude. The relation has no single meaning, so it is read as written |
| 63 | `unicode.yaml`, `ℓ`: "ell" | script l | ell | `C_\ell`, `\ell = 80`: 1173 uses in the corpus |
| 64 | `unicode.yaml`, `°`: "circle" after a one-letter Latin base | y degrees of u | y circle of u | `y^\circ` marks a mean-field value; `90^\circ` and `\theta^\circ` stay degrees |
| 65 | `unicode-full.yaml`, `⋉ ⋊` | the right normal factor semidirect product of | semidirect product | `SO(2) \rtimes Z_2` |
| 66 | `unicode-full.yaml`, nabla in bold, italic and bold italic: "grad" | bold nahblah | grad | `\bm{\nabla}` |
| 67 | `unicode-full.yaml`, `⊙` in a subscript: "sun"; double-struck letters and digits (the block `𝔸`-`𝕫`, `𝟘`-`𝟡`, `ℍ ℙ ℾ ℿ` and the MathType private-use copies) lose "double struck" | double struck capital S to the d minus one | capital S to the d minus one | same wording as row 8 for bold; `M_\odot`. Number sets (R, N, Z, Q, C) keep their own readings |
| 68 | `ClearSpeak_Rules.yaml`: `number-set-sub`, `indexed-by-starred`, `indexed-by-plain` for `mtext`, `*` and `⊙` | the integers two; delta sub star end sub; sub peak end sub | Z two; delta star; peak | `\mathbb{Z}_2`, `Re_{\delta^*}`, `\text{peak}` |
| 69 | `ClearSpeak_Rules.yaml`: `function-space-power`, `function-label-power`, `function-inverse-power` | the first power of, capital H of capital omega | capital H one of capital omega; f inverse of x | `H^1(\Omega)`, `W^{1,p}`, `L^\infty`, `\mathcal{R}^{out}(u)`, `\tau^1(x)`. 277 uses in the corpus. A power of a lower-case function (`f^n(x)`, `f^2(x)`) is unchanged |
| 70 | `ClearSpeak_Rules.yaml`: `indicator-function` | double struck one A | indicator of A | `\mathbb{1}_A` |
| 71 | `Intent/general.yaml`: `permutation-msubsup` only for lower-case letters n and k | 1 permutations of mu | capital P 1 to the mu-th power | `P_1^\mu` is a momentum, `P_0^{amp}` a pressure |
| 72 | `Rules/definitions.yaml`, `LikelyFunctionNames`: O and script O | order times, e B | order e B | `\mathcal{O}(eB)` |
| 73 | rows 54 and 55: the bar also counts when it is two levels below the bracket | E of Y divides X, D equal to 1 | E of Y given X, D equal to 1 | `E[Y | X, D=1]` |

## Added after review of the readings a listener hears (rows 77 to 90)

| # | Edit | Old reading | New reading | Why |
|---|---|---|---|---|
| 77 | `ClearSpeak_Rules.yaml`, `no-say-parens`: parentheses around a sum are silent in a numerator or denominator, next to an invisible times, and (preference `Exponents` `Auto`) as the base of a power; a group with brackets inside keeps them | open paren a plus b close paren times, open paren c plus d close paren | a plus b, times, c plus d | `(a+b)(c+d)`, `2(x+1)` |
| 78 | `ClearSpeak_Rules.yaml`, `ClearSpeak-times`: "times" is also said after a bracketed sum, with a short pause before it | open paren a plus b close paren c | a plus b, times c | `(a+b)c` |
| 79 | `ClearSpeak_Rules.yaml`, `power-parenthesized-sum` (preference `Exponents` `Auto`): a sum or product in parentheses raised to a power says "all" | open paren a plus b close paren squared | a plus b, all squared | `(a+b)^2`, `(2\pi h)^4` |
| 80 | `ClearSpeak_Rules.yaml`, `fraction-over-pauses` (preference `Fractions` `Auto`, verbosity below Verbose): a fraction with no fraction inside and not in an exponent says "over" between pauses | the fraction with numerator; a plus b; and denominator c plus d | a plus b, over, c plus d | `\frac{a+b}{c+d}`; nested fractions and fractions in exponents keep the long form |
| 81 | `ClearSpeak_Rules.yaml`, `indexed-by-sum`: a subscript made of letters, numbers, signs and other subscripts has no "end sub" | x sub n plus 1 end sub | x sub n plus 1 | `x_{n+1}`, `W_+` |
| 82 | `ClearSpeak_Rules.yaml`, `underlined`: a line under a symbol | mu with line below | mu underlined | `\underline{\mu}` |
| 83 | `ClearSpeak_Rules.yaml`, `evaluated-at-2` (same name as the shared rule, placed after the includes): "at" | f of x evaluated at x equal to 5 | f of x at x equal to 5 | `\left. f \right\vert_{x=0}`; `evaluated-at-3` keeps "evaluated at" |
| 84 | `Intent/general.yaml`, `set`: U+2223 counts as a bar; `ClearSpeak_Rules.yaml`, `set`: the colon or bar may sit anywhere in the braces and the condition may hold commas | the set t colon x of t in S, t greater than 0 | the set of all t such that x of t in S, t greater than 0 | `\{t : x(t) \in S, t>0\}`, `\{x \mid x>0\}` |
| 85 | `Intent/general.yaml`, `mtable-array-property`: a table of two or more columns after a relation, with no relation in it, is an array | x equal to two lines; line one; a b; line two; c d | x equal to the 2 by 2 matrix with rows a, b, and c, d | `x = \begin{array}{cc}...` |
| 86 | `Intent/general.yaml`, `mtext-inference-unit`: a word after a number in a subscript or superscript is not a unit | M sub 200 metres | M sub 200 m | `M_{200\mathrm{m}}` |
| 87 | `unicode.yaml`, `⇒`: "implies" | right double arrow | implies | `A \Rightarrow B` |
| 88 | `unicode.yaml`, the ASCII bar: the fallback is "vertical line"; "divides" only between two numbers | x divides y | x vertical line y | `x \vert y`; `\mid` (U+2223) still says "divides" between letters |
| 89 | `ClearSpeak_Rules.yaml`, `limit-lines`, `limit-lines-row`, `limit-lines-cell`: the lines of a `\substack` under a big operator are conditions joined by "and" | sum over two lines; line one; ... | sum over i equal to 1, and j greater than i | `\sum_{\substack{...}}` |
| 90 | `Intent/general.yaml`, `floor`, `ceiling`; `ClearSpeak_Rules.yaml`, `floor-ceiling` | left floor x right floor | floor of x | `\lfloor x \rfloor`, `\lceil x \rceil` |

## Added for definitions, events and signed sums (rows 91 to 96)

| # | Edit | Old reading | New reading | Why |
|---|---|---|---|---|
| 91 | `ClearSpeak_Rules.yaml`, `minus-bracketed-sum`: a sign before a bracketed sum that ends the term (nothing follows, or a relation or comma follows; not a power, not a subscript) | negative open paren a plus b close paren | minus, a plus b | `y = -(a+b)`; with more terms after it the brackets stay |
| 92 | `Intent/general.yaml`, `event`; `ClearSpeak_Rules.yaml`, `event`: braces round one comparison | open brace a greater than b close brace | the event a greater than b | `\{X > t\}`; a list or a condition is still a set |
| 93 | `ClearSpeak_Rules.yaml`, `indexed-by-pause-end`: a subscript that is not a single symbol ends with a pause, not "end sub", when nothing follows in the expression or a relation, comma or semicolon follows | x sub n squared end sub, equal to 1 | x sub n squared, equal to 1 | a factor, a sum or a fraction after the subscript still gets "end sub" |
| 94 | `unicode-full.yaml`, `≔`: "defined as" | x colon equals 1 | x defined as 1 | `\coloneqq`; the engine writes `:=` as `\coloneqq` before conversion |
| 95 | `Intent/general.yaml`, `mtable-equations-property` and the two rules that test for a relation in a table: `≔` and `≕` count as an equals sign | array of; row one; column one; E of f, column two; defined as, ... | the rows of an `align` with `&:=` are lines of equations | `E(f)&:=a^2,\\ D(f)&:=E(f)-b` |
| 96 | `ClearSpeak_Rules.yaml`, `fraction-over-simple`, `fraction-over-text`, `fraction-over-pauses`: a short pause after a fraction read with "over" when a sum or a product goes on after it (next sibling is an operator with something after it that is not a space, text or a full stop: `+`, `-`, `±`, `∓`, an invisible times, `×`, `·`, `*`) | a over b plus c | a over b, plus c | `\frac{a}{b}+c` could be heard as a over (b plus c); `c+\frac{a}{b}`, and a fraction last in its group, are unchanged |

Also with these rows, outside the rule files (all in `crates/tex2speech/src/math.rs`): `\|\cdot\|` and `f(\cdot)` name the dot ("norm of dot", "f of dot"), because the converter's mrow grouping puts the second bar inside a product with the dot; `\operatorname{Re}` before a Greek letter, a Latin letter or a group is the real part, as is `Re` or `\mathrm{Re}` before a Greek letter. `unicode.yaml` in English quotes `'Terse'` (the unquoted name is a child-element test). `fr` and `pt` `unicode.yaml` keep upstream's unquoted `Terse`: the upstream test `fr::units::si_other_numbers` encodes the Terse output that follows from it.

## Upstream tests changed by these edits

29 assertions, each following one row above, none removed or loosened:

- Rows 34 and 35: `parens_interval_*` (10 tests in `tests/Languages/en/ClearSpeak/functions.rs`). A closed `[c,d]` alone is still an interval.
- Row 43: `multiline::case_1`, `navigate::cases_speech` (three strings: the table is read as a definition, with no "case N").
- Row 40: `definitions::linear_algebra_tests`.
- Row 44: `mtable::matrix_4x4` and `matrix_4x2` (ClearSpeak strings only), and `matrix_simple_table`, `mtable_blank_op` (a one-column table reads "3, 2").
- Row 46: `sets::element_alone`.
- Row 51 (the word "comma" becomes a pause): assertions in `tests/Languages/en/alphabets.rs` (one per alphabet line), `units.rs`
  (the comma in a list of units), `ClearSpeak/sets.rs`, `SimpleSpeak/sets.rs`, `ClearSpeak/symbols_and_adornments.rs`
  (the eight `ellipses_*` tests) and `ClearSpeak/functions.rs`. A script replaced the expected string by the new output only after
  the diff showed that the only change was the missing word and, where it applies, a pause kind. In `units.rs` the pause kinds
  between neighbouring units (`,` or `;`) come from the pause-merging of the speech engine, as before.
- Rows 54 and 55 (`P(A|B)` is a function of a condition): `ClearSpeak::symbols_and_adornments::vertical_line_probability_given`,
  `shared::given`, `shared::ignore_period`, `shared::ignore_period_and_space` (English), `fr::shared::given`,
  `fr::shared::ignore_period_and_space`, `pt::shared::given` (French and Portuguese gain "de" after the P in the
  SimpleSpeak and ClearSpeak strings that say the parentheses).
- Row 67 (double struck): the lines of `alphabets::double_struck` that name double-struck letters and digits, MathType copies included.
- Rows 77, 79, 80, 81, 83 (the words for a sum in parentheses, a fraction, a subscript and an evaluation changed): `ClearSpeak::mfrac::non_simple_fraction`,
  `ClearSpeak::mfrac::non_simple_function_over_function` (the line with `Fractions` `Auto` and Medium verbosity; the lines for other
  preferences are unchanged), `ClearSpeak::msup::nested_non_simple_squared_exp` (the `Auto` line),
  `ClearSpeak::symbols_and_adornments::vertical_line_evaluated_at` and `vertical_line_evaluated_at_divides`, `mtable::matrix_3x1_not_simple`,
  `shared::non_simple_msubsup`, and `ClearSpeak::functions::more_implied_times`, `parens_superscript` (the product `2x` in parentheses
  squared: "2 x, all squared").
- Row 61: `mtable::determinant_2x2` (ClearSpeak string only; SimpleSpeak keeps the row form) and `navigate::determinant_speech`
  (ClearSpeak; moving into a row of a determinant no longer says "row 1").

## Upstream tests changed

Expected strings changed only to follow the edits above. A script took the failing assertions, applied
the listed edits to the old string, and updated the test only when the result equalled the new output.
About 280 assertions in `tests/Languages/en/**` and 7 in `src/navigate.rs` changed this way.

Tests removed or adapted because the folder they use is trimmed:

- `src/braille.rs`: `ueb_highlight_24`, `ueb_lower_word_sign_contractions`, `test_UEB_start_mode`
  removed (UEB rules removed). `find_mathml_from_braille` keeps the Nemeth part; the UEB and CMU parts
  are removed.
- `src/navigate.rs`: `zoom_speech_ru`, `zoom_speech_pl`, `move_char_speech_pl`, `move_char_speech_ru`,
  `describe_nested_fraction_ru` removed (Russian and Polish rules removed).
- `src/prefs.rs`: `found_all_files`, `test_prefs`, `test_some_changes` use the Nemeth braille code
  instead of UEB, and `test_language_change_invalidates_definitions_caches` uses `pt` instead of `nb`.
- `tests/fuzz_regressions.rs` removed (it held no test).
- `tests/braille.rs` and `tests/languages.rs` list only the kept codes and languages.

## Other fork changes (not speech edits)

- `fr/ClearSpeak_Rules.yaml`, `set`: the `count(*)=2` branch said "l'ensemble vide" for a set of two arguments. Removed, and the match is narrowed as in English so that sets with several arguments reach `function-intent`. Tests `two_argument_set_is_not_empty` and `three_argument_set_keeps_every_element`.
- `pt/SharedRules/general.yaml`, `imaginary-part`: `a parte imaginária# rules on ...` had no space before `#`, so the comment was spoken. Test `imaginary_part_has_no_comment_text`.
- `yaml-rust` 0.4 (unmaintained, RUSTSEC-2024-0320) replaced by `yaml-rust2` 0.13. Upstream leaves this change commented in its `Cargo.toml`. `yaml-rust2` 0.11 to 0.13 cannot parse a one-pair flow sequence whose value is a flow collection (`[a: [b]]`) and rejects a closing `}` or `]` indented as little as its key. Rule changes with the same meaning: `[set_variables: [...]]` became `[{set_variables: [...]}]` in the four `navigate.yaml` files, the closing `}` or `]` of 50 flow collections in eight `definitions.yaml` files got one more space, and four inline test strings in `src/definitions.rs` were wrapped the same way.
- `build.rs` finds `Rules` through `CARGO_MANIFEST_DIR`.
- `src/speech.rs`, `replace_single_char`: the test for "ignore rule file times" was `CheckRuleFiles == "All"`, the reverse of the test in `SpeechRules::read_files`. The embedded rules have no file times, so with the default preference the full Unicode table (`unicode-full.yaml`, decompressed and parsed) was read again for every character outside the short table: 8 ms for each of `∑`, `∇`, `∂`. The test now matches `read_files`. Equations with such characters read 20 to 40 times faster; the output is unchanged.
- `tests/Languages/pt/alphabets.rs`: 29 of the 30 ignored tests ran with English expected text copied from the English tests, with the reason that `unicode-full.yaml` was removed from pt. The file is present and the engine reads it. They now expect the Portuguese output and run. `greek_mathtype_private` stays ignored: the MathType private-use characters have no entry, and the engine says only the comma.
