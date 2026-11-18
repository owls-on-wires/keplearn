# Changelog

## 1.3.1

- A snapped named or fractional constant counts as structural only when
  the data single it out: when another value that takes no more digits
  to write fits as well (within half a nat, before the credit for being
  structural), the constant stays a decimal and is free. Planets typed
  to two significant digits gave `(5/(8*pi))*a**(3/2)` with no free
  constant; they now give `0.2*a**(3/2)`, and the NASA table gives
  `0.199*a**(3/2)` with one free constant. A gravity law at 5% noise gave
  `(7/(4*sqrt(3)))*m1*m2/r**2`; it is now `1.01*m1*m2/r**2`. The same
  holds for a named constant a law does not contain: at 1% noise, a
  harmonic-number table is `0.29/n + 0.577*log(n)`, not
  `(euler_gamma/2)/n + euler_gamma*log(n)`.
- Held-out scoring leaves out exactly the 2% of rows with the largest
  targets (ties in row order). It cut at a value, so on a 0/1 target every
  1 was dropped and nothing could be scored ("no law found"). A step
  function now gets a fitted line, approximate at r2 0.74.
- The message for an abstention says why: no candidate predicted the rows
  it was checked on better than their mean, or could be evaluated on
  enough of them.
- README: the `--pretty` example is the program's output as printed.

## 1.3.0

The output schema is unchanged (`keplearn.result.v2`).

**One selection criterion**

- Where the data allow more than one answer, one description length now
  decides: the cost of the target's cells given the answer, judged
  against the precision they are written to (the feature cells' rounding
  carried through the answer, read as exact settings or as rounded
  readings, whichever costs less, and any extra noise, additive or
  relative), plus ½ ln n for each node and each free constant. The
  reference documents it under "The selection criterion".
- Snapped constants: among every simple value the tolerance admits, the
  criterion chooses; the order the values are tried in only breaks ties.
  A two-digit pendulum gives `2*pi*sqrt(L/g)` whichever order they come
  in.
- An answer short of exact has its constants chosen together, not only
  one at a time: combinations of nearby structural values (and an offset
  of 0) are offered to the criterion, admitted while the R² they lose is
  within what the criterion could pay for (k·ln(n)·(1 − R²)/n for k
  constants). On 1% noise, `x1/(4*pi*x2*x3)` is found where one at a
  time left `0.0795*x1/(x2*x3) + 0.0011`: the offset, refitted first,
  absorbed each single change. On an answer short of exact a joint
  choice must write fewer constants, so a fitted 8.31 is not traded for
  a named product such as `7*2**(1/4)`.
- Reductions (dropping a term, unwrapping a function, dropping a factor)
  are kept only when the criterion does not rise. On whole numbers, read
  as known to a unit, the reductions traded `2*x + 1` for `2.2*x`
  (verdict exact, r2 0.99): an offset dropped inside the rounding floor.
  It is now refused, and `y = 2x + 1`, `a*b + 3` and `2*x + 100` written
  as integers are exact.
- A term is negligible in the exact verdict only below the precision of
  a full-precision fit; on rounded data the criterion decides whether a
  term stays (the rounding floor of whole numbers called an offset of up
  to 14% of a sum negligible).
- Answers within a verdict are ordered by the criterion. The 1.2.1
  ordering by relative misses is removed; relative noise in the
  criterion covers the targets it was for.
- Below 80 rows, `holdout_r2` is the score the verdict reads (it was the
  leave-one-out of the answer with a refitted scale and offset, so
  `2.2*x` reported 1).

**Input**

- A table whose rows have fewer fields than its header says so, instead
  of "no numeric feature columns".
- A file over 1 GiB redirected to stdin is refused before it is read.

**Documentation**

- The reference's memory figures for 30 searched columns are measured
  (170 to 430 MB).

## 1.2.3

- Constants snap to the simple value that takes the fewest digits to
  write (a named constant counting one), the nearest among equals. A
  pendulum rounded to two significant digits fitted 6.285 and was
  reported exact as `19/3*sqrt(L/g)`: thirds of one-digit decimals were
  tried before multiples of pi, and the rounding floor admitted 19/3. It
  is now `2*pi*sqrt(L/g)`, which fits the rows better.

## 1.2.2

**Wide tables**

- `--timeout` bounds the whole run, from reading the table: on 10,000
  columns reading took 4 s outside the budget, and a check that a column
  holds numbers read every row once per column.
- Above 30 feature columns the search uses the 30 most related to the
  target one at a time, with a warning naming them. The search's first
  step grows with the cube of the columns: at 10,000 columns it took
  17 GB without finishing, and from 45 columns a 10 s budget was spent
  before any law was found. A two-column law among 45 to 400 columns is
  now found within a 10 s budget.
- The first step checks the deadline while ranking its children (a
  30-column table took 17 s on a 10 s budget), and builds the product
  library's unary terms from the smallest up instead of building the
  largest and refusing it (4 s at 30 columns). A form the first step has
  completed is kept when the budget runs out.

**Memory**

- `--max-memory MB` (default 4096; 0 is unbounded) bounds the heap. Past
  75% of it the search stops and the answers found so far are written
  (`stopped_by: memory`); an allocation past the limit ends the program
  with exit status 3.

**Verdicts**

- A law with a constant of extreme magnitude is confirmed: `E =
  6.626e-34*c/lambda` was "no law found" because the leave-one-out check
  dropped the term below its tolerance. The check now scores every model
  as written, which also changes the order of approximate answers on some
  small tables.
- A snapped constant may no longer cost an exact fit its per-row
  precision: `1000*exp(-t/12.3)` was rounded to
  `1000*exp(-0.08130075*t) - 0.0001`, approximate. When a snap does, the
  argument constants keep their fitted values.
- The reason's largest miss is written to two significant digits below
  0.01% (`0.00091%`, not `0.00%`), and the summary writes
  `r2 > 0.9999999999` for an answer short of exact whose r2 rounds to 1.

**Documentation**

- 1.2.1 said Kepler's power law leads on the NASA planet table. That
  held only on one table and only while the first approximate answer
  missed some planet by more than 5%; with this release's scoring a form
  missing Mercury by 3% leads there and the power law is second. The
  reference documents the column screen and the memory limit.

## 1.2.1

- On a table of fewer than 80 rows whose target spans a factor of 100 or
  more, when the first approximate answer misses some row by more than 5%,
  approximate answers are ranked by their relative residuals (each row
  predicted by leave-one-out):
  Kepler's power law now leads on the NASA planet table, where a form bent
  to the outer planets missed Mercury by 11%.
- A row-index column (R's `rownames`, pandas' `Unnamed: 0`, `index`,
  `id`, counting the rows) is dropped with a warning instead of being
  fitted.
- Repeated feature names are renamed `x_2`, `x_3`, ...: an answer no
  longer reads `x + x`.
- The `why not exact` reason states the answer's largest miss, and the
  summary never prints `r2 = 1.0000` for an answer that is not exact.
- `--pretty` with `--format summary` warns that it has no effect.
- Documentation: the README's third example shows the output 1.2.0
  prints; the reference's memory figure for wide tables and its statement
  about noisy tables are corrected.

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
