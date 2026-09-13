use crate::*;

pub(crate) static TRACER: OnceLock<Mutex<BufWriter<fs::File>>> = OnceLock::new();

pub(crate) static SEARCH_COUNT: AtomicU64 = AtomicU64::new(0);

pub(crate) static HARVEST_EVS: AtomicU64 = AtomicU64::new(0);

pub(crate) fn trace_enabled() -> bool {
    TRACER.get().is_some()
}

pub(crate) fn trace(line: &str) {
    if let Some(m) = TRACER.get() {
        if let Ok(mut w) = m.lock() {
            let _ = writeln!(w, "{}", line);
            let _ = w.flush();
        }
    }
}

pub(crate) struct ChildProto {
    plane: u8,
    priority: f64,
    cost: usize,
    order: usize,
    red: Red,
    target: Vec<f64>,
    probe_r2: f64,
    extra_vcols: Vec<VC>,
    desc: String,
    forced_finalist: Vec<(String, f64)>,
}

pub(crate) fn fmt_pow(k: f64) -> String {
    if k.fract() == 0.0 {
        format!("{}", k as i64)
    } else {
        format!("{}", k)
    }
}

#[allow(clippy::too_many_arguments)]
fn io_shell_proto(
    name: String,
    cost: usize,
    tvec: &[f64],
    inputs: &[&[f64]],
    qn: usize,
    node_desc: &str,
    plane: u8,
    pr: f64,
    order: usize,
) -> ChildProto {
    ChildProto {
        plane,
        priority: pr,
        cost,
        order,
        red: Red::Shell { name: name.clone() },
        target: tvec.to_vec(),
        probe_r2: quick_probe(tvec, inputs, qn),
        extra_vcols: Vec::new(),
        desc: format!("io:{}({})", name, node_desc),
        forced_finalist: Vec::new(),
    }
}

