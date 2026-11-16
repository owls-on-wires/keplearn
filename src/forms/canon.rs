use crate::forms::ast::Ast;
use crate::forms::nf::{Arg, Base, Expr, Factor, Fitted, Term, Un, Q};

const EPS: f64 = 1e-9;

#[derive(Clone, Debug)]
pub struct ATerm {
    pub neg: bool,
    pub coef: Option<f64>,
    pub factors: Vec<AFactor>,
}

#[derive(Clone, Debug)]
pub struct AFactor {
    pub base: ABase,
    pub q: Q,
}

#[derive(Clone, Debug)]
pub enum ABase {
    Var(u8),
    Un(Un, Box<AArg>),
    Group(Box<AArg>),
}

#[derive(Clone, Debug)]
pub struct AArg {
    pub terms: Vec<ATerm>,
    pub scale: Option<f64>,
    pub shift: Option<f64>,
}

impl ATerm {
    pub fn strip(&self) -> Term {
        Term {
            neg: self.neg,
            coef: self.coef.is_some(),
            factors: self.factors.iter().map(|f| f.strip()).collect(),
        }
    }

    fn inner(&self, out: &mut Vec<f64>) {
        if let Some(k) = self.coef {
            out.push(k);
        }
        for f in &self.factors {
            f.inner(out);
        }
    }
}

impl AFactor {
    pub fn strip(&self) -> Factor {
        Factor {
            base: self.base.strip(),
            q: self.q,
        }
    }

    fn inner(&self, out: &mut Vec<f64>) {
        match &self.base {
            ABase::Var(_) => {}
            ABase::Un(_, a) | ABase::Group(a) => a.inner(out),
        }
    }
}

impl ABase {
    pub fn strip(&self) -> Base {
        match self {
            ABase::Var(i) => Base::Var(*i),
            ABase::Un(u, a) => Base::Un(*u, Box::new(a.strip())),
            ABase::Group(a) => Base::Group(Box::new(a.strip())),
        }
    }
}

impl AArg {
    pub fn strip(&self) -> Arg {
        Arg {
            terms: self.terms.iter().map(|t| t.strip()).collect(),
            scale: self.scale.is_some(),
            shift: self.shift.is_some(),
        }
    }

    fn inner(&self, out: &mut Vec<f64>) {
        if let Some(s) = self.scale {
            out.push(s);
        }
        if let Some(s) = self.shift {
            out.push(s);
        }
        for t in &self.terms {
            t.inner(out);
        }
    }
}

#[derive(Clone, Debug)]
struct PProd(Vec<(ABase, f64)>);

#[derive(Clone, Debug)]
struct PSum(Vec<(f64, PProd)>);

fn base_cmp(a: &ABase, b: &ABase) -> std::cmp::Ordering {
    a.strip().cmp(&b.strip())
}

fn prod_key(p: &PProd) -> Vec<(Base, i64)> {
    p.0.iter()
        .map(|(b, q)| (b.strip(), (q * 1e6).round() as i64))
        .collect()
}

impl PProd {
    fn one() -> PProd {
        PProd(Vec::new())
    }

    fn var(i: u8) -> PProd {
        PProd(vec![(ABase::Var(i), 1.0)])
    }

    fn mul(&self, o: &PProd) -> PProd {
        let mut out: Vec<(ABase, f64)> = Vec::new();
        let (mut i, mut j) = (0, 0);
        while i < self.0.len() || j < o.0.len() {
            let take = if i == self.0.len() {
                2
            } else if j == o.0.len() {
                1
            } else {
                match base_cmp(&self.0[i].0, &o.0[j].0) {
                    std::cmp::Ordering::Less => 1,
                    std::cmp::Ordering::Greater => 2,
                    std::cmp::Ordering::Equal => 3,
                }
            };
            match take {
                1 => {
                    out.push(self.0[i].clone());
                    i += 1;
                }
                2 => {
                    out.push(o.0[j].clone());
                    j += 1;
                }
                _ => {
                    let q = self.0[i].1 + o.0[j].1;
                    if q.abs() > EPS {
                        out.push((self.0[i].0.clone(), q));
                    }
                    i += 1;
                    j += 1;
                }
            }
        }
        PProd(out)
    }

    fn pow(&self, q: f64) -> PProd {
        PProd(self.0.iter().map(|(b, e)| (b.clone(), e * q)).collect())
    }
}

