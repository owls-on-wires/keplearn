use crate::forms::data::Table;
use crate::forms::fit::{gather, FitConfig, FitResult};
use crate::forms::nf::Expr;

pub const SNAP_TOL: f64 = 1e-13;

pub const DEGENERATE_REL: f64 = 3e-2;

pub(crate) fn nice_near(v: f64) -> Vec<f64> {
    let mut out: Vec<(usize, usize, f64, f64)> = vec![(0, 1, 0.0, 0.0)];
    if v == 0.0 || !v.is_finite() {
        return vec![0.0];
    }
    let mag = v.abs().log10().floor();
    for sig in 1..=6usize {
        let p = 10f64.powf(mag - sig as f64 + 1.0);
        for den in [1usize, 2, 3, 4] {
            let k = (v / p * den as f64).round();
            let cand = k / den as f64 * p;
            if cand != 0.0 {
                out.push((sig, den, (cand - v).abs(), cand));
            }
        }
    }
    for base in crate::forms::nf::SNAP_CONSTANTS {
        for k in crate::forms::nf::SNAP_POWERS {
            let b = base.powf(k);
            for den in crate::forms::nf::SNAP_DENOMS {
                let num = (v / b * den as f64).round();
                if num != 0.0 && num.abs() <= crate::forms::nf::SNAP_MULT_MAX {
                    out.push((
                        2,
                        den,
                        (num / den as f64 * b - v).abs(),
                        num / den as f64 * b,
                    ));
                }
            }
        }
    }
    out.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then(a.1.cmp(&b.1))
            .then(a.2.partial_cmp(&b.2).unwrap())
    });
    let mut vals: Vec<f64> = Vec::new();
    for (_, _, _, c) in out {
        if !vals.iter().any(|x| *x == c) {
            vals.push(c);
        }
    }
    vals
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

fn coef_for_offset(t: &[Vec<f64>], y: &[f64], offset: f64) -> Option<Vec<f64>> {
    let k = t.len();
    let n = y.len();
    let rows: Vec<usize> = (0..n)
        .filter(|&r| y[r].is_finite() && t.iter().all(|g| g[r].is_finite()))
        .collect();
    if k == 0 || rows.len() < k {
        return None;
    }
    let mut a = vec![0.0f64; k * (k + 1)];
    for &r in &rows {
        for i in 0..k {
            for j in 0..k {
                a[i * (k + 1) + j] += t[i][r] * t[j][r];
            }
            a[i * (k + 1) + k] += t[i][r] * (y[r] - offset);
        }
    }
    for col in 0..k {
        let piv = (col..k).max_by(|&p, &q| {
            a[p * (k + 1) + col]
                .abs()
                .partial_cmp(&a[q * (k + 1) + col].abs())
                .unwrap()
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
    let x: Vec<f64> = (0..k)
        .map(|i| a[i * (k + 1) + k] / a[i * (k + 1) + i])
        .collect();
    x.iter().all(|v| v.is_finite()).then_some(x)
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
    let score = |f: &crate::forms::nf::Fitted| -> f64 {
        let t = crate::forms::nf::eval_terms(e, x_ho, &f.inner);
        let yhat: Vec<f64> = (0..y_ho.len())
            .map(|r| f.offset + f.coef.iter().zip(&t).map(|(c, g)| c * g[r]).sum::<f64>())
            .collect();
        crate::forms::fit::r2(y_ho, &yhat)
    };
    let base = score(&fr.fitted);
    if !base.is_finite() {
        return;
    }
    for k in 0..fr.fitted.inner.len() {
        for cand in nice_near(fr.fitted.inner[k]) {
            let mut inner = fr.fitted.inner.clone();
            inner[k] = cand;
            let t = crate::forms::nf::eval_terms(e, x_fit, &inner);
            let Some((coef, offset)) = crate::forms::fit::linear_fit(&t, y_fit) else {
                continue;
            };
            let f = crate::forms::nf::Fitted {
                coef,
                offset,
                inner,
            };
            if score(&f) >= base - tol {
                fr.fitted = f;
                break;
            }
        }
    }
    let t_fit = crate::forms::nf::eval_terms(e, x_fit, &fr.fitted.inner);
    for cand in nice_near(fr.fitted.offset) {
        let coef = coef_for_offset(&t_fit, y_fit, cand).unwrap_or_else(|| fr.fitted.coef.clone());
        let mut f = crate::forms::nf::Fitted {
            coef,
            offset: cand,
            inner: fr.fitted.inner.clone(),
        };
        if score(&f) >= base - tol {
            std::mem::swap(&mut fr.fitted, &mut f);
            break;
        }
    }
    for t in 0..fr.fitted.coef.len() {
        for cand in nice_near(fr.fitted.coef[t]) {
            if cand == 0.0 {
                continue;
            }
            let mut f = crate::forms::nf::Fitted {
                coef: fr.fitted.coef.clone(),
                offset: fr.fitted.offset,
                inner: fr.fitted.inner.clone(),
            };
            f.coef[t] = cand;
            if score(&f) >= base - tol {
                std::mem::swap(&mut fr.fitted, &mut f);
                break;
            }
        }
    }
    fr.r2_holdout = score(&fr.fitted);
}

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
