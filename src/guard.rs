thread_local! {
    static FLOOR : std::cell::Cell < f64 > = const { std::cell::Cell::new(0.0) };
}

pub(crate) fn set_floor(f: f64) {
    FLOOR.with(|c| c.set(f));
}

pub(crate) fn canonical_answer(
    model: &str,
    features: &[Vec<f64>],
    target: &[f64],
    n_cols: usize,
    train_n: usize,
) -> Option<(String, String)> {
    let (e, fr, exact, _) = canonical_core(model, features, target, n_cols, train_n)?;
    let (ce, cf) = clean_answer(
        e.clone(),
        fr.fitted.clone(),
        features,
        target,
        n_cols,
        train_n,
    );
    let cleaned = crate::forms::nf::print_answer(&ce, &cf);
    let (e, f) = if exact && misses_far_beyond_precision(&cleaned, features, target, train_n) {
        (e, fr.fitted)
    } else {
        (ce, cf)
    };
    Some((
        crate::forms::nf::print_structural(&e),
        crate::forms::nf::print_answer(&e, &f),
    ))
}

pub(crate) fn simple_near(v: f64) -> Option<f64> {
    if v == 0.0 || !v.is_finite() || structural_constant(v) {
        return None;
    }
    let close = |s: f64| s != 0.0 && ((s - v) / v).abs() <= crate::config::CLEAN_REL;
    let sig = |d: i32| {
        let p = 10f64.powi(v.abs().log10().floor() as i32 - d + 1);
        (v / p).round() * p
    };
    if close(sig(1)) {
        return Some(sig(1));
    }
    let m = crate::config::STRUCT_RATIONAL_MAX as i64;
    let mut best: Option<(f64, f64)> = None;
    for q in 2..=m {
        let p = (v * q as f64).round();
        let s = p / q as f64;
        if p != 0.0 && p.abs() <= m as f64 && close(s) {
            let d = ((s - v) / v).abs();
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some((s, d));
            }
        }
    }
    if let Some((s, _)) = best {
        return Some(s);
    }
    close(sig(2)).then(|| sig(2))
}

fn clean_answer(
    e: crate::forms::nf::Expr,
    f: crate::forms::nf::Fitted,
    features: &[Vec<f64>],
    target: &[f64],
    n_cols: usize,
    train_n: usize,
) -> (crate::forms::nf::Expr, crate::forms::nf::Fitted) {
    use crate::forms::nf::{eval_terms, Fitted};
    let cols: Vec<Vec<f64>> = features[..n_cols]
        .iter()
        .map(|c| c[..train_n].to_vec())
        .collect();
    let y = &target[..train_n];
    let mean_abs = {
        let v: Vec<f64> = y
            .iter()
            .filter(|v| v.is_finite())
            .map(|v| v.abs())
            .collect();
        v.iter().sum::<f64>() / v.len().max(1) as f64
    };
    let unexplained = |e: &crate::forms::nf::Expr, f: &Fitted| -> f64 {
        let t = eval_terms(e, &cols, &f.inner);
        let p: Vec<f64> = (0..y.len())
            .map(|r| f.offset + f.coef.iter().zip(&t).map(|(c, g)| c * g[r]).sum::<f64>())
            .collect();
        let rows: Vec<usize> = (0..y.len())
            .filter(|&r| y[r].is_finite() && p[r].is_finite())
            .collect();
        if rows.len() < 2 {
            return f64::INFINITY;
        }
        let m = rows.iter().map(|&r| y[r]).sum::<f64>() / rows.len() as f64;
        let sst: f64 = rows.iter().map(|&r| (y[r] - m).powi(2)).sum();
        let ssr: f64 = rows.iter().map(|&r| (y[r] - p[r]).powi(2)).sum();
        if sst > 0.0 && rows.len() * 10 >= y.len() * 9 {
            ssr / sst
        } else {
            f64::INFINITY
        }
    };
    let mut c = f.clone();
    let mut changed = false;
    let common = common_constants();
    let with_physical: Vec<f64> = common
        .iter()
        .copied()
        .chain(crate::forms::nf::physical_multiples())
        .collect();
    let pin = |c: &Fitted, inner: bool, k: usize| -> Option<(f64, f64)> {
        let v = if inner { c.inner[k] } else { c.coef[k] };
        let at = |x: f64| -> Option<f64> {
            let mut t = c.clone();
            if inner {
                t.inner[k] = x;
                if !refit_linear(&e, &cols, y, &mut t) {
                    return None;
                }
            } else {
                t.coef[k] = x;
            }
            Some(unexplained(&e, &t)).filter(|u| u.is_finite())
        };
        let d = crate::config::NAME_MAX_REL_SE;
        let (u0, up, dn) = (unexplained(&e, c), at(v * (1.0 + d))?, at(v * (1.0 - d))?);
        let rise = (up + dn) / 2.0 - u0;
        if !(rise > 0.0) {
            return None;
        }
        let slope = (up - dn) / 2.0;
        Some(((u0 - slope * slope / (4.0 * rise)).max(0.0), rise))
    };
    let name_slots = |c: &mut Fitted, inner: bool| -> bool {
        let rows = (0..y.len()).filter(|&r| y[r].is_finite()).count();
        let free = c.coef.iter().filter(|v| !structural_constant(**v)).count()
            + usize::from(!structural_constant(c.offset))
            + c.inner.iter().filter(|v| !structural_constant(**v)).count();
        let dof = rows.saturating_sub(free).max(1) as f64;
        let mut any = false;
        for k in 0..if inner { c.inner.len() } else { c.coef.len() } {
            let v = if inner { c.inner[k] } else { c.coef[k] };
            if v == 0.0 || structural_constant(v) {
                continue;
            }
            let scale_only =
                !inner && c.offset == 0.0 && c.coef.iter().filter(|x| **x != 0.0).count() == 1;
            let candidates = if scale_only { &with_physical } else { &common };
            let Some(name) = candidates
                .iter()
                .map(|n| n.copysign(v))
                .filter(|n| ((n - v) / v).abs() <= COMMON_PREFILTER)
                .min_by(|a, b| (a - v).abs().total_cmp(&(b - v).abs()))
            else {
                continue;
            };
            let mut trial = c.clone();
            if inner {
                trial.inner[k] = name;
                if !refit_linear(&e, &cols, y, &mut trial) {
                    continue;
                }
            } else {
                trial.coef[k] = name;
            }
            let u_new = unexplained(&e, &trial);
            let Some((u_min, rise)) = pin(c, inner, k) else {
                continue;
            };
            let pinned = rise >= u_min / dof;
            if pinned
                && u_new.is_finite()
                && u_new - u_min <= crate::config::COMMON_Z2 * u_min / dof
            {
                *c = trial;
                any = true;
            }
        }
        any
    };
    {
        let t = eval_terms(&e, &cols, &c.inner);
        let live = c.coef.iter().filter(|v| **v != 0.0).count();
        let mut dropped = 0;
        for (j, g) in t.iter().enumerate() {
            let most = g
                .iter()
                .filter(|v| v.is_finite())
                .fold(0.0f64, |m, v| m.max((c.coef[j] * v).abs()));
            if c.coef[j] != 0.0 && live - dropped > 1 && most <= crate::config::CLEAN_REL * mean_abs
            {
                c.coef[j] = 0.0;
                dropped += 1;
                changed = true;
            }
        }
    }
    let unpin = |c: &mut Fitted, inner: bool| -> bool {
        let rows = (0..y.len()).filter(|&r| y[r].is_finite()).count();
        let free = c.coef.iter().filter(|v| !structural_constant(**v)).count()
            + usize::from(!structural_constant(c.offset))
            + c.inner.iter().filter(|v| !structural_constant(**v)).count();
        let dof = rows.saturating_sub(free.max(1)).max(1) as f64;
        let mut any = false;
        for k in 0..if inner { c.inner.len() } else { c.coef.len() } {
            let v = if inner { c.inner[k] } else { c.coef[k] };
            let named = v != 0.0
                && (crate::forms::nf::symbolic_constant(v).is_some()
                    || crate::forms::nf::physical_constant(v).is_some());
            if !named {
                continue;
            }
            let set = |c: &Fitted, x: f64| -> Option<Fitted> {
                let mut t = c.clone();
                if inner {
                    t.inner[k] = x;
                    refit_linear(&e, &cols, y, &mut t).then_some(t)
                } else {
                    t.coef[k] = x;
                    Some(t)
                }
            };
            let u0 = unexplained(&e, c);
            match pin(c, inner, k) {
                Some((u_min, rise)) if rise < u_min / dof => {}
                _ => continue,
            }
            for d in 1..=4 {
                let m = 10f64.powi(d - 1 - v.abs().log10().floor() as i32);
                let r = (v * m).round() / m;
                if let Some(t) = set(c, r) {
                    if unexplained(&e, &t) - u0 <= u0 / dof {
                        *c = t;
                        any = true;
                        break;
                    }
                }
            }
        }
        any
    };
    changed |= unpin(&mut c, true);
    changed |= name_slots(&mut c, true);
    for v in c.inner.iter_mut() {
        if let Some(s) = simple_near(*v).filter(|s| s != v) {
            *v = s;
            changed = true;
        }
    }
    if changed && !refit_linear(&e, &cols, y, &mut c) {
        return (e, f);
    }
    changed |= unpin(&mut c, false);
    changed |= name_slots(&mut c, false);
    for v in c.coef.iter_mut() {
        if let Some(s) = simple_near(*v).filter(|s| s != v) {
            *v = s;
            changed = true;
        }
    }
    if c.offset != 0.0 && c.offset.abs() <= crate::config::CLEAN_REL * mean_abs {
        c.offset = 0.0;
        changed = true;
    } else if let Some(s) = simple_near(c.offset).filter(|s| *s != c.offset) {
        c.offset = s;
        changed = true;
    }
    if !changed {
        return (e, f);
    }
    let (u0, u1) = (unexplained(&e, &f), unexplained(&e, &c));
    if !(u1 <= (crate::config::CLEAN_LOSS_MAX * u0).max(crate::config::CLEAN_LOSS_ABS)) {
        return (e, f);
    }
    let printed = crate::forms::nf::print_answer(&e, &c);
    match crate::forms::ast::parse(&printed)
        .ok()
        .and_then(|a| crate::forms::canon::normalize(&a).ok())
    {
        Some((e2, f2)) if f2.coef.len() == e2.terms.len() => (e2, f2),
        _ => (e, c),
    }
}