impl PSum {
    fn konst(v: f64) -> PSum {
        if v.abs() < EPS {
            PSum(Vec::new())
        } else {
            PSum(vec![(v, PProd::one())])
        }
    }

    fn term(c: f64, p: PProd) -> PSum {
        PSum(vec![(c, p)])
    }

    fn normalize(mut self) -> PSum {
        self.0.sort_by(|a, b| prod_key(&a.1).cmp(&prod_key(&b.1)));
        let mut out: Vec<(f64, PProd)> = Vec::new();
        for (c, p) in self.0 {
            if let Some(last) = out.last_mut() {
                if prod_key(&last.1) == prod_key(&p) {
                    last.0 += c;
                    continue;
                }
            }
            out.push((c, p));
        }
        out.retain(|(c, _)| c.abs() > EPS);
        PSum(out)
    }

    fn add(self, o: PSum) -> PSum {
        let mut v = self.0;
        v.extend(o.0);
        PSum(v).normalize()
    }

    fn scale(self, k: f64) -> PSum {
        PSum(self.0.into_iter().map(|(c, p)| (c * k, p)).collect()).normalize()
    }

    fn mul(&self, o: &PSum) -> PSum {
        let mut v = Vec::new();
        for (c1, p1) in &self.0 {
            for (c2, p2) in &o.0 {
                v.push((c1 * c2, p1.mul(p2)));
            }
        }
        PSum(v).normalize()
    }

    fn constant_part(&self) -> f64 {
        self.0
            .iter()
            .filter(|(_, p)| p.0.is_empty())
            .map(|(c, _)| *c)
            .sum()
    }

    fn nonconst(&self) -> Vec<(f64, PProd)> {
        self.0
            .iter()
            .filter(|(_, p)| !p.0.is_empty())
            .cloned()
            .collect()
    }

    fn as_constant(&self) -> Option<f64> {
        if self.0.iter().all(|(_, p)| p.0.is_empty()) {
            Some(self.constant_part())
        } else {
            None
        }
    }
}

fn prod_to_aterm(p: &PProd, neg: bool) -> Result<ATerm, String> {
    let mut factors = Vec::new();
    for (b, e) in &p.0 {
        match b {
            ABase::Un(Un::Exp, a) if (e - 1.0).abs() > EPS => {
                let cur = a.scale.unwrap_or(if a.terms[0].neg { -1.0 } else { 1.0 });
                let nc = cur * e;
                let mut a2 = (**a).clone();
                if (nc.abs() - 1.0).abs() < EPS {
                    a2.scale = None;
                    a2.terms[0].neg = nc < 0.0;
                } else {
                    a2.scale = Some(nc);
                    a2.terms[0].neg = false;
                }
                factors.push(AFactor {
                    base: ABase::Un(Un::Exp, Box::new(a2)),
                    q: Q::P1,
                });
            }
            _ => {
                let q = Q::from_f64(*e).ok_or_else(|| format!("exponent {} outside Q", e))?;
                factors.push(AFactor { base: b.clone(), q });
            }
        }
    }
    factors.sort_by(|x, y| x.strip().cmp(&y.strip()));
    Ok(ATerm {
        neg,
        coef: None,
        factors,
    })
}

fn aterm_to_prod(t: &ATerm) -> (f64, PProd) {
    let c = if t.neg { -1.0 } else { 1.0 } * t.coef.unwrap_or(1.0);
    (
        c,
        PProd(
            t.factors
                .iter()
                .map(|f| (f.base.clone(), f.q.val()))
                .collect(),
        ),
    )
}

fn arg_to_psum(a: &AArg) -> PSum {
    let mut s = PSum(a.terms.iter().map(aterm_to_prod).collect()).normalize();
    if let Some(k) = a.scale {
        s = s.scale(k);
    }
    if let Some(k) = a.shift {
        s = s.add(PSum::konst(k));
    }
    s
}

fn flip(t: &mut ATerm) {
    match t.coef.as_mut() {
        Some(c) => *c = -*c,
        None => t.neg = !t.neg,
    }
}

