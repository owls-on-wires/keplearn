use keplearn::{run, Config};
use std::env;
use std::io::IsTerminal;

const HELP: &str = "\
keplearn - symbolic regression engine

Reads a TSV or CSV with a header row on STDIN (tab-delimited if the
header has a tab, else comma) and searches for a closed-form expression
of the target column in terms of the other numeric columns.

USAGE:
  keplearn [OPTIONS] < data.tsv

OPTIONS:
  --target NAME           target column name (default: \"target\")
  --top-k N               number of answers to emit (default: 10)
  --timeout MS            budget in milliseconds (default: 60000; 0 is
                          unbounded). The search stops at 80% of it and
                          the answers are assembled in the rest; a budget
                          shorter than the first search step (up to about
                          a second on wide tables) ends after that step
  --format json|summary   json (default) or a short report in the
                          table's column names
  --pretty                indent the JSON
  -o, --out FILE          write the result to FILE instead of stdout
  --verbose               print the search's progress lines on stderr
  --timing                add time_ms to the JSON (without it the same
                          input gives the same bytes)
  --sample N              max training rows the search uses (default 500)
  --quick-n N             rows for the quick pre-probe (default: 50)
  --max-depth D           lower the search depth limit
  --trace FILE            append a JSONL event trace to FILE
  --dump-candidates FILE  write every scored candidate to FILE
  --seed N, --max-evals N accepted for compatibility; no effect
  -h, --help              print this help and exit
  -V, --version           print the version and exit

OUTPUT (stdout, one JSON object, schema keplearn.result.v2):
  schema        \"keplearn.result.v2\"
  results       up to --top-k answers; each has every field below
  verdict       the first answer's verdict (see VERDICTS)
  reason        why the verdict is not exact, or null
  stopped_by    exact_exit | frontier_exhausted | timeout
  validation    holdout (80 rows or more: a quarter of the rows, at
                most 2000, held out) or leave-one-out (fewer rows)
  noise_floor   the error the cells' written precision implies, as a
                fraction of the target's variance (0: full precision)
  names         {target, columns}: column names; x1..xN in a model are
                columns[0..N-1]
  rows          data rows read
  warnings      notes on the input (dropped columns, skipped rows)
  time_ms       elapsed milliseconds, only with --timing

  Each answer:
  model         the closed form in x1..xN, constants to 13 digits
  symbolic      model with snapped constants by name (2*pi, sqrt(2))
  named         symbolic in the column names
  verdict       this answer's verdict
  r2            R^2 of model evaluated on every row (null when the
                target is constant)
  holdout_r2    R^2 on the held-out rows, or the leave-one-out R^2
  search_r2     the search's internal score (diagnostic only)
  free_constants  constants read off the data (an exact answer does not
                count integers, small fractions and multiples of pi, e,
                sqrt(2), sqrt(3))
  uses          the columns the model reads
  structure     the model with its constants as c1, c2, ..., b
  canonical     the model in canonical form (equal to model)
  expr, scale, offset, columns, factor
                the raw form: model = scale*expr + offset, with xj in
                expr meaning column columns[j-1] (0-based)

  Below 80 rows, answers are ordered exact first (fewest free
  constants, then shortest), then by description length; from 80 rows,
  by description length on the held-out rows. Ranking uses the
  validation score, so a lower answer can have the higher r2.

VERDICTS:
  exact         fits to the precision the cells are written to (to
                1e-9 of the variance on full-precision data; row by row
                when the target spans decades), is well conditioned,
                and has rows to spare for its constants
  approximate   fits far better than a constant, but not to the data's
                precision (noisy data, or the form is close but wrong)
  no law found  nothing beats a constant by enough, or the table has
                too few rows to check the best fit; it is still listed

EXIT STATUS:
  0  a result was written (whatever its verdict)
  1  the input could not be used (no data, no target column, not a
     table, over 1 GiB) or the output could not be written
  2  invalid command line

NOTES:
  * Diagnostics and warnings go to stderr; stdout holds only the result.
  * With many columns, run subsets of 2-5 at a time: irrelevant columns
    drop out of the answer, but the search slows as they accumulate.
  * Output is deterministic; --seed has no effect.

AUTHOR:
  https://github.com/owls-on-wires
  Written by Chandler Freeman <chandler@mnty.sh>
";

fn main() {
    std::panic::set_hook(Box::new(|info| {
        eprintln!(
                "keplearn: internal error, please report at https://github.com/owls-on-wires/keplearn/issues: {}",
                info
            );
    }));
    let args: Vec<String> = env::args().collect();
    if args[1..].iter().any(|a| a == "-h" || a == "--help") {
        print!("{}", HELP);
        return;
    }
    if args[1..].iter().any(|a| a == "-V" || a == "--version") {
        println!("keplearn {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    fn take_val<'a>(args: &'a [String], i: usize, flag: &str) -> &'a str {
        match args.get(i + 1) {
            Some(v) => v,
            None => {
                eprintln!("keplearn: {} expects a value (see --help)", flag);
                std::process::exit(2);
            }
        }
    }
    fn parse_val<T: std::str::FromStr>(args: &[String], i: usize, flag: &str) -> T {
        let v = take_val(args, i, flag);
        v.parse().unwrap_or_else(|_| {
            eprintln!("keplearn: invalid value '{}' for {} (see --help)", v, flag);
            std::process::exit(2);
        })
    }
    let mut top_k: usize = 10;
    let mut sample: usize = 500;
    let mut target_col = "target".to_string();
    let mut quick_n: usize = 50;
    let mut timeout_ms: u64 = 60_000;
    let mut max_evals: u64 = 0;
    let mut seed: u64 = 0x9E3779B97F4A7C15;
    let mut max_depth: Option<usize> = None;
    let mut trace_path = String::new();
    let mut verbose = false;
    let mut timing = false;
    let mut dump_candidates_path = String::new();
    let mut out_path = String::new();
    let mut pretty = false;
    let mut format = "json".to_string();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--top-k" => {
                top_k = parse_val(&args, i, "--top-k");
                i += 2;
            }
            "--sample" => {
                sample = parse_val(&args, i, "--sample");
                i += 2;
            }
            "--target" => {
                target_col = take_val(&args, i, "--target").to_string();
                i += 2;
            }
            "--quick-n" => {
                quick_n = parse_val(&args, i, "--quick-n");
                i += 2;
            }
            "--timeout" => {
                timeout_ms = parse_val(&args, i, "--timeout");
                i += 2;
            }
            "--max-evals" => {
                max_evals = parse_val(&args, i, "--max-evals");
                i += 2;
            }
            "--seed" => {
                seed = parse_val(&args, i, "--seed");
                i += 2;
            }
            "--max-depth" => {
                max_depth = Some(parse_val(&args, i, "--max-depth"));
                i += 2;
            }
            "-o" | "--out" => {
                out_path = take_val(&args, i, "--out").to_string();
                i += 2;
            }
            "--format" => {
                format = take_val(&args, i, "--format").to_string();
                if format != "json" && format != "summary" {
                    eprintln!("keplearn: --format takes json or summary");
                    std::process::exit(2);
                }
                i += 2;
            }
            "--pretty" => {
                pretty = true;
                i += 1;
            }
            "--verbose" => {
                verbose = true;
                i += 1;
            }
            "--timing" => {
                timing = true;
                i += 1;
            }
            "--trace" => {
                trace_path = take_val(&args, i, "--trace").to_string();
                i += 2;
            }
            "--dump-candidates" => {
                dump_candidates_path = take_val(&args, i, "--dump-candidates").to_string();
                i += 2;
            }
            "--pool" | "--reject" | "--prune-cols" | "--reeval-cap" | "--max-arity" => {
                i += 2;
            }
            "--compose" => {
                i += 2;
            }
            other => {
                eprintln!("keplearn: unknown flag '{}' (see --help)", other);
                std::process::exit(2);
            }
        }
    }
    for (v, flag) in [
        (top_k, "--top-k"),
        (sample, "--sample"),
        (quick_n, "--quick-n"),
    ] {
        if v == 0 {
            eprintln!("keplearn: {} must be at least 1", flag);
            std::process::exit(2);
        }
    }
    if std::io::stdin().is_terminal() {
        eprintln!("keplearn: no input on stdin; expects a TSV or CSV (see --help)");
        eprintln!("usage: keplearn [OPTIONS] < data.tsv");
        std::process::exit(2);
    }
    run(Config {
        top_k,
        sample,
        target_col,
        quick_n,
        timeout_ms,
        max_evals,
        seed,
        max_depth,
        trace_path,
        dump_candidates_path,
        out_path,
        pretty,
        format,
        input_warnings: Vec::new(),
        verbose,
        timing,
    });
}
