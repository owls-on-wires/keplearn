use crate::forms::fit::{fit_expr, FitConfig, FitResult};
use crate::forms::nf::{Expr, Term};

pub struct Reduced {
    pub expr: Expr,
    pub fit: FitResult,
}

pub fn library(d: usize, max_nodes: usize, cap: usize) -> Option<Vec<Expr>> {
    use crate::forms::grammar::{children, Step};
    let lim = crate::forms::nf::Limits {
        n_cols: d,
        max_nodes,
        max_nest: 1,
        max_consts: 0,
        max_terms: 2,
        max_factors: 4,
        max_arg_nodes: max_nodes,
        exps: crate::forms::nf::ExpSet::Core,
    };
    let mut seen: std::collections::HashSet<Expr> = std::collections::HashSet::new();
    let mut stack = vec![crate::forms::grammar::root()];
    while let Some(st) = stack.pop() {
        if st.top.len() > 1 || (st.top.len() == 1 && !st.stack.is_empty()) {
            continue;
        }
        for step in children(&lim, &st) {
            match step {
                Step::Partial(s) => stack.push(s),
                Step::Complete(e) => {
                    if e.terms.len() == 1 && seen.insert(e) && seen.len() > cap {
                        return None;
                    }
                }
            }
        }
    }
    let mut out: Vec<Expr> = seen.into_iter().collect();
    out.sort();
    Some(out)
}

pub const BASIS_LIB_CAP: usize = 1000;

pub const BASIS_ATOMS: usize = 4000;

pub const PRODUCT_BUDGET: usize = 3_000_000;

pub const PRODUCT_SUPPORT: usize = 3;

pub const PRODUCT_U_NODES: usize = 6;

pub const PRODUCT_ROWS: usize = 64;

pub const PRODUCT_STEPS: usize = 4;

pub const PRODUCT_KEEP: usize = 100;

pub const PRODUCT_KEEP_U: usize = 400;

pub const PRODUCT_KEEP_M: usize = 200;

fn library_cached(d: usize, cap: usize) -> Option<std::sync::Arc<Vec<Expr>>> {
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<
            std::collections::HashMap<(usize, usize), Option<std::sync::Arc<Vec<Expr>>>>,
        >,
    > = std::sync::OnceLock::new();
    let m = CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Some(v) = m.lock().unwrap().get(&(d, cap)) {
        return v.clone();
    }
    let v = library_capped(d, cap).map(std::sync::Arc::new);
    m.lock().unwrap().insert((d, cap), v.clone());
    v
}

pub const MONOMIAL_BUDGET: usize = 300_000;

fn product_support(d: usize) -> usize {
    let nq = crate::forms::nf::Q::CORE.len();
    let count = |s: usize| -> usize {
        let (mut total, mut choose, mut pow) = (0usize, 1usize, 1usize);
        for j in 0..=s.min(d) {
            if j > 0 {
                choose = choose.saturating_mul(d - j + 1) / j;
                pow = pow.saturating_mul(nq);
            }
            total = total.saturating_add(choose.saturating_mul(pow));
        }
        total
    };
    (1..=PRODUCT_SUPPORT.min(d))
        .rev()
        .find(|&s| count(s) <= MONOMIAL_BUDGET)
        .unwrap_or(1)
}

fn monomial_factors(d: usize, support: usize) -> Vec<Vec<crate::forms::nf::Factor>> {
    let mut out: Vec<Vec<crate::forms::nf::Factor>> = vec![Vec::new()];
    for c in 0..d {
        let mut next = out.clone();
        for m in &out {
            if m.len() >= support {
                continue;
            }
            for q in crate::forms::nf::Q::CORE {
                let mut f = m.clone();
                f.push(crate::forms::nf::Factor::var_q(c as u8, q));
                next.push(f);
            }
        }
        out = next;
    }
    out
}

type Cache<T> = std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<(u64, u8), std::sync::Arc<Option<T>>>>,
>;

static SCREEN_CACHE: Cache<ScreenTables> = std::sync::OnceLock::new();

static ATOM_CACHE: Cache<AtomSet> = std::sync::OnceLock::new();

static RATIONAL_CACHE: Cache<RationalBase> = std::sync::OnceLock::new();

fn fingerprint(sub: &[Vec<f64>]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    sub.len().hash(&mut h);
    for c in sub {
        c.len().hash(&mut h);
        for v in c {
            v.to_bits().hash(&mut h);
        }
    }
    h.finish()
}

