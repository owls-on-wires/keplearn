use crate::config::{LOGLIN_MAX_TERMS, LOGLIN_SINGLES, REL_FIT_SPAN, REL_FIT_SPAN_MAX};

pub(crate) fn proportional(target: &[f64]) -> bool {
    let v: Vec<f64> = target.iter().copied().filter(|v| v.is_finite()).collect();
    if v.len() < 5 || !v.iter().all(|x| *x > 0.0) {
        return false;
    }
    let (lo, hi) = v
        .iter()
        .fold((f64::INFINITY, 0.0f64), |(l, h), &x| (l.min(x), h.max(x)));
    hi >= REL_FIT_SPAN * lo && hi <= REL_FIT_SPAN_MAX * lo
}

#[derive(Clone, Copy, PartialEq)]
enum Term {
    Log(usize),
    Lin(usize),
    Shifted(usize, f64),
}

impl Term {
    fn col(&self) -> usize {
        match *self {
            Term::Log(c) | Term::Lin(c) | Term::Shifted(c, _) => c,
        }
    }

    fn value(&self, x: f64) -> f64 {
        match *self {
            Term::Log(_) => x.ln(),
            Term::Lin(_) => x,
            Term::Shifted(_, e) => (x - e).ln(),
        }
    }

    fn write(&self, b: f64) -> String {
        match *self {
            Term::Log(c) => format!("exp({:e}*log(x{}))", b, c + 1),
            Term::Lin(c) => format!("exp({:e}*x{})", b, c + 1),
            Term::Shifted(c, e) => {
                let shift = if e < 0.0 {
                    format!("x{} + {:e}", c + 1, -e)
                } else {
                    format!("x{} - {:e}", c + 1, e)
                };
                format!("exp({:e}*log({}))", b, shift)
            }
        }
    }
}

struct LogFit {
    terms: Vec<Term>,
    ssr: f64,
    model: String,
}

fn fit_terms(terms: &[Term], cols: &[&[f64]], ly: &[f64], rows: &[usize]) -> Option<LogFit> {
    let g: Vec<Vec<f64>> = terms
        .iter()
        .map(|t| rows.iter().map(|&r| t.value(cols[t.col()][r])).collect())
        .collect();
    if g.iter().any(|c| c.iter().any(|v| !v.is_finite())) {
        return None;
    }
    let y: Vec<f64> = rows.iter().map(|&r| ly[r]).collect();
    let (coef, a) = crate::forms::fit::linear_fit(&g, &y)?;
    if !a.is_finite() || coef.iter().any(|b| !b.is_finite()) {
        return None;
    }
    let ssr: f64 = (0..y.len())
        .map(|k| {
            let p = a + coef.iter().zip(&g).map(|(b, c)| b * c[k]).sum::<f64>();
            (y[k] - p).powi(2)
        })
        .sum();
    let scale = a.exp();
    if !(scale.is_finite() && scale > 0.0) {
        return None;
    }
    let mut model = format!("{:e}", scale);
    for (t, b) in terms.iter().zip(&coef) {
        model.push('*');
        model.push_str(&t.write(*b));
    }
    Some(LogFit {
        terms: terms.to_vec(),
        ssr,
        model,
    })
}

fn cost(f: &LogFit, m: usize) -> f64 {
    let m = m as f64;
    0.5 * m * (f.ssr.max(1e-300) / m).ln() + 0.5 * m.ln() * (f.terms.len() + 1) as f64
}

