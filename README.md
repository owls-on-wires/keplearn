# Keplearn

**Symbolic regression in seconds.**

Keplearn is named after Johannes Kepler, who deduced his laws of planetary motion by studying tables of astronomical observations taken by Tycho Brahe. That is what this program attempts to do: given columns of measurements, find the compact law that relates them.

```text
    m1   |  m2   |  r   | force
   ------+-------+------+-------
    1.0  |  2.0  | 1.0  |  2.00
    2.0  |  2.0  | 2.0  |  1.00        ┌──────────┐
    1.5  |  3.0  | 3.0  |  0.50  ──►   │ keplearn │ ──►   force = m1 * m2 / r**2
    2.5  |  1.0  | 0.5  | 10.00        └──────────┘
    3.0  |  4.0  | 2.0  |  3.00
```

More formally, it's a deterministic symbolic regression engine (written in
Rust) that recovers closed-form equations from tabular data by best-first
recursive reduction.

- Reads a TSV or CSV from stdin, prints its answers as one JSON object or a
  short text report
- Says how far to trust each answer: exact, approximate, or no law found,
  judged against the precision the data is written to
- Handles small tables (a law with one constant from 6 rows) and rounded
  data; noise beyond the data's written precision is reported as an
  approximate fit, not an exact law
- Single binary, zero dependencies (fully static with the musl target)
- Deterministic; the same input produces the same output, byte for byte
- The reported `r2` is computed from the printed equation itself

Full documentation: [docs/reference.md](docs/reference.md).

## Download