fn cached<T>(
    cache: &Cache<T>,
    key: u64,
    build: impl FnOnce() -> Option<T>,
) -> std::sync::Arc<Option<T>> {
    cached_tag(cache, (key, 0), build)
}

fn cached_tag<T>(
    cache: &Cache<T>,
    key: (u64, u8),
    build: impl FnOnce() -> Option<T>,
) -> std::sync::Arc<Option<T>> {
    let m = cache.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Some(v) = m.lock().unwrap().get(&key) {
        return v.clone();
    }
    let v = std::sync::Arc::new(build());
    m.lock().unwrap().insert(key, v.clone());
    v
}

struct AtomSet {
    keep: Vec<Term>,
    prep: crate::forms::pursuit::Prep,
    keys: std::collections::HashSet<Vec<i64>>,
}

fn atom_key(v: &[f64]) -> Option<Vec<i64>> {
    if !v.iter().all(|t| t.is_finite()) {
        return None;
    }
    let m = v.iter().sum::<f64>() / v.len() as f64;
    let var = v.iter().map(|t| (t - m) * (t - m)).sum::<f64>() / v.len() as f64;
    if !(var > 1e-24 * (m * m).max(1e-300)) {
        return None;
    }
    let sd = (var * v.len() as f64).sqrt();
    let sign = v
        .iter()
        .map(|t| t - m)
        .find(|c| c.abs() > 1e-9 * sd)
        .map_or(1.0, |c| c.signum());
    Some(
        v.iter()
            .map(|t| ((t - m) / sd * sign * 1e8).round() as i64)
            .collect(),
    )
}

fn new_atoms(
    sub: &[Vec<f64>],
    terms: impl IntoIterator<Item = Term>,
    keys: &mut std::collections::HashSet<Vec<i64>>,
    cap: usize,
) -> (Vec<Term>, Vec<Vec<f64>>) {
    let (mut ts, mut vs) = (Vec::new(), Vec::new());
    for t in terms {
        if keys.len() >= cap {
            break;
        }
        let e = Expr {
            terms: vec![t.clone()],
        };
        let Some(v) = crate::forms::nf::eval_terms(&e, sub, &[])
            .into_iter()
            .next()
        else {
            continue;
        };
        let Some(k) = atom_key(&v) else { continue };
        if keys.insert(k) {
            ts.push(t);
            vs.push(v);
        }
    }
    (ts, vs)
}

fn stage_set(sub: &[Vec<f64>], stage: u8) -> Option<AtomSet> {
    let d = sub.len();
    let lib = library_cached(d, if stage == 0 { 4000 } else { BASIS_LIB_CAP })?;
    let mut terms: Vec<Term> = lib.iter().map(|e| e.terms[0].clone()).collect();
    if stage != 1 {
        terms.extend(monomial_terms(sub));
    }
    let mut keys = std::collections::HashSet::new();
    let (keep, vals) = new_atoms(sub, terms, &mut keys, BASIS_ATOMS);
    Some(AtomSet {
        keep,
        prep: crate::forms::pursuit::Prep::new(vals),
        keys,
    })
}

fn monomial_terms(sub: &[Vec<f64>]) -> Vec<Term> {
    let d = sub.len();
    monomial_factors(d, product_support(d))
        .into_iter()
        .filter(|f| !f.is_empty())
        .map(|factors| Term {
            neg: false,
            coef: false,
            factors,
        })
        .collect()
}

fn unary_factors(d: usize, nodes: usize) -> Option<std::sync::Arc<Vec<crate::forms::nf::Factor>>> {
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<
            std::collections::HashMap<
                (usize, usize),
                Option<std::sync::Arc<Vec<crate::forms::nf::Factor>>>,
            >,
        >,
    > = std::sync::OnceLock::new();
    let m = CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Some(v) = m.lock().unwrap().get(&(d, nodes)) {
        return v.clone();
    }
    let v = library(d, nodes, 1_000_000).map(|l| {
        std::sync::Arc::new(
            l.into_iter()
                .filter_map(|e| {
                    let f = &e.terms[0].factors;
                    (f.len() == 1 && !matches!(f[0].base, crate::forms::nf::Base::Var(_)))
                        .then(|| f[0].clone())
                })
                .collect::<Vec<_>>(),
        )
    });
    m.lock().unwrap().insert((d, nodes), v.clone());
    v
}

