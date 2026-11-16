# Changelog

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