[![macOS](https://img.shields.io/badge/macOS-universal-2ea44f?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/owls-on-wires/keplearn/releases/latest/download/keplearn-macos-universal.tar.gz)
[![Windows](https://img.shields.io/badge/Windows-x86__64-2ea44f?style=for-the-badge&logoColor=white)](https://github.com/owls-on-wires/keplearn/releases/latest/download/keplearn-windows-x86_64.zip)
[![Linux](https://img.shields.io/badge/Linux-x86__64-2ea44f?style=for-the-badge&logo=linux&logoColor=white)](https://github.com/owls-on-wires/keplearn/releases/latest/download/keplearn-linux-x86_64.tar.gz)

Each download is an archive holding one folder with the `keplearn` binary,
this README, the LICENSE and the [documentation](docs/reference.md).

**macOS** (universal: Apple Silicon and Intel)

```sh
curl -L -O https://github.com/owls-on-wires/keplearn/releases/latest/download/keplearn-macos-universal.tar.gz
tar -xzf keplearn-macos-universal.tar.gz
cd keplearn-macos-universal
./keplearn --help
```

**Windows** (PowerShell)

```powershell
curl.exe -L -O https://github.com/owls-on-wires/keplearn/releases/latest/download/keplearn-windows-x86_64.zip
Expand-Archive keplearn-windows-x86_64.zip -DestinationPath .
cd keplearn-windows-x86_64
.\keplearn.exe --help
```

**Linux** (x86_64, fully static)

```sh
curl -L -O https://github.com/owls-on-wires/keplearn/releases/latest/download/keplearn-linux-x86_64.tar.gz
tar -xzf keplearn-linux-x86_64.tar.gz
cd keplearn-linux-x86_64
./keplearn --help
```

## Usage

Name the column to predict with `--target` (the default is a column called
`target`) and pipe in a TSV or CSV with a header row. The delimiter comes
from the header: tab if present, else comma. Every other numeric column is
a candidate variable; text columns are dropped with a warning.

```sh
$ ./keplearn --target force --format summary < gravity.tsv
force = m1*m2/r**2                                exact (r2 = 1.0000, 60 rows)
constants: none free
data: 60 rows, 3 columns; rounding floor 9.9e-7 of the target's variance
```

The summary states a verdict for the answer:

- **exact**: the law fits the data to the precision the cells are written
  to, is well conditioned, and the table has rows to spare for its
  constants;
- **approximate**: it fits far better than a constant, but not to the data's
  precision (measurement noise, or a form that is close but not right); the
  line `why not exact` says which;
- **no law found**: nothing beats a constant by enough, or the table is too
  small to check the best fit; the best fit is still shown, with the reason.

Rounded data is handled by its written precision. Kepler's planets typed to
two significant digits still give the law, and the verdict accounts for the
rounding:

```sh
$ ./keplearn --target T_years --format summary < planets_2_digits.tsv
T_years = a_AU**(3/2)                             exact (r2 = 0.9990, 9 rows)
constants: none free
data: 9 rows, 1 column; rounding floor 5.3e-4 of the target's variance
```

and noisy data is never called exact:

```sh
$ ./keplearn --target P_kPa --format summary < gas_1pct_noise.tsv
P_kPa = 8.33333333333*T_K/V_L                     approximate (r2 = 0.9990, 441 rows)
constants: 1 free
why not exact: the best fit leaves residuals above the precision the cells are written to
data: 441 rows, 2 columns; rounding floor 3.8e-7 of the target's variance
```

The default output is one JSON object (schema `keplearn.result.v2`), the
same for the same input, byte for byte:

```sh
$ ./keplearn --target force --top-k 1 --pretty < gravity.tsv
{
  "schema": "keplearn.result.v2",
  "results": [
    {
      "expr": "x1*x2/x3**2",
      "model": "x1*x2/x3**2",
      "r2": 1.0000000000,
      "search_r2": 1.0000000000,
      "holdout_r2": 0.9999999999934012,
      "scale": 1,
      "offset": 0,
      "columns": [0, 1, 2],
      "uses": ["m1", "m2", "r"],
      "factor": "direct",
      "structure": "c1*x1*x2/x3**2 + b",
      "canonical": "x1*x2/x3**2",
      "verdict": "exact",
      "symbolic": "x1*x2/x3**2",
      "named": "m1*m2/r**2",
      "free_constants": 0
    }
  ],
  "verdict": "exact",
  "reason": null,
  "stopped_by": "frontier_exhausted",
  "validation": "leave-one-out",
  "noise_floor": 9.909047392326821e-7,
  "names": {"target": "force", "columns": ["m1", "m2", "r"]},
  "rows": 60,
  "warnings": []
}
```

`model` is in `x1..xN`, the feature columns in file order (target
excluded); `named` is the same law in the table's column names and
`symbolic` writes snapped constants by name (`2*pi`, `sqrt(2)`). `r2` is
computed from the printed model on every row.

The [reference](docs/reference.md) documents every option, every output
field, and the exact rules behind the verdicts; `--help` is the short
version.

Useful flags:

```sh
./keplearn --target Tc --timeout 10000 < superconductors.csv   # 10 s budget (default 60 s; 0 is unbounded)
./keplearn --top-k 3 < data.tsv                                # the 3 best answers
./keplearn --target y --sample 1000 < big.tsv                  # search on more rows
./keplearn --verbose < data.tsv                                # progress lines on stderr
./keplearn --help                                              # every flag and output field
```

## Method

The engine searches over chains of invertible reductions applied to the
target column: it divides out a fitted factor, inverts a transform such as
log, sqrt, or arcsin, fits an inner affine node, and repeats this to a fixed
depth. At each node it tries terminal fits of the reduced target: an affine
monomial whose exponents come from a log-linear regression, a sparse sum of
terms chosen by basis pursuit from a library of the grammar's short
expressions (and products of them), or a ratio of two such sums.

Every candidate is put into one canonical normal form, refitted, and has
its constants snapped to nearby rationals and multiples of pi, e, sqrt(2)
and sqrt(3) when the data cannot tell the difference. An exact form is then
reduced: a term, transform or factor the fit does not need is removed.
Below 80 rows the candidates are scored by leave-one-out with every
constant refitted; above, on a held-out quarter of the rows, with a
description-length penalty for size.

Exactness is judged against the precision of the data. The number of
digits written in each column sets a rounding floor, the error those
digits alone imply through the candidate law; an answer is exact when its
residuals are within that floor, every row fits to its own precision when
the target spans orders of magnitude, its constants are not tuned to
cancel, and the table has more rows than its constants need. Below 80
rows, answers are ranked exact first (fewest free constants, then
shortest), then by description length; from 80 rows, by description
length on the held-out rows.

The `--timeout` budget is split: the search uses 80% of it and the
answers are assembled in the rest.

## Benchmarks

Keplearn is a method in [SRBench](https://github.com/cavalab/srbench), the
standard symbolic regression benchmark, added in
[cavalab/srbench#223](https://github.com/cavalab/srbench/pull/223).

These numbers were measured on Keplearn v0.1.0, the release the SRBench
integration builds (v0.1.1 and v0.1.2 changed only output flags and release
automation, not the search). They come from SRBench's own containerized
pipeline, graded by its sympy equivalence check, at one trial per dataset
and a 60 second per-fit budget. Ground-truth symbolic solution rate by target noise:

| suite | noise 0 | 0.001 | 0.01 | 0.1 |
|---|---|---|---|---|
| Feynman (116) | 75 (64.7%) | 65 (56.0%) | 51 (44.0%) | 10 (8.6%) |
| Strogatz (14) | 6 (42.9%) | 6 (42.9%) | 3 (21.4%) | 1 (7.1%) |

The highest published Feynman symbolic rate on SRBench is AIFeynman at
55.8%, with the next group near 27%; Keplearn reaches 64.7% at a fraction of
their compute budget.

On the 138 black-box datasets, which have no closed-form ground truth, the
median test R2 is near zero (about -0.003). Keplearn targets exact analytic
laws rather than general curve fitting, and that result reflects the
trade-off.

Fit time under the same 60 second cap, one core per fit:

| suite | median | mean | max | fits under 1 s |
|---|---|---|---|---|
| Feynman, noise 0 | 1.3 s | 13.3 s | 60.7 s | 55/116 |
| Feynman, noise 0.001-0.1 | 11-14 s | 15-23 s | 60.7 s | 0/348 |
| Strogatz, all noise levels | 1.3 s | 1.5 s | 3.6 s | 8/56 |
| Black-box | 60.0 s | 39.1 s | 65.8 s | 15/138 |

Solved cases exit early. At noise 0, half the Feynman suite finishes in under
a second; under noise the exact-exit path cannot fire, so those fits run to
the cap.

## License

Keplearn is released under the MIT license.

Created by Chandler Freeman <chandler@mnty.sh> (https://github.com/owls-on-wires).
