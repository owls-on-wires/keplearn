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
