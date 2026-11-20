pub(crate) fn json_esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

pub(crate) fn jnum(x: f64) -> String {
    if x.is_finite() {
        format!("{:.6}", x)
    } else {
        "-1".to_string()
    }
}

pub(crate) fn jnum_rt(x: f64) -> String {
    if x.is_finite() {
        format!("{}", x)
    } else {
        "null".to_string()
    }
}

pub(crate) fn jopt_f(x: Option<f64>) -> String {
    match x {
        Some(v) => jnum_rt(v),
        None => "null".to_string(),
    }
}

pub(crate) fn jopt_u(x: Option<usize>) -> String {
    match x {
        Some(v) => v.to_string(),
        None => "null".to_string(),
    }
}

pub(crate) fn json_pretty(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    let mut indent = 0usize;
    let mut in_str = false;
    let mut esc = false;
    let mut chars = s.chars().peekable();
    fn newline(out: &mut String, n: usize) {
        out.push('\n');
        for _ in 0..n {
            out.push_str("  ");
        }
    }
    while let Some(c) = chars.next() {
        if in_str {
            out.push(c);
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_str = true;
                out.push(c);
            }
            '{' | '[' => {
                out.push(c);
                let closer = if c == '{' { '}' } else { ']' };
                if chars.peek() == Some(&closer) {
                    out.push(closer);
                    chars.next();
                } else {
                    indent += 1;
                    newline(&mut out, indent);
                }
            }
            '}' | ']' => {
                indent = indent.saturating_sub(1);
                newline(&mut out, indent);
                out.push(c);
            }
            ',' => {
                out.push(c);
                newline(&mut out, indent);
            }
            ':' => {
                out.push_str(": ");
            }
            c if c.is_whitespace() => {}
            _ => out.push(c),
        }
    }
    out
}

