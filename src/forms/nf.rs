use std::cmp::Ordering;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Q {
    pub num: i8,
    pub den: u8,
}

impl PartialOrd for Q {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

impl Ord for Q {
    fn cmp(&self, o: &Self) -> Ordering {
        (self.num as i32 * o.den as i32).cmp(&(o.num as i32 * self.den as i32))
    }
}

const fn q(num: i8, den: u8) -> Q {
    Q { num, den }
}

impl Q {
    pub const P1: Q = q(1, 1);

    pub const ALL: [Q; 26] = [
        q(-6, 1),
        q(-5, 1),
        q(-4, 1),
        q(-3, 1),
        q(-5, 2),
        q(-2, 1),
        q(-3, 2),
        q(-1, 1),
        q(-3, 4),
        q(-2, 3),
        q(-1, 2),
        q(-1, 3),
        q(-1, 4),
        q(1, 4),
        q(1, 3),
        q(1, 2),
        q(2, 3),
        q(3, 4),
        q(1, 1),
        q(3, 2),
        q(2, 1),
        q(5, 2),
        q(3, 1),
        q(4, 1),
        q(5, 1),
        q(6, 1),
    ];

    pub const GROUP: [Q; 25] = [
        q(-6, 1),
        q(-5, 1),
        q(-4, 1),
        q(-3, 1),
        q(-5, 2),
        q(-2, 1),
        q(-3, 2),
        q(-1, 1),
        q(-3, 4),
        q(-2, 3),
        q(-1, 2),
        q(-1, 3),
        q(-1, 4),
        q(1, 4),
        q(1, 3),
        q(1, 2),
        q(2, 3),
        q(3, 4),
        q(3, 2),
        q(2, 1),
        q(5, 2),
        q(3, 1),
        q(4, 1),
        q(5, 1),
        q(6, 1),
    ];

    pub const CORE: [Q; 6] = [q(-2, 1), q(-1, 1), q(-1, 2), q(1, 2), q(1, 1), q(2, 1)];

    pub const GROUP_CORE: [Q; 4] = [q(-2, 1), q(-1, 1), q(-1, 2), q(1, 2)];

    pub const GROUP_TOP: [Q; 20] = [
        q(-6, 1),
        q(-5, 1),
        q(-4, 1),
        q(-3, 1),
        q(-5, 2),
        q(-2, 1),
        q(-3, 2),
        q(-1, 1),
        q(-3, 4),
        q(-2, 3),
        q(-1, 2),
        q(-1, 3),
        q(-1, 4),
        q(1, 4),
        q(1, 3),
        q(1, 2),
        q(2, 3),
        q(3, 4),
        q(3, 2),
        q(5, 2),
    ];

    pub fn val(self) -> f64 {
        self.num as f64 / self.den as f64
    }

    pub fn from_f64(v: f64) -> Option<Q> {
        Q::ALL.iter().copied().find(|q| (q.val() - v).abs() < 1e-9)
    }

    pub fn is_neg(self) -> bool {
        self.num < 0
    }

    pub fn is_frac(self) -> bool {
        self.den > 1
    }

    pub fn is_int(self) -> bool {
        self.den == 1
    }

