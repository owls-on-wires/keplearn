use crate::*;

pub(crate) fn quick_probe(t: &[f64], inputs: &[&[f64]], qn: usize) -> f64 {
    let q = qn.min(t.len());
    let mut best = 0.0f64;
    for inp in inputs {
        let (r, _, _) = r2_score(&t[..q], &inp[..q.min(inp.len())]);
        if r > best {
            best = r;
        }
    }
    best
}

pub(crate) fn build_probe_basis(inputs: &[&[f64]], qn: usize) -> Vec<Vec<f64>> {
    let n = inputs.len();
    let mut out: Vec<Vec<f64>> = Vec::with_capacity(2 * n + n * n * 2);
    for i in 0..n {
        let a = inputs[i];
        let q = qn.min(a.len());
        out.push((0..q).map(|k| a[k] * a[k]).collect());
        out.push(
            (0..q)
                .map(|k| {
                    if a[k].abs() > 1e-12 {
                        1.0 / a[k]
                    } else {
                        f64::NAN
                    }
                })
                .collect(),
        );
    }
    for i in 0..n {
        if out_of_time() {
            return out;
        }
        for j in 0..n {
            if i == j {
                continue;
            }
            let (a, b) = (inputs[i], inputs[j]);
            let q = qn.min(a.len()).min(b.len());
            if i < j {
                out.push((0..q).map(|k| a[k] * b[k]).collect());
            }
            out.push(
                (0..q)
                    .map(|k| {
                        if b[k].abs() > 1e-12 {
                            a[k] / b[k]
                        } else {
                            f64::NAN
                        }
                    })
                    .collect(),
            );
            out.push((0..q).map(|k| a[k] * b[k] * b[k]).collect());
        }
    }
    out
}

pub(crate) fn quick_probe_ext(t: &[f64], inputs: &[&[f64]], basis: &[Vec<f64>], qn: usize) -> f64 {
    let mut best = quick_probe(t, inputs, qn);
    let q = qn.min(t.len());
    for b in basis {
        let m = q.min(b.len());
        let (r, _, _) = r2_score(&t[..m], &b[..m]);
        if r > best {
            best = r;
        }
    }
    best
}

