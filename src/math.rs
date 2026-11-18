use crate::*;

#[inline]
pub(crate) fn r2_score(yt: &[f64], yp: &[f64]) -> (f64, f64, f64) {
    let mut n = 0u32;
    let (mut sx, mut sy, mut sxx, mut syy, mut sxy) = (0.0f64, 0.0, 0.0, 0.0, 0.0);
    for i in 0..yt.len().min(yp.len()) {
        let (x, y) = (yp[i], yt[i]);
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        n += 1;
        sx += x;
        sy += y;
        sxx += x * x;
        syy += y * y;
        sxy += x * y;
    }
    if n < 3 {
        return (-1.0, 0.0, 0.0);
    }
    let nf = n as f64;
    let dx = nf * sxx - sx * sx;
    let dy = nf * syy - sy * sy;
    if dx.abs() < 1e-30 || dy.abs() < 1e-30 {
        return (0.0, 0.0, 0.0);
    }
    let num = nf * sxy - sx * sy;
    let r2 = (num * num) / (dx * dy);
    let a = num / dx;
    let b = (sy - a * sx) / nf;
    (if r2.is_finite() { r2.min(1.0) } else { -1.0 }, a, b)
}

pub(crate) fn masked_divide(
    numer: &[f64],
    denom: &[f64],
    buf: &mut [f64],
    scratch: &mut Vec<f64>,
    n: usize,
) -> usize {
    scratch.clear();
    for k in 0..n {
        let d = denom[k];
        if d.is_finite() && d != 0.0 {
            scratch.push(d.abs());
        }
    }
    let eps = if scratch.len() >= 8 {
        let mid = scratch.len() / 2;
        scratch.select_nth_unstable_by(mid, |a, b| a.total_cmp(b));
        (scratch[mid] * 1e-6).max(1e-10)
    } else {
        1e-10
    };
    let mut vc = 0usize;
    for k in 0..n {
        let d = denom[k];
        if d.is_finite() && d.abs() > eps && numer[k].is_finite() {
            buf[k] = numer[k] / d;
            vc += 1;
        } else {
            buf[k] = f64::NAN;
        }
    }
    vc
}

pub(crate) fn loo_model_r2(
    model: &str,
    features: &[Vec<f64>],
    target: &[f64],
    n: usize,
    ev: &mut Evaluator,
) -> Option<(f64, usize)> {
    let parsed = Parser::parse(model).ok()?;
    if parsed.arity() > features.len() || n == 0 {
        return None;
    }
    let compiled = compile(&parsed);
    ev.ensure_capacity(compiled.max_stack.max(2), n);
    let vars: Vec<&[f64]> = features.iter().map(|col| &col[..n]).collect();
    let pred = ev.eval(&compiled.ops, &vars, n).to_vec();
    let rows: Vec<usize> = (0..n)
        .filter(|&i| pred[i].is_finite() && target[i].is_finite())
        .collect();
    let k = rows.len();
    if k < 3 {
        return None;
    }
    let kf = k as f64;
    let mm = rows.iter().map(|&i| pred[i]).sum::<f64>() / kf;
    let ym = rows.iter().map(|&i| target[i]).sum::<f64>() / kf;
    let sxx: f64 = rows.iter().map(|&i| (pred[i] - mm).powi(2)).sum();
    let sxy: f64 = rows
        .iter()
        .map(|&i| (pred[i] - mm) * (target[i] - ym))
        .sum();
    let syy: f64 = rows.iter().map(|&i| (target[i] - ym).powi(2)).sum();
    if !(sxx > 0.0) || !(syy > 0.0) {
        return None;
    }
    let a = sxy / sxx;
    let b = ym - a * mm;
    let mut press = 0.0f64;
    for &i in &rows {
        let h = 1.0 / kf + (pred[i] - mm).powi(2) / sxx;
        let e = target[i] - (a * pred[i] + b);
        let d = 1.0 - h;
        if !(d > 1e-12) {
            return None;
        }
        press += (e / d).powi(2);
    }
    let r2 = 1.0 - press / syy;
    r2.is_finite().then_some((r2, k))
}