fn build_arg(
    terms: Vec<(f64, PProd)>,
    scale: Option<f64>,
    shift: Option<f64>,
) -> Result<AArg, String> {
    let mut at = Vec::new();
    for (c, p) in terms {
        let mut t = prod_to_aterm(&p, c < 0.0)?;
        if (c.abs() - 1.0).abs() > EPS {
            t.coef = Some(c);
            t.neg = false;
        }
        at.push(t);
    }
    at.sort_by(|x, y| x.strip().cmp(&y.strip()));
    if let Some(k) = at[0].coef {
        return Err(format!(
            "leading coefficient {} inside an argument is not ±1",
            k
        ));
    }
    Ok(AArg {
        terms: at,
        scale,
        shift,
    })
}

fn factor_leading(s: &PSum) -> (f64, Vec<(f64, PProd)>, f64) {
    let mut nc = s.nonconst();
    nc.sort_by(|a, b| prod_key(&a.1).cmp(&prod_key(&b.1)));
    let k = nc[0].0.abs();
    let terms = nc.into_iter().map(|(c, p)| (c / k, p)).collect();
    (k, terms, s.constant_part() / k)
}

fn pow_sum(a: &PSum, q: f64, inside_arg: bool) -> Result<PSum, String> {
    if q.abs() < EPS {
        return Ok(PSum::konst(1.0));
    }
    if let Some(c) = a.as_constant() {
        return Ok(PSum::konst(c.powf(q)));
    }
    if a.0.len() == 1 {
        let (c, p) = &a.0[0];
        let (mut c, mut p) = (*c, p.clone());
        if c < 0.0 && (q - q.round()).abs() > EPS {
            let odd = p.0.iter().position(|(b, e)| {
                matches!(b, ABase::Group(_))
                    && (e - e.round()).abs() < EPS
                    && (e.round() as i64).rem_euclid(2) == 1
            });
            let Some(i) = odd else {
                return Err("fractional power of a negative coefficient".into());
            };
            if let ABase::Group(arg) = &mut p.0[i].0 {
                for t in &mut arg.terms {
                    flip(t);
                }
                if let Some(s) = arg.shift.as_mut() {
                    *s = -*s;
                }
            }
            c = -c;
        }
        return Ok(PSum::term(c.powf(q), p.pow(q)));
    }
    if (q - 1.0).abs() < EPS {
        return Ok(a.clone());
    }
    if (q - q.round()).abs() < EPS && q > 1.0 && q <= 6.0 && !inside_arg {
        let mut r = a.clone();
        for _ in 1..(q.round() as usize) {
            r = r.mul(a);
        }
        return Ok(r);
    }
    let qq = Q::from_f64(q).ok_or_else(|| format!("group exponent {} outside Q", q))?;
    let (k, terms, konst) = factor_leading(a);
    let shift = if konst.abs() > EPS { Some(konst) } else { None };
    let mut arg = build_arg(terms, None, shift)?;
    let mut coef = k.powf(q);
    if !qq.is_frac() && arg.terms[0].neg {
        for t in &mut arg.terms {
            flip(t);
        }
        if let Some(s) = arg.shift.as_mut() {
            *s = -*s;
        }
        if qq.is_int() && qq.num.rem_euclid(2) == 1 {
            coef = -coef;
        }
    }
    Ok(PSum::term(
        coef,
        PProd(vec![(ABase::Group(Box::new(arg)), q)]),
    ))
}