    pub fn is_pos_int_power(self) -> bool {
        self.den == 1 && self.num >= 2
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Un {
    Exp,
    Log,
    Sin,
    Cos,
    Arcsin,
    Arccos,
    Arctan,
    Tanh,
}

impl Un {
    pub const ALL: [Un; 8] = [
        Un::Exp,
        Un::Log,
        Un::Sin,
        Un::Cos,
        Un::Arcsin,
        Un::Arccos,
        Un::Arctan,
        Un::Tanh,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Un::Exp => "exp",
            Un::Log => "log",
            Un::Sin => "sin",
            Un::Cos => "cos",
            Un::Arcsin => "arcsin",
            Un::Arccos => "arccos",
            Un::Arctan => "arctan",
            Un::Tanh => "tanh",
        }
    }

    pub fn apply(self, v: f64) -> f64 {
        match self {
            Un::Exp => v.exp(),
            Un::Log => v.ln(),
            Un::Sin => v.sin(),
            Un::Cos => v.cos(),
            Un::Arcsin => v.asin(),
            Un::Arccos => v.acos(),
            Un::Arctan => v.atan(),
            Un::Tanh => v.tanh(),
        }
    }

    pub fn is_odd(self) -> bool {
        matches!(self, Un::Sin | Un::Arcsin | Un::Arctan | Un::Tanh)
    }

    pub fn takes_shift(self) -> bool {
        !matches!(self, Un::Exp | Un::Cos)
    }

    pub fn takes_scale(self) -> bool {
        self != Un::Log
    }
}

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Base {
    Var(u8),
    Un(Un, Box<Arg>),
    Group(Box<Arg>),
}

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct Factor {
    pub base: Base,
    pub q: Q,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Term {
    pub neg: bool,
    pub coef: bool,
    pub factors: Vec<Factor>,
}

impl PartialOrd for Term {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

impl Ord for Term {
    fn cmp(&self, o: &Self) -> Ordering {
        self.factors
            .cmp(&o.factors)
            .then(self.neg.cmp(&o.neg))
            .then(self.coef.cmp(&o.coef))
    }
}

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct Arg {
    pub terms: Vec<Term>,
    pub scale: bool,
    pub shift: bool,
}

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct Expr {
    pub terms: Vec<Term>,
}

pub const SNAP_CONSTANTS: [f64; 4] = [
    std::f64::consts::PI,
    std::f64::consts::E,
    std::f64::consts::SQRT_2,
    1.732_050_807_568_877_2,
];

pub const SNAP_POWERS: [f64; 6] = [-2.0, -1.0, -0.5, 0.5, 1.0, 2.0];

pub const SNAP_DENOMS: [usize; 6] = [1, 2, 3, 4, 6, 8];

pub const SNAP_MULT_MAX: f64 = 8.0;

pub fn symbolic_constant(v: f64) -> Option<String> {
    if !v.is_finite() || v == 0.0 {
        return None;
    }
    let close = |c: f64| (c - v).abs() <= 1e-12 * v.abs();
    for q in 1..=12i64 {
        let p = (v * q as f64).round();
        if p.abs() <= 1000.0 && close(p / q as f64) {
            return None;
        }
    }
    let names = ["pi", "e", "sqrt(2)", "sqrt(3)"];
    for (base, name) in SNAP_CONSTANTS.iter().zip(names) {
        for k in [1.0, -1.0, 2.0, -2.0, 0.5, -0.5] {
            let b = base.powf(k);
            for den in SNAP_DENOMS {
                let num = (v / b * den as f64).round();
                if num == 0.0 || num.abs() > SNAP_MULT_MAX || !close(num / den as f64 * b) {
                    continue;
                }
                let g = gcd(num.abs() as i64, den as i64);
                let (p, q) = (num as i64 / g, den as i64 / g);
                let pow = match k {
                    x if x == 1.0 || x == -1.0 => name.to_string(),
                    x if x == 2.0 || x == -2.0 => format!("{}**2", name),
                    _ => match name {
                        "sqrt(2)" => "2**(1/4)".to_string(),
                        "sqrt(3)" => "3**(1/4)".to_string(),
                        n => format!("sqrt({})", n),
                    },
                };
                let sign = if p < 0 { "-" } else { "" };
                let pa = p.abs();
                let text = if k > 0.0 {
                    match (pa, q) {
                        (1, 1) => pow,
                        (_, 1) => format!("{}*{}", pa, pow),
                        (1, _) => format!("{}/{}", pow, q),
                        _ => format!("{}*{}/{}", pa, pow, q),
                    }
                } else {
                    match q {
                        1 => format!("{}/{}", pa, pow),
                        _ => format!("{}/({}*{})", pa, q, pow),
                    }
                };
                return Some(format!("{}{}", sign, text));
            }
        }
    }
    None
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 {
        a.max(1)
    } else {
        gcd(b, a % b)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpSet {
    Core,
    Full,
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub n_cols: usize,
    pub max_nodes: usize,
    pub max_nest: usize,
    pub max_consts: usize,
    pub max_terms: usize,
    pub max_factors: usize,
    pub max_arg_nodes: usize,
    pub exps: ExpSet,
}

impl Limits {
    pub fn full(n_cols: usize) -> Limits {
        Limits {
            n_cols,
            max_nodes: 32,
            max_nest: 3,
            max_consts: 4,
            max_terms: 6,
            max_factors: 8,
            max_arg_nodes: 32,
            exps: ExpSet::Full,
        }
    }

    pub fn exps(&self) -> &'static [Q] {
        match self.exps {
            ExpSet::Core => &Q::CORE,
            ExpSet::Full => &Q::ALL,
        }
    }

    pub fn group_exps(&self, top: bool) -> &'static [Q] {
        match (self.exps, top) {
            (ExpSet::Core, _) => &Q::GROUP_CORE,
            (ExpSet::Full, true) => &Q::GROUP_TOP,
            (ExpSet::Full, false) => &Q::GROUP,
        }
    }
}

impl Factor {
    pub fn var_q(i: u8, q: Q) -> Factor {
        Factor {
            base: Base::Var(i),
            q,
        }
    }
}

impl Term {}

impl Arg {}

impl Expr {}

pub fn nodes_arg(a: &Arg) -> usize {
    let mut n = a.scale as usize + a.shift as usize + a.terms.len() - 1;
    for t in &a.terms {
        n += nodes_term(t);
    }
    n
}

pub fn nodes_term(t: &Term) -> usize {
    let mut n = t.neg as usize + t.coef as usize + t.factors.len() - 1;
    for f in &t.factors {
        n += nodes_factor(f);
    }
    n
}

pub fn nodes_factor(f: &Factor) -> usize {
    let base = match &f.base {
        Base::Var(_) => 1,
        Base::Un(_, a) => 1 + nodes_arg(a),
        Base::Group(a) => 1 + nodes_arg(a),
    };
    base + (f.q != Q::P1) as usize
}

pub fn nest_arg(a: &Arg) -> usize {
    a.terms.iter().map(nest_term).max().unwrap_or(0)
}

pub fn nest_term(t: &Term) -> usize {
    t.factors.iter().map(nest_factor).max().unwrap_or(0)
}

pub fn nest_factor(f: &Factor) -> usize {
    match &f.base {
        Base::Var(_) => 0,
        Base::Un(_, a) | Base::Group(a) => 1 + nest_arg(a),
    }
}

pub fn consts_arg(a: &Arg) -> usize {
    a.scale as usize + a.shift as usize + a.terms.iter().map(consts_term).sum::<usize>()
}

pub fn consts_term(t: &Term) -> usize {
    t.coef as usize
        + t.factors
            .iter()
            .map(|f| match &f.base {
                Base::Var(_) => 0,
                Base::Un(_, a) | Base::Group(a) => consts_arg(a),
            })
            .sum::<usize>()
}

impl Expr {
    pub fn nodes(&self) -> usize {
        self.terms.len() - 1 + self.terms.iter().map(nodes_term).sum::<usize>()
    }

    pub fn nest(&self) -> usize {
        self.terms.iter().map(nest_term).max().unwrap_or(0)
    }

    pub fn n_consts(&self) -> usize {
        self.terms.iter().map(consts_term).sum()
    }

    pub fn max_col(&self) -> Option<u8> {
        let mut m = None;
        self.visit_vars(&mut |i| m = Some(m.map_or(i, |x: u8| x.max(i))));
        m
    }

    pub fn visit_vars(&self, f: &mut dyn FnMut(u8)) {
        for t in &self.terms {
            visit_vars_term(t, f);
        }
    }
}

pub fn visit_vars_term(t: &Term, f: &mut dyn FnMut(u8)) {
    for fac in &t.factors {
        match &fac.base {
            Base::Var(i) => f(*i),
            Base::Un(_, a) | Base::Group(a) => {
                for tt in &a.terms {
                    visit_vars_term(tt, f);
                }
            }
        }
    }
}

fn sorted_unique_by_factors(terms: &[Term]) -> bool {
    terms.windows(2).all(|w| w[0].factors < w[1].factors)
}

fn check_term(t: &Term, lim: &Limits, inside_arg: bool) -> Result<(), &'static str> {
    if t.factors.is_empty() {
        return Err("empty term");
    }
    if t.factors.len() > lim.max_factors {
        return Err("too many factors");
    }
    if !t.factors.windows(2).all(|w| w[0].base < w[1].base) {
        return Err("factors not sorted/unique");
    }
    if t.coef && !inside_arg {
        return Err("term constant at top level");
    }
    if t.coef && t.neg {
        return Err("a term constant carries the sign");
    }
    for f in &t.factors {
        check_factor(f, lim, inside_arg)?;
    }
    Ok(())
}

fn check_factor(f: &Factor, lim: &Limits, inside_arg: bool) -> Result<(), &'static str> {
    match &f.base {
        Base::Var(i) => {
            if (*i as usize) >= lim.n_cols {
                return Err("column out of range");
            }
            if !lim.exps().contains(&f.q) {
                return Err("exponent outside the limits' exponent set");
            }
        }
        Base::Un(u, a) => {
            check_arg_common(a, lim)?;
            if !lim.exps().contains(&f.q) {
                return Err("exponent outside the limits' exponent set");
            }
            match u {
                Un::Exp => {
                    if f.q != Q::P1 {
                        return Err("exp with exponent");
                    }
                    if a.terms.len() != 1 || a.shift {
                        return Err("exp arg must be one term, no shift");
                    }
                    if a.scale && a.terms[0].neg {
                        return Err("exp arg sign under scale");
                    }
                    if is_lone_log(&a.terms[0]) && !a.scale {
                        return Err("exp(±log s) is s^±1");
                    }
                }
                Un::Log => {
                    if a.scale {
                        return Err("log arg scale");
                    }
                    if a.terms.len() == 1 && !a.shift {
                        let t = &a.terms[0];
                        let single = t.factors.len() == 1
                            && t.factors[0].q == Q::P1
                            && !matches!(t.factors[0].base, Base::Un(Un::Exp, _) | Base::Group(_));
                        if !single || t.neg {
                            return Err(
                                "log of a single term must be one unpowered non-exp, non-group factor",
                            );
                        }
                    }
                }
                _ => {
                    if a.terms[0].neg {
                        return Err("unary arg first term negative");
                    }
                    if !u.takes_shift() && a.shift {
                        return Err("cos(a + C) is sin(a + C')");
                    }
                }
            }
        }
        Base::Group(a) => {
            check_arg_common(a, lim)?;
            if f.q == Q::P1 {
                return Err("group exponent 1 distributes");
            }
            if !lim.group_exps(!inside_arg).contains(&f.q) {
                return Err("group exponent outside the limits' exponent set");
            }
            if f.q.is_pos_int_power() && !inside_arg {
                return Err("positive integer group power expands at the top level");
            }
            if a.scale {
                return Err("group arg scale");
            }
            if a.terms.len() == 1 && !a.shift {
                return Err("group needs two terms or a shift");
            }
            if !f.q.is_frac() && a.terms[0].neg {
                return Err("group arg first term negative");
            }
        }
    }
    Ok(())
}

pub fn is_lone_log(t: &Term) -> bool {
    t.factors.len() == 1
        && t.factors[0].q == Q::P1
        && matches!(t.factors[0].base, Base::Un(Un::Log, _))
}

fn check_arg_common(a: &Arg, lim: &Limits) -> Result<(), &'static str> {
    if a.terms.is_empty() {
        return Err("empty arg");
    }
    if a.terms.len() > lim.max_terms {
        return Err("too many terms in arg");
    }
    if !sorted_unique_by_factors(&a.terms) {
        return Err("arg terms not sorted/unique");
    }
    if a.terms[0].coef {
        return Err("arg first term carries a constant (it is the scale)");
    }
    for t in &a.terms {
        check_term(t, lim, true)?;
    }
    Ok(())
}

impl Expr {
    pub fn check(&self, lim: &Limits) -> Result<(), &'static str> {
        if self.terms.is_empty() {
            return Err("empty expr");
        }
        if self.terms.len() > lim.max_terms {
            return Err("too many terms");
        }
        if self.terms.iter().any(|t| t.neg) {
            return Err("top-level sign");
        }
        if !sorted_unique_by_factors(&self.terms) {
            return Err("top terms not sorted/unique");
        }
        for t in &self.terms {
            check_term(t, lim, false)?;
        }
        if self.nodes() > lim.max_nodes {
            return Err("too many nodes");
        }
        if self.nest() > lim.max_nest {
            return Err("too deep");
        }
        if self.n_consts() > lim.max_consts {
            return Err("too many constants");
        }
        Ok(())
    }
}

