# Keplearn reference

Version 2.0.1, result schema `keplearn.result.v3`.

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
by `�`, and a warning says so. A `--target` argument that is not valid
UTF-8 is decoded the same way, so a name typed in the table's own
encoding finds its column. Any other command-line argument that is not
valid UTF-8 is refused (exit status 2). Control characters in column
names are escaped in error messages.

**Tables that are not searched.** Three cases return at once, with exit
status 0:

- a target with the same value on every row (two rows or more): the
  answer is that value, verdict `law`, `r2` null (r² is undefined for a
  constant), and a warning names the value;
- a feature column equal to the target on every row: the answer is that
  column, verdict `law`, with a warning (a longer form of the same law
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
| `-V`, `--version` | | print `keplearn` and the version (`keplearn 2.0.1`) and exit |

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
| `schema` | string | `"keplearn.result.v3"`; changes whenever a field changes |
| `results` | array | the answers, best first (see [ordering](#how-answers-are-ordered)), at most `--top-k` |
| `verdict` | string | the first answer's verdict: `law` or `no law found` (see [verdicts](#verdicts)) |
| `reason` | string or null | why no law was found; null for a law |
| `simpler_law` | integer or null | when the first answer is a law: the index in `results` of a later law with fewer free constants, no more nodes and an `r2` of at least 0.98 (the fewest constants, then the highest `r2`); a second effect in real data can put a longer law first (the giants' rotation in NASA's surface gravities), and this keeps the simpler one in view. Null otherwise |
| `stopped_by` | string | `exact_exit` (an answer within the cells' precision ended the search), `frontier_exhausted` (the search finished) or `timeout` (the budget ran out) or `memory` (the heap reached 75% of `--max-memory`) |
| `validation` | string | `holdout` (80 rows or more) or `leave-one-out` (fewer); see [scoring](#scoring) |
| `noise_floor` | number | the rounding floor: the error the cells' written precision implies, as a fraction of the target's variance; 0 for full-precision cells, and also 0 when it was not estimated (a warning says so, see [the rounding floor](#the-rounding-floor)) |
| `names` | object | `{"target": ..., "columns": [...]}`: the target's name and the kept feature columns in `x1..xN` order |
| `rows` | integer | data rows read (after skipped rows) |
| `warnings` | array of strings | notes on the input (dropped or constant columns, a column equal to the target, skipped rows, duplicate names, more than 20 columns, the columns kept above 30, invalid UTF-8, values that overflow, a constant target) and on the first answer (a fit within the cells' precision only with the features read as rounded; a rounding floor not estimated) |
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
| `verdict` | string | this answer's verdict: `law` or `no law found` |
| `within_precision` | boolean or null | whether the answer fits every row to the precision its cells are written to (see [within precision](#within-precision)); null when no law |
| `largest_miss` | number or null | the answer's largest miss on any row, as a fraction of the target's value there; null when no law, or when the target is 0 on some row |
| `largest_miss_at` | number or null | the target's value on that row |
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
force = m1*m2/r**2                                law (r2 = 1.0000, 60 rows)
constants: none free
fit: largest miss 0.000379% of the target's value (where the target is 1.06791), within the precision the cells are written to
data: 60 rows, 3 columns; rounding floor 1.0e-6 of the target's variance
```

- line 1: the first answer in the column names, its verdict, `r2`, rows
  (`r2` to 4 decimals, or to as many as show it is below 1 on an answer
  not within precision: `0.999956`, never `1.0000`; `r2 > 0.9999999999`
  when it is within 1e-10 of 1);
- `constants`: the free constants, and whether snapped constants were
  written by name;
- `fit`: the answer's largest miss as a percentage of the target (two
  significant digits), the target value where it happens, and whether
  that is within the precision the cells are written to (when the target
  is 0 on some row no miss is given as a fraction of it, and the line
  says so);
- `simpler law`: the answer `simpler_law` points to, when there is one;
- `data`: rows, columns, and the rounding floor (or `full precision`);
  with no law, rows and columns only;
- `next best`: the second answer and its verdict, when there is one;
- `note`: each warning.

When no law is found the first lines read
`target: no law found (rows, columns)`, then `best fit: ...` with its r²
marked `not confirmed`, then `reason: ...`.

Numbers in the summary are written to 12 significant digits.

## Verdicts

Keplearn is a quick check for a clean law. Every table is taken as
measured, with noise, whatever its digits: an answer is either a law
that fits reasonably well, written as cleanly as its constants allow
(see [clean constants](#clean-constants)), or no law. Whether the fit
reaches the precision the cells are written to is reported on the
answer (`within_precision`, `largest_miss`), not as a verdict.

### law

An answer is a law when all of these hold:

1. **It is supported.** It fits within precision (below), or its
   description length beats a constant's by a margin:

       n/2 × ln(1 − R²) + ln(n)/2 × size  <  −ln(S)

   where `n` is the number of scoring rows, R² is the validation score,
   `size` is the number of nodes in the expression, and `S` is the number
   of search nodes the run expanded. The right-hand side is the cost of
   having chosen this structure among all those tried.
2. **It fits reasonably well.** Its `r2` on every row is at least 0.9,
   or, when the target keeps one sign, its median miss is within 5% of
   the target's value. A trend through scattered rows (Hubble's 1929
   galaxies, r² 0.61; the 1930 stopping distances, r² 0.63 with a median
   miss near 20%) is not a law, however unlikely by chance; a law over a
   target that barely moves (100 to 103 with 1% noise) has a low r² and
   is still a law. And when the target keeps one sign, it misses at most
   max(1, 5% of the rows) by more than 25% of the target's value (and
   more than the cell's written quantum): over a target spanning decades
   an r² near 1 can come from the large values alone (a steam table's
   `0.002229*exp((4*sqrt(3)/3)*T**(1/3)) + 1`, r² 0.99999, misses its two
   smallest pressures by 28% and 64%). One wild row, a typo, is allowed.
3. **It has rows to spare.** The table has at least (constants + 3)
   rows, counting every constant when it is not within precision and the
   free ones when it is (see [free constants](#free-constants)).

### within precision

A law is within precision (`within_precision: true`) when all of these
hold:

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
     3e-5 (the square root of 1e-9).
   - No sum's terms exceed the sum itself by a factor of 1e6 (huge
     constants cancelling).
   - No function or power group has an argument that barely varies over
     the rows (a range below 3% of its typical size, or 0.03 in absolute
     terms for `exp`, `sin` and `cos`).
   - No function is padding: a function or square root whose output
     varies by less than 2% of its mean over the rows is saturated, and
     if replacing it by its mean keeps the answer within the tolerance,
     it is a constant written long (`arctan(T)` where T is large on every
     row).
5. **It is close to the best answer.** An answer below the first is
   within precision only if its unexplained variance (1 − `r2`) is at
   most 3 times the first answer's, or 3e-9 if that is larger.

Below 80 rows, an answer whose leave-one-out score cannot be computed is
never within precision (see [scoring](#scoring)). Within precision is
judged on the answer as written, after its constants are made clean: a
fitted 1.0004 written as 1 may leave the cells' precision, and is still
written 1.

### no law found

The answer is still listed, as the best fit, and `reason` says which of
these applies to the first answer:

- the table is too small to check it by leaving one row out (the refit on
  all rows but one needs its constants + 3 rows), or, below 6 rows, only
  a law with no fitted constant could have been confirmed;
- it has more constants than the rows can confirm (fewer than its
  constants + 3 rows);
- it does not beat a constant by the margin above;
- it explains less than 90% of the target's variance and its median miss
  is over 5% of the target's value (or the target changes sign);
- it misses more rows than allowed by more than 25% of their value;
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
  or it repeats on a design grid: at most half as many distinct values as
  rows, lying at least 2 written quanta apart (median gap), as
  temperatures 273.15, 278.15, ... repeated across volumes. A column of
  distinct six-digit values is a set of measurements however precise the
  target, and so is one drawn so densely that its values repeat at their
  written precision (a million six-digit readings in [0.5, 3] lie one
  quantum apart).

Otherwise both columns are read as rounded as written. When the first answer is
within precision only under that second reading, a warning says so.

Keplearn computes each floor through every candidate whose R² on the
target is within 0.01 of the best (at most 30 of them), and takes the
smallest. It does this only when some candidate of the search reaches R²
0.999; otherwise the floor is not estimated, `noise_floor` is 0, a
warning says so, and the summary prints "rounding floor not estimated"
instead of "full precision". A floor below 1e-18 is reported as 0.

During the search, before any candidate exists, "within precision" is judged against
the target's own floor, which needs no candidate (its cells' rounding
variance relative to its variance): a law fitting a precisely written
target ends the search at once.

A floor is an upper bound on the error from rounding. When the features
are read as rounded and are in fact exact settings, the floor is too
generous and several close forms can be within precision at once; the
warning above flags this, and rule 5 keeps alternatives far worse than
the first answer from being called within precision.

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
- 1, 2 or ½ times a physical constant in SI units, or the reciprocal of
  one of those (see [physical constants](#physical-constants)).

  These are the forms `symbolic` writes by name.

On a law within precision, `free_constants` counts only the free ones.
On another law it counts every constant except those written by name:
`R*T/V` and `(2*pi)*sqrt(L)/sqrt(g)` have none, but a rounded 6.3 or
0.1 was read off the data and counts. An answer that is no law counts
every constant. The rows a law needs (constants + 3) count every constant
when it is short of the cells' precision, because there a snap only
rounds a fitted value. A coefficient of 1 and an offset of 0 are not
constants.

### Clean constants

Last, every answer is written as cleanly as its constants allow. The
data are not asked whether they single the clean value out: every table
is taken as noisy, and a law is what is wanted.

- A fitted constant within 0.1% of a simple value takes that value: a
  value written with one or two significant digits, or a fraction p/q
  with |p| and q at most 12. A fitted exponent of 1.4998 is 3/2, a scale
  of 0.9994 is 1; 8.314 stays (0.17% from 8.3), and so do 0.399 and a
  fitted 1.005.
- An offset under 0.1% of the target's mean size is dropped, and so is a
  term that contributes less than that on every row (unless it is the
  only one).
- A value the snap already wrote as structural (`2*pi`, `1/(4*pi)`, 2/3)
  is left as it is.
- A common mathematical constant, or a physical one, takes the place of
  a fitted value when the data allow it: within 2% of the value, putting
  it in raises the unexplained variance by at most 9 times the residual
  variance per remaining degree of freedom (three standard errors of the
  constant). This is tried before the rounding above, so 2/pi is
  preferred to the fraction 7/11 near it. The mathematical ones are pi
  times 1/4, 1/2, 1, 2, 4 or 4/3; e, sqrt(2), sqrt(3), log(2) and
  sqrt(pi) times 1/2, 1 or 2; sqrt(2*pi); and the reciprocals of all of
  these (2/pi, 1/(4*pi), 1/e, sqrt(2)/2). A physical constant is tried
  only as the overall scale of a one-term law (R in `R*T/V`); inside a
  function or beside an offset it would mean nothing. Thirty pendulum
  swings with 1% noise fit 6.30; `2*pi` is within three standard errors
  and is written.
- A constant is written by name only when the data pin it: moved by 1% either way from its best value,
  the fit worsens by at least one residual variance per degree of
  freedom (a relative standard error of at most 1%). A name the snap
  chose that the data do not pin is written to the digits they support,
  the fewest significant digits within one standard error: a Cepheid
  slope known to 2% is `-1.41`, not `-sqrt(2)`. An answer that is no law
  writes no constant by name.
  Other names (`7*2**(1/4)`, `sqrt(e)`) still need the data to single
  them out (see [snapping](#the-selection-criterion)).
- When a constant inside a function or power moves, the scales and the
  offset are refitted with it, and the answer is normalized again:
  `exp(1.5*log(a))` is written `a**(3/2)`.

#### Physical constants

These are written by name, in SI units (CODATA 2018), each as itself,
twice or half it, or the reciprocal of one of those; a product of two is
never written by name:

| name | constant | value |
|---|---|---|
| `c` | speed of light | 299792458 m/s |
| `G` | gravitational constant | 6.67430e-11 m³/(kg·s²) |
| `h`, `hbar` | Planck constant, reduced | 6.62607015e-34, 1.054571817e-34 J·s |
| `q_e` | elementary charge | 1.602176634e-19 C |
| `k_B` | Boltzmann constant | 1.380649e-23 J/K |
| `N_A` | Avogadro constant | 6.02214076e23 /mol |
| `R` | gas constant | 8.314462618 J/(mol·K) |
| `F` | Faraday constant | 96485.33212 C/mol |
| `eps0`, `mu0` | vacuum permittivity, permeability | 8.8541878128e-12 F/m, 1.25663706212e-6 N/A² |
| `k_e` | Coulomb constant | 8.9875517923e9 N·m²/C² |
| `sigma_SB` | Stefan–Boltzmann constant | 5.670374419e-8 W/(m²·K⁴) |
| `b_wien` | Wien displacement constant | 2.897771955e-3 m·K |
| `m_e`, `m_p`, `m_u` | electron, proton, atomic mass unit | 9.1093837015e-31, 1.67262192369e-27, 1.66053906660e-27 kg |
| `a0` | Bohr radius | 5.29177210903e-11 m |
| `R_inf` | Rydberg constant | 10973731.568160 /m |
| `alpha` | fine-structure constant | 7.2973525693e-3 |
| `g_n` | standard gravity | 9.80665 m/s² |
| `atm` | standard atmosphere | 101325 Pa |

The values match only data in these units (and units that share them:
the gas constant is also 8.314 kPa·L/(mol·K)); in others they are not
near any fitted value and nothing is written. A name equal to a column's
name is not written: the constant is printed as a number. Euler's number
is `e`; the elementary charge is `q_e`.

The cleaned answer is refused only when it would no longer fit: its
unexplained variance (1 − R² on the training rows) over 10 times the
fitted answer's and over 1e-6. The `r2`, `largest_miss` and the verdict
are those of the answer as written. On eight planets to three or four
digits the fitted `exp(1.4998*log(a))` (r² 0.99999995) is written
`a**(3/2)` (r² 0.99999953, largest miss 0.10%); the real periods differ
from it by the planets' own masses.

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
refitted scale and offset (the within-precision threshold is calibrated on it);
every answer there carries its own fitted scale and offset.

**Constants.** Each answer's constants are refitted on the training rows.
The fit is least squares. When the target's cells are written to
different places (three significant digits from 500 down to 2.74), the
answer is also fitted with each row weighted by the inverse of its
spread: its cell's rounding (q²/12) plus the noise the plain fit leaves
beyond the rounding, the same on every row. The
[selection criterion](#the-selection-criterion) keeps whichever of the
two fits it scores lower. The plain fit is set by the largest values: on
a decay written to three digits it missed the small rows beyond their
rounding, and the answer was not within precision; the weighted fit lands every
row in its cell. Where the noise is well beyond the rounding the weights
are nearly equal and the two fits agree.

A simple value near a constant is admitted when the fit stays within a
tolerance:

- on a fit within precision, a loss of 1e-13 of R² or the rounding floor,
  whichever is larger;
- otherwise, one standard error of the fitted value.

Admitted values: zero; decimals of one to six significant digits and
their halves, thirds and quarters; and the multiples of π, e, √2, √3 and
their powers listed under [free constants](#free-constants). Among the
fitted value and every admitted one, the
[selection criterion](#the-selection-criterion) chooses: the value it
scores lowest, or, among the values within half a nat of that, the one
that takes the fewest digits to write (the numerator's significant
digits, the denominator's when it is not 1, one for a named constant,
and one for a power of it other than ±1: `pi` and `2*pi` take two,
`7*2**(1/4)` and `1/sqrt(2*pi)` three). A fitted 6.285 on a two-digit
pendulum becomes `2*pi`, which is structural and fits the rows better,
not `19/3`, which the rounding floor also admits. On a fit within precision at the
full-precision tolerance the simplest admitted value is taken without
the criterion, which there would weigh misses of a few 1e-14 and take
offsets of that size; so 6.62607015e-34 on cells written to sixteen
digits becomes 6.62607e-34, which moves R² by 1e-15. Being snapped does
not make a constant structural: a snapped 115 is still a free constant.

A named or fractional value counts as structural only when the data
single it out. Naming one of k values that fit equally well costs about
ln k nats, which the credit for a structural constant does not pay for.
So a snapped value is withdrawn when another value that takes no more
digits to write fits within half a nat of it, on the data alone (the
criterion without that credit). The values tried are the other
candidates that write one of its structural constants otherwise and,
for each structural constant, the two nearest values as short to write
put in its place. The constant then stays a decimal
and counts as free. On a gas law at 1% noise the data's best constant
is 8.3244, which is `7*2**(1/4)`; 8.32, as short, fits as well, so the
answer is written with a decimal. A Gaussian density written to four
digits keeps `1/sqrt(2*pi)`; written to three, 0.399 fits as well and
is written instead. A value is named, then, only where no decimal as
short fits as well: a slope of 2.72 written to three digits is
indistinguishable from `e`, which is shorter, and is written `e`.
Planets typed to two digits support 0.199 as well as `5/(8*pi)`, so the
answer is written `0.199*a**(3/2)` (or the shorter 0.2 where it fits),
with the constant free. A fitted 1.0104 on 5% noise is written 1.01, not
`7/(4*sqrt(3))`. A two-digit pendulum keeps `2*pi` because no value as
short fits as well.

Constants are first snapped one at a time. An answer short of the cells' precision then
has its constants chosen together: each constant inside a function takes
its fitted value or one of the three nearest structural values its
change alone admits, each scale and the offset is refitted or set to a
nearby structural value (the offset also to 0; a scale also to one of
the three shortest decimals, the fitted value rounded to fewer
significant digits, that its change alone admits), and every combination
with more structural constants than the answer has, and within the
tolerance, is offered to the criterion alongside the answer as it
stands (at most 27 inner and 64 outer combinations). On an answer short
of the cells' precision a combination must also write fewer constants (a scale of 1, an
offset of 0): there a snapped value only rounds a fitted one, so 8.31
on a gas law at 1% noise stays 8.31 rather than becoming `7*2**(1/4)`
in a joint choice,
which the tolerance also admits. The tolerance here
is widened to what the criterion could pay for: moving a constant by z
standard errors costs about z²/2 nats and making it structural saves
½ ln n, so a combination of k constants is admitted while it loses no
more than k·ln(n)·(1 − R²)/n of R². One at a time, a snapped scale that
needs the offset at 0 is never tried, because the offset refitted first
absorbs the change; together, `x1/(4*pi*x2*x3)` on 1% noise is found
where one at a time left `0.0795*x1/(x2*x3) + 0.0011`. On
`E = 1239.84/λ` written to six digits, one at a time the offset kept
−3e-6 and the scale could not then reach 1239.84; together, offset 0 and
scale 1239.84 land every row in its cell. An answer within precision keeps a joint
choice only if it is still within precision.

**Reductions.** The canonical stage then tries the smaller forms inside
an answer (drop a term, unwrap a function, drop a factor or an argument
constant), refitted. A smaller form replaces the answer when it is
still within precision (for an answer within precision) and the selection criterion rises by
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
  extra noise, either the same on every row or proportional to the
  predicted value (measurements good to a fraction of their size),
  whichever costs less (½ ln n when not 0), at the level that makes the
  rows cost least (searched over half decades below the mean squared
  residual, then refined); τ is
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

Answers are ordered: laws within precision first, then the other laws,
then `no law found`. Laws within precision are ordered by the
[selection criterion](#the-selection-criterion), lowest first. The other
laws and the answers that are no law are ordered by a practical
description length:

    n/2 × ln(max(m², 0.005²)) + ln(n)/2 × (nodes + constants)

where `m` is the rms miss as a fraction of the target's value (of the
target's spread when it changes sign). Misses under 0.5% of the target
do not count, so a clean law is not passed over for a longer form that
fits a small second effect: JPL's 72 moons of Jupiter give
`6.4621e-9*a**(3/2)` (rms miss 0.125%, Io and Europa off by the Laplace
resonance), not a three-node longer form at 0.109%. The search's order
breaks ties. An answer holding a saturated function
(see rule 4 above) comes after the answers without one.

The verdicts are given to the first 3 × `--top-k` distinct candidates the
search ranks highest (below 80 rows, candidates within precision by fewest free
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
memory runs somewhat above it (the allocator's own overhead). A table
whose law is found early stays in tens of megabytes (13 MB for a clean
three-column law on 60 rows). A search that runs to its budget fills
the library of prepared terms: measured peaks are 185 to 190 MB on
100- and 120-row tables of three columns (noisy or unrelated), 435 MB
on 1,000 rows of five columns, and 170 to 190 MB at 30 searched columns
on 50 and 200 rows. The sum-of-terms step holds up to two sets of 4,000
prepared library terms, about 128 MB each, on tables of many columns;
a 35-column table screened to 30 measured 430 MB.

An answer within precision ends the search early (`stopped_by: exact_exit`) when its
canonical form is within precision, reduced, holds on the rows at the edges of the
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
  approximation is reported as a law not within precision, or as `no law found`.
- **Columns.** Two to five columns work best. Irrelevant columns drop out
  of the answer, but every column widens the search: thirty columns take
  about 7 s and 170 MB to 430 MB even for an easy law
  ([memory limit](#memory-limit)). Above 30 columns the search
  uses the 30 most related to the target (see [input](#input)); a
  two-column law among 45 to 400 columns of 50 or 200 rows was found
  within a 10-second budget. The product library uses
  products of up to three columns up to 21 columns and of two beyond.
- **Rows.** A table needs 5 rows to be searched; a law with k constants
  needs at least k + 3 rows. Five rows suffice for a law with
  no constant, six for a power law with one. Below 80
  rows the leave-one-out refits make scoring slower in proportion to the
  rows.
- **Data spanning orders of magnitude.** Constants are fitted by least
  squares on the target as written, so where the target spans decades
  with errors proportional to its size (body weights, event counts, river
  flows) the largest values decide the fit. A power law or exponential
  there may be missed (no law found) or replaced by another form: 56
  perch weighed by length gave `(4/pi)*exp(sqrt(L)) - 20`, not
  `a*L**3.16`. Check such a law against a log-log plot.
- **Families the search reaches poorly.** A sinusoid of unknown period
  (tides) and growth curves (logistic) are often not found.
- **Noise.** On noisy data no answer is within precision, and the search
  can spend its whole budget looking for one. Set `--timeout` to
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
