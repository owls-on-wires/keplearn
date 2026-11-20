use crate::*;

pub(crate) fn harvest_monomial_vcols(
    t: &[f64],
    features: &[Vec<f64>],
    n_cols: usize,
    sample_n: usize,
    qn: usize,
    max_terms: usize,
) -> Vec<VC> {
    let q = qn.min(sample_n);
    if q < fit_min_rows(n_cols + 1) || n_cols == 0 {
        return Vec::new();
    }
    let mut logcols: Vec<Vec<f64>> = Vec::with_capacity(n_cols);
    for c in 0..n_cols {
        let mut col = vec![f64::NAN; sample_n];
        for k in 0..sample_n {
            let v = features[c][k].abs();
            col[k] = if v > 1e-300 { v.ln() } else { f64::NAN };
        }
        logcols.push(col);
    }
    let mut resid: Vec<f64> = t[..sample_n].to_vec();
    let mut out: Vec<VC> = Vec::new();
    let mut used: Vec<Vec<i32>> = Vec::new();
    for _iter in 0..(max_terms + PURSUIT_EXTRA_ROUNDS) {
        if out_of_budget() {
            break;
        }
        if out.len() >= max_terms {
            break;
        }
        let mut y = vec![f64::NAN; sample_n];
        for k in 0..sample_n {
            let v = resid[k].abs();
            y[k] = if resid[k].is_finite() && v > 1e-300 {
                v.ln()
            } else {
                f64::NAN
            };
        }
        let Some(sol) = solve_ls(&logcols, &y) else {
            break;
        };
        let mut exps: Vec<i32> = Vec::with_capacity(n_cols);
        let mut deg = 0i32;
        for c in 0..n_cols {
            let e = sol[c];
            let ei = round_clamp_exp(e, &MONO_HARVEST);
            deg += ei.abs();
            exps.push(ei);
        }
        if deg == 0 || deg > MONO_HARVEST.deg_cap {
            break;
        }
        if used.iter().any(|u| *u == exps) {
            break;
        }
        let mut col = vec![0.0f64; sample_n];
        for k in 0..sample_n {
            let mut p = 1.0f64;
            for c in 0..n_cols {
                if exps[c] != 0 {
                    p *= features[c][k].powi(exps[c]);
                }
            }
            col[k] = p;
        }
        let finite = col.iter().filter(|x| x.is_finite()).count();
        if finite < min_finite_rows(q) {
            break;
        }
        let (r2, a, b) = r2_score(&resid[..sample_n], &col[..sample_n]);
        let _ = b;
        if !(r2 >= MONO_MIN_R2) || !a.is_finite() || a == 0.0 {
            break;
        }
        if deg >= HARVEST_VCOL_MIN_DEG {
            let (gexpr, desc) = monomial_strings(&exps);
            out.push(VC {
                gexpr,
                desc,
                data: Rc::new(col.clone()),
            });
        }
        used.push(exps);
        for k in 0..sample_n {
            if resid[k].is_finite() && col[k].is_finite() {
                resid[k] -= a * col[k];
            }
        }
    }
    out
}

pub(crate) fn mono_fit(inputs: &[&[f64]], target: &[f64], sample_n: usize, root: bool) -> Vec<Hit> {
    let min_r2 = if root {
        MONO_FIT_ROOT_MIN_R2
    } else {
        MONO_FIT_MIN_R2
    };
    let eps = 1e-12;
    let sn = sample_n.min(target.len());
    let k = inputs.len();
    if !(MONO_FIT_MIN_ARITY..=12).contains(&k) || sn < fit_min_rows(k + 1) {
        return Vec::new();
    }
    let mut basis: Vec<Vec<f64>> = Vec::with_capacity(k);
    for &col in inputs.iter() {
        let mut lc = vec![f64::NAN; sn];
        for r in 0..sn {
            let v = col[r];
            if v.is_finite() && v.abs() > eps {
                lc[r] = v.abs().ln();
            }
        }
        basis.push(lc);
    }
    let mut out: Vec<Hit> = Vec::new();
    for &off in const_offsets_fit(target, &basis, sn).iter() {
        let mut ly = vec![f64::NAN; sn];
        for r in 0..sn {
            let t = target[r] - off;
            if target[r].is_finite() && t.abs() > eps {
                ly[r] = t.abs().ln();
            }
        }
        let Some(sol) = solve_ls(&basis, &ly) else {
            continue;
        };
        let se = if root {
            crate::math::ls_std_errors(&basis, &ly, &sol)
        } else {
            None
        };
        let mut exps: Vec<(usize, f64)> = Vec::new();
        let mut clean = true;
        for c in 0..k {
            let b = sol[c];
            let tol = se.as_ref().map_or(EXP_SNAP_TOL, |se| {
                (EXP_SNAP_Z * se[c]).min(EXP_SNAP_MAX).max(EXP_SNAP_TOL)
            });
            match crate::math::snap_exp_within(b, tol) {
                Some(snapped) => {
                    if snapped != 0.0 {
                        exps.push((c, snapped));
                    }
                }
                None => {
                    clean = false;
                    break;
                }
            }
        }
        if !clean || exps.is_empty() || exps.len() > MONO_FIT_EXP_CAP {
            continue;
        }
        let mut m = vec![f64::NAN; sn];
        for r in 0..sn {
            let mut p = 1.0f64;
            let mut ok = true;
            for &(c, e) in &exps {
                let v = inputs[c][r];
                if !v.is_finite() {
                    ok = false;
                    break;
                }
                p *= v.powf(e);
            }
            if ok && p.is_finite() {
                m[r] = p;
            }
        }
        let (r2, a, b) = r2_score(&target[..sn], &m);
        if !(r2 >= min_r2) || !a.is_finite() || a == 0.0 || !b.is_finite() {
            continue;
        }
        if r2 < MONO_FIT_MIN_R2 {
            let e: Vec<f64> = (0..sn)
                .map(|r| ly[r] - sol[k] - (0..k).map(|c| sol[c] * basis[c][r]).sum::<f64>())
                .collect();
            if crate::math::residuals_structured(&basis, &e) != Some(false) {
                continue;
            }
        }
        let mut expr = String::from("(");
        for (i, &(_c, e)) in exps.iter().enumerate() {
            if i > 0 {
                expr.push('*');
            }
            expr.push_str(&format!("(x{}**({}))", i + 1, fmt_exp(e)));
        }
        expr.push(')');
        let columns: Vec<usize> = exps.iter().map(|&(c, _)| c).collect();
        if out.iter().any(|h| h.expr == expr && h.columns == columns) {
            continue;
        }
        let is_raw = off == 0.0;
        out.push(Hit {
            expr,
            r2,
            scale: a,
            offset: b,
            columns,
            gate_r2: r2,
        });
        if is_raw {
            break;
        }
    }
    out
}

pub(crate) fn harvest_cheap(t: &[f64], inputs: &[&[f64]], qn: usize) -> f64 {
    affine_monomial_r2(t, inputs, qn)
        .max(monomial_loglin_r2(t, inputs, qn))
        .max(quick_probe(t, inputs, qn))
}

pub(crate) fn harvest_la(t: &[f64], inputs: &[&[f64]], qn: usize, sample_n: usize) -> f64 {
    let base = harvest_cheap(t, inputs, qn);
    if base >= IO_NEAREXACT_R2 {
        return base;
    }
    let la = fit_shell_const(t, inputs, qn, sample_n, |y, c| {
        let z = 1.0 + c * y;
        if z > 1e-30 {
            z.ln()
        } else {
            f64::NAN
        }
    })
    .1;
    base.max(la)
}