#[inline]
pub fn pow_q(b: f64, q: Q) -> f64 {
    match (q.den, q.num) {
        (1, 1) => b,
        (1, 2) => b * b,
        (1, -1) => 1.0 / b,
        (1, -2) => 1.0 / (b * b),
        (2, 1) => b.sqrt(),
        (2, -1) => 1.0 / b.sqrt(),
        (1, 3) => b * b * b,
        (1, -3) => 1.0 / (b * b * b),
        (1, n) if n > 0 => b.powi(n as i32),
        (1, n) => 1.0 / b.powi(-n as i32),
        _ => b.powf(q.val()),
    }
}

pub fn eval_terms(e: &Expr, cols: &[Vec<f64>], inner: &[f64]) -> Vec<Vec<f64>> {
    let n = cols.first().map_or(0, |c| c.len());
    let mut cursor = 0usize;
    e.terms
        .iter()
        .map(|t| {
            let mut out = vec![0.0; n];
            for r in 0..n {
                let mut c = cursor;
                out[r] = eval_term(t, cols, r, inner, &mut c);
                if r == n - 1 {
                    cursor = c;
                }
            }
            if n == 0 {
                let mut c = cursor;
                let _ = count_consts_term(t, &mut c);
                cursor = c;
            }
            out
        })
        .collect()
}

