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