fn un_sum(u: Un, a: &PSum) -> Result<PSum, String> {
    if let Some(c) = a.as_constant() {
        return Ok(PSum::konst(u.apply(c)));
    }
    match u {
        Un::Exp => {
            let mut out = PSum::konst(a.constant_part().exp());
            for (c, p) in a.nonconst() {
                if p.0.len() == 1 {
                    if let (ABase::Un(Un::Log, la), e) = (&p.0[0].0, p.0[0].1) {
                        if (e - 1.0).abs() < EPS && Q::from_f64(c).is_some() {
                            out = out.mul(&pow_sum(&arg_to_psum(la), c, false)?);
                            continue;
                        }
                    }
                }
                let (neg, scale) = if (c.abs() - 1.0).abs() < EPS {
                    (c < 0.0, None)
                } else {
                    (false, Some(c))
                };
                let t = prod_to_aterm(&p, neg)?;
                let arg = AArg {
                    terms: vec![t],
                    scale,
                    shift: None,
                };
                out = out.mul(&PSum::term(
                    1.0,
                    PProd(vec![(ABase::Un(Un::Exp, Box::new(arg)), 1.0)]),
                ));
            }
            Ok(out)
        }
        Un::Log => {
            if a.0.len() == 1 {
                let (c, p) = &a.0[0];
                if *c <= 0.0 {
                    return Err("log of a negative coefficient".into());
                }
                let mut out = PSum::konst(c.ln());
                for (b, e) in &p.0 {
                    let piece = match b {
                        ABase::Var(i) => {
                            let arg = AArg {
                                terms: vec![ATerm {
                                    neg: false,
                                    coef: None,
                                    factors: vec![AFactor {
                                        base: ABase::Var(*i),
                                        q: Q::P1,
                                    }],
                                }],
                                scale: None,
                                shift: None,
                            };
                            PSum::term(*e, PProd(vec![(ABase::Un(Un::Log, Box::new(arg)), 1.0)]))
                        }
                        ABase::Un(Un::Exp, arg) => arg_to_psum(arg).scale(*e),
                        ABase::Group(arg) => un_sum(Un::Log, &arg_to_psum(arg))?.scale(*e),
                        ABase::Un(_, _) => {
                            let arg = AArg {
                                terms: vec![ATerm {
                                    neg: false,
                                    coef: None,
                                    factors: vec![AFactor {
                                        base: b.clone(),
                                        q: Q::P1,
                                    }],
                                }],
                                scale: None,
                                shift: None,
                            };
                            PSum::term(*e, PProd(vec![(ABase::Un(Un::Log, Box::new(arg)), 1.0)]))
                        }
                    };
                    out = out.add(piece);
                }
                return Ok(out);
            }
            let (k, terms, konst) = factor_leading(a);
            let shift = if konst.abs() > EPS { Some(konst) } else { None };
            let arg = build_arg(terms, None, shift)?;
            let out = PSum::konst(k.ln());
            Ok(out.add(PSum::term(
                1.0,
                PProd(vec![(ABase::Un(Un::Log, Box::new(arg)), 1.0)]),
            )))
        }
        _ => {
            let (mut k, terms, konst) = factor_leading(a);
            let mut arg = build_arg(terms, None, None)?;
            let mut shift = konst;
            let mut coef = 1.0;
            let mut offset = 0.0;
            let has_scale = (k - 1.0).abs() > EPS;
            if arg.terms[0].neg {
                for t in &mut arg.terms {
                    flip(t);
                }
                if has_scale {
                    k = -k;
                } else {
                    shift = -shift;
                    if u.is_odd() {
                        coef = -1.0;
                    } else if u == Un::Arccos {
                        coef = -1.0;
                        offset = std::f64::consts::PI;
                    }
                }
            }
            let mut un = u;
            if u == Un::Cos && shift.abs() > EPS {
                un = Un::Sin;
                shift += std::f64::consts::FRAC_PI_2;
            }
            arg.scale = if has_scale { Some(k) } else { None };
            arg.shift = if shift.abs() > EPS { Some(shift) } else { None };
            Ok(PSum::konst(offset).add(PSum::term(
                coef,
                PProd(vec![(ABase::Un(un, Box::new(arg)), 1.0)]),
            )))
        }
    }
}

fn to_psum_recip(a: &Ast, inside: bool) -> Result<PSum, String> {
    Ok(match a {
        Ast::Pow(x, y) => {
            let q = to_psum(y, inside)?
                .as_constant()
                .ok_or("non-constant exponent")?;
            pow_sum(&to_psum(x, true)?, -q, inside)?
        }
        Ast::Sqrt(x) => pow_sum(&to_psum(x, true)?, -0.5, inside)?,
        Ast::Mul(x, y) => to_psum_recip(x, inside)?.mul(&to_psum_recip(y, inside)?),
        Ast::Div(x, y) => to_psum_recip(x, inside)?.mul(&to_psum(y, inside)?),
        Ast::Neg(x) => to_psum_recip(x, inside)?.scale(-1.0),
        _ => pow_sum(&to_psum(a, true)?, -1.0, inside)?,
    })
}