fn count_consts_term(t: &Term, c: &mut usize) {
    *c += consts_term(t);
}

fn eval_term(t: &Term, cols: &[Vec<f64>], r: usize, inner: &[f64], c: &mut usize) -> f64 {
    let mut v = if t.coef {
        let k = inner[*c];
        *c += 1;
        k
    } else {
        1.0
    };
    for f in &t.factors {
        v *= eval_factor(f, cols, r, inner, c);
    }
    if t.neg {
        -v
    } else {
        v
    }
}

fn eval_factor(f: &Factor, cols: &[Vec<f64>], r: usize, inner: &[f64], c: &mut usize) -> f64 {
    let b = match &f.base {
        Base::Var(i) => cols[*i as usize][r],
        Base::Un(u, a) => u.apply(eval_arg(a, cols, r, inner, c)),
        Base::Group(a) => eval_arg(a, cols, r, inner, c),
    };
    pow_q(b, f.q)
}

fn eval_arg(a: &Arg, cols: &[Vec<f64>], r: usize, inner: &[f64], c: &mut usize) -> f64 {
    let scale = if a.scale {
        let v = inner[*c];
        *c += 1;
        v
    } else {
        1.0
    };
    let shift = if a.shift {
        let v = inner[*c];
        *c += 1;
        v
    } else {
        0.0
    };
    let mut s = 0.0;
    for t in &a.terms {
        s += eval_term(t, cols, r, inner, c);
    }
    scale * s + shift
}