fn common_constants() -> Vec<f64> {
    use std::f64::consts::{E, LN_2, PI, SQRT_2};
    let mut out = Vec::new();
    let bases: [(f64, &[f64]); 7] = [
        (PI, &[0.25, 0.5, 1.0, 2.0, 4.0, 4.0 / 3.0]),
        (E, &[0.5, 1.0, 2.0]),
        (SQRT_2, &[0.5, 1.0, 2.0]),
        (3f64.sqrt(), &[0.5, 1.0, 2.0]),
        (LN_2, &[0.5, 1.0, 2.0]),
        (PI.sqrt(), &[0.5, 1.0, 2.0]),
        ((2.0 * PI).sqrt(), &[1.0]),
    ];
    for (b, ms) in bases {
        for m in ms {
            out.push(m * b);
            out.push(1.0 / (m * b));
        }
    }
    out
}

const COMMON_PREFILTER: f64 = 0.02;

fn refit_linear(
    e: &crate::forms::nf::Expr,
    cols: &[Vec<f64>],
    y: &[f64],
    c: &mut crate::forms::nf::Fitted,
) -> bool {
    let all = crate::forms::nf::eval_terms(e, cols, &c.inner);
    let live: Vec<usize> = (0..all.len()).filter(|&j| c.coef[j] != 0.0).collect();
    let t: Vec<&Vec<f64>> = live.iter().map(|&j| &all[j]).collect();
    let rows: Vec<usize> = (0..y.len())
        .filter(|&r| y[r].is_finite() && t.iter().all(|g| g[r].is_finite()))
        .collect();
    let with_offset = c.offset != 0.0;
    let k = t.len() + usize::from(with_offset);
    if k == 0 || rows.len() < k {
        return false;
    }
    let col = |i: usize, r: usize| if i < t.len() { t[i][r] } else { 1.0 };
    let mut a = vec![vec![0.0f64; k + 1]; k];
    for i in 0..k {
        for j in 0..k {
            a[i][j] = rows.iter().map(|&r| col(i, r) * col(j, r)).sum();
        }
        a[i][k] = rows.iter().map(|&r| col(i, r) * y[r]).sum();
    }
    for p in 0..k {
        let Some(piv) = (p..k).max_by(|&x, &z| a[x][p].abs().total_cmp(&a[z][p].abs())) else {
            return false;
        };
        a.swap(p, piv);
        if !(a[p][p].abs() > 1e-300) {
            return false;
        }
        for r in 0..k {
            if r != p {
                let m = a[r][p] / a[p][p];
                for j in p..=k {
                    a[r][j] -= m * a[p][j];
                }
            }
        }
    }
    let sol: Vec<f64> = (0..k).map(|i| a[i][k] / a[i][i]).collect();
    if !sol.iter().all(|v| v.is_finite()) {
        return false;
    }
    for (i, &j) in live.iter().enumerate() {
        c.coef[j] = sol[i];
    }
    c.offset = if with_offset { sol[t.len()] } else { 0.0 };
    true
}

pub(crate) fn median_miss(
    model: &str,
    features: &[Vec<f64>],
    target: &[f64],
    n: usize,
) -> Option<f64> {
    let ast = crate::forms::ast::parse(model).ok()?;
    let cols: Vec<Vec<f64>> = features.iter().map(|c| c[..n].to_vec()).collect();
    let ys: Vec<f64> = target[..n]
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .collect();
    if ys.is_empty() || !(ys.iter().all(|v| *v > 0.0) || ys.iter().all(|v| *v < 0.0)) {
        return None;
    }
    let mut m: Vec<f64> = (0..n)
        .filter(|&i| target[i].is_finite())
        .map(|i| ((target[i] - ast.eval(&cols, i)) / target[i]).abs())
        .filter(|v| v.is_finite())
        .collect();
    if m.len() * 10 < ys.len() * 9 {
        return None;
    }
    m.sort_by(|a, b| a.total_cmp(b));
    Some(m[m.len() / 2])
}

pub(crate) fn exit_check(
    model: &str,
    features: &[Vec<f64>],
    target: &[f64],
    n_cols: usize,
    train_n: usize,
) -> Option<(usize, bool)> {
    let (e, fr, exact, xf) = canonical_core(model, features, target, n_cols, train_n)?;
    if !exact {
        return None;
    }
    let ans = crate::forms::nf::print_answer(&e, &fr.fitted);
    if !fits_each_row(&ans, features, target, train_n) || !well_conditioned(&ans, features, train_n)
    {
        return None;
    }
    let degenerate = crate::forms::search::is_degenerate(
        &e,
        &xf,
        &fr.fitted.inner,
        crate::config::EXIT_DEGENERATE_REL,
    );
    Some((e.nodes(), !degenerate))
}