struct ScreenTables {
    srows: Vec<usize>,
    mf: Vec<Vec<crate::forms::nf::Factor>>,
    mv: Vec<Vec<f64>>,
    ufk: Vec<Option<crate::forms::nf::Factor>>,
    uv: Vec<Vec<f64>>,
    m32: Vec<f32>,
    u32_: Vec<f32>,
    s1: Vec<f32>,
    s2: Vec<f32>,
}

fn screen_tables(sub: &[Vec<f64>]) -> Option<ScreenTables> {
    use crate::forms::nf::Factor;
    let d = sub.len();
    let n = sub.first().map_or(0, |c| c.len());
    let srows: Vec<usize> = if n > PRODUCT_ROWS {
        (0..PRODUCT_ROWS).map(|k| k * n / PRODUCT_ROWS).collect()
    } else {
        (0..n).collect()
    };
    let ns = srows.len();
    if ns < 8 {
        return None;
    }
    let finite = |v: &[f64]| v.iter().all(|t| t.is_finite());
    let mut mf: Vec<Vec<Factor>> = Vec::new();
    let mut mv: Vec<Vec<f64>> = Vec::new();
    for f in monomial_factors(d, product_support(d)) {
        let v: Vec<f64> = srows
            .iter()
            .map(|&r| {
                f.iter().fold(1.0, |a, fa| match fa.base {
                    crate::forms::nf::Base::Var(c) => a * sub[c as usize][r].powf(fa.q.val()),
                    _ => a,
                })
            })
            .collect();
        if finite(&v) {
            mf.push(f);
            mv.push(v);
        }
    }
    let mut uf: Vec<Option<Factor>> = vec![None];
    for nodes in (2..=PRODUCT_U_NODES).rev() {
        let Some(u) = unary_factors(d, nodes) else {
            continue;
        };
        if mf.len() * (u.len() + 1) <= PRODUCT_BUDGET {
            uf.extend(u.iter().cloned().map(Some));
            break;
        }
    }
    let mut uv: Vec<Vec<f64>> = Vec::new();
    let mut ufk: Vec<Option<Factor>> = Vec::new();
    let ssub: Vec<Vec<f64>> = sub
        .iter()
        .map(|c| srows.iter().map(|&r| c[r]).collect())
        .collect();
    for u in uf {
        let v = match &u {
            None => vec![1.0; ns],
            Some(f) => {
                let e = Expr {
                    terms: vec![Term {
                        neg: false,
                        coef: false,
                        factors: vec![f.clone()],
                    }],
                };
                match crate::forms::nf::eval_terms(&e, &ssub, &[])
                    .into_iter()
                    .next()
                {
                    Some(v) => v,
                    None => continue,
                }
            }
        };
        if finite(&v) {
            uv.push(v);
            ufk.push(u);
        }
    }
    let (nm, nu) = (mv.len(), uv.len());
    let m32: Vec<f32> = mv
        .iter()
        .flat_map(|v| v.iter().map(|&x| x as f32))
        .collect();
    let u32_: Vec<f32> = uv
        .iter()
        .flat_map(|v| v.iter().map(|&x| x as f32))
        .collect();
    let mut s1 = vec![0f32; nm * nu];
    let mut s2 = vec![0f32; nm * nu];
    for i in 0..nm {
        let mi = &m32[i * ns..(i + 1) * ns];
        let m2: Vec<f32> = mi.iter().map(|x| x * x).collect();
        for j in 0..nu {
            let uj = &u32_[j * ns..(j + 1) * ns];
            let (mut a, mut b) = (0f32, 0f32);
            for k in 0..ns {
                a += mi[k] * uj[k];
                b += m2[k] * uj[k] * uj[k];
            }
            s1[i * nu + j] = a;
            s2[i * nu + j] = b;
        }
    }
    Some(ScreenTables {
        srows,
        mf,
        mv,
        ufk,
        uv,
        m32,
        u32_,
        s1,
        s2,
    })
}