#[derive(Clone, Debug)]
pub struct Fitted {
    pub coef: Vec<f64>,
    pub offset: f64,
    pub inner: Vec<f64>,
}

pub fn fmt_num(v: f64) -> String {
    if v == v.round() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{:?}", v)
    }
}

pub fn print_structural(e: &Expr) -> String {
    let mut c = 0usize;
    let mut s = String::new();
    for (i, t) in e.terms.iter().enumerate() {
        if i > 0 {
            s.push_str(" + ");
        }
        s.push_str(&format!("c{}*", i + 1));
        s.push_str(&print_term_body(t, None, &mut c));
    }
    s.push_str(" + b");
    s
}

pub fn print_fitted(e: &Expr, f: &Fitted) -> String {
    print_fitted_u(e, f, false)
}

pub fn print_answer(e: &Expr, f: &Fitted) -> String {
    print_fitted_u(e, f, true)
}

fn print_fitted_u(e: &Expr, f: &Fitted, unit: bool) -> String {
    let mut c = 0usize;
    let mut s = String::new();
    for (i, t) in e.terms.iter().enumerate() {
        let coef = f.coef[i];
        if coef == 0.0 {
            let _ = print_term_body_u(t, Some(&f.inner), &mut c, unit);
            continue;
        }
        let lone = match (&t.factors[..], unit, t.coef) {
            ([fa], true, false) if fa.q.den > 1 => match &fa.base {
                Base::Group(a) if !a.shift => Some((a, fa.q)),
                _ => None,
            },
            _ => None,
        };
        let (body, mult) = match lone {
            Some((a, q)) => {
                let lam = coef.abs().powf(q.den as f64 / q.num as f64);
                let b = print_pow_abs(
                    format!("({})", print_arg_scaled(a, &f.inner, &mut c, lam)),
                    q,
                );
                (
                    if q.is_neg() { format!("1/{}", b) } else { b },
                    1.0 / coef.abs(),
                )
            }
            None => print_term_body_u(t, Some(&f.inner), &mut c, unit),
        };
        let coef = coef * mult;
        if !s.is_empty() {
            s.push_str(if coef < 0.0 { " - " } else { " + " });
        } else if coef < 0.0 {
            s.push('-');
        }
        let a = coef.abs();
        if (a - 1.0).abs() > 1e-12 {
            s.push_str(&fmt_num(a));
            s.push('*');
        }
        s.push_str(&body);
    }
    if f.offset != 0.0 || s.is_empty() {
        if s.is_empty() {
            s.push_str(&fmt_num(f.offset));
        } else {
            s.push_str(if f.offset < 0.0 { " - " } else { " + " });
            s.push_str(&fmt_num(f.offset.abs()));
        }
    }
    s
}

