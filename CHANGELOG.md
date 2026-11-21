# Changelog

## 2.1.3

Fixes from a blind review of 2.1.2.

- **A copied or rescaled column no longer hides the law.** 27 rows of
  `lam*temp` with an exact copy of `lam` gave "no law found": the
  power-law fit's least-squares system was singular and returned
  nothing, so no candidate was left. The copy in other units (2.54
  times) gave a false law, `1.128*lam**2*temp/lam2**(3/4) + 4` at
  r2 0.99, its exponents split between the two columns. Now a feature
  equal to an earlier one times a constant on every row, to the digits
  written in its cells (finer than 0.1% of its range), is dropped with
  a warning, and the least-squares fits give a column that depends on
  the earlier ones (such as `r` and `r**2` in logs) the coefficient 0
  instead of failing. Both tables give `lam*temp`.
- **Two columns with the same name are refused** (exit status 1, `two
  columns are named '...'`); they were renamed `name_2`, which put a
  name the table does not have into the answer, and left `--target`
  ambiguous (a second column named as the target was dropped). Unnamed
  columns may still repeat.
- Tests: the copied and rescaled column, a dependent least-squares
  column, the repeated name refused; 85 library and 8 command-line
  tests.

## 2.1.2

Fixes from a blind review of 2.1.1.

- **A large table keeps its law under a budget.** The canonical stage
  takes a finalist the search confirmed exact first, and fits each
  finalist on at most 10,000 training rows (scores still use every row).
  200,000 rows of `3*a*b/c + 2` under `--timeout 3000` gave "no law
  found"; 1,000,000 rows took 68 s against the 60 s default (a
  three-factor log-linear form alone took 40 s fitted on every row).
  Both now give the law, the million rows in about 3 s.
- **`stopped_by` says when the budget cut the answers' assembly**:
  `timeout` (or `memory`) when finalists were left unexamined, even if
  the search had ended early (it said `exact_exit`).
- **`--max-memory` stops and writes a result.** The sum-of-terms step's
  prepared terms (a Gram matrix of up to 128 MB in one block) are not
  taken when they would not fit under 75% of the limit; the search stops
  there with `stopped_by: memory`. Before, the block took the heap past
  the limit and the program exited 3 with nothing written (300 rows of
  six columns under `--max-memory 64`).
- **Memory documented as measured**: 190 MB on 100- and 120-row tables
  of three columns, 470 MB on 1,000 rows of five, 575 MB on 300 rows of
  six columns of noise, 485 MB at 30 columns (the reference gave 185 to
  435 MB, and 170 to 190 MB at 30 columns).
- An added zero is not printed (`... + 0))` after a shift snapped to 0);
  an undecodable byte in a column name is shown in messages as `�`, as
  the warning says, not as the escape `\u{fffd}`.
- Tests: a 200,000-row law under a 3 s budget, a memory-limited noise
  table writing a result, the added zero, the replacement character; 83
  library and 8 command-line tests.

## 2.1.1

- **A noisy power law on the target as given is found.** Its exponents
  are fitted on the logarithms and each taken as the grammar's nearest
  when within two standard errors (at least 0.06, at most 0.5), and it is
  a candidate from r² 0.9 (0.995 before). Below r² 0.995 its misses must
  look like noise: neighbouring rows, by their inputs, may not miss alike
  more than chance allows (a permutation test at 1%). Eight planets'
  surface gravities (exponents 1.07 ± 0.06 and −2.21 ± 0.15) now give
  `61000000*mass_1e24kg/radius_km**2`, a law at r² 0.984; 2.1.0 gave a
  three-constant form. Reduced targets inside the search keep the
  tolerance of 0.06 and r² 0.995.
- Tests: the eight planets alone give the one-term law; a missed curve's
  residuals are structured and plain noise's are not; 81 tests.

## 2.1.0

