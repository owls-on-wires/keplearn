use keplearn::{run, Config};
use std::env;
use std::io::IsTerminal;

const HELP: &str = "\
keplearn - symbolic regression engine

Reads a TSV or CSV with a header row on STDIN (the delimiter comes from
the header: tab if present, else comma) and searches for a closed-form
expression of the target column in terms of the remaining columns.

USAGE:
  keplearn [OPTIONS] < data.tsv

OPTIONS:
  --target NAME           target column name (default: \"target\")
  --top-k N               number of candidate models to emit (default: 10)
  --sample N              max training rows used by the search (default: 500)
  --quick-n N             rows for the quick pre-probe (default: 50)
  --timeout MS            wall-clock budget in milliseconds; best-so-far is
                          emitted on expiry (default: 0 = unbounded)
  --max-depth D           clamp search depth downward (default: compiled max)
  --trace FILE            append a JSONL event trace to FILE
  --dump-candidates FILE  write the full scored candidate list to FILE
  --seed N, --max-evals N accepted for compatibility; unused
  -h, --help              print this help and exit
  -V, --version           print the version and exit

OUTPUT (stdout, one JSON object):
  {
    \"results\": [
      {
        \"expr\":       \"(2.000000000000*x1+3.000000000000)\",
        \"model\":      \"(2.000000000000*x1+3.000000000000)\",
        \"r2\":         1.0,
        \"search_r2\":  1.0,
        \"holdout_r2\": 1.0,
        \"scale\":      1,
        \"offset\":     0,
        \"columns\":    [0],
        \"factor\":     \"direct\"
      }
      ... up to --top-k entries, best-first ...
    ],
    \"time_ms\":    17,
    \"stopped_by\": \"exact_exit\"
  }

FIELDS:
  model        closed form in x1..xN (i-th feature column, target
               excluded) with fitted constants baked in
  r2           R² of the model string evaluated on the supplied data
  search_r2    internal fit-stage score, kept for diagnostics; on noisy
               data it can sit far above r2
  holdout_r2   score on the internal 25% holdout, invariant to scale
               and offset
  expr, scale, offset, columns
               raw leaf form for symbolic tooling; expr references a
               column subset through `columns`
  stopped_by   exact_exit | frontier_exhausted | timeout

Diagnostics print to stderr; parse stdout starting at the first '{'.

USAGE NOTES:
  * Name the prediction column \"target\" or pass --target. Every other
    column is treated as a feature.
  * results[0].model is the best model and r2 is its score. Base any
    downstream decision on r2 or holdout_r2 rather than search_r2.
  * For batch runs set --timeout 60000. Fits that land an analytic law
    usually finish in milliseconds; the cap bounds the rest.
  * With many columns, run subsets of 2-5 at a time. Within a run,
    irrelevant columns fit to exponent 0 and drop out, but conditioning
    worsens as they accumulate. Column order has no effect.
  * The engine is built for analytic laws (ratios, powers, trig,
    exp/log). On noisy tabular data the best model may carry a low r2;
    that score still comes from evaluating the printed string.
  * Output is deterministic; --seed has no effect.
  * Input larger than 1 GiB is rejected.

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
    let mut timeout_ms: u64 = 0;
    let mut max_evals: u64 = 0;
    let mut seed: u64 = 0x9E3779B97F4A7C15;
    let mut max_depth: Option<usize> = None;
    let mut trace_path = String::new();
    let mut dump_candidates_path = String::new();
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
    });
}
