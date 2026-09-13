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
        scratch.select_nth_unstable_by(mid, |a, b| a.partial_cmp(b).unwrap_or(CmpOrd::Equal));
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

pub(crate) fn abs_corr(x: &[f64], y: &[f64], n: usize) -> f64 {
    let (r2, _, _) = r2_score(&y[..n.min(y.len())], &x[..n.min(x.len())]);
    if r2 > 0.0 {
        r2.sqrt()
    } else {
        0.0
    }
}

pub(crate) fn combinations_capped(n: usize, k: usize, cap: usize) -> Vec<Vec<usize>> {
    let mut result = Vec::new();
    fn go(
        start: usize,
        n: usize,
        k: usize,
        cap: usize,
        cur: &mut Vec<usize>,
        out: &mut Vec<Vec<usize>>,
    ) {
        if out.len() >= cap {
            return;
        }
        if cur.len() == k {
            out.push(cur.clone());
            return;
        }
        for i in start..n {
            cur.push(i);
            go(i + 1, n, k, cap, cur, out);
            cur.pop();
            if out.len() >= cap {
                return;
            }
        }
    }
    go(0, n, k, cap, &mut Vec::new(), &mut result);
    result
}

pub(crate) fn mdl_dl(hn_f: f64, rh: f64, nodes: f64) -> f64 {
    0.5 * hn_f * (1.0 - rh).max(1e-12).ln() + 0.5 * hn_f.ln() * nodes
}

pub(crate) fn leverage_trimmed_r2(tyt: &[f64], typ: &[f64]) -> (f64, f64, f64) {
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
    if m < OUTLIER_TRIM_MIN_M {
        return (r0, a0, b0);
    }
    let mut mags: Vec<f64> = resid.iter().map(|x| x.0).collect();
    let mid = mags.len() / 2;
    mags.select_nth_unstable_by(mid, |a, b| a.partial_cmp(b).unwrap_or(CmpOrd::Equal));
    let thresh = POLE_RESID_MULT * mags[mid].max(1e-300);
    let kmax = (m / OUTLIER_TRIM_DEN).max(OUTLIER_TRIM_MIN);
    resid.sort_unstable_by(|x, y| y.0.partial_cmp(&x.0).unwrap_or(CmpOrd::Equal));
    let mut ndrop = 0usize;
    while ndrop < kmax && ndrop < m && resid[ndrop].0 > thresh {
        ndrop += 1;
    }
    if ndrop == 0 {
        return (r0, a0, b0);
    }
    let mut kt: Vec<f64> = Vec::with_capacity(m - ndrop);
    let mut kp: Vec<f64> = Vec::with_capacity(m - ndrop);
    for &(_, i) in &resid[ndrop..] {
        kt.push(tyt[i]);
        kp.push(typ[i]);
    }
    let (r1, a1, b1) = r2_score(&kt, &kp);
    if r1 >= r0 {
        (r1, a1, b1)
    } else {
        (r0, a0, b0)
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
    let (rh, _, _) = leverage_trimmed_r2(&tyt, &typ);
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

pub(crate) fn full_model_affine_fit(
    model: &str,
    features: &[Vec<f64>],
    target: &[f64],
    ev: &mut Evaluator,
) -> Option<(f64, f64)> {
    let n = target.len();
    if n == 0 {
        return None;
    }
    let parsed = Parser::parse(model).ok()?;
    if parsed.arity() > features.len() {
        return None;
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
        return None;
    }
    let (_, a, b) = r2_score(target, pred);
    (a.is_finite() && b.is_finite() && a != 0.0).then_some((a, b))
}

pub(crate) fn full_model_affine_r2(
    model: &str,
    features: &[Vec<f64>],
    target: &[f64],
    ev: &mut Evaluator,
) -> f64 {
    full_model_score(model, features, target, ev, |yt, yp| r2_score(yt, yp).0)
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
    for &q in EXP_DENOMS {
        let s = (b * q as f64).round() / q as f64;
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

pub(crate) fn literal_spans(s: &str) -> Vec<(usize, usize, f64)> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        if b[i].is_ascii_digit() {
            let prev = if i == 0 { b'\0' } else { b[i - 1] };
            if prev.is_ascii_alphanumeric() || prev == b'_' {
                while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
                    i += 1;
                }
                continue;
            }
            let start = i;
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
                i += 1;
            }
            if let Ok(v) = s[start..i].parse::<f64>() {
                out.push((start, i, v));
            }
        } else {
            i += 1;
        }
    }
    out
}

pub(crate) fn apply_snaps(model: &str, snaps: &[(usize, usize, String)], mask: &[bool]) -> String {
    let mut out = String::with_capacity(model.len() + 16);
    let mut pos = 0usize;
    for (k, &(a, b, ref rep)) in snaps.iter().enumerate() {
        if !mask[k] {
            continue;
        }
        out.push_str(&model[pos..a]);
        out.push_str(rep);
        pos = b;
    }
    out.push_str(&model[pos..]);
    out
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn snap_model_constants(
    model: &str,
    base_rh: f64,
    features: &[Vec<f64>],
    target: &[f64],
    h_lo: usize,
    h_hi: usize,
    ho_keep: &[bool],
    ho_min_rows: usize,
    ev: &mut Evaluator,
) -> Option<String> {
    let spans = literal_spans(model);
    let mmax = spans
        .iter()
        .map(|&(_, _, v)| v.abs())
        .fold(0.0f64, f64::max);
    let snaps: Vec<(usize, usize, String)> = spans
        .into_iter()
        .filter_map(|(a, b, v)| {
            let s = snap_value(v)
                .or_else(|| (v != 0.0 && v.abs() <= SNAP_ZERO_FRAC * mmax).then_some(0.0))?;
            Some((a, b, fmt_const(s)))
        })
        .collect();
    if snaps.is_empty() || snaps.len() > SNAP_MAX_COUNT {
        return None;
    }
    let slack = (1.0 - base_rh).max(0.0) * SNAP_HOLDOUT_SLACK + 1e-9;
    let base_full = full_model_affine_r2(model, features, target, ev);
    let full_slack = (1.0 - base_full).max(0.0) * SNAP_HOLDOUT_SLACK + 1e-9;
    let verify = |m: &str, ev: &mut Evaluator| -> bool {
        let ho_ok = holdout_model_r2(m, features, target, h_lo, h_hi, ho_keep, ho_min_rows, ev)
            .map(|(rh, _)| rh >= base_rh - slack)
            .unwrap_or(false);
        ho_ok
            && (base_full <= -1.0
                || full_model_affine_r2(m, features, target, ev) >= base_full - full_slack)
    };
    let all = vec![true; snaps.len()];
    let m = apply_snaps(model, &snaps, &all);
    if verify(&m, ev) {
        return Some(m);
    }
    let mut mask = vec![false; snaps.len()];
    for k in 0..snaps.len() {
        mask[k] = true;
        let m = apply_snaps(model, &snaps, &mask);
        if !verify(&m, ev) {
            mask[k] = false;
        }
    }
    if mask.iter().any(|&x| x) {
        Some(apply_snaps(model, &snaps, &mask))
    } else {
        None
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
    if rows < (m as u32 + 8) {
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