#[allow(clippy::too_many_arguments)]
fn invert_off_target(
    t: &[f64],
    inputs: &[&[f64]],
    node: &Node,
    qn: usize,
    sample_n: usize,
    min_valid: usize,
    has_shell: bool,
    last_is_affc: bool,
    chain_has_log: bool,
) -> Vec<ChildProto> {
    let mut out: Vec<ChildProto> = Vec::new();
    let mut rbuf = vec![f64::NAN; sample_n];
    let base_h = harvest_la(t, inputs, qn, sample_n);
    if !has_shell {
        for (name, cost) in shells() {
            for k in 0..sample_n {
                rbuf[k] = if t[k].is_finite() {
                    shell_apply(name, t[k])
                } else {
                    f64::NAN
                };
            }
            let vc = rbuf.iter().filter(|x| x.is_finite()).count();
            if vc < min_valid {
                continue;
            }
            let hv = harvest_la(&rbuf, inputs, qn, sample_n);
            if hv >= IO_NEAREXACT_R2 || hv > base_h + SHELL_PROGRESS_MARGIN {
                let plane = if hv >= IO_NEAREXACT_R2 {
                    PLANE_NEAREXACT
                } else {
                    PLANE_NORMAL
                };
                let pr = if hv >= IO_NEAREXACT_R2 {
                    hv
                } else {
                    hv - base_h
                };
                out.push(io_shell_proto(
                    name.to_string(),
                    cost,
                    &rbuf,
                    inputs,
                    qn,
                    &node.desc,
                    plane,
                    pr,
                    0,
                ));
            }
        }
    }
    let last_is_inv = matches!(
        node.chain.last(), Some(Red::Shell { name, .. }) if name == "inv"
    );
    if (!has_shell || last_is_inv) && !chain_has_log {
        let (c, lmr) = fit_shell_const(t, inputs, qn, sample_n, |y, cc| {
            let z = 1.0 + cc * y;
            if z > 1e-30 {
                z.ln()
            } else {
                f64::NAN
            }
        });
        if lmr >= AFFC_LOOKAHEAD_R2 {
            for k in 0..sample_n {
                rbuf[k] = if t[k].is_finite() {
                    1.0 + c * t[k]
                } else {
                    f64::NAN
                };
            }
            let vc = rbuf.iter().filter(|x| x.is_finite()).count();
            if vc >= min_valid {
                out.push(ChildProto {
                    plane: PLANE_NEAREXACT,
                    priority: lmr,
                    cost: 2,
                    order: 0,
                    red: Red::Affc { c },
                    target: rbuf.clone(),
                    probe_r2: quick_probe(&rbuf, inputs, qn),
                    extra_vcols: Vec::new(),
                    desc: format!("io:affc({})", node.desc),
                    forced_finalist: Vec::new(),
                });
            }
        }
    }
    if last_is_affc {
        for k in 0..sample_n {
            rbuf[k] = if t[k].is_finite() && t[k] > 1e-30 {
                t[k].ln()
            } else {
                f64::NAN
            };
        }
        let vc = rbuf.iter().filter(|x| x.is_finite()).count();
        if vc >= min_valid {
            let pr = harvest_la(&rbuf, inputs, qn, sample_n);
            out.push(io_shell_proto(
                "log".to_string(),
                2,
                &rbuf,
                inputs,
                qn,
                &node.desc,
                PLANE_NEAREXACT,
                pr,
                0,
            ));
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn expand_children(
    node: &Node,
    features: &[Vec<f64>],
    n_cols: usize,
    sample_n: usize,
    qn: usize,
    seq: &mut u64,
) -> Vec<Node> {
    let t: &[f64] = &node.target;
    let mut input_data: Vec<&[f64]> = features.iter().map(|c| &c[..sample_n]).collect();
    let mut input_g: Vec<String> = (0..n_cols).map(|i| format!("x{}", i + 1)).collect();
    let mut input_d: Vec<String> = (0..n_cols).map(|i| format!("c{}", i)).collect();
    for v in &node.vcols {
        input_data.push(&v.data[..sample_n]);
        input_g.push(v.gexpr.clone());
        input_d.push(v.desc.clone());
    }
    let n_in = input_data.len();
    let min_div_rows = (min_finite_rows(sample_n)).min(DIV_MIN_VALID_ROWS_CAP);
    let min_valid = min_finite_rows(sample_n);
    let probe_basis = build_probe_basis(&input_data, qn);
    let mut fbuf = vec![0.0f64; sample_n];
    let mut rbuf = vec![0.0f64; sample_n];
    let mut scratch: Vec<f64> = Vec::with_capacity(sample_n);
    let mut protos: Vec<ChildProto> = Vec::new();
    let mut order = 0usize;
    let prio = |q: f64, cost: usize| -> f64 {
        let pr = (q - node.probe_r2).max(0.0) / cost as f64;
        if pr.is_finite() {
            pr
        } else {
            0.0
        }
    };
    let div_used = |gexpr: &str| -> bool {
        node.chain
            .iter()
            .any(|r| matches!(r, Red::Div { gexpr : g, .. } if g == gexpr))
    };
    let base_harv = harvest_cheap(t, &input_data, qn);
    let progress_eps = DIVIDE_PROGRESS_EPS;
    let push_div = |protos: &mut Vec<ChildProto>,
                    order: &mut usize,
                    rbuf: &[f64],
                    fdesc: String,
                    gexpr: String,
                    cost: usize,
                    input_data: &[&[f64]],
                    probe_basis: &[Vec<f64>],
                    additive: bool| {
        let q = quick_probe(rbuf, input_data, qn);
        let mut plane = PLANE_NORMAL;
        let mut pr = prio(q, cost);
        let qe = quick_probe_ext(rbuf, input_data, probe_basis, qn);
        if qe >= TERMINAL_PROBE_R2 {
            plane = PLANE_NEAREXACT;
            pr = qe;
        } else if additive {
            let mr = monomial_loglin_r2(rbuf, input_data, qn);
            if mr >= ADDITIVE_PROMO_R2 {
                plane = PLANE_NEAREXACT;
                pr = mr;
            }
        }
        if plane != PLANE_NEAREXACT {
            let dh = harvest_cheap(rbuf, input_data, qn);
            if dh > base_harv + progress_eps {
                pr = pr.max(dh);
            }
        }
        protos.push(ChildProto {
            plane,
            priority: pr,
            cost,
            order: *order,
            red: Red::Div {
                desc: fdesc.clone(),
                gexpr,
                cost,
            },
            target: rbuf.to_vec(),
            probe_r2: q,
            extra_vcols: Vec::new(),
            desc: format!("{}/{}", node.desc, fdesc),
            forced_finalist: Vec::new(),
        });
        *order += 1;
    };
    let last_is_affc = matches!(node.chain.last(), Some(Red::Affc { .. }));
    let n_gen = if last_is_affc { 0 } else { n_in };
    for ii in 0..n_gen {
        for &(name, f, cost) in &unary_factor_ops() {
            let gexpr = unary_gexpr(name, &input_g[ii]);
            if div_used(&gexpr) {
                continue;
            }
            for k in 0..sample_n {
                fbuf[k] = f(input_data[ii][k]);
            }
            let vc = masked_divide(t, &fbuf, &mut rbuf, &mut scratch, sample_n);
            if vc < min_div_rows {
                continue;
            }
            push_div(
                &mut protos,
                &mut order,
                &rbuf,
                format!("{}({})", name, input_d[ii]),
                gexpr,
                cost,
                &input_data,
                &probe_basis,
                false,
            );
        }
    }
    'pair_sweep: for ii in 0..n_gen {
        for jj in (ii + 1)..n_in {
            if out_of_time() {
                break 'pair_sweep;
            }
            for &(name, f, cost, swap) in &pair_factor_ops() {
                for &sw in &[false, true] {
                    if sw && !swap {
                        continue;
                    }
                    let (a, b) = if sw { (jj, ii) } else { (ii, jj) };
                    let gexpr = pair_gexpr(name, &input_g[a], &input_g[b]);
                    if div_used(&gexpr) {
                        continue;
                    }
                    for k in 0..sample_n {
                        fbuf[k] = f(input_data[a][k], input_data[b][k]);
                    }
                    let vc = masked_divide(t, &fbuf, &mut rbuf, &mut scratch, sample_n);
                    if vc < min_div_rows {
                        continue;
                    }
                    let additive = matches!(name, "add" | "sub");
                    push_div(
                        &mut protos,
                        &mut order,
                        &rbuf,
                        format!("{}({},{})", name, input_d[a], input_d[b]),
                        gexpr,
                        cost,
                        &input_data,
                        &probe_basis,
                        additive,
                    );
                }
            }
        }
    }
    'pow_sweep: for ii in 0..n_gen {
        for jj in (ii + 1)..n_in {
            if out_of_time() {
                break 'pow_sweep;
            }
            for &(bname, add_flag) in &[("sub", true), ("div", false)] {
                for &sw in &[false, true] {
                    let (a, b) = if sw { (jj, ii) } else { (ii, jj) };
                    let base_gx = pair_gexpr(bname, &input_g[a], &input_g[b]);
                    for &kp in POW_K_SET {
                        let gexpr = format!("(({})**{})", base_gx, fmt_pow(kp));
                        if div_used(&gexpr) {
                            continue;
                        }
                        for k in 0..sample_n {
                            let bv = if bname == "sub" {
                                input_data[a][k] - input_data[b][k]
                            } else if input_data[b][k].abs() > 1e-10 {
                                input_data[a][k] / input_data[b][k]
                            } else {
                                f64::NAN
                            };
                            fbuf[k] = if bv.is_finite() {
                                bv.powf(kp)
                            } else {
                                f64::NAN
                            };
                        }
                        let vc = masked_divide(t, &fbuf, &mut rbuf, &mut scratch, sample_n);
                        if vc < min_div_rows {
                            continue;
                        }
                        push_div(
                            &mut protos,
                            &mut order,
                            &rbuf,
                            format!("{}({},{})^{}", bname, input_d[a], input_d[b], fmt_pow(kp)),
                            gexpr,
                            5,
                            &input_data,
                            &probe_basis,
                            add_flag,
                        );
                    }
                }
            }
        }
    }
    let has_shell = node
        .chain
        .iter()
        .any(|r| matches!(r, Red::Shell { .. } | Red::Affc { .. }));
    let chain_has_log = node
        .chain
        .iter()
        .any(|r| matches!(r, Red::Shell { name, .. } if name == "log"));
    {
        let extra = invert_off_target(
            t,
            &input_data,
            node,
            qn,
            sample_n,
            min_valid,
            has_shell,
            last_is_affc,
            chain_has_log,
        );
        let n = extra.len();
        for (i, mut p) in extra.into_iter().enumerate() {
            p.order = order + i;
            protos.push(p);
        }
        order += n;
    }
    let has_mat = node.chain.iter().any(|r| matches!(r, Red::Mat { .. }));
    if !last_is_affc && !has_mat && node.vcols.len() < MAX_VCOLS {
        struct PP {
            key: (usize, usize),
            corr: f64,
            vc: VC,
        }
        let mut pps: Vec<PP> = Vec::new();
        let mut norm_forced: Vec<(String, f64)> = Vec::new();
        let mut trig_forced: Vec<(String, f64)> = Vec::new();
        let mut varpow_forced: Vec<(String, f64)> = Vec::new();
        let mut rational_forced: Vec<(String, f64)> = Vec::new();
        let mut additive_forced: Vec<(String, f64)> = Vec::new();
        for ci in 0..n_cols {
            for cj in (ci + 1)..n_cols {
                let mut best: Option<(f64, VC)> = None;
                for &(name, f, _cost, swap) in &mat_pair_ops() {
                    for &sw in &[false, true] {
                        if sw && !swap {
                            continue;
                        }
                        let (a, b) = if sw { (cj, ci) } else { (ci, cj) };
                        for k in 0..sample_n {
                            fbuf[k] = f(features[a][k], features[b][k]);
                        }
                        let valid = fbuf.iter().filter(|x| x.is_finite()).count();
                        if valid < min_valid {
                            continue;
                        }
                        let (r, _, _) = r2_score(&t[..sample_n], &fbuf);
                        if r < PAIR_VCOL_MIN_R2 {
                            continue;
                        }
                        if best.as_ref().map(|(br, _)| r > *br).unwrap_or(true) {
                            best = Some((
                                r,
                                VC {
                                    gexpr: pair_gexpr(
                                        name,
                                        &format!("x{}", a + 1),
                                        &format!("x{}", b + 1),
                                    ),
                                    desc: format!("{}(c{},c{})", name, a, b),
                                    data: Rc::new(fbuf.clone()),
                                },
                            ));
                        }
                    }
                }
                if let Some((r, vc)) = best {
                    pps.push(PP {
                        key: (ci, cj),
                        corr: r,
                        vc,
                    });
                }
            }
        }
        let mut plan: Vec<(f64, &'static str)> = Vec::new();
        for cap in VCOL_CAPS {
            if n_cols < cap.min_cols {
                continue;
            }
            let benefit = match cap.name {
                "norm" => {
                    if residual_near_miss(t, &input_data, qn)
                        || structurally_invisible(t, &input_data, sample_n, qn)
                    {
                        1.0
                    } else {
                        0.0
                    }
                }
                "trig" => {
                    if residual_near_miss(t, &input_data, qn)
                        || structurally_invisible(t, &input_data, sample_n, qn)
                    {
                        1.0
                    } else {
                        0.0
                    }
                }
                "varpow" => {
                    if structurally_invisible(t, &input_data, sample_n, qn) {
                        1.0
                    } else {
                        0.0
                    }
                }
                "rational" => {
                    if residual_near_miss(t, &input_data, qn)
                        || structurally_invisible(t, &input_data, sample_n, qn)
                    {
                        1.0
                    } else {
                        0.0
                    }
                }
                _ => 0.0,
            };
            if benefit <= 0.0 {
                continue;
            }
            let cost = (n_cols as f64).powi(cap.cost_exp);
            plan.push((benefit / cost, cap.name));
        }
        plan.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        for (_, name) in plan {
            match name {
                "norm" => {
                    norm_forced = gen_norm_finalists(
                        t,
                        features,
                        n_cols,
                        sample_n,
                        qn,
                        min_valid,
                        &node.chain,
                    );
                }
                "trig" => {
                    trig_forced = gen_trig_finalists(
                        t,
                        features,
                        n_cols,
                        sample_n,
                        qn,
                        min_valid,
                        &node.chain,
                    );
                }
                "varpow" => {
                    varpow_forced = gen_varpow_finalists(
                        t,
                        features,
                        n_cols,
                        sample_n,
                        qn,
                        min_valid,
                        &node.chain,
                    );
                }
                "rational" => {
                    rational_forced = gen_rational_finalists(
                        t,
                        features,
                        n_cols,
                        sample_n,
                        qn,
                        min_valid,
                        &node.chain,
                    );
                }
                _ => {}
            }
        }
        if node.depth == 0 && n_cols <= ADDITIVE_MAX_VARS {
            additive_forced =
                gen_additive_finalists(t, features, n_cols, sample_n, qn, min_valid, &node.chain);
        }
        pps.sort_by(|a, b| {
            b.corr
                .partial_cmp(&a.corr)
                .unwrap_or(CmpOrd::Equal)
                .then(a.key.cmp(&b.key))
        });
        let room = MAX_VCOLS - node.vcols.len();
        pps.truncate(room);
        let has_forced = !norm_forced.is_empty()
            || !trig_forced.is_empty()
            || !varpow_forced.is_empty()
            || !rational_forced.is_empty()
            || !additive_forced.is_empty();
        if !pps.is_empty() || has_forced {
            let best_corr = pps.first().map(|p| p.corr).unwrap_or(0.0);
            let cost = 3usize;
            let q = node.probe_r2.max(best_corr);
            let mut all_vcs: Vec<VC> = pps.into_iter().map(|p| p.vc).collect();
            let mut mono_force = false;
            let mut forced_fin: Option<(String, f64)> = None;
            if node.depth == 0 {
                let k_terms = mono_terms(n_cols);
                let mono = harvest_monomial_vcols(t, features, n_cols, sample_n, qn, k_terms);
                for m in mono {
                    if all_vcs.iter().any(|v| v.desc == m.desc) {
                        continue;
                    }
                    all_vcs.push(m);
                }
                let (kmono, cr2, coeffs) = harvest_k_monomial(
                    t,
                    features,
                    n_cols,
                    sample_n,
                    qn,
                    k_terms,
                    KMONO_KCOL_MIN_DEG,
                );
                if cr2 >= KMONO_FINALIST_R2 && kmono.len() >= 2 {
                    if let Some(sol) = coeffs {
                        if sol.len() == kmono.len() + 1 && sol.iter().all(|c| c.is_finite()) {
                            let mut terms: Vec<String> = Vec::with_capacity(kmono.len() + 1);
                            for (i, m) in kmono.iter().enumerate() {
                                terms.push(format!("{}*({})", fmt_const(sol[i]), m.gexpr));
                            }
                            terms.push(fmt_const(sol[kmono.len()]));
                            let inner = format!("({})", terms.join("+"));
                            let model = invert_chain(&node.chain, node.chain.len(), inner);
                            if Parser::parse(&model).is_ok() {
                                forced_fin = Some((model, cr2));
                            }
                        }
                    }
                }
            } else if node.depth == 1
                && matches!(
                    node.chain.last(),
                    Some(Red::Shell { .. }) | Some(Red::Div { .. })
                )
            {
                let (mono, cr2, coeffs) =
                    harvest_k_monomial(t, features, n_cols, sample_n, qn, 2, 2);
                if cr2 >= KMONO_FINALIST_R2 {
                    if let Some(sol) = coeffs {
                        if mono.len() == 2 && sol.len() == 3 && sol.iter().all(|c| c.is_finite()) {
                            let inner = format!(
                                "({}*({})+{}*({})+{})",
                                fmt_const(sol[0]),
                                mono[0].gexpr,
                                fmt_const(sol[1]),
                                mono[1].gexpr,
                                fmt_const(sol[2])
                            );
                            let model = invert_chain(&node.chain, node.chain.len(), inner);
                            if Parser::parse(&model).is_ok() {
                                forced_fin = Some((model, cr2));
                            }
                        }
                    }
                    for m in mono {
                        if all_vcs.iter().any(|v| v.desc == m.desc) {
                            continue;
                        }
                        all_vcs.push(m);
                    }
                    mono_force = true;
                }
            }
            let (plane, pr) = if node.depth == 0 {
                (PLANE_MAT_ROOT, 0.0)
            } else if mono_force {
                (PLANE_MAT_MONO, 0.0)
            } else {
                (PLANE_NORMAL, prio(q, cost))
            };
            let descs: Vec<String> = all_vcs.iter().map(|v| v.desc.clone()).collect();
            protos.push(ChildProto {
                plane,
                priority: pr,
                cost,
                order,
                red: Red::Mat {
                    desc: descs.join("+"),
                },
                target: t.to_vec(),
                probe_r2: q,
                extra_vcols: all_vcs,
                desc: format!("{}+v", node.desc),
                forced_finalist: {
                    let mut ff: Vec<(String, f64)> = [forced_fin].into_iter().flatten().collect();
                    ff.extend(norm_forced);
                    ff.extend(trig_forced);
                    ff.extend(varpow_forced);
                    ff.extend(rational_forced);
                    ff.extend(additive_forced);
                    ff
                },
            });
        }
    }
    protos.sort_by(|a, b| {
        b.plane
            .cmp(&a.plane)
            .then(b.priority.partial_cmp(&a.priority).unwrap_or(CmpOrd::Equal))
            .then(a.cost.cmp(&b.cost))
            .then(a.order.cmp(&b.order))
    });
    {
        let beam = beam_at(node.depth, n_cols);
        let mut kept: Vec<ChildProto> = Vec::with_capacity(protos.len().min(beam + 16));
        let mut general = 0usize;
        for p in protos.drain(..) {
            if matches!(p.red, Red::Shell { .. } | Red::Affc { .. }) {
                kept.push(p);
            } else if general < beam {
                general += 1;
                kept.push(p);
            }
        }
        protos = kept;
    }
    protos
        .into_iter()
        .map(|p| {
            *seq += 1;
            let mut chain = node.chain.clone();
            chain.push(p.red);
            let mut vcols = node.vcols.clone();
            vcols.extend(p.extra_vcols);
            Node {
                target: Rc::new(p.target),
                chain,
                vcols,
                depth: node.depth + 1,
                complexity: node.complexity + p.cost,
                probe_r2: node.probe_r2.max(p.probe_r2),
                plane: p.plane,
                priority: p.priority,
                seq: *seq,
                desc: p.desc,
                forced_finalist: p.forced_finalist,
            }
        })
        .collect()
}
