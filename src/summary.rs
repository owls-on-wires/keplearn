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
            let within = top.get("within_precision").and_then(|v| v.as_bool()) == Some(true);
            let r2 = f(top, "r2").map_or("= undefined".to_string(), |r| r2_text(r, within));
            out.push_str(&format!(
                "{:<w$}  {} (r2 {}, {})\n",
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
            let miss = f(top, "largest_miss").filter(|m| *m > 0.0).map(|m| {
                let at = f(top, "largest_miss_at").map_or(String::new(), |y| {
                    format!(
                        " (where the target is {})",
                        crate::output::tidy_number(y, 6)
                    )
                });
                format!(
                    "largest miss {} of the target's value{}",
                    percent_text(100.0 * m),
                    at
                )
            });
            let fit = match (miss, within) {
                (Some(m), true) => {
                    format!("fit: {}, within the precision the cells are written to", m)
                }
                (Some(m), false) => {
                    format!(
                        "fit: {}, more than the precision the cells are written to", m
                    )
                }
                (None, true) => {
                    "fit: every row to the precision its cells are written to"
                        .to_string()
                }
                (None, false) => {
                    "fit: more than the precision the cells are written to (the target is 0 on some rows, so no miss is given as a fraction of it)"
                        .to_string()
                }
            };
            out.push_str(&fit);
            out.push('\n');
            let n_free = |r: &Json| f(r, "free_constants").map_or(usize::MAX, |v| v as usize);
            if let Some(r) = f(&j, "simpler_law").and_then(|i| results.get(i as usize)) {
                out.push_str(&format!(
                    "simpler law: {} = {} (r2 {}, {})\n",
                    target,
                    law(r),
                    r2_text(f(r, "r2").unwrap_or(0.0), false),
                    match n_free(r) {
                        0 => "no constant free".to_string(),
                        1 => "1 constant free".to_string(),
                        k => format!("{} constants free", k),
                    }
                ));
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
                    "best fit: {} = {} (r2 {}; not confirmed)\n",
                    target,
                    law(top),
                    r2_text(f(top, "r2").unwrap_or(-1.0), false)
                ));
            }
            if !s(&j, "reason").is_empty() {
                out.push_str(&format!("reason: {}\n", s(&j, "reason")));
            }
        }
    }
    let not_estimated = j.get("warnings").and_then(|w| w.as_arr()).is_some_and(|w| {
        w.iter()
            .any(|x| x.as_str() == Some(crate::output::FLOOR_NOT_ESTIMATED))
    });
    let precision = if floor > 0.0 {
        format!("rounding floor {:.1e} of the target's variance", floor)
    } else if not_estimated {
        "rounding floor not estimated (no search candidate fitted to r2 0.999)".to_string()
    } else {
        "full precision".to_string()
    };
    if verdict == "no law found" {
        out.push_str(&format!(
            "data: {}, {}\n",
            plural(rows, "row"),
            plural(cols, "column")
        ));
    } else {
        out.push_str(&format!(
            "data: {}, {}; {}\n",
            plural(rows, "row"),
            plural(cols, "column"),
            precision
        ));
    }
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
            if let Some(t) = x
                .as_str()
                .filter(|t| *t != crate::output::FLOOR_NOT_ESTIMATED)
            {
                out.push_str(&format!("note: {}\n", t));
            }
        }
    }
    out
}

fn r2_text(r: f64, exact: bool) -> String {
    if !exact && r >= 1.0 {
        return "> 0.9999999999".to_string();
    }
    let mut d = 4;
    while d < 12 && r < 1.0 && format!("{:.*}", d, r).starts_with("1.") {
        d += 2;
    }
    format!("= {:.*}", d, r)
}

fn percent_text(p: f64) -> String {
    let d = if p >= 1.0 {
        2
    } else {
        (2 - p.log10().floor() as i32).clamp(2, 12) as usize
    };
    format!("{:.*}%", d, p)
}

fn plural(n: usize, w: &str) -> String {
    if n == 1 {
        format!("1 {}", w)
    } else {
        format!("{} {}s", n, w)
    }
}
