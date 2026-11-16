use crate::forms::json::Json;

fn s<'a>(j: &'a Json, k: &str) -> &'a str {
    j.get(k).and_then(|v| v.as_str()).unwrap_or("")
}

fn f(j: &Json, k: &str) -> Option<f64> {
    j.get(k).and_then(|v| v.as_f64()).filter(|v| v.is_finite())
}

pub fn render(json: &str) -> String {
    let Ok(j) = crate::forms::json::parse(json) else {
        return json.to_string();
    };
    let names = j.get("names");
    let target = names.map(|n| s(n, "target")).unwrap_or("y");
    let cols = names
        .and_then(|n| n.get("columns"))
        .and_then(|c| c.as_arr())
        .map_or(0, |c| c.len());
    let rows = f(&j, "rows").unwrap_or(0.0) as usize;
    let floor = f(&j, "noise_floor").unwrap_or(0.0);
    let verdict = s(&j, "verdict");
    let results: Vec<&Json> = j
        .get("results")
        .and_then(|r| r.as_arr())
        .map(|r| r.iter().collect())
        .unwrap_or_default();
    let mut out = String::new();
    let law = |r: &Json| {
        let named = s(r, "named");
        crate::output::tidy(
            if named.is_empty() {
                s(r, "model")
            } else {
                named
            },
            12,
        )
    };
    let width = 48;
    match results.first() {
        Some(top) if verdict != "no law found" => {
            let line = format!("{} = {}", target, law(top));
            let r2 = f(top, "r2").map_or("undefined".to_string(), |r| format!("{:.4}", r));
            out.push_str(&format!(
                "{:<w$}  {} (r2 = {}, {})\n",
                line,
                verdict,
                r2,
                plural(rows, "row"),
                w = width
            ));
            let free = f(top, "free_constants").map(|v| v as usize);
            let named_consts =
                !s(top, "symbolic").is_empty() && s(top, "symbolic") != s(top, "model");
            let mut c = match free {
                Some(0) => "constants: none free".to_string(),
                Some(n) => format!("constants: {} free", n),
                None => "constants: not counted".to_string(),
            };
            if named_consts {
                c.push_str("; snapped constants written by name");
            }
            out.push_str(&c);
            out.push('\n');
            if verdict != "exact" && !s(&j, "reason").is_empty() {
                out.push_str(&format!("why not exact: {}\n", s(&j, "reason")));
            }
        }
        _ => {
            out.push_str(&format!(
                "{}: no law found ({}, {})\n",
                target,
                plural(rows, "row"),
                plural(cols, "column")
            ));
            if let Some(top) = results.first().filter(|r| !s(r, "named").is_empty()) {
                out.push_str(&format!(
                    "best fit: {} = {} (r2 = {:.4}; not confirmed)\n",
                    target,
                    law(top),
                    f(top, "r2").unwrap_or(-1.0)
                ));
            }
            if !s(&j, "reason").is_empty() {
                out.push_str(&format!("reason: {}\n", s(&j, "reason")));
            }
        }
    }
    let precision = if floor > 0.0 {
        format!("rounding floor {:.1e} of the target's variance", floor)
    } else {
        "full precision".to_string()
    };
    out.push_str(&format!(
        "data: {}, {}; {}\n",
        plural(rows, "row"),
        plural(cols, "column"),
        precision
    ));
    if let Some(second) = results.get(1).filter(|r| !s(r, "named").is_empty()) {
        out.push_str(&format!(
            "next best: {} = {}    {}\n",
            target,
            law(second),
            s(second, "verdict")
        ));
    }
    if let Some(w) = j.get("warnings").and_then(|w| w.as_arr()) {
        for x in w {
            if let Some(t) = x.as_str() {
                out.push_str(&format!("note: {}\n", t));
            }
        }
    }
    out
}

fn plural(n: usize, w: &str) -> String {
    if n == 1 {
        format!("1 {}", w)
    } else {
        format!("{} {}s", n, w)
    }
}
