use crate::*;

#[derive(Clone)]
pub(crate) enum Red {
    Div {
        desc: String,
        gexpr: String,
        cost: usize,
    },
    Shell {
        name: String,
    },
    Affc {
        c: f64,
    },
    Mat {
        desc: String,
    },
}

pub(crate) fn red_desc(r: &Red) -> String {
    match r {
        Red::Div { desc, .. } => format!("div:{}", desc),
        Red::Shell { name, .. } => format!("shell:{}", name),
        Red::Affc { c, .. } => format!("affc:{}", c),
        Red::Mat { desc, .. } => format!("mat:{}", desc),
    }
}

#[derive(Clone)]
pub(crate) struct VC {
    pub(crate) gexpr: String,
    pub(crate) desc: String,
    pub(crate) data: Rc<Vec<f64>>,
}

pub(crate) struct Node {
    pub(crate) target: Rc<Vec<f64>>,
    pub(crate) chain: Vec<Red>,
    pub(crate) vcols: Vec<VC>,
    pub(crate) depth: usize,
    pub(crate) complexity: usize,
    pub(crate) probe_r2: f64,
    pub(crate) plane: u8,
    pub(crate) priority: f64,
    pub(crate) seq: u64,
    pub(crate) desc: String,
    pub(crate) forced_finalist: Vec<(String, f64)>,
}

#[derive(Clone)]
pub(crate) struct Hit {
    pub(crate) expr: String,
    pub(crate) r2: f64,
    pub(crate) scale: f64,
    pub(crate) offset: f64,
    pub(crate) columns: Vec<usize>,
    pub(crate) gate_r2: f64,
}

pub(crate) fn subst_pool_vars(expr: &str, subs: &[String]) -> String {
    let n_vars = subs.len();
    let chars: Vec<char> = expr.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(n + 32);
    let mut i = 0;
    while i < n {
        if chars[i] == 'x' && i + 1 < n && chars[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            while j < n && chars[j].is_ascii_digit() {
                j += 1;
            }
            let num_str: String = chars[i + 1..j].iter().collect();
            if let Ok(vi) = num_str.parse::<usize>() {
                if vi >= 1 && vi <= n_vars {
                    out.push_str(&subs[vi - 1]);
                    i = j;
                    continue;
                }
            }
            out.push('x');
            i += 1;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

pub(crate) fn fmt_const(x: f64) -> String {
    if !x.is_finite() {
        return "0".to_string();
    }
    format!("{:.12}", x)
}

#[inline]
pub(crate) fn affine_trivial(scale: f64, offset: f64) -> bool {
    (scale - 1.0).abs() < AFFINE_SCALE_TOL && offset.abs() < AFFINE_OFFSET_TOL
}

pub(crate) fn invert_chain(chain: &[Red], upto: usize, inner: String) -> String {
    let mut e = inner;
    for red in chain[..upto].iter().rev() {
        e = match red {
            Red::Div { gexpr, .. } => format!("({}*{})", gexpr, e),
            Red::Shell { name, .. } => shell_invert(name, &e),
            Red::Affc { c, .. } => format!("(({}-1)/({}))", e, fmt_const(*c)),
            Red::Mat { .. } => e,
        };
    }
    e
}

#[derive(Clone)]
pub(crate) struct Cand {
    pub(crate) model: String,
    pub(crate) desc: String,
    pub(crate) expr: String,
    pub(crate) columns: Vec<usize>,
    pub(crate) scale: f64,
    pub(crate) offset: f64,
    pub(crate) fit_r2: f64,
    pub(crate) gate_r2: f64,
    pub(crate) nodes: usize,
    pub(crate) root_raw: bool,
}

pub(crate) fn build_candidate(
    chain: &[Red],
    desc: &str,
    hit: &Hit,
    vcols: &[VC],
    n_cols: usize,
) -> Option<Cand> {
    let subs: Option<Vec<String>> = hit
        .columns
        .iter()
        .map(|&c| {
            if c < n_cols {
                Some(format!("x{}", c + 1))
            } else {
                vcols.get(c - n_cols).map(|v| v.gexpr.clone())
            }
        })
        .collect();
    let leaf_g = subst_pool_vars(&hit.expr, &subs?);
    let baked_leaf = if affine_trivial(hit.scale, hit.offset) {
        format!("({})", leaf_g)
    } else {
        format!(
            "({}*({})+{})",
            fmt_const(hit.scale),
            leaf_g,
            fmt_const(hit.offset)
        )
    };
    let (model, fit_r2, gate_r2);
    if chain.is_empty() {
        model = baked_leaf;
        fit_r2 = hit.r2;
        gate_r2 = hit.gate_r2;
    } else {
        model = invert_chain(chain, chain.len(), baked_leaf);
        fit_r2 = hit.r2;
        gate_r2 = hit.gate_r2;
    }
    let parsed = Parser::parse(&model).ok()?;
    let nodes = parsed.node_count();
    Some(Cand {
        model,
        desc: desc.to_string(),
        expr: hit.expr.clone(),
        columns: hit.columns.clone(),
        scale: hit.scale,
        offset: hit.offset,
        fit_r2,
        gate_r2,
        nodes,
        root_raw: chain.is_empty() && hit.columns.iter().all(|&c| c < n_cols),
    })
}

pub(crate) fn fmt_exp(e: f64) -> String {
    if (e - e.round()).abs() < 1e-9 {
        format!("{}", e.round() as i64)
    } else {
        format!("{}", e)
    }
}
