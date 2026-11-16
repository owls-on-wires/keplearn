# Keplearn reference

Version 1.2.1, result schema `keplearn.result.v2`.

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
  `target`). If two columns carry that name, the first is the target and
  the other is dropped with a warning (an answer in it would read as the
  target in terms of itself).
- Every other column is a feature, in file order: the first feature is
  `x1` in the output, the second `x2`, and so on.
- A feature column with no numeric value in any row (names, labels) is
  dropped, with a warning. So is a feature with the same value on every
  row: it cannot explain anything. The remaining features keep their
  order.
- More than 20 feature columns get a warning: the search's first step
  grows with the pairs of columns, and a wide table is searched only in
  part within a budget.
- A column with values beyond 1e150 gets a warning: their squares
  overflow, and laws over them may not be found. Rescale such a column.
- A duplicated column name produces a warning; later feature columns
  with the same name are renamed `name_2`, `name_3`, ... so that answers
  read unambiguously.
- A row index is not a variable: a feature column named as one (an empty
  name, `rownames`, `row.names`, `row_names`, `index`, `id`, or a name
  starting `Unnamed:`) whose values count up from 0 or 1 in row order is
  dropped with a warning. R's and pandas' saved tables carry such a
  column.

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

Trailing zeros after a decimal point are written digits (`2.50` has
three). Trailing zeros of a whole number written without a point are not:
`1250` is known to 10 and `300` to 100, while `1250.` and `1250.0` keep
their written places. The rounding model matters only to the verdicts (see [the rounding floor](#the-rounding-floor)).

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
| every feature column constant | `every feature column has the same value on every row ...` |
| every row skipped | `no usable data rows` |
| more than 1 GiB | `input exceeds the 1024 MiB cap` |

Input that is not valid UTF-8 is read with the undecodable bytes replaced
by `�`, and a warning says so. Control characters in column names are
escaped in error messages.

**Tables that are not searched.** Three cases return at once, with exit
status 0:

- a target with the same value on every row (two rows or more): the
  answer is that value, verdict `exact`, `r2` null (r² is undefined for a
  constant), and a warning names the value;
- a feature column equal to the target on every row: the answer is that
  column, verdict `exact`, with a warning (a longer form of the same law
  could otherwise be found first);
- fewer than 5 data rows: no answer, verdict `no law found`, and a
  `reason` saying a law needs at least 5 rows (6 if it has a fitted
  constant).

Every candidate is first fitted with a scale and an offset, which takes 5
rows; a law with no constant (such as `m1*m2/r**2`) is what remains when
they snap to 1 and 0. With 5 rows only such a law can be confirmed; a law
with one constant is also checked by leaving each row out, which takes 6
rows. The `reason` says so when nothing is confirmed.

A target that is negative on every row is searched as its negation and
the answers are negated back; the output is in the target's own sign.

## Options

| option | default | meaning |
|---|---|---|
| `--target NAME` | `target` | the column to explain |
| `--top-k N` | 10 | the most answers to return (at least 1) |
| `--timeout MS` | 60000 | the time budget in milliseconds; 0 is unbounded (see [time budget](#time-budget)) |
| `--format json\|summary` | `json` | the JSON result, or a short report in the table's column names |
| `--pretty` | off | indent the JSON (with `--format summary` it has no effect, and a warning says so) |
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
| `-V`, `--version` | | print `keplearn` and the version (`keplearn 1.2.1`) and exit |

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
| `results` | array | the answers, best first (see [ordering](#how-answers-are-ordered)), at most `--top-k` |
| `verdict` | string | the first answer's verdict: `exact`, `approximate` or `no law found` |
| `reason` | string or null | why the verdict is not `exact`; null when it is |
| `stopped_by` | string | `exact_exit` (an exact answer ended the search), `frontier_exhausted` (the search finished) or `timeout` (the budget ran out) |
| `validation` | string | `holdout` (80 rows or more) or `leave-one-out` (fewer); see [scoring](#scoring) |
| `noise_floor` | number | the rounding floor: the error the cells' written precision implies, as a fraction of the target's variance; 0 for full-precision cells, and also 0 when it was not estimated (a warning says so, see [the rounding floor](#the-rounding-floor)) |
| `names` | object | `{"target": ..., "columns": [...]}`: the target's name and the kept feature columns in `x1..xN` order |
| `rows` | integer | data rows read (after skipped rows) |
| `warnings` | array of strings | notes on the input (dropped or constant columns, a column equal to the target, skipped rows, duplicate names, more than 20 columns, invalid UTF-8, values that overflow, a constant target) and on the verdict (an exact verdict that rests on reading the features as rounded; a rounding floor not estimated) |
| `time_ms` | integer | elapsed milliseconds; present only with `--timing` |

`results` is empty only for a table too small to search (fewer than 5
rows). When nothing better is found, the one answer is the target's mean,
with `r2` 0 and verdict `no law found`.

### An answer

Every answer carries every field below; a value that does not exist is
null.

| field | type | meaning |
|---|---|---|
| `model` | string | the law in `x1..xN`, constants written to 13 significant digits |
| `symbolic` | string | `model` with snapped constants written by name (`2*pi`, `sqrt(2)/2`, `pi**2`, `1/sqrt(2*pi)`, `log(2)`, `euler_gamma`); `euler_gamma` is NumPy's `np.euler_gamma` |
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

- line 1: the first answer in the column names, its verdict, `r2`, rows
  (`r2` to 4 decimals, or to as many as show it is below 1 on an answer
  that is not exact: `0.999956`, never `1.0000`);
- `constants`: the free constants, and whether snapped constants were
  written by name;
- `why not exact`: the `reason`, when the verdict is `approximate`; it
  ends with the answer's largest miss as a percentage of the target, and
  the target value where it happens;
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
2. **No row misses by more than rounding can.** Rounding is bounded: a
   value written to a place q is off by at most q/2. Every row's residual
   must be within 1.5 times the error its cells' written digits can cause
   through the answer (q_y/2 + Σ_j |∂f/∂x_j| q_xj/2, with the features'
   part only when they are read as rounded, see
   [the rounding floor](#the-rounding-floor)), plus 3e-4 standard
   deviations of the target for full-precision cells.
3. **Every row fits, when the target spans orders of magnitude.** If the
   target has one sign and its largest magnitude is at least 1000 times
   its smallest, R² is set by the largest values. The answer must also
   fit each row: the mean over rows of (residual² / tolerance) must be at
   most 1, where a row's tolerance is 3 times its rounding variance plus
   (1e-8 × the row's value)².
4. **It is well conditioned.**
   - No term of any sum is negligible: its share of the sum is at least
     the precision exact is judged at, the square root of the larger of
     the rounding floor and 1e-9 (so at least 3e-5).
   - No sum's terms exceed the sum itself by a factor of 1e6 (huge
     constants cancelling).
   - No function or power group has an argument that barely varies over
     the rows (a range below 3% of its typical size, or 0.03 in absolute
     terms for `exp`, `sin` and `cos`). Over such a range the function is
     nearly linear and stands in for a simpler form.
   - No function is padding: a function or square root whose output
     varies by less than 2% of its mean over the rows is saturated, and
     if replacing it by its mean keeps the answer within the exact
     tolerance, it is a constant written long (`arctan(T)` where T is
     large on every row).
5. **It has rows to spare.** The table has at least (free constants + 3)
   rows.
6. **It is close to the best answer.** An answer below the first is exact
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
  all rows but one needs its constants + 3 rows), or, below 6 rows, only
  a law with no fitted constant could have been confirmed;
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
training rows, divided by the target's variance.

**Features as settings or as readings.** A round feature value (a length
of 0.25 or 0.5, an angle of 0.1, an integer count) may be an exact setting
or a coarse reading, and the digits alone cannot say which. Keplearn
computes the floor both ways: with the target's precision only (features
exact) and with the features' too. The features are read as exact
settings when both hold:

- the second floor is more than 100 times the first: the target was
  written far more precisely than the features' rounding would let anyone
  know it;
- every feature column with written precision looks like settings: it
  is mostly written with at most 3 significant digits (0.25, 1.5, 120),
  or it repeats on a design grid (at most half as many distinct values as
  rows, as temperatures 273.15, 278.15, ... repeated across volumes). A
  column of distinct six-digit values is a set of measurements however
  precise the target.

Otherwise both columns are read as rounded as written. When an answer is
exact only under that second reading, a warning says so.

Keplearn computes each floor through every candidate whose R² on the
target is within 0.01 of the best (at most 30 of them), and takes the
smallest. It does this only when some candidate of the search reaches R²
0.999; otherwise the floor is not estimated, `noise_floor` is 0, a
warning says so, and the summary prints "rounding floor not estimated"
instead of "full precision". A floor below 1e-18 is reported as 0.

During the search, before any candidate exists, "exact" is judged against
the target's own floor, which needs no candidate (its cells' rounding
variance relative to its variance): a law fitting a precisely written
target ends the search at once.

A floor is an upper bound on the error from rounding. When the features
are read as rounded and are in fact exact settings, the floor is too
generous and several close forms can be exact at once; the warning above
flags this, and rule 6 keeps alternatives far worse than the first answer
from being called exact.

### Free constants

A constant is free when it was read off the data. These are not free:

- integers and fractions p/q with |p| and q at most 12;
- multiples p/q, with |p| at most 8 and q one of 1, 2, 3, 4, 6, 8, of π,
  e, √2 and √3, and of their squares, square roots and reciprocals;
- 1, 2 or ½ times (only):
  - √(2π) and 1/√(2π) (Gaussian normalisation);
  - ln 2 and ln 10, and their reciprocals;
  - the Euler–Mascheroni constant γ (written `euler_gamma`);
  - the golden ratio φ and 1/φ (written `(1 + sqrt(5))/2`).

  The second group is limited so that named constants do not cover the
  number line (2.0148 is within 1e-5 of 7·ln 10/8).

  These are the forms `symbolic` writes by name.

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

Answers are ordered by verdict: every `exact` answer first, then
`approximate`, then `no law found`. Within a verdict:

- below 80 rows: exact answers by fewest free constants, then fewest
  nodes; the rest by description length (the expression above, smallest
  first);
- from 80 rows: by description length on the held-out rows.

Below 80 rows, when the target has one sign and spans a factor of 100 or
more, and the first approximate answer misses some row by more than 5% of
its value, approximate answers are ordered instead by the description
length of their relative residuals ((y − p)/y), each row predicted by
leave-one-out. On such a target r² is set by the largest values, and a form bent to
them can miss the small rows by tens of percent.

Within a verdict, an answer holding a saturated function (see rule 4
above) comes after the answers without one.

The verdicts are given to the first 3 × `--top-k` distinct candidates the
search ranks highest, before this ordering.

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

The first search step (the root of the search) always completes, but it
checks the deadline inside its larger parts (the term library, the
product screen, the children over column pairs), so even on a wide table
the run ends close to the budget. Below about a second the budget is not
honoured exactly: the first step's fixed work takes that long on a wide
table. With `--timeout 0` the search runs until it
finishes.

An exact answer ends the search early (`stopped_by: exact_exit`) when its
canonical form is exact, reduced, holds on the rows at the edges of the
data, and is well conditioned. Noisy data never ends early: a noisy table
runs until the search has nothing left to try or the budget is spent,
which on all but small tables is usually the budget.

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
  of the answer, but every column widens the search: twenty columns take
  seconds and about 200 MB even for an easy law, and from about 40
  columns a 10-second budget is spent in the first step without finding
  a two-column law. The product library uses products of up to three
  columns up to 21 columns and of two beyond. Memory grows with how long
  the search runs: a 40-column table stays in tens of megabytes on a
  10-second budget, but reached about 550 MB finding its law in 40 s at
  the default budget.
- **Rows.** A table needs 5 rows to be searched; a law with k constants
  needs at least k + 3 rows to be exact. Five rows suffice for a law with
  no constant, six for a power law with one. Below 80
  rows the leave-one-out refits make scoring slower in proportion to the
  rows.
- **Noise.** Noisy data gives `approximate` at best, and the search can
  spend its whole budget looking for an exact form. Set `--timeout` to
  what you can afford.
- **Precision of written numbers.** Precision is read from the digits as
  written. Values printed with spurious digits, such as `0.30000000000000004`,
  read as full precision. Values rounded to integers by the instrument
  rather than the writer read as rounded to 1.
- **Magnitudes.** Values beyond about 1e150 overflow when squared; the
  table is not rejected, but laws over such columns may not be found.
- **Size.** Input over 1 GiB is rejected. The whole input is held in
  memory, and the search uses at most `--sample` training rows (500 by
  default).
