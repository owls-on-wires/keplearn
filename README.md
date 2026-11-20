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
- A quick check for a clean law: it answers with the law, written cleanly
  (`a**(3/2)`, not `exp(1.4998*log(a))`), when one fits reasonably well,
  and says no law when none does
- Takes every table as measured data, noisy or rounded, and reports how
  closely the law fits: its `r2`, computed from the printed equation, its
  largest miss, and whether that is within the precision the cells are
  written to
- Fits a target spanning decades (body weights, event counts, river
  flows) by its relative misses, where a power law or exponential with a
  free exponent is the usual law
- Handles small tables (a law with one constant from 6 rows, one with none
  from 5)
- Single binary, zero dependencies (fully static with the musl target)
- Deterministic; the same input produces the same output, byte for byte,
  unless the time budget runs out (`--timeout 0` removes the budget)

Full documentation: [docs/reference.md](docs/reference.md).

## Download

[![macOS](https://img.shields.io/badge/macOS-universal-2ea44f?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/owls-on-wires/keplearn/releases/latest/download/keplearn-macos-universal.tar.gz)
[![Windows](https://img.shields.io/badge/Windows-x86__64-2ea44f?style=for-the-badge&logoColor=white)](https://github.com/owls-on-wires/keplearn/releases/latest/download/keplearn-windows-x86_64.zip)
[![Linux](https://img.shields.io/badge/Linux-x86__64-2ea44f?style=for-the-badge&logo=linux&logoColor=white)](https://github.com/owls-on-wires/keplearn/releases/latest/download/keplearn-linux-x86_64.tar.gz)

Each download is an archive holding one folder with the `keplearn` binary,
this README, the CHANGELOG, the LICENSE, the
[documentation](docs/reference.md) and the [example tables](examples).

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
a candidate variable; text columns are dropped with a warning. The tables
used below are in [`examples/`](examples), in the repository and in every
release archive.

```sh
$ ./keplearn --target force --format summary < examples/gravity.tsv
force = m1*m2/r**2                                law (r2 = 1.0000, 60 rows)
constants: none free
fit: largest miss 0.000379% of the target's value (where the target is 1.06791), within the precision the cells are written to
data: 60 rows, 3 columns; rounding floor 1.0e-6 of the target's variance
next best: force = 1.3*m2/r**2    no law found
```

The verdict is `law` or `no law found`. A law fits reasonably well (r² of
at least 0.9, or a median miss within 5% of the target), has the shape
laws take (a product of powers with at most one exponential or group, not
`T*cos(T)` or `h + h*log(h)`), and is written as
cleanly as its constants allow: a fitted constant within 0.1% of a simple
value takes it, and one within the data's uncertainty of a common
constant (`pi`, `2*pi`, `e`, `sqrt(2)`) or a physical constant in SI
units (`c`, `G`, `h`, `k_B`, `R`, `g_n`, ...) is written by name. The `fit`
line says how closely: the largest miss, and whether the cells' written
digits account for it. When a law with fewer constants also fits closely,
the summary shows it as `simpler law`.

Real measurements are what it is for. The eight planets as NASA lists
them, distances and periods to three or four digits:

```sh
$ ./keplearn --target T_yr --format summary < examples/planets.csv
T_yr = a_AU**(3/2)                                law (r2 = 0.99999953, 8 rows)
constants: none free
fit: largest miss 0.104% of the target's value (where the target is 0.241), more than the precision the cells are written to
data: 8 rows, 1 column; rounding floor 3.6e-8 of the target's variance
next best: T_yr = 1.6*a_AU*log(a_AU) + 0.9    no law found
note: column 'planet' has no numeric values; it is dropped
```

The fitted exponent is 1.4998; the periods differ from Kepler's law by
about 0.1% (the planets' own masses), and the law is what is asked for.
Noise gives the law with its misses; the fitted 8.31 is within the
data's uncertainty of the gas constant, so it is written `R`:

```sh
$ ./keplearn --target P_kPa --format summary < examples/gas_1pct_noise.tsv
P_kPa = R*T_K/V_L                                 law (r2 = 0.9992, 441 rows)
constants: none free; snapped constants written by name
fit: largest miss 3.39% of the target's value (where the target is 132.01), more than the precision the cells are written to
data: 441 rows, 2 columns; rounding floor 3.1e-9 of the target's variance
next best: P_kPa = 0.000683*T_K**2 + 2651/V_L - 70    law
```

Scattered data gives no law. Hubble's 1929 galaxies show the trend, not a
law that fits:

```sh
$ ./keplearn --target velocity_kms --format summary < examples/hubble_1929.tsv
velocity_kms: no law found (24 rows, 1 column)
best fit: velocity_kms = 400*distance_Mpc (r2 = 0.6141; not confirmed)
reason: the best fit (r2 = 0.6141) does not beat a constant by enough to be a law
data: 24 rows, 1 column
next best: velocity_kms = 470*sqrt(distance_Mpc)    no law found
```

The default output is one JSON object (schema `keplearn.result.v3`), the
same for the same input, byte for byte:

```sh
$ ./keplearn --target force --top-k 1 --pretty < examples/gravity.tsv
{
  "schema": "keplearn.result.v3",
  "results": [
    {
      "expr": "x1*x2/x3**2",
      "model": "x1*x2/x3**2",
      "r2": 1.0000000000,
      "search_r2": 1.0000000000,
      "holdout_r2": 0.9999999999941845,
      "scale": 1,
      "offset": 0,
      "columns": [
        0,
        1,
        2
      ],
      "uses": [
        "m1",
        "m2",
        "r"
      ],
      "factor": "direct",
      "structure": "c1*x1*x2/x3**2 + b",
      "canonical": "x1*x2/x3**2",
      "verdict": "law",
      "symbolic": "x1*x2/x3**2",
      "named": "m1*m2/r**2",
      "free_constants": 0,
      "within_precision": true,
      "largest_miss": 0.0000037943926898313663,
      "largest_miss_at": 1.06791
    }
  ],
  "verdict": "law",
  "reason": null,
  "simpler_law": null,
  "stopped_by": "exact_exit",
  "validation": "leave-one-out",
  "noise_floor": 1.037230275256967e-6,
  "names": {
    "target": "force",
    "columns": [
      "m1",
      "m2",
      "r"
    ]
  },
  "rows": 60,
  "warnings": []
}
```

`model` is in `x1..xN`, the feature columns in file order (target
excluded); `named` is the same law in the table's column names and
`symbolic` writes snapped constants by name (`2*pi`, `sqrt(2)`, `11/7`).
`r2` and `largest_miss` are computed from the printed model on every row.

The [reference](docs/reference.md) documents every option, every output
field, and the rules behind the verdicts; `--help` is the short version.

Useful flags:

```sh
./keplearn --target Tc --timeout 10000 < superconductors.csv   # 10 s budget (default 60 s; 0 is unbounded)
./keplearn --top-k 3 < data.tsv                                # the 3 best answers
./keplearn --target y --sample 1000 < big.tsv                  # search on more rows
./keplearn --max-memory 8192 < wide.tsv                        # 8 GiB heap limit (default 4 GiB; 0 is unbounded)
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
and sqrt(3) when the data cannot tell the difference. A form that fits
to the data's precision is then reduced: a term, transform or factor the
fit does not need is removed. Below 80 rows the candidates are scored by
leave-one-out with every constant refitted; above, on a held-out quarter
of the rows, with a description-length penalty for size.

The precision of the data is read from the digits written in each
column: a rounding floor, the error those digits alone imply through the
candidate law. An answer within that floor, fitting every row to its own
precision and with constants not tuned to cancel, is within precision;
it ends the search early and is listed first.

Where the data allow more than one answer (which value a constant snaps
to, whether a term stays, which answer comes first), one description
length decides: the cost of the target's cells given the answer, judged
against the precision they are written to, plus the cost of the answer's
nodes and free constants. Last, each answer is written cleanly: a fitted
constant within 0.1% of a simple value (3/2, 1, 0.25) takes that value,
and an offset or term under 0.1% of the target is dropped.

The `--timeout` budget is split: the search uses 80% of it and the
answers are assembled in the rest. The heap is bounded by `--max-memory`:
past 75% of it the search stops and writes what it found. Above 30
feature columns the search uses the 30 most related to the target.

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