fn print_pow_abs(b: String, q: Q) -> String {
    let n = q.num.abs();
    match (q.den, n) {
        (1, 1) => b,
        (1, n) => format!("{}**{}", b, n),
        (2, 1) => format!("sqrt({})", b),
        (d, n) => format!("{}**({}/{})", b, n, d),
    }
}

fn print_term_body(t: &Term, inner: Option<&[f64]>, c: &mut usize) -> String {
    print_term_body_u(t, inner, c, false).0
}

fn print_term_body_u(t: &Term, inner: Option<&[f64]>, c: &mut usize, unit: bool) -> (String, f64) {
    let coef = if t.coef {
        Some(placeholder(inner, c))
    } else {
        None
    };
    let mut num: Vec<String> = Vec::new();
    let mut den: Vec<String> = Vec::new();
    let mut mult = 1.0;
    for f in &t.factors {
        let b = match (&f.base, inner) {
            (Base::Group(a), Some(v))
                if unit && f.q.den > 1 && a.shift && v[*c + a.scale as usize] != 0.0 =>
            {
                let sv = v[*c + a.scale as usize].abs();
                mult *= sv.powf(f.q.num as f64 / f.q.den as f64);
                format!("({})", print_arg_unit(a, v, c))
            }
            _ => print_base_u(&f.base, inner, c, unit),
        };
        let s = print_pow_abs(b, f.q);
        if f.q.is_neg() {
            den.push(s)
        } else {
            num.push(s)
        }
    }
    let n = if num.is_empty() {
        "1".to_string()
    } else {
        num.join("*")
    };
    let mut body = if den.is_empty() {
        n
    } else if den.len() == 1 && !den[0].replace("**", "^").contains('*') {
        format!("{}/{}", n, den[0])
    } else {
        format!("{}/({})", n, den.join("*"))
    };
    if let Some(k) = coef {
        let k = if k.starts_with('-') {
            format!("({})", k)
        } else {
            k
        };
        body = if body == "1" {
            k
        } else {
            format!("{}*{}", k, body)
        };
    }
    if t.neg {
        (format!("-{}", body), mult)
    } else {
        (body, mult)
    }
}

