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

- Reads a TSV or CSV from stdin, prints candidate equations as JSON
- Single binary, zero dependencies (fully static with the musl target)
- Designed to answer in seconds: exact laws usually resolve in
  milliseconds, and `--timeout` caps the hard cases
- Deterministic; the same input produces the same output, byte for byte
- The reported `r2` is computed from the printed equation itself, never
  from an internal fitting stage
- Fitted constants snap to rationals and multiples of pi, e, and sqrt(2),
  with every snap verified before it is kept

## Download

[![macOS](https://img.shields.io/badge/macOS-universal-2ea44f?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/owls-on-wires/keplearn/releases/latest/download/keplearn-macos-universal)
[![Windows](https://img.shields.io/badge/Windows-x86__64-2ea44f?style=for-the-badge&logoColor=white)](https://github.com/owls-on-wires/keplearn/releases/latest/download/keplearn-windows-x86_64.exe)
[![Linux](https://img.shields.io/badge/Linux-x86__64-2ea44f?style=for-the-badge&logo=linux&logoColor=white)](https://github.com/owls-on-wires/keplearn/releases/latest/download/keplearn-linux-x86_64)

**macOS** (universal: Apple Silicon and Intel)

```sh
curl -L -o keplearn https://github.com/owls-on-wires/keplearn/releases/latest/download/keplearn-macos-universal
chmod +x keplearn
./keplearn --help
```

**Windows** (PowerShell)

```powershell
curl.exe -L -o keplearn.exe https://github.com/owls-on-wires/keplearn/releases/latest/download/keplearn-windows-x86_64.exe
.\keplearn.exe --help
```

**Linux** (x86_64, fully static)

```sh
curl -L -o keplearn https://github.com/owls-on-wires/keplearn/releases/latest/download/keplearn-linux-x86_64
chmod +x keplearn
./keplearn --help
```

## Usage

Name the column to predict with `--target` (the default is a column called
`target`) and pipe in a TSV or CSV with a header row. The delimiter comes
from the header: tab if present, else comma.

```sh
$ ./keplearn --target force --top-k 1 --pretty --out laws.json < gravity.tsv
$ cat laws.json
{
  "results": [
    {
      "expr": "((x1**(1))*(x2**(1))*(x3**(-2)))",
      "r2": 1.0000000000,
      "search_r2": 1.0000000000,
      "holdout_r2": 0.9999999999998754,
      "scale": 0.9999984680738088,
      "offset": 0.000011830325937959667,
      "columns": [
        0,
        1,
        2
      ],
      "factor": "direct",
      "model": "((x1*x2)*((x3)**(-2)))"
    }
  ],
  "time_ms": 0,
  "n_expressions": 0,
  "n_evaluated": 0,
  "stopped_by": "exact_exit"
}
```

`x1..xN` are the feature columns in file order, target excluded; here
x1=m1, x2=m2, x3=r, so the recovered law reads m1*m2/r**2. Candidates
arrive best-first: `results[0].model` is the answer and its `r2` is scored
on the supplied data. Diagnostics print to stderr, so stdout is always one
JSON object.

Useful flags:

```sh
./keplearn --target Tc --timeout 60000 < superconductors.csv   # cap the search at 60 s
./keplearn --top-k 3 < data.tsv                                # emit the 3 best candidates
./keplearn --target y --sample 1000 < big.tsv                  # screen on more rows
./keplearn --help                                              # full flag list and output schema
```

## Method

The engine searches over chains of invertible reductions applied to the
target column: it divides out a fitted factor, inverts a transform such as
log, sqrt, or arcsin, fits an inner affine node, and repeats this to a fixed
depth. At each node it attempts a terminal fit of the reduced target with an
affine monomial whose exponents come from a log-linear regression, with a
sparse k-term monomial, or with one of a small set of composite forms (L2
norms, rationals, trig). Candidates are ranked on an internal 25% holdout
with an MDL complexity penalty.

Fitted constants are snapped to nearby rationals and to multiples of pi, e,
and sqrt(2). A snap is kept only when a verification pass shows the model
still scores as well on holdout and full data. The winning expression is
then simplified algebraically, refit for scale and offset against the full
dataset, and re-scored. The `r2` in the output comes from evaluating the
printed model string; it is not carried over from any internal fitting
stage.

## Benchmarks

Keplearn is a method in [SRBench](https://github.com/cavalab/srbench), the
standard symbolic regression benchmark, added in
[cavalab/srbench#223](https://github.com/cavalab/srbench/pull/223).

These numbers come from SRBench's own containerized pipeline, graded by its
sympy equivalence check, at one trial per dataset and a 60 second per-fit
budget. Ground-truth symbolic solution rate by target noise:

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
