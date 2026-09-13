use crate::*;

pub(crate) fn disjoint_pair_partitions(
    n_cols: usize,
    p: usize,
    cap: usize,
) -> Vec<Vec<(usize, usize)>> {
    fn rec(
        n: usize,
        p: usize,
        min_first: usize,
        used: &mut Vec<bool>,
        cur: &mut Vec<(usize, usize)>,
        out: &mut Vec<Vec<(usize, usize)>>,
        cap: usize,
    ) {
        if out.len() >= cap {
            return;
        }
        if cur.len() == p {
            out.push(cur.clone());
            return;
        }
        for i in min_first..n {
            if used[i] {
                continue;
            }
            for j in (i + 1)..n {
                if used[j] {
                    continue;
                }
                used[i] = true;
                used[j] = true;
                cur.push((i, j));
                rec(n, p, i + 1, used, cur, out, cap);
                cur.pop();
                used[i] = false;
                used[j] = false;
                if out.len() >= cap {
                    return;
                }
            }
        }
    }
    let mut out = Vec::new();
    let mut used = vec![false; n_cols];
    let mut cur = Vec::new();
    rec(n_cols, p, 0, &mut used, &mut cur, &mut out, cap);
    out
}

#[allow(clippy::too_many_arguments)]
fn composite_bake(
    v: &[f64],
    v_gexpr: &str,
    exclude: &[usize],
    divisor: bool,
    t: &[f64],
    features: &[Vec<f64>],
    n_cols: usize,
    sample_n: usize,
    qn: usize,
    min_valid: usize,
    chain: &[Red],
) -> Option<(String, f64)> {
    let mut lin = vec![f64::NAN; sample_n];
    let mut fin = 0usize;
    for k in 0..sample_n {
        let vv = v[k];
        lin[k] = if !t[k].is_finite() || !vv.is_finite() {
            f64::NAN
        } else if divisor {
            t[k] * vv
        } else if vv.abs() > 1e-30 {
            t[k] / vv
        } else {
            f64::NAN
        };
        if lin[k].is_finite() {
            fin += 1;
        }
    }
    if fin < min_valid {
        return None;
    }
    let inrefs: Vec<&[f64]> = features.iter().map(|c| &c[..sample_n]).collect();
    let (r2, sol_opt) = affine_monomial_best(&lin, &inrefs, qn);
    if r2 < COMPOSITE_R2 {
        return None;
    }
    let sol = sol_opt?;
    if !exclude.iter().all(|&c| (sol[c].round() as i32) == 0) {
        return None;
    }
    let mbuf = monomial_col_from_sol(&sol, features, n_cols, sample_n);
    let (_r2b, sa, sb) = r2_score(&lin[..sample_n], &mbuf[..sample_n]);
    let m_gexpr = monomial_gexpr_expanded(&sol, n_cols);
    let inner = if divisor {
        format!(
            "(({}*({})+{})/({}))",
            fmt_const(sa),
            m_gexpr,
            fmt_const(sb),
            v_gexpr
        )
    } else {
        format!(
            "(({}*({})+{})*({}))",
            fmt_const(sa),
            m_gexpr,
            fmt_const(sb),
            v_gexpr
        )
    };
    let model = invert_chain(chain, chain.len(), inner);
    if Parser::parse(&model).is_ok() {
        Some((model, r2))
    } else {
        None
    }
}

fn clean_monomial_fit(
    lin: &[f64],
    features: &[Vec<f64>],
    n_cols: usize,
    sample_n: usize,
    qn: usize,
) -> Option<Vec<i32>> {
    let q = qn.min(sample_n);
    let mut basis: Vec<Vec<f64>> = Vec::with_capacity(n_cols);
    for c in 0..n_cols {
        let mut col = vec![f64::NAN; q];
        for k in 0..q {
            let v = features[c][k].abs();
            col[k] = if v > 1e-300 { v.ln() } else { f64::NAN };
        }
        basis.push(col);
    }
    let mut y = vec![f64::NAN; q];
    for k in 0..q {
        let v = lin[k].abs();
        y[k] = if lin[k].is_finite() && v > 1e-300 {
            v.ln()
        } else {
            f64::NAN
        };
    }
    let sol = solve_ls(&basis, &y)?;
    let (mut sse, mut sy, mut syy, mut n) = (0.0f64, 0.0f64, 0.0f64, 0u32);
    for r in 0..q {
        if !y[r].is_finite() {
            continue;
        }
        let mut pred = sol[n_cols];
        let mut ok = true;
        for i in 0..n_cols {
            if !basis[i][r].is_finite() {
                ok = false;
                break;
            }
            pred += sol[i] * basis[i][r];
        }
        if !ok {
            continue;
        }
        let e = y[r] - pred;
        sse += e * e;
        sy += y[r];
        syy += y[r] * y[r];
        n += 1;
    }
    if n < HARVEST_MIN_ROWS as u32 {
        return None;
    }
    let sst = syy - sy * sy / n as f64;
    if sst <= 1e-12 || 1.0 - sse / sst < COMPOSITE_R2 {
        return None;
    }
    let mut exps = Vec::with_capacity(n_cols);
    let mut deg = 0i32;
    for i in 0..n_cols {
        let e = sol[i];
        if !e.is_finite() {
            return None;
        }
        let ei = e.round();
        if (e - ei).abs() > CLEAN_MONO_INT_TOL {
            return None;
        }
        let eii = ei as i32;
        deg += eii.abs();
        exps.push(eii);
    }
    if deg == 0 || deg > CLEAN_MONO_DEG_CAP {
        return None;
    }
    Some(exps)
}