fn misses_far_beyond_precision(
    model: &str,
    features: &[Vec<f64>],
    target: &[f64],
    n: usize,
) -> bool {
    let Ok(ast) = crate::forms::ast::parse(model) else {
        return true;
    };
    let q: Vec<f64> = QUANTA.with(|q| q.borrow().1.clone());
    if q.len() < n {
        return false;
    }
    let cols: Vec<Vec<f64>> = features.iter().map(|c| c[..n].to_vec()).collect();
    let rows: Vec<usize> = (0..n)
        .filter(|&i| target[i].is_finite() && target[i] != 0.0)
        .collect();
    if rows.is_empty() {
        return false;
    }
    let mut rel_q: Vec<f64> = rows.iter().map(|&i| q[i] / target[i].abs()).collect();
    rel_q.sort_by(|a, b| a.total_cmp(b));
    let med_q = rel_q[rel_q.len() / 2];
    let mut ss = 0.0;
    for &i in &rows {
        let p = ast.eval(&cols, i);
        if !p.is_finite() {
            return true;
        }
        ss += ((target[i] - p) / target[i]).powi(2);
    }
    (ss / rows.len() as f64).sqrt() > crate::config::CLEAN_PRECISION_K * med_q.max(1e-15)
}

fn canonical_core(
    model: &str,
    features: &[Vec<f64>],
    target: &[f64],
    n_cols: usize,
    train_n: usize,
) -> Option<(
    crate::forms::nf::Expr,
    crate::forms::fit::FitResult,
    bool,
    Vec<Vec<f64>>,
)> {
    let e0 = structure_of(model, target)?;
    let lim = crate::forms::nf::Limits::full(n_cols);
    let over = e0.check(&lim).is_err();
    let red_lim = if over {
        crate::forms::nf::Limits {
            max_nodes: 2 * lim.max_nodes,
            max_consts: 2 * lim.max_consts,
            max_terms: 2 * lim.max_terms,
            max_factors: 2 * lim.max_factors,
            max_arg_nodes: 2 * lim.max_arg_nodes,
            ..lim
        }
    } else {
        lim
    };
    if over && e0.check(&red_lim).is_err() {
        return None;
    }
    let cols: Vec<Vec<f64>> = features[..n_cols]
        .iter()
        .map(|c| c[..train_n].to_vec())
        .collect();
    let y: Vec<f64> = target[..train_n].to_vec();
    let table = crate::forms::data::Table {
        names: (1..=n_cols).map(|i| format!("x{}", i)).collect(),
        cols,
        y,
        target_name: "target".to_string(),
    };
    let (fi, hi) = crate::config::fit_split(train_n);
    let xf = crate::forms::fit::gather(&table.cols, &fi);
    let xh = crate::forms::fit::gather(&table.cols, &hi);
    let yf: Vec<f64> = fi.iter().map(|&r| table.y[r]).collect();
    let yh: Vec<f64> = hi.iter().map(|&r| table.y[r]).collect();
    let floor = FLOOR.with(|f| f.get());
    let mut cfg = crate::forms::fit::FitConfig::default();
    cfg.exact_r2 = cfg.exact_r2.min(1.0 - crate::config::NOISE_FLOOR_K * floor);
    cfg.init_1d = over;
    let snap_tol = crate::forms::search::SNAP_TOL.max(floor);
    let stat_tol = |r2: f64| {
        crate::config::SNAP_Z * crate::config::SNAP_Z * (1.0 - r2).max(0.0) / yh.len().max(1) as f64
    };
    let rows_ok = |e: &crate::forms::nf::Expr, fr: &crate::forms::fit::FitResult| {
        let ans = crate::forms::nf::print_answer(e, &fr.fitted);
        fits_each_row(&ans, features, target, train_n)
            && within_rounding(&ans, features, target, train_n)
            && well_conditioned(&ans, features, train_n)
    };
    let quanta: Option<Vec<f64>> = QUANTA.with(|q| {
        let q = q.borrow();
        let qf: Vec<f64> = fi
            .iter()
            .map(|&r| q.1.get(r).copied().unwrap_or(0.0))
            .collect();
        let (lo, hi) = qf.iter().fold((f64::INFINITY, 0.0f64), |(lo, hi), &v| {
            (lo.min(v), hi.max(v))
        });
        (lo > 0.0 && lo.is_finite() && hi > lo && hi <= WEIGHT_RANGE_MAX * lo).then_some(qf)
    });
    let weights_for =
        |e: &crate::forms::nf::Expr, fr: &crate::forms::fit::FitResult| -> Option<Vec<f64>> {
            let qf = quanta.as_ref()?;
            let t = crate::forms::nf::eval_terms(e, &xf, &fr.fitted.inner);
            let rows: Vec<usize> = (0..yf.len())
                .filter(|&r| yf[r].is_finite() && t.iter().all(|c| c[r].is_finite()))
                .collect();
            if rows.is_empty() {
                return None;
            }
            let extra = rows
                .iter()
                .map(|&r| {
                    (yf[r]
                        - fr.fitted.offset
                        - fr.fitted
                            .coef
                            .iter()
                            .zip(&t)
                            .map(|(c, g)| c * g[r])
                            .sum::<f64>())
                    .powi(2)
                        - qf[r] * qf[r] / 12.0
                })
                .sum::<f64>()
                / rows.len() as f64;
            let extra = extra.max(0.0);
            Some(
                qf.iter()
                    .map(|v| 1.0 / (v * v / 12.0 + extra).sqrt())
                    .collect(),
            )
        };
    let dl = |e: &crate::forms::nf::Expr, fr: &crate::forms::fit::FitResult| {
        cached_dl(
            &crate::forms::nf::print_answer(e, &fr.fitted),
            features,
            target,
            train_n,
        )
    };
    let fit = |e: &crate::forms::nf::Expr| -> Option<crate::forms::fit::FitResult> {
        let mut fr = crate::forms::fit::fit_expr(e, &xf, &yf, &xh, &yh, &cfg)?;
        if let Some(w) = weights_for(e, &fr) {
            let w = &w;
            if let Some(fw) = crate::forms::fit::weighted_fit(e, &xf, &yf, w, &fr.fitted.inner) {
                let th = crate::forms::nf::eval_terms(e, &xh, &fw.inner);
                let yhat: Vec<f64> = (0..yh.len())
                    .map(|r| {
                        fw.offset + fw.coef.iter().zip(&th).map(|(c, g)| c * g[r]).sum::<f64>()
                    })
                    .collect();
                let alt = crate::forms::fit::FitResult {
                    r2_holdout: crate::forms::fit::r2(&yh, &yhat),
                    fitted: fw,
                };
                if dl(e, &alt) < dl(e, &fr) {
                    fr = alt;
                }
            }
        }
        let exact = fr.is_exact(&cfg);
        let tol = if exact {
            snap_tol
        } else {
            snap_tol.max(stat_tol(fr.r2_holdout))
        };
        let pre = exact.then(|| fr.clone());
        let choose =
            |opts: &[crate::forms::nf::Fitted]| snap_choice(e, opts, features, target, train_n);
        let chooser: Option<crate::forms::search::SnapChoice> =
            (tol > crate::forms::search::SNAP_TOL).then_some(&choose);
        crate::forms::search::snap_by(e, &mut fr, &xf, &yf, &xh, &yh, tol, true, chooser);
        if let Some(mut alt) = pre {
            if !inner_unchanged(&alt, &fr) && !rows_ok(e, &fr) {
                crate::forms::search::snap_by(e, &mut alt, &xf, &yf, &xh, &yh, tol, false, chooser);
                if rows_ok(e, &alt) {
                    fr = alt;
                }
            }
        }
        Some(fr)
    };
    let fr0 = fit(&e0)?;
    let mut cur = (e0, fr0);
    if over && !cur.1.is_exact(&cfg) {
        return None;
    }
    if cur.1.is_exact(&cfg) {
        let ext = crate::forms::search::Extrap::new(&table);
        let mut changed = false;
        'outer: loop {
            let inner = &cur.1.fitted.inner;
            let cands = crate::forms::repair::reductions(&cur.0, inner, &red_lim)
                .into_iter()
                .chain(crate::forms::repair::factor_drops(&cur.0, inner, &red_lim));
            for c in cands {
                if crate::out_of_budget() {
                    break 'outer;
                }
                let Some(f) = fit(&c) else { continue };
                if f.is_exact(&cfg)
                    && ext.holds(&c, &f, &cfg)
                    && no_worse(dl(&c, &f), dl(&cur.0, &cur.1))
                {
                    cur = (c, f);
                    changed = true;
                    continue 'outer;
                }
            }
            break;
        }
        if !changed && !ext.holds(&cur.0, &cur.1, &cfg) {
            return None;
        }
        if over && cur.0.check(&lim).is_err() {
            return None;
        }
    } else if !over {
        'noisy: loop {
            let inner = &cur.1.fitted.inner;
            let cands = crate::forms::repair::reductions(&cur.0, inner, &red_lim)
                .into_iter()
                .chain(crate::forms::repair::factor_drops(&cur.0, inner, &red_lim));
            for c in cands {
                if crate::out_of_budget() {
                    break 'noisy;
                }
                let Some(f) = fit(&c) else { continue };
                if no_worse(dl(&c, &f), dl(&cur.0, &cur.1)) {
                    cur = (c, f);
                    continue 'noisy;
                }
            }
            break;
        }
    }
    {
        let exact = cur.1.is_exact(&cfg);
        let tol = if exact {
            snap_tol
        } else {
            snap_tol.max(stat_tol(cur.1.r2_holdout))
        };
        if tol > crate::forms::search::SNAP_TOL && !crate::out_of_budget() {
            let e = cur.0.clone();
            let choose = |opts: &[crate::forms::nf::Fitted]| {
                snap_choice(&e, opts, features, target, train_n)
            };
            let mut f = cur.1.clone();
            crate::forms::search::snap_joint(&e, &mut f, &xf, &yf, &xh, &yh, tol, exact, &choose);
            if !exact || f.is_exact(&cfg) {
                cur.1 = f;
            }
        }
    }
    let exact = cur.1.is_exact(&cfg);
    Some((cur.0, cur.1, exact, xf))
}