fn shifted(c: usize, cols: &[&[f64]], ly: &[f64], rows: &[usize]) -> Option<LogFit> {
    let xs: Vec<f64> = rows.iter().map(|&r| cols[c][r]).collect();
    let (lo, hi) = xs
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &x| {
            (l.min(x), h.max(x))
        });
    let span = hi - lo;
    if !(span > 0.0) {
        return None;
    }
    let at = |ln_d: f64| fit_terms(&[Term::Shifted(c, lo - ln_d.exp())], cols, ly, rows);
    let ssr = |ln_d: f64| at(ln_d).map_or(f64::INFINITY, |f| f.ssr);
    let (a0, b0) = ((1e-3 * span).ln(), (10.0 * span).ln());
    let steps = 24;
    let mut best = (f64::INFINITY, a0);
    for k in 0..=steps {
        let x = a0 + (b0 - a0) * k as f64 / steps as f64;
        let s = ssr(x);
        if s < best.0 {
            best = (s, x);
        }
    }
    let h = (b0 - a0) / steps as f64;
    let (mut a, mut b) = (best.1 - h, best.1 + h);
    let g = 0.5 * (5f64.sqrt() - 1.0);
    let (mut x1, mut x2) = (b - g * (b - a), a + g * (b - a));
    let (mut f1, mut f2) = (ssr(x1), ssr(x2));
    for _ in 0..30 {
        if f1 < f2 {
            b = x2;
            x2 = x1;
            f2 = f1;
            x1 = b - g * (b - a);
            f1 = ssr(x1);
        } else {
            a = x1;
            x1 = x2;
            f1 = f2;
            x2 = a + g * (b - a);
            f2 = ssr(x2);
        }
    }
    let x = if f1 < f2 { x1 } else { x2 };
    at(x)
}

pub(crate) fn loglin_finalists(
    features: &[Vec<f64>],
    n_cols: usize,
    target: &[f64],
    n: usize,
) -> Vec<(String, f64)> {
    let n = n.min(target.len());
    let cols: Vec<&[f64]> = features[..n_cols].iter().map(|c| &c[..n]).collect();
    let rows: Vec<usize> = (0..n)
        .filter(|&r| {
            target[r].is_finite() && target[r] > 0.0 && cols.iter().all(|c| c[r].is_finite())
        })
        .collect();
    let m = rows.len();
    if m < 5 {
        return Vec::new();
    }
    let mut ly = vec![f64::NAN; n];
    for &r in &rows {
        ly[r] = target[r].ln();
    }
    let mut pool: Vec<Term> = Vec::new();
    for (c, col) in cols.iter().enumerate() {
        let (lo, hi) = rows
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &r| {
                (l.min(col[r]), h.max(col[r]))
            });
        if !(hi > lo) {
            continue;
        }
        if lo > 0.0 {
            pool.push(Term::Log(c));
        }
        pool.push(Term::Lin(c));
    }
    let mut singles: Vec<LogFit> = pool
        .iter()
        .filter_map(|t| fit_terms(&[*t], &cols, &ly, &rows))
        .collect();
    singles.sort_by(|a, b| a.ssr.total_cmp(&b.ssr));
    let mut out: Vec<LogFit> = Vec::new();
    if let Some(first) = singles.first() {
        let mut cur = fit_terms(&first.terms, &cols, &ly, &rows);
        while let Some(c) = cur.take() {
            if c.terms.len() >= LOGLIN_MAX_TERMS {
                out.push(c);
                break;
            }
            let next = pool
                .iter()
                .filter(|t| !c.terms.iter().any(|u| u.col() == t.col()))
                .filter_map(|t| {
                    let mut ts = c.terms.clone();
                    ts.push(*t);
                    fit_terms(&ts, &cols, &ly, &rows)
                })
                .min_by(|a, b| a.ssr.total_cmp(&b.ssr));
            let more = next.filter(|nx| cost(nx, m) < cost(&c, m));
            out.push(c);
            cur = more;
        }
    }
    if let Some(p) = singles.iter().find(|f| matches!(f.terms[0], Term::Log(_))) {
        if let Some(s) = shifted(p.terms[0].col(), &cols, &ly, &rows) {
            if cost(&s, m) + 0.5 * (m as f64).ln() < cost(p, m) {
                out.push(s);
            }
        }
    }
    for s in singles.into_iter().take(LOGLIN_SINGLES) {
        if !out.iter().any(|o| o.terms == s.terms) {
            out.push(s);
        }
    }
    let y: Vec<f64> = rows.iter().map(|&r| target[r]).collect();
    out.into_iter()
        .filter_map(|f| {
            let ast = crate::forms::ast::parse(&f.model).ok()?;
            let all: Vec<Vec<f64>> = features
                .iter()
                .map(|c| rows.iter().map(|&r| c[r]).collect())
                .collect();
            let p: Vec<f64> = (0..m).map(|k| ast.eval(&all, k)).collect();
            if p.iter().any(|v| !v.is_finite()) {
                return None;
            }
            let r2 = crate::forms::fit::r2(&y, &p);
            r2.is_finite().then_some((f.model, r2))
        })
        .collect()
}
