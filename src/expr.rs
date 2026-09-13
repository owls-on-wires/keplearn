#[derive(Clone)]
pub(crate) enum Expr {
    Var(usize),
    Const(f64),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Neg(Box<Expr>),
    Pow(Box<Expr>, Box<Expr>),
    Sqrt(Box<Expr>),
    Sin(Box<Expr>),
    Cos(Box<Expr>),
    Tan(Box<Expr>),
    Exp(Box<Expr>),
    Log(Box<Expr>),
    Arcsin(Box<Expr>),
    Arccos(Box<Expr>),
    Tanh(Box<Expr>),
}

impl Expr {
    pub(crate) fn arity(&self) -> usize {
        match self {
            Expr::Var(i) => *i + 1,
            Expr::Const(_) => 0,
            Expr::Add(a, b)
            | Expr::Sub(a, b)
            | Expr::Mul(a, b)
            | Expr::Div(a, b)
            | Expr::Pow(a, b) => a.arity().max(b.arity()),
            Expr::Neg(a)
            | Expr::Sqrt(a)
            | Expr::Sin(a)
            | Expr::Cos(a)
            | Expr::Tan(a)
            | Expr::Exp(a)
            | Expr::Log(a)
            | Expr::Arcsin(a)
            | Expr::Arccos(a)
            | Expr::Tanh(a) => a.arity(),
        }
    }