fn no_worse(d: f64, d0: f64) -> bool {
    d.is_finite() && d0.is_finite() && d <= d0 + DL_BAND
}

fn inner_unchanged(a: &crate::forms::fit::FitResult, b: &crate::forms::fit::FitResult) -> bool {
    a.fitted.inner == b.fitted.inner
}

pub(crate) fn structural_constant(v: f64) -> bool {
    if v == 0.0 {
        return true;
    }
    if !v.is_finite() {
        return false;
    }
    let m = crate::config::STRUCT_RATIONAL_MAX;
    for q in 1..=m as i64 {
        let p = (v * q as f64).round();
        if p != 0.0 && p.abs() <= m && (p / q as f64 - v).abs() <= 1e-12 * v.abs() {
            return true;
        }
    }
    crate::forms::nf::symbolic_constant(v).is_some()
        || crate::forms::nf::physical_constant(v).is_some()
}

pub(crate) fn free_constants(model: &str) -> Option<usize> {
    let (_, f) =
        crate::forms::canon::normalize(&tame_scales(crate::forms::ast::parse(model).ok()?)).ok()?;
    let free = |v: &f64| !structural_constant(*v);
    Some(
        f.coef.iter().filter(|v| free(v)).count()
            + usize::from(free(&f.offset))
            + f.inner.iter().filter(|v| free(v)).count(),
    )
}

pub(crate) fn all_constants(model: &str) -> Option<usize> {
    let (_, f) =
        crate::forms::canon::normalize(&tame_scales(crate::forms::ast::parse(model).ok()?)).ok()?;
    Some(
        f.coef.iter().filter(|v| **v != 1.0).count() + usize::from(f.offset != 0.0) + f.inner.len(),
    )
}

pub(crate) fn unnamed_constants(model: &str) -> Option<usize> {
    let (_, f) =
        crate::forms::canon::normalize(&tame_scales(crate::forms::ast::parse(model).ok()?)).ok()?;
    let named = |v: f64| {
        crate::forms::nf::symbolic_constant(v).is_some()
            || crate::forms::nf::physical_constant(v).is_some()
    };
    Some(
        f.coef.iter().filter(|v| **v != 1.0 && !named(**v)).count()
            + usize::from(f.offset != 0.0 && !named(f.offset))
            + f.inner.iter().filter(|v| !named(**v)).count(),
    )
}

pub(crate) fn loo_refit_r2(
    model: &str,
    features: &[Vec<f64>],
    target: &[f64],
    n_cols: usize,
    n: usize,
) -> Option<f64> {
    let (e, f0) =
        crate::forms::canon::normalize(&tame_scales(crate::forms::ast::parse(model).ok()?)).ok()?;
    let cfg = crate::forms::fit::FitConfig::default();
    let cols: Vec<Vec<f64>> = features[..n_cols].iter().map(|c| c[..n].to_vec()).collect();
    let mut press = 0.0f64;
    let mut ys = Vec::new();
    for i in 0..n {
        let rest: Vec<usize> = (0..n).filter(|&k| k != i).collect();
        let xf = crate::forms::fit::gather(&cols, &rest);
        let yf: Vec<f64> = rest.iter().map(|&k| target[k]).collect();
        let xi = crate::forms::fit::gather(&cols, &[i]);
        let Some(fr) = crate::forms::fit::refit_from(&e, &f0.inner, &xf, &yf, &xf, &yf, &cfg)
        else {
            continue;
        };
        let t = crate::forms::nf::eval_terms(&e, &xi, &fr.fitted.inner);
        let pred = fr.fitted.offset
            + fr.fitted
                .coef
                .iter()
                .zip(&t)
                .map(|(c, g)| c * g[0])
                .sum::<f64>();
        if pred.is_finite() && target[i].is_finite() {
            press += (target[i] - pred).powi(2);
            ys.push(target[i]);
        }
    }
    if ys.len() < 3 {
        return None;
    }
    let m = ys.iter().sum::<f64>() / ys.len() as f64;
    let sst: f64 = ys.iter().map(|v| (v - m).powi(2)).sum();
    (sst > 0.0).then(|| 1.0 - press / sst)
}

const TAMED_SCALE: f64 = 1.2345678901;

fn tame_scales(a: crate::forms::ast::Ast) -> crate::forms::ast::Ast {
    use crate::forms::ast::Ast;
    fn factor(a: Ast) -> Ast {
        match a {
            Ast::Num(v) if v.is_finite() && crate::forms::nf::physical_constant(v).is_some() => {
                Ast::Num(std::f64::consts::PI.copysign(v))
            }
            Ast::Num(v) if v != 0.0 && v.is_finite() && (v.abs() < 1e-6 || v.abs() > 1e6) => {
                Ast::Num(TAMED_SCALE.copysign(v))
            }
            Ast::Mul(x, y) => Ast::Mul(Box::new(factor(*x)), Box::new(factor(*y))),
            Ast::Div(x, y) => Ast::Div(Box::new(factor(*x)), y),
            Ast::Neg(x) => Ast::Neg(Box::new(factor(*x))),
            other => other,
        }
    }
    match a {
        Ast::Add(x, y) => Ast::Add(Box::new(tame_scales(*x)), Box::new(tame_scales(*y))),
        Ast::Sub(x, y) => Ast::Sub(Box::new(tame_scales(*x)), Box::new(tame_scales(*y))),
        Ast::Neg(x) => Ast::Neg(Box::new(tame_scales(*x))),
        other => factor(other),
    }
}

