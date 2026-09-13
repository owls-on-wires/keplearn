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
}

pub(crate) fn ingest(input: &str, target_col: &str) -> Result<Ingest, String> {
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
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
            "no tab or comma delimiter in the header row; expected TSV or CSV input".into(),
        );
    };
    let header: Vec<String> = split_row(head_line, delim);
    let mut warnings: Vec<String> = Vec::new();
    for (i, h) in header.iter().enumerate() {
        let t = h.trim();
        if !t.is_empty()
            && header[..i].iter().any(|p| p.trim() == t)
            && !warnings.iter().any(|w| w.contains(&format!("'{}'", t)))
        {
            warnings.push(format!(
                "duplicate column name '{}' in header; the first occurrence is used",
                t
            ));
        }
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
                    .map(|h| h.trim())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;
    let feat_idx: Vec<usize> = (0..header.len()).filter(|&i| i != target_idx).collect();
    let n_cols = feat_idx.len();
    if n_cols == 0 {
        return Err("input has a target column but no feature columns".into());
    }
    let mut features: Vec<Vec<f64>> = vec![Vec::new(); n_cols];
    let mut target: Vec<f64> = Vec::new();
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
        }
        target.push(tv);
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
    Ok(Ingest {
        features,
        target,
        n_cols,
        warnings,
    })
}