pub(crate) fn rounding_floor(
    model: &str,
    features: &[Vec<f64>],
    target: &[f64],
    q_features: &[Vec<f64>],
    q_target: &[f64],
    n: usize,
    ev: &mut Evaluator,
) -> f64 {
    let Some(var) = rounding_var_rows(model, features, q_features, q_target, n, ev) else {
        return 0.0;
    };
    let ym = target[..n].iter().sum::<f64>() / n as f64;
    let vy = target[..n].iter().map(|v| (v - ym) * (v - ym)).sum::<f64>() / n as f64;
    if !(vy > 0.0) {
        return 0.0;
    }
    let f = var.iter().filter(|x| x.is_finite()).sum::<f64>() / n as f64 / vy;
    if f.is_finite() {
        f
    } else {
        0.0
    }
}

pub(crate) fn rounding_var_rows(
    model: &str,
    features: &[Vec<f64>],
    q_features: &[Vec<f64>],
    q_target: &[f64],
    n: usize,
    ev: &mut Evaluator,
) -> Option<Vec<f64>> {
    rounding_rows(model, features, q_features, q_target, n, ev)
        .map(|r| r.into_iter().map(|(v, _)| v).collect())
}

pub(crate) fn rounding_bound_rows(
    model: &str,
    features: &[Vec<f64>],
    q_features: &[Vec<f64>],
    q_target: &[f64],
    n: usize,
    ev: &mut Evaluator,
) -> Option<Vec<f64>> {
    rounding_rows(model, features, q_features, q_target, n, ev)
        .map(|r| r.into_iter().map(|(_, b)| b).collect())
}

fn rounding_rows(
    model: &str,
    features: &[Vec<f64>],
    q_features: &[Vec<f64>],
    q_target: &[f64],
    n: usize,
    ev: &mut Evaluator,
) -> Option<Vec<(f64, f64)>> {
    let parsed = Parser::parse(model).ok()?;
    if parsed.arity() > features.len() || n < 2 || q_target.len() < n {
        return None;
    }
    let compiled = compile(&parsed);
    ev.ensure_capacity(compiled.max_stack.max(2), n);
    let mut var = vec![0.0f64; n];
    let mut bound = vec![0.0f64; n];
    for i in 0..n {
        var[i] = q_target[i] * q_target[i] / 12.0;
        bound[i] = q_target[i] / 2.0;
    }
    for j in 0..features.len().min(q_features.len()) {
        if q_features[j].iter().take(n).all(|q| *q == 0.0) {
            continue;
        }
        let h: Vec<f64> = (0..n)
            .map(|i| 1e-6 * features[j][i].abs().max(1e-3))
            .collect();
        let mut up = features.to_vec();
        let mut dn = features.to_vec();
        for i in 0..n {
            up[j][i] += h[i];
            dn[j][i] -= h[i];
        }
        let vu: Vec<&[f64]> = up.iter().map(|c| &c[..n]).collect();
        let pu = ev.eval(&compiled.ops, &vu, n).to_vec();
        let vd: Vec<&[f64]> = dn.iter().map(|c| &c[..n]).collect();
        let pd = ev.eval(&compiled.ops, &vd, n).to_vec();
        for i in 0..n {
            let d = (pu[i] - pd[i]) / (2.0 * h[i]);
            if d.is_finite() {
                var[i] += d * d * q_features[j][i] * q_features[j][i] / 12.0;
                bound[i] += d.abs() * q_features[j][i] / 2.0;
            }
        }
    }
    Some(var.into_iter().zip(bound).collect())
}

pub(crate) fn mdl_dl(hn_f: f64, rh: f64, nodes: f64) -> f64 {
    0.5 * hn_f * (1.0 - rh).max(1e-12).ln() + 0.5 * hn_f.ln() * nodes
}