pub(crate) fn loo_rows_needed(model: &str) -> Option<usize> {
    let (_, f) =
        crate::forms::canon::normalize(&tame_scales(crate::forms::ast::parse(model).ok()?)).ok()?;
    Some(crate::config::fit_min_rows(f.coef.len() + 1 + f.inner.len()) + 1)
}

pub(crate) fn canonical(model: &str, target: &[f64]) -> Option<String> {
    structure_of(model, target).map(|e| crate::forms::nf::print_structural(&e))
}

fn structure_of(model: &str, _target: &[f64]) -> Option<crate::forms::nf::Expr> {
    use crate::forms::ast::Ast;
    fn tame(a: Ast) -> Ast {
        match a {
            Ast::Num(v) if v != 0.0 && v.is_finite() && (v.abs() < 1e-6 || v.abs() > 1e6) => {
                Ast::Num(2.0f64.copysign(v))
            }
            Ast::Add(x, y) => Ast::Add(Box::new(tame(*x)), Box::new(tame(*y))),
            Ast::Sub(x, y) => Ast::Sub(Box::new(tame(*x)), Box::new(tame(*y))),
            Ast::Mul(x, y) => Ast::Mul(Box::new(tame(*x)), Box::new(tame(*y))),
            Ast::Div(x, y) => Ast::Div(Box::new(tame(*x)), Box::new(tame(*y))),
            Ast::Pow(x, y) => Ast::Pow(Box::new(tame(*x)), y),
            Ast::Neg(x) => Ast::Neg(Box::new(tame(*x))),
            Ast::Un(u, x) => Ast::Un(u, Box::new(tame(*x))),
            Ast::Sqrt(x) => Ast::Sqrt(Box::new(tame(*x))),
            other => other,
        }
    }
    let ast = tame(crate::forms::ast::parse(model).ok()?);
    crate::forms::canon::normalize(&ast).ok().map(|(e, _)| e)
}

thread_local! {
    static QUANTA : std::cell::RefCell < (Vec < Vec < f64 >>, Vec < f64 >) > = const {
    std::cell::RefCell::new((Vec::new(), Vec::new())) };
}

pub(crate) fn set_quanta(q_features: &[Vec<f64>], q_target: &[f64]) {
    QUANTA.with(|q| *q.borrow_mut() = (q_features.to_vec(), q_target.to_vec()));
    clear_dl_cache();
}

pub(crate) fn fits_each_row(model: &str, features: &[Vec<f64>], target: &[f64], n: usize) -> bool {
    use crate::config::{NOISE_FLOOR_K, REL_EXACT_EPS, REL_SPAN};
    let rows: Vec<usize> = (0..n).filter(|&i| target[i].is_finite()).collect();
    if rows.len() < 2 {
        return true;
    }
    let pos = rows.iter().all(|&i| target[i] > 0.0);
    let neg = rows.iter().all(|&i| target[i] < 0.0);
    if !(pos || neg) {
        return true;
    }
    let lo = rows
        .iter()
        .map(|&i| target[i].abs())
        .fold(f64::INFINITY, f64::min);
    let hi = rows.iter().map(|&i| target[i].abs()).fold(0.0, f64::max);
    if !(hi >= REL_SPAN * lo) {
        return true;
    }
    let Ok(ast) = crate::forms::ast::parse(model) else {
        return false;
    };
    let cols: Vec<Vec<f64>> = features.iter().map(|c| c[..n].to_vec()).collect();
    let var = QUANTA.with(|q| {
        let q = q.borrow();
        let mut ev = crate::Evaluator::new(2, n);
        crate::math::rounding_var_rows(model, features, &q.0, &q.1, n, &mut ev)
    });
    let mut s = 0.0f64;
    for &i in &rows {
        let p = ast.eval(&cols, i);
        if !p.is_finite() {
            return false;
        }
        let e = var.as_ref().map_or(0.0, |v| v[i].max(0.0));
        let tol = NOISE_FLOOR_K * e + (REL_EXACT_EPS * target[i]).powi(2);
        s += (target[i] - p).powi(2) / tol;
    }
    s / rows.len() as f64 <= 1.0
}

pub(crate) fn well_conditioned(model: &str, features: &[Vec<f64>], n: usize) -> bool {
    conditioning_fault(model, features, n).is_none()
}

pub(crate) fn conditioning_fault(
    model: &str,
    features: &[Vec<f64>],
    n: usize,
) -> Option<&'static str> {
    use crate::config::{COND_CANCEL_MAX, COND_NEGLIGIBLE};
    use crate::forms::ast::Ast;
    let Ok(ast) = crate::forms::ast::parse(model) else {
        return None;
    };
    let cols: Vec<Vec<f64>> = features.iter().map(|c| c[..n].to_vec()).collect();
    fn sums<'a>(a: &'a Ast, out: &mut Vec<(&'a Ast, &'a Ast, bool)>) {
        match a {
            Ast::Add(x, y) | Ast::Sub(x, y) => {
                out.push((x, y, matches!(a, Ast::Sub(..))));
                sums(x, out);
                sums(y, out);
            }
            Ast::Mul(x, y) | Ast::Div(x, y) | Ast::Pow(x, y) => {
                sums(x, out);
                sums(y, out);
            }
            Ast::Neg(x) | Ast::Un(_, x) | Ast::Sqrt(x) => sums(x, out),
            Ast::Num(_) | Ast::Var(_) => {}
        }
    }
    if let Ok((e, f)) = crate::forms::canon::normalize(&ast) {
        if crate::forms::search::is_degenerate(
            &e,
            &cols,
            &f.inner,
            crate::forms::search::DEGENERATE_REL,
        ) {
            return Some(
                "a function in it has an argument that barely varies over the rows, where it stands in for a simpler form",
            );
        }
    }
    let mut pairs = Vec::new();
    sums(&ast, &mut pairs);
    for (x, y, sub) in pairs {
        let (mut sa, mut sb, mut sv) = (0.0f64, 0.0f64, 0.0f64);
        for i in 0..n {
            let (a, b) = (x.eval(&cols, i), y.eval(&cols, i));
            if !(a.is_finite() && b.is_finite()) {
                continue;
            }
            sa += a.abs();
            sb += b.abs();
            sv += if sub { (a - b).abs() } else { (a + b).abs() };
        }
        if sa == 0.0 || sb == 0.0 || !(sv > 0.0) {
            continue;
        }
        let negligible = COND_NEGLIGIBLE.max((1.0 - crate::config::EXACT_EXIT_R2).sqrt());
        if sa.min(sb) < negligible * sv {
            return Some("a term in it is negligible against its sum");
        }
        if sa + sb > COND_CANCEL_MAX * sv {
            return Some("terms in it cancel to a small part of their size");
        }
    }
    None
}