fn to_psum(a: &Ast, inside: bool) -> Result<PSum, String> {
    Ok(match a {
        Ast::Num(v) => PSum::konst(*v),
        Ast::Var(i) => PSum::term(1.0, PProd::var(*i)),
        Ast::Add(x, y) => to_psum(x, inside)?.add(to_psum(y, inside)?),
        Ast::Sub(x, y) => to_psum(x, inside)?.add(to_psum(y, inside)?.scale(-1.0)),
        Ast::Neg(x) => to_psum(x, inside)?.scale(-1.0),
        Ast::Mul(x, y) => to_psum(x, inside)?.mul(&to_psum(y, inside)?),
        Ast::Div(x, y) => to_psum(x, inside)?.mul(&to_psum_recip(y, inside)?),
        Ast::Pow(x, y) => {
            let q = to_psum(y, inside)?
                .as_constant()
                .ok_or("non-constant exponent")?;
            pow_sum(&to_psum(x, true)?, q, inside)?
        }
        Ast::Sqrt(x) => pow_sum(&to_psum(x, true)?, 0.5, inside)?,
        Ast::Un(u, x) => un_sum(*u, &to_psum(x, true)?)?,
    })
}

pub fn normalize(a: &Ast) -> Result<(Expr, Fitted), String> {
    let s = to_psum(a, false)?;
    let offset = s.constant_part();
    let mut terms: Vec<(f64, ATerm)> = Vec::new();
    for (c, p) in s.nonconst() {
        terms.push((c, prod_to_aterm(&p, false)?));
    }
    if terms.is_empty() {
        return Err("constant expression".into());
    }
    terms.sort_by(|x, y| x.1.strip().cmp(&y.1.strip()));
    let mut inner = Vec::new();
    for (_, t) in &terms {
        t.inner(&mut inner);
    }
    let expr = Expr {
        terms: terms.iter().map(|(_, t)| t.strip()).collect(),
    };
    let coef = terms.iter().map(|(c, _)| *c).collect();
    Ok((
        expr,
        Fitted {
            coef,
            offset,
            inner,
        },
    ))
}

pub fn to_ast(e: &Expr, f: &crate::forms::nf::Fitted) -> Ast {
    let mut c = 0usize;
    let mut acc: Option<Ast> = None;
    for (i, t) in e.terms.iter().enumerate() {
        let body = term_ast(t, &f.inner, &mut c);
        let scaled = Ast::Mul(Box::new(Ast::Num(f.coef[i])), Box::new(body));
        acc = Some(match acc {
            None => scaled,
            Some(a) => Ast::Add(Box::new(a), Box::new(scaled)),
        });
    }
    let a = acc.unwrap();
    if f.offset != 0.0 {
        Ast::Add(Box::new(a), Box::new(Ast::Num(f.offset)))
    } else {
        a
    }
}

fn term_ast(t: &Term, inner: &[f64], c: &mut usize) -> Ast {
    let coef = if t.coef {
        let k = inner[*c];
        *c += 1;
        Some(k)
    } else {
        None
    };
    let mut acc: Option<Ast> = None;
    for fac in &t.factors {
        let b = match &fac.base {
            Base::Var(i) => Ast::Var(*i),
            Base::Un(u, a) => Ast::Un(*u, Box::new(arg_ast(a, inner, c))),
            Base::Group(a) => arg_ast(a, inner, c),
        };
        let p = if fac.q == Q::P1 {
            b
        } else {
            Ast::Pow(Box::new(b), Box::new(Ast::Num(fac.q.val())))
        };
        acc = Some(match acc {
            None => p,
            Some(a) => Ast::Mul(Box::new(a), Box::new(p)),
        });
    }
    let mut a = acc.unwrap();
    if let Some(k) = coef {
        a = Ast::Mul(Box::new(Ast::Num(k)), Box::new(a));
    }
    if t.neg {
        Ast::Neg(Box::new(a))
    } else {
        a
    }
}

fn arg_ast(a: &Arg, inner: &[f64], c: &mut usize) -> Ast {
    let scale = if a.scale {
        *c += 1;
        Some(inner[*c - 1])
    } else {
        None
    };
    let shift = if a.shift {
        *c += 1;
        Some(inner[*c - 1])
    } else {
        None
    };
    let mut acc: Option<Ast> = None;
    for t in &a.terms {
        let ta = term_ast(t, inner, c);
        acc = Some(match acc {
            None => ta,
            Some(x) => Ast::Add(Box::new(x), Box::new(ta)),
        });
    }
    let mut out = acc.unwrap();
    if let Some(s) = scale {
        out = Ast::Mul(Box::new(Ast::Num(s)), Box::new(out));
    }
    if let Some(s) = shift {
        out = Ast::Add(Box::new(out), Box::new(Ast::Num(s)));
    }
    out
}