- **Targets spanning decades.** A one-signed target whose largest value
  is 100 to 1e6 times its smallest is taken as measured to a fraction of
  its size. Log-linear laws (power laws with free exponents, exponentials
  with free rates, power laws of a shifted column) join the candidates;
  each answer is also fitted by its relative misses (rows weighted by the
  inverse of the prediction, reweighted three times), the criterion,
  with proportional extra noise only, choosing; exponents, rates and
  shifts take a small fraction within two standard errors and never a
  name, and constants short of precision are written to the digits the
  data support. 56 perch give `0.0075*L**3.1` (a contorted
  three-constant form before); a river's rating curve gives
  `4330*(h - 2.31)**1.6 + 450` (both printed in log form, `exp(3.1*log(L))`:
  their exponents are free). Past 1e6 the target is fitted as written.
- **A law short of the cells' precision is clean**: one term of powers
  with at most one group and one exponential (of a scaled argument or a
  free power), `log` alone, or two terms of whole powers; any other
  function only of a scaled argument or of two or more columns; a
  logarithm of one product of columns, alone or beside a power
  (`0.5772*log(n) + 0.2887/n`). Six
  filament temperatures no longer give `-6.77*T*cos(T) + 4.42`, a river
  gauge `-26530*h + 15170*h*log(h) + 32373`, nor yeast growth a ratio of
  `exp(pi*sqrt(t))`; `reason` says what is wrong with the shape. A
  constant added inside a function is a phase, of a column to the first
  power: `arccos(1 - 3/V**6)` is not clean.
- A constant times `1/x` is written `k/x`, not `k*1/x` (Boyle's
  `1406/volume`), in `model`, `symbolic`, `named`, `structure` and
  `canonical`. A power law fitted in log form whose exponent becomes one
  of the grammar's fractions is written as a power (`1.3*m2/r**2`, not
  `1.3*m2*exp(-2*log(r))`). On such a target a free exponent, rate
  or shift is written as a decimal, never as a fraction that only
  looks structural (`10.17`, not `61/6`, for a fitted 10.167).
- An answer that is no law writes a snapped named value to the digits
  the data support (`0.79`, not `0.7853981633974`).
- A result that cannot be written to stdout exits with status 1 and a
  message: a full device (`> /dev/full`) panicked with status 101, and a
  stdout closed at start (`>&-`) exited 0 with the result lost. The same
  holds for `--help` and `--version`.
- Acceptance test: Brofeldt's 1917 perch from Lake Laengelmavesi
  (`fixtures/real/perch_length_weight.tsv`, from the JSE fishcatch data).

## 2.0.1

- A named physical constant is not a free constant: `E = h*f` reports
  none free (a scale below 1e-6 was counted before the name was seen).
- Whether the data pin a constant to 1% is judged around its best value,
  moved either way: a fitted value short of the least-squares one (a
  weighted fit) was judged by the side it lay on. Fifty pendulum swings
  with 5% noise give `(2*pi)*sqrt(L)/sqrt(g)`, not 6.207.
- `simpler_law` points only to a law no larger in its form than the
  first answer (not `(2*pi)*sqrt(L)*tanh(g)/sqrt(g)`, a factor that saved
  a constant).
- Acceptance test: the 5% pendulum (`fixtures/real/pendulum50_5pct.tsv`).

## 2.0.0

Keplearn is a quick check for a clean law: feed it a table, get the law
written cleanly when one fits reasonably well, otherwise no law. Every
table is taken as measured data, noisy or rounded.

- **Two verdicts: `law` and `no law found`** (schema
  `keplearn.result.v3`). `exact` and `approximate` are gone; whether a
  law fits to the precision the cells are written to is reported on the
  answer as `within_precision`, with its `largest_miss` (a fraction of
  the target's value) and `largest_miss_at`. Answers within precision
  are still listed first. `reason` is set only when no law is found. The
  summary's `why not exact` line is replaced by a `fit` line on every
  law.
- **Clean constants.** A fitted constant within 0.1% of a simple value
  (one or two significant digits, or a fraction p/q with |p|, q ≤ 12)
  takes it, and an offset or a term under 0.1% of the target's mean size
  is dropped; scales and the offset are refitted when a constant inside
  a function moves, and the answer is normalized again. Eight planets as
  NASA lists them gave `exp(1.49978538784*log(a_AU))` first, with
  `0.9994*a_AU**(3/2) + 0.00047` second; both are now `a_AU**(3/2)`
  (r2 0.99999953, largest miss 0.10%).
- **A law fits reasonably well:** its r² on every row is at least 0.9,
  or, for a target of one sign, its median miss is within 5% of the
  target's value (a target that barely moves has a low r² under small
  noise). The 1930 stopping distances (r² 0.63, misses up to 89%) were an
  approximate law and are now no law, with the best fit shown.
