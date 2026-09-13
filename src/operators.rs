use crate::*;

struct UnaryOpDef {
    name: &'static str,
    cost: usize,
    apply: fn(f64) -> f64,
    render: fn(&str) -> String,
    factor: bool,
}

struct BinOpDef {
    name: &'static str,
    cost: usize,
    swap: bool,
    apply: fn(f64, f64) -> f64,
    render: fn(&str, &str) -> String,
}

pub(crate) type UnaryOp = (&'static str, fn(f64) -> f64, usize);

pub(crate) type BinOp = (&'static str, fn(f64, f64) -> f64, usize, bool);

fn unary_registry() -> &'static [UnaryOpDef] {
    static REG: OnceLock<Vec<UnaryOpDef>> = OnceLock::new();
    REG.get_or_init(|| {
        vec![
            UnaryOpDef {
                name: "x",
                cost: 1,
                factor: true,
                apply: |x| x,
                render: |g| g.to_string(),
            },
            UnaryOpDef {
                name: "inv",
                cost: 2,
                factor: true,
                apply: |x| if x.abs() > 1e-10 { 1.0 / x } else { f64::NAN },
                render: |g| format!("(1/{})", g),
            },
            UnaryOpDef {
                name: "sqrt",
                cost: 2,
                factor: true,
                apply: |x| if x >= 0.0 { x.sqrt() } else { f64::NAN },
                render: |g| format!("sqrt({})", g),
            },
            UnaryOpDef {
                name: "abs",
                cost: 2,
                factor: true,
                apply: |x| x.abs(),
                render: |g| format!("abs({})", g),
            },
            UnaryOpDef {
                name: "neg",
                cost: 2,
                factor: false,
                apply: |x| -x,
                render: |g| format!("(-{})", g),
            },
            UnaryOpDef {
                name: "sin",
                cost: 2,
                factor: true,
                apply: |x| x.sin(),
                render: |g| format!("sin({})", g),
            },
            UnaryOpDef {
                name: "cos",
                cost: 2,
                factor: true,
                apply: |x| x.cos(),
                render: |g| format!("cos({})", g),
            },
            UnaryOpDef {
                name: "exp",
                cost: 2,
                factor: true,
                apply: |x| if x < 50.0 { x.exp() } else { f64::NAN },
                render: |g| format!("exp({})", g),
            },
            UnaryOpDef {
                name: "log",
                cost: 2,
                factor: true,
                apply: |x| if x > 1e-30 { x.ln() } else { f64::NAN },
                render: |g| format!("log({})", g),
            },
            UnaryOpDef {
                name: "tanh",
                cost: 2,
                factor: true,
                apply: |x| x.tanh(),
                render: |g| format!("tanh({})", g),
            },
        ]
    })
}

fn div_raw(a: f64, b: f64) -> f64 {
    if b.abs() > DIV_GUARD_RAW {
        a / b
    } else {
        f64::NAN
    }
}

fn div_mat(a: f64, b: f64) -> f64 {
    if b.abs() > DIV_GUARD_MAT {
        a / b
    } else {
        f64::NAN
    }
}

fn bin_ops(div: fn(f64, f64) -> f64) -> Vec<BinOpDef> {
    vec![
        BinOpDef {
            name: "mul",
            cost: 3,
            swap: false,
            apply: |a, b| a * b,
            render: |a, b| format!("({}*{})", a, b),
        },
        BinOpDef {
            name: "div",
            cost: 3,
            swap: true,
            apply: div,
            render: |a, b| format!("({}/{})", a, b),
        },
        BinOpDef {
            name: "sub",
            cost: 3,
            swap: true,
            apply: |a, b| a - b,
            render: |a, b| format!("({}-{})", a, b),
        },
        BinOpDef {
            name: "add",
            cost: 3,
            swap: false,
            apply: |a, b| a + b,
            render: |a, b| format!("({}+{})", a, b),
        },
    ]
}

fn pair_registry() -> &'static [BinOpDef] {
    static REG: OnceLock<Vec<BinOpDef>> = OnceLock::new();
    REG.get_or_init(|| bin_ops(div_raw))
}

fn mat_registry() -> &'static [BinOpDef] {
    static REG: OnceLock<Vec<BinOpDef>> = OnceLock::new();
    REG.get_or_init(|| bin_ops(div_mat))
}

