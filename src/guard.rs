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
    let (e, fr, _, _) = canonical_core(model, features, target, n_cols, train_n)?;
    Some((
        crate::forms::nf::print_structural(&e),
        crate::forms::nf::print_answer(&e, &fr.fitted),
    ))
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
    let fit = |e: &crate::forms::nf::Expr| -> Option<crate::forms::fit::FitResult> {
        let mut fr = crate::forms::fit::fit_expr(e, &xf, &yf, &xh, &yh, &cfg)?;
        let tol = if fr.is_exact(&cfg) {
            snap_tol
        } else {
            snap_tol.max(stat_tol(fr.r2_holdout))
        };
        crate::forms::search::snap(e, &mut fr, &xf, &yf, &xh, &yh, tol);
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
                if crate::out_of_time() {
                    break 'outer;
                }
                let Some(f) = fit(&c) else { continue };
                if f.is_exact(&cfg) && ext.holds(&c, &f, &cfg) {
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
        let base = cur.1.r2_holdout;
        'noisy: loop {
            let inner = &cur.1.fitted.inner;
            let cands = crate::forms::repair::reductions(&cur.0, inner, &red_lim)
                .into_iter()
                .chain(crate::forms::repair::factor_drops(&cur.0, inner, &red_lim));
            for c in cands {
                if crate::out_of_time() {
                    break 'noisy;
                }
                let Some(f) = fit(&c) else { continue };
                if f.r2_holdout >= base - stat_tol(base) {
                    cur = (c, f);
                    continue 'noisy;
                }
            }
            break;
        }
    }
    let exact = cur.1.is_exact(&cfg);
    Some((cur.0, cur.1, exact, xf))
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
}

pub(crate) fn free_constants(model: &str) -> Option<usize> {
    let (_, f) = crate::forms::canon::normalize(&crate::forms::ast::parse(model).ok()?).ok()?;
    let free = |v: &f64| !structural_constant(*v);
    Some(
        f.coef.iter().filter(|v| free(v)).count()
            + usize::from(free(&f.offset))
            + f.inner.iter().filter(|v| free(v)).count(),
    )
}

pub(crate) fn all_constants(model: &str) -> Option<usize> {
    let (_, f) = crate::forms::canon::normalize(&crate::forms::ast::parse(model).ok()?).ok()?;
    Some(
        f.coef.iter().filter(|v| **v != 1.0).count() + usize::from(f.offset != 0.0) + f.inner.len(),
    )
}

pub(crate) fn loo_refit_predictions(
    model: &str,
    features: &[Vec<f64>],
    target: &[f64],
    n_cols: usize,
    n: usize,
) -> Option<Vec<Option<f64>>> {
    let (e, f0) = crate::forms::canon::normalize(&crate::forms::ast::parse(model).ok()?).ok()?;
    let cfg = crate::forms::fit::FitConfig::default();
    let cols: Vec<Vec<f64>> = features[..n_cols].iter().map(|c| c[..n].to_vec()).collect();
    Some(
        (0..n)
            .map(|i| {
                let rest: Vec<usize> = (0..n).filter(|&k| k != i).collect();
                let xf = crate::forms::fit::gather(&cols, &rest);
                let yf: Vec<f64> = rest.iter().map(|&k| target[k]).collect();
                let xi = crate::forms::fit::gather(&cols, &[i]);
                let fr = crate::forms::fit::refit_from(&e, &f0.inner, &xf, &yf, &xf, &yf, &cfg)?;
                let t = crate::forms::nf::eval_terms(&e, &xi, &fr.fitted.inner);
                let p = fr.fitted.offset
                    + fr.fitted
                        .coef
                        .iter()
                        .zip(&t)
                        .map(|(c, g)| c * g[0])
                        .sum::<f64>();
                p.is_finite().then_some(p)
            })
            .collect(),
    )
}

pub(crate) fn loo_refit_r2(
    model: &str,
    features: &[Vec<f64>],
    target: &[f64],
    n_cols: usize,
    n: usize,
) -> Option<f64> {
    let (e, f0) = crate::forms::canon::normalize(&crate::forms::ast::parse(model).ok()?).ok()?;
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

pub(crate) fn loo_rows_needed(model: &str) -> Option<usize> {
    let (_, f) = crate::forms::canon::normalize(&crate::forms::ast::parse(model).ok()?).ok()?;
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
    use crate::config::{COND_CANCEL_MAX, COND_NEGLIGIBLE};
    use crate::forms::ast::Ast;
    let Ok(ast) = crate::forms::ast::parse(model) else {
        return true;
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
            return false;
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
        let negligible = COND_NEGLIGIBLE.max(
            FLOOR
                .with(|f| f.get())
                .max(1.0 - crate::config::EXACT_EXIT_R2)
                .sqrt(),
        );
        if sa.min(sb) < negligible * sv || sa + sb > COND_CANCEL_MAX * sv {
            return false;
        }
    }
    true
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

pub(crate) fn relative_dl(
    model: &str,
    features: &[Vec<f64>],
    target: &[f64],
    n_cols: usize,
    n: usize,
    nodes: usize,
    loo: bool,
) -> Option<f64> {
    use crate::config::RANK_REL_SPAN as REL_SPAN;
    let rows: Vec<usize> = (0..n).filter(|&i| target[i].is_finite()).collect();
    if rows.len() < 2 {
        return None;
    }
    let pos = rows.iter().all(|&i| target[i] > 0.0);
    let neg = rows.iter().all(|&i| target[i] < 0.0);
    let lo = rows
        .iter()
        .map(|&i| target[i].abs())
        .fold(f64::INFINITY, f64::min);
    let hi = rows.iter().map(|&i| target[i].abs()).fold(0.0, f64::max);
    if !(pos || neg) || !(hi >= REL_SPAN * lo) {
        return None;
    }
    let preds: Vec<Option<f64>> = if loo {
        loo_refit_predictions(model, features, target, n_cols, n)?
    } else {
        let ast = crate::forms::ast::parse(model).ok()?;
        let cols: Vec<Vec<f64>> = features.iter().map(|c| c[..n].to_vec()).collect();
        (0..n)
            .map(|i| Some(ast.eval(&cols, i)).filter(|p| p.is_finite()))
            .collect()
    };
    let mut s = 0.0f64;
    for &i in &rows {
        let Some(p) = preds[i] else {
            return Some(f64::INFINITY);
        };
        s += ((target[i] - p) / target[i]).powi(2);
    }
    let k = rows.len() as f64;
    Some(0.5 * k * (s / k).max(1e-30).ln() + 0.5 * k.ln() * nodes as f64)
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
