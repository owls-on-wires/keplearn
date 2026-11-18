use crate::forms::data::Table;
use crate::forms::fit::{gather, FitConfig, FitResult};
use crate::forms::nf::Expr;

pub const SNAP_TOL: f64 = 1e-13;

pub const DEGENERATE_REL: f64 = 3e-2;

pub(crate) fn nice_near(v: f64) -> Vec<f64> {
    let mut vals: Vec<f64> = Vec::new();
    for (_, c) in nice_near_costed(v) {
        if !vals.iter().any(|x| *x == c) {
            vals.push(c);
        }
    }
    #[cfg(test)]
    if REVERSED_ORDER.with(|r| r.get()) {
        vals.reverse();
    }
    vals
}

pub(crate) fn written_cost(v: f64) -> usize {
    if !v.is_finite() {
        return UNWRITTEN_COST;
    }
    thread_local! {
        static COSTS : std::cell::RefCell < std::collections::HashMap < u64, usize >> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    }
    if let Some(k) = COSTS.with(|c| c.borrow().get(&v.to_bits()).copied()) {
        return k;
    }
    let k = nice_near_costed(v)
        .into_iter()
        .filter(|(_, c)| (c - v).abs() <= 1e-12 * v.abs().max(1e-300))
        .map(|(k, _)| k)
        .min()
        .unwrap_or(UNWRITTEN_COST);
    COSTS.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 1 << 16 {
            c.clear();
        }
        c.insert(v.to_bits(), k);
    });
    k
}

pub(crate) const UNWRITTEN_COST: usize = 13;