pub(crate) fn monomial_loglin_r2(resid: &[f64], inputs: &[&[f64]], qn: usize) -> f64 {
    let q = qn.min(resid.len());
    if q < MONOMIAL_FIT_MIN_ROWS {
        return 0.0;
    }
    let mut basis: Vec<Vec<f64>> = Vec::with_capacity(inputs.len());
    for inp in inputs {
        let m = q.min(inp.len());
        let mut col = vec![f64::NAN; q];
        for k in 0..m {
            let v = inp[k].abs();
            col[k] = if v > 1e-300 { v.ln() } else { f64::NAN };
        }
        basis.push(col);
    }
    let mut y = vec![f64::NAN; q];
    for k in 0..q {
        let v = resid[k].abs();
        y[k] = if resid[k].is_finite() && v > 1e-300 {
            v.ln()
        } else {
            f64::NAN
        };
    }
    let Some(sol) = solve_ls(&basis, &y) else {
        return 0.0;
    };
    let k = basis.len();
    let (mut sse, mut sy, mut syy, mut n) = (0.0f64, 0.0f64, 0.0f64, 0u32);
    for r in 0..q {
        if !y[r].is_finite() {
            continue;
        }
        let mut ok = true;
        let mut pred = sol[k];
        for i in 0..k {
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
    if n < MONOMIAL_FIT_MIN_ROWS as u32 {
        return 0.0;
    }
    let sst = syy - sy * sy / n as f64;
    if sst <= 1e-12 {
        return 0.0;
    }
    (1.0 - sse / sst).max(0.0)
}

fn loglin_r2_at(target: &[f64], off: f64, logcols: &[Vec<f64>], q: usize) -> f64 {
    let mut ylog = vec![f64::NAN; q];
    for k in 0..q {
        let d = target[k] - off;
        ylog[k] = if target[k].is_finite() && d.is_finite() && d.abs() > 1e-300 {
            d.abs().ln()
        } else {
            f64::NAN
        };
    }
    let Some(sol) = solve_ls(logcols, &ylog) else {
        return 0.0;
    };
    let kk = logcols.len();
    let (mut sse, mut sy, mut syy, mut n) = (0.0f64, 0.0f64, 0.0f64, 0u32);
    for r in 0..q {
        if !ylog[r].is_finite() {
            continue;
        }
        let mut pred = sol[kk];
        let mut ok = true;
        for i in 0..kk {
            if !logcols[i][r].is_finite() {
                ok = false;
                break;
            }
            pred += sol[i] * logcols[i][r];
        }
        if !ok {
            continue;
        }
        let e = ylog[r] - pred;
        sse += e * e;
        sy += ylog[r];
        syy += ylog[r] * ylog[r];
        n += 1;
    }
    if n < LOGLIN_MIN_ROWS as u32 {
        return 0.0;
    }
    let sst = syy - sy * sy / n as f64;
    if sst <= 1e-12 {
        return 0.0;
    }
    (1.0 - sse / sst).max(0.0)
}

fn golden_max(mut a: f64, mut b: f64, f: impl Fn(f64) -> f64) -> f64 {
    let g = 0.618_033_988_749_895_f64;
    let (mut c1, mut c2) = (b - g * (b - a), a + g * (b - a));
    let (mut f1, mut f2) = (f(c1), f(c2));
    for _ in 0..GOLDEN_ITERS {
        if f1 < f2 {
            a = c1;
            c1 = c2;
            f1 = f2;
            c2 = a + g * (b - a);
            f2 = f(c2);
        } else {
            b = c2;
            c2 = c1;
            f2 = f1;
            c1 = b - g * (b - a);
            f1 = f(c1);
        }
    }
    0.5 * (a + b)
}

pub(crate) fn const_offsets_fit(target: &[f64], logcols: &[Vec<f64>], q: usize) -> Vec<f64> {
    let mut out = vec![0.0f64];
    let mut mn = f64::INFINITY;
    for k in 0..q {
        let v = target[k];
        if v.is_finite() && v < mn {
            mn = v;
        }
    }
    if !(mn.is_finite() && mn > 0.0) {
        return out;
    }
    for &frac in OFFSET_FRAC_GRID {
        out.push(frac * mn);
    }
    out.push(1.0);
    out.push(mn);
    let obj = |c: f64| loglin_r2_at(target, c, logcols, q);
    let mut best_c = 0.0f64;
    let mut best_r = obj(0.0);
    let n_coarse = OFFSET_COARSE_STEPS;
    for i in 1..n_coarse {
        let c = mn * (i as f64) / (n_coarse as f64);
        let r = obj(c);
        if r > best_r {
            best_r = r;
            best_c = c;
        }
    }
    if best_c > 0.0 {
        let step = mn / (n_coarse as f64);
        let a = (best_c - step).max(1e-9 * mn);
        let b = (best_c + step).min(mn * (1.0 - 1e-9));
        if b > a {
            let cc = golden_max(a, b, obj);
            if obj(cc) >= best_r {
                best_c = cc;
            }
        }
        out.push(best_c);
    }
    out
}

pub(crate) fn affine_monomial_r2(resid: &[f64], inputs: &[&[f64]], qn: usize) -> f64 {
    affine_monomial_best(resid, inputs, qn).0
}

pub(crate) fn affine_monomial_best(
    resid: &[f64],
    inputs: &[&[f64]],
    qn: usize,
) -> (f64, Option<Vec<f64>>) {
    let q = qn.min(resid.len());
    if q < MONOMIAL_FIT_MIN_ROWS {
        return (0.0, None);
    }
    let mut basis: Vec<Vec<f64>> = Vec::with_capacity(inputs.len());
    for inp in inputs {
        let m = q.min(inp.len());
        let mut col = vec![f64::NAN; q];
        for k in 0..m {
            let v = inp[k].abs();
            col[k] = if v > 1e-300 { v.ln() } else { f64::NAN };
        }
        basis.push(col);
    }
    let kk = basis.len();
    let mut best = 0.0f64;
    let mut best_sol: Option<Vec<f64>> = None;
    for &off in &const_offsets_fit(resid, &basis, q) {
        let mut ylog = vec![f64::NAN; q];
        for k in 0..q {
            let d = (resid[k] - off).abs();
            ylog[k] = if resid[k].is_finite() && d > 1e-300 {
                d.ln()
            } else {
                f64::NAN
            };
        }
        let Some(sol) = solve_ls(&basis, &ylog) else {
            continue;
        };
        let mut mono = vec![f64::NAN; q];
        for r in 0..q {
            let mut lp = 0.0f64;
            let mut ok = true;
            for i in 0..kk {
                if !basis[i][r].is_finite() {
                    ok = false;
                    break;
                }
                lp += sol[i] * basis[i][r];
            }
            mono[r] = if ok && lp < 700.0 { lp.exp() } else { f64::NAN };
        }
        let (r2, _, _) = r2_score(&resid[..q], &mono[..q]);
        if r2 > best {
            best = r2;
            best_sol = Some(sol);
        }
    }
    (best, best_sol)
}

#[allow(dead_code)]
pub(crate) fn monomial_vc_from_sol(
    sol: &[f64],
    inputs: &[&[f64]],
    input_g: &[String],
    input_d: &[String],
    sample_n: usize,
) -> Option<VC> {
    let kk = inputs.len();
    if sol.len() < kk {
        return None;
    }
    let mut exps: Vec<i32> = Vec::with_capacity(kk);
    let mut deg = 0i32;
    for i in 0..kk {
        let e = sol[i];
        let ei = round_clamp_exp(e, &MONO_VCOL);
        deg += ei.abs();
        exps.push(ei);
    }
    if deg == 0 || deg > MONO_VCOL.deg_cap {
        return None;
    }
    let mut col = vec![f64::NAN; sample_n];
    for k in 0..sample_n {
        let mut p = 1.0f64;
        let mut ok = true;
        for i in 0..kk {
            if exps[i] != 0 {
                let v = inputs[i][k];
                if !v.is_finite() {
                    ok = false;
                    break;
                }
                p *= v.powi(exps[i]);
            }
        }
        col[k] = if ok && p.is_finite() { p } else { f64::NAN };
    }
    let finite = col.iter().filter(|x| x.is_finite()).count();
    if finite < min_finite_rows(sample_n) {
        return None;
    }
    let mut num: Vec<String> = Vec::new();
    let mut den: Vec<String> = Vec::new();
    let mut dparts: Vec<String> = Vec::new();
    for i in 0..kk {
        let e = exps[i];
        if e == 0 {
            continue;
        }
        let ae = e.unsigned_abs();
        let base = &input_g[i];
        let g = if ae == 1 {
            format!("({})", base)
        } else {
            format!("({})**{}", base, ae)
        };
        if e > 0 {
            num.push(g);
        } else {
            den.push(g);
        }
        dparts.push(format!("{}^{}", input_d[i], e));
    }
    let nums = if num.is_empty() {
        "1".to_string()
    } else {
        num.join("*")
    };
    let gexpr = if den.is_empty() {
        format!("({})", nums)
    } else {
        format!("({}/({}))", nums, den.join("*"))
    };
    Some(VC {
        gexpr,
        desc: format!("amono({})", dparts.join(",")),
        data: Rc::new(col),
    })
}

pub(crate) fn monomial_strings(exps: &[i32]) -> (String, String) {
    let mut num: Vec<String> = Vec::new();
    let mut den: Vec<String> = Vec::new();
    let mut dparts: Vec<String> = Vec::new();
    for (c, &e) in exps.iter().enumerate() {
        if e == 0 {
            continue;
        }
        let ae = e.unsigned_abs();
        let g = if ae == 1 {
            format!("x{}", c + 1)
        } else {
            format!("(x{})**{}", c + 1, ae)
        };
        if e > 0 {
            num.push(g);
        } else {
            den.push(g);
        }
        dparts.push(format!("c{}^{}", c, e));
    }
    let nums = if num.is_empty() {
        "1".to_string()
    } else {
        num.join("*")
    };
    let gexpr = if den.is_empty() {
        format!("({})", nums)
    } else {
        format!("({}/({}))", nums, den.join("*"))
    };
    (gexpr, format!("mono({})", dparts.join(",")))
}

pub(crate) fn monomial_col_from_sol(
    sol: &[f64],
    features: &[Vec<f64>],
    n_cols: usize,
    sample_n: usize,
) -> Vec<f64> {
    let mut mbuf = vec![f64::NAN; sample_n];
    for k in 0..sample_n {
        let mut p = 1.0f64;
        let mut ok = true;
        for c in 0..n_cols {
            let e = round_clamp_exp(sol[c], &MONO_VCOL);
            if e != 0 {
                let v = features[c][k];
                if v == 0.0 && e < 0 {
                    ok = false;
                    break;
                }
                p *= v.powi(e);
            }
        }
        mbuf[k] = if ok && p.is_finite() { p } else { f64::NAN };
    }
    mbuf
}

pub(crate) fn monomial_gexpr_expanded(sol: &[f64], n_cols: usize) -> String {
    let mut num: Vec<String> = Vec::new();
    let mut den: Vec<String> = Vec::new();
    for c in 0..n_cols {
        let e = round_clamp_exp(sol[c], &MONO_VCOL);
        for _ in 0..e.abs() {
            if e > 0 {
                num.push(format!("x{}", c + 1));
            } else {
                den.push(format!("x{}", c + 1));
            }
        }
    }
    let mg = if num.is_empty() {
        "1".to_string()
    } else {
        num.join("*")
    };
    if den.is_empty() {
        mg
    } else {
        format!("(({})/({}))", mg, den.join("*"))
    }
}

pub(crate) fn fit_shell_const(
    t: &[f64],
    inputs: &[&[f64]],
    qn: usize,
    sample_n: usize,
    shell: impl Fn(f64, f64) -> f64,
) -> (f64, f64) {
    let score = |c: f64| -> f64 {
        let mut buf = vec![f64::NAN; sample_n];
        for k in 0..sample_n {
            buf[k] = if t[k].is_finite() {
                shell(t[k], c)
            } else {
                f64::NAN
            };
        }
        affine_monomial_r2(&buf, inputs, qn).max(quick_probe(&buf, inputs, qn))
    };
    let mut best_c = 1.0f64;
    let mut best_s = score(1.0);
    for e in CONST_DECADE_EXP.0..=CONST_DECADE_EXP.1 {
        for &m in CONST_DECADE_MANTISSA {
            let c = m * 10f64.powi(e);
            let s = score(c);
            if s > best_s {
                best_s = s;
                best_c = c;
            }
        }
    }
    if best_c > 0.0 {
        let lg = best_c.log10();
        let cc = 10f64.powf(golden_max(lg - GOLDEN_BRACKET, lg + GOLDEN_BRACKET, |x| {
            score(10f64.powf(x))
        }));
        let sc = score(cc);
        if sc > best_s {
            best_s = sc;
            best_c = cc;
        }
    }
    let rc = best_c.round();
    for &nice in &[1.0f64, rc, 0.5, 2.0] {
        if nice > 0.0 && (nice - best_c).abs() <= NICE_SNAP_BAND * best_c.max(1.0) {
            let s = score(nice);
            if s >= best_s - NICE_TIE_SLACK {
                return (nice, s);
            }
        }
    }
    (best_c, best_s)
}