- **Common constants by name.** A fitted value within three standard
  errors of pi, 2*pi, 4*pi, their reciprocals and halves, e, sqrt(2),
  sqrt(3), log(2) or sqrt(2*pi) is written by name: thirty pendulum swings
  with 1% noise gave `6.3*sqrt(L)/sqrt(g)`, now `(2*pi)*sqrt(L)/sqrt(g)`.
  Physical constants in SI units (CODATA 2018: c, G, h, hbar, q_e, k_B,
  N_A, R, F, eps0, mu0, k_e, sigma_SB, b_wien, m_e, m_p, m_u, a0, R_inf,
  alpha, g_n, atm), each alone, doubled, halved or inverted, are written
  the same way: the 1%-noise gas table in kPa and litres gives
  `R*T_K/V_L`. A name equal to a column's name is not written. Constants
  written by name are tried before simple rounding (Griffith's 2/pi is
  no longer 7/11), and a law's `free_constants` does not count them.
  Rarer names still need the data to single them out. `sqrt(2)` is no
  longer written `2/sqrt(2)`.
- **Summary:** a `simpler law` line shows a listed law with fewer free
  constants and an r² of at least 0.98 (NASA's surface gravities give a
  two-term fit first, the giants' rotation being in the data, and
  `6.1e7*M/R**2` at r² 0.989 below it); it is in the JSON as
  `simpler_law`, an index into `results`.