fn print_base_u(b: &Base, inner: Option<&[f64]>, c: &mut usize, unit: bool) -> String {
    match b {
        Base::Var(i) => format!("x{}", i + 1),
        Base::Un(u, a) => format!("{}({})", u.name(), print_arg_u(a, inner, c, unit)),
        Base::Group(a) => format!("({})", print_arg_u(a, inner, c, unit)),
    }
}

fn print_arg_unit(a: &Arg, inner: &[f64], c: &mut usize) -> String {
    let k = if a.scale {
        *c += 1;
        inner[*c - 1]
    } else {
        1.0
    };
    let sv = inner[*c];
    *c += 1;
    let terms = print_arg_terms(a, Some(inner), c, true);
    let k = k / sv.abs();
    let body = if (k - 1.0).abs() <= 1e-12 {
        terms
    } else if a.terms.len() > 1 {
        format!("{}*({})", fmt_num(k), terms)
    } else if let Some(rest) = terms.strip_prefix('-') {
        format!("-{}*{}", fmt_num(k), rest)
    } else {
        format!("{}*{}", fmt_num(k), terms)
    };
    format!("{} {} 1", body, if sv < 0.0 { "-" } else { "+" })
}

fn print_arg_scaled(a: &Arg, inner: &[f64], c: &mut usize, lam: f64) -> String {
    let k = if a.scale {
        *c += 1;
        inner[*c - 1]
    } else {
        1.0
    };
    let sv = if a.shift {
        *c += 1;
        Some(inner[*c - 1])
    } else {
        None
    };
    let terms = print_arg_terms(a, Some(inner), c, true);
    let k = k * lam;
    let mut body = if (k - 1.0).abs() <= 1e-12 {
        terms
    } else if a.terms.len() > 1 {
        format!("{}*({})", fmt_num(k), terms)
    } else if let Some(rest) = terms.strip_prefix('-') {
        format!("-{}*{}", fmt_num(k), rest)
    } else {
        format!("{}*{}", fmt_num(k), terms)
    };
    if let Some(sv) = sv {
        let sv = sv * lam;
        body = format!(
            "{} {} {}",
            body,
            if sv < 0.0 { "-" } else { "+" },
            fmt_num(sv.abs())
        );
    }
    body
}

fn print_arg_terms(a: &Arg, inner: Option<&[f64]>, c: &mut usize, unit: bool) -> String {
    let mut s = String::new();
    for (i, t) in a.terms.iter().enumerate() {
        let (body, mult) = print_term_body_u(
            &Term {
                neg: false,
                coef: t.coef,
                factors: t.factors.clone(),
            },
            inner,
            c,
            unit,
        );
        let body = if mult != 1.0 {
            format!("{}*{}", fmt_num(mult), body)
        } else {
            body
        };
        if i == 0 {
            if t.neg {
                s.push('-');
            }
        } else {
            s.push_str(if t.neg { " - " } else { " + " });
        }
        s.push_str(&body);
    }
    s
}

