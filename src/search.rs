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
) -> Vec<ChildProto> {
    let mut out: Vec<ChildProto> = Vec::new();
    let mut rbuf = vec![f64::NAN; sample_n];
    let base_h = harvest_cheap(t, inputs, qn);
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
            if hv >= IO_NEAREXACT_R2 || hv > base_h + SHELL_PROGRESS_MARGIN || node.chain.is_empty()
            {
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
    let chain_has_aff = node.chain.iter().any(|r| matches!(r, Red::ShellAff { .. }));
    if (!has_shell || last_is_inv) && !chain_has_aff {
        out.extend(offset_shells(
            t, inputs, node, qn, sample_n, min_valid, base_h,
        ));
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn offset_shells(
    t: &[f64],
    inputs: &[&[f64]],
    node: &Node,
    qn: usize,
    sample_n: usize,
    min_valid: usize,
    base_h: f64,
) -> Vec<ChildProto> {
    const SHELLS: [&str; 5] = ["log", "sqrt", "inv", "pow-2", "pow-0.5"];
    let mut out = Vec::new();
    let fin: Vec<f64> = t[..sample_n]
        .iter()
        .cloned()
        .filter(|v| v.is_finite())
        .collect();
    if fin.len() < min_valid {
        return out;
    }
    let lo = fin.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = fin.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let span = (hi - lo).abs().max(1e-300);
    let mut cands: Vec<f64> = Vec::new();
    for k in -16..=0 {
        let d = 10f64.powf(k as f64 / 4.0);
        cands.push(lo - d * span);
        cands.push(hi + d * span);
    }
    for v in [lo, hi] {
        for c in [Some(v), snap_value(v)].into_iter().flatten() {
            if c != 0.0 && !cands.contains(&c) {
                cands.push(c);
            }
        }
    }
    let transform = |name: &str, a: f64, b: f64, buf: &mut Vec<f64>| -> usize {
        buf.clear();
        let mut ok = 0;
        for k in 0..sample_n {
            let v = if t[k].is_finite() && (t[k] - b).abs() > 1e-6 * t[k].abs().max(b.abs()) {
                shell_apply(name, (t[k] - b) / a)
            } else {
                f64::NAN
            };
            ok += v.is_finite() as usize;
            buf.push(v);
        }
        ok
    };
    let mut buf: Vec<f64> = Vec::with_capacity(sample_n);
    let need = ((fin.len() as f64 * 0.75).ceil() as usize).max(fit_min_rows(inputs.len() + 2));
    let score = |name: &str, a: f64, b: f64, buf: &mut Vec<f64>| -> f64 {
        if transform(name, a, b, buf) < need {
            return f64::NEG_INFINITY;
        }
        affine_monomial_r2(buf, inputs, qn).max(quick_probe(buf, inputs, qn))
    };
    for name in SHELLS {
        let signs: &[f64] = if name == "inv" { &[1.0] } else { &[1.0, -1.0] };
        for &a in signs {
            let mut best = (f64::NEG_INFINITY, 0.0f64);
            for &b in &cands {
                let s = score(name, a, b, &mut buf);
                if s > best.0 {
                    best = (s, b);
                }
            }
            if !best.0.is_finite() {
                continue;
            }
            let w = (best.1 - if best.1 < lo { lo } else { hi })
                .abs()
                .max(1e-6 * span);
            let (mut l, mut r) = (best.1 - w, best.1 + w);
            let g = 0.618_033_988_75;
            let (mut x1, mut x2) = (r - g * (r - l), l + g * (r - l));
            let (mut f1, mut f2) = (score(name, a, x1, &mut buf), score(name, a, x2, &mut buf));
            for _ in 0..40 {
                if f1 > f2 {
                    r = x2;
                    x2 = x1;
                    f2 = f1;
                    x1 = r - g * (r - l);
                    f1 = score(name, a, x1, &mut buf);
                } else {
                    l = x1;
                    x1 = x2;
                    f1 = f2;
                    x2 = l + g * (r - l);
                    f2 = score(name, a, x2, &mut buf);
                }
            }
            for (f, x) in [(f1, x1), (f2, x2)] {
                if f > best.0 {
                    best = (f, x);
                }
            }
            if let Some(sv) = snap_value(best.1) {
                let s = score(name, a, sv, &mut buf);
                if s >= best.0 - 1e-12 {
                    best = (s.max(best.0), sv);
                }
            }
            if best.0 <= base_h + SHELL_PROGRESS_MARGIN && best.0 < IO_NEAREXACT_R2 {
                continue;
            }
            if best.1 == 0.0 {
                continue;
            }
            if transform(name, a, best.1, &mut buf) < need {
                continue;
            }
            let hv = harvest_la(&buf, inputs, qn, sample_n);
            if !(hv >= IO_NEAREXACT_R2 || hv > base_h + SHELL_PROGRESS_MARGIN) {
                continue;
            }
            let (plane, pr) = if hv >= IO_NEAREXACT_R2 {
                (PLANE_NEAREXACT, hv)
            } else {
                (PLANE_NORMAL, hv - base_h)
            };
            out.push(ChildProto {
                plane,
                priority: pr,
                cost: 3,
                order: 0,
                red: Red::ShellAff {
                    name: name.to_string(),
                    a,
                    b: best.1,
                },
                target: buf.clone(),
                probe_r2: quick_probe(&buf, inputs, qn),
                extra_vcols: Vec::new(),
                desc: format!("io:{}(({}-{})/{})", name, node.desc, best.1, a),
                forced_finalist: Vec::new(),
            });
        }
    }
    out
}

fn basis_finalist(
    t: &[f64],
    features: &[Vec<f64>],
    n_cols: usize,
    sample_n: usize,
    chain: &[Red],
    products: bool,
) -> Vec<(String, f64)> {
    let cols: Vec<Vec<f64>> = features[..n_cols]
        .iter()
        .map(|c| c[..sample_n].to_vec())
        .collect();
    let y: Vec<f64> = t[..sample_n].to_vec();
    if !y.iter().all(|v| v.is_finite()) || !cols.iter().all(|c| c.iter().all(|v| v.is_finite())) {
        return Vec::new();
    }
    let (fi, hi) = crate::config::fit_split(sample_n);
    let xf = crate::forms::fit::gather(&cols, &fi);
    let xh = crate::forms::fit::gather(&cols, &hi);
    let yf: Vec<f64> = fi.iter().map(|&r| y[r]).collect();
    let yh: Vec<f64> = hi.iter().map(|&r| y[r]).collect();
    let cfg = crate::forms::fit::FitConfig::default();
    let r = match crate::forms::reduce::basis_ext(&cols, &y, &xf, &yf, &xh, &yh, &cfg, products) {
        Some(r) => r,
        None => match crate::forms::reduce::rational(&cols, &y, &xf, &yf, &xh, &yh, &cfg) {
            Some(r) => r,
            None => return Vec::new(),
        },
    };
    let inner = format!(
        "({})",
        crate::forms::nf::print_fitted(&r.expr, &r.fit.fitted)
    );
    let model = invert_chain(chain, chain.len(), inner);
    if Parser::parse(&model).is_ok() {
        vec![(model, r.fit.r2_holdout)]
    } else {
        Vec::new()
    }
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
        let cost = Parser::parse(&gexpr)
            .map(|p| p.node_count())
            .unwrap_or(cost)
            .max(1);
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
    let n_gen = n_in;
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
        .any(|r| matches!(r, Red::Shell { .. } | Red::ShellAff { .. }));
    {
        let extra = invert_off_target(t, &input_data, node, qn, sample_n, min_valid, has_shell);
        let n = extra.len();
        for (i, mut p) in extra.into_iter().enumerate() {
            p.order = order + i;
            protos.push(p);
        }
        order += n;
    }
    let has_mat = node.chain.iter().any(|r| matches!(r, Red::Mat { .. }));
    if !has_mat && node.vcols.len() < MAX_VCOLS {
        struct PP {
            key: (usize, usize),
            corr: f64,
            vc: VC,
        }
        let mut pps: Vec<PP> = Vec::new();
        let mut basis_forced: Vec<(String, f64)> = Vec::new();
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
        if node.depth == 0 {
            basis_forced = basis_finalist(t, features, n_cols, sample_n, &node.chain, true);
        } else if node.depth == 1 && node.basis_ok {
            basis_forced =
                basis_finalist(t, features, n_cols, sample_n, &node.chain, node.product_ok);
        }
        pps.sort_by(|a, b| {
            b.corr
                .partial_cmp(&a.corr)
                .unwrap_or(CmpOrd::Equal)
                .then(a.key.cmp(&b.key))
        });
        let room = MAX_VCOLS - node.vcols.len();
        pps.truncate(room);
        let has_forced = !basis_forced.is_empty();
        if !pps.is_empty() || has_forced {
            let best_corr = pps.first().map(|p| p.corr).unwrap_or(0.0);
            let cost = 3usize;
            let q = node.probe_r2.max(best_corr);
            let mut all_vcs: Vec<VC> = pps.into_iter().map(|p| p.vc).collect();
            let mut mono_force = false;
            let forced_fin: Option<(String, f64)> = None;
            if node.depth == 0 {
                let k_terms = mono_terms(n_cols);
                let mono = harvest_monomial_vcols(t, features, n_cols, sample_n, qn, k_terms);
                for m in mono {
                    if all_vcs.iter().any(|v| v.desc == m.desc) {
                        continue;
                    }
                    all_vcs.push(m);
                }
            } else if node.depth == 1
                && matches!(
                    node.chain.last(),
                    Some(Red::Shell { .. }) | Some(Red::Div { .. })
                )
            {
                if has_forced {
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
                    ff.extend(basis_forced);
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
            if matches!(p.red, Red::Shell { .. } | Red::ShellAff { .. }) {
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
                basis_ok: false,
                product_ok: false,
            }
        })
        .collect()
}