- **A law does not miss many rows badly:** when the target keeps one
  sign, at most max(1, 5% of the rows) may be missed by more than 25% of
  their value (and more than the cell's quantum). An r² near 1 over a
  target spanning decades can come from its large values alone: the
  steam table's `0.002229*exp((4*sqrt(3)/3)*T**(1/3)) + 1` (r² 0.99999)
  missed its two smallest pressures by 28% and 64% and is no longer a
  law. When the target is 0 on a row the `fit`
  line no longer claims every row is within precision.
- **Ranking of laws short of the cells' precision** ignores misses under
  0.5% of the target: of two laws fitting within that, the one with fewer
  nodes and constants comes first. JPL's moons of Jupiter gave a longer
  form fitting Io's and Europa's 0.75% resonance offsets; they give
  `6.4621e-9*a**(3/2)`.
- **Names on evidence:** a constant is written by name only when the data
  pin it to 1% (relative standard error); a name the snap chose that they
  do not pin is written to the digits they support (a Cepheid slope
  `-1.41`, not `-sqrt(2)`). An answer that is no law writes no names. A
  physical constant is tried only as the overall scale of a one-term law
  (no `g_n/2` inside a cooling curve). A power that is itself rational is
  never a name (`1/16`, not `1/(8*(sqrt(2)**2))`).
- A snapped fraction whose decimal does not end is written p/q in
  `symbolic` and `named` (`(11/7)*a*log(a)`, not `1.57142857143*...`).
- The note that the rounding floor was not estimated is given only with
  a law (it qualifies `within_precision`); with no law the summary's
  `data` line gives only the table's size.
- Real-world acceptance tests (`fixtures/real`): NASA planets and JPL
  orbits (Kepler's third law), pendulum swings on four worlds at 1%
  noise, 200 and 30 of them (`2*pi*sqrt(L/g)`), Boyle's 1662 table
  (P = k/V), NASA surface gravities (the simpler law shown), a steam
  table (its fit missing the small pressures refused), and Hubble's 1929 galaxies and the
  1930 stopping distances (no law).
- README rewritten around the clean-law check, with the NASA planets and
  Hubble's galaxies as examples (`examples/planets.csv`,
  `examples/hubble_1929.tsv`).

## 1.3.4

- A `--target` argument that is not valid UTF-8 no longer panics. It is
  decoded as the header is (undecodable bytes become `�`), so a latin-1
  name finds its latin-1 column; any other such argument is refused with
  exit status 2.
- A feature column counts as a design grid (exact settings) only when its
  repeated values also lie at least two written quanta apart. A million
  six-digit readings in [0.5, 3] repeat because six digits run out, not
  because they are settings; the column was read as exact, and the
  gravity law that is exact at 200,000 rows was `approximate` at a
  million.
- README: the determinism bullet names the exception the reference
  already gave (a time budget that runs out).

## 1.3.3

- A named value is written only where no decimal as short to write fits
  as well. Its written cost now counts its power: `pi` and `2*pi` take
  two, `7*2**(1/4)` and `1/sqrt(2*pi)` three (the multiplier, the name,
  the power), like a three-digit decimal. The test that withdraws a
  structural value no longer depends on which decimals the snap happened
  to offer: the two nearest values as short to write are put in its
  place. A gas law at 1% noise, whose best constant is 8.3244, was
  written `(7*2**(1/4))*n*T/V` with the constant counted as structural
  (the reference said it would not be); 8.32 fits as well, and the
  constant is now a decimal. A Gaussian density written to four digits
  keeps `1/sqrt(2*pi)`; written to three, it is `0.399`, which fits as
  well. A slope of 2.72 written to three digits is still `e`, which the
  digits cannot tell from it and which is shorter. A rival now challenges
  a value only where it writes one of that value's structural constants
  otherwise: on a channel capacity written to three digits, the withdrawn
  `1/(2*log(2))*log(P/N + 1)` also withdrew `0.7214*log(P/N + 1)`, which
  differs from it only in its free scale, and left
  `0.72165*log(P/N + 1.0016) - 0.00111`.
- Every warning in the result is printed on stderr. The note that an
  exact verdict reads the features as rounded, and the note that the
  rounding floor was not estimated, were only in `warnings`.
- Reference: the exact-fit snap tolerance is a loss of R², not a relative
  change of the constant; memory figures for noisy searches.

## 1.3.2

- When the target's cells are written to different places (a fixed number
  of significant digits over a wide range), the canonical stage also fits
  each answer with every row weighted by its spread: its cell's rounding
  plus the noise the plain fit leaves, the same on every row. The
  selection criterion keeps whichever fit it scores lower. A decay
  `500*exp(-t/7.3)` written to three digits was approximate: the plain
  fit is set by the large rows and missed the small ones beyond their
  rounding. It is now exact (`500*exp(-0.1369801921*t)`). The weighted fit
  is tried only while the places span at most a factor of 1e6 (the solve
  squares that range); a target from 1e-25 to 1e13 keeps the plain fit.
- The selection criterion's extra noise is the level at which the rows
  cost least, searched, not the moment estimate. Where the residuals are
  of the rounding's size the moment estimate could be 0 or below while a
  few rows sat just past their cells, and each such row cost the most a
  row can: a fit that landed all but two rows in their cells scored
  worse than one that missed twelve.
- A joint choice of constants also offers each scale rounded to fewer
  significant digits. `E = 1239.84/λ` written to six digits gave
  `1239.84186804/λ - 3e-6`, approximate with `stopped_by: exact_exit`:
  one at a time the offset kept −3e-6, and the scale could then not
  reach 1239.84. It is now `1239.84/λ`, exact.
- An answer that lands every cell but fails a condition on its form says
  which (a term negligible against its sum, terms that cancel, a function
  whose argument barely varies, or one a constant could replace) instead
  of blaming its residuals.
- README: the two-digit planets example's second answer is the one this
  release prints.

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