fn product_screen(sub: &[Vec<f64>], ys: &[f64], have: &[Term]) -> Vec<Term> {
    let t = cached(&SCREEN_CACHE, fingerprint(sub), || screen_tables(sub));
    let Some(ScreenTables {
        srows,
        mf,
        mv,
        ufk,
        uv,
        m32,
        u32_,
        s1,
        s2,
    }) = &*t
    else {
        return Vec::new();
    };
    let ns = srows.len();
    let (nm, nu) = (mv.len(), uv.len());
    let ysr: Vec<f64> = srows.iter().map(|&r| ys[r]).collect();
    let mut chosen: Vec<(usize, usize)> = Vec::new();
    let mut picked: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
    let mut resid = ysr.clone();
    let mut step_best: Vec<Vec<f64>> = Vec::new();
    for _ in 0..PRODUCT_STEPS {
        if crate::out_of_time() {
            break;
        }
        let rm = resid.iter().sum::<f64>() / ns as f64;
        let rc: Vec<f32> = resid.iter().map(|v| (v - rm) as f32).collect();
        let rr: f32 = rc.iter().map(|v| v * v).sum();
        if !(rr > 0.0) {
            break;
        }
        let mut best_u: Vec<(f32, usize)> = vec![(-1.0, 0); nu];
        let mut best_m: Vec<(f32, usize)> = vec![(-1.0, 0); nm];
        let mut heap: std::collections::BinaryHeap<std::cmp::Reverse<(u32, usize, usize)>> =
            std::collections::BinaryHeap::new();
        for i in 0..nm {
            let mi = &m32[i * ns..(i + 1) * ns];
            let w: Vec<f32> = mi.iter().zip(&rc).map(|(a, b)| a * b).collect();
            for j in 0..nu {
                let uj = &u32_[j * ns..(j + 1) * ns];
                let num: f32 = w.iter().zip(uj).map(|(a, b)| a * b).sum();
                let var = s2[i * nu + j] - s1[i * nu + j] * s1[i * nu + j] / ns as f32;
                if !(var > 0.0) {
                    continue;
                }
                let c2 = num * num / (var * rr);
                if !c2.is_finite() {
                    continue;
                }
                if c2 > best_u[j].0 {
                    best_u[j] = (c2, i);
                }
                if c2 > best_m[i].0 {
                    best_m[i] = (c2, j);
                }
                let key = c2.to_bits();
                if heap.len() < PRODUCT_KEEP {
                    heap.push(std::cmp::Reverse((key, i, j)));
                } else if key > heap.peek().unwrap().0 .0 {
                    heap.pop();
                    heap.push(std::cmp::Reverse((key, i, j)));
                }
            }
        }
        let mut top: Vec<(u32, usize, usize)> = heap.into_iter().map(|r| r.0).collect();
        top.sort_by(|a, b| b.0.cmp(&a.0));
        let mut bu: Vec<(f32, usize, usize)> = best_u
            .iter()
            .enumerate()
            .filter(|(_, b)| b.0 >= 0.0)
            .map(|(j, b)| (b.0, b.1, j))
            .collect();
        bu.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut bm: Vec<(f32, usize, usize)> = best_m
            .iter()
            .enumerate()
            .filter(|(_, b)| b.0 >= 0.0)
            .map(|(i, b)| (b.0, i, b.1))
            .collect();
        bm.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (i, j) in top
            .iter()
            .map(|t| (t.1, t.2))
            .chain(bu.iter().take(PRODUCT_KEEP_U).map(|t| (t.1, t.2)))
            .chain(bm.iter().take(PRODUCT_KEEP_M).map(|t| (t.1, t.2)))
        {
            if picked.insert((i, j)) {
                chosen.push((i, j));
            }
        }
        let Some(&(_, bi, bj)) = top.first() else {
            break;
        };
        step_best.push(mv[bi].iter().zip(&uv[bj]).map(|(a, c)| a * c).collect());
        let Some((coef, off)) = crate::forms::fit::linear_fit(&step_best, &ysr) else {
            break;
        };
        for r in 0..ns {
            resid[r] = ysr[r]
                - off
                - coef
                    .iter()
                    .zip(&step_best)
                    .map(|(c, g)| c * g[r])
                    .sum::<f64>();
        }
    }
    let all = chosen;
    let have: std::collections::HashSet<&Term> = have.iter().collect();
    let mut out = Vec::new();
    for (i, j) in all {
        let mut factors = mf[i].clone();
        if let Some(u) = &ufk[j] {
            factors.push(u.clone());
        }
        if factors.is_empty() {
            continue;
        }
        let t = Term {
            neg: false,
            coef: false,
            factors,
        };
        if !have.contains(&t) {
            out.push(t);
        }
    }
    out
}

pub const BASIS_MAX_TERMS: usize = 6;

pub const RATIONAL_LIB_CAP: usize = 500;