    pub(crate) fn node_count(&self) -> usize {
        match self {
            Expr::Var(_) | Expr::Const(_) => 1,
            Expr::Add(a, b)
            | Expr::Sub(a, b)
            | Expr::Mul(a, b)
            | Expr::Div(a, b)
            | Expr::Pow(a, b) => 1 + a.node_count() + b.node_count(),
            Expr::Neg(a)
            | Expr::Sqrt(a)
            | Expr::Sin(a)
            | Expr::Cos(a)
            | Expr::Tan(a)
            | Expr::Exp(a)
            | Expr::Log(a)
            | Expr::Arcsin(a)
            | Expr::Arccos(a)
            | Expr::Tanh(a) => 1 + a.node_count(),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Op {
    LoadVar(u8),
    LoadConst(f64),
    Add,
    Sub,
    Mul,
    Div,
    Neg,
    Pow,
    Sqrt,
    Sin,
    Cos,
    Tan,
    Exp,
    Log,
    Arcsin,
    Arccos,
    Tanh,
}

#[derive(Clone)]
pub(crate) struct CompiledExpr {
    pub(crate) ops: Vec<Op>,
    pub(crate) max_stack: usize,
}

pub(crate) fn compile(expr: &Expr) -> CompiledExpr {
    let mut ops = Vec::new();
    fn emit(expr: &Expr, ops: &mut Vec<Op>) {
        match expr {
            Expr::Var(i) => ops.push(Op::LoadVar(*i as u8)),
            Expr::Const(c) => ops.push(Op::LoadConst(*c)),
            Expr::Add(a, b) => {
                emit(a, ops);
                emit(b, ops);
                ops.push(Op::Add);
            }
            Expr::Sub(a, b) => {
                emit(a, ops);
                emit(b, ops);
                ops.push(Op::Sub);
            }
            Expr::Mul(a, b) => {
                emit(a, ops);
                emit(b, ops);
                ops.push(Op::Mul);
            }
            Expr::Div(a, b) => {
                emit(a, ops);
                emit(b, ops);
                ops.push(Op::Div);
            }
            Expr::Neg(a) => {
                emit(a, ops);
                ops.push(Op::Neg);
            }
            Expr::Pow(a, b) => {
                emit(a, ops);
                emit(b, ops);
                ops.push(Op::Pow);
            }
            Expr::Sqrt(a) => {
                emit(a, ops);
                ops.push(Op::Sqrt);
            }
            Expr::Sin(a) => {
                emit(a, ops);
                ops.push(Op::Sin);
            }
            Expr::Cos(a) => {
                emit(a, ops);
                ops.push(Op::Cos);
            }
            Expr::Tan(a) => {
                emit(a, ops);
                ops.push(Op::Tan);
            }
            Expr::Exp(a) => {
                emit(a, ops);
                ops.push(Op::Exp);
            }
            Expr::Log(a) => {
                emit(a, ops);
                ops.push(Op::Log);
            }
            Expr::Arcsin(a) => {
                emit(a, ops);
                ops.push(Op::Arcsin);
            }
            Expr::Arccos(a) => {
                emit(a, ops);
                ops.push(Op::Arccos);
            }
            Expr::Tanh(a) => {
                emit(a, ops);
                ops.push(Op::Tanh);
            }
        }
    }
    emit(expr, &mut ops);
    let mut depth = 0usize;
    let mut max_depth = 0usize;
    for op in &ops {
        match op {
            Op::LoadVar(_) | Op::LoadConst(_) => {
                depth += 1;
            }
            Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Pow => {
                depth -= 1;
            }
            _ => {}
        }
        if depth > max_depth {
            max_depth = depth;
        }
    }
    CompiledExpr {
        ops,
        max_stack: max_depth,
    }
}

pub(crate) struct Evaluator {
    buffers: Vec<Vec<f64>>,
}

impl Evaluator {
    pub(crate) fn new(max_stack: usize, max_n: usize) -> Self {
        Evaluator {
            buffers: (0..max_stack).map(|_| vec![0.0; max_n]).collect(),
        }
    }

    pub(crate) fn ensure_capacity(&mut self, max_stack: usize, n: usize) {
        while self.buffers.len() < max_stack {
            self.buffers.push(vec![0.0; n]);
        }
        for buf in &mut self.buffers {
            if buf.len() < n {
                buf.resize(n, 0.0);
            }
        }
    }

    #[inline]
    pub(crate) fn eval<'a>(&'a mut self, ops: &[Op], vars: &[&[f64]], n: usize) -> &'a [f64] {
        let mut sp: usize = 0;
        for op in ops {
            match *op {
                Op::LoadVar(v) => {
                    self.buffers[sp][..n].copy_from_slice(&vars[v as usize][..n]);
                    sp += 1;
                }
                Op::LoadConst(c) => {
                    self.buffers[sp][..n].fill(c);
                    sp += 1;
                }
                Op::Add => {
                    sp -= 1;
                    let (l, r) = self.buffers.split_at_mut(sp);
                    let a = &mut l[sp - 1][..n];
                    let b = &r[0][..n];
                    for i in 0..n {
                        a[i] += b[i];
                    }
                }
                Op::Sub => {
                    sp -= 1;
                    let (l, r) = self.buffers.split_at_mut(sp);
                    let a = &mut l[sp - 1][..n];
                    let b = &r[0][..n];
                    for i in 0..n {
                        a[i] -= b[i];
                    }
                }
                Op::Mul => {
                    sp -= 1;
                    let (l, r) = self.buffers.split_at_mut(sp);
                    let a = &mut l[sp - 1][..n];
                    let b = &r[0][..n];
                    for i in 0..n {
                        a[i] *= b[i];
                    }
                }
                Op::Div => {
                    sp -= 1;
                    let (l, r) = self.buffers.split_at_mut(sp);
                    let a = &mut l[sp - 1][..n];
                    let b = &r[0][..n];
                    for i in 0..n {
                        a[i] /= b[i];
                    }
                }
                Op::Neg => {
                    let a = &mut self.buffers[sp - 1][..n];
                    for i in 0..n {
                        a[i] = -a[i];
                    }
                }
                Op::Pow => {
                    sp -= 1;
                    let (l, r) = self.buffers.split_at_mut(sp);
                    let a = &mut l[sp - 1][..n];
                    let b = &r[0][..n];
                    for i in 0..n {
                        a[i] = a[i].powf(b[i]);
                    }
                }
                Op::Sqrt => {
                    let a = &mut self.buffers[sp - 1][..n];
                    for i in 0..n {
                        a[i] = a[i].sqrt();
                    }
                }
                Op::Sin => {
                    let a = &mut self.buffers[sp - 1][..n];
                    for i in 0..n {
                        a[i] = a[i].sin();
                    }
                }
                Op::Cos => {
                    let a = &mut self.buffers[sp - 1][..n];
                    for i in 0..n {
                        a[i] = a[i].cos();
                    }
                }
                Op::Tan => {
                    let a = &mut self.buffers[sp - 1][..n];
                    for i in 0..n {
                        a[i] = a[i].tan();
                    }
                }
                Op::Exp => {
                    let a = &mut self.buffers[sp - 1][..n];
                    for i in 0..n {
                        a[i] = a[i].exp();
                    }
                }
                Op::Log => {
                    let a = &mut self.buffers[sp - 1][..n];
                    for i in 0..n {
                        a[i] = a[i].ln();
                    }
                }
                Op::Arcsin => {
                    let a = &mut self.buffers[sp - 1][..n];
                    for i in 0..n {
                        a[i] = a[i].asin();
                    }
                }
                Op::Arccos => {
                    let a = &mut self.buffers[sp - 1][..n];
                    for i in 0..n {
                        a[i] = a[i].acos();
                    }
                }
                Op::Tanh => {
                    let a = &mut self.buffers[sp - 1][..n];
                    for i in 0..n {
                        a[i] = a[i].tanh();
                    }
                }
            }
        }
        &self.buffers[0][..n]
    }
}

pub(crate) struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    pub(crate) fn parse(s: &str) -> Result<Expr, String> {
        let mut p = Parser {
            chars: s.chars().collect(),
            pos: 0,
        };
        let e = p.parse_add()?;
        p.ws();
        if p.pos < p.chars.len() {
            return Err(format!("trailing at {}", p.pos));
        }
        Ok(e)
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn ws(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_whitespace()) {
            self.pos += 1;
        }
    }