pub(crate) fn within_rounding(
    model: &str,
    features: &[Vec<f64>],
    target: &[f64],
    n: usize,
) -> bool {
    use crate::config::{ROUND_BOUND_FLOOR, ROUND_BOUND_SLACK};
    let rows: Vec<usize> = (0..n).filter(|&i| target[i].is_finite()).collect();
    if rows.len() < 2 {
        return true;
    }
    let m = rows.iter().map(|&i| target[i]).sum::<f64>() / rows.len() as f64;
    let sd =
        (rows.iter().map(|&i| (target[i] - m).powi(2)).sum::<f64>() / rows.len() as f64).sqrt();
    let Ok(ast) = crate::forms::ast::parse(model) else {
        return false;
    };
    let cols: Vec<Vec<f64>> = features.iter().map(|c| c[..n].to_vec()).collect();
    let bound = QUANTA.with(|q| {
        let q = q.borrow();
        let mut ev = crate::Evaluator::new(2, n);
        crate::math::rounding_bound_rows(model, features, &q.0, &q.1, n, &mut ev)
    });
    for &i in &rows {
        let p = ast.eval(&cols, i);
        if !p.is_finite() {
            return false;
        }
        let b = bound.as_ref().map_or(0.0, |v| v[i].max(0.0));
        if (target[i] - p).abs() > ROUND_BOUND_SLACK * b + ROUND_BOUND_FLOOR * sd {
            return false;
        }
    }
    true
}

fn saturated_nodes(ast: &crate::forms::ast::Ast, cols: &[Vec<f64>], n: usize) -> Vec<(usize, f64)> {
    use crate::forms::ast::Ast;
    fn walk(a: &Ast, cols: &[Vec<f64>], n: usize, idx: &mut usize, out: &mut Vec<(usize, f64)>) {
        let me = *idx;
        *idx += 1;
        if matches!(a, Ast::Un(..) | Ast::Sqrt(..)) {
            let v: Vec<f64> = (0..n)
                .map(|i| a.eval(cols, i))
                .filter(|x| x.is_finite())
                .collect();
            if v.len() >= 2 {
                let m = v.iter().sum::<f64>() / v.len() as f64;
                let lo = v.iter().cloned().fold(f64::INFINITY, f64::min);
                let hi = v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                if m.abs() > 0.0 && (hi - lo) < crate::config::SATURATION_REL * m.abs() {
                    out.push((me, m));
                    return;
                }
            }
        }
        match a {
            Ast::Add(x, y) | Ast::Sub(x, y) | Ast::Mul(x, y) | Ast::Div(x, y) | Ast::Pow(x, y) => {
                walk(x, cols, n, idx, out);
                walk(y, cols, n, idx, out);
            }
            Ast::Neg(x) | Ast::Un(_, x) | Ast::Sqrt(x) => walk(x, cols, n, idx, out),
            Ast::Num(_) | Ast::Var(_) => {}
        }
    }
    let mut out = Vec::new();
    walk(ast, cols, n, &mut 0, &mut out);
    out
}

pub(crate) fn has_saturated(model: &str, features: &[Vec<f64>], n: usize) -> bool {
    let Ok(ast) = crate::forms::ast::parse(model) else {
        return false;
    };
    let cols: Vec<Vec<f64>> = features.iter().map(|c| c[..n].to_vec()).collect();
    !saturated_nodes(&ast, &cols, n).is_empty()
}

pub(crate) fn padding_removable(
    model: &str,
    features: &[Vec<f64>],
    target: &[f64],
    n: usize,
    tol: f64,
) -> bool {
    use crate::forms::ast::Ast;
    let Ok(ast) = crate::forms::ast::parse(model) else {
        return false;
    };
    let cols: Vec<Vec<f64>> = features.iter().map(|c| c[..n].to_vec()).collect();
    let sat = saturated_nodes(&ast, &cols, n);
    if sat.is_empty() {
        return false;
    }
    let r2_of = |a: &Ast| -> f64 {
        let rows: Vec<usize> = (0..n).filter(|&i| target[i].is_finite()).collect();
        let m = rows.iter().map(|&i| target[i]).sum::<f64>() / rows.len().max(1) as f64;
        let (mut res, mut tot) = (0.0f64, 0.0f64);
        for &i in &rows {
            let p = a.eval(&cols, i);
            if !p.is_finite() {
                return f64::NEG_INFINITY;
            }
            res += (target[i] - p).powi(2);
            tot += (target[i] - m).powi(2);
        }
        if tot > 0.0 {
            1.0 - res / tot
        } else {
            f64::NEG_INFINITY
        }
    };
    fn replace(a: &Ast, at: usize, v: f64, idx: &mut usize) -> Ast {
        let me = *idx;
        *idx += 1;
        if me == at {
            return Ast::Num(v);
        }
        match a {
            Ast::Add(x, y) => Ast::Add(
                Box::new(replace(x, at, v, idx)),
                Box::new(replace(y, at, v, idx)),
            ),
            Ast::Sub(x, y) => Ast::Sub(
                Box::new(replace(x, at, v, idx)),
                Box::new(replace(y, at, v, idx)),
            ),
            Ast::Mul(x, y) => Ast::Mul(
                Box::new(replace(x, at, v, idx)),
                Box::new(replace(y, at, v, idx)),
            ),
            Ast::Div(x, y) => Ast::Div(
                Box::new(replace(x, at, v, idx)),
                Box::new(replace(y, at, v, idx)),
            ),
            Ast::Pow(x, y) => Ast::Pow(
                Box::new(replace(x, at, v, idx)),
                Box::new(replace(y, at, v, idx)),
            ),
            Ast::Neg(x) => Ast::Neg(Box::new(replace(x, at, v, idx))),
            Ast::Un(u, x) => Ast::Un(*u, Box::new(replace(x, at, v, idx))),
            Ast::Sqrt(x) => Ast::Sqrt(Box::new(replace(x, at, v, idx))),
            other => other.clone(),
        }
    }
    let base = r2_of(&ast);
    sat.iter()
        .any(|&(at, v)| r2_of(&replace(&ast, at, v, &mut 0)) >= base - tol)
}

pub(crate) fn practical_dl(
    model: &str,
    nodes: usize,
    features: &[Vec<f64>],
    target: &[f64],
    n: usize,
) -> Option<f64> {
    let ast = crate::forms::ast::parse(model).ok()?;
    let cols: Vec<Vec<f64>> = features.iter().map(|c| c[..n].to_vec()).collect();
    let rows: Vec<usize> = (0..n).filter(|&i| target[i].is_finite()).collect();
    if rows.len() < 2 {
        return None;
    }
    let one_sign = rows.iter().all(|&i| target[i] > 0.0) || rows.iter().all(|&i| target[i] < 0.0);
    let mean = rows.iter().map(|&i| target[i]).sum::<f64>() / rows.len() as f64;
    let spread = (rows
        .iter()
        .map(|&i| (target[i] - mean).powi(2))
        .sum::<f64>()
        / rows.len() as f64)
        .sqrt();
    let mut ss = 0.0;
    let mut k = 0usize;
    for &i in &rows {
        let p = ast.eval(&cols, i);
        if !p.is_finite() {
            continue;
        }
        let s = if one_sign { target[i].abs() } else { spread };
        ss += ((target[i] - p) / s).powi(2);
        k += 1;
    }
    if k * 10 < rows.len() * 9 || !(spread > 0.0) {
        return None;
    }
    let m2 = (ss / k as f64).max(crate::config::RANK_NOISE_FLOOR.powi(2));
    let consts = all_constants(model).unwrap_or(0);
    let nf = k as f64;
    Some(nf / 2.0 * m2.ln() + nf.ln() / 2.0 * (nodes + consts) as f64)
}

pub(crate) fn badly_missed(
    model: &str,
    features: &[Vec<f64>],
    target: &[f64],
    q_target: &[f64],
    n: usize,
) -> Option<(usize, usize)> {
    let ast = crate::forms::ast::parse(model).ok()?;
    let cols: Vec<Vec<f64>> = features.iter().map(|c| c[..n].to_vec()).collect();
    let ys: Vec<usize> = (0..n).filter(|&i| target[i].is_finite()).collect();
    if ys.is_empty()
        || !(ys.iter().all(|&i| target[i] > 0.0) || ys.iter().all(|&i| target[i] < 0.0))
    {
        return None;
    }
    let mut bad = 0;
    for &i in &ys {
        let (y, p) = (target[i], ast.eval(&cols, i));
        let q = q_target.get(i).copied().unwrap_or(0.0);
        if !p.is_finite()
            || ((y - p).abs() > crate::config::LAW_BAD_MISS * y.abs() && (y - p).abs() > q)
        {
            bad += 1;
        }
    }
    Some((
        bad,
        ((crate::config::LAW_BAD_ROWS * ys.len() as f64).floor() as usize).max(1),
    ))
}