fn library_capped(d: usize, cap: usize) -> Option<Vec<Expr>> {
    let mut lib = None;
    for max_nodes in 1..=16 {
        match library(d, max_nodes, cap) {
            Some(l) => lib = Some(l),
            None => break,
        }
    }
    lib
}

struct RationalBase {
    names: Vec<String>,
    vals: Vec<Vec<f64>>,
    prep: crate::forms::pursuit::Prep,
}

fn rational_base(sub: &[Vec<f64>]) -> Option<RationalBase> {
    let d = sub.len();
    let lib = library_capped(d, RATIONAL_LIB_CAP)?;
    let plain = |e: &Expr| {
        crate::forms::nf::print_fitted(
            e,
            &crate::forms::nf::Fitted {
                coef: vec![1.0],
                offset: 0.0,
                inner: vec![],
            },
        )
    };
    let mut vals: Vec<Vec<f64>> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for e in &lib {
        let Some(v) = crate::forms::nf::eval_terms(e, sub, &[]).into_iter().next() else {
            continue;
        };
        if !v.iter().all(|t| t.is_finite()) {
            continue;
        }
        let m = v.iter().sum::<f64>() / v.len() as f64;
        let var = v.iter().map(|t| (t - m) * (t - m)).sum::<f64>() / v.len() as f64;
        if !(var > 1e-24 * (m * m).max(1e-300)) {
            continue;
        }
        vals.push(v);
        names.push(plain(e));
    }
    let prep = crate::forms::pursuit::Prep::new(vals.clone());
    Some(RationalBase { names, vals, prep })
}

pub fn rational(
    cols: &[Vec<f64>],
    y: &[f64],
    x_fit: &[Vec<f64>],
    y_fit: &[f64],
    x_ho: &[Vec<f64>],
    y_ho: &[f64],
    cfg: &FitConfig,
) -> Option<Reduced> {
    let d = cols.len();
    let n = y.len();
    let rows: Vec<usize> = if n > 256 {
        (0..256).map(|k| k * n / 256).collect()
    } else {
        (0..n).collect()
    };
    let sub: Vec<Vec<f64>> = cols
        .iter()
        .map(|c| rows.iter().map(|&r| c[r]).collect())
        .collect();
    let ys: Vec<f64> = rows.iter().map(|&r| y[r]).collect();
    if !ys.iter().all(|v| v.is_finite()) {
        return None;
    }
    let rb = cached_tag(&RATIONAL_CACHE, (fingerprint(&sub), 0), || {
        rational_base(&sub)
    });
    let Some(RationalBase {
        names,
        vals,
        prep: fprep,
    }) = &*rb
    else {
        return None;
    };
    let nf_ = vals.len();
    let lim = crate::forms::nf::Limits::full(d);
    let g0s: Vec<Option<usize>> = std::iter::once(None).chain((0..d).map(Some)).collect();
    for g0 in g0s {
        let g0v: Vec<f64> = match g0 {
            None => vec![1.0; ys.len()],
            Some(c) => sub[c].clone(),
        };
        let target: Vec<f64> = ys.iter().zip(&g0v).map(|(a, b)| a * b).collect();
        let g0name = match g0 {
            None => "1".to_string(),
            Some(c) => format!("x{}", c + 1),
        };
        let mut den_vals: Vec<Vec<f64>> = Vec::new();
        for (v, nm) in vals.iter().zip(names.iter()) {
            if *nm == g0name {
                continue;
            }
            den_vals.push(v.iter().zip(&ys).map(|(a, b)| a * b).collect());
        }
        let prep = fprep.extend(den_vals);
        let atoms = &prep.atoms;
        let den_names: Vec<&String> = names.iter().filter(|nm| **nm != g0name).collect();
        let mut allowed = vec![true; atoms.len()];
        let mut found = None;
        for _ in 0..4 {
            if atoms.len() > 4000 {
                break;
            }
            let Some(ch) = crate::forms::pursuit::pursue_masked(
                &prep,
                &target,
                BASIS_MAX_TERMS,
                2,
                Some(&allowed),
            ) else {
                break;
            };
            if ch.iter().any(|&k| k < nf_) {
                if ch.iter().any(|&k| k >= nf_) {
                    found = Some(ch);
                }
                break;
            }
            for &k in &ch {
                allowed[k] = false;
            }
        }
        let Some(chosen) = found else { continue };
        let g: Vec<Vec<f64>> = chosen.iter().map(|&k| atoms[k].clone()).collect();
        let Some((coef, off)) = crate::forms::fit::linear_fit(&g, &target) else {
            continue;
        };
        let mut num = vec![crate::forms::nf::fmt_num(off)];
        let mut den = vec![g0name.clone()];
        for (&k, c) in chosen.iter().zip(&coef) {
            if k < nf_ {
                num.push(format!("{}*({})", crate::forms::nf::fmt_num(*c), names[k]));
            } else {
                den.push(format!(
                    "{}*({})",
                    crate::forms::nf::fmt_num(-*c),
                    den_names[k - nf_]
                ));
            }
        }
        let src = format!("({})/({})", num.join(" + "), den.join(" + "));
        let Ok(ast) = crate::forms::ast::parse(&src) else {
            continue;
        };
        let Ok((e, _)) = crate::forms::canon::normalize(&ast) else {
            continue;
        };
        if e.check(&lim).is_err() {
            continue;
        }
        let Some(mut fr) = fit_expr(&e, x_fit, y_fit, x_ho, y_ho, cfg) else {
            continue;
        };
        if !fr.is_exact(cfg) {
            continue;
        }
        crate::forms::search::snap(
            &e,
            &mut fr,
            x_fit,
            y_fit,
            x_ho,
            y_ho,
            crate::forms::search::SNAP_TOL,
        );
        return Some(Reduced { expr: e, fit: fr });
    }
    None
}

