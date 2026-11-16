use crate::forms::nf;

pub const SEED_GRID: [f64; 8] = [-3.0, -2.0, -1.0, -0.5, 0.5, 1.0, 2.0, 3.0];

pub const FIT_SURPLUS: usize = 3;

pub fn fit_min_rows(p: usize) -> usize {
    p + FIT_SURPLUS
}

const RANK_TOL: f64 = 1e-11;

const FD_REL_STEP: f64 = 1e-6;

const LAMBDA_START: f64 = 1e-3;

const LAMBDA_MIN: f64 = 1e-15;

const LAMBDA_MAX: f64 = 1e12;

const LM_REL_TOL: f64 = 1e-12;

const SSR_FLOOR: f64 = 1e-30;

#[derive(Clone, Copy, Debug)]
pub struct FitConfig {
    pub exact_r2: f64,
    pub screen_r2: f64,
    pub max_mask_frac: f64,
    pub lm_iters: usize,
    pub lm_restarts: usize,
    pub init_1d: bool,
}

impl Default for FitConfig {
    fn default() -> Self {
        FitConfig {
            exact_r2: 1.0 - 1e-12,
            screen_r2: 0.5,
            max_mask_frac: 0.25,
            lm_iters: 50,
            lm_restarts: 3,
            init_1d: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct FitResult {
    pub fitted: nf::Fitted,
    pub r2_holdout: f64,
}

impl FitResult {
    pub fn is_exact(&self, cfg: &FitConfig) -> bool {
        self.r2_holdout >= cfg.exact_r2
    }
}

pub fn fit_expr(
    e: &nf::Expr,
    x_fit: &[Vec<f64>],
    y_fit: &[f64],
    x_ho: &[Vec<f64>],
    y_ho: &[f64],
    cfg: &FitConfig,
) -> Option<FitResult> {
    if e.terms.is_empty() {
        return None;
    }
    if let Some(m) = e.max_col() {
        if m as usize >= x_fit.len() || m as usize >= x_ho.len() {
            return None;
        }
    }
    let mut ev = Evaluator {
        e,
        x: x_fit,
        y: y_fit,
        evals: 0,
    };
    let k = e.n_consts();
    let mut seeds: Vec<(Vec<f64>, LinFit)> = Vec::new();
    if !(cfg.init_1d && k >= 3) {
        for inner in seed_grid(k) {
            if let Some(f) = ev.linear_fit(&inner, cfg) {
                seeds.push((inner, f));
            }
        }
    }
    if cfg.init_1d && k >= 1 {
        if let Some(s) = coordinate_seed(&mut ev, k, cfg) {
            seeds.push(s);
        }
    }
    if seeds.is_empty() {
        return None;
    }
    seeds.sort_by(|a, b| b.1.r2.total_cmp(&a.1.r2));
    let (mut best_inner, mut best) = seeds[0].clone();
    if k > 0 && best.r2 >= cfg.screen_r2 {
        for (seed, _) in seeds.iter().take(cfg.lm_restarts) {
            let Some(theta) = ev.levenberg_marquardt(seed, cfg) else {
                continue;
            };
            let Some(f) = ev.linear_fit(&theta, cfg) else {
                continue;
            };
            if f.r2 > best.r2 {
                best = f;
                best_inner = theta;
            }
        }
    }
    let mut ho = Evaluator {
        e,
        x: x_ho,
        y: y_ho,
        evals: 0,
    };
    let (r2_holdout, _) = ho.score(&best, &best_inner, cfg);
    Some(FitResult {
        fitted: nf::Fitted {
            coef: best.coef,
            offset: best.offset,
            inner: best_inner,
        },
        r2_holdout,
    })
}

pub fn refit_from(
    e: &nf::Expr,
    inner0: &[f64],
    x_fit: &[Vec<f64>],
    y_fit: &[f64],
    x_ho: &[Vec<f64>],
    y_ho: &[f64],
    cfg: &FitConfig,
) -> Option<FitResult> {
    let mut ev = Evaluator {
        e,
        x: x_fit,
        y: y_fit,
        evals: 0,
    };
    let mut best_inner = inner0.to_vec();
    let mut best = ev.linear_fit(&best_inner, cfg)?;
    if !inner0.is_empty() {
        if let Some(theta) = ev.levenberg_marquardt(inner0, cfg) {
            if let Some(f) = ev.linear_fit(&theta, cfg) {
                if f.r2 > best.r2 {
                    best = f;
                    best_inner = theta;
                }
            }
        }
    }
    let mut ho = Evaluator {
        e,
        x: x_ho,
        y: y_ho,
        evals: 0,
    };
    let (r2_holdout, _) = ho.score(&best, &best_inner, cfg);
    Some(FitResult {
        fitted: nf::Fitted {
            coef: best.coef,
            offset: best.offset,
            inner: best_inner,
        },
        r2_holdout,
    })
}

pub fn r2(y: &[f64], yhat: &[f64]) -> f64 {
    let n = y.len().min(yhat.len());
    let (mut sum, mut m) = (0.0f64, 0usize);
    for i in 0..n {
        if y[i].is_finite() && yhat[i].is_finite() {
            sum += y[i];
            m += 1;
        }
    }
    if m == 0 {
        return -1.0;
    }
    let mean = sum / m as f64;
    let (mut ssr, mut sst) = (0.0f64, 0.0f64);
    for i in 0..n {
        if y[i].is_finite() && yhat[i].is_finite() {
            ssr += (y[i] - yhat[i]) * (y[i] - yhat[i]);
            sst += (y[i] - mean) * (y[i] - mean);
        }
    }
    r2_from(ssr, sst)
}

pub fn split_indices(n: usize, seed: u64) -> (Vec<usize>, Vec<usize>) {
    let mut idx: Vec<usize> = (0..n).collect();
    let mut rng = Lcg::new(seed);
    for i in (1..n).rev() {
        let j = rng.below(i + 1);
        idx.swap(i, j);
    }
    let n_fit = (3 * n + 2) / 4;
    let mut fit = idx[..n_fit].to_vec();
    let mut ho = idx[n_fit..].to_vec();
    fit.sort_unstable();
    ho.sort_unstable();
    (fit, ho)
}

pub fn gather(cols: &[Vec<f64>], idx: &[usize]) -> Vec<Vec<f64>> {
    cols.iter()
        .map(|c| idx.iter().map(|&i| c[i]).collect())
        .collect()
}

struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Lcg {
        Lcg(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    fn next_u32(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }

    fn below(&mut self, n: usize) -> usize {
        ((self.next_u32() as u64 * n as u64) >> 32) as usize
    }
}

#[derive(Clone, Debug)]
struct LinFit {
    coef: Vec<f64>,
    offset: f64,
    r2: f64,
}

fn r2_from(ssr: f64, sst: f64) -> f64 {
    if sst <= 0.0 {
        return if ssr <= 0.0 { 1.0 } else { 0.0 };
    }
    let v = 1.0 - ssr / sst;
    if v.is_finite() {
        v
    } else {
        -1.0
    }
}

fn coordinate_seed(ev: &mut Evaluator, k: usize, cfg: &FitConfig) -> Option<(Vec<f64>, LinFit)> {
    let mut cur = vec![1.0f64; k];
    let mut best: Option<LinFit> = ev.linear_fit(&cur, cfg);
    let r2_of = |f: &Option<LinFit>| f.as_ref().map_or(f64::NEG_INFINITY, |f| f.r2);
    for _ in 0..2 {
        for j in 0..k {
            let mut bj = (r2_of(&best), cur[j]);
            for e in -3..=3 {
                for m in [1.0, 2.0, 5.0] {
                    for sg in [1.0, -1.0] {
                        let v = sg * m * 10f64.powi(e);
                        let mut t = cur.clone();
                        t[j] = v;
                        let f = ev.linear_fit(&t, cfg);
                        if r2_of(&f) > bj.0 {
                            bj = (r2_of(&f), v);
                        }
                    }
                }
            }
            let sg = bj.1.signum();
            let (mut a, mut c) = (bj.1.abs().log10() - 0.5, bj.1.abs().log10() + 0.5);
            let g = 0.618_033_988_75;
            for _ in 0..12 {
                let m1 = c - g * (c - a);
                let m2 = a + g * (c - a);
                let mut t1 = cur.clone();
                t1[j] = sg * 10f64.powf(m1);
                let mut t2 = cur.clone();
                t2[j] = sg * 10f64.powf(m2);
                if r2_of(&ev.linear_fit(&t1, cfg)) > r2_of(&ev.linear_fit(&t2, cfg)) {
                    c = m2;
                } else {
                    a = m1;
                }
            }
            let mut t = cur.clone();
            t[j] = sg * 10f64.powf(0.5 * (a + c));
            let f = ev.linear_fit(&t, cfg);
            if r2_of(&f) > bj.0 {
                cur = t;
            } else {
                cur[j] = bj.1;
            }
            best = ev.linear_fit(&cur, cfg);
        }
    }
    best.map(|f| (cur, f))
}

fn seed_grid(k: usize) -> Vec<Vec<f64>> {
    let mut out = vec![Vec::new()];
    for _ in 0..k {
        let mut next = Vec::with_capacity(out.len() * SEED_GRID.len());
        for prefix in &out {
            for &v in &SEED_GRID {
                let mut p = prefix.clone();
                p.push(v);
                next.push(p);
            }
        }
        out = next;
    }
    out
}

fn valid_rows(g: &[Vec<f64>], y: &[f64]) -> Vec<usize> {
    (0..y.len())
        .filter(|&r| y[r].is_finite() && g.iter().all(|t| t[r].is_finite()))
        .collect()
}

fn too_masked(n_valid: usize, n: usize, cfg: &FitConfig) -> bool {
    n == 0 || (n - n_valid) as f64 > cfg.max_mask_frac * n as f64
}

fn predict_row(g: &[Vec<f64>], coef: &[f64], offset: f64, r: usize) -> f64 {
    let mut v = offset;
    for (t, c) in g.iter().zip(coef) {
        v += c * t[r];
    }
    v
}

fn solve_linear(g: &[Vec<f64>], y: &[f64], rows: &[usize]) -> Option<(Vec<f64>, f64)> {
    qr_solve(&qr_factor(g, rows)?, y, rows)
}

pub struct QrFactor {
    t: usize,
    scale: Vec<f64>,
    a: Vec<Vec<f64>>,
    vnorm2: Vec<f64>,
    rdiag: Vec<f64>,
}

pub fn qr_factor(g: &[Vec<f64>], rows: &[usize]) -> Option<QrFactor> {
    let t = g.len();
    let m = t + 1;
    let n = rows.len();
    if n < fit_min_rows(m) {
        return None;
    }
    let mut a: Vec<Vec<f64>> = Vec::with_capacity(m);
    for col in g {
        a.push(rows.iter().map(|&r| col[r]).collect());
    }
    a.push(vec![1.0; n]);
    let mut scale = vec![0.0f64; m];
    for j in 0..m {
        let s = a[j].iter().map(|v| v * v).sum::<f64>().sqrt();
        if !(s.is_finite() && s > 0.0) {
            return None;
        }
        scale[j] = s;
        for v in a[j].iter_mut() {
            *v /= s;
        }
    }
    let mut rdiag = vec![0.0f64; m];
    let mut vnorm2s = vec![0.0f64; m];
    for j in 0..m {
        let norm = a[j][j..].iter().map(|v| v * v).sum::<f64>().sqrt();
        if norm < RANK_TOL {
            return None;
        }
        let alpha = if a[j][j] > 0.0 { -norm } else { norm };
        a[j][j] -= alpha;
        let vnorm2: f64 = a[j][j..].iter().map(|v| v * v).sum();
        rdiag[j] = alpha;
        vnorm2s[j] = vnorm2;
        if vnorm2 == 0.0 {
            continue;
        }
        let (head, tail) = a.split_at_mut(j + 1);
        let v = &head[j][j..];
        for col in tail.iter_mut() {
            let d: f64 = v.iter().zip(&col[j..]).map(|(p, q)| p * q).sum();
            let f = 2.0 * d / vnorm2;
            for (c, p) in col[j..].iter_mut().zip(v) {
                *c -= f * p;
            }
        }
    }
    let rmax = rdiag.iter().fold(0.0f64, |acc, v| acc.max(v.abs()));
    if rdiag.iter().any(|v| v.abs() < RANK_TOL * rmax) {
        return None;
    }
    Some(QrFactor {
        t,
        scale,
        a,
        vnorm2: vnorm2s,
        rdiag,
    })
}

pub fn qr_solve(f: &QrFactor, y: &[f64], rows: &[usize]) -> Option<(Vec<f64>, f64)> {
    let m = f.t + 1;
    let mut b: Vec<f64> = rows.iter().map(|&r| y[r]).collect();
    for j in 0..m {
        let vnorm2 = f.vnorm2[j];
        if vnorm2 == 0.0 {
            continue;
        }
        let v = &f.a[j][j..];
        let d: f64 = v.iter().zip(&b[j..]).map(|(p, q)| p * q).sum();
        let fct = 2.0 * d / vnorm2;
        for (c, p) in b[j..].iter_mut().zip(v) {
            *c -= fct * p;
        }
    }
    let mut z = vec![0.0f64; m];
    for r in (0..m).rev() {
        let mut s = b[r];
        for c in (r + 1)..m {
            s -= f.a[c][r] * z[c];
        }
        z[r] = s / f.rdiag[r];
    }
    let sol: Vec<f64> = z.iter().zip(&f.scale).map(|(v, s)| v / s).collect();
    if sol.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let offset = sol[f.t];
    Some((sol[..f.t].to_vec(), offset))
}

fn solve_small(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let k = b.len();
    for col in 0..k {
        let mut piv = col;
        for r in (col + 1)..k {
            if a[r][col].abs() > a[piv][col].abs() {
                piv = r;
            }
        }
        if !(a[piv][col].abs() > 0.0) {
            return None;
        }
        a.swap(col, piv);
        b.swap(col, piv);
        for r in (col + 1)..k {
            let f = a[r][col] / a[col][col];
            for c in col..k {
                a[r][c] -= f * a[col][c];
            }
            b[r] -= f * b[col];
        }
    }
    let mut x = vec![0.0f64; k];
    for r in (0..k).rev() {
        let mut s = b[r];
        for c in (r + 1)..k {
            s -= a[r][c] * x[c];
        }
        x[r] = s / a[r][r];
    }
    if x.iter().all(|v| v.is_finite()) {
        Some(x)
    } else {
        None
    }
}

struct Evaluator<'a> {
    e: &'a nf::Expr,
    x: &'a [Vec<f64>],
    y: &'a [f64],
    evals: usize,
}

impl Evaluator<'_> {
    fn terms(&mut self, inner: &[f64]) -> Vec<Vec<f64>> {
        self.evals += 1;
        nf::eval_terms(self.e, self.x, inner)
    }

    fn linear_fit(&mut self, inner: &[f64], cfg: &FitConfig) -> Option<LinFit> {
        let g = self.terms(inner);
        let rows = valid_rows(&g, self.y);
        if too_masked(rows.len(), self.y.len(), cfg) {
            return None;
        }
        let (coef, offset) = solve_linear(&g, self.y, &rows)?;
        let mean = rows.iter().map(|&r| self.y[r]).sum::<f64>() / rows.len() as f64;
        let (mut ssr, mut sst) = (0.0f64, 0.0f64);
        for &r in &rows {
            let d = self.y[r] - predict_row(&g, &coef, offset, r);
            ssr += d * d;
            sst += (self.y[r] - mean) * (self.y[r] - mean);
        }
        Some(LinFit {
            r2: r2_from(ssr, sst),
            coef,
            offset,
        })
    }

    fn projected_residual(&mut self, inner: &[f64], rows: &[usize]) -> Option<Vec<f64>> {
        let g = self.terms(inner);
        if !rows.iter().all(|&r| g.iter().all(|t| t[r].is_finite())) {
            return None;
        }
        let (coef, offset) = solve_linear(&g, self.y, rows)?;
        let res: Vec<f64> = rows
            .iter()
            .map(|&r| self.y[r] - predict_row(&g, &coef, offset, r))
            .collect();
        if res.iter().all(|v| v.is_finite()) {
            Some(res)
        } else {
            None
        }
    }

    fn jacobian(&mut self, theta: &[f64], r0: &[f64], rows: &[usize]) -> Option<Vec<Vec<f64>>> {
        let mut jac = Vec::with_capacity(theta.len());
        for j in 0..theta.len() {
            let h = FD_REL_STEP * theta[j].abs().max(1.0);
            let mut tp = theta.to_vec();
            tp[j] += h;
            let mut tm = theta.to_vec();
            tm[j] -= h;
            let rp = self.projected_residual(&tp, rows);
            let rm = self.projected_residual(&tm, rows);
            let col: Vec<f64> = match (rp, rm) {
                (Some(p), Some(m)) => p.iter().zip(&m).map(|(a, b)| (a - b) / (2.0 * h)).collect(),
                (Some(p), None) => p.iter().zip(r0).map(|(a, b)| (a - b) / h).collect(),
                (None, Some(m)) => r0.iter().zip(&m).map(|(a, b)| (a - b) / h).collect(),
                (None, None) => return None,
            };
            jac.push(col);
        }
        Some(jac)
    }

    fn levenberg_marquardt(&mut self, seed: &[f64], cfg: &FitConfig) -> Option<Vec<f64>> {
        let g = self.terms(seed);
        let rows = valid_rows(&g, self.y);
        let mut theta = seed.to_vec();
        let mut res = self.projected_residual(&theta, &rows)?;
        let mut ssr: f64 = res.iter().map(|v| v * v).sum();
        let mean = rows.iter().map(|&r| self.y[r]).sum::<f64>() / rows.len() as f64;
        let sst: f64 = rows
            .iter()
            .map(|&r| (self.y[r] - mean) * (self.y[r] - mean))
            .sum();
        let k = theta.len();
        let mut lambda = LAMBDA_START;
        let mut jtj = vec![vec![0.0f64; k]; k];
        let mut jtr = vec![0.0f64; k];
        let mut need_jac = true;
        let mut iters = 0;
        while iters < cfg.lm_iters {
            if ssr <= SSR_FLOOR * sst {
                break;
            }
            if need_jac {
                let Some(jac) = self.jacobian(&theta, &res, &rows) else {
                    break;
                };
                for a in 0..k {
                    for b in 0..k {
                        jtj[a][b] = jac[a].iter().zip(&jac[b]).map(|(p, q)| p * q).sum();
                    }
                    jtr[a] = jac[a].iter().zip(&res).map(|(p, q)| p * q).sum();
                }
                need_jac = false;
            }
            iters += 1;
            let dmax = (0..k).fold(0.0f64, |acc, a| acc.max(jtj[a][a]));
            let mut lhs = jtj.clone();
            for a in 0..k {
                lhs[a][a] += lambda * jtj[a][a].max(1e-12 * dmax).max(f64::MIN_POSITIVE);
            }
            let rhs: Vec<f64> = jtr.iter().map(|v| -v).collect();
            let step = solve_small(lhs, rhs);
            let trial: Option<(Vec<f64>, Vec<f64>, f64)> = step.and_then(|d| {
                let t: Vec<f64> = theta.iter().zip(&d).map(|(a, b)| a + b).collect();
                let r = self.projected_residual(&t, &rows)?;
                let s = r.iter().map(|v| v * v).sum::<f64>();
                Some((t, r, s))
            });
            match trial {
                Some((t, r, s)) if s < ssr => {
                    let rel = (ssr - s) / ssr;
                    theta = t;
                    res = r;
                    ssr = s;
                    lambda = (lambda / 10.0).max(LAMBDA_MIN);
                    need_jac = true;
                    if rel < LM_REL_TOL {
                        break;
                    }
                }
                _ => {
                    lambda *= 10.0;
                    if lambda > LAMBDA_MAX {
                        break;
                    }
                }
            }
        }
        Some(theta)
    }

    fn score(&mut self, f: &LinFit, inner: &[f64], cfg: &FitConfig) -> (f64, usize) {
        let n = self.y.len();
        if n == 0 {
            return (-1.0, 0);
        }
        let g = self.terms(inner);
        let yhat: Vec<f64> = (0..n)
            .map(|r| {
                if g.iter().all(|t| t[r].is_finite()) {
                    predict_row(&g, &f.coef, f.offset, r)
                } else {
                    f64::NAN
                }
            })
            .collect();
        let n_valid = (0..n)
            .filter(|&r| self.y[r].is_finite() && yhat[r].is_finite())
            .count();
        if too_masked(n_valid, n, cfg) {
            return (-1.0, n_valid);
        }
        (r2(self.y, &yhat), n_valid)
    }
}

pub fn linear_fit(g: &[Vec<f64>], y: &[f64]) -> Option<(Vec<f64>, f64)> {
    let rows: Vec<usize> = (0..y.len())
        .filter(|&r| y[r].is_finite() && g.iter().all(|c| c[r].is_finite()))
        .collect();
    solve_linear(g, y, &rows)
}