fn placeholder(inner: Option<&[f64]>, c: &mut usize) -> String {
    let s = match inner {
        Some(v) => fmt_num(v[*c]),
        None => format!("C{}", *c + 1),
    };
    *c += 1;
    s
}

fn print_arg_u(a: &Arg, inner: Option<&[f64]>, c: &mut usize, unit: bool) -> String {
    let scale = if a.scale {
        Some(placeholder(inner, c))
    } else {
        None
    };
    let shift = if a.shift {
        Some(placeholder(inner, c))
    } else {
        None
    };
    let s = print_arg_terms(a, inner, c, unit);
    let mut out = match scale {
        Some(k) if a.terms.len() > 1 => format!("{}*({})", k, s),
        Some(k) => format!("{}*{}", k, s),
        None => s,
    };
    if let Some(k) = shift {
        if k.starts_with('-') {
            out = format!("{} - {}", out, &k[1..]);
        } else {
            out = format!("{} + {}", out, k);
        }
    }
    out
}

pub struct ArgStat {
    pub kind: ArgKind,
    pub min: f64,
    pub max: f64,
    pub mean_abs: f64,
    pub n: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArgKind {
    Un(Un),
    Group(Q),
}

pub fn arg_stats(e: &Expr, cols: &[Vec<f64>], inner: &[f64]) -> Vec<ArgStat> {
    let n = cols.first().map_or(0, |c| c.len());
    let mut acc: Vec<ArgStat> = Vec::new();
    for r in 0..n {
        let mut c = 0usize;
        let mut k = 0usize;
        for t in &e.terms {
            stats_term(t, cols, r, inner, &mut c, &mut k, &mut acc);
        }
    }
    acc
}

fn stats_term(
    t: &Term,
    cols: &[Vec<f64>],
    r: usize,
    inner: &[f64],
    c: &mut usize,
    k: &mut usize,
    acc: &mut Vec<ArgStat>,
) -> f64 {
    let mut v = if t.coef {
        let kv = inner[*c];
        *c += 1;
        kv
    } else {
        1.0
    };
    for f in &t.factors {
        v *= stats_factor(f, cols, r, inner, c, k, acc);
    }
    if t.neg {
        -v
    } else {
        v
    }
}

fn stats_factor(
    f: &Factor,
    cols: &[Vec<f64>],
    r: usize,
    inner: &[f64],
    c: &mut usize,
    k: &mut usize,
    acc: &mut Vec<ArgStat>,
) -> f64 {
    let b = match &f.base {
        Base::Var(i) => cols[*i as usize][r],
        Base::Un(u, a) => {
            let v = stats_arg(a, ArgKind::Un(*u), cols, r, inner, c, k, acc);
            u.apply(v)
        }
        Base::Group(a) => stats_arg(a, ArgKind::Group(f.q), cols, r, inner, c, k, acc),
    };
    pow_q(b, f.q)
}

#[allow(clippy::too_many_arguments)]
fn stats_arg(
    a: &Arg,
    kind: ArgKind,
    cols: &[Vec<f64>],
    r: usize,
    inner: &[f64],
    c: &mut usize,
    k: &mut usize,
    acc: &mut Vec<ArgStat>,
) -> f64 {
    let idx = *k;
    *k += 1;
    if acc.len() <= idx {
        acc.push(ArgStat {
            kind,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            mean_abs: 0.0,
            n: 0,
        });
    }
    let scale = if a.scale {
        let v = inner[*c];
        *c += 1;
        v
    } else {
        1.0
    };
    let shift = if a.shift {
        let v = inner[*c];
        *c += 1;
        v
    } else {
        0.0
    };
    let mut s = 0.0;
    for t in &a.terms {
        s += stats_term(t, cols, r, inner, c, k, acc);
    }
    let v = scale * s + shift;
    if v.is_finite() {
        let st = &mut acc[idx];
        st.min = st.min.min(v);
        st.max = st.max.max(v);
        st.n += 1;
        st.mean_abs += (v.abs() - st.mean_abs) / st.n as f64;
    }
    v
}
