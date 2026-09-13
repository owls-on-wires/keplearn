# Keplearn

**Symbolic regression in seconds.**

[![Linux](https://img.shields.io/badge/Linux-x86__64-2ea44f?style=for-the-badge&logo=linux&logoColor=white)](https://github.com/owls-on-wires/keplearn/releases/latest/download/keplearn-linux-x86_64)
[![macOS](https://img.shields.io/badge/macOS-universal-2ea44f?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/owls-on-wires/keplearn/releases/latest/download/keplearn-macos-universal)
[![Windows](https://img.shields.io/badge/Windows-x86__64-2ea44f?style=for-the-badge&logoColor=white)](https://github.com/owls-on-wires/keplearn/releases/latest/download/keplearn-windows-x86_64.exe)

The buttons download the prebuilt binary from the latest release
(`chmod +x` it on Linux and macOS). Building from source is one command;
see Usage.

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

The pipeline contains no randomness. Runs are reproducible byte for byte.

## Results

The scores below come from SRBench's containerized grader, which checks
symbolic equivalence with sympy. Each fit ran under a 60 second budget at
noise level 0.

| suite | symbolic solution rate |
|---|---|
| Feynman (116) | 75/116 = 64.7% |
| Strogatz (14) | 6/14 = 42.9% |

The highest published Feynman symbolic rate on SRBench is AIFeynman at
55.8%; the next group of methods sits near 27%.

## Usage

```
cargo build --release
./target/release/keplearn --help
./target/release/keplearn --target y --timeout 60000 < data.tsv
```

The input is a TSV or CSV with a header row on standard input (the
delimiter is taken from the header: tab if present, else comma), and every
column
except the target is treated as a feature. The output is one JSON object on
standard output with up to `--top-k` candidate models, best first. Each
entry carries the closed-form `model` string in `x1..xN` together with its
`r2` on the supplied data. Diagnostics print to stderr.

```
$ printf 'a\tb\ttarget\n1\t2\t5\n2\t3\t12\n3\t1\t7\n4\t5\t29\n5\t2\t17\n6\t4\t34\n' | \
    ./target/release/keplearn
{"results":[{"expr":"...","r2":1.0000000000,...,"model":"..."}],...}
```

The `--help` output documents the full flag list, the output schema, and
usage notes.

## Fixtures

The `fixtures/` directory holds small TSVs with known closed forms for a
quick check:

```
./target/release/keplearn --timeout 5000 < fixtures/fx_monomial.tsv
# results[0].model = (x1*x2), r2 = 1.0
```

## License

Keplearn is released under the MIT license. It was written by Chandler
Freeman <chandler@mnty.sh> (https://github.com/owls-on-wires).