pub(crate) fn largest_miss(
    model: &str,
    features: &[Vec<f64>],
    target: &[f64],
    n: usize,
) -> Option<(f64, f64)> {
    let ast = crate::forms::ast::parse(model).ok()?;
    let cols: Vec<Vec<f64>> = features.iter().map(|c| c[..n].to_vec()).collect();
    let mut best: Option<(f64, f64)> = None;
    for i in 0..n {
        let y = target[i];
        if !y.is_finite() {
            continue;
        }
        if y == 0.0 {
            return None;
        }
        let p = ast.eval(&cols, i);
        if !p.is_finite() {
            return None;
        }
        let r = ((y - p) / y).abs();
        if best.is_none_or(|(b, _)| r > b) {
            best = Some((r, y));
        }
    }
    best
}

pub(crate) const DL_BAND: f64 = 0.5;

const DL_ROWS: usize = 256;

pub(crate) fn description_length(
    model: &str,
    features: &[Vec<f64>],
    target: &[f64],
    n: usize,
) -> Option<f64> {
    let parsed = crate::Parser::parse(model).ok()?;
    let nodes = parsed.node_count() as f64;
    let free = free_constants(model).unwrap_or_else(|| all_constants(model).unwrap_or(0)) as f64;
    let ast = crate::forms::ast::parse(model).ok()?;
    let rows: Vec<usize> = (0..n).filter(|&i| target[i].is_finite()).collect();
    let rows: Vec<usize> = if rows.len() > DL_ROWS {
        (0..DL_ROWS)
            .map(|k| rows[k * rows.len() / DL_ROWS])
            .collect()
    } else {
        rows
    };
    let m = rows.len();
    if m < 2 {
        return None;
    }
    let cols: Vec<Vec<f64>> = features
        .iter()
        .map(|c| rows.iter().map(|&i| c[i]).collect())
        .collect();
    let y: Vec<f64> = rows.iter().map(|&i| target[i]).collect();
    let p: Vec<f64> = (0..m).map(|k| ast.eval(&cols, k)).collect();
    let undefined = p.iter().filter(|v| !v.is_finite()).count() as f64 * CELL_NLL_CAP;
    let defined: Vec<usize> = (0..m).filter(|&k| p[k].is_finite()).collect();
    if defined.len() < 2 {
        return None;
    }
    let (qy, u) = QUANTA.with(|q| {
        let q = q.borrow();
        let qy: Vec<f64> = rows
            .iter()
            .map(|&i| q.1.get(i).copied().unwrap_or(0.0))
            .collect();
        let qf: Vec<Vec<f64>> =
            q.0.iter()
                .map(|c| {
                    rows.iter()
                        .map(|&i| c.get(i).copied().unwrap_or(0.0))
                        .collect()
                })
                .collect();
        let mut ev = crate::Evaluator::new(2, m);
        let u: Vec<f64> = crate::math::rounding_var_rows(model, &cols, &qf, &qy, m, &mut ev)
            .map(|v| {
                v.iter()
                    .zip(&qy)
                    .map(|(v, q)| (v - q * q / 12.0).max(0.0))
                    .collect()
            })
            .unwrap_or_else(|| vec![0.0; m]);
        (qy, u)
    });
    let mean = y.iter().sum::<f64>() / m as f64;
    let sd = (y.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / m as f64).sqrt();
    let tau2: Vec<f64> = (0..m)
        .map(|k| (1e-13 * y[k].abs().max(p[k].abs())).powi(2))
        .collect();
    let rows_cost = |readings: bool, s2: f64, relative: bool| -> f64 {
        undefined
            + defined
                .iter()
                .map(|&k| {
                    let extra = if relative { s2 * p[k] * p[k] } else { s2 };
                    let var = if readings { u[k] } else { 0.0 } + extra + tau2[k];
                    cell_nll(y[k] - p[k], qy[k], var.sqrt())
                })
                .sum::<f64>()
    };
    let half_ln = 0.5 * (m as f64).ln();
    let readings_differ = u.iter().any(|v| *v > 0.0);
    let nd = defined.len() as f64;
    let mut best = f64::INFINITY;
    for readings in [false, true] {
        if readings && !readings_differ {
            continue;
        }
        best = best.min(rows_cost(readings, 0.0, false));
        let allowed = |k: usize| (if readings { u[k] } else { 0.0 } + qy[k] * qy[k] / 12.0);
        let additive = defined
            .iter()
            .map(|&k| (y[k] - p[k]).powi(2) - allowed(k))
            .sum::<f64>()
            / nd;
        let relative = defined
            .iter()
            .filter(|&&k| p[k] != 0.0)
            .map(|&k| ((y[k] - p[k]).powi(2) - allowed(k)) / (p[k] * p[k]))
            .sum::<f64>()
            / nd;
        if sd > 0.0 {
            let mean_allowed = defined.iter().map(|&k| allowed(k)).sum::<f64>() / nd;
            for (s2m, rel) in [(additive, false), (relative, true)] {
                let scale = if rel {
                    defined
                        .iter()
                        .filter(|&&k| p[k] != 0.0)
                        .map(|&k| allowed(k) / (p[k] * p[k]))
                        .sum::<f64>()
                        / nd
                } else {
                    mean_allowed
                };
                let top = if s2m.is_finite() {
                    s2m.max(scale)
                } else {
                    scale
                };
                if !(top > 0.0 && top.is_finite()) {
                    continue;
                }
                let cost = |ln_s2: f64| rows_cost(readings, ln_s2.exp(), rel);
                let decade = std::f64::consts::LN_10;
                let (mut at, mut lowest) = (top.ln(), f64::INFINITY);
                if s2m > NOISE_GAUSSIAN * scale {
                    lowest = cost(at);
                } else {
                    for j in 0..=NOISE_GRID_DECADES {
                        let x = top.ln() - decade * j as f64;
                        let c = cost(x);
                        if c < lowest {
                            lowest = c;
                            at = x;
                        }
                    }
                }
                let (mut a, mut b) = (at - decade, at + decade);
                let g = 0.5 * (5f64.sqrt() - 1.0);
                let (mut x1, mut x2) = (b - g * (b - a), a + g * (b - a));
                let (mut f1, mut f2) = (cost(x1), cost(x2));
                for _ in 0..NOISE_GOLDEN_STEPS {
                    if f1 < f2 {
                        b = x2;
                        x2 = x1;
                        f2 = f1;
                        x1 = b - g * (b - a);
                        f1 = cost(x1);
                    } else {
                        a = x1;
                        x1 = x2;
                        f1 = f2;
                        x2 = a + g * (b - a);
                        f2 = cost(x2);
                    }
                }
                best = best.min(lowest.min(f1).min(f2) + half_ln);
            }
        }
    }
    Some(best + half_ln * (nodes + free))
}

thread_local! {
    static DL_CACHE : std::cell::RefCell < std::collections::HashMap < String, f64 >> =
    std::cell::RefCell::new(std::collections::HashMap::new());
}

fn clear_dl_cache() {
    DL_CACHE.with(|c| c.borrow_mut().clear());
}