pub(crate) fn symbolic_model(model: &str, columns: &[String]) -> String {
    let b: Vec<char> = model.chars().collect();
    let mut out = String::with_capacity(model.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let prev_word = i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == '_');
        if (c.is_ascii_digit() || (c == '.' && i + 1 < b.len() && b[i + 1].is_ascii_digit()))
            && !prev_word
        {
            let start = i;
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == '.') {
                i += 1;
            }
            if i < b.len() && (b[i] == 'e' || b[i] == 'E') {
                let mut j = i + 1;
                if j < b.len() && (b[j] == '+' || b[j] == '-') {
                    j += 1;
                }
                if j < b.len() && b[j].is_ascii_digit() {
                    while j < b.len() && b[j].is_ascii_digit() {
                        j += 1;
                    }
                    i = j;
                }
            }
            let tok: String = b[start..i].iter().collect();
            match tok
                .parse::<f64>()
                .ok()
                .and_then(crate::forms::nf::symbolic_constant)
            {
                Some(sym) if is_atomic(&sym) => out.push_str(&sym),
                Some(sym) => out.push_str(&format!("({})", sym)),
                None => {
                    match tok
                        .parse::<f64>()
                        .ok()
                        .and_then(crate::forms::nf::physical_constant)
                        .filter(|(_, n)| !columns.iter().any(|c| c == n))
                    {
                        Some((t, _)) if is_atomic(&t) => out.push_str(&t),
                        Some((t, _)) => out.push_str(&format!("({})", t)),
                        None => match repeating_fraction(&tok) {
                            Some(fr) => out.push_str(&format!("({})", fr)),
                            None => out.push_str(&tok),
                        },
                    }
                }
            }
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

fn repeating_fraction(tok: &str) -> Option<String> {
    let v: f64 = tok.parse().ok()?;
    if tok.chars().filter(|c| c.is_ascii_digit()).count() < 8 || v == 0.0 {
        return None;
    }
    (2..=12i64)
        .filter(|q| {
            let mut r = *q;
            while r % 2 == 0 {
                r /= 2;
            }
            while r % 5 == 0 {
                r /= 5;
            }
            r > 1
        })
        .find_map(|q| {
            let p = (v * q as f64).round();
            (p != 0.0 && p.abs() <= 144.0 && ((p / q as f64 - v) / v).abs() <= 1e-11)
                .then(|| format!("{}/{}", p as i64, q))
        })
}

fn is_atomic(e: &str) -> bool {
    if e.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return true;
    }
    let Some(open) = e.find('(') else {
        return false;
    };
    if !e.ends_with(')')
        || !e[..open]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        || open == 0
    {
        return false;
    }
    let mut depth = 0;
    for (i, c) in e.char_indices().skip(open) {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 && i + 1 != e.len() {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

pub(crate) fn named_model(model: &str, names: &[String]) -> String {
    let b: Vec<char> = model.chars().collect();
    let mut out = String::with_capacity(model.len());
    let mut i = 0;
    while i < b.len() {
        let prev_word = i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == '_');
        if b[i] == 'x' && !prev_word && i + 1 < b.len() && b[i + 1].is_ascii_digit() {
            let mut j = i + 1;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            let k: usize = b[i + 1..j].iter().collect::<String>().parse().unwrap_or(0);
            if k >= 1
                && k <= names.len()
                && !(j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == '_'))
            {
                let n = &names[k - 1];
                let ident = n
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                    && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
                if ident {
                    out.push_str(n);
                } else {
                    out.push('`');
                    out.push_str(n);
                    out.push('`');
                }
                i = j;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

pub(crate) const FLOOR_NOT_ESTIMATED: &str = "the rounding floor was not estimated: no candidate of the search fitted to r2 0.999, so the data's precision was not measured and within_precision was judged at r2 1 - 1e-9";

pub(crate) const SCHEMA: &str = "keplearn.result.v3";

static VERBOSE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn set_verbose(v: bool) {
    VERBOSE.store(v, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn verbose() -> bool {
    VERBOSE.load(std::sync::atomic::Ordering::Relaxed)
}

pub(crate) struct ResultObj {
    pub(crate) expr: String,
    pub(crate) model: String,
    pub(crate) r2: Option<f64>,
    pub(crate) search_r2: Option<f64>,
    pub(crate) holdout_r2: Option<f64>,
    pub(crate) scale: f64,
    pub(crate) offset: f64,
    pub(crate) columns: Vec<usize>,
    pub(crate) uses: Vec<String>,
    pub(crate) structure: Option<String>,
    pub(crate) canonical: Option<String>,
    pub(crate) verdict: &'static str,
    pub(crate) symbolic: String,
    pub(crate) named: String,
    pub(crate) free_constants: Option<usize>,
    pub(crate) within_precision: Option<bool>,
    pub(crate) largest_miss: Option<(f64, f64)>,
}

fn jstr(s: &str) -> String {
    format!("\"{}\"", json_esc(s))
}

fn jopt_s(s: &Option<String>) -> String {
    s.as_ref().map_or("null".to_string(), |v| jstr(v))
}

fn j10(x: Option<f64>) -> String {
    match x {
        Some(v) if v.is_finite() => format!("{:.10}", v),
        _ => "null".to_string(),
    }
}

impl ResultObj {
    pub(crate) fn constant(
        v: f64,
        verdict: &'static str,
        free: Option<usize>,
        r2: Option<f64>,
    ) -> ResultObj {
        let c = tidy_number(v, 13);
        ResultObj {
            expr: c.clone(),
            model: c.clone(),
            r2,
            search_r2: None,
            holdout_r2: None,
            scale: 1.0,
            offset: 0.0,
            columns: Vec::new(),
            uses: Vec::new(),
            structure: Some("b".to_string()),
            canonical: Some(c.clone()),
            verdict,
            symbolic: c.clone(),
            named: c,
            free_constants: free,
            within_precision: (verdict == "law").then_some(true),
            largest_miss: (verdict == "law").then_some((0.0, v)),
        }
    }

    pub(crate) fn json(&self) -> String {
        format!(
            "{{\"expr\":{},\"model\":{},\"r2\":{},\"search_r2\":{},\"holdout_r2\":{},\"scale\":{},\"offset\":{},\"columns\":{:?},\"uses\":[{}],\"factor\":\"direct\",\"structure\":{},\"canonical\":{},\"verdict\":\"{}\",\"symbolic\":{},\"named\":{},\"free_constants\":{},\"within_precision\":{},\"largest_miss\":{},\"largest_miss_at\":{}}}",
            jstr(& self.expr), jstr(& self.model), j10(self.r2), j10(self.search_r2),
            jopt_f(self.holdout_r2), self.scale, self.offset, self.columns, self.uses
            .iter().map(| u | jstr(u)).collect::< Vec < _ >> ().join(","), jopt_s(& self
            .structure), jopt_s(& self.canonical), self.verdict, jstr(& self.symbolic),
            jstr(& self.named), jopt_u(self.free_constants), self.within_precision
            .map_or("null".to_string(), | b | b.to_string()), jopt_f(self.largest_miss
            .map(| m | m.0)), jopt_f(self.largest_miss.map(| m | m.1))
        )
    }
}

fn simpler_law(results: &[ResultObj]) -> Option<usize> {
    let top = results.first().filter(|r| r.verdict == "law")?;
    let free = |r: &ResultObj| r.free_constants.unwrap_or(usize::MAX);
    (1..results.len())
        .filter(|&i| {
            let r = &results[i];
            r.verdict == "law"
                && free(r) < free(top)
                && r.r2.is_some_and(|v| v >= crate::config::SIMPLER_MIN_R2)
        })
        .min_by(|&a, &b| {
            free(&results[a]).cmp(&free(&results[b])).then(
                results[b]
                    .r2
                    .unwrap_or(0.0)
                    .total_cmp(&results[a].r2.unwrap_or(0.0)),
            )
        })
}

pub(crate) struct Envelope<'a> {
    pub(crate) results: Vec<ResultObj>,
    pub(crate) verdict: &'static str,
    pub(crate) reason: Option<String>,
    pub(crate) stopped_by: &'static str,
    pub(crate) validation: &'static str,
    pub(crate) noise_floor: f64,
    pub(crate) target: &'a str,
    pub(crate) columns: &'a [String],
    pub(crate) rows: usize,
    pub(crate) warnings: &'a [String],
    pub(crate) time_ms: Option<u128>,
}

impl Envelope<'_> {
    pub(crate) fn json(&self) -> String {
        let mut s = format!(
            "{{\"schema\":\"{}\",\"results\":[{}],\"verdict\":\"{}\",\"reason\":{},\"simpler_law\":{},\"stopped_by\":\"{}\",\"validation\":\"{}\",\"noise_floor\":{:e},\"names\":{{\"target\":{},\"columns\":[{}]}},\"rows\":{},\"warnings\":[{}]",
            SCHEMA, self.results.iter().map(| r | r.json()).collect::< Vec < _ >> ()
            .join(","), self.verdict, jopt_s(& self.reason), simpler_law(& self.results)
            .map_or("null".to_string(), | i | i.to_string()), self.stopped_by, self
            .validation, self.noise_floor, jstr(self.target), self.columns.iter().map(| c
            | jstr(c)).collect::< Vec < _ >> ().join(","), self.rows, self.warnings
            .iter().map(| w | jstr(w)).collect::< Vec < _ >> ().join(",")
        );
        if let Some(t) = self.time_ms {
            s.push_str(&format!(",\"time_ms\":{}", t));
        }
        s.push('}');
        s
    }
}

pub(crate) fn used_columns(model: &str, names: &[String]) -> Vec<String> {
    let b = model.as_bytes();
    let mut used = vec![false; names.len()];
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'x' && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_')) {
            let mut k = i + 1;
            while k < b.len() && b[k].is_ascii_digit() {
                k += 1;
            }
            let ident_end = k < b.len() && (b[k].is_ascii_alphabetic() || b[k] == b'_');
            if k > i + 1 && !ident_end {
                if let Ok(j) = model[i + 1..k].parse::<usize>() {
                    if (1..=names.len()).contains(&j) {
                        used[j - 1] = true;
                    }
                }
            }
            i = k.max(i + 1);
        } else {
            i += 1;
        }
    }
    names
        .iter()
        .zip(used)
        .filter(|(_, u)| *u)
        .map(|(n, _)| n.clone())
        .collect()
}

pub(crate) fn tidy(e: &str, digits: usize) -> String {
    let b = e.as_bytes();
    let mut out = String::with_capacity(e.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_alphabetic() || c == b'_' || c == b'`' {
            let st = i;
            if c == b'`' {
                i += 1;
                while i < b.len() && b[i] != b'`' {
                    i += 1;
                }
                i = (i + 1).min(b.len());
            } else {
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
            }
            out.push_str(&e[st..i]);
        } else if c.is_ascii_digit()
            || (c == b'.' && b.get(i + 1).is_some_and(|d| d.is_ascii_digit()))
        {
            let st = i;
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
                i += 1;
            }
            if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
                let mut k = i + 1;
                if k < b.len() && (b[k] == b'+' || b[k] == b'-') {
                    k += 1;
                }
                if k < b.len() && b[k].is_ascii_digit() {
                    while k < b.len() && b[k].is_ascii_digit() {
                        k += 1;
                    }
                    i = k;
                }
            }
            match e[st..i].parse::<f64>() {
                Ok(v) => out.push_str(&tidy_number(v, digits)),
                Err(_) => out.push_str(&e[st..i]),
            }
        } else {
            let ch = e[i..].chars().next().unwrap_or(' ');
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

pub(crate) fn tidy_number(v: f64, digits: usize) -> String {
    if v == 0.0 || !v.is_finite() {
        return format!("{}", v);
    }
    let r: f64 = format!("{:.*e}", digits.saturating_sub(1), v)
        .parse()
        .unwrap_or(v);
    if (1e-4..1e15).contains(&r.abs()) {
        format!("{}", r)
    } else {
        format!("{:e}", r)
    }
}
