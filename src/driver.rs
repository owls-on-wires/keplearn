use crate::*;

pub struct Config {
    pub top_k: usize,
    pub sample: usize,
    pub target_col: String,
    pub quick_n: usize,
    pub timeout_ms: u64,
    pub max_evals: u64,
    pub seed: u64,
    pub max_depth: Option<usize>,
    pub trace_path: String,
    pub dump_candidates_path: String,
    pub out_path: String,
    pub pretty: bool,
    pub format: String,
    pub input_warnings: Vec<String>,
    pub verbose: bool,
    pub timing: bool,
    pub max_memory_mb: u64,
}

pub(crate) fn split_row(line: &str, delim: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                cur.push(c);
            }
        } else if c == '"' && cur.is_empty() {
            in_quotes = true;
        } else if c == delim {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    out.push(cur);
    out
}

pub(crate) struct Ingest {
    pub(crate) features: Vec<Vec<f64>>,
    pub(crate) target: Vec<f64>,
    pub(crate) n_cols: usize,
    pub(crate) warnings: Vec<String>,
    pub(crate) q_features: Vec<Vec<f64>>,
    pub(crate) q_target: Vec<f64>,
    pub(crate) names: Vec<String>,
    pub(crate) feature_settings: Vec<bool>,
}

pub(crate) fn cell_sig(s: &str) -> usize {
    let t = s.trim().trim_start_matches(['+', '-']);
    let mant = t.split(['e', 'E']).next().unwrap_or("");
    if mant.is_empty() || !mant.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return 0;
    }
    let digits: String = mant.chars().filter(|c| c.is_ascii_digit()).collect();
    let digits = digits.trim_start_matches('0');
    if !mant.contains('.') {
        return digits.trim_end_matches('0').len().max(1);
    }
    digits.len()
}

pub(crate) fn cell_quantum(s: &str) -> f64 {
    let t = s.trim().trim_start_matches(['+', '-']);
    let (mant, exp) = match t.find(['e', 'E']) {
        Some(i) => (&t[..i], t[i + 1..].parse::<i32>().unwrap_or(0)),
        None => (t, 0),
    };
    if mant.is_empty() || !mant.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return 0.0;
    }
    let decimals = mant.find('.').map_or(0, |i| (mant.len() - i - 1) as i32);
    let zeros = if mant.contains('.') {
        0
    } else {
        let t = mant.trim_start_matches('0');
        (t.len() - t.trim_end_matches('0').len()) as i32
    };
    10f64.powi(exp - decimals + zeros)
}

fn printable(s: &str) -> String {
    let mut out: String = s
        .chars()
        .take(80)
        .map(|c| {
            if c.is_control() || c == '\u{fffd}' {
                format!("\\u{{{:x}}}", c as u32)
            } else {
                c.to_string()
            }
        })
        .collect();
    if s.chars().count() > 80 {
        out.push('…');
    }
    out
}

pub(crate) fn design_grid(c: &[f64], q: &[f64]) -> bool {
    let mut vq: Vec<(f64, f64)> = c.iter().zip(q).map(|(v, q)| (*v, *q)).collect();
    vq.sort_by(|a, b| a.0.total_cmp(&b.0));
    vq.dedup_by(|a, b| a.0 == b.0);
    if 2 * vq.len() > c.len() || vq.len() < 2 {
        return false;
    }
    let mut gaps: Vec<f64> = vq
        .windows(2)
        .map(|w| {
            let qq = w[0].1.max(w[1].1);
            if qq > 0.0 {
                (w[1].0 - w[0].0) / qq
            } else {
                f64::INFINITY
            }
        })
        .collect();
    gaps.sort_by(|a, b| a.total_cmp(b));
    gaps[gaps.len() / 2] >= GRID_MIN_GAP
}