    fn parse_add(&mut self) -> Result<Expr, String> {
        let mut e = self.parse_mul()?;
        loop {
            self.ws();
            match self.peek() {
                Some('+') => {
                    self.advance();
                    let r = self.parse_mul()?;
                    e = Expr::Add(Box::new(e), Box::new(r));
                }
                Some('-') => {
                    self.advance();
                    let r = self.parse_mul()?;
                    e = Expr::Sub(Box::new(e), Box::new(r));
                }
                _ => break,
            }
        }
        Ok(e)
    }

    fn parse_mul(&mut self) -> Result<Expr, String> {
        let mut e = self.parse_unary()?;
        loop {
            self.ws();
            match self.peek() {
                Some('*') => {
                    let s = self.pos;
                    self.advance();
                    if self.peek() == Some('*') {
                        self.pos = s;
                        break;
                    }
                    let r = self.parse_unary()?;
                    e = Expr::Mul(Box::new(e), Box::new(r));
                }
                Some('/') => {
                    self.advance();
                    let r = self.parse_unary()?;
                    e = Expr::Div(Box::new(e), Box::new(r));
                }
                _ => break,
            }
        }
        Ok(e)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        self.ws();
        if self.peek() == Some('-') {
            self.advance();
            let e = self.parse_unary()?;
            Ok(Expr::Neg(Box::new(e)))
        } else {
            self.parse_pow()
        }
    }

    fn parse_pow(&mut self) -> Result<Expr, String> {
        let base = self.parse_atom()?;
        self.ws();
        if self.pos + 1 < self.chars.len()
            && self.chars[self.pos] == '*'
            && self.chars[self.pos + 1] == '*'
        {
            self.pos += 2;
            let exp = self.parse_unary()?;
            Ok(Expr::Pow(Box::new(base), Box::new(exp)))
        } else {
            Ok(base)
        }
    }

    fn parse_atom(&mut self) -> Result<Expr, String> {
        self.ws();
        match self.peek() {
            Some('(') => {
                self.advance();
                let e = self.parse_add()?;
                self.ws();
                if self.advance() != Some(')') {
                    return Err("expected ')'".into());
                }
                Ok(e)
            }
            Some(c) if c.is_ascii_digit() || c == '.' => {
                let mut s = String::new();
                while matches!(self.peek(), Some(c) if c.is_ascii_digit() || c == '.') {
                    s.push(self.advance().unwrap());
                }
                Ok(Expr::Const(
                    s.parse::<f64>().map_err(|e| format!("bad num: {}", e))?,
                ))
            }
            Some(c) if c.is_ascii_alphabetic() => {
                let mut name = String::new();
                while matches!(self.peek(), Some(c) if c.is_ascii_alphabetic()) {
                    name.push(self.advance().unwrap());
                }
                if name == "x" {
                    let mut ds = String::new();
                    while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                        ds.push(self.advance().unwrap());
                    }
                    if ds.is_empty() {
                        return Err("expected digit after 'x'".into());
                    }
                    let idx: usize = ds
                        .parse()
                        .map_err(|_| format!("bad variable index x{}", ds))?;
                    if idx == 0 {
                        return Err("variable indices start at x1".into());
                    }
                    return Ok(Expr::Var(idx - 1));
                }
                self.ws();
                if self.advance() != Some('(') {
                    return Err(format!("expected '(' after '{}'", name));
                }
                let arg = self.parse_add()?;
                self.ws();
                if self.advance() != Some(')') {
                    return Err(format!("expected ')' for '{}'", name));
                }
                match name.as_str() {
                    "sqrt" => Ok(Expr::Sqrt(Box::new(arg))),
                    "sin" => Ok(Expr::Sin(Box::new(arg))),
                    "cos" => Ok(Expr::Cos(Box::new(arg))),
                    "tan" => Ok(Expr::Tan(Box::new(arg))),
                    "exp" => Ok(Expr::Exp(Box::new(arg))),
                    "log" => Ok(Expr::Log(Box::new(arg))),
                    "arcsin" => Ok(Expr::Arcsin(Box::new(arg))),
                    "arccos" => Ok(Expr::Arccos(Box::new(arg))),
                    "tanh" => Ok(Expr::Tanh(Box::new(arg))),
                    "abs" => Ok(Expr::Sqrt(Box::new(Expr::Pow(
                        Box::new(arg),
                        Box::new(Expr::Const(2.0)),
                    )))),
                    _ => Err(format!("unknown func: {}", name)),
                }
            }
            other => Err(format!("unexpected: {:?}", other)),
        }
    }
}