#[allow(clippy::too_many_arguments)]
fn trig_clean_bake(
    v: &[f64],
    v_gexpr: &str,
    t: &[f64],
    features: &[Vec<f64>],
    n_cols: usize,
    sample_n: usize,
    qn: usize,
    min_valid: usize,
    chain: &[Red],
) -> Option<(String, f64)> {
    let mut lin = vec![f64::NAN; sample_n];
    let mut fin = 0usize;
    for k in 0..sample_n {
        lin[k] = if t[k].is_finite() && v[k].is_finite() && v[k].abs() > 1e-30 {
            t[k] / v[k]
        } else {
            f64::NAN
        };
        if lin[k].is_finite() {
            fin += 1;
        }
    }
    if fin < min_valid {
        return None;
    }
    let exps = clean_monomial_fit(&lin, features, n_cols, sample_n, qn)?;
    let sol_f: Vec<f64> = exps.iter().map(|&e| e as f64).collect();
    let mbuf = monomial_col_from_sol(&sol_f, features, n_cols, sample_n);
    let (r2, sa, sb) = r2_score(&lin[..sample_n], &mbuf[..sample_n]);
    if r2 < COMPOSITE_R2 || !sa.is_finite() || !sb.is_finite() {
        return None;
    }
    let m_gexpr = monomial_gexpr_expanded(&sol_f, n_cols);
    let inner = format!(
        "(({}*({})+{})*({}))",
        fmt_const(sa),
        m_gexpr,
        fmt_const(sb),
        v_gexpr
    );
    let model = invert_chain(chain, chain.len(), inner);
    if Parser::parse(&model).is_ok() {
        Some((model, r2))
    } else {
        None
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn gen_norm_finalists(
    t: &[f64],
    features: &[Vec<f64>],
    n_cols: usize,
    sample_n: usize,
    qn: usize,
    min_valid: usize,
    chain: &[Red],
) -> Vec<(String, f64)> {
    let mut out: Vec<(String, f64)> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let maxsz = NORM_MAX_ACTIVE.min(n_cols);
    let subset_budget = norm_subset_budget(n_cols);
    let mut n_subsets = 0usize;
    'outer: for sz in 2..=maxsz {
        for combo in combinations_capped(n_cols, sz, subset_budget + 1) {
            if n_subsets >= subset_budget || out_of_time() {
                break 'outer;
            }
            n_subsets += 1;
            let mut ssq = vec![f64::NAN; sample_n];
            for k in 0..sample_n {
                let mut acc = 0.0f64;
                let mut ok = true;
                for &c in &combo {
                    let v = features[c][k];
                    if !v.is_finite() {
                        ok = false;
                        break;
                    }
                    acc += v * v;
                }
                ssq[k] = if ok { acc } else { f64::NAN };
            }
            let hyp: Vec<f64> = ssq
                .iter()
                .map(|&x| {
                    if x.is_finite() && x >= 0.0 {
                        x.sqrt()
                    } else {
                        f64::NAN
                    }
                })
                .collect();
            let sq_parts: Vec<String> = combo
                .iter()
                .map(|&c| format!("((x{})**2)", c + 1))
                .collect();
            let ssq_gx = format!("({})", sq_parts.join("+"));
            let hyp_gx = format!("sqrt({})", ssq_gx);
            for (v, gx) in [(&ssq, &ssq_gx), (&hyp, &hyp_gx)] {
                for &divisor in &[true, false] {
                    let excl: &[usize] = if divisor { &combo } else { &[] };
                    if let Some((model, r2)) = composite_bake(
                        v, gx, excl, divisor, t, features, n_cols, sample_n, qn, min_valid, chain,
                    ) {
                        if !seen.contains(&model) {
                            seen.push(model.clone());
                            out.push((model, r2));
                        }
                    }
                }
            }
        }
    }
    let pmax = (n_cols / 2).min(NORM_PARTITION_MAX);
    for p in (1..=pmax).rev() {
        if out_of_time() {
            break;
        }
        if n_cols < 2 * p {
            continue;
        }
        for pairs in disjoint_pair_partitions(n_cols, p, SUMSQ_VISIT_CAP) {
            if out_of_time() {
                break;
            }
            let mut d = vec![f64::NAN; sample_n];
            for k in 0..sample_n {
                let mut acc = 0.0f64;
                let mut ok = true;
                for &(a, b) in &pairs {
                    let (va, vb) = (features[a][k], features[b][k]);
                    if !va.is_finite() || !vb.is_finite() {
                        ok = false;
                        break;
                    }
                    let df = va - vb;
                    acc += df * df;
                }
                d[k] = if ok { acc } else { f64::NAN };
            }
            let dparts: Vec<String> = pairs
                .iter()
                .map(|&(a, b)| format!("(x{}-x{})**2", a + 1, b + 1))
                .collect();
            let dgx = format!("({})", dparts.join("+"));
            let excl: Vec<usize> = pairs.iter().flat_map(|&(a, b)| [a, b]).collect();
            if let Some((model, r2)) = composite_bake(
                &d, &dgx, &excl, true, t, features, n_cols, sample_n, qn, min_valid, chain,
            ) {
                if !seen.contains(&model) {
                    seen.push(model.clone());
                    out.push((model, r2));
                }
                break;
            }
        }
    }
    out
}

struct TrigArg {
    data: Vec<f64>,
    gexpr: String,
    cols: Vec<usize>,
}

fn trig_args(features: &[Vec<f64>], n_cols: usize, sample_n: usize) -> Vec<TrigArg> {
    let mut out: Vec<TrigArg> = Vec::new();
    for i in 0..n_cols {
        out.push(TrigArg {
            data: features[i][..sample_n].to_vec(),
            gexpr: format!("(x{})", i + 1),
            cols: vec![i],
        });
        if out.len() >= TRIG_ARG_CAP {
            return out;
        }
    }
    for i in 0..n_cols {
        for j in (i + 1)..n_cols {
            let d: Vec<f64> = (0..sample_n)
                .map(|k| features[i][k] * features[j][k])
                .collect();
            out.push(TrigArg {
                data: d,
                gexpr: format!("(x{}*x{})", i + 1, j + 1),
                cols: vec![i, j],
            });
            if out.len() >= TRIG_ARG_CAP {
                return out;
            }
        }
    }
    for i in 0..n_cols {
        for j in 0..n_cols {
            for l in (j + 1)..n_cols {
                if i == j || i == l {
                    continue;
                }
                let d: Vec<f64> = (0..sample_n)
                    .map(|k| features[i][k] * (features[j][k] - features[l][k]))
                    .collect();
                out.push(TrigArg {
                    data: d,
                    gexpr: format!("(x{}*(x{}-x{}))", i + 1, j + 1, l + 1),
                    cols: vec![i, j, l],
                });
                if out.len() >= TRIG_ARG_CAP {
                    return out;
                }
            }
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn gen_trig_finalists(
    t: &[f64],
    features: &[Vec<f64>],
    n_cols: usize,
    sample_n: usize,
    qn: usize,
    min_valid: usize,
    chain: &[Red],
) -> Vec<(String, f64)> {
    let mut out: Vec<(String, f64)> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let sinc2 = |u: f64| -> f64 {
        let h = u * 0.5;
        if h.abs() > 1e-9 {
            let s = h.sin();
            (s * s) / (h * h)
        } else {
            1.0
        }
    };
    let push = |model: String, r2: f64, out: &mut Vec<(String, f64)>, seen: &mut Vec<String>| {
        if !seen.contains(&model) {
            seen.push(model.clone());
            out.push((model, r2));
        }
    };
    let mut v = vec![f64::NAN; sample_n];
    for arg in &trig_args(features, n_cols, sample_n) {
        if out_of_time() {
            break;
        }
        for n in 1..=TRIG_SCALE_HARM {
            let s = 1.0 / n as f64;
            let sgx = if n == 1 {
                arg.gexpr.clone()
            } else {
                format!("(({})/{})", arg.gexpr, n)
            };
            for &(tname, is_sin) in &[("sin", true), ("cos", false)] {
                for p in 1..=TRIG_POW_MAX {
                    for k in 0..sample_n {
                        let x = arg.data[k];
                        let a = if x.is_finite() {
                            if is_sin {
                                (s * x).sin()
                            } else {
                                (s * x).cos()
                            }
                        } else {
                            f64::NAN
                        };
                        v[k] = a.powi(p);
                    }
                    let vgx = format!("(({}({}))**{})", tname, sgx, p);
                    for &divisor in &[true, false] {
                        if let Some((model, r2)) = composite_bake(
                            &v, &vgx, &arg.cols, divisor, t, features, n_cols, sample_n, qn,
                            min_valid, chain,
                        ) {
                            push(model, r2, &mut out, &mut seen);
                        }
                    }
                }
            }
        }
        for k in 0..sample_n {
            v[k] = if arg.data[k].is_finite() {
                sinc2(arg.data[k])
            } else {
                f64::NAN
            };
        }
        let vgx = format!("((sin(({})/2))**2/(({})/2)**2)", arg.gexpr, arg.gexpr);
        if let Some((model, r2)) = trig_clean_bake(
            &v, &vgx, t, features, n_cols, sample_n, qn, min_valid, chain,
        ) {
            push(model, r2, &mut out, &mut seen);
        }
    }
    out
}

pub(crate) fn structurally_invisible(
    t: &[f64],
    inputs: &[&[f64]],
    sample_n: usize,
    qn: usize,
) -> bool {
    let max_corr = inputs
        .iter()
        .map(|c| abs_corr(c, t, sample_n))
        .fold(0.0f64, f64::max);
    if max_corr >= INVIS_CORR {
        return false;
    }
    monomial_loglin_r2(t, inputs, qn) < INVIS_MONO
}

pub(crate) fn residual_near_miss(t: &[f64], inputs: &[&[f64]], qn: usize) -> bool {
    let m = monomial_loglin_r2(t, inputs, qn);
    (NEAR_LO..NEAR_HI).contains(&m)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn gen_varpow_finalists(
    t: &[f64],
    features: &[Vec<f64>],
    n_cols: usize,
    sample_n: usize,
    qn: usize,
    min_valid: usize,
    chain: &[Red],
) -> Vec<(String, f64)> {
    let mut out: Vec<(String, f64)> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let mut v = vec![f64::NAN; sample_n];
    for base in 0..n_cols {
        if out_of_time() {
            break;
        }
        for exp in 0..n_cols {
            if base == exp {
                continue;
            }
            for k in 0..sample_n {
                let (bv, ev) = (features[base][k], features[exp][k]);
                v[k] = if bv > 1e-30 && ev.is_finite() {
                    (ev * bv.ln()).exp()
                } else {
                    f64::NAN
                };
            }
            let vgx = format!("(x{}**x{})", base + 1, exp + 1);
            for &divisor in &[false, true] {
                if let Some((m, r2)) = composite_bake(
                    &v,
                    &vgx,
                    &[base],
                    divisor,
                    t,
                    features,
                    n_cols,
                    sample_n,
                    qn,
                    min_valid,
                    chain,
                ) {
                    if !seen.contains(&m) {
                        seen.push(m.clone());
                        out.push((m, r2));
                    }
                }
            }
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn gen_rational_finalists(
    t: &[f64],
    features: &[Vec<f64>],
    n_cols: usize,
    sample_n: usize,
    qn: usize,
    min_valid: usize,
    chain: &[Red],
) -> Vec<(String, f64)> {
    let mut out: Vec<(String, f64)> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let inrefs: Vec<&[f64]> = features.iter().map(|c| &c[..sample_n]).collect();
    let mut d = vec![f64::NAN; sample_n];
    for i in 0..n_cols {
        if out_of_time() {
            break;
        }
        let score = |a: f64, d: &mut [f64]| -> f64 {
            for k in 0..sample_n {
                d[k] = if t[k].is_finite() && features[i][k].is_finite() {
                    t[k] * (1.0 + a * features[i][k])
                } else {
                    f64::NAN
                };
            }
            affine_monomial_r2(d, &inrefs, qn)
        };
        let mut best_a = 0.0f64;
        let mut best_s = -1.0f64;
        for e in CONST_DECADE_EXP.0..=CONST_DECADE_EXP.1 {
            for &m in CONST_DECADE_MANTISSA {
                for &sgn in &[1.0f64, -1.0] {
                    let a = sgn * m * 10f64.powi(e);
                    let s = score(a, &mut d);
                    if s > best_s {
                        best_s = s;
                        best_a = a;
                    }
                }
            }
        }
        if best_s < COMPOSITE_R2 || best_a == 0.0 {
            continue;
        }
        for k in 0..sample_n {
            d[k] = if features[i][k].is_finite() {
                1.0 + best_a * features[i][k]
            } else {
                f64::NAN
            };
        }
        let dgx = format!("(1+({})*x{})", fmt_const(best_a), i + 1);
        if let Some((m, r2)) = composite_bake(
            &d,
            &dgx,
            &[],
            true,
            t,
            features,
            n_cols,
            sample_n,
            qn,
            min_valid,
            chain,
        ) {
            if !seen.contains(&m) {
                seen.push(m.clone());
                out.push((m, r2));
            }
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn gen_additive_finalists(
    t: &[f64],
    features: &[Vec<f64>],
    n_cols: usize,
    sample_n: usize,
    _qn: usize,
    _min_valid: usize,
    chain: &[Red],
) -> Vec<(String, f64)> {
    if n_cols == 0 || n_cols > ADDITIVE_MAX_VARS {
        return Vec::new();
    }
    let mut firsts: Vec<(Vec<f64>, String)> = Vec::new();
    for c in 0..n_cols {
        let mut e1 = vec![0i32; n_cols];
        e1[c] = 1;
        firsts.push((features[c][..sample_n].to_vec(), monomial_strings(&e1).0));
        let mut e2 = vec![0i32; n_cols];
        e2[c] = 2;
        let sq: Vec<f64> = features[c][..sample_n].iter().map(|v| v * v).collect();
        firsts.push((sq, monomial_strings(&e2).0));
    }
    let mut vs: Vec<(Vec<f64>, String)> = Vec::new();
    for i in 0..n_cols {
        if out_of_time() {
            break;
        }
        for &(name, is_sin) in &[("sin", true), ("cos", false)] {
            let base: Vec<f64> = (0..sample_n)
                .map(|k| {
                    let x = features[i][k];
                    if !x.is_finite() {
                        f64::NAN
                    } else if is_sin {
                        x.sin()
                    } else {
                        x.cos()
                    }
                })
                .collect();
            vs.push((base.clone(), format!("{}(x{})", name, i + 1)));
            for j in 0..n_cols {
                if j == i {
                    continue;
                }
                let div: Vec<f64> = (0..sample_n)
                    .map(|k| {
                        let d = features[j][k];
                        if base[k].is_finite() && d.is_finite() && d.abs() > 1e-30 {
                            base[k] / d
                        } else {
                            f64::NAN
                        }
                    })
                    .collect();
                vs.push((div, format!("({}(x{})/(x{}))", name, i + 1, j + 1)));
            }
        }
    }
    let mut out: Vec<(String, f64)> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for (c1, c1gx) in &firsts {
        for (vcol, vgx) in &vs {
            let basis = vec![c1.clone(), vcol.clone()];
            let (r2, sol) = combined_r2_k(&basis, &t[..sample_n]);
            if r2 < ADDITIVE_ASSEMBLY_R2 {
                continue;
            }
            let Some(s) = sol else { continue };
            if s.len() != 3 || !s.iter().all(|c| c.is_finite()) || s[1] == 0.0 {
                continue;
            }
            let inner = format!(
                "(({}*({}))+({}*({}))+({}))",
                fmt_const(s[0]),
                c1gx,
                fmt_const(s[1]),
                vgx,
                fmt_const(s[2])
            );
            let model = invert_chain(chain, chain.len(), inner);
            if Parser::parse(&model).is_ok() && !seen.contains(&model) {
                seen.push(model.clone());
                out.push((model, r2));
            }
        }
    }
    out
}

pub(crate) struct VcolCapDesc {
    pub(crate) name: &'static str,
    pub(crate) min_cols: usize,
    pub(crate) cost_exp: i32,
}

pub(crate) const VCOL_CAPS: &[VcolCapDesc] = &[
    VcolCapDesc {
        name: "norm",
        min_cols: 2,
        cost_exp: 3,
    },
    VcolCapDesc {
        name: "trig",
        min_cols: 1,
        cost_exp: 2,
    },
    VcolCapDesc {
        name: "varpow",
        min_cols: 2,
        cost_exp: 2,
    },
    VcolCapDesc {
        name: "rational",
        min_cols: 1,
        cost_exp: 2,
    },
];