pub(crate) fn ingest(input: &str, target_col: &str) -> Result<Ingest, String> {
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    let head: String = input.chars().take(4096).collect();
    let bad = head
        .chars()
        .filter(|c| {
            *c == '\0' || *c == '\u{fffd}' || (c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
        })
        .count();
    if bad * 10 > head.chars().count().max(1) {
        return Err(
            "the input is not text (binary bytes); expected a TSV or CSV table with a header row"
                .into(),
        );
    }
    if matches!(input.trim_start().chars().next(), Some('{') | Some('[')) {
        return Err(
            "the input looks like JSON; expected a TSV or CSV table with a header row".into(),
        );
    }
    let lines: Vec<&str> = input.lines().collect();
    if lines.len() < 2 {
        return Err(
            "no data on stdin (expected a TSV or CSV with a header row and at least one data row)"
                .into(),
        );
    }
    let head_line = lines[0].trim_end_matches('\r');
    let delim = if head_line.contains('\t') {
        '\t'
    } else if head_line.contains(',') {
        ','
    } else {
        return Err(
            format!(
                "the header has a single column ('{}'); a table needs the target and at least one feature column, separated by tabs or commas",
                printable(head_line.trim())
            ),
        );
    };
    let header: Vec<String> = split_row(head_line, delim);
    let mut warnings: Vec<String> = Vec::new();
    let mut seen_names: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut warned: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for h in header.iter() {
        let t = h.trim();
        if !seen_names.insert(t) && !t.is_empty() && warned.insert(t) {
            warnings
                .push(
                    if t == target_col {
                        format!(
                            "duplicate column name '{}' in header; the first such column is the target",
                            t
                        )
                    } else {
                        format!(
                            "duplicate column name '{}' in header; the later columns are renamed '{}_2', '{}_3', ...",
                            t, t, t
                        )
                    },
                );
        }
    }
    if header.iter().all(|h| h.trim().parse::<f64>().is_ok())
        && !header.iter().any(|h| h.trim() == target_col)
    {
        return Err(
            format!(
                "the first row ({}) holds numbers, not column names; add a header row naming the columns (the target '{}' among them)",
                printable(& header.iter().map(| h | h.trim()).collect::< Vec < _ >> ()
                .join(", ")), printable(target_col)
            ),
        );
    }
    let target_idx = header
        .iter()
        .position(|h| h.trim() == target_col)
        .ok_or_else(|| {
            format!(
                "target column '{}' not found; columns are: {}",
                target_col,
                header
                    .iter()
                    .map(|h| printable(h.trim()))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;
    {
        let mut counts: std::collections::BTreeMap<usize, usize> =
            std::collections::BTreeMap::new();
        let mut matching = false;
        for l in &lines[1..] {
            let l = l.trim_end_matches('\r');
            if l.is_empty() {
                continue;
            }
            let k = split_row(l, delim).len();
            if k == header.len() {
                matching = true;
                break;
            }
            *counts.entry(k).or_insert(0) += 1;
        }
        if !matching {
            if let Some((&k, _)) = counts
                .iter()
                .max_by_key(|(k, c)| (**c, std::cmp::Reverse(**k)))
            {
                return Err(
                    format!(
                        "no data row has the header's {} fields (most rows have {}); check that every row has a value for each column",
                        header.len(), k
                    ),
                );
            }
        }
    }
    let mut is_numeric = vec![false; header.len()];
    let mut unseen = header.len();
    for l in &lines[1..] {
        if unseen == 0 {
            break;
        }
        let vals = split_row(l.trim_end_matches('\r'), delim);
        if vals.len() != header.len() {
            continue;
        }
        for (i, v) in vals.iter().enumerate() {
            if !is_numeric[i] && v.trim().parse::<f64>().is_ok_and(|v| v.is_finite()) {
                is_numeric[i] = true;
                unseen -= 1;
            }
        }
    }
    let numeric = |i: usize| is_numeric[i];
    let mut feat_idx: Vec<usize> = Vec::new();
    for i in (0..header.len()).filter(|&i| i != target_idx) {
        if header[i].trim() == target_col {
            warnings.push(format!(
                "a second column is named '{}' like the target; it is dropped",
                target_col
            ));
            continue;
        }
        if numeric(i) {
            feat_idx.push(i);
        } else {
            warnings.push(format!(
                "column '{}' has no numeric values; it is dropped",
                header[i].trim()
            ));
        }
    }
    let n_cols = feat_idx.len();
    if n_cols == 0 {
        return Err("input has a target column but no numeric feature columns".into());
    }
    let mut features: Vec<Vec<f64>> = vec![Vec::new(); n_cols];
    let mut target: Vec<f64> = Vec::new();
    let mut q_features: Vec<Vec<f64>> = vec![Vec::new(); n_cols];
    let mut q_target: Vec<f64> = Vec::new();
    let mut s_features: Vec<Vec<usize>> = vec![Vec::new(); n_cols];
    let mut s_target: Vec<usize> = Vec::new();
    let mut skipped = 0usize;
    for line in &lines[1..] {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let vals = split_row(line, delim);
        if vals.len() != header.len() {
            skipped += 1;
            continue;
        }
        let tv: f64 = match vals[target_idx].trim().parse::<f64>() {
            Ok(v) if v.is_finite() => v,
            _ => {
                skipped += 1;
                continue;
            }
        };
        let mut fv = Vec::with_capacity(n_cols);
        let mut ok = true;
        for &fi in &feat_idx {
            match vals[fi].trim().parse::<f64>() {
                Ok(v) if v.is_finite() => fv.push(v),
                _ => {
                    ok = false;
                    break;
                }
            }
        }
        if !ok {
            skipped += 1;
            continue;
        }
        for (j, v) in fv.into_iter().enumerate() {
            features[j].push(v);
            q_features[j].push(cell_quantum(&vals[feat_idx[j]]));
            s_features[j].push(cell_sig(&vals[feat_idx[j]]));
        }
        target.push(tv);
        q_target.push(cell_quantum(&vals[target_idx]));
        s_target.push(cell_sig(&vals[target_idx]));
    }
    let n_rows = target.len();
    if skipped > 0 {
        warnings.push(format!(
            "skipped {} of {} data rows (wrong field count, unparseable, or non-finite values)",
            skipped,
            n_rows + skipped
        ));
    }
    if n_rows == 0 {
        return Err("no usable data rows".into());
    }
    let mut names: Vec<String> = feat_idx
        .iter()
        .map(|&i| header[i].trim().to_string())
        .collect();
    {
        let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for nm in names.iter_mut() {
            let k = seen.entry(nm.clone()).or_insert(0);
            *k += 1;
            if *k > 1 {
                *nm = format!("{}_{}", nm, k);
            }
        }
    }
    let mut j = 0;
    while j < features.len() {
        let nm = names[j].to_lowercase();
        let named_index = nm.is_empty()
            || matches!(
                nm.as_str(),
                "rownames" | "row.names" | "row_names" | "index" | "id"
            )
            || nm.starts_with("unnamed:");
        let c = &features[j];
        let counts = !c.is_empty()
            && (c[0] == 0.0 || c[0] == 1.0)
            && c.iter().enumerate().all(|(r, v)| *v == c[0] + r as f64);
        if named_index && counts && features.len() > 1 {
            warnings.push(format!(
                "column '{}' is a row index (it counts the rows); it is dropped",
                names[j]
            ));
            features.remove(j);
            q_features.remove(j);
            s_features.remove(j);
            names.remove(j);
        } else {
            j += 1;
        }
    }
    let mut j = 0;
    while j < features.len() {
        let c = &features[j];
        if n_rows > 1 && c.iter().all(|v| *v == c[0]) {
            warnings.push(format!(
                "column '{}' has the same value ({}) on every row; it is dropped",
                names[j],
                crate::output::tidy_number(c[0], 13)
            ));
            features.remove(j);
            q_features.remove(j);
            s_features.remove(j);
            names.remove(j);
        } else {
            j += 1;
        }
    }
    if features.is_empty() {
        return Err(
            "every feature column has the same value on every row; nothing can explain the target"
                .into(),
        );
    }
    if features.len() > SCREEN_COLUMNS {
        let keep = screen_columns(&features, &target, SCREEN_COLUMNS);
        warnings
            .push(
                format!(
                    "the table has {} feature columns; the search uses the {} most related to the target one at a time ({}), and laws are found far more readily in subsets of 2 to 5 columns",
                    features.len(), SCREEN_COLUMNS, keep.iter().map(|& j | names[j]
                    .as_str()).collect::< Vec < _ >> ().join(", ")
                ),
            );
        fn pick<T>(v: &mut Vec<T>, keep: &[usize]) {
            let mut i = 0;
            v.retain(|_| {
                i += 1;
                keep.binary_search(&(i - 1)).is_ok()
            });
        }
        pick(&mut features, &keep);
        pick(&mut q_features, &keep);
        pick(&mut s_features, &keep);
        pick(&mut names, &keep);
    } else if features.len() > WIDE_COLUMNS {
        warnings
            .push(
                format!(
                    "the table has {} feature columns; within a budget the search covers few of their combinations, so laws are found far more readily in subsets of 2 to 5 columns",
                    features.len()
                ),
            );
    }
    let n_cols = features.len();
    let big = |v: &[f64]| v.iter().any(|x| x.abs() > OVERFLOW_ABS);
    for (j, c) in features.iter().enumerate() {
        if big(c) {
            warnings.push(format!(
                "column '{}' has values beyond {:e}, whose squares overflow; rescale it",
                names[j], OVERFLOW_ABS
            ));
        }
    }
    if big(&target) {
        warnings.push(format!(
            "the target has values beyond {:e}, whose squares overflow; rescale it",
            OVERFLOW_ABS
        ));
    }
    fn column_quanta(v: &[f64], q: &mut [f64], sig: &[usize]) {
        let n = q.iter().filter(|x| **x > 0.0).count();
        if n == 0 {
            return;
        }
        let qa = q
            .iter()
            .copied()
            .filter(|x| *x > 0.0)
            .fold(f64::INFINITY, f64::min);
        let abs_share = q
            .iter()
            .filter(|x| **x > 0.0 && (**x - qa).abs() <= 1e-9 * qa)
            .count();
        let mut counts: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &d in sig.iter().filter(|d| **d > 0) {
            *counts.entry(d).or_insert(0) += 1;
        }
        let (smode, rel_share) = counts
            .iter()
            .max_by_key(|(d, c)| (**c, **d))
            .map(|(d, c)| (*d, *c))
            .unwrap_or((0, 0));
        for (x, qq) in v.iter().zip(q.iter_mut()) {
            if *qq == 0.0 {
                continue;
            }
            *qq = if abs_share >= rel_share || *x == 0.0 || smode == 0 {
                qa
            } else {
                10f64.powi(x.abs().log10().floor() as i32 - smode as i32 + 1)
            };
        }
    }
    for ((c, q), sg) in features.iter().zip(q_features.iter_mut()).zip(&s_features) {
        column_quanta(c, q, sg);
    }
    let feature_settings: Vec<bool> = s_features
        .iter()
        .zip(&features)
        .zip(&q_features)
        .map(|((sg, c), q)| {
            let mut counts: std::collections::HashMap<usize, usize> =
                std::collections::HashMap::new();
            for &d in sg.iter().filter(|d| **d > 0) {
                *counts.entry(d).or_insert(0) += 1;
            }
            let mode = counts
                .into_iter()
                .max_by_key(|(d, c)| (*c, *d))
                .map_or(0, |(d, _)| d);
            mode <= SETTING_MAX_DIGITS || design_grid(c, q)
        })
        .collect();
    column_quanta(&target, &mut q_target, &s_target);
    Ok(Ingest {
        features,
        target,
        n_cols,
        warnings,
        q_features,
        q_target,
        names,
        feature_settings,
    })
}

static STDOUT_CLOSED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn mark_stdout_closed() {
    STDOUT_CLOSED.store(true, std::sync::atomic::Ordering::Relaxed);
}

pub fn write_stdout(s: &str) -> std::io::Result<()> {
    if STDOUT_CLOSED.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(std::io::Error::other("stdout is closed"));
    }
    let mut o = std::io::stdout().lock();
    o.write_all(s.as_bytes())?;
    o.flush()
}

fn refuse_closed_stdout(out_path: &str) {
    if out_path.is_empty() && STDOUT_CLOSED.load(std::sync::atomic::Ordering::Relaxed) {
        eprintln!("keplearn: cannot write the result: stdout is closed");
        std::process::exit(1);
    }
}

pub fn run(cfg: Config) {
    refuse_closed_stdout(&cfg.out_path);
    if !cfg.trace_path.is_empty() {
        let f = match fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&cfg.trace_path)
        {
            Ok(f) => f,
            Err(e) => {
                eprintln!(
                    "keplearn: cannot open trace file '{}': {}",
                    cfg.trace_path, e
                );
                std::process::exit(1);
            }
        };
        let _ = TRACER.set(Mutex::new(BufWriter::new(f)));
    }
    if stdin_file_len().is_some_and(|n| n > INPUT_BYTE_CAP) {
        eprintln!(
            "keplearn: input exceeds the {} MiB cap",
            INPUT_BYTE_CAP >> 20
        );
        std::process::exit(1);
    }
    let mut raw = Vec::new();
    if let Err(e) = std::io::stdin()
        .take(INPUT_BYTE_CAP + 1)
        .read_to_end(&mut raw)
    {
        eprintln!("keplearn: failed to read stdin: {}", e);
        std::process::exit(1);
    }
    if raw.len() as u64 > INPUT_BYTE_CAP {
        eprintln!(
            "keplearn: input exceeds the {} MiB cap",
            INPUT_BYTE_CAP >> 20
        );
        std::process::exit(1);
    }
    let mut cfg = cfg;
    let input = match String::from_utf8(raw) {
        Ok(s) => s,
        Err(e) => {
            cfg.input_warnings.push(
                "the input is not valid UTF-8; undecodable bytes in names are shown as \u{fffd}"
                    .to_string(),
            );
            String::from_utf8_lossy(e.as_bytes()).into_owned()
        }
    };
    let out_path = cfg.out_path.clone();
    let format = cfg.format.clone();
    let rendered = match run_text(&input, cfg) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("keplearn: {}", e);
            std::process::exit(1);
        }
    };
    let rendered = if format == "summary" {
        crate::summary::render(&rendered)
    } else {
        rendered
    };
    if out_path.is_empty() {
        if let Err(e) = write_stdout(&rendered) {
            eprintln!("keplearn: cannot write the result to stdout: {}", e);
            std::process::exit(1);
        }
    } else if let Err(e) = fs::write(&out_path, &rendered) {
        eprintln!("keplearn: cannot write output file '{}': {}", out_path, e);
        std::process::exit(1);
    }
}