pub(crate) fn unary_factor_ops() -> Vec<UnaryOp> {
    unary_registry()
        .iter()
        .filter(|o| o.factor)
        .map(|o| (o.name, o.apply, o.cost))
        .collect()
}

pub(crate) fn unary_gexpr(name: &str, g: &str) -> String {
    match unary_registry().iter().find(|o| o.name == name) {
        Some(o) => (o.render)(g),
        None => g.to_string(),
    }
}

pub(crate) fn pair_factor_ops() -> Vec<BinOp> {
    pair_registry()
        .iter()
        .map(|o| (o.name, o.apply, o.cost, o.swap))
        .collect()
}

pub(crate) fn mat_pair_ops() -> Vec<BinOp> {
    mat_registry()
        .iter()
        .map(|o| (o.name, o.apply, o.cost, o.swap))
        .collect()
}

pub(crate) fn pair_gexpr(name: &str, a: &str, b: &str) -> String {
    match pair_registry()
        .iter()
        .chain(mat_registry().iter())
        .find(|o| o.name == name)
    {
        Some(o) => (o.render)(a, b),
        None => format!("({}*{})", a, b),
    }
}

struct ShellDef {
    name: &'static str,
    cost: usize,
    apply: fn(f64) -> f64,
    invert: fn(&str) -> String,
    spawn_default: bool,
}

fn shell_registry() -> &'static [ShellDef] {
    static REG: OnceLock<Vec<ShellDef>> = OnceLock::new();
    REG.get_or_init(|| {
        vec![
            ShellDef {
                name: "sq",
                cost: 2,
                spawn_default: true,
                apply: |y| y * y,
                invert: |e| format!("sqrt({})", e),
            },
            ShellDef {
                name: "inv",
                cost: 2,
                spawn_default: true,
                apply: |y| if y.abs() > 1e-30 { 1.0 / y } else { f64::NAN },
                invert: |e| format!("(1/{})", e),
            },
            ShellDef {
                name: "log",
                cost: 2,
                spawn_default: true,
                apply: |y| if y > 1e-30 { y.ln() } else { f64::NAN },
                invert: |e| format!("exp({})", e),
            },
            ShellDef {
                name: "sqrt",
                cost: 2,
                spawn_default: true,
                apply: |y| if y >= 0.0 { y.sqrt() } else { f64::NAN },
                invert: |e| format!("(({})**2)", e),
            },
            ShellDef {
                name: "exp",
                cost: 2,
                spawn_default: true,
                apply: |y| if y < 50.0 { y.exp() } else { f64::NAN },
                invert: |e| format!("log({})", e),
            },
            ShellDef {
                name: "sin",
                cost: 2,
                spawn_default: true,
                apply: |y| {
                    if (-std::f64::consts::FRAC_PI_2..=std::f64::consts::FRAC_PI_2).contains(&y) {
                        y.sin()
                    } else {
                        f64::NAN
                    }
                },
                invert: |e| format!("arcsin({})", e),
            },
            ShellDef {
                name: "cos",
                cost: 2,
                spawn_default: true,
                apply: |y| {
                    if (0.0..=std::f64::consts::PI).contains(&y) {
                        y.cos()
                    } else {
                        f64::NAN
                    }
                },
                invert: |e| format!("arccos({})", e),
            },
            ShellDef {
                name: "atanh",
                cost: 2,
                spawn_default: true,
                apply: |y| if y.abs() < 1.0 { y.atanh() } else { f64::NAN },
                invert: |e| format!("tanh({})", e),
            },
        ]
    })
}

pub(crate) fn shell_apply(name: &str, y: f64) -> f64 {
    match shell_registry().iter().find(|s| s.name == name) {
        Some(s) => (s.apply)(y),
        None => y,
    }
}

pub(crate) fn shell_invert(name: &str, inner: &str) -> String {
    match shell_registry().iter().find(|s| s.name == name) {
        Some(s) => (s.invert)(inner),
        None => inner.to_string(),
    }
}

pub(crate) fn shells() -> Vec<(&'static str, usize)> {
    shell_registry()
        .iter()
        .filter(|s| s.spawn_default)
        .map(|s| (s.name, s.cost))
        .collect()
}
