# Keplearn reference

Version 1.3.1, result schema `keplearn.result.v2`.

This page documents everything Keplearn reads, every option, every field it
writes, and the rules behind its verdicts. `keplearn --help` is the short
form of the same material.

- [Input](#input)
- [Options](#options)
- [Output](#output)
- [Verdicts](#verdicts)
- [Scoring](#scoring)
- [The selection criterion](#the-selection-criterion)
- [How answers are ordered](#how-answers-are-ordered)
- [Time budget](#time-budget)
- [Memory limit](#memory-limit)
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
- Above 30 feature columns the search uses the 30 most related to the
  target one at a time: the largest squared correlation of x, ln|x| or
  1/x with y or ln|y| (ties to the earlier column). The others are left
  out of the search and of `names.columns`, and a warning lists the 30
  kept. The first step's work grows with the cube of the columns: it
  completes in about 7 s at 30 columns and not within 8 s at 45, and at
  10,000 columns it took 17 GB before reaching its first division. A law
  whose columns are each unrelated to the target (a difference of two
  large columns) can be screened out; search such columns as a subset.
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
| no data row with as many fields as the header | `no data row has the header's N fields (most rows have M); check that every row has a value for each column` |
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
| `--max-memory MB` | 4096 | the heap limit in MiB; 0 is unbounded (see [memory limit](#memory-limit)) |
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
| `-V`, `--version` | | print `keplearn` and the version (`keplearn 1.3.1`) and exit |

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
| `stopped_by` | string | `exact_exit` (an exact answer ended the search), `frontier_exhausted` (the search finished) or `timeout` (the budget ran out) or `memory` (the heap reached 75% of `--max-memory`) |
| `validation` | string | `holdout` (80 rows or more) or `leave-one-out` (fewer); see [scoring](#scoring) |
| `noise_floor` | number | the rounding floor: the error the cells' written precision implies, as a fraction of the target's variance; 0 for full-precision cells, and also 0 when it was not estimated (a warning says so, see [the rounding floor](#the-rounding-floor)) |
| `names` | object | `{"target": ..., "columns": [...]}`: the target's name and the kept feature columns in `x1..xN` order |
| `rows` | integer | data rows read (after skipped rows) |
| `warnings` | array of strings | notes on the input (dropped or constant columns, a column equal to the target, skipped rows, duplicate names, more than 20 columns, the columns kept above 30, invalid UTF-8, values that overflow, a constant target) and on the verdict (an exact verdict that rests on reading the features as rounded; a rounding floor not estimated) |
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
| `holdout_r2` | number or null | the score the verdict reads (see [scoring](#scoring)): R² on the held-out rows (`validation: holdout`) or by leave-one-out (`validation: leave-one-out`); null when it cannot be computed |
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
  that is not exact: `0.999956`, never `1.0000`; `r2 > 0.9999999999`
  when it is within 1e-10 of 1);
- `constants`: the free constants, and whether snapped constants were
  written by name;
- `why not exact`: the `reason`, when the verdict is `approximate`; it
  ends with the answer's largest miss as a percentage of the target (to
  two decimals, or two significant digits below 0.01%), and the target
  value where it happens;
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
     3e-5 (the square root of 1e-9, the precision of an exact fit on
     full-precision data). On rounded data whether a term is needed is
     decided by the [selection criterion](#the-selection-criterion): the
     canonical stage drops it when the data do not pay for it.
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
- the search found no candidate, or none predicted the rows it was
  checked on better than their mean (or could be evaluated on enough of
  them).

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
largest target magnitudes when there are 20 or more (exactly that many,
ties taken in row order, so a 0/1 target keeps its 1s), and requiring
the answer to be defined on nine tenths of them.

**Fewer than 80 rows.** No row is held out. Answers are scored by
leave-one-out: for each row, every constant of the answer is refitted on
the other rows and predicts the left-out row, and R² is computed over
those predictions. An answer with no constant at all is scored on its
own R², since there is nothing to refit.

**The score printed.** `holdout_r2` is the score the verdict reads.
Below 80 rows it is the lower of the leave-one-out R² with a refitted
scale and offset around the answer and the leave-one-out R² with the
answer's own constants refitted, so an answer printed without an offset
is not credited with one. From 80 rows it is the held-out R² with a
refitted scale and offset (the exact threshold is calibrated on it);
every answer there carries its own fitted scale and offset.

**Constants.** Each answer's constants are refitted on the training rows.
A simple value near a constant is admitted when the fit stays within a
tolerance:

- on an exact fit, a relative 1e-13 or the rounding floor, whichever is
  larger;
- otherwise, one standard error of the fitted value.

Admitted values: zero; decimals of one to six significant digits and
their halves, thirds and quarters; and the multiples of π, e, √2, √3 and
their powers listed under [free constants](#free-constants). Among the
fitted value and every admitted one, the
[selection criterion](#the-selection-criterion) chooses: the value it
scores lowest, or, among the values within half a nat of that, the one
that takes the fewest digits to write (the numerator's significant
digits, the denominator's when it is not 1, and one for a named
constant). A fitted 6.285 on a two-digit pendulum becomes `2*pi`, which
is structural and fits the rows better, not `19/3`, which the rounding
floor also admits. Being snapped does not make a constant structural: a
snapped 115 is still a free constant.

A named or fractional value counts as structural only when the data
single it out. Naming one of k values that fit equally well costs about
ln k nats, which the credit for a structural constant does not pay for.
So a snapped value is withdrawn when another candidate that takes no
more digits to write fits within half a nat of it, on the data alone
(the criterion without that credit). The constant then stays a decimal
and counts as free. Planets typed to two digits support 0.199 as well as
`5/(8*pi)`, so the answer is written `0.199*a**(3/2)` (or the shorter
0.2 where it fits), with the constant free. A fitted 1.0104 on 5% noise is
written 1.01, not `7/(4*sqrt(3))`. A two-digit pendulum keeps `2*pi`
because no value as short fits as well.

Constants are first snapped one at a time. An answer short of exact then
has its constants chosen together: each constant inside a function takes
its fitted value or one of the three nearest structural values its
change alone admits, each scale and the offset is refitted or set to a
nearby structural value (the offset also to 0), and every combination
with more structural constants than the answer has, and within the
tolerance, is offered to the criterion alongside the answer as it
stands (at most 27 inner and 64 outer combinations). On an answer short
of exact a combination must also write fewer constants (a scale of 1, an
offset of 0): there a snapped value only rounds a fitted one, so 8.31
on a gas law at 1% noise stays 8.31 rather than becoming `7*2**(1/4)`,
which the tolerance also admits. The tolerance here
is widened to what the criterion could pay for: moving a constant by z
standard errors costs about z²/2 nats and making it structural saves
½ ln n, so a combination of k constants is admitted while it loses no
more than k·ln(n)·(1 − R²)/n of R². One at a time, a snapped scale that
needs the offset at 0 is never tried, because the offset refitted first
absorbs the change; together, `x1/(4*pi*x2*x3)` on 1% noise is found
where one at a time left `0.0795*x1/(x2*x3) + 0.0011`. An exact answer
keeps a joint choice only if it is still exact.

**Reductions.** The canonical stage then tries the smaller forms inside
an answer (drop a term, unwrap a function, drop a factor or an argument
constant), refitted. A smaller form replaces the answer when it is
still exact (for an exact answer) and the selection criterion rises by
no more than half a nat. On integer-written y = 2x + 1, dropping the
offset leaves `2.2*x`: inside the rounding floor of whole numbers, but
it adds a free constant and misses cells that `2*x + 1` hits exactly, so
it is refused.

## The selection criterion

Wherever the data allow more than one answer (which constant value,
whether a term stays, which answer comes first), one criterion decides:
the description length of the answer on the training rows (at most 256,
evenly spaced), in nats.

- **The data given the answer.** Each target cell holds the true value
  rounded to the cell's written place q. The answer predicts it with a
  spread σ² = u + s² + τ²: u is the feature cells' rounding carried
  through the answer when the features are read as rounded readings, or
  0 when they are read as exact settings (whichever costs less); s² is
  extra noise fitted to the residuals, either the same on every row or
  proportional to the predicted value (measurements good to a fraction
  of their size), whichever costs less (½ ln n when not 0); τ is
  1e-13 of the value, the evaluation's own precision. A row costs
  −ln P, the probability that the predicted value rounds to the cell
  written: P = Φ((r + q/2)/σ) − Φ((r − q/2)/σ) for the residual r. An
  answer that lands every row in its own cell costs nothing.
- **The answer.** ½ ln n for each node of the expression and for each
  free constant (integers, small fractions and named constants are not
  free).

Two answers within half a nat of each other (about one standard error of
one constant) count as equally supported, and the simpler is taken.

## How answers are ordered

Answers are ordered by verdict: every `exact` answer first, then
`approximate`, then `no law found`. Within a verdict:

by the [selection criterion](#the-selection-criterion), lowest first,
the search's order breaking ties. An answer holding a saturated function
(see rule 4 above) comes after the answers without one.

The verdicts are given to the first 3 × `--top-k` distinct candidates the
search ranks highest (below 80 rows, exact candidates by fewest free
constants then fewest nodes, the rest by description length on the
scoring rows; from 80 rows by description length on the held-out rows),
before this ordering.

Answers that share a structure are listed once (the best).

The criterion charges for size, so a lower answer can have the higher
`r2`: a simpler answer can sit above an alternative whose extra terms or
constants buy less fit than they cost.

## Time budget

`--timeout MS` (default 60000) bounds the whole run, from reading the
table to writing the result, not just the search:

- the search stops at 80% of the budget;
- the remaining 20% assembles the answers: the rounding floor, the
  canonical form of each candidate (smallest first), and the scores. No
  new candidate is started once the budget is spent, and at least one is
  always finished.

The first search step (the root of the search) always completes, but it
checks the deadline inside its larger parts (the term library, the
product screen, the children over column pairs, the ranking of those
children), so even on a wide table the run ends close to the budget.
Forms a step has already completed (the root's sum of library terms) are
still collected once the budget is spent. Below about a second the budget
is not honoured exactly: the first step's fixed work takes that long on a
wide table. With `--timeout 0` the search runs until it finishes.

## Memory limit

`--max-memory MB` (default 4096; 0 is unbounded) bounds the heap the
program holds, counted at every allocation:

- past 75% of the limit the search stops as it does when the time budget
  runs out (`stopped_by: memory`), and the answers found so far are
  assembled and written;
- an allocation that would pass the limit ends the program with exit
  status 3 and a message on stderr, and nothing on stdout.

The count is of the bytes the program asks for; the process's resident
memory runs somewhat above it (the allocator's own overhead). Most tables
stay in tens of megabytes. The sum-of-terms step holds up to two sets of
4,000 prepared library terms, about 128 MB each, on tables of many
columns: measured at 30 searched columns, 170 to 190 MB on 50- and
200-row tables and 430 MB on a 35-column table screened to 30.

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
| 3 | the memory limit (`--max-memory`) was reached before a result could be written |
| 101 | an internal error; the message asks for a report |

**Determinism.** The same input and options give the same output, byte
for byte, on every run and machine. The holdout split uses a fixed seed,
and `--seed` has no effect. Two things break this:

- `--timing`, which adds the elapsed time;
- a budget that runs out (`stopped_by: timeout`). How far the search got
  depends on the machine's speed, and so can the answer. Pass
  `--timeout 0` for answers that do not depend on speed.

A stop at the memory limit does not depend on speed: the same input and
options stop at the same point.

## Limits

- **Search space.** Answers are built from the target by chains of up to
  four reductions (divide out a factor, invert a function, subtract an
  affine part), ending in a monomial, a sparse sum of short terms, or a
  ratio of two such sums. A law outside that space is not found. Its best
  approximation is reported as `approximate` or `no law found`.
- **Columns.** Two to five columns work best. Irrelevant columns drop out
  of the answer, but every column widens the search: thirty columns take
  about 7 s and 170 MB to 430 MB even for an easy law
  ([memory limit](#memory-limit)). Above 30 columns the search
  uses the 30 most related to the target (see [input](#input)); a
  two-column law among 45 to 400 columns of 50 or 200 rows was found
  within a 10-second budget. The product library uses
  products of up to three columns up to 21 columns and of two beyond.
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
- **Size.** Input over 1 GiB is rejected: a file redirected to stdin
  before it is read, a pipe once 1 GiB has arrived. The whole input is
  held in memory, and the search uses at most `--sample` training rows (500 by
  default).