fn constant_target(target: &[f64]) -> Option<f64> {
    let first = *target.first()?;
    (first.is_finite() && target.iter().all(|v| *v == first)).then_some(first)
}

pub fn run_text(input: &str, cfg: Config) -> Result<String, String> {
    let Config {
        top_k,
        sample,
        target_col,
        quick_n,
        timeout_ms,
        max_depth,
        dump_candidates_path,
        pretty,
        input_warnings: cfg_input_warnings,
        verbose,
        timing,
        max_memory_mb,
        ..
    } = cfg;
    crate::heap::set_limit_mb(max_memory_mb);
    crate::output::set_verbose(verbose);
    let t0 = Instant::now();
    let eff_max_depth = max_depth.unwrap_or(MAX_DEPTH).min(MAX_DEPTH);
    crate::guard::set_floor(0.0);
    let global_max_stack = 2usize;
    let mut ing = ingest(input, &target_col)?;
    ing.warnings.splice(0..0, cfg_input_warnings);
    for w in &ing.warnings {
        eprintln!("keplearn: {}", w);
    }
    let n_rows_in = ing.target.len();
    let Ingest {
        mut features,
        mut target,
        n_cols,
        mut q_features,
        mut q_target,
        names,
        warnings: ingest_warnings,
        feature_settings,
    } = ing;
    let n_rows = target.len();
    let few = n_rows < fit_min_rows(2);
    let constant = constant_target(&target).filter(|_| n_rows >= 2);
    if constant.is_some() || few {
        let mut warnings = ingest_warnings.clone();
        let (results, verdict, reason) = match constant {
            Some(v) => {
                warnings
                    .push(
                        format!(
                            "the target is {} on every row; nothing to search", crate
                            ::output::tidy_number(v, 13)
                        ),
                    );
                (
                    vec![
                        crate ::output::ResultObj::constant(v, "law", Some(usize::from(!
                        crate ::guard::structural_constant(v))), None)
                    ],
                    "law",
                    None,
                )
            }
            None => {
                (
                    Vec::new(),
                    "no law found",
                    Some(
                        format!(
                            "the table has {} data row{}; a law needs at least {} (and {} if it has a fitted constant)",
                            n_rows, if n_rows == 1 { "" } else { "s" }, fit_min_rows(2),
                            min_law_rows()
                        ),
                    ),
                )
            }
        };
        for w in &warnings[ingest_warnings.len().min(warnings.len())..] {
            eprintln!("keplearn: {}", w);
        }
        let oj = crate::output::Envelope {
            results,
            verdict,
            reason,
            stopped_by: if constant.is_some() {
                "exact_exit"
            } else {
                "frontier_exhausted"
            },
            validation: if n_rows >= HOLDOUT_MIN_ROWS {
                "holdout"
            } else {
                "leave-one-out"
            },
            noise_floor: 0.0,
            target: &target_col,
            columns: &names,
            rows: n_rows_in,
            warnings: &warnings,
            time_ms: timing.then_some(0),
        }
        .json();
        let mut rendered = if pretty { json_pretty(&oj) } else { oj };
        rendered.push('\n');
        return Ok(rendered);
    }
    if let Some(j) = features
        .iter()
        .position(|c| c.len() == target.len() && c.iter().zip(&target).all(|(a, b)| a == b))
    {
        let mut warnings = ingest_warnings.clone();
        warnings.push(format!(
            "column '{}' equals the target on every row",
            names[j]
        ));
        let model = format!("x{}", j + 1);
        let r = crate::output::ResultObj {
            expr: model.clone(),
            model: model.clone(),
            r2: Some(1.0),
            search_r2: Some(1.0),
            holdout_r2: Some(1.0),
            scale: 1.0,
            offset: 0.0,
            columns: (0..n_cols).collect(),
            uses: vec![names[j].clone()],
            structure: Some(format!("c1*{} + b", model)),
            canonical: Some(model.clone()),
            verdict: "law",
            symbolic: model.clone(),
            named: crate::output::named_model(&model, &names),
            free_constants: Some(0),
            within_precision: Some(true),
            largest_miss: Some((
                0.0,
                target
                    .iter()
                    .copied()
                    .find(|v| v.is_finite())
                    .unwrap_or(0.0),
            )),
        };
        for w in &warnings[ingest_warnings.len().min(warnings.len())..] {
            eprintln!("keplearn: {}", w);
        }
        let oj = crate::output::Envelope {
            results: vec![r],
            verdict: "law",
            reason: None,
            stopped_by: "exact_exit",
            validation: if n_rows >= HOLDOUT_MIN_ROWS {
                "holdout"
            } else {
                "leave-one-out"
            },
            noise_floor: 0.0,
            target: &target_col,
            columns: &names,
            rows: n_rows_in,
            warnings: &warnings,
            time_ms: timing.then_some(0),
        }
        .json();
        let mut rendered = if pretty { json_pretty(&oj) } else { oj };
        rendered.push('\n');
        return Ok(rendered);
    }
    let negated =
        target.iter().any(|v| v.is_finite()) && target.iter().all(|v| !v.is_finite() || *v < 0.0);
    if negated {
        for v in target.iter_mut() {
            *v = -*v;
        }
    }
    let holdout_n_full = if n_rows >= HOLDOUT_MIN_ROWS {
        n_rows / HOLDOUT_FRAC_DEN
    } else {
        0
    };
    let train_n = n_rows - holdout_n_full;
    if holdout_n_full > 0 {
        let mut idx: Vec<usize> = (0..n_rows).collect();
        let mut seed: u64 = HOLDOUT_SHUFFLE_SEED;
        for i in (1..n_rows).rev() {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let j = ((seed >> 33) as usize) % (i + 1);
            idx.swap(i, j);
        }
        let new_target: Vec<f64> = idx.iter().map(|&k| target[k]).collect();
        target = new_target;
        for col in features.iter_mut() {
            let reordered: Vec<f64> = idx.iter().map(|&k| col[k]).collect();
            *col = reordered;
        }
        q_target = idx.iter().map(|&k| q_target[k]).collect();
        for col in q_features.iter_mut() {
            *col = idx.iter().map(|&k| col[k]).collect();
        }
    }
    crate::guard::set_quanta(&q_features, &q_target);
    crate::guard::set_relative(false);
    {
        let t = &target[..train_n];
        let m = t.iter().sum::<f64>() / train_n.max(1) as f64;
        let vy = t.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / train_n.max(1) as f64;
        let e = q_target[..train_n]
            .iter()
            .map(|q| q * q / 12.0)
            .sum::<f64>()
            / train_n.max(1) as f64;
        let f = if vy > 0.0 { e / vy } else { 0.0 };
        crate::guard::set_floor(if f.is_finite() && f > NOISE_FLOOR_MIN {
            f
        } else {
            0.0
        });
    }
    let sample_n = sample.min(train_n.max(1));
    let hold_n = if holdout_n_full >= HOLDOUT_MIN_ACTIVE {
        holdout_n_full.min(HOLDOUT_MAX)
    } else {
        0
    };
    if crate::output::verbose() {
        eprintln!(
            "Data: {} rows x {} cols, sample={}, holdout={}",
            n_rows, n_cols, sample_n, hold_n
        );
    }
    trace(&format!(
        "{{\"ev\":\"start\",\"n_cols\":{},\"n_rows\":{}}}",
        n_cols, n_rows
    ));
    let mut evaluator = Evaluator::new(global_max_stack, sample_n.max(hold_n));
    let search_ms = if timeout_ms > 0 {
        ((timeout_ms as f64 * SEARCH_SHARE) as u64).max(1)
    } else {
        0
    };
    arm_deadline_at((search_ms > 0).then(|| t0 + std::time::Duration::from_millis(search_ms)));
    let n_evaluated: u64 = 0;
    let qn = quick_n.min(sample_n);
    let h_lo = train_n;
    let h_hi = train_n + hold_n;
    let ho_keep: Vec<bool> = {
        let mut keep = vec![false; hold_n];
        let mut mags: Vec<f64> = Vec::with_capacity(hold_n);
        for (j, i) in (h_lo..h_hi).enumerate() {
            if target[i].is_finite() {
                keep[j] = true;
                mags.push(target[i].abs());
            }
        }
        let m = mags.len();
        if m >= OUTLIER_TRIM_MIN_M {
            let k = (m / OUTLIER_TRIM_DEN).max(OUTLIER_TRIM_MIN);
            let mut by_mag: Vec<usize> = (0..hold_n).filter(|&j| keep[j]).collect();
            by_mag.sort_by(|&a, &b| {
                target[h_lo + b]
                    .abs()
                    .total_cmp(&target[h_lo + a].abs())
                    .then(a.cmp(&b))
            });
            for &j in by_mag.iter().take(k) {
                keep[j] = false;
            }
        }
        keep
    };
    let ho_kept_n = ho_keep.iter().filter(|&&k| k).count();
    let ho_min_rows = (ho_kept_n * HOLDOUT_COVER_NUM / HOLDOUT_COVER_DEN).max(HOLDOUT_COVER_MIN);
    let hn_f = (ho_kept_n as f64).max(1.0);
    let mut seq: u64 = 0;
    let mut frontier = Frontier::new();
    {
        let base_inputs: Vec<&[f64]> = features.iter().map(|c| &c[..sample_n]).collect();
        let root_probe = quick_probe(&target[..sample_n], &base_inputs, qn);
        frontier.push(Node {
            target: Rc::new(target[..sample_n].to_vec()),
            chain: Vec::new(),
            vcols: Vec::new(),
            depth: 0,
            complexity: 0,
            probe_r2: root_probe,
            plane: PLANE_ROOT,
            priority: 0.0,
            seq: 0,
            desc: "y".to_string(),
            forced_finalist: Vec::new(),
            basis_ok: false,
            product_ok: false,
        });
    }
    let mut expansions = [0usize; MAX_DEPTH + 1];
    let mut mat_expanded = 0usize;
    let mut best_gate = -1.0f64;
    let mut best_confirmed = -1.0f64;
    let mut finalists: Vec<Cand> = Vec::new();
    let mut pareto: Vec<Option<(f64, Cand)>> = vec![None; FRONT_TIERS];
    fn merge_pareto(pareto: &mut [Option<(f64, Cand)>], c: &Cand, depth: usize) {
        let tier = c.nodes.min(FRONT_TIERS - 1);
        let ev_ok =
            trace_enabled() && HARVEST_EVS.fetch_add(1, AtomicOrdering::Relaxed) < HARVEST_EV_CAP;
        let stage = format!("d{}", depth);
        match pareto[tier].take() {
            Some((cur_g, cur_c)) if c.gate_r2 <= cur_g => {
                if ev_ok {
                    trace(
                        &format!(
                            "{{\"ev\":\"harvest\",\"stage\":\"{}\",\"expr\":\"{}\",\"factor\":\"{}\",\"r2\":{},\"tier\":{},\"kept\":false,\"beaten_by\":{{\"expr\":\"{}\",\"r2\":{}}}}}",
                            stage, json_esc(& c.expr), json_esc(& c.desc), jnum(c
                            .gate_r2), tier, json_esc(& cur_c.expr), jnum(cur_g)
                        ),
                    );
                }
                pareto[tier] = Some((cur_g, cur_c));
            }
            incumbent => {
                if ev_ok {
                    if let Some((_, cur_c)) = &incumbent {
                        trace(
                            &format!(
                                "{{\"ev\":\"harvest\",\"stage\":\"{}\",\"expr\":\"{}\",\"factor\":\"{}\",\"r2\":{},\"tier\":{},\"kept\":false,\"beaten_by\":{{\"expr\":\"{}\",\"r2\":{}}}}}",
                                stage, json_esc(& cur_c.expr), json_esc(& cur_c.desc),
                                jnum(cur_c.gate_r2), tier, json_esc(& c.expr), jnum(c
                                .gate_r2)
                            ),
                        );
                    }
                    trace(
                        &format!(
                            "{{\"ev\":\"harvest\",\"stage\":\"{}\",\"expr\":\"{}\",\"factor\":\"{}\",\"r2\":{},\"tier\":{},\"kept\":true}}",
                            stage, json_esc(& c.expr), json_esc(& c.desc), jnum(c
                            .gate_r2), tier
                        ),
                    );
                }
                pareto[tier] = Some((c.gate_r2, c.clone()));
            }
        }
    }
    let mut early = false;
    let mut bound: Option<usize> = None;
    let mut timed_out = false;
    let mut memory_stop = false;
    let mut turn_ctr: u64 = 0;
    let mut any_expanded = false;
    let mut lane;
    'main: loop {
        if !timed_out
            && search_ms > 0
            && any_expanded
            && t0.elapsed().as_millis() as u64 >= search_ms
        {
            timed_out = true;
        }
        if !timed_out && any_expanded && crate::heap::over_soft() {
            timed_out = true;
            memory_stop = true;
        }
        if timed_out {
            let Some(node) = frontier.pop_forced() else {
                break;
            };
            collect_forced(&node, &mut finalists);
            continue;
        }
        lane = match turn_ctr % 3 {
            2 => {
                if (turn_ctr / 3) % 2 == 0 {
                    Lane::Structural
                } else {
                    Lane::Sweep
                }
            }
            _ => Lane::Tunnel,
        };
        let node = loop {
            let Some(n) = frontier.pop_lane(lane) else {
                break 'main;
            };
            if bound.is_some_and(|b| n.complexity >= b) {
                continue;
            }
            let depth_cap = depth_cap_at(n.depth, n_cols);
            if expansions[n.depth] >= depth_cap {
                continue;
            }
            if matches!(n.chain.last(), Some(Red::Mat { .. })) {
                if mat_expanded >= MAT_EXPAND_CAP {
                    continue;
                }
                mat_expanded += 1;
            }
            break n;
        };
        expansions[node.depth] += 1;
        any_expanded = true;
        for (model, cr2) in node.forced_finalist.clone() {
            if let Ok(parsed) = Parser::parse(&model) {
                let nodes = parsed.node_count();
                if cr2 >= EXACT_EXIT_R2 {
                    let conf = if hold_n > 0 {
                        holdout_model_r2(
                            &model,
                            &features,
                            &target,
                            h_lo,
                            h_hi,
                            &ho_keep,
                            ho_min_rows,
                            &mut evaluator,
                        )
                        .map(|(rh, _)| cr2.min(rh))
                    } else {
                        Some(cr2)
                    };
                    if conf.is_some_and(|v| v >= EXACT_EXIT_R2) {
                        match crate::guard::exit_check(&model, &features, &target, n_cols, train_n)
                        {
                            Some((_, true)) => {
                                best_confirmed = best_confirmed.max(EXACT_EXIT_R2);
                            }
                            Some((nodes, false)) => {
                                bound = Some(bound.map_or(nodes, |b: usize| b.min(nodes)));
                            }
                            None => {}
                        }
                    }
                }
                if trace_enabled() {
                    trace(
                        &format!(
                            "{{\"ev\":\"two_mono_finalist\",\"depth\":{},\"desc\":\"{}\",\"model\":\"{}\",\"r2\":{}}}",
                            node.depth, json_esc(& node.desc), json_esc(& model),
                            jnum(cr2)
                        ),
                    );
                }
                finalists.push(Cand {
                    model,
                    desc: node.desc.clone(),
                    expr: "two_mono".to_string(),
                    columns: Vec::new(),
                    scale: 1.0,
                    offset: 0.0,
                    fit_r2: cr2,
                    gate_r2: cr2,
                    nodes,
                    root_raw: false,
                });
            }
        }
        if trace_enabled() {
            let red = node
                .chain
                .last()
                .map(red_desc)
                .unwrap_or_else(|| "root".to_string());
            trace(
                &format!(
                    "{{\"ev\":\"expand\",\"depth\":{},\"reduction\":\"{}\",\"priority\":{},\"src\":\"{}\",\"chain\":\"{}\"}}",
                    node.depth, json_esc(& red), jnum(node.priority), match lane {
                    Lane::Tunnel => "tunnel", Lane::Sweep => "sweep", Lane::Structural =>
                    "structural" }, json_esc(& node.desc)
                ),
            );
        }
        turn_ctr += 1;
        let mut inputs: Vec<&[f64]> = features.iter().map(|c| &c[..sample_n]).collect();
        for v in &node.vcols {
            inputs.push(&v.data[..sample_n]);
        }
        SEARCH_COUNT.fetch_add(1, AtomicOrdering::Relaxed);
        if !node.vcols.is_empty() {
            for (vidx, v) in node.vcols.iter().enumerate() {
                let (r2, a, b) = r2_score(&node.target[..sample_n], &v.data[..sample_n]);
                if !(r2 >= NEAR_EXIT_R2) {
                    continue;
                }
                let vh = Hit {
                    expr: "x1".to_string(),
                    r2,
                    scale: a,
                    offset: b,
                    columns: vec![n_cols + vidx],
                    gate_r2: r2,
                };
                if let Some(c) = build_candidate(&node.chain, &node.desc, &vh, &node.vcols, n_cols)
                {
                    if c.gate_r2 > best_gate {
                        best_gate = c.gate_r2;
                    }
                    finalists.push(c.clone());
                    merge_pareto(&mut pareto, &c, node.depth);
                }
            }
        }
        for mh in mono_fit(&inputs, &node.target, sample_n) {
            if let Some(c) = build_candidate(&node.chain, &node.desc, &mh, &node.vcols, n_cols) {
                if c.gate_r2 > best_gate {
                    best_gate = c.gate_r2;
                }
                if c.gate_r2 >= NEAR_EXIT_R2 && c.gate_r2 > best_confirmed {
                    let conf = if hold_n > 0 {
                        holdout_model_r2(
                            &c.model,
                            &features,
                            &target,
                            h_lo,
                            h_hi,
                            &ho_keep,
                            ho_min_rows,
                            &mut evaluator,
                        )
                        .map(|(rh, _)| c.gate_r2.min(rh))
                    } else {
                        match crate::guard::exit_check(
                            &c.model, &features, &target, n_cols, train_n,
                        ) {
                            Some((_, true)) => Some(c.gate_r2),
                            _ => None,
                        }
                    };
                    if let Some(v) = conf {
                        if v > best_confirmed {
                            best_confirmed = v;
                        }
                    }
                }
                if trace_enabled() {
                    trace(
                        &format!(
                            "{{\"ev\":\"monofit\",\"depth\":{},\"desc\":\"{}\",\"expr\":\"{}\",\"r2\":{},\"gate\":{}}}",
                            node.depth, json_esc(& node.desc), json_esc(& mh.expr),
                            jnum(mh.r2), jnum(c.gate_r2)
                        ),
                    );
                }
                finalists.push(c.clone());
                merge_pareto(&mut pareto, &c, node.depth);
            }
        }
        if best_confirmed >= EXACT_EXIT_R2 {
            trace(&format!(
                "{{\"ev\":\"early_exit\",\"stage\":\"d{}\",\"r2\":{}}}",
                node.depth,
                jnum(best_confirmed)
            ));
            early = true;
            break;
        }
        if node.depth >= eff_max_depth {
            continue;
        }
        if matches!(node.chain.last(), Some(Red::Mat { .. })) {
            continue;
        }
        let mut children = expand_children(&node, &features, n_cols, sample_n, qn, &mut seq);
        if node.depth == 0 {
            let base_inputs: Vec<&[f64]> = features.iter().map(|c| &c[..sample_n]).collect();
            let basis = crate::fit::build_probe_basis(&base_inputs, sample_n);
            let rows: Vec<usize> = if sample_n > 256 {
                (0..256).map(|k| k * sample_n / 256).collect()
            } else {
                (0..sample_n).collect()
            };
            let atoms = crate::fit::library_atoms(&features, n_cols, &rows);
            let score = |t: &[f64]| {
                if out_of_budget() {
                    return 0.0;
                }
                let tr: Vec<f64> = rows.iter().map(|&r| t[r]).collect();
                crate::fit::joint_probe(t, &base_inputs, &basis)
                    .max(crate::fit::best_atom_r2(&tr, &atoms))
            };
            let mut ranked: Vec<(f64, usize)> = children
                .iter()
                .enumerate()
                .filter(|(_, ch)| {
                    matches!(
                        ch.chain.last(),
                        Some(Red::Shell { .. }) | Some(Red::Div { .. })
                    )
                })
                .map(|(i, ch)| (score(&ch.target), i))
                .collect();
            ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
            let mut offset: Vec<(f64, usize)> = children
                .iter()
                .enumerate()
                .filter(|(_, ch)| matches!(ch.chain.last(), Some(Red::ShellAff { .. })))
                .map(|(i, ch)| (score(&ch.target), i))
                .collect();
            offset.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
            for &(_, i) in offset.iter().take(OFFSET_BASIS_CAP) {
                children[i].basis_ok = true;
            }
            for ch in children.iter_mut() {
                if matches!(ch.chain.last(), Some(Red::Shell { .. })) {
                    ch.basis_ok = true;
                    ch.product_ok = true;
                }
            }
            let rest: Vec<usize> = ranked
                .iter()
                .map(|&(_, i)| i)
                .filter(|&i| !children[i].basis_ok)
                .collect();
            for (rank, &i) in rest.iter().take(BASIS_DEPTH1_CAP).enumerate() {
                children[i].basis_ok = true;
                children[i].product_ok = rank < PRODUCT_DEPTH1_CAP;
            }
        }
        for ch in children {
            frontier.push(ch);
        }
    }
    if crate::output::verbose() {
        eprintln!("stage search done: {} ms", t0.elapsed().as_millis());
    }
    for (tier, slot) in pareto.iter().enumerate() {
        let Some((fr2, c)) = slot else { continue };
        let already = finalists
            .iter()
            .any(|e| e.model == c.model && e.desc == c.desc);
        if trace_enabled() {
            trace(
                &format!(
                    "{{\"ev\":\"harvest\",\"stage\":\"final\",\"expr\":\"{}\",\"factor\":\"{}\",\"r2\":{},\"tier\":{},\"kept\":true,\"forwarded\":{}}}",
                    json_esc(& c.expr), json_esc(& c.desc), jnum(* fr2), tier, ! already
                ),
            );
        }
        if !already {
            finalists.push(c.clone());
        }
    }
    if crate::relative::proportional(&target[..train_n]) {
        crate::guard::set_relative(true);
        for (model, r2) in crate::relative::loglin_finalists(&features, n_cols, &target, sample_n) {
            let Ok(parsed) = Parser::parse(&model) else {
                continue;
            };
            finalists.push(Cand {
                model,
                desc: "loglin".to_string(),
                expr: "loglin".to_string(),
                columns: Vec::new(),
                scale: 1.0,
                offset: 0.0,
                fit_r2: r2,
                gate_r2: r2,
                nodes: parsed.node_count(),
                root_raw: false,
            });
        }
    }
    let zero_qf: Vec<Vec<f64>> = q_features.iter().map(|c| vec![0.0; c.len()]).collect();
    let (floor, features_exact, floor_with_features, floor_estimated) = {
        let t: Vec<f64> = if negated {
            target[..train_n].iter().map(|v| -v).collect()
        } else {
            target[..train_n].to_vec()
        };
        let feats_tr: Vec<Vec<f64>> = features.iter().map(|c| c[..train_n].to_vec()).collect();
        let mut scored: Vec<(f64, String)> = finalists
            .iter()
            .map(|c| {
                let m = if negated {
                    format!("(-({}))", c.model)
                } else {
                    c.model.clone()
                };
                (full_model_r2(&m, &feats_tr, &t, &mut evaluator), m)
            })
            .filter(|(r, _)| r.is_finite())
            .collect();
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        let best_fit = scored.first().map_or(0.0, |s| s.0);
        let mut floor_of = |qf: &[Vec<f64>]| -> f64 {
            let f = scored
                .iter()
                .filter(|(r, _)| best_fit >= FLOOR_MIN_FIT && *r >= best_fit - FLOOR_FIT_SLACK)
                .take(FLOOR_FINALISTS)
                .map(|(_, m)| {
                    rounding_floor(m, &features, &t, qf, &q_target, train_n, &mut evaluator)
                })
                .filter(|f| *f > 0.0)
                .fold(f64::INFINITY, f64::min);
            if f.is_finite() && f > NOISE_FLOOR_MIN {
                f
            } else {
                0.0
            }
        };
        let ft = floor_of(&zero_qf);
        let ff = floor_of(&q_features);
        let few_digits = q_features
            .iter()
            .zip(&feature_settings)
            .all(|(q, &set)| q.iter().all(|x| *x == 0.0) || set);
        let estimated = best_fit >= FLOOR_MIN_FIT;
        if ff <= ft || (ff > FEATURE_EXACT_RATIO * ft && few_digits) {
            (ft, true, ft, estimated)
        } else {
            (ff, false, ft, estimated)
        }
    };
    let floor_settings = floor_with_features;
    crate::guard::set_quanta(
        if features_exact {
            &zero_qf
        } else {
            &q_features
        },
        &q_target,
    );
    crate::guard::set_floor(floor);
    if crate::output::verbose() {
        eprintln!("stage floor: {} ms", t0.elapsed().as_millis());
    }
    if negated {
        for v in target.iter_mut() {
            *v = -*v;
        }
        for c in finalists.iter_mut() {
            c.model = format!("(-({}))", c.model);
            c.root_raw = false;
        }
        trace(&"{\"ev\":\"negated\"}".to_string());
    }
    {
        let mut answers: std::collections::HashMap<String, Option<(String, String)>> =
            std::collections::HashMap::new();
        let mut best_for: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut kept: Vec<Cand> = Vec::with_capacity(finalists.len());
        let mut dropped = 0usize;
        arm_deadline_at(
            (timeout_ms > 0).then(|| t0 + std::time::Duration::from_millis(timeout_ms)),
        );
        finalists.sort_by(|a, b| a.nodes.cmp(&b.nodes).then(b.gate_r2.total_cmp(&a.gate_r2)));
        for mut c in finalists.drain(..) {
            if !kept.is_empty() && out_of_budget() {
                dropped += 1;
                continue;
            }
            let tc = Instant::now();
            let ans = answers
                .entry(c.model.clone())
                .or_insert_with(|| {
                    crate::guard::canonical_answer(&c.model, &features, &target, n_cols, train_n)
                })
                .clone();
            if crate::output::verbose() && tc.elapsed().as_millis() > 1000 {
                eprintln!(
                    "canonical form took {} ms: {}",
                    tc.elapsed().as_millis(),
                    c.model
                );
            }
            let Some((st, fitted)) = ans else {
                dropped += 1;
                continue;
            };
            let Ok(p) = Parser::parse(&fitted) else {
                dropped += 1;
                continue;
            };
            if trace_enabled() && fitted != c.model {
                trace(&format!(
                    "{{\"ev\":\"canonical\",\"from\":\"{}\",\"to\":\"{}\"}}",
                    json_esc(&c.model),
                    json_esc(&fitted)
                ));
            }
            c.model = crate::output::tidy(&fitted, 13);
            c.nodes = p.node_count();
            c.root_raw = false;
            match best_for.get(&st) {
                Some(&k) if kept[k].gate_r2 >= c.gate_r2 => {}
                Some(&k) => kept[k] = c,
                None => {
                    best_for.insert(st, kept.len());
                    kept.push(c);
                }
            }
        }
        if trace_enabled() {
            trace(&format!(
                "{{\"ev\":\"canonical_stage\",\"kept\":{},\"dropped\":{}}}",
                kept.len(),
                dropped
            ));
        }
        finalists = kept;
    }
    if crate::output::verbose() {
        eprintln!("stage canonical: {} ms", t0.elapsed().as_millis());
    }
    let mut ho_r2: Vec<Option<f64>> = vec![None; finalists.len()];
    let mut ho_rows: Vec<usize> = vec![0; finalists.len()];
    let mut dl: Vec<Option<f64>> = vec![None; finalists.len()];
    if hold_n > 0 {
        for (ci, c) in finalists.iter().enumerate() {
            let Some((rh, rows)) = holdout_model_r2(
                &c.model,
                &features,
                &target,
                h_lo,
                h_hi,
                &ho_keep,
                ho_min_rows,
                &mut evaluator,
            ) else {
                continue;
            };
            ho_r2[ci] = Some(rh);
            ho_rows[ci] = rows;
            if rh > 0.0 {
                dl[ci] = Some(mdl_dl(rows as f64, rh, c.nodes as f64));
            }
        }
    }
    let _ = hn_f;
    let feats_tr: Vec<Vec<f64>> = features.iter().map(|c| c[..train_n].to_vec()).collect();
    let fixed_r2 = |model: &str, ev: &mut Evaluator| -> Option<f64> {
        (crate::guard::all_constants(model) == Some(0))
            .then(|| full_model_r2(model, &feats_tr, &target[..train_n], ev))
            .filter(|r| r.is_finite())
    };
    let mut loo: Vec<Option<f64>> = vec![None; finalists.len()];
    if hold_n == 0 {
        for (ci, c) in finalists.iter().enumerate() {
            let fixed = fixed_r2(&c.model, &mut evaluator).map(|r| (r, train_n));
            let Some((r2, rows)) = fixed
                .or_else(|| loo_model_r2(&c.model, &features, &target, train_n, &mut evaluator))
            else {
                continue;
            };
            if (rows as f64) < LOO_COVER * train_n as f64 {
                continue;
            }
            loo[ci] = Some(r2);
            ho_r2[ci] = Some(r2);
            ho_rows[ci] = rows;
            if r2 > 0.0 {
                dl[ci] = Some(mdl_dl(rows as f64, r2, c.nodes as f64));
            }
        }
    }
    let exact_tol = (1.0 - EXACT_EXIT_R2).max(NOISE_FLOOR_K * floor);
    let law_shaped = |model: &str| {
        crate::guard::fits_each_row(model, &features, &target, n_rows)
            && crate::guard::within_rounding(model, &features, &target, n_rows)
            && crate::guard::well_conditioned(model, &features, n_rows)
            && !crate::guard::padding_removable(model, &features, &target, n_rows, exact_tol)
    };
    if crate::output::verbose() {
        eprintln!("stage scoring: {} ms", t0.elapsed().as_millis());
    }
    let exact_key: Vec<Option<(usize, usize)>> = finalists
        .iter()
        .enumerate()
        .map(|(ci, c)| match loo[ci] {
            Some(r2)
                if r2 >= EXACT_EXIT_R2.min(1.0 - NOISE_FLOOR_K * floor) && law_shaped(&c.model) =>
            {
                Some((
                    crate::guard::free_constants(&c.model).unwrap_or(usize::MAX),
                    c.nodes,
                ))
            }
            _ => None,
        })
        .collect();
    let abstain = !dl.iter().any(|d| d.is_some()) || finalists.is_empty();
    let train_mean = {
        let (mut s, mut c) = (0.0f64, 0usize);
        for k in 0..train_n {
            let v = target[k];
            if v.is_finite() {
                s += v;
                c += 1;
            }
        }
        if c > 0 {
            s / c as f64
        } else {
            0.0
        }
    };
    let mut order: Vec<usize> = (0..finalists.len()).collect();
    order.sort_by(|&a, &b| match (exact_key[a], exact_key[b]) {
        (Some(x), Some(y)) => x.cmp(&y).then(a.cmp(&b)),
        (Some(_), None) => CmpOrd::Less,
        (None, Some(_)) => CmpOrd::Greater,
        (None, None) => match (dl[a], dl[b]) {
            (Some(x), Some(y)) => x.total_cmp(&y).then(a.cmp(&b)),
            (Some(_), None) => CmpOrd::Less,
            (None, Some(_)) => CmpOrd::Greater,
            (None, None) => finalists[b]
                .gate_r2
                .total_cmp(&finalists[a].gate_r2)
                .then(finalists[a].nodes.cmp(&finalists[b].nodes))
                .then(a.cmp(&b)),
        },
    });
    let mut out_r2: Vec<Option<f64>> = vec![None; finalists.len()];
    let mut out_ho: Vec<Option<f64>> = vec![None; finalists.len()];
    if !abstain {
        for &ci in order.iter().take(top_k) {
            out_r2[ci] = Some(full_model_r2(
                &finalists[ci].model,
                &features,
                &target,
                &mut evaluator,
            ));
            if hold_n > 0 {
                out_ho[ci] = holdout_model_r2(
                    &finalists[ci].model,
                    &features,
                    &target,
                    h_lo,
                    h_hi,
                    &ho_keep,
                    ho_min_rows,
                    &mut evaluator,
                )
                .map(|(rh, _)| rh);
            } else {
                out_ho[ci] = loo[ci];
            }
        }
    }
    if crate::output::verbose() {
        eprintln!("stage out scoring: {} ms", t0.elapsed().as_millis());
    }
    let elapsed = t0.elapsed();
    let chosen_idx: Option<usize> = order.first().copied();
    if let Some(ci) = chosen_idx.filter(|_| crate::output::verbose()) {
        eprintln!(
            "Picked: {} :: {} (emitted R2={:?}, search R2={:.6}, holdout={:?})",
            finalists[ci].desc, finalists[ci].model, out_r2[ci], finalists[ci].fit_r2, out_ho[ci]
        );
    }
    if crate::output::verbose() {
        eprintln!(
            "Total: {:.1}ms, {} evals, {} searches, best gate R2={:.6}, {} finalists",
            elapsed.as_secs_f64() * 1000.0,
            n_evaluated,
            SEARCH_COUNT.load(AtomicOrdering::Relaxed),
            best_gate,
            finalists.len()
        );
    }
    let exact_r2 = EXACT_EXIT_R2.min(1.0 - NOISE_FLOOR_K * floor);
    let dl_margin = (expansions.iter().sum::<usize>().max(1) as f64).ln();
    let verdict_of = |score: Option<f64>, d: Option<f64>| -> &'static str {
        match (score, d) {
            (Some(r), _) if r >= exact_r2 => "exact",
            (_, Some(d)) if d < -dl_margin => "approximate",
            _ => "no law found",
        }
    };
    let mut top_verdict: &'static str = "no law found";
    let mut top_reason: Option<String> = None;
    let mut results: Vec<crate::output::ResultObj> = Vec::new();
    if abstain {
        results.push(crate::output::ResultObj::constant(
            train_mean,
            "no law found",
            Some(1),
            Some(0.0),
        ));
        top_reason = Some(if train_n < min_law_rows() {
            format!(
                    "with {} rows only a law with no fitted constant can be confirmed, and none was found; one with a constant takes at least {} rows",
                    train_n, min_law_rows()
                )
        } else if finalists.is_empty() {
            "the search found no candidate form".to_string()
        } else {
            format!(
                    "no candidate form predicted the rows it was checked on (of {}) better than their mean, or could be evaluated on enough of them",
                    n_rows_in
                )
        });
    } else {
        let identity_cols: Vec<usize> = (0..n_cols).collect();
        let mut seen_struct: std::collections::HashSet<String> = std::collections::HashSet::new();
        struct Entry {
            ci: usize,
            canon: Option<String>,
            d: Option<f64>,
            resid: f64,
            v: &'static str,
            why: Option<String>,
        }
        let rank = |v: &str| match v {
            "exact" => 0,
            "approximate" => 1,
            _ => 2,
        };
        let mut pool: Vec<Entry> = Vec::new();
        for &ci in order.iter() {
            if pool.len() >= VERDICT_POOL * top_k {
                break;
            }
            let c = &finalists[ci];
            let canon = crate::guard::canonical(&c.model, &target);
            if let Some(st) = &canon {
                if !seen_struct.insert(st.clone()) {
                    continue;
                }
            }
            if out_r2[ci].is_none() {
                out_r2[ci] = Some(full_model_r2(&c.model, &features, &target, &mut evaluator));
                if hold_n > 0 {
                    out_ho[ci] = holdout_model_r2(
                        &c.model,
                        &features,
                        &target,
                        h_lo,
                        h_hi,
                        &ho_keep,
                        ho_min_rows,
                        &mut evaluator,
                    )
                    .map(|(rh, _)| rh);
                } else {
                    out_ho[ci] = loo[ci];
                }
            }
            let mut why: Option<String> = None;
            let (score, d) = if hold_n == 0 {
                match fixed_r2(&c.model, &mut evaluator).or_else(|| {
                    crate::guard::loo_refit_r2(&c.model, &features, &target, n_cols, train_n)
                }) {
                    Some(r) => {
                        let s = out_ho[ci].map_or(r, |o| o.min(r));
                        out_ho[ci] = Some(s);
                        (
                            Some(s),
                            (s > 0.0).then(|| mdl_dl(train_n as f64, s, c.nodes as f64)),
                        )
                    }
                    None => {
                        why = Some(match crate::guard::loo_rows_needed(&c.model) {
                            Some(m) if train_n < m => {
                                format!(
                                        "{} rows are too few to check the best fit by leaving one row out (it needs {})",
                                        train_n, m
                                    )
                            }
                            _ => "the best fit could not be checked by leaving one row out"
                                .to_string(),
                        });
                        (None, None)
                    }
                }
            } else {
                (out_ho[ci], dl[ci])
            };
            let v = match verdict_of(score, d) {
                "exact" if !law_shaped(&c.model) => {
                    let fits = crate::guard::fits_each_row(&c.model, &features, &target, n_rows)
                        && crate::guard::within_rounding(&c.model, &features, &target, n_rows);
                    if fits {
                        let fault = crate::guard::conditioning_fault(
                                &c.model,
                                &features,
                                n_rows,
                            )
                            .unwrap_or(
                                "a function in it is nearly constant over the rows, and a constant fits as well",
                            );
                        why = Some(format!(
                            "the best fit is within the precision the cells are written to, but {}",
                            fault
                        ));
                    }
                    if d.is_some_and(|d| d < -dl_margin) {
                        "approximate"
                    } else {
                        "no law found"
                    }
                }
                v => v,
            };
            let free = if v == "exact" {
                crate::guard::free_constants(&c.model)
            } else {
                crate::guard::all_constants(&c.model)
            };
            let v = match free {
                Some(p) if train_n < fit_min_rows(p) => {
                    why = Some(
                        format!(
                            "the best fit has {} constants; confirming it needs at least {} rows, the table has {}",
                            p, fit_min_rows(p), train_n
                        ),
                    );
                    "no law found"
                }
                _ => v,
            };
            let v = match out_r2[ci] {
                Some(r)
                    if v != "no law found"
                        && !(r >= LAW_MIN_R2)
                        && !crate::guard::median_miss(&c.model, &features, &target, n_rows)
                            .is_some_and(|m| m <= crate::config::LAW_MAX_MEDIAN_MISS) =>
                {
                    why = Some(
                        format!(
                            "the best fit (r2 = {:.4}) explains less than {}% of the target's variance, and its typical miss is over {}% of the target's value",
                            r, (100.0 * LAW_MIN_R2).round(), (100.0 * crate
                            ::config::LAW_MAX_MEDIAN_MISS).round()
                        ),
                    );
                    "no law found"
                }
                _ => v,
            };
            let v = match crate::guard::not_clean(&c.model) {
                Some(fault) if v == "approximate" => {
                    why = Some(format!("the best fit is not a clean law: {}", fault));
                    "no law found"
                }
                _ => v,
            };
            let missed =
                crate::guard::badly_missed(&c.model, &features, &target, &q_target, n_rows);
            let v = match missed {
                Some((bad, allowed)) if v != "no law found" && bad > allowed => {
                    let rows = (0..n_rows).filter(|&i| target[i].is_finite()).count();
                    let pct = (100.0 * crate::config::LAW_BAD_MISS).round();
                    why = Some(
                        format!(
                            "the best fit misses {bad} of {rows} rows by more than {pct}% of the target's value"
                        ),
                    );
                    "no law found"
                }
                _ => v,
            };
            pool.push(Entry {
                ci,
                canon,
                d,
                resid: 1.0 - out_r2[ci].unwrap_or(-1.0),
                v,
                why,
            });
        }
        let padded: Vec<bool> = pool
            .iter()
            .map(|e| crate::guard::has_saturated(&finalists[e.ci].model, &features, n_rows))
            .collect();
        let rel: Vec<f64> = pool
            .iter()
            .map(|e| match e.v {
                "approximate" => crate::guard::practical_dl(
                    &finalists[e.ci].model,
                    finalists[e.ci].nodes,
                    &features,
                    &target,
                    train_n,
                )
                .unwrap_or(f64::INFINITY),
                _ => crate::guard::cached_dl(&finalists[e.ci].model, &features, &target, train_n),
            })
            .collect();
        let mut keyed: Vec<(usize, bool, f64, Entry)> = pool
            .into_iter()
            .zip(padded)
            .zip(rel)
            .map(|((e, p), r)| (rank(e.v), p, r, e))
            .collect();
        keyed.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)).then(a.2.total_cmp(&b.2)));
        let mut pool: Vec<Entry> = keyed.into_iter().map(|(_, _, _, e)| e).collect();
        if let Some(top_resid) = pool.first().map(|e| e.resid) {
            for e in pool.iter_mut().skip(1) {
                if e.v == "exact" && e.resid > NOISE_FLOOR_K * top_resid.max(1.0 - EXACT_EXIT_R2) {
                    e.v = if e.d.is_some_and(|d| d < -dl_margin) {
                        "approximate"
                    } else {
                        "no law found"
                    };
                }
            }
        }
        pool.sort_by_key(|e| rank(e.v));
        pool.truncate(top_k);
        for (i, e) in pool.into_iter().enumerate() {
            let c = &finalists[e.ci];
            let (out_expr, out_cols, out_scale, out_offset): (&str, &[usize], f64, f64) =
                if c.root_raw {
                    (&c.expr, &c.columns, c.scale, c.offset)
                } else {
                    (&c.model, &identity_cols, 1.0, 0.0)
                };
            let v = e.v;
            let free = match v {
                "exact" => crate::guard::free_constants(&c.model),
                "no law found" => crate::guard::all_constants(&c.model),
                _ => crate::guard::unnamed_constants(&c.model),
            };
            let why = e
                .why
                .or_else(|| match v {
                    "approximate" => {
                        Some(
                            format!(
                                "the best fit leaves residuals above the precision the cells are written to{}",
                                crate ::guard::largest_miss(& c.model, & features, & target,
                                n_rows).map_or(String::new(), | (rel, y) |
                                format!("; its largest miss is {}% of the target's value (where the target is {})",
                                percent(100.0 * rel), crate ::output::tidy_number(y, 6)))
                            ),
                        )
                    }
                    "no law found" => {
                        Some(
                            format!(
                                "the best fit (r2 = {:.4}) does not beat a constant by enough to be a law",
                                out_r2[e.ci].unwrap_or(- 1.0)
                            ),
                        )
                    }
                    _ => None,
                });
            let why = if v == "no law found" && train_n < min_law_rows() && free.unwrap_or(1) > 0 {
                Some(
                    format!(
                        "with {} rows only a law with no fitted constant can be confirmed; one with a constant takes at least {} rows",
                        train_n, min_law_rows()
                    ),
                )
            } else {
                why
            };
            let is_law = v != "no law found";
            if i == 0 {
                top_verdict = if is_law { "law" } else { "no law found" };
                top_reason = if is_law { None } else { why };
            }
            let miss = if is_law {
                crate::guard::largest_miss(&c.model, &features, &target, n_rows)
            } else {
                None
            };
            let sym = if is_law {
                crate::output::symbolic_model(&c.model, &names)
            } else {
                crate::guard::unnamed_answer(&c.model, &features, &target, n_cols, train_n)
                    .unwrap_or_else(|| c.model.clone())
            };
            results.push(crate::output::ResultObj {
                expr: out_expr.to_string(),
                model: c.model.clone(),
                r2: out_r2[e.ci],
                search_r2: Some(c.fit_r2),
                holdout_r2: out_ho[e.ci],
                scale: out_scale,
                offset: out_offset,
                columns: out_cols.to_vec(),
                uses: crate::output::used_columns(&c.model, &names),
                structure: e.canon,
                canonical: Some(c.model.clone()),
                verdict: if is_law { "law" } else { "no law found" },
                named: crate::output::named_model(&sym, &names),
                symbolic: sym,
                free_constants: free,
                within_precision: is_law.then_some(v == "exact"),
                largest_miss: miss,
            });
        }
    }
    if crate::output::verbose() {
        eprintln!(
            "stage results: {} ms, heap {} MB",
            t0.elapsed().as_millis(),
            crate::heap::live_mb()
        );
    }
    let stopped_by = if early {
        "exact_exit"
    } else if memory_stop || (timed_out && crate::heap::over_soft()) {
        "memory"
    } else if timed_out {
        "timeout"
    } else {
        "frontier_exhausted"
    };
    let mut warnings = ingest_warnings.clone();
    let exact_as_settings = results.first().is_some_and(|r| {
        let fits = r
            .r2
            .is_some_and(|r2| r2 >= 1.0 - NOISE_FLOOR_K * floor_settings.max(1.0 - EXACT_EXIT_R2));
        fits && {
            crate::guard::set_quanta(&zero_qf, &q_target);
            crate::guard::within_rounding(&r.model, &features, &target, n_rows)
        }
    });
    if !floor_estimated && top_verdict == "law" {
        warnings.push(crate::output::FLOOR_NOT_ESTIMATED.to_string());
    }
    if results
        .first()
        .is_some_and(|r| r.within_precision == Some(true))
        && !features_exact
        && !exact_as_settings
    {
        warnings
            .push(
                "the first answer is within the cells' precision with the feature columns read as rounded to their written digits; as exact settings it misses them"
                    .to_string(),
            );
    }
    for w in &warnings[ingest_warnings.len().min(warnings.len())..] {
        eprintln!("keplearn: {}", w);
    }
    let oj = crate::output::Envelope {
        results,
        verdict: top_verdict,
        reason: top_reason,
        stopped_by,
        validation: if hold_n > 0 {
            "holdout"
        } else {
            "leave-one-out"
        },
        noise_floor: floor,
        target: &target_col,
        columns: &names,
        rows: n_rows_in,
        warnings: &warnings,
        time_ms: timing.then(|| elapsed.as_millis()),
    }
    .json();
    let mut rendered = if pretty { json_pretty(&oj) } else { oj };
    rendered.push('\n');
    if trace_enabled() {
        let (bdesc, bexpr, br2) = chosen_idx
            .map(|ci| {
                (
                    finalists[ci].desc.as_str(),
                    finalists[ci].model.as_str(),
                    finalists[ci].fit_r2,
                )
            })
            .unwrap_or(("", "", -1.0));
        trace(
            &format!(
                "{{\"ev\":\"final\",\"factor\":\"{}\",\"expr\":\"{}\",\"r2\":{},\"total_searches\":{}}}",
                json_esc(bdesc), json_esc(bexpr), jnum(br2), SEARCH_COUNT
                .load(AtomicOrdering::Relaxed)
            ),
        );
    }
    if !dump_candidates_path.is_empty() {
        let mut dorder: Vec<usize> = (0..finalists.len()).collect();
        let capped = dorder.len() > DUMP_CANDIDATES_CAP;
        if capped {
            dorder.sort_by(|&a, &b| {
                finalists[b]
                    .fit_r2
                    .total_cmp(&finalists[a].fit_r2)
                    .then(a.cmp(&b))
            });
            dorder.truncate(DUMP_CANDIDATES_CAP);
        }
        if let Some(ci) = chosen_idx {
            if !dorder.contains(&ci) {
                dorder.push(ci);
            }
        }
        struct DumpRow {
            holdout_r2: Option<f64>,
            penalty: Option<f64>,
            final_score: Option<f64>,
        }
        let rows: Vec<DumpRow> = dorder
            .iter()
            .map(|&ci| {
                let c = &finalists[ci];
                let mut row = DumpRow {
                    holdout_r2: ho_r2[ci],
                    penalty: None,
                    final_score: None,
                };
                if let Some(rh) = ho_r2[ci] {
                    if rh > 0.0 && ho_rows[ci] > 0 {
                        let nf = ho_rows[ci] as f64;
                        row.penalty = Some(0.5 * nf.ln() * c.nodes as f64);
                        row.final_score = Some(mdl_dl(nf, rh, c.nodes as f64));
                    }
                }
                row
            })
            .collect();
        let mut ranked: Vec<usize> = (0..rows.len())
            .filter(|&i| rows[i].final_score.is_some())
            .collect();
        ranked.sort_by(|&a, &b| {
            rows[a]
                .final_score
                .unwrap()
                .total_cmp(&rows[b].final_score.unwrap())
                .then(dorder[a].cmp(&dorder[b]))
        });
        let mut final_rank: Vec<Option<usize>> = vec![None; rows.len()];
        for (rank, &i) in ranked.iter().enumerate() {
            final_rank[i] = Some(rank);
        }
        let mut out = String::new();
        out.push_str(&format!(
            "{{\"n_rows\":{},\"n_cols\":{},\"sample_n\":{},\"holdout_n\":{}",
            train_n, n_cols, sample_n, hold_n
        ));
        if capped {
            out.push_str(",\"capped\":true");
        }
        out.push_str(",\"candidates\":[\n");
        for (i, (&ci, row)) in dorder.iter().zip(rows.iter()).enumerate() {
            let c = &finalists[ci];
            if i > 0 {
                out.push_str(",\n");
            }
            out.push_str(
                &format!(
                    "{{\"model\":\"{}\",\"expr\":\"{}\",\"factor\":\"{}\",\"columns\":{:?},\"scale\":{},\"offset\":{},\"node_count\":{},\"fit_r2\":{},\"holdout_r2\":{},\"penalty\":{},\"final_score\":{},\"final_rank\":{},\"chosen\":{}}}",
                    json_esc(& c.model), json_esc(& c.expr), json_esc(& c.desc), c
                    .columns, jnum_rt(c.scale), jnum_rt(c.offset), c.nodes, jnum_rt(c
                    .fit_r2), jopt_f(row.holdout_r2), jopt_f(row.penalty), jopt_f(row
                    .final_score), jopt_u(final_rank[i]), chosen_idx == Some(ci)
                ),
            );
        }
        out.push_str("\n]}\n");
        if let Err(e) = fs::write(&dump_candidates_path, out) {
            eprintln!("failed to write --dump-candidates file: {}", e);
        }
    }
    Ok(rendered)
}