fn nice_near_costed(v: f64) -> Vec<(usize, f64)> {
    fn digits(k: f64) -> usize {
        let mut k = k.abs().round() as u64;
        if k == 0 {
            return 1;
        }
        while k % 10 == 0 {
            k /= 10;
        }
        k.to_string().len()
    }
    let den_cost = |den: usize| if den > 1 { digits(den as f64) } else { 0 };
    let mut out: Vec<(usize, f64, f64)> = vec![(0, 0.0, 0.0)];
    if v == 0.0 || !v.is_finite() {
        return vec![(0, 0.0)];
    }
    let mag = v.abs().log10().floor();
    for sig in 1..=6usize {
        let p = 10f64.powf(mag - sig as f64 + 1.0);
        for den in [1usize, 2, 3, 4] {
            let k = (v / p * den as f64).round();
            let cand = k / den as f64 * p;
            if cand != 0.0 {
                out.push((digits(k) + den_cost(den), (cand - v).abs(), cand));
            }
        }
    }
    for &(base, _, powers, full) in crate::forms::nf::SNAP_FAMILIES {
        for &k in powers {
            let b = base.powf(k);
            for den in crate::forms::nf::SNAP_DENOMS {
                let num = (v / b * den as f64).round();
                let admitted = full
                    || crate::forms::nf::SNAP_FEW_MULTS
                        .iter()
                        .any(|&(p, q)| p == num.abs() && q == den);
                if num != 0.0 && num.abs() <= crate::forms::nf::SNAP_MULT_MAX && admitted {
                    out.push((
                        digits(num) + den_cost(den) + 1,
                        (num / den as f64 * b - v).abs(),
                        num / den as f64 * b,
                    ));
                }
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    out.into_iter().map(|(k, _, c)| (k, c)).collect()
}

#[cfg(test)]
thread_local! {
    pub static REVERSED_ORDER : std::cell::Cell < bool > = const {
    std::cell::Cell::new(false) };
}

pub struct Extrap {
    x_in: Vec<Vec<f64>>,
    y_in: Vec<f64>,
    x_out: Vec<Vec<f64>>,
    y_out: Vec<f64>,
}

impl Extrap {
    pub fn new(table: &Table) -> Extrap {
        let n = table.y.len();
        let mut out = vec![0.0f64; n];
        for c in &table.cols {
            let mut idx: Vec<usize> = (0..n).collect();
            idx.sort_by(|&a, &b| c[a].total_cmp(&c[b]));
            for (rank, &r) in idx.iter().enumerate() {
                let d = (rank as f64 / (n.max(2) - 1) as f64 - 0.5).abs();
                out[r] = out[r].max(d);
            }
        }
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| out[a].total_cmp(&out[b]).then(a.cmp(&b)));
        let k = n - n / 5;
        let (inn, outer) = order.split_at(k);
        Extrap {
            x_in: gather(&table.cols, inn),
            y_in: inn.iter().map(|&r| table.y[r]).collect(),
            x_out: gather(&table.cols, outer),
            y_out: outer.iter().map(|&r| table.y[r]).collect(),
        }
    }

    pub fn holds(&self, e: &Expr, fr: &FitResult, cfg: &FitConfig) -> bool {
        if self.y_out.len() < 4 {
            return true;
        }
        crate::forms::fit::refit_from(
            e,
            &fr.fitted.inner,
            &self.x_in,
            &self.y_in,
            &self.x_out,
            &self.y_out,
            cfg,
        )
        .map_or(false, |r| r.r2_holdout >= cfg.exact_r2)
    }
}

fn offset_solver(t: &[Vec<f64>], y: &[f64]) -> impl Fn(f64) -> Option<Vec<f64>> {
    let k = t.len();
    let rows: Vec<usize> = (0..y.len())
        .filter(|&r| y[r].is_finite() && t.iter().all(|g| g[r].is_finite()))
        .collect();
    let ok = k > 0 && rows.len() >= k;
    let mut g = vec![0.0f64; k * k];
    let mut ty = vec![0.0f64; k];
    let mut t1 = vec![0.0f64; k];
    for &r in &rows {
        for i in 0..k {
            for j in 0..k {
                g[i * k + j] += t[i][r] * t[j][r];
            }
            ty[i] += t[i][r] * y[r];
            t1[i] += t[i][r];
        }
    }
    move |offset: f64| -> Option<Vec<f64>> {
        if !ok {
            return None;
        }
        let mut a = vec![0.0f64; k * (k + 1)];
        for i in 0..k {
            a[i * (k + 1)..i * (k + 1) + k].copy_from_slice(&g[i * k..(i + 1) * k]);
            a[i * (k + 1) + k] = ty[i] - offset * t1[i];
        }
        for col in 0..k {
            let piv = (col..k).max_by(|&p, &q| {
                a[p * (k + 1) + col]
                    .abs()
                    .total_cmp(&a[q * (k + 1) + col].abs())
            })?;
            if a[piv * (k + 1) + col].abs() < 1e-300 {
                return None;
            }
            for c in 0..=k {
                a.swap(col * (k + 1) + c, piv * (k + 1) + c);
            }
            for r in 0..k {
                if r != col {
                    let f = a[r * (k + 1) + col] / a[col * (k + 1) + col];
                    for c in col..=k {
                        a[r * (k + 1) + c] -= f * a[col * (k + 1) + c];
                    }
                }
            }
        }
        let c: Vec<f64> = (0..k)
            .map(|i| a[i * (k + 1) + k] / a[i * (k + 1) + i])
            .collect();
        c.iter().all(|v| v.is_finite()).then_some(c)
    }
}

pub fn snap(
    e: &Expr,
    fr: &mut FitResult,
    x_fit: &[Vec<f64>],
    y_fit: &[f64],
    x_ho: &[Vec<f64>],
    y_ho: &[f64],
    tol: f64,
) {
    snap_by(e, fr, x_fit, y_fit, x_ho, y_ho, tol, true, None)
}

pub type SnapChoice<'a> = &'a dyn Fn(&[crate::forms::nf::Fitted]) -> usize;

#[allow(clippy::too_many_arguments)]
pub fn snap_by(
    e: &Expr,
    fr: &mut FitResult,
    x_fit: &[Vec<f64>],
    y_fit: &[f64],
    x_ho: &[Vec<f64>],
    y_ho: &[f64],
    tol: f64,
    inner: bool,
    choose: Option<SnapChoice>,
) {
    let terms: std::cell::RefCell<std::collections::HashMap<Vec<u64>, std::rc::Rc<Vec<Vec<f64>>>>> =
        Default::default();
    let score = |f: &crate::forms::nf::Fitted| -> f64 {
        let key: Vec<u64> = f.inner.iter().map(|v| v.to_bits()).collect();
        let t = terms.borrow().get(&key).cloned();
        let t = t.unwrap_or_else(|| {
            let t = std::rc::Rc::new(crate::forms::nf::eval_terms(e, x_ho, &f.inner));
            terms.borrow_mut().insert(key, t.clone());
            t
        });
        let yhat: Vec<f64> = (0..y_ho.len())
            .map(|r| {
                f.offset
                    + f.coef
                        .iter()
                        .zip(t.iter())
                        .map(|(c, g)| c * g[r])
                        .sum::<f64>()
            })
            .collect();
        crate::forms::fit::r2(y_ho, &yhat)
    };
    let base = score(&fr.fitted);
    if !base.is_finite() {
        return;
    }
    let admitted = |f: &crate::forms::nf::Fitted| score(f) >= base - tol;
    let every = |n: usize| -> Vec<usize> {
        if n > SNAP_SCREEN_ROWS {
            (0..SNAP_SCREEN_ROWS)
                .map(|k| k * n / SNAP_SCREEN_ROWS)
                .collect()
        } else {
            (0..n).collect()
        }
    };
    let (sf, sh) = (every(y_fit.len()), every(y_ho.len()));
    let xs_fit: Vec<Vec<f64>> = x_fit
        .iter()
        .map(|c| sf.iter().map(|&r| c[r]).collect())
        .collect();
    let ys_fit: Vec<f64> = sf.iter().map(|&r| y_fit[r]).collect();
    let xs_ho: Vec<Vec<f64>> = x_ho
        .iter()
        .map(|c| sh.iter().map(|&r| c[r]).collect())
        .collect();
    let ys_ho: Vec<f64> = sh.iter().map(|&r| y_ho[r]).collect();
    let r2_on = |inner: &[f64],
                 x_fit: &[Vec<f64>],
                 y_fit: &[f64],
                 x_ho: &[Vec<f64>],
                 y_ho: &[f64]|
     -> Option<f64> {
        let t = crate::forms::nf::eval_terms(e, x_fit, inner);
        let (coef, offset) = crate::forms::fit::linear_fit(&t, y_fit)?;
        let th = crate::forms::nf::eval_terms(e, x_ho, inner);
        let yhat: Vec<f64> = (0..y_ho.len())
            .map(|r| offset + coef.iter().zip(&th).map(|(c, g)| c * g[r]).sum::<f64>())
            .collect();
        Some(crate::forms::fit::r2(y_ho, &yhat))
    };
    let base_s =
        r2_on(&fr.fitted.inner, &xs_fit, &ys_fit, &xs_ho, &ys_ho).unwrap_or(f64::NEG_INFINITY);
    let screened = |inner: &[f64]| -> bool {
        choose.is_none()
            || sf.len() == y_fit.len()
            || r2_on(inner, &xs_fit, &ys_fit, &xs_ho, &ys_ho)
                .is_some_and(|r| r >= base_s - 2.0 * tol)
    };
    let pick = |opts: Vec<crate::forms::nf::Fitted>| -> crate::forms::nf::Fitted {
        let k = match choose {
            Some(c) if opts.len() > 1 => c(&opts).min(opts.len() - 1),
            _ => usize::from(opts.len() > 1),
        };
        opts.into_iter().nth(k).unwrap()
    };
    let gather = |vals: Vec<f64>,
                  build: &mut dyn FnMut(f64) -> Option<crate::forms::nf::Fitted>,
                  opts: &mut Vec<crate::forms::nf::Fitted>| {
        let mut free = 0;
        for v in vals {
            let structural = crate::guard::structural_constant(v);
            if choose.is_some() && !structural && free >= SNAP_FREE_OPTIONS {
                continue;
            }
            let Some(f) = build(v) else { continue };
            if admitted(&f) {
                opts.push(f);
                if choose.is_none() {
                    return;
                }
                if !structural {
                    free += 1;
                }
            }
        }
    };
    for k in 0..if inner { fr.fitted.inner.len() } else { 0 } {
        let mut opts = vec![fr.fitted.clone()];
        let cur = fr.fitted.inner.clone();
        let mut build = |cand: f64| {
            let mut inner = cur.clone();
            inner[k] = cand;
            if !screened(&inner) {
                return None;
            }
            let t = crate::forms::nf::eval_terms(e, x_fit, &inner);
            let (coef, offset) = crate::forms::fit::linear_fit(&t, y_fit)?;
            let f = crate::forms::nf::Fitted {
                coef,
                offset,
                inner,
            };
            Some(f)
        };
        gather(nice_near(fr.fitted.inner[k]), &mut build, &mut opts);
        fr.fitted = pick(opts);
    }
    let t_fit = crate::forms::nf::eval_terms(e, x_fit, &fr.fitted.inner);
    let mut opts = vec![fr.fitted.clone()];
    let cur = fr.fitted.clone();
    let solve = offset_solver(&t_fit, y_fit);
    let mut build = |cand: f64| {
        let coef = solve(cand).unwrap_or_else(|| cur.coef.clone());
        Some(crate::forms::nf::Fitted {
            coef,
            offset: cand,
            inner: cur.inner.clone(),
        })
    };
    gather(nice_near(fr.fitted.offset), &mut build, &mut opts);
    fr.fitted = pick(opts);
    for t in 0..fr.fitted.coef.len() {
        let mut opts = vec![fr.fitted.clone()];
        let cur = fr.fitted.clone();
        let mut build = |cand: f64| {
            let mut f = cur.clone();
            f.coef[t] = cand;
            Some(f)
        };
        gather(
            nice_near(cur.coef[t])
                .into_iter()
                .filter(|c| *c != 0.0)
                .collect(),
            &mut build,
            &mut opts,
        );
        fr.fitted = pick(opts);
    }
    fr.r2_holdout = score(&fr.fitted);
}

pub const SNAP_SCREEN_ROWS: usize = 256;

fn ls_cols(cols: &[&[f64]], y: &[f64]) -> Option<Vec<f64>> {
    let k = cols.len();
    if k == 0 {
        return Some(Vec::new());
    }
    let rows: Vec<usize> = (0..y.len())
        .filter(|&r| y[r].is_finite() && cols.iter().all(|c| c[r].is_finite()))
        .collect();
    if rows.len() < k {
        return None;
    }
    let mut a = vec![0.0f64; k * (k + 1)];
    for &r in &rows {
        for i in 0..k {
            for j in 0..k {
                a[i * (k + 1) + j] += cols[i][r] * cols[j][r];
            }
            a[i * (k + 1) + k] += cols[i][r] * y[r];
        }
    }
    for col in 0..k {
        let piv = (col..k).max_by(|&p, &q| {
            a[p * (k + 1) + col]
                .abs()
                .total_cmp(&a[q * (k + 1) + col].abs())
        })?;
        if a[piv * (k + 1) + col].abs() < 1e-300 {
            return None;
        }
        for c in 0..=k {
            a.swap(col * (k + 1) + c, piv * (k + 1) + c);
        }
        for r in 0..k {
            if r != col {
                let f = a[r * (k + 1) + col] / a[col * (k + 1) + col];
                for c in col..=k {
                    a[r * (k + 1) + c] -= f * a[col * (k + 1) + c];
                }
            }
        }
    }
    let c: Vec<f64> = (0..k)
        .map(|i| a[i * (k + 1) + k] / a[i * (k + 1) + i])
        .collect();
    c.iter().all(|v| v.is_finite()).then_some(c)
}

fn structural_count(f: &crate::forms::nf::Fitted) -> usize {
    f.inner
        .iter()
        .chain(f.coef.iter())
        .chain(std::iter::once(&f.offset))
        .filter(|v| crate::guard::structural_constant(**v))
        .count()
}

fn written_count(f: &crate::forms::nf::Fitted) -> usize {
    f.coef.iter().filter(|v| **v != 1.0).count()
        + usize::from(f.offset != 0.0)
        + f.inner
            .iter()
            .filter(|v| v.abs() != 1.0 && **v != 0.0)
            .count()
}

pub const JOINT_VALUES: usize = 3;

pub const JOINT_INNER_COMBOS: usize = 27;

pub const JOINT_LINEAR_COMBOS: usize = 64;

#[allow(clippy::too_many_arguments)]
pub fn snap_joint(
    e: &Expr,
    fr: &mut FitResult,
    x_fit: &[Vec<f64>],
    y_fit: &[f64],
    x_ho: &[Vec<f64>],
    y_ho: &[f64],
    tol: f64,
    exact: bool,
    choose: SnapChoice,
) {
    use crate::forms::nf::Fitted;
    let score_with = |f: &Fitted, th: &[Vec<f64>]| -> f64 {
        let yhat: Vec<f64> = (0..y_ho.len())
            .map(|r| f.offset + f.coef.iter().zip(th).map(|(c, g)| c * g[r]).sum::<f64>())
            .collect();
        crate::forms::fit::r2(y_ho, &yhat)
    };
    let th0 = crate::forms::nf::eval_terms(e, x_ho, &fr.fitted.inner);
    let base = score_with(&fr.fitted, &th0);
    if !base.is_finite() {
        return;
    }
    let n_ho = y_ho.len().max(2) as f64;
    let n_consts = (fr.fitted.inner.len() + fr.fitted.coef.len() + 1) as f64;
    let tol = tol.max(n_consts * n_ho.ln() * (1.0 - base).max(0.0) / n_ho);
    let near = |v: f64, n: usize| -> Vec<f64> {
        let mut c: Vec<f64> = nice_near(v)
            .into_iter()
            .filter(|c| *c != v && crate::guard::structural_constant(*c))
            .collect();
        c.sort_by(|a, b| (a - v).abs().total_cmp(&(b - v).abs()));
        c.dedup();
        c.truncate(n);
        c
    };
    let mut inner_lists: Vec<Vec<f64>> = fr
        .fitted
        .inner
        .iter()
        .enumerate()
        .map(|(j, &v)| {
            let mut l = vec![v];
            for c in near(v, 2 * JOINT_VALUES) {
                if l.len() > JOINT_VALUES {
                    break;
                }
                let mut inner = fr.fitted.inner.clone();
                inner[j] = c;
                let t = crate::forms::nf::eval_terms(e, x_fit, &inner);
                let Some((coef, offset)) = crate::forms::fit::linear_fit(&t, y_fit) else {
                    continue;
                };
                let th = crate::forms::nf::eval_terms(e, x_ho, &inner);
                let f = Fitted {
                    coef,
                    offset,
                    inner,
                };
                if score_with(&f, &th) >= base - 2.0 * tol {
                    l.push(c);
                }
            }
            l
        })
        .collect();
    while inner_lists.iter().map(|l| l.len()).product::<usize>() > JOINT_INNER_COMBOS {
        let Some(l) = inner_lists.iter_mut().max_by_key(|l| l.len()) else {
            break;
        };
        l.pop();
    }
    let mut opts: Vec<Fitted> = vec![fr.fitted.clone()];
    let mut combo = vec![0usize; inner_lists.len()];
    loop {
        let inner: Vec<f64> = combo.iter().zip(&inner_lists).map(|(&k, l)| l[k]).collect();
        let t = crate::forms::nf::eval_terms(e, x_fit, &inner);
        let th = crate::forms::nf::eval_terms(e, x_ho, &inner);
        if let Some((coef0, off0)) = crate::forms::fit::linear_fit(&t, y_fit) {
            let mut lin: Vec<Vec<Option<f64>>> = std::iter::once(off0)
                .chain(coef0.iter().copied())
                .map(|v| {
                    std::iter::once(None)
                        .chain(
                            near(v, JOINT_VALUES)
                                .into_iter()
                                .filter(|c| *c != 0.0 || v.abs() < 1e-300)
                                .map(Some),
                        )
                        .collect()
                })
                .collect();
            lin[0] = std::iter::once(None)
                .chain(
                    near(off0, JOINT_VALUES)
                        .into_iter()
                        .chain(std::iter::once(0.0))
                        .map(Some),
                )
                .collect::<Vec<_>>();
            lin[0].dedup();
            while lin.iter().map(|l| l.len()).product::<usize>() > JOINT_LINEAR_COMBOS {
                let Some(l) = lin.iter_mut().max_by_key(|l| l.len()) else {
                    break;
                };
                l.pop();
            }
            let mut lc = vec![0usize; lin.len()];
            loop {
                let choice: Vec<Option<f64>> = lc.iter().zip(&lin).map(|(&k, l)| l[k]).collect();
                let fixed_off = choice[0];
                let mut yr: Vec<f64> = y_fit.iter().map(|v| v - fixed_off.unwrap_or(0.0)).collect();
                for (i, c) in choice[1..].iter().enumerate() {
                    if let Some(c) = c {
                        for r in 0..yr.len() {
                            yr[r] -= c * t[i][r];
                        }
                    }
                }
                let ones = vec![1.0f64; y_fit.len()];
                let mut cols: Vec<&[f64]> = Vec::new();
                for (i, c) in choice[1..].iter().enumerate() {
                    if c.is_none() {
                        cols.push(&t[i]);
                    }
                }
                if fixed_off.is_none() {
                    cols.push(&ones);
                }
                if let Some(sol) = ls_cols(&cols, &yr) {
                    let mut it = sol.into_iter();
                    let coef: Vec<f64> = choice[1..]
                        .iter()
                        .map(|c| c.unwrap_or_else(|| it.next().unwrap_or(0.0)))
                        .collect();
                    let offset = fixed_off.unwrap_or_else(|| it.next().unwrap_or(0.0));
                    let f = Fitted {
                        coef,
                        offset,
                        inner: inner.clone(),
                    };
                    let gains = if exact {
                        structural_count(&f) > structural_count(&fr.fitted)
                    } else {
                        written_count(&f) < written_count(&fr.fitted)
                    };
                    if f.coef.iter().all(|v| v.is_finite())
                        && gains
                        && score_with(&f, &th) >= base - tol
                    {
                        opts.push(f);
                    }
                }
                let mut i = 0;
                while i < lc.len() {
                    lc[i] += 1;
                    if lc[i] < lin[i].len() {
                        break;
                    }
                    lc[i] = 0;
                    i += 1;
                }
                if i == lc.len() {
                    break;
                }
            }
        }
        let mut i = 0;
        while i < combo.len() {
            combo[i] += 1;
            if combo[i] < inner_lists[i].len() {
                break;
            }
            combo[i] = 0;
            i += 1;
        }
        if i == combo.len() {
            break;
        }
    }
    if opts.len() > 1 {
        let k = choose(&opts).min(opts.len() - 1);
        let f = opts.swap_remove(k);
        let th = crate::forms::nf::eval_terms(e, x_ho, &f.inner);
        fr.r2_holdout = score_with(&f, &th);
        fr.fitted = f;
    }
}

pub const SNAP_FREE_OPTIONS: usize = 3;

pub fn is_degenerate(e: &Expr, cols: &[Vec<f64>], inner: &[f64], rel: f64) -> bool {
    for st in crate::forms::nf::arg_stats(e, cols, inner) {
        if st.n < 2 {
            return true;
        }
        let range = st.max - st.min;
        let tiny = match st.kind {
            crate::forms::nf::ArgKind::Un(crate::forms::nf::Un::Exp)
            | crate::forms::nf::ArgKind::Un(crate::forms::nf::Un::Sin)
            | crate::forms::nf::ArgKind::Un(crate::forms::nf::Un::Cos) => range < rel,
            _ => range < rel * st.mean_abs,
        };
        if tiny {
            return true;
        }
    }
    false
}
