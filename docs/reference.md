# Keplearn reference

Version 1.0.0, result schema `keplearn.result.v2`.

This page documents everything Keplearn reads, every option, every field it
writes, and the rules behind its verdicts. `keplearn --help` is the short
form of the same material.

- [Input](#input)
- [Options](#options)
- [Output](#output)
- [Verdicts](#verdicts)
- [How answers are ordered](#how-answers-are-ordered)
- [Time budget](#time-budget)
- [Messages, exit status and determinism](#messages-exit-status-and-determinism)
- [Limits](#limits)

## Input

Keplearn reads one table from standard input. It never reads files named
on the command line and has no configuration files or environment
variables.

**Format.** A header row, then data rows. If the header contains a tab,
the table is tab-separated; otherwise it must contain a comma and is
comma-separated. A leading byte-order mark and Windows line endings are
accepted. A field may be quoted with `"`, and `""` inside a quoted field
is a literal quote. Blank lines are ignored.

**Columns.**

- The target is the column whose header equals `--target` (default
  `target`). If two columns carry that name, the first is the target.
- Every other column is a feature, in file order: the first feature is
  `x1` in the output, the second `x2`, and so on.
- A feature column with no numeric value in any row (names, labels) is
  dropped, with a warning. The remaining features keep their order.
- A duplicated column name produces a warning.

**Rows.** A row is skipped, and counted in a warning, when it has a
different number of fields from the header, or when the target or any
kept feature is missing, not a number, or not finite (`NA`, `nan`, `inf`
and empty cells all count). Numbers may use exponent notation (`1.2e-3`).

**Precision.** The digits written in each cell are part of the input:
Keplearn treats `3.14` as a value known to the nearest 0.01. For each
column it chooses one rounding model, whichever more cells support:

- fixed decimals: every cell is known to the column's finest written
  place (a column of `29.1250`, `30.5625` is known to 0.0001);
- fixed significant digits: every cell is known to the most common
  number of significant digits (a column of `0.24`, `1.9`, `84` is known
  to two significant digits, so `84` is known to 1).

Trailing zeros count as written digits (`250` has three). The rounding
model matters only to the verdicts (see [the rounding floor](#the-rounding-floor)).

**Rejected input** (exit status 1, see [messages](#messages-exit-status-and-determinism)):

| condition | message |
|---|---|
| nothing on stdin, or a header with no data row | `no data on stdin ...` |
| binary data (more than a tenth of the first 4 KiB is control bytes or undecodable) | `the input is not text ...` |
| text starting with `{` or `[` | `the input looks like JSON ...` |
| a header with no tab or comma | `the header has a single column (...)` |
| a first row made of numbers that does not contain the target name | `the first row (...) holds numbers, not column names ...` |
| no column named as the target | `target column '...' not found; columns are: ...` |
| a target but no numeric feature column | `input has a target column but no numeric feature columns` |
| every row skipped | `no usable data rows` |
| more than 1 GiB | `input exceeds the 1024 MiB cap` |

Input that is not valid UTF-8 is read with the undecodable bytes replaced
by `�`, and a warning says so. Control characters in column names are
escaped in error messages.

**Tables that are not searched.** Two cases return at once, with exit
status 0:

- a target with the same value on every row (two rows or more): the
  answer is that value, verdict `exact`, `r2` null (r² is undefined for a
  constant), and a warning names the value;
- fewer than 4 data rows: no answer, verdict `no law found`, and a
  `reason` saying a law with one constant needs at least 4 rows.

A target that is negative on every row is searched as its negation and
the answers are negated back; the output is in the target's own sign.

## Options

| option | default | meaning |
|---|---|---|
| `--target NAME` | `target` | the column to explain |
| `--top-k N` | 10 | the most answers to return (at least 1) |
| `--timeout MS` | 60000 | the time budget in milliseconds; 0 is unbounded (see [time budget](#time-budget)) |
| `--format json\|summary` | `json` | the JSON result, or a short report in the table's column names |
| `--pretty` | off | indent the JSON |
| `-o FILE`, `--out FILE` | stdout | write the result to FILE |
| `--verbose` | off | print the search's progress lines on stderr |
| `--timing` | off | add `time_ms` to the JSON |
| `--sample N` | 500 | the most training rows the search uses (at least 1); answers are scored on every row regardless |
| `--quick-n N` | 50 | rows used by the quick pre-screen of each search node (at least 1) |
| `--max-depth D` | 4 | the deepest chain of reductions searched; values above 4 are treated as 4, and 0 searches only a constant |
| `--trace FILE` | none | append a JSON-lines trace of the search's events to FILE (a diagnostic; its events may change between versions) |
| `--dump-candidates FILE` | none | write the scored candidates (at most 200) to FILE as JSON (a diagnostic) |
| `--seed N`, `--max-evals N` | | accepted and ignored: the search is deterministic and bounded by `--timeout` |
| `-h`, `--help` | | print the help and exit |
| `-V`, `--version` | | print `keplearn 1.0.0` and exit |

For compatibility with older wrappers, `--pool`, `--reject`,
`--prune-cols`, `--reeval-cap`, `--max-arity` and `--compose` are
accepted with a value and ignored. Any other option is an error (exit
status 2).

## Output

By default stdout holds exactly one JSON object and nothing else. With
`--format summary` it holds the text report instead.

### The result

| field | type | meaning |
|---|---|---|
| `schema` | string | `"keplearn.result.v2"`; changes whenever a field changes |
| `results` | array | the answers, best first, at most `--top-k` (see below) |
| `verdict` | string | the first answer's verdict: `exact`, `approximate` or `no law found` |
| `reason` | string or null | why the verdict is not `exact`; null when it is |
| `stopped_by` | string | `exact_exit` (an exact answer ended the search), `frontier_exhausted` (the search finished) or `timeout` (the budget ran out) |
| `validation` | string | `holdout` (80 rows or more) or `leave-one-out` (fewer); see [scoring](#scoring) |
| `noise_floor` | number | the rounding floor: the error the cells' written precision implies, as a fraction of the target's variance; 0 for full-precision data |
| `names` | object | `{"target": ..., "columns": [...]}`: the target's name and the kept feature columns in `x1..xN` order |
| `rows` | integer | data rows read (after skipped rows) |
| `warnings` | array of strings | notes on the input: dropped columns, skipped rows, duplicate names, invalid UTF-8, a constant target |
| `time_ms` | integer | elapsed milliseconds; present only with `--timing` |

`results` is empty only for a table too small to search.

### An answer

Every answer carries every field below; a value that does not exist is
null.

| field | type | meaning |
|---|---|---|
| `model` | string | the law in `x1..xN`, constants written to 13 significant digits |
| `symbolic` | string | `model` with snapped constants written by name (`2*pi`, `sqrt(2)/2`, `pi**2`) |
| `named` | string | `symbolic` in the table's column names (a name that is not a plain identifier is written in backticks) |
| `verdict` | string | this answer's verdict |
| `r2` | number or null | R² of `model` evaluated on every row; null for a constant target. Rows where the model is undefined are left out, and the answer scores −1 if that is more than a tenth of them |
| `holdout_r2` | number or null | R² on the held-out rows (`validation: holdout`) or by leave-one-out (`validation: leave-one-out`); null when it cannot be computed |
| `search_r2` | number or null | the search's internal score for the candidate, before it became this answer; diagnostic only, and it can be far above `r2` |
| `free_constants` | integer or null | constants read off the data (see [free constants](#free-constants)); null when it cannot be counted |
| `uses` | array of strings | the columns `model` reads, by name |
| `structure` | string or null | `model` with its constants replaced by `c1, c2, ...` and the offset by `b`, as the canonical form prints it |
| `canonical` | string | the canonical form; equal to `model` |
| `expr`, `scale`, `offset`, `columns`, `factor` | | the raw form: `model = scale*expr + offset`, where `xj` in `expr` means feature `columns[j-1]` (0-based). In this version `expr` equals `model`, `scale` is 1, `offset` is 0, `columns` lists every feature and `factor` is `"direct"`; the fields are kept for tools written against 0.1 |

Expressions use `+ - * / **`, parentheses, and the functions `sqrt`,
`exp`, `log` (natural), `sin`, `cos`, `tanh`, `arcsin`, `arccos` and
`arctan`. They evaluate as written in Python with NumPy's functions
of those names.

### The summary

`--format summary` prints, for a law:

```text
force = m1*m2/r**2                                exact (r2 = 1.0000, 60 rows)
constants: none free
data: 60 rows, 3 columns; rounding floor 9.9e-7 of the target's variance
next best: force = (2*sqrt(3)/3)*m1*m2**(3/4)*sqrt(arctan(m2))/r**2    approximate
```

- line 1: the first answer in the column names, its verdict, `r2`, rows;
- `constants`: the free constants, and whether snapped constants were
  written by name;
- `why not exact`: the `reason`, when the verdict is `approximate`;
- `data`: rows, columns, and the rounding floor (or `full precision`);
- `next best`: the second answer and its verdict, when there is one;
- `note`: each warning.

When no law is found the first lines read
`target: no law found (rows, columns)`, then `best fit: ...` with its r²
marked `not confirmed`, then `reason: ...`.

Numbers in the summary are written to 12 significant digits.

## Verdicts

Each answer gets one of three verdicts. They are decided in this order.

### exact

All of these hold:

1. **It fits to the data's precision.** Its validation score (held-out
   R², or leave-one-out R²; see [scoring](#scoring)) is at least
   `1 − max(1e-9, 3 × rounding floor)`. On full-precision data that is
   R² ≥ 1 − 1e-9.
2. **Every row fits, when the target spans orders of magnitude.** If the
   target has one sign and its largest magnitude is at least 1000 times
   its smallest, R² is set by the largest values. The answer must also
   fit each row: the mean over rows of (residual² / tolerance) must be at
   most 1, where a row's tolerance is 3 times its rounding variance plus
   (1e-8 × the row's value)².
3. **It is well conditioned.**
   - No term of any sum is smaller than 1e-9 of the sum.
   - No sum's terms exceed the sum itself by a factor of 1e6 (huge
     constants cancelling).
   - No function or power group has an argument that barely varies over
     the rows (a range below 3% of its typical size, or 0.03 in absolute
     terms for `exp`, `sin` and `cos`). Over such a range the function is
     nearly linear and stands in for a simpler form.
4. **It has rows to spare.** The table has at least (free constants + 3)
   rows.
5. **It is close to the best answer.** An answer below the first is exact
   only if its unexplained variance (1 − `r2`) is at most 3 times the
   first answer's, or 3e-9 if that is larger.

Below 80 rows, an answer whose leave-one-out score cannot be computed is
never exact (see [scoring](#scoring)).

### approximate

Not exact, but its description length beats a constant's by a margin:

    n/2 × ln(1 − R²) + ln(n)/2 × size  <  −ln(S)

where `n` is the number of scoring rows, R² is the validation score,
`size` is the number of nodes in the expression, and `S` is the number of
search nodes the run expanded. The right-hand side is the cost of having
chosen this structure among all those tried.

### no law found

Neither of the above. The answer is still listed, and `reason` says which
of these applies to the first answer:

- the table is too small to check it by leaving one row out (the refit on
  all rows but one needs its constants + 3 rows);
- it has more constants than the rows can confirm (fewer than its
  constants + 3 rows);
- it does not beat a constant by the margin above;
- the search found no candidate, or none could be scored.

### The rounding floor

The rounding floor is the error the cells' written precision alone
implies, propagated through a candidate law. A cell known to a place `q`
carries a rounding error of variance q²/12. Through a law `y = f(x1..xN)`
a row's error variance is

    q_y²/12 + Σ_j (∂f/∂x_j)² q_xj²/12

with the derivatives taken numerically. The floor is the mean over the
training rows, divided by the target's variance. Keplearn computes it
through each candidate whose R² on the target is within 0.01 of the best
(at most 30 of them), and takes the smallest. It does this only when the
best R² is at least 0.999; otherwise rounding is not what limits the fit,
and the floor is 0. A floor below 1e-18 is reported as 0.

A floor is an upper bound on the error from rounding. A column of whole
numbers is read as rounded to 1. If those values were in fact exact
settings, the floor is too generous, and several close forms can be
exact at once. Rule 5 above keeps alternatives far worse than the first
answer from being called exact.

### Free constants

A constant is free when it was read off the data. These are not free:

- integers and fractions p/q with |p| and q at most 12;
- multiples p/q of π, e, √2 and √3, or of their squares, square roots
  or reciprocals, with |p| at most 8 and q one of 1, 2, 3, 4, 6, 8 (the
  forms `symbolic` writes by name).

On an `exact` answer, `free_constants` counts only the free ones. On any
other answer every constant counts, because snapping a constant on a fit
short of exact only rounds a fitted value. A coefficient of 1 and an
offset of 0 are not constants.

## Scoring

**80 rows or more.** A quarter of the rows, at most 2000, is held out
(the same rows on every run). The search fits on the rest, and answers
are scored on the held-out rows: R², leaving out the 2% of rows with the
largest target magnitudes when there are 20 or more, and requiring the
answer to be defined on nine tenths of them.

**Fewer than 80 rows.** No row is held out. Answers are scored by
leave-one-out: for each row, every constant of the answer is refitted on
the other rows and predicts the left-out row, and R² is computed over
those predictions. An answer with no constant at all is scored on its
own R², since there is nothing to refit.

**Constants.** Each answer's constants are refitted on the training rows,
then snapped to a nearby simple value when the data cannot tell the
difference:

- on an exact fit, within a relative 1e-13 or within the rounding floor,
  whichever is larger;
- otherwise, within one standard error of the fitted value.

Snap targets, tried shortest first: zero; decimals of one to six
significant digits and their halves, thirds and quarters; and the
multiples of π, e, √2, √3 and their powers listed under
[free constants](#free-constants). Being snapped does not make a
constant structural: a snapped 115 is still a free constant.

## How answers are ordered

- Below 80 rows: exact answers first, by fewest free constants, then
  fewest nodes; then the rest by description length (the expression
  above, smallest first).
- From 80 rows: by description length on the held-out rows.

Answers that share a structure are listed once (the best).

Ranking uses the validation score (`holdout_r2`), not `r2`, so a lower
answer can have the higher `r2`:

- an exact answer ranks by simplicity, so a simpler exact answer can sit
  above an alternative with a slightly higher `r2`, the difference being
  within the data's precision;
- among answers that are not exact, one that fits the supplied rows
  better but predicts held-out or left-out rows worse (it is fitting the
  noise) ranks lower.

## Time budget

`--timeout MS` (default 60000) bounds the whole run, not just the search:

- the search stops at 80% of the budget;
- the remaining 20% assembles the answers: the rounding floor, the
  canonical form of each candidate (smallest first), and the scores. No
  new candidate is started once the budget is spent, and at least one is
  always finished.

The first search step (the root of the search) always completes. On a
wide table that can take up to a second or so, which is the shortest
budget that is honoured. With `--timeout 0` the search runs until it
finishes.

An exact answer ends the search early (`stopped_by: exact_exit`) when its
canonical form is exact, reduced, holds on the rows at the edges of the
data, and is well conditioned. Noisy data never ends early, so a noisy
table uses its whole budget.

## Messages, exit status and determinism

**stderr.** Warnings print as `keplearn: <warning>` (they are also in
`warnings`). With `--verbose`, the search's progress lines and stage
timings print as well. Nothing else is written to stderr except errors.

**Exit status.**

| status | meaning |
|---|---|
| 0 | a result was written, whatever its verdict |
| 1 | the input could not be used, or the output could not be written |
| 2 | invalid command line, or stdin is a terminal |
| 101 | an internal error; the message asks for a report |

**Determinism.** The same input and options give the same output, byte
for byte, on every run and machine. The holdout split uses a fixed seed,
and `--seed` has no effect. Two things break this:

- `--timing`, which adds the elapsed time;
- a budget that runs out (`stopped_by: timeout`). How far the search got
  depends on the machine's speed, and so can the answer. Pass
  `--timeout 0` for answers that do not depend on speed.

## Limits

- **Search space.** Answers are built from the target by chains of up to
  four reductions (divide out a factor, invert a function, subtract an
  affine part), ending in a monomial, a sparse sum of short terms, or a
  ratio of two such sums. A law outside that space is not found. Its best
  approximation is reported as `approximate` or `no law found`.
- **Columns.** Two to five columns work best. Irrelevant columns drop out
  of the answer, but every column widens the search, and twenty columns
  take seconds and hundreds of megabytes even for an easy law.
- **Rows.** A law with k constants needs at least k + 3 rows to be
  exact. Six rows suffice for a power law with one constant. Below 80
  rows the leave-one-out refits make scoring slower in proportion to the
  rows.
- **Noise.** Noisy data gives `approximate` at best, and the search
  spends its whole budget looking for an exact form. Set `--timeout` to
  what you can afford.
- **Precision of written numbers.** Precision is read from the digits as
  written. Values printed with spurious digits, such as `0.30000000000000004`,
  read as full precision. Values rounded to integers by the instrument
  rather than the writer read as rounded to 1.
- **Size.** Input over 1 GiB is rejected. The whole input is held in
  memory, and the search uses at most `--sample` training rows (500 by
  default).