pub(crate) fn cached_dl(model: &str, features: &[Vec<f64>], target: &[f64], n: usize) -> f64 {
    let key = format!("{}|{}", n, model);
    if let Some(v) = DL_CACHE.with(|c| c.borrow().get(&key).copied()) {
        return v;
    }
    let v = description_length(model, features, target, n).unwrap_or(f64::INFINITY);
    DL_CACHE.with(|c| c.borrow_mut().insert(key, v));
    v
}

const CELL_NLL_CAP: f64 = 700.0;

const SINGLED_OUT_RIVALS: usize = 2;

const WEIGHT_RANGE_MAX: f64 = 1e6;

const NOISE_GRID_DECADES: usize = 8;

const NOISE_GOLDEN_STEPS: usize = 6;

const NOISE_GAUSSIAN: f64 = 10.0;

fn cell_nll(r: f64, q: f64, sigma: f64) -> f64 {
    const CAP: f64 = CELL_NLL_CAP;
    if !(sigma > 0.0) {
        return if r.abs() <= q / 2.0 { 0.0 } else { CAP };
    }
    let z = r.abs() / sigma;
    let ln_sqrt_2pi = 0.5 * (2.0 * std::f64::consts::PI).ln();
    if q < 1e-3 * sigma {
        let width = if q > 0.0 {
            (sigma / q).ln()
        } else {
            sigma.ln()
        };
        return (0.5 * z * z + ln_sqrt_2pi + width).min(CAP);
    }
    let (a, b) = ((r.abs() - q / 2.0) / sigma, (r.abs() + q / 2.0) / sigma);
    let pr = upper_tail(a) - upper_tail(b);
    if pr > 1e-300 && pr.is_finite() {
        return (-pr.ln()).min(CAP);
    }
    (0.5 * a * a + ln_sqrt_2pi + (sigma / q).ln().max(0.0)).min(CAP)
}

fn upper_tail(x: f64) -> f64 {
    let z = (x / std::f64::consts::SQRT_2).abs();
    let t = 1.0 / (1.0 + 0.5 * z);
    let erfc = t
        * (-z * z - 1.26551223
            + t * (1.00002368
                + t * (0.37409196
                    + t * (0.09678418
                        + t * (-0.18628806
                            + t * (0.27886807
                                + t * (-1.13520398
                                    + t * (1.48851587 + t * (-0.82215223 + t * 0.17087277)))))))))
            .exp();
    let q = 0.5 * erfc;
    if x >= 0.0 {
        q
    } else {
        1.0 - q
    }
}

pub(crate) fn snap_choice(
    e: &crate::forms::nf::Expr,
    opts: &[crate::forms::nf::Fitted],
    features: &[Vec<f64>],
    target: &[f64],
    n: usize,
) -> usize {
    let dls: Vec<f64> = opts
        .iter()
        .map(|f| cached_dl(&crate::forms::nf::print_answer(e, f), features, target, n))
        .collect();
    let min = dls.iter().cloned().fold(f64::INFINITY, f64::min);
    if !min.is_finite() {
        return 0;
    }
    let free_memo: Vec<std::cell::Cell<Option<usize>>> =
        vec![std::cell::Cell::new(None); opts.len()];
    let free = |k: usize| -> usize {
        free_memo[k].get().unwrap_or_else(|| {
            let v =
                free_constants(&crate::forms::nf::print_answer(e, &opts[k])).unwrap_or(usize::MAX);
            free_memo[k].set(Some(v));
            v
        })
    };
    let rows = (0..n.min(target.len()))
        .filter(|&i| target[i].is_finite())
        .count()
        .min(DL_ROWS)
        .max(2);
    let half_ln = 0.5 * (rows as f64).ln();
    let cost = |k: usize| -> usize {
        let f = &opts[k];
        f.coef
            .iter()
            .filter(|v| **v != 1.0)
            .map(|v| crate::forms::search::written_cost(*v))
            .sum::<usize>()
            + if f.offset != 0.0 {
                crate::forms::search::written_cost(f.offset)
            } else {
                0
            }
            + f.inner
                .iter()
                .map(|v| crate::forms::search::written_cost(*v))
                .sum::<usize>()
    };
    let written_count = |k: usize| {
        opts[k].coef.iter().filter(|v| **v != 1.0).count()
            + usize::from(opts[k].offset != 0.0)
            + opts[k].inner.len()
    };
    let data = |k: usize| {
        if free(k) == usize::MAX {
            f64::INFINITY
        } else {
            dls[k] + half_ln * (written_count(k) as f64 - free(k) as f64)
        }
    };
    let credited = |k: usize| free(k) < written_count(k);
    let same = |a: f64, b: f64| (a - b).abs() <= 1e-9 * a.abs().max(b.abs());
    let differs = |a: &crate::forms::nf::Fitted, k: &crate::forms::nf::Fitted| {
        let st = |v: f64| structural_constant(v);
        (k.offset != 0.0 && st(k.offset) && !same(a.offset, k.offset))
            || a.coef
                .iter()
                .zip(&k.coef)
                .any(|(x, y)| *y != 1.0 && st(*y) && !same(*x, *y))
            || a.inner
                .iter()
                .zip(&k.inner)
                .any(|(x, y)| st(*y) && !same(*x, *y))
    };
    let near_rival = |k: usize, dk: f64| -> bool {
        let f = &opts[k];
        let slots = f.coef.len() + 1 + f.inner.len();
        (0..slots).any(|s| {
            let v = if s < f.coef.len() {
                f.coef[s]
            } else if s == f.coef.len() {
                f.offset
            } else {
                f.inner[s - f.coef.len() - 1]
            };
            if v == 0.0 || v.abs() == 1.0 || !structural_constant(v) {
                return false;
            }
            let w = crate::forms::search::written_cost(v);
            let mut vals: Vec<f64> = crate::forms::search::nice_near_costed(v)
                .into_iter()
                .filter(|&(c, r)| c <= w && r != 0.0 && !same(r, v))
                .map(|(_, r)| r)
                .collect();
            vals.sort_by(|a, b| (a - v).abs().total_cmp(&(b - v).abs()));
            vals.dedup();
            vals.into_iter().take(SINGLED_OUT_RIVALS).any(|r| {
                let mut g = f.clone();
                if s < g.coef.len() {
                    g.coef[s] = r;
                } else if s == g.coef.len() {
                    g.offset = r;
                } else {
                    g.inner[s - f.coef.len() - 1] = r;
                }
                let ans = crate::forms::nf::print_answer(e, &g);
                let d = cached_dl(&ans, features, target, n);
                let Some(fr) = free_constants(&ans) else {
                    return false;
                };
                d.is_finite() && d + half_ln * (written_count(k) as f64 - fr as f64) <= dk + DL_BAND
            })
        })
    };
    let singled_out = |k: usize| {
        if k == 0 || !credited(k) {
            return true;
        }
        let (dk, ck) = (data(k), cost(k));
        !((0..opts.len()).any(|j| {
            j != k
                && dls[j] <= dk + DL_BAND
                && differs(&opts[j], &opts[k])
                && data(j) <= dk + DL_BAND
                && cost(j) <= ck
        }) || near_rival(k, dk))
    };
    let mut order: Vec<usize> = (0..opts.len()).filter(|&k| dls[k].is_finite()).collect();
    order.sort_by(|&a, &b| dls[a].total_cmp(&dls[b]).then(a.cmp(&b)));
    let Some(min) = order.iter().find(|&&k| singled_out(k)).map(|&k| dls[k]) else {
        return 0;
    };
    order
        .into_iter()
        .take_while(|&k| dls[k] <= min + DL_BAND)
        .filter(|&k| singled_out(k))
        .min_by_key(|&k| (free(k), usize::from(k == 0), k))
        .unwrap_or(0)
}
