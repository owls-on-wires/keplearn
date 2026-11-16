# Changelog

## 1.2.0

The output schema is unchanged (`keplearn.result.v2`).

**Wide tables**

- `--timeout` holds on wide tables: the first search step checks the
  deadline inside its larger parts, and the product library uses products
  of up to three columns only up to 21 columns (two beyond). A 60-column
  table with a 10 s budget took 49 s and 6.8 GB (and could fail to
  allocate memory); it now ends within the budget in tens of megabytes.
- More than 20 feature columns get a warning suggesting subsets.

**Verdicts and ranking**

- A function that is nearly constant over the data (`arctan(T)` where T
  is large) is padding: it never makes an answer exact when a constant
  could replace it, and such answers rank after clean ones of the same
  verdict. A term below the data's precision counts as negligible.
- Feature columns are read as exact settings only when they look like
  settings: written with at most three significant digits, or repeating
  on a design grid. Columns of distinct six-digit values are readings.
- The 1.1.0 constants (`log(2)`, `log(10)`, `euler_gamma`, the golden
  ratio, `sqrt(2*pi)`) are named only as 1, 2 or 1/2 times themselves; a
  fitted 2.0148 is no longer written as `7*log(10)/8`.
- A precisely written target ends the search as soon as its law is found
  (the README's gravity example: 20 s before, now milliseconds).

**Small and odd tables**

- A table needs 5 rows to be searched (4 rows could never succeed: every
  law is first fitted with a scale and an offset). Five rows confirm a law
  with no constant, six one with a constant.
- A second column named as the target is dropped with a warning; a
  feature equal to the target on every row is reported as the answer.
- When the rounding floor could not be estimated, the result says so
  (a warning; the summary prints "rounding floor not estimated") instead
  of "full precision".

**Documentation**

- The reference states the version `-V` prints, and documents every rule
  above. The README examples show their full output.

## 1.1.0

The output schema is unchanged (`keplearn.result.v2`). What "exact" means
has changed, so some verdicts on rounded data differ from 1.0.0.

**Verdicts**

- Round feature values (lengths of 0.25 and 0.5, angles of 0.1, integers)
  are read as exact settings when the target is written far more precisely
  than their rounding would allow; otherwise both columns are read as
  rounded as written, and a warning says when an exact verdict depends on
  that reading. In 1.0.0, round settings were always read as coarse
  readings, which let wrong laws pass as exact (`0.7*theta + 0.1` for
  `sin(theta)`) and rounded constants away (`2*sqrt(L)` for
  `2.006*sqrt(L)`).
- Trailing zeros of a whole number written without a decimal point are no
  longer read as significant (`1250` is known to 10), so a column rounded
  to three significant digits is read as such.
- An exact answer must miss no row by more than rounding can cause there
  (rounding is bounded by half the last written place), not only match
  the rounding error on average.
- Answers are ordered by verdict: every exact answer first, then
  approximate, then no law found.

**Search**

- Exponentials and logarithms with an offset are found
  (`3*exp(-0.5*t) + 2`).
- Constants snap to and are written as `1/sqrt(2*pi)`, `sqrt(2*pi)`,
  `log(2)`, `log(10)`, `euler_gamma` and the golden ratio, besides
  multiples of pi, e, sqrt(2) and sqrt(3).

**Input and robustness**

- Values near the floating-point limit (1e308) no longer crash the
  program; values beyond 1e150 get a warning.
- A feature column with the same value on every row is dropped with a
  warning; a table whose features are all constant is an input error.
- Tables of 4 and 5 rows say that only a law with no fitted constant can
  be confirmed there (one with a constant takes 6 rows). The result always
  has a reason when no law is found.

**Documentation and releases**

- `--help` lists exit status 101.
- The example tables the README uses are in `examples/`.
- Release archives include this changelog and the examples.

## 1.0.0

Breaking changes from 0.1.2:

- Output schema `keplearn.result.v2`. Every answer carries the same
  fields; the result adds `schema`, `verdict`, `reason`, `validation`,
  `noise_floor`, `names`, `rows` and `warnings`. `n_expressions` and
  `n_evaluated` are removed; `time_ms` is written only with `--timing`.
- `--timeout` defaults to 60000 ms (was unbounded; 0 is unbounded). The
  search uses 80% of the budget and the answers are assembled in the rest.
- Progress lines on stderr only with `--verbose`.
- Constants in `model` are written to 13 significant digits.

New:

- Each answer gets a verdict, exact, approximate or no law found, judged
  against the precision the cells are written to, with a reason when it is
  not exact.
- Small tables: row minimums from the number of constants, leave-one-out
  scoring below 80 rows.
- `--format summary`: a short report in the table's column names, with
  snapped constants written by name.
- Messages for JSON, binary, single-column and headerless input; a
  constant target returns its value.
- `docs/reference.md` documents every option, output field and verdict
  rule.
- Release assets are archives with the binary, README, LICENSE and docs.

## 0.1.2

- `-o/--out FILE` and `--pretty`.

## 0.1.1

- Release automation and download links.

## 0.1.0

- First release.