pub(crate) fn percent(p: f64) -> String {
    if p != 0.0 && p.abs() < 0.01 {
        crate::output::tidy_number(p, 2)
    } else {
        format!("{:.2}", p)
    }
}

fn collect_forced(node: &Node, finalists: &mut Vec<Cand>) {
    for (model, cr2) in &node.forced_finalist {
        let Ok(parsed) = Parser::parse(model) else {
            continue;
        };
        finalists.push(Cand {
            model: model.clone(),
            desc: node.desc.clone(),
            expr: "two_mono".to_string(),
            columns: Vec::new(),
            scale: 1.0,
            offset: 0.0,
            fit_r2: *cr2,
            gate_r2: *cr2,
            nodes: parsed.node_count(),
            root_raw: false,
        });
    }
}

fn screen_columns(features: &[Vec<f64>], target: &[f64], k: usize) -> Vec<usize> {
    fn r2(a: &[f64], b: &[f64]) -> f64 {
        let n = a.len() as f64;
        if n < 3.0 {
            return 0.0;
        }
        let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
        let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
        for (x, y) in a.iter().zip(b) {
            sab += (x - ma) * (y - mb);
            saa += (x - ma) * (x - ma);
            sbb += (y - mb) * (y - mb);
        }
        if saa > 0.0 && sbb > 0.0 {
            sab * sab / (saa * sbb)
        } else {
            0.0
        }
    }
    let one_signed = |v: &[f64]| v.iter().all(|x| *x > 0.0) || v.iter().all(|x| *x < 0.0);
    let views = |v: &[f64]| -> Vec<Vec<f64>> {
        let mut out = vec![v.to_vec()];
        if one_signed(v) {
            out.push(v.iter().map(|x| x.abs().ln()).collect());
        }
        if v.iter().all(|x| *x != 0.0) {
            out.push(v.iter().map(|x| 1.0 / x).collect());
        }
        out
    };
    let mut scored: Vec<(f64, usize)> = features
        .iter()
        .enumerate()
        .map(|(j, c)| {
            let rows: Vec<usize> = (0..target.len())
                .filter(|&r| target[r].is_finite() && c[r].is_finite())
                .collect();
            let x: Vec<f64> = rows.iter().map(|&r| c[r]).collect();
            let y: Vec<f64> = rows.iter().map(|&r| target[r]).collect();
            let mut ys = vec![y.clone()];
            if one_signed(&y) {
                ys.push(y.iter().map(|v| v.abs().ln()).collect());
            }
            let best = views(&x)
                .iter()
                .flat_map(|g| ys.iter().map(move |h| r2(g, h)))
                .fold(0.0f64, |a, b| if b.is_finite() { a.max(b) } else { a });
            (best, j)
        })
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut keep: Vec<usize> = scored.into_iter().take(k).map(|(_, j)| j).collect();
    keep.sort_unstable();
    keep
}

fn stdin_file_len() -> Option<u64> {
    #[cfg(unix)]
    let f: fs::File = {
        use std::os::fd::AsFd;
        std::io::stdin().as_fd().try_clone_to_owned().ok()?.into()
    };
    #[cfg(windows)]
    let f: fs::File = {
        use std::os::windows::io::AsHandle;
        std::io::stdin()
            .as_handle()
            .try_clone_to_owned()
            .ok()?
            .into()
    };
    #[cfg(not(any(unix, windows)))]
    return None;
    #[cfg(any(unix, windows))]
    {
        let m = f.metadata().ok()?;
        m.is_file().then(|| m.len())
    }
}