pub fn basis_ext(
    cols: &[Vec<f64>],
    y: &[f64],
    x_fit: &[Vec<f64>],
    y_fit: &[f64],
    x_ho: &[Vec<f64>],
    y_ho: &[f64],
    cfg: &FitConfig,
    products: bool,
) -> Option<Reduced> {
    let d = cols.len();
    let n = y.len();
    let rows: Vec<usize> = if n > 256 {
        (0..256).map(|k| k * n / 256).collect()
    } else {
        (0..n).collect()
    };
    let sub: Vec<Vec<f64>> = cols
        .iter()
        .map(|c| rows.iter().map(|&r| c[r]).collect())
        .collect();
    let ys: Vec<f64> = rows.iter().map(|&r| y[r]).collect();
    if !ys.iter().all(|v| v.is_finite()) {
        return None;
    }
    let fp = fingerprint(&sub);
    let mut screened: Option<Vec<Term>> = None;
    for stage in 0u8..3 {
        if stage >= 1 && !products {
            break;
        }
        if crate::out_of_time() {
            return None;
        }
        let set = cached_tag(&ATOM_CACHE, (fp, stage), || stage_set(&sub, stage));
        let Some(set) = &*set else { continue };
        let found = if stage == 0 {
            crate::forms::pursuit::pursue(&set.prep, &ys, BASIS_MAX_TERMS, 1).map(|ch| {
                ch.into_iter()
                    .map(|k| set.keep[k].clone())
                    .collect::<Vec<_>>()
            })
        } else {
            if screened.is_none() {
                let base = cached_tag(&ATOM_CACHE, (fp, 1), || stage_set(&sub, 1));
                let have: Vec<Term> = (*base).as_ref().map_or(Vec::new(), |b| b.keep.clone());
                screened = Some(product_screen(&sub, &ys, &have));
            }
            let mut keys = set.keys.clone();
            let (ts, vs) = new_atoms(
                &sub,
                screened.iter().flatten().cloned(),
                &mut keys,
                BASIS_ATOMS,
            );
            let prep = set.prep.extend(vs);
            crate::forms::pursuit::pursue(&prep, &ys, BASIS_MAX_TERMS, 1).map(|ch| {
                ch.into_iter()
                    .map(|k| {
                        if k < set.keep.len() {
                            set.keep[k].clone()
                        } else {
                            ts[k - set.keep.len()].clone()
                        }
                    })
                    .collect::<Vec<_>>()
            })
        };
        let Some(terms) = found else { continue };
        let joined = Expr { terms };
        let Some(e) =
            crate::forms::repair::canonical(&joined, &[], &crate::forms::nf::Limits::full(d))
        else {
            continue;
        };
        let Some(mut fr) = fit_expr(&e, x_fit, y_fit, x_ho, y_ho, cfg) else {
            continue;
        };
        if !fr.is_exact(cfg) {
            continue;
        }
        crate::forms::search::snap(
            &e,
            &mut fr,
            x_fit,
            y_fit,
            x_ho,
            y_ho,
            crate::forms::search::SNAP_TOL,
        );
        return Some(Reduced { expr: e, fit: fr });
    }
    None
}