pub fn run(cfg: Config) {
    let Config {
        top_k,
        sample,
        target_col,
        quick_n,
        timeout_ms,
        max_depth,
        trace_path,
        dump_candidates_path,
        ..
    } = cfg;
    let eff_max_depth = max_depth.unwrap_or(MAX_DEPTH).min(MAX_DEPTH);
    if !trace_path.is_empty() {
        let f = match fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&trace_path)
        {
            Ok(f) => f,
            Err(e) => {
                eprintln!("keplearn: cannot open trace file '{}': {}", trace_path, e);
                std::process::exit(1);
            }
        };
        let _ = TRACER.set(Mutex::new(BufWriter::new(f)));
    }
    let global_max_stack = 2usize;
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
    let input = String::from_utf8_lossy(&raw);
    let ing = match ingest(&input, &target_col) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("keplearn: {}", e);
            std::process::exit(1);
        }
    };
    for w in &ing.warnings {
        eprintln!("keplearn: {}", w);
    }
    let Ingest {
        mut features,
        mut target,
        n_cols,
        ..
    } = ing;
    let n_rows = target.len();
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
    }
    let sample_n = sample.min(train_n.max(1));
    let hold_n = if holdout_n_full >= HOLDOUT_MIN_ACTIVE {
        holdout_n_full.min(HOLDOUT_MAX)
    } else {
        0
    };
    eprintln!(
        "Data: {} rows x {} cols, sample={}, holdout={}",
        n_rows, n_cols, sample_n, hold_n
    );
    trace(&format!(
        "{{\"ev\":\"start\",\"n_cols\":{},\"n_rows\":{}}}",
        n_cols, n_rows
    ));
    let mut evaluator = Evaluator::new(global_max_stack, sample_n.max(hold_n));
    let t0 = Instant::now();
    arm_deadline(timeout_ms);
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
            let cut_idx = m - k;
            mags.select_nth_unstable_by(cut_idx, |a, b| a.partial_cmp(b).unwrap_or(CmpOrd::Equal));
            let cutoff = mags[cut_idx];
            for (j, i) in (h_lo..h_hi).enumerate() {
                if keep[j] && target[i].abs() >= cutoff {
                    keep[j] = false;
                }
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
    let mut timed_out = false;
    let mut turn_ctr: u64 = 0;
    let mut any_expanded = false;
    let mut lane;
    'main: loop {
        if timeout_ms > 0 && any_expanded && t0.elapsed().as_millis() as u64 >= timeout_ms {
            timed_out = true;
            break;
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
                        Some(c.gate_r2)
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
        let children = expand_children(&node, &features, n_cols, sample_n, qn, &mut seq);
        for ch in children {
            frontier.push(ch);
        }
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
    let abstain = hold_n > 0 && !dl.iter().any(|d| d.is_some());
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
    order.sort_by(|&a, &b| match (dl[a], dl[b]) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(CmpOrd::Equal).then(a.cmp(&b)),
        (Some(_), None) => CmpOrd::Less,
        (None, Some(_)) => CmpOrd::Greater,
        (None, None) => finalists[b]
            .gate_r2
            .partial_cmp(&finalists[a].gate_r2)
            .unwrap_or(CmpOrd::Equal)
            .then(finalists[a].nodes.cmp(&finalists[b].nodes))
            .then(a.cmp(&b)),
    });
    if hold_n > 0 && !abstain {
        let emit_idxs: Vec<usize> = order.iter().take(top_k).copied().collect();
        for ci in emit_idxs {
            if finalists[ci].root_raw {
                continue;
            }
            let Some(rh) = ho_r2[ci] else { continue };
            if let Some(m) = snap_model_constants(
                &finalists[ci].model,
                rh,
                &features,
                &target,
                h_lo,
                h_hi,
                &ho_keep,
                ho_min_rows,
                &mut evaluator,
            ) {
                if trace_enabled() {
                    trace(&format!(
                        "{{\"ev\":\"snap\",\"from\":\"{}\",\"to\":\"{}\"}}",
                        json_esc(&finalists[ci].model),
                        json_esc(&m)
                    ));
                }
                finalists[ci].model = m;
            }
        }
    }
    if !abstain {
        for &ci in order.iter().take(top_k) {
            if let Some(m) = simplify_model(&finalists[ci].model) {
                if trace_enabled() {
                    trace(&format!(
                        "{{\"ev\":\"simplify\",\"from\":\"{}\",\"to\":\"{}\"}}",
                        json_esc(&finalists[ci].model),
                        json_esc(&m)
                    ));
                }
                if let Ok(p) = Parser::parse(&m) {
                    finalists[ci].nodes = p.node_count();
                }
                finalists[ci].model = m;
            }
        }
    }
    if !abstain {
        for &ci in order.iter().take(top_k) {
            let Some((a, b)) =
                full_model_affine_fit(&finalists[ci].model, &features, &target, &mut evaluator)
            else {
                continue;
            };
            if affine_trivial(a, b) {
                continue;
            }
            let m2 = format!(
                "({}*({})+{})",
                fmt_const(a),
                finalists[ci].model,
                fmt_const(b)
            );
            let base = full_model_r2(&finalists[ci].model, &features, &target, &mut evaluator);
            let recal = full_model_r2(&m2, &features, &target, &mut evaluator);
            if recal > base + RECAL_MIN_GAIN {
                if trace_enabled() {
                    trace(
                        &format!(
                            "{{\"ev\":\"recal\",\"from\":\"{}\",\"to\":\"{}\",\"r2_from\":{},\"r2_to\":{}}}",
                            json_esc(& finalists[ci].model), json_esc(& m2),
                            jnum_rt(base), jnum_rt(recal)
                        ),
                    );
                }
                if let Ok(p) = Parser::parse(&m2) {
                    finalists[ci].nodes = p.node_count();
                }
                finalists[ci].model = m2;
            }
        }
    }
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
            }
        }
    }
    let elapsed = t0.elapsed();
    let chosen_idx: Option<usize> = order.first().copied();
    if let Some(ci) = chosen_idx {
        eprintln!(
            "Picked: {} :: {} (emitted R2={:?}, search R2={:.6}, holdout={:?})",
            finalists[ci].desc, finalists[ci].model, out_r2[ci], finalists[ci].fit_r2, out_ho[ci]
        );
    }
    eprintln!(
        "Total: {:.1}ms, {} evals, {} searches, best gate R2={:.6}, {} finalists",
        elapsed.as_secs_f64() * 1000.0,
        n_evaluated,
        SEARCH_COUNT.load(AtomicOrdering::Relaxed),
        best_gate,
        finalists.len()
    );
    print!("{{\"results\":[");
    if abstain {
        let mc = fmt_const(train_mean);
        print!(
            "{{\"expr\":\"{}\",\"r2\":0.0000000000,\"search_r2\":0.0000000000,\"holdout_r2\":null,\"scale\":1,\"offset\":0,\"columns\":[],\"factor\":\"direct\",\"model\":\"{}\"}}",
            mc, mc
        );
    } else {
        let identity_cols: Vec<usize> = (0..n_cols).collect();
        for (i, &ci) in order.iter().take(top_k).enumerate() {
            let c = &finalists[ci];
            if i > 0 {
                print!(",");
            }
            let (out_expr, out_cols, out_scale, out_offset): (&str, &[usize], f64, f64) =
                if c.root_raw {
                    (&c.expr, &c.columns, c.scale, c.offset)
                } else {
                    (&c.model, &identity_cols, 1.0, 0.0)
                };
            print!(
                "{{\"expr\":\"{}\",\"r2\":{:.10},\"search_r2\":{:.10},\"holdout_r2\":{},\"scale\":{},\"offset\":{},\"columns\":{:?},\"factor\":\"direct\",\"model\":\"{}\"}}",
                json_esc(out_expr), out_r2[ci].unwrap_or(- 1.0), c.fit_r2,
                jopt_f(out_ho[ci]), out_scale, out_offset, out_cols, json_esc(& c.model)
            );
        }
    }
    let stopped_by = if early {
        "exact_exit"
    } else if timed_out {
        "timeout"
    } else {
        "frontier_exhausted"
    };
    println!(
        "],\"time_ms\":{},\"n_expressions\":0,\"n_evaluated\":{},\"stopped_by\":\"{}\"}}",
        elapsed.as_millis(),
        n_evaluated,
        stopped_by
    );
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
                    .partial_cmp(&finalists[a].fit_r2)
                    .unwrap_or(CmpOrd::Equal)
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
                .partial_cmp(&rows[b].final_score.unwrap())
                .unwrap_or(CmpOrd::Equal)
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
}