pub(crate) fn leverage_trimmed_r2(tyt: &[f64], typ: &[f64]) -> (f64, f64) {
    let (r0, a0, b0) = r2_score(tyt, typ);
    let n = tyt.len().min(typ.len());
    let mut resid: Vec<(f64, usize)> = Vec::with_capacity(n);
    for i in 0..n {
        let (p, t) = (typ[i], tyt[i]);
        if p.is_finite() && t.is_finite() {
            resid.push(((t - (a0 * p + b0)).abs(), i));
        }
    }
    let m = resid.len();
    let all = (r0, plain_r2(tyt, typ));
    if m < OUTLIER_TRIM_MIN_M {
        return all;
    }
    let mut mags: Vec<f64> = resid.iter().map(|x| x.0).collect();
    let mid = mags.len() / 2;
    mags.select_nth_unstable_by(mid, |a, b| a.total_cmp(b));
    let thresh = POLE_RESID_MULT * mags[mid].max(1e-300);
    let kmax = (m / OUTLIER_TRIM_DEN).max(OUTLIER_TRIM_MIN);
    resid.sort_unstable_by(|x, y| y.0.total_cmp(&x.0));
    let mut ndrop = 0usize;
    while ndrop < kmax && ndrop < m && resid[ndrop].0 > thresh {
        ndrop += 1;
    }
    if ndrop == 0 {
        return all;
    }
    let mut kt: Vec<f64> = Vec::with_capacity(m - ndrop);
    let mut kp: Vec<f64> = Vec::with_capacity(m - ndrop);
    for &(_, i) in &resid[ndrop..] {
        kt.push(tyt[i]);
        kp.push(typ[i]);
    }
    let (r1, _, _) = r2_score(&kt, &kp);
    if r1 >= r0 {
        (r1, plain_r2(&kt, &kp))
    } else {
        all
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn holdout_model_r2(
    model: &str,
    features: &[Vec<f64>],
    target: &[f64],
    h_lo: usize,
    h_hi: usize,
    ho_keep: &[bool],
    ho_min_rows: usize,
    ev: &mut Evaluator,
) -> Option<(f64, usize)> {
    let hold_n = h_hi - h_lo;
    if hold_n == 0 {
        return None;
    }
    let parsed = Parser::parse(model).ok()?;
    if parsed.arity() > features.len() {
        return None;
    }
    let compiled = compile(&parsed);
    ev.ensure_capacity(compiled.max_stack.max(2), hold_n);
    let vars: Vec<&[f64]> = features.iter().map(|col| &col[h_lo..h_hi]).collect();
    let pred = ev.eval(&compiled.ops, &vars, hold_n);
    let mut tyt: Vec<f64> = Vec::with_capacity(hold_n);
    let mut typ: Vec<f64> = Vec::with_capacity(hold_n);
    for (j, i) in (h_lo..h_hi).enumerate() {
        if ho_keep[j] && pred[j].is_finite() {
            tyt.push(target[i]);
            typ.push(pred[j]);
        }
    }
    if tyt.len() < ho_min_rows {
        return None;
    }
    let (rh, _) = leverage_trimmed_r2(&tyt, &typ);
    if rh.is_finite() {
        Some((rh, tyt.len()))
    } else {
        None
    }
}

pub(crate) fn plain_r2(yt: &[f64], yp: &[f64]) -> f64 {
    let n = yt.len().min(yp.len());
    let (mut sy, mut m) = (0.0f64, 0usize);
    for i in 0..n {
        if yt[i].is_finite() && yp[i].is_finite() {
            sy += yt[i];
            m += 1;
        }
    }
    if m < HOLDOUT_COVER_MIN {
        return -1.0;
    }
    let mean = sy / m as f64;
    let (mut ss_res, mut ss_tot) = (0.0f64, 0.0f64);
    for i in 0..n {
        if yt[i].is_finite() && yp[i].is_finite() {
            ss_res += (yt[i] - yp[i]) * (yt[i] - yp[i]);
            ss_tot += (yt[i] - mean) * (yt[i] - mean);
        }
    }
    if ss_tot <= 0.0 {
        return if ss_res <= 0.0 { 1.0 } else { 0.0 };
    }
    let r2 = 1.0 - ss_res / ss_tot;
    if r2.is_finite() {
        r2
    } else {
        -1.0
    }
}

fn full_model_score(
    model: &str,
    features: &[Vec<f64>],
    target: &[f64],
    ev: &mut Evaluator,
    score: impl Fn(&[f64], &[f64]) -> f64,
) -> f64 {
    let n = target.len();
    if n == 0 {
        return -1.0;
    }
    let Ok(parsed) = Parser::parse(model) else {
        return -1.0;
    };
    if parsed.arity() > features.len() {
        return -1.0;
    }
    let compiled = compile(&parsed);
    ev.ensure_capacity(compiled.max_stack.max(2), n);
    let vars: Vec<&[f64]> = features.iter().map(|col| &col[..n]).collect();
    let pred = ev.eval(&compiled.ops, &vars, n);
    let tt = target.iter().filter(|v| v.is_finite()).count();
    let kept = (0..n)
        .filter(|&i| target[i].is_finite() && pred[i].is_finite())
        .count();
    if kept * HOLDOUT_COVER_DEN < tt * HOLDOUT_COVER_NUM || kept < HOLDOUT_COVER_MIN {
        return -1.0;
    }
    score(target, pred)
}

pub(crate) fn full_model_r2(
    model: &str,
    features: &[Vec<f64>],
    target: &[f64],
    ev: &mut Evaluator,
) -> f64 {
    full_model_score(model, features, target, ev, plain_r2)
}

pub(crate) fn snap_value(v: f64) -> Option<f64> {
    if !v.is_finite() {
        return None;
    }
    let av = v.abs();
    if av >= SNAP_MAX_MAG {
        return None;
    }
    const MULT_MAX: f64 = SNAP_MULT_MAX;
    let mut best: Option<(f64, f64)> = None;
    for &(_, base) in CONSTANTS {
        if base <= 0.0 {
            continue;
        }
        for &q in SNAP_DENOMS {
            let p = (av / base * q as f64).round();
            if p == 0.0 && base != 1.0 {
                continue;
            }
            if base != 1.0 && (p > MULT_MAX || q as f64 > MULT_MAX) {
                continue;
            }
            if FEW_MULT_BASES.iter().any(|&b| b == base)
                && !matches!((p as u32, q), (1, 1) | (2, 1) | (1, 2))
            {
                continue;
            }
            let s = p / q as f64 * base;
            if s == av {
                return None;
            }
            let resid = (av - s).abs();
            let tol = (SNAP_TOL_REL * av)
                .max(SNAP_TOL_ABS)
                .min(SNAP_TOL_GRIDFRAC * base / q as f64);
            if resid <= tol && best.is_none_or(|(br, _)| resid < br) {
                best = Some((resid, s));
            }
        }
    }
    best.map(|(_, s)| if v < 0.0 { -s } else { s })
}

pub(crate) fn snap_exp(b: f64) -> Option<f64> {
    if !b.is_finite() {
        return None;
    }
    let mut best: Option<(f64, f64)> = None;
    for s in std::iter::once(0.0).chain(crate::forms::nf::Q::ALL.iter().map(|q| q.val())) {
        let r = (b - s).abs();
        if best.is_none_or(|(br, _)| r < br) {
            best = Some((r, s));
        }
    }
    match best {
        Some((r, s)) if r <= EXP_SNAP_TOL => Some(s),
        _ => None,
    }
}

pub(crate) fn solve_ls(basis: &[Vec<f64>], y: &[f64]) -> Option<Vec<f64>> {
    let k = basis.len();
    let m = k + 1;
    let n = y.len();
    let mut ata = vec![vec![0.0f64; m]; m];
    let mut atb = vec![0.0f64; m];
    let mut rows = 0u32;
    let mut x = vec![0.0f64; m];
    for r in 0..n {
        if !y[r].is_finite() {
            continue;
        }
        let mut ok = true;
        for (i, b) in basis.iter().enumerate() {
            if !b[r].is_finite() {
                ok = false;
                break;
            }
            x[i] = b[r];
        }
        if !ok {
            continue;
        }
        x[k] = 1.0;
        for i in 0..m {
            for j in 0..m {
                ata[i][j] += x[i] * x[j];
            }
            atb[i] += x[i] * y[r];
        }
        rows += 1;
    }
    if (rows as usize) < fit_min_rows(m) {
        return None;
    }
    for col in 0..m {
        let mut piv = col;
        for r in (col + 1)..m {
            if ata[r][col].abs() > ata[piv][col].abs() {
                piv = r;
            }
        }
        if ata[piv][col].abs() < 1e-12 {
            return None;
        }
        ata.swap(col, piv);
        atb.swap(col, piv);
        for r in (col + 1)..m {
            let f = ata[r][col] / ata[col][col];
            for c in col..m {
                ata[r][c] -= f * ata[col][c];
            }
            atb[r] -= f * atb[col];
        }
    }
    let mut sol = vec![0.0f64; m];
    for r in (0..m).rev() {
        let mut s = atb[r];
        for c in (r + 1)..m {
            s -= ata[r][c] * sol[c];
        }
        sol[r] = s / ata[r][r];
    }
    if sol.iter().any(|v| !v.is_finite()) {
        return None;
    }
    Some(sol)
}
