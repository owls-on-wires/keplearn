use crate::forms::nf::Un;

#[derive(Clone, Debug, PartialEq)]
pub enum Ast {
    Num(f64),
    Var(u8),
    Add(Box<Ast>, Box<Ast>),
    Sub(Box<Ast>, Box<Ast>),
    Mul(Box<Ast>, Box<Ast>),
    Div(Box<Ast>, Box<Ast>),
    Pow(Box<Ast>, Box<Ast>),
    Neg(Box<Ast>),
    Un(Un, Box<Ast>),
    Sqrt(Box<Ast>),
}

impl Ast {
    pub fn eval(&self, cols: &[Vec<f64>], r: usize) -> f64 {
        match self {
            Ast::Num(v) => *v,
            Ast::Var(i) => cols[*i as usize][r],
            Ast::Add(a, b) => a.eval(cols, r) + b.eval(cols, r),
            Ast::Sub(a, b) => a.eval(cols, r) - b.eval(cols, r),
            Ast::Mul(a, b) => a.eval(cols, r) * b.eval(cols, r),
            Ast::Div(a, b) => a.eval(cols, r) / b.eval(cols, r),
            Ast::Pow(a, b) => a.eval(cols, r).powf(b.eval(cols, r)),
            Ast::Neg(a) => -a.eval(cols, r),
            Ast::Un(u, a) => u.apply(a.eval(cols, r)),
            Ast::Sqrt(a) => a.eval(cols, r).sqrt(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Op(char),
    Pow,
    LPar,
    RPar,
}

fn lex(s: &str) -> Result<Vec<Tok>, String> {
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < cs.len() {
        let c = cs[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || (c == '.' && i + 1 < cs.len() && cs[i + 1].is_ascii_digit())
        {
            let st = i;
            while i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == '.') {
                i += 1;
            }
            if i < cs.len() && (cs[i] == 'e' || cs[i] == 'E') {
                let save = i;
                i += 1;
                if i < cs.len() && (cs[i] == '+' || cs[i] == '-') {
                    i += 1;
                }
                if i < cs.len() && cs[i].is_ascii_digit() {
                    while i < cs.len() && cs[i].is_ascii_digit() {
                        i += 1;
                    }
                } else {
                    i = save;
                }
            }
            let t: String = cs[st..i].iter().collect();
            out.push(Tok::Num(
                t.parse::<f64>().map_err(|e| format!("{}: {}", t, e))?,
            ));
        } else if c.is_ascii_alphabetic() || c == '_' {
            let st = i;
            while i < cs.len() && (cs[i].is_ascii_alphanumeric() || cs[i] == '_') {
                i += 1;
            }
            out.push(Tok::Ident(cs[st..i].iter().collect()));
        } else if c == '*' && i + 1 < cs.len() && cs[i + 1] == '*' {
            out.push(Tok::Pow);
            i += 2;
        } else if c == '^' {
            out.push(Tok::Pow);
            i += 1;
        } else if "+-*/".contains(c) {
            out.push(Tok::Op(c));
            i += 1;
        } else if c == '(' {
            out.push(Tok::LPar);
            i += 1;
        } else if c == ')' {
            out.push(Tok::RPar);
            i += 1;
        } else {
            return Err(format!("unexpected character '{}'", c));
        }
    }
    Ok(out)
}

struct P {
    toks: Vec<Tok>,
    i: usize,
}

impl P {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.i)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.i).cloned();
        self.i += 1;
        t
    }

    fn expr(&mut self) -> Result<Ast, String> {
        let mut lhs = self.term()?;
        while let Some(Tok::Op(c)) = self.peek() {
            let c = *c;
            if c != '+' && c != '-' {
                break;
            }
            self.next();
            let rhs = self.term()?;
            lhs = if c == '+' {
                Ast::Add(Box::new(lhs), Box::new(rhs))
            } else {
                Ast::Sub(Box::new(lhs), Box::new(rhs))
            };
        }
        Ok(lhs)
    }

    fn term(&mut self) -> Result<Ast, String> {
        let mut lhs = self.unary()?;
        while let Some(Tok::Op(c)) = self.peek() {
            let c = *c;
            if c != '*' && c != '/' {
                break;
            }
            self.next();
            let rhs = self.unary()?;
            lhs = if c == '*' {
                Ast::Mul(Box::new(lhs), Box::new(rhs))
            } else {
                Ast::Div(Box::new(lhs), Box::new(rhs))
            };
        }
        Ok(lhs)
    }

    fn unary(&mut self) -> Result<Ast, String> {
        if let Some(Tok::Op('-')) = self.peek() {
            self.next();
            return Ok(Ast::Neg(Box::new(self.unary()?)));
        }
        if let Some(Tok::Op('+')) = self.peek() {
            self.next();
            return self.unary();
        }
        self.power()
    }

    fn power(&mut self) -> Result<Ast, String> {
        let base = self.atom()?;
        if let Some(Tok::Pow) = self.peek() {
            self.next();
            let e = self.unary()?;
            return Ok(Ast::Pow(Box::new(base), Box::new(e)));
        }
        Ok(base)
    }

    fn atom(&mut self) -> Result<Ast, String> {
        match self.next() {
            Some(Tok::Num(v)) => Ok(Ast::Num(v)),
            Some(Tok::LPar) => {
                let e = self.expr()?;
                match self.next() {
                    Some(Tok::RPar) => Ok(e),
                    t => Err(format!("expected ')', got {:?}", t)),
                }
            }
            Some(Tok::Ident(name)) => {
                if let Some(Tok::LPar) = self.peek() {
                    self.next();
                    let a = self.expr()?;
                    match self.next() {
                        Some(Tok::RPar) => {}
                        t => return Err(format!("expected ')', got {:?}", t)),
                    }
                    let b = Box::new(a);
                    return match name.as_str() {
                        "exp" => Ok(Ast::Un(Un::Exp, b)),
                        "log" | "ln" => Ok(Ast::Un(Un::Log, b)),
                        "sin" => Ok(Ast::Un(Un::Sin, b)),
                        "cos" => Ok(Ast::Un(Un::Cos, b)),
                        "arcsin" | "asin" => Ok(Ast::Un(Un::Arcsin, b)),
                        "arccos" | "acos" => Ok(Ast::Un(Un::Arccos, b)),
                        "arctan" | "atan" => Ok(Ast::Un(Un::Arctan, b)),
                        "tanh" => Ok(Ast::Un(Un::Tanh, b)),
                        "sqrt" => Ok(Ast::Sqrt(b)),
                        _ => Err(format!("unknown function {}", name)),
                    };
                }
                match name.as_str() {
                    "pi" => Ok(Ast::Num(std::f64::consts::PI)),
                    "e" | "E" => Ok(Ast::Num(std::f64::consts::E)),
                    _ => {
                        if let Some(rest) = name.strip_prefix('x') {
                            if let Ok(k) = rest.parse::<usize>() {
                                if k >= 1 && k <= 255 {
                                    return Ok(Ast::Var((k - 1) as u8));
                                }
                            }
                        }
                        Err(format!("unknown identifier {}", name))
                    }
                }
            }
            t => Err(format!("unexpected token {:?}", t)),
        }
    }
}

pub fn parse(s: &str) -> Result<Ast, String> {
    let toks = lex(s)?;
    let mut p = P { toks, i: 0 };
    let e = p.expr()?;
    if p.i != p.toks.len() {
        return Err(format!("trailing tokens at {}", p.i));
    }
    Ok(e)
}