pub(crate) fn simplify_expr(e: Expr) -> Expr {
    use Expr::*;
    fn z(e: &Expr) -> bool {
        matches!(e, Const(c) if * c == 0.0)
    }
    fn one(e: &Expr) -> bool {
        matches!(e, Const(c) if * c == 1.0)
    }
    match e {
        Add(a, b) => {
            let (a, b) = (simplify_expr(*a), simplify_expr(*b));
            if z(&a) {
                b
            } else if z(&b) {
                a
            } else {
                Add(Box::new(a), Box::new(b))
            }
        }
        Sub(a, b) => {
            let (a, b) = (simplify_expr(*a), simplify_expr(*b));
            if z(&b) {
                a
            } else if z(&a) {
                simplify_expr(Neg(Box::new(b)))
            } else {
                Sub(Box::new(a), Box::new(b))
            }
        }
        Mul(a, b) => {
            let (a, b) = (simplify_expr(*a), simplify_expr(*b));
            if z(&a) || z(&b) {
                Const(0.0)
            } else if one(&a) {
                b
            } else if one(&b) {
                a
            } else {
                Mul(Box::new(a), Box::new(b))
            }
        }
        Div(a, b) => {
            let (a, b) = (simplify_expr(*a), simplify_expr(*b));
            if z(&a) {
                Const(0.0)
            } else if one(&b) {
                a
            } else {
                Div(Box::new(a), Box::new(b))
            }
        }
        Pow(a, b) => {
            let (a, b) = (simplify_expr(*a), simplify_expr(*b));
            if z(&b) || one(&a) {
                Const(1.0)
            } else if one(&b) {
                a
            } else {
                Pow(Box::new(a), Box::new(b))
            }
        }
        Neg(a) => {
            let a = simplify_expr(*a);
            match a {
                Const(c) => Const(-c),
                a => Neg(Box::new(a)),
            }
        }
        Sqrt(a) => Sqrt(Box::new(simplify_expr(*a))),
        Sin(a) => Sin(Box::new(simplify_expr(*a))),
        Cos(a) => Cos(Box::new(simplify_expr(*a))),
        Tan(a) => Tan(Box::new(simplify_expr(*a))),
        Exp(a) => Exp(Box::new(simplify_expr(*a))),
        Log(a) => Log(Box::new(simplify_expr(*a))),
        Arcsin(a) => Arcsin(Box::new(simplify_expr(*a))),
        Arccos(a) => Arccos(Box::new(simplify_expr(*a))),
        Tanh(a) => Tanh(Box::new(simplify_expr(*a))),
        leaf => leaf,
    }
}

pub(crate) fn to_model_string(e: &Expr) -> String {
    use crate::model::{fmt_const, fmt_exp};
    use Expr::*;
    match e {
        Var(i) => format!("x{}", i + 1),
        Const(c) => {
            if *c == 0.0 {
                fmt_const(0.0)
            } else if *c < 0.0 {
                format!("(-{})", fmt_const(-c))
            } else {
                fmt_const(*c)
            }
        }
        Add(a, b) => format!("({}+{})", to_model_string(a), to_model_string(b)),
        Sub(a, b) => format!("({}-{})", to_model_string(a), to_model_string(b)),
        Mul(a, b) => format!("({}*{})", to_model_string(a), to_model_string(b)),
        Div(a, b) => format!("({}/{})", to_model_string(a), to_model_string(b)),
        Neg(a) => format!("(-{})", to_model_string(a)),
        Pow(a, b) => {
            let exp = match &**b {
                Const(c) => fmt_exp(*c),
                other => to_model_string(other),
            };
            format!("(({})**({}))", to_model_string(a), exp)
        }
        Sqrt(a) => format!("sqrt({})", to_model_string(a)),
        Sin(a) => format!("sin({})", to_model_string(a)),
        Cos(a) => format!("cos({})", to_model_string(a)),
        Tan(a) => format!("tan({})", to_model_string(a)),
        Exp(a) => format!("exp({})", to_model_string(a)),
        Log(a) => format!("log({})", to_model_string(a)),
        Arcsin(a) => format!("arcsin({})", to_model_string(a)),
        Arccos(a) => format!("arccos({})", to_model_string(a)),
        Tanh(a) => format!("tanh({})", to_model_string(a)),
    }
}

pub(crate) fn simplify_model(model: &str) -> Option<String> {
    let e = Parser::parse(model).ok()?;
    let before = e.node_count();
    let s = simplify_expr(e);
    if s.node_count() >= before {
        return None;
    }
    let out = to_model_string(&s);
    Parser::parse(&out).ok()?;
    Some(out)
}
