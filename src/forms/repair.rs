use crate::forms::ast::Ast;
use crate::forms::canon::normalize;
use crate::forms::canon::to_ast;
use crate::forms::nf::{Arg, Base, Expr, Fitted, Limits, Term};

pub fn canonical(e: &Expr, inner: &[f64], lim: &Limits) -> Option<Expr> {
    let n = e.n_consts();
    if n > lim.max_consts || e.terms.is_empty() {
        return None;
    }
    let mut inn: Vec<f64> = inner.iter().cloned().take(n).collect();
    while inn.len() < n {
        inn.push(2.0);
    }
    let f = Fitted {
        coef: vec![1.0; e.terms.len()],
        offset: 0.0,
        inner: inn,
    };
    let ast: Ast = to_ast(e, &f);
    let (c, _) = normalize(&ast).ok()?;
    if c.check(lim).is_err() || c.nodes() > lim.max_nodes || c.nest() > lim.max_nest {
        return None;
    }
    Some(c)
}

pub fn factor_drops(e: &Expr, inner: &[f64], lim: &Limits) -> Vec<Expr> {
    let mut lists = Vec::new();
    drop_factors(&e.terms, &mut lists);
    let size = (e.nodes(), e.n_consts());
    let mut out: Vec<Expr> = lists
        .into_iter()
        .filter_map(|t| canonical(&Expr { terms: t }, inner, lim))
        .filter(|c| (c.nodes(), c.n_consts()) < size && c.nodes() <= size.0)
        .collect();
    out.sort();
    out.dedup();
    out.sort_by_key(|c| (c.nodes(), c.n_consts()));
    out
}

fn drop_factors(terms: &[Term], out: &mut Vec<Vec<Term>>) {
    for (i, t) in terms.iter().enumerate() {
        for (j, f) in t.factors.iter().enumerate() {
            let arg = match &f.base {
                Base::Un(_, a) | Base::Group(a) => a,
                Base::Var(_) => continue,
            };
            if t.factors.len() >= 2 {
                let mut ts = terms.to_vec();
                ts[i].factors.remove(j);
                out.push(ts);
            }
            let mut inner = Vec::new();
            drop_factors(&arg.terms, &mut inner);
            for it in inner {
                let mut ts = terms.to_vec();
                let a = Arg {
                    terms: it,
                    scale: arg.scale,
                    shift: arg.shift,
                };
                ts[i].factors[j].base = match &f.base {
                    Base::Un(u, _) => Base::Un(*u, Box::new(a)),
                    _ => Base::Group(Box::new(a)),
                };
                out.push(ts);
            }
        }
    }
}

fn reduce_terms(terms: &[Term], out: &mut Vec<Vec<Term>>) {
    if terms.len() >= 2 {
        for i in 0..terms.len() {
            let mut t = terms.to_vec();
            t.remove(i);
            out.push(t);
        }
    }
    for (i, t) in terms.iter().enumerate() {
        if t.coef {
            let mut ts = terms.to_vec();
            ts[i].coef = false;
            out.push(ts);
        }
        for (j, f) in t.factors.iter().enumerate() {
            let (arg, un) = match &f.base {
                Base::Un(u, a) => (a, Some(*u)),
                Base::Group(a) => (a, None),
                Base::Var(_) => continue,
            };
            let rebuild = |a: Arg| -> Vec<Term> {
                let mut ts = terms.to_vec();
                ts[i].factors[j].base = match un {
                    Some(u) => Base::Un(u, Box::new(a)),
                    None => Base::Group(Box::new(a)),
                };
                ts
            };
            if un.is_some() {
                let mut ts = terms.to_vec();
                ts[i].factors[j].base = Base::Group(arg.clone());
                out.push(ts);
            }
            if arg.scale {
                out.push(rebuild(Arg {
                    terms: arg.terms.clone(),
                    scale: false,
                    shift: arg.shift,
                }));
            }
            if arg.shift {
                out.push(rebuild(Arg {
                    terms: arg.terms.clone(),
                    scale: arg.scale,
                    shift: false,
                }));
            }
            let mut inner = Vec::new();
            reduce_terms(&arg.terms, &mut inner);
            for lt in inner {
                out.push(rebuild(Arg {
                    terms: lt,
                    scale: arg.scale,
                    shift: arg.shift,
                }));
            }
        }
    }
}

pub fn reductions(e: &Expr, inner: &[f64], lim: &Limits) -> Vec<Expr> {
    let mut lists = Vec::new();
    reduce_terms(&e.terms, &mut lists);
    let size = (e.nodes(), e.n_consts());
    let mut out: Vec<Expr> = lists
        .into_iter()
        .filter_map(|t| canonical(&Expr { terms: t }, inner, lim))
        .filter(|c| (c.nodes(), c.n_consts()) < size && c.nodes() <= size.0)
        .collect();
    out.sort();
    out.dedup();
    out.sort_by_key(|c| (c.nodes(), c.n_consts()));
    out
}
