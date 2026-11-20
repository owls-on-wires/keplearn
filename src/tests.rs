use super::*;

fn eval_str(s: &str, vars: &[&[f64]], n: usize) -> Vec<f64> {
    let e = Parser::parse(s).unwrap_or_else(|err| panic!("parse '{}': {}", s, err));
    let c = compile(&e);
    let mut ev = Evaluator::new(c.max_stack.max(2), n);
    ev.eval(&c.ops, vars, n).to_vec()
}

#[test]
#[allow(clippy::approx_constant)]
fn snap_value_targets_and_refusals() {
    assert_eq!(snap_value(0.999063), Some(1.0));
    assert_eq!(snap_value(-2.0011), Some(-2.0));
    assert_eq!(snap_value(0.333333333333), Some(1.0 / 3.0));
    assert_eq!(snap_value(0.4995), Some(0.5));
    assert_eq!(snap_value(2.0), None);
    assert_eq!(snap_value(0.5), None);
    assert_eq!(snap_value(0.0), None);
    assert_eq!(snap_value(0.437), None);
    let s = snap_value(0.0795).unwrap();
    assert!(
        (s - 1.0 / (4.0 * std::f64::consts::PI)).abs() < 1e-9,
        "got {}",
        s
    );
    assert!((snap_value(3.1416).unwrap() - std::f64::consts::PI).abs() < 1e-9);
}

#[test]
#[allow(clippy::type_complexity)]
fn parser_round_trip() {
    let xs1 = [0.5, 1.0, 1.7, 2.3, 0.9];
    let xs2 = [0.3, 0.9, 1.1, 2.0, 1.4];
    let cases: Vec<(&str, fn(f64, f64) -> f64)> = vec![
        ("x1*sin(x2)+3", |a, b| a * b.sin() + 3.0),
        ("sqrt(x1)/x2", |a, b| a.sqrt() / b),
        ("((x1-x2))**2", |a, b| (a - b) * (a - b)),
        ("exp(-x1**2/2)*cos(x2)", |a, b| {
            (-(a * a) / 2.0).exp() * b.cos()
        }),
        ("(1.500000000000*(x1/x2)+-0.250000000000)", |a, b| {
            1.5 * (a / b) - 0.25
        }),
        ("(sin(x1)*exp((x2)))", |a, b| a.sin() * b.exp()),
    ];
    for (s, f) in cases {
        let out = eval_str(s, &[&xs1, &xs2], 5);
        for i in 0..5 {
            assert!(
                (out[i] - f(xs1[i], xs2[i])).abs() < 1e-10,
                "mismatch for '{}' at row {}: {} vs {}",
                s,
                i,
                out[i],
                f(xs1[i], xs2[i])
            );
        }
    }
}

#[test]
fn probe_basis_scores_monomial_residuals() {
    let n = 50usize;
    let pr = |i: usize, k: f64| -> f64 {
        let h = (i as f64 + 1.0) * k;
        let s = h.sin() * 43758.5453;
        s - s.floor()
    };
    let m: Vec<f64> = (0..n).map(|i| 0.5 + 2.0 * pr(i, 12.9898)).collect();
    let v: Vec<f64> = (0..n).map(|i| 0.1 + 0.8 * pr(i, 78.233)).collect();
    let c: Vec<f64> = (0..n).map(|i| 1.0 + 3.0 * pr(i, 39.425)).collect();
    let resid: Vec<f64> = (0..n).map(|i| m[i] * c[i] * c[i]).collect();
    let inputs: Vec<&[f64]> = vec![&m, &v, &c];
    let basis = build_probe_basis(&inputs, n);
    let single = quick_probe(&resid, &inputs, n);
    let ext = quick_probe_ext(&resid, &inputs, &basis, n);
    assert!(
        single < 0.9,
        "single-input probe unexpectedly high: {}",
        single
    );
    assert!(
        ext > 1.0 - 1e-9,
        "extended probe must catch m*c^2 exactly: {}",
        ext
    );
    let z: Vec<f64> = (0..n)
        .map(|i| if i == 3 { 0.0 } else { 1.0 + i as f64 })
        .collect();
    let inputs2: Vec<&[f64]> = vec![&z];
    let basis2 = build_probe_basis(&inputs2, n);
    let q = quick_probe_ext(&resid, &inputs2, &basis2, n);
    assert!(q.is_finite() && (0.0..=1.0).contains(&q));
}

#[test]
fn affine_fit_known_data() {
    let x: Vec<f64> = (0..50).map(|i| i as f64 * 0.1).collect();
    let y: Vec<f64> = x.iter().map(|v| 3.0 * v + 2.0).collect();
    let (r2, a, b) = r2_score(&y, &x);
    assert!((r2 - 1.0).abs() < 1e-12, "r2 = {}", r2);
    assert!((a - 3.0).abs() < 1e-9, "scale = {}", a);
    assert!((b - 2.0).abs() < 1e-9, "offset = {}", b);
}

#[test]
fn masked_division_excludes_rows() {
    let numer = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let denom = [1.0, 0.0, 2.0, 1e-12, 4.0, -5.0];
    let mut buf = [0.0f64; 6];
    let mut scratch = Vec::new();
    let vc = masked_divide(&numer, &denom, &mut buf, &mut scratch, 6);
    assert_eq!(vc, 4);
    assert!(buf[1].is_nan(), "zero divisor must be masked");
    assert!(buf[3].is_nan(), "near-zero divisor must be masked");
    assert!((buf[0] - 1.0).abs() < 1e-12);
    assert!((buf[2] - 1.5).abs() < 1e-12);
    assert!((buf[5] + 1.2).abs() < 1e-12);
    let clean = [1.0, 9.0, 1.5, 9.0, 1.25, -1.2];
    let (r2, _, _) = r2_score(&buf, &clean);
    assert!(
        (r2 - 1.0).abs() < 1e-9,
        "masked rows leaked into fit: r2 = {}",
        r2
    );
}

#[test]
fn mdl_prefers_simpler_at_equal_holdout() {
    let hn = 100.0;
    let dl_simple = mdl_dl(hn, 0.95, 5.0);
    let dl_complex = mdl_dl(hn, 0.95, 12.0);
    assert!(dl_simple < dl_complex, "{} vs {}", dl_simple, dl_complex);
    let dl_better = mdl_dl(hn, 0.9999, 12.0);
    assert!(dl_better < dl_simple, "{} vs {}", dl_better, dl_simple);
}

#[test]
fn chain_reconstruction_matches_target_algebra() {
    let n = 60usize;
    let x1: Vec<f64> = (0..n).map(|i| 0.5 + (i as f64) * 0.015).collect();
    let x2: Vec<f64> = (0..n).map(|i| 0.1 + (i as f64) * 0.02).collect();
    let y: Vec<f64> = (0..n).map(|i| x1[i].sin() * x2[i].exp()).collect();
    let chain = vec![
        Red::Div {
            desc: "sin(c0)".into(),
            gexpr: "sin(x1)".into(),
            cost: 2,
        },
        Red::Shell {
            name: "log".to_string(),
        },
    ];
    let model = invert_chain(&chain, chain.len(), "(x2)".to_string());
    let out = eval_str(&model, &[&x1, &x2], n);
    for k in 0..n {
        assert!(
            (out[k] - y[k]).abs() < 1e-9 * (1.0 + y[k].abs()),
            "model '{}' row {}: {} vs {}",
            model,
            k,
            out[k],
            y[k]
        );
    }
}

#[test]
fn lorentz_gexpr_matches_numeric_gamma() {
    let a = [0.3, 0.5, 0.9, 0.1];
    let b = [1.0, 2.0, 1.0, 0.4];
    let g = "(1/sqrt(1-((x1/x2)**2)))";
    let out = eval_str(g, &[&a, &b], 4);
    for i in 0..4 {
        let r = a[i] / b[i];
        let v = 1.0 / (1.0 - r * r).sqrt();
        assert!((out[i] - v).abs() < 1e-12, "row {}: {} vs {}", i, out[i], v);
    }
}

#[test]
fn trig_shells_invert_on_principal_branch() {
    let n = 40usize;
    for (name, lo, hi) in [("sin", -1.4f64, 1.4f64), ("cos", 0.1, 3.0)] {
        let y: Vec<f64> = (0..n)
            .map(|i| lo + (hi - lo) * i as f64 / (n - 1) as f64)
            .collect();
        let t: Vec<f64> = y.iter().map(|&v| shell_apply(name, v)).collect();
        assert!(
            t.iter().all(|v| v.is_finite()),
            "{} shell masked in-branch rows",
            name
        );
        let chain = vec![Red::Shell {
            name: if name == "sin" { "sin" } else { "cos" }.to_string(),
        }];
        let model = invert_chain(&chain, 1, "(x1)".to_string());
        let out = eval_str(&model, &[&t], n);
        for k in 0..n {
            assert!(
                (out[k] - y[k]).abs() < 1e-10,
                "{} row {}: {} vs {}",
                name,
                k,
                out[k],
                y[k]
            );
        }
        let bad = if name == "sin" { 2.0 } else { -0.5 };
        assert!(!shell_apply(name, bad).is_finite());
    }
}

#[test]
fn joint_refit_recovers_peeled_coefficients() {
    let n = 80usize;
    let x: Vec<f64> = (0..n).map(|i| -2.0 + (i as f64) * 0.05).collect();
    let y: Vec<f64> = x
        .iter()
        .map(|v| 3.0 * v * v + 2.0 * v.sin() + 1.0)
        .collect();
    let basis = vec![
        x.iter().map(|v| v * v).collect::<Vec<f64>>(),
        x.iter().map(|v| v.sin()).collect::<Vec<f64>>(),
    ];
    let sol = solve_ls(&basis, &y).expect("solvable");
    assert!((sol[0] - 3.0).abs() < 1e-8, "c0 = {}", sol[0]);
    assert!((sol[1] - 2.0).abs() < 1e-8, "c1 = {}", sol[1]);
    assert!((sol[2] - 1.0).abs() < 1e-8, "const = {}", sol[2]);
}

#[test]
fn build_candidate_reconstructs_chained_model() {
    let n = 64usize;
    let x1: Vec<f64> = (0..n).map(|i| 0.5 + (i as f64) * 0.05).collect();
    let x2: Vec<f64> = (0..n).map(|i| 0.2 + (i as f64) * 0.03).collect();
    let y: Vec<f64> = (0..n).map(|i| x2[i] * x2[i] / x1[i]).collect();
    let chain = vec![Red::Div {
        desc: "sq(c1)".into(),
        gexpr: "((x2)**2)".into(),
        cost: 2,
    }];
    let hit = Hit {
        expr: "(1/x1)".to_string(),
        r2: 1.0,
        scale: 1.0,
        offset: 0.0,
        columns: vec![0],
        gate_r2: 1.0,
    };
    let cand = build_candidate(&chain, "y/sq(c1)", &hit, &[], 2).expect("candidate");
    let out = eval_str(&cand.model, &[&x1, &x2], n);
    for k in 0..n {
        assert!(
            (out[k] - y[k]).abs() < 1e-9 * (1.0 + y[k].abs()),
            "model '{}' row {}: {} vs {}",
            cand.model,
            k,
            out[k],
            y[k]
        );
    }
    assert!(cand.nodes > 0);
    assert!(!cand.root_raw);
}

#[test]
fn plain_r2_penalizes_what_affine_r2_forgives() {
    let x: Vec<f64> = (0..50).map(|i| i as f64 * 0.1).collect();
    let y: Vec<f64> = x.iter().map(|v| 3.0 * v + 2.0).collect();
    assert!((r2_score(&y, &x).0 - 1.0).abs() < 1e-12);
    assert!(plain_r2(&y, &x) < 0.0, "plain_r2 = {}", plain_r2(&y, &x));
    assert!((plain_r2(&y, &y) - 1.0).abs() < 1e-12);
    let mean = y.iter().sum::<f64>() / y.len() as f64;
    let m: Vec<f64> = vec![mean; y.len()];
    assert!(plain_r2(&y, &m).abs() < 1e-12);
}

#[test]
fn full_model_r2_scores_emitted_string() {
    let n = 40usize;
    let pr = |i: usize, k: f64| -> f64 {
        let h = (i as f64 + 1.0) * k;
        let s = h.sin() * 43758.5453;
        s - s.floor()
    };
    let x1: Vec<f64> = (0..n).map(|i| 0.5 + 3.0 * pr(i, 12.9898)).collect();
    let x2: Vec<f64> = (0..n).map(|i| 1.0 + 2.0 * pr(i, 78.233)).collect();
    let features = vec![x1.clone(), x2.clone()];
    let target: Vec<f64> = (0..n).map(|i| 2.0 * x1[i] + x2[i]).collect();
    let mut ev = Evaluator::new(4, n);
    let r = full_model_r2("((x1*2.000000000000)+x2)", &features, &target, &mut ev);
    assert!((r - 1.0).abs() < 1e-9, "exact model: {}", r);
    let r = full_model_r2("(x1*2.000000000000)", &features, &target, &mut ev);
    assert!(r < 0.999, "precursor scored {}", r);
    let r = full_model_r2("sqrt((x1-3.000000000000))", &features, &target, &mut ev);
    assert!((r + 1.0).abs() < 1e-12, "masked-row model scored {}", r);
}

#[test]
fn split_row_handles_quotes_and_delims() {
    assert_eq!(split_row("a\tb\tc", '\t'), vec!["a", "b", "c"]);
    assert_eq!(split_row("1.5,2,3", ','), vec!["1.5", "2", "3"]);
    assert_eq!(
        split_row("\"bond, length\",2,\"say \"\"hi\"\"\"", ','),
        vec!["bond, length", "2", "say \"hi\""]
    );
    assert_eq!(split_row("a,,c", ','), vec!["a", "", "c"]);
    assert_eq!(split_row(",", ','), vec!["", ""]);
}

#[test]
fn parser_rejects_bad_variable_indices() {
    assert!(Parser::parse("x0").is_err());
    assert!(Parser::parse("x99999999999999999999999").is_err());
    assert!(Parser::parse("x1").is_ok());
}

#[test]
fn json_pretty_formats_without_touching_tokens() {
    let compact = r#"{"results":[{"expr":"(x1*x2)","r2":1.0,"columns":[0,1],"note":"a\"b, {c}"}],"n":0,"empty":[]}"#;
    let pretty = json_pretty(compact);
    let mut squashed = String::new();
    let (mut in_str, mut esc) = (false, false);
    for c in pretty.chars() {
        if in_str {
            squashed.push(c);
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
        } else if !c.is_whitespace() {
            if c == '"' {
                in_str = true;
            }
            squashed.push(c);
        }
    }
    assert_eq!(squashed, compact);
    assert!(pretty.contains("\n  \"results\": ["));
    assert!(pretty.contains(r#""a\"b, {c}""#));
    assert!(pretty.contains("\"empty\": []"));
}

fn test_cfg() -> Config {
    Config {
        top_k: 5,
        sample: 500,
        target_col: "y".to_string(),
        quick_n: 50,
        timeout_ms: 60_000,
        max_evals: 0,
        seed: 0,
        max_depth: None,
        trace_path: String::new(),
        dump_candidates_path: String::new(),
        out_path: String::new(),
        pretty: false,
        format: "json".to_string(),
        input_warnings: Vec::new(),
        verbose: false,
        timing: false,
        max_memory_mb: 0,
    }
}

fn lcg(s: &mut u64) -> f64 {
    *s = s
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*s >> 11) as f64 / (1u64 << 53) as f64
}

fn csv_table(
    n: usize,
    ranges: &[(f64, f64)],
    mut f: impl FnMut(&[f64]) -> f64,
    seed: u64,
) -> String {
    let mut s = seed;
    let mut out: String = (1..=ranges.len())
        .map(|i| format!("x{},", i))
        .collect::<String>()
        + "y\n";
    for _ in 0..n {
        let x: Vec<f64> = ranges
            .iter()
            .map(|&(lo, hi)| lo + (hi - lo) * lcg(&mut s))
            .collect();
        for v in &x {
            out.push_str(&format!("{:.12e},", v));
        }
        out.push_str(&format!("{:.12e}\n", f(&x)));
    }
    out
}

fn run_json(table: &str) -> crate::forms::json::Json {
    let out = run_text(table, test_cfg()).expect("run_text");
    crate::forms::json::parse(&out).expect("json")
}

fn top_structure(table: &str) -> String {
    let j = run_json(table);
    let r = j
        .get("results")
        .and_then(|r| r.as_arr())
        .and_then(|r| r.first())
        .cloned();
    r.and_then(|r| {
        r.get("structure")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string())
    })
    .unwrap_or_default()
}

#[test]
fn ingest_drops_columns_without_numbers() {
    let t = "body,a,p\nMercury,0.387,0.241\nVenus,0.723,0.615\nEarth,1.0,1.0\n";
    let ing = ingest(t, "p").expect("ingest");
    assert_eq!(ing.n_cols, 1);
    assert_eq!(ing.target.len(), 3);
    assert!(
        ing.warnings.iter().any(|w| w.contains("'body'")),
        "{:?}",
        ing.warnings
    );
}

#[test]
fn end_to_end_power_law_40_rows() {
    let t = csv_table(40, &[(0.5, 5.0)], |x| x[0].powf(1.5), 12345);
    assert_eq!(top_structure(&t), "c1*x1**(3/2) + b");
}

fn precision_verdict(r: &crate::forms::json::Json) -> String {
    match (
        r.get("verdict").and_then(|v| v.as_str()),
        r.get("within_precision").and_then(|v| v.as_bool()),
    ) {
        (Some("law"), Some(true)) => "exact".to_string(),
        (Some("law"), _) => "approximate".to_string(),
        (Some(v), _) => v.to_string(),
        _ => String::new(),
    }
}

fn run_verdict(j: &crate::forms::json::Json) -> String {
    match j.get("verdict").and_then(|v| v.as_str()) {
        Some("law") => j
            .get("results")
            .and_then(|r| r.as_arr())
            .and_then(|r| r.first())
            .map(precision_verdict)
            .unwrap_or_default(),
        Some(v) => v.to_string(),
        None => String::new(),
    }
}

fn top_and_verdict(table: &str) -> (String, String, String) {
    let j = run_json(table);
    let r = j
        .get("results")
        .and_then(|r| r.as_arr())
        .and_then(|r| r.first())
        .cloned();
    let get = |k: &str| {
        r.as_ref()
            .and_then(|r| r.get(k).and_then(|s| s.as_str()).map(|s| s.to_string()))
            .unwrap_or_default()
    };
    (get("model"), get("structure"), run_verdict(&j))
}

fn same_function(model: &str, ranges: &[(f64, f64)], f: impl Fn(&[f64]) -> f64) -> bool {
    let mut s = 99u64;
    let n = 50;
    let cols: Vec<Vec<f64>> = ranges
        .iter()
        .map(|&(lo, hi)| (0..n).map(|_| lo + (hi - lo) * lcg(&mut s)).collect())
        .collect();
    let vars: Vec<&[f64]> = cols.iter().map(|c| c.as_slice()).collect();
    let got = eval_str(model, &vars, n);
    (0..n).all(|r| {
        let x: Vec<f64> = cols.iter().map(|c| c[r]).collect();
        let want = f(&x);
        (got[r] - want).abs() <= 1e-6 * want.abs().max(1e-12)
    })
}

#[test]
fn small_power_law_6_8_12_rows() {
    for n in [6, 8, 12] {
        let t = csv_table(n, &[(0.5, 5.0)], |x| x[0].powf(1.5), 4242 + n as u64);
        let (_, st, v) = top_and_verdict(&t);
        assert_eq!(
            (st.as_str(), v.as_str()),
            ("c1*x1**(3/2) + b", "exact"),
            "{} rows",
            n
        );
    }
}

#[test]
fn small_two_column_monomial_6_8_12_rows() {
    for n in [6, 8, 12] {
        let t = csv_table(
            n,
            &[(0.5, 5.0), (0.5, 5.0)],
            |x| x[0] / (x[1] * x[1]),
            5151 + n as u64,
        );
        let (_, st, v) = top_and_verdict(&t);
        assert_eq!(
            (st.as_str(), v.as_str()),
            ("c1*x1/x2**2 + b", "exact"),
            "{} rows",
            n
        );
    }
}

#[test]
fn small_shell_form_8_12_rows() {
    let f = |x: &[f64]| (x[0] * x[1] + 1.0).sqrt();
    for n in [8, 12] {
        let t = csv_table(n, &[(0.5, 5.0), (0.5, 5.0)], f, 6161 + n as u64);
        let (m, _, v) = top_and_verdict(&t);
        assert!(
            v == "exact" && same_function(&m, &[(0.5, 5.0), (0.5, 5.0)], f),
            "{} rows: {} ({})",
            n,
            m,
            v
        );
    }
}

#[test]
fn unrelated_target_8_rows_abstains() {
    let mut s = 31337u64;
    let t = csv_table(8, &[(0.5, 5.0)], |_| 1.0 + 4.0 * lcg(&mut s), 7171);
    let (_, _, v) = top_and_verdict(&t);
    assert_eq!(v, "no law found");
}

fn gauss(s: &mut u64) -> f64 {
    let u1 = lcg(s).max(1e-300);
    let u2 = lcg(s);
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
}

fn csv_rounded(
    n: usize,
    ranges: &[(f64, f64)],
    f: impl Fn(&[f64]) -> f64,
    seed: u64,
    dec: usize,
) -> String {
    let mut s = seed;
    let mut out: String = (1..=ranges.len())
        .map(|i| format!("x{},", i))
        .collect::<String>()
        + "y\n";
    for _ in 0..n {
        let x: Vec<f64> = ranges
            .iter()
            .map(|&(lo, hi)| lo + (hi - lo) * lcg(&mut s))
            .collect();
        let xr: Vec<f64> = x
            .iter()
            .map(|v| format!("{:.*}", dec, v).parse().unwrap())
            .collect();
        for v in &xr {
            out.push_str(&format!("{:.*},", dec, v));
        }
        out.push_str(&format!("{:.*}\n", dec, f(&xr)));
    }
    out
}

fn num(j: &crate::forms::json::Json, k: &str) -> f64 {
    j.get(k).and_then(|v| v.as_f64()).unwrap_or(f64::NAN)
}

#[test]
fn rounded_power_law_40_rows_exact_with_floor() {
    let t = csv_rounded(40, &[(0.5, 5.0)], |x| x[0].powf(1.5), 8181, 3);
    let j = run_json(&t);
    let top = j
        .get("results")
        .and_then(|r| r.as_arr())
        .and_then(|r| r.first())
        .cloned()
        .unwrap();
    assert_eq!(
        top.get("structure").and_then(|s| s.as_str()),
        Some("c1*x1**(3/2) + b")
    );
    assert_eq!(run_verdict(&j), "exact");
    let rows: Vec<(f64, f64)> = t
        .lines()
        .skip(1)
        .map(|l| {
            let v: Vec<f64> = l.split(',').map(|c| c.parse().unwrap()).collect();
            (v[0], v[1])
        })
        .collect();
    let q2 = 1e-6 / 12.0;
    let known = rows
        .iter()
        .map(|(x, _)| q2 + (1.5 * x.sqrt()).powi(2) * q2)
        .sum::<f64>()
        / rows.len() as f64;
    let ym = rows.iter().map(|r| r.1).sum::<f64>() / rows.len() as f64;
    let vy = rows.iter().map(|r| (r.1 - ym).powi(2)).sum::<f64>() / rows.len() as f64;
    let ratio = num(&j, "noise_floor") / (known / vy);
    assert!((0.5..=2.0).contains(&ratio), "floor ratio {}", ratio);
}

#[test]
fn noisy_power_law_40_rows_not_exact() {
    let mut s = 9191u64;
    let t = csv_table(
        40,
        &[(0.5, 5.0)],
        |x| x[0].powf(1.5) * (1.0 + 0.001 * gauss(&mut s)),
        9292,
    );
    let (_, st, v) = top_and_verdict(&t);
    assert_eq!(st, "c1*x1**(3/2) + b");
    assert_eq!(v, "approximate");
}

#[test]
fn rounded_and_noisy_monomial_and_shell_40_rows() {
    let r2 = [(0.5, 5.0), (0.5, 5.0)];
    let mono = |x: &[f64]| x[0] / (x[1] * x[1]);
    let shell = |x: &[f64]| (x[0] * x[1] + 1.0).sqrt();
    let forms: [(&dyn Fn(&[f64]) -> f64, u64); 2] = [(&mono, 1212), (&shell, 1313)];
    for (f, seed) in forms {
        let (m, _, v) = top_and_verdict(&csv_rounded(40, &r2, f, seed, 3));
        assert!(
            v == "exact" && same_function(&m, &r2, f),
            "rounded: {} ({})",
            m,
            v
        );
        let mut s = seed + 7;
        let (m, _, v) = top_and_verdict(&csv_table(
            40,
            &r2,
            |x| f(x) * (1.0 + 0.001 * gauss(&mut s)),
            seed + 1,
        ));
        assert!(v == "approximate", "noisy: {} ({})", m, v);
    }
}

#[test]
fn noisy_pendulum_200_rows_snaps_two_pi() {
    let mut s = 4343u64;
    let gs = [9.81, 3.71, 1.62, 24.79];
    let mut out = String::from("length,g,period\n");
    let mut r = 5454u64;
    for i in 0..200 {
        let l = 0.2 + 1.8 * lcg(&mut r);
        let g = gs[i % 4];
        let p = 2.0 * std::f64::consts::PI * (l / g).sqrt() * (1.0 + 0.01 * gauss(&mut s));
        out.push_str(&format!("{:.4},{},{:.5}\n", l, g, p));
    }
    let mut cfg = test_cfg();
    cfg.target_col = "period".to_string();
    let j = crate::forms::json::parse(&run_text(&out, cfg).unwrap()).unwrap();
    let top = j
        .get("results")
        .and_then(|r| r.as_arr())
        .and_then(|r| r.first())
        .cloned()
        .unwrap();
    let named = top
        .get("named")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();
    assert!(
        named.contains("2*pi") && named.contains("length") && named.contains("g"),
        "{}",
        named
    );
    assert_ne!(run_verdict(&j), "exact");
}

#[test]
fn ranking_puts_the_best_scoring_law_first() {
    let t = csv_rounded(
        40,
        &[(0.5, 5.0), (0.5, 5.0)],
        |x| x[0] / (x[1] * x[1]),
        1212,
        3,
    );
    let j = run_json(&t);
    let rs: Vec<f64> = j
        .get("results")
        .and_then(|r| r.as_arr())
        .unwrap()
        .iter()
        .map(|r| num(r, "r2"))
        .collect();
    assert!(rs.iter().skip(1).all(|r| rs[0] >= *r - 1e-9), "{:?}", rs);
}

#[test]
fn summary_of_the_rounded_bodies_table() {
    let input = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/field_bodies26_r3.csv"
    ))
    .unwrap();
    let want = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/field_bodies26_r3.summary.txt"
    ))
    .unwrap();
    let mut cfg = test_cfg();
    cfg.target_col = "period_years".to_string();
    let got = crate::summary::render(&run_text(&input, cfg).unwrap());
    assert_eq!(got, want);
}

#[test]
fn constant_target_is_the_law_and_few_rows_are_named() {
    let t: String =
        "x,y\n".to_string() + &(0..50).map(|i| format!("{},4.2\n", i)).collect::<String>();
    let j = run_json(&t);
    assert_eq!(run_verdict(&j), "exact");
    let top = j
        .get("results")
        .and_then(|r| r.as_arr())
        .and_then(|r| r.first())
        .cloned()
        .unwrap();
    assert_eq!(top.get("model").and_then(|v| v.as_str()), Some("4.2"));
    let t = "x,y\n1,2\n2,4\n3,6\n";
    let j = run_json(t);
    assert_eq!(
        j.get("verdict").and_then(|v| v.as_str()),
        Some("no law found")
    );
    assert!(j
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .contains("3 data rows"));
}

#[test]
fn input_that_is_not_a_table_is_named() {
    for (t, want) in [
        ("{\"a\": 1}\n", "JSON"),
        ("a\x00\x01\x02\x03\x04,b\n\x05\x06,\x07\n", "not text"),
        ("y\n1\n2\n", "single column"),
        ("1,2\n3,4\n", "holds numbers"),
    ] {
        let e = run_text(t, test_cfg()).unwrap_err();
        assert!(e.contains(want), "{}: {}", want, e);
    }
}

#[test]
fn planets_to_two_digits_give_the_power_law() {
    let t = "a,y\n0.39,0.24\n0.72,0.62\n1.0,1.0\n1.5,1.9\n5.2,12.0\n9.5,29.0\n19.0,84.0\n30.0,160.0\n39.0,250.0\n";
    let (m, _, v) = top_and_verdict(t);
    assert_eq!((m.as_str(), v.as_str()), ("x1**(3/2)", "exact"));
}

#[test]
fn every_result_has_every_field_and_the_bytes_repeat() {
    let t = csv_table(
        60,
        &[(0.5, 5.0), (0.5, 5.0)],
        |x| x[0] * x[1].sin() + 0.1 * x[0],
        2468,
    );
    let a = run_text(&t, test_cfg()).unwrap();
    let b = run_text(&t, test_cfg()).unwrap();
    assert_eq!(a, b);
    let j = crate::forms::json::parse(&a).unwrap();
    assert_eq!(
        j.get("schema").and_then(|v| v.as_str()),
        Some("keplearn.result.v3")
    );
    assert!(j.get("time_ms").is_none());
    let keys = [
        "expr",
        "model",
        "r2",
        "search_r2",
        "holdout_r2",
        "scale",
        "offset",
        "columns",
        "uses",
        "factor",
        "structure",
        "canonical",
        "verdict",
        "symbolic",
        "named",
        "free_constants",
        "within_precision",
        "largest_miss",
        "largest_miss_at",
    ];
    assert!(j.get("simpler_law").is_some());
    for r in j.get("results").and_then(|r| r.as_arr()).unwrap() {
        for k in keys {
            assert!(r.get(k).is_some(), "missing {}", k);
        }
    }
}

#[test]
fn ill_conditioned_forms_are_never_exact() {
    let x: Vec<f64> = (0..40).map(|i| 0.2 + 0.05 * i as f64).collect();
    let f = vec![x.clone()];
    assert!(!crate::guard::well_conditioned(
        "8.025e42*1/((1/sqrt(x1) + (-2e21)*1/x1**(1/4))**2)",
        &f,
        40
    ));
    assert!(!crate::guard::well_conditioned(
        "65.7*x1*arccos(-0.00008*1/x1**2 + 1)",
        &f,
        40
    ));
    assert!(crate::guard::well_conditioned("2.006*sqrt(x1)", &f, 40));
    assert!(
        crate::guard::structural_constant(1.5)
            && crate::guard::structural_constant(2.0 * std::f64::consts::PI)
    );
    assert!(!crate::guard::structural_constant(115.0) && !crate::guard::structural_constant(0.03));
}

#[test]
fn tables_below_80_rows_use_every_row() {
    for (n, want) in [(60, "leave-one-out"), (100, "holdout")] {
        let t = csv_table(
            n,
            &[(0.5, 5.0), (0.5, 5.0)],
            |x| x[0] * x[1].sqrt(),
            1357 + n as u64,
        );
        let j = run_json(&t);
        assert_eq!(
            j.get("validation").and_then(|v| v.as_str()),
            Some(want),
            "{} rows",
            n
        );
        let (m, _, v) = top_and_verdict(&t);
        assert!(
            v == "exact" && same_function(&m, &[(0.5, 5.0), (0.5, 5.0)], |x| x[0] * x[1].sqrt()),
            "{} rows: {} ({})",
            n,
            m,
            v
        );
    }
}

#[test]
fn reference_documents_every_output_field() {
    let doc =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/reference.md")).unwrap();
    let mut cfg = test_cfg();
    cfg.timing = true;
    let t = csv_table(30, &[(0.5, 5.0)], |x| 2.0 * x[0], 97);
    let out = run_text(&t, cfg).unwrap();
    let j = crate::forms::json::parse(&out).unwrap();
    let schema = j.get("schema").and_then(|v| v.as_str()).unwrap_or("");
    assert!(
        !schema.is_empty() && doc.contains(schema),
        "schema {} not in the reference",
        schema
    );
    let answer = &out[out.find("\"results\":[").unwrap() + 11..];
    let answer = &answer[..answer.find('}').unwrap() + 1];
    let fields = json_keys(&out)
        .into_iter()
        .chain(json_keys(answer))
        .collect::<Vec<_>>();
    assert!(fields.len() > 25, "{:?}", fields);
    for k in fields {
        assert!(
            doc.contains(&format!("`{}`", k)),
            "field {} is not in docs/reference.md",
            k
        );
    }
}

fn json_keys(obj: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let (mut depth, mut in_str, mut esc, mut cur) = (0i32, false, false, None::<String>);
    for c in obj.chars() {
        if in_str {
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            } else if let Some(k) = cur.as_mut() {
                k.push(c);
            }
            continue;
        }
        match c {
            '{' | '[' => depth += 1,
            '}' | ']' => depth -= 1,
            '"' => {
                in_str = true;
                cur = (depth == 1).then(String::new);
            }
            ':' if depth == 1 => keys.extend(cur.take()),
            _ => {
                if c != ' ' {
                    cur = None;
                }
            }
        }
    }
    keys
}

#[test]
fn values_near_the_float_limit_do_not_crash() {
    let t = "x,y\n1e308,1e308\n-1e308,-1e308\n2e307,2e307\n3,3\n4,4\n5,5\n";
    let j = run_json(t);
    assert!(j.get("verdict").is_some());
    let w = format!("{:?}", j.get("warnings"));
    assert!(w.contains("overflow"), "{}", w);
}

#[test]
fn small_tables_say_what_their_rows_allow() {
    let j = run_json("x,y\n1,2\n2,4\n3,6\n4,8\n");
    let r = j.get("reason").and_then(|v| v.as_str()).unwrap_or("");
    assert!(
        r.contains("4 data rows") && r.contains("at least 5"),
        "{}",
        r
    );
    let j = run_json("x,y\n1,2\n2,4\n3,6\n4,8\n5,10\n");
    let r = j.get("reason").and_then(|v| v.as_str()).unwrap_or("");
    assert!(r.contains("no fitted constant") && r.contains('6'), "{}", r);
    let t = "m,r,f\n1.5,2.5,0.24\n2.5,1.5,1.1111111111111\n3.5,0.5,14\n0.5,3.5,0.040816326530612\n4.5,4.5,0.22222222222222\n";
    let mut cfg = test_cfg();
    cfg.target_col = "f".to_string();
    let j = crate::forms::json::parse(&run_text(t, cfg).unwrap()).unwrap();
    assert_eq!(run_verdict(&j), "exact");
}

#[test]
fn a_constant_feature_is_dropped() {
    let mut s = 1u64;
    let t: String = "x,c,y\n".to_string()
        + &(0..20)
            .map(|_| {
                let x = 1.0 + 4.0 * lcg(&mut s);
                format!("{},5,{}\n", x, x)
            })
            .collect::<String>();
    let j = run_json(&t);
    assert!(format!("{:?}", j.get("warnings")).contains("'c'"));
    assert!(run_text("a,b,y\n1,2,3\n1,2,4\n1,2,5\n1,2,6\n", test_cfg())
        .unwrap_err()
        .contains("same value"));
}

#[test]
fn round_feature_values_are_settings_when_the_target_is_precise() {
    let mut t = String::from("theta,y\n");
    for k in 1..=15 {
        let th = k as f64 / 10.0;
        t.push_str(&format!("{},{:.10}\n", th, th.sin()));
    }
    let (m, _, v) = top_and_verdict(&t);
    assert_eq!((m.as_str(), v.as_str()), ("sin(x1)", "exact"));
    let mut t = String::from("L,y\n");
    for l in [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0] {
        t.push_str(&format!(
            "{},{:.5}\n",
            l,
            2.0 * std::f64::consts::PI * (l / 9.81f64).sqrt()
        ));
    }
    let (m, _, v) = top_and_verdict(&t);
    assert!(v == "exact" && m.starts_with("2.006"), "{} ({})", m, v);
    let t: String = "r,y\n".to_string()
        + &(1..=6)
            .map(|r| format!("{},{:.11}\n", r, std::f64::consts::PI * (r * r) as f64))
            .collect::<String>();
    let j = run_json(&t);
    let top = j
        .get("results")
        .and_then(|r| r.as_arr())
        .and_then(|r| r.first())
        .cloned()
        .unwrap();
    assert!(
        top.get("symbolic")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("pi"),
        "{:?}",
        top.get("symbolic")
    );
}

#[test]
fn exponential_with_an_offset_and_the_wider_constants() {
    let t: String = "t,y\n".to_string()
        + &(0..25)
            .map(|k| {
                let t = k as f64 * 10.0 / 24.0;
                format!("{},{}\n", t, 3.0 * (-0.5 * t).exp() + 2.0)
            })
            .collect::<String>();
    let (m, _, v) = top_and_verdict(&t);
    assert_eq!((m.as_str(), v.as_str()), ("3*exp(-0.5*x1) + 2", "exact"));
    for (v, want) in [
        (0.398_942_280_401_432_7, "1/sqrt(2*pi)"),
        (std::f64::consts::LN_2, "log(2)"),
        (0.577_215_664_901_532_9, "euler_gamma"),
        (2.0 * std::f64::consts::PI, "2*pi"),
    ] {
        assert_eq!(
            crate::forms::nf::symbolic_constant(v).as_deref(),
            Some(want)
        );
    }
    assert_eq!(
        crate::forms::nf::symbolic_constant(7.0 * std::f64::consts::LN_10 / 8.0),
        None
    );
    assert_eq!(
        crate::forms::nf::symbolic_constant(2.0 * std::f64::consts::LN_2).as_deref(),
        Some("2*log(2)")
    );
}

#[test]
fn five_rows_confirm_a_law_without_constants() {
    let t = "m1,m2,r,y\n1.5,2.5,0.5,15\n2.5,1.5,1.5,1.6666666666667\n3.5,0.5,2.5,0.28\n0.5,3.5,1,1.75\n4.5,4.5,2,5.0625\n";
    let (m, _, v) = top_and_verdict(t);
    assert_eq!((m.as_str(), v.as_str()), ("x1*x2/x3**2", "exact"));
    let j = run_json(
        "m1,m2,r,y\n1.5,2.5,0.5,15\n2.5,1.5,1.5,1.6666666666667\n3.5,0.5,2.5,0.28\n0.5,3.5,1,1.75\n",
    );
    assert!(j
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .contains("at least 5"));
}

#[test]
fn a_copy_of_the_target_and_a_second_target_name() {
    let mut s = 3u64;
    let t: String = "x,copy,y\n".to_string()
        + &(0..12)
            .map(|_| {
                let x = 0.2 + lcg(&mut s);
                format!("{},{},{}\n", x, x * x, x * x)
            })
            .collect::<String>();
    let (m, _, v) = top_and_verdict(&t);
    assert_eq!((m.as_str(), v.as_str()), ("x2", "exact"));
    let t: String = "y,x,y\n".to_string()
        + &(0..12)
            .map(|_| {
                let x = 0.2 + lcg(&mut s);
                format!("{},{},{}\n", 0.5 * x, x, 1.5 * x)
            })
            .collect::<String>();
    let j = run_json(&t);
    assert_eq!(
        j.get("names")
            .and_then(|n| n.get("columns"))
            .and_then(|c| c.as_arr())
            .map(|c| c.len()),
        Some(1)
    );
}

#[test]
fn a_saturated_function_ranks_after_the_clean_law() {
    let t = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/nasa_planets_orbit.tsv"
    ))
    .unwrap();
    let mut cfg = test_cfg();
    cfg.target_col = "orbital_velocity_kms".to_string();
    let j = crate::forms::json::parse(&run_text(&t, cfg).unwrap()).unwrap();
    let top = j
        .get("results")
        .and_then(|r| r.as_arr())
        .and_then(|r| r.first())
        .cloned()
        .unwrap();
    let m = top
        .get("named")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    assert_eq!(m, "72.4*distance_1e6km/orbital_period_days");
    let d = [
        57.9, 108.2, 149.6, 228.0, 778.5, 1432.0, 2867.0, 4515.0, 5906.4,
    ];
    let p = [
        88.0, 224.7, 365.2, 687.0, 4331.0, 10747.0, 30589.0, 59800.0, 90560.0,
    ];
    assert!(crate::guard::has_saturated(
        "232*arctan(x2)/sqrt(x1)",
        &[d.to_vec(), p.to_vec()],
        9
    ));
}

#[test]
fn an_unmeasured_floor_is_not_called_full_precision() {
    let t = "L,y\n0.1,0.63\n0.2,0.9\n0.3,1.1\n0.4,1.3\n0.5,1.4\n0.6,1.6\n0.7,1.7\n0.8,1.8\n0.9,1.9\n1.0,2.0\n1.1,2.1\n1.2,2.2\n";
    let out = run_text(t, test_cfg()).unwrap();
    let s = crate::summary::render(&out);
    assert!(
        !s.contains("full precision") && s.contains("not estimated"),
        "{}",
        s
    );
}

#[test]
fn a_precise_target_ends_the_search_early_and_wide_tables_keep_the_budget() {
    let mut s = 9u64;
    let t = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/gravity.tsv"))
        .unwrap();
    let mut cfg = test_cfg();
    cfg.target_col = "force".to_string();
    let j = crate::forms::json::parse(&run_text(&t, cfg).unwrap()).unwrap();
    assert_eq!(
        j.get("stopped_by").and_then(|v| v.as_str()),
        Some("exact_exit")
    );
    let cols = 45;
    let t: String = (1..=cols).map(|i| format!("f{},", i)).collect::<String>()
        + "y\n"
        + &(0..50)
            .map(|_| {
                let x: Vec<f64> = (0..cols).map(|_| 0.5 + 4.5 * lcg(&mut s)).collect();
                x.iter().map(|v| format!("{},", v)).collect::<String>()
                    + &format!("{}\n", x[4] * x[40])
            })
            .collect::<String>();
    let mut cfg = test_cfg();
    cfg.timeout_ms = 3000;
    let t0 = std::time::Instant::now();
    let out = run_text(&t, cfg).unwrap();
    assert!(t0.elapsed().as_secs_f64() < 12.0, "{:?}", t0.elapsed());
    assert!(out.contains("feature columns"));
}

#[test]
fn a_wide_span_target_ranks_approximate_answers_by_relative_misses() {
    let t = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/nasa_planets_period.tsv"
    ))
    .unwrap();
    let mut cfg = test_cfg();
    cfg.target_col = "period_days".to_string();
    let j = crate::forms::json::parse(&run_text(&t, cfg).unwrap()).unwrap();
    let named: Vec<String> = j
        .get("results")
        .and_then(|r| r.as_arr())
        .unwrap()
        .iter()
        .take(2)
        .map(|r| {
            r.get("named")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        })
        .collect();
    assert_eq!(named[0], "0.199*distance_1e6km**(3/2)", "{:?}", named);
    let top = j
        .get("results")
        .and_then(|r| r.as_arr())
        .and_then(|r| r.first())
        .unwrap();
    assert!(top
        .get("largest_miss")
        .and_then(|v| v.as_f64())
        .is_some_and(|m| m > 0.0 && m < 0.05));
    assert_eq!(
        top.get("within_precision").and_then(|v| v.as_bool()),
        Some(false)
    );
    let summary = crate::summary::render(
        &run_text(&t, {
            let mut c = test_cfg();
            c.target_col = "period_days".to_string();
            c
        })
        .unwrap(),
    );
    assert!(
        summary.contains("r2 = 0.9999") && !summary.contains("r2 = 1.0000"),
        "{}",
        summary
    );
}

#[test]
fn row_index_columns_are_dropped_and_repeated_names_renamed() {
    let mut s = 21u64;
    let t: String = "rownames,a,b,y\n".to_string()
        + &(1..=30)
            .map(|k| {
                let (a, b) = (0.5 + 4.0 * lcg(&mut s), 0.5 + 4.0 * lcg(&mut s));
                format!("{},{},{},{}\n", k, a, b, a * b)
            })
            .collect::<String>();
    let j = run_json(&t);
    assert_eq!(
        j.get("names")
            .and_then(|n| n.get("columns"))
            .and_then(|c| c.as_arr())
            .map(|c| c.len()),
        Some(2)
    );
    let t: String = "t,a,y\n".to_string()
        + &(1..=30)
            .map(|k| {
                let a = 0.5 + 4.0 * lcg(&mut s);
                format!("{},{},{}\n", k, a, a * k as f64)
            })
            .collect::<String>();
    let (m, _, v) = top_and_verdict(&t);
    assert_eq!((m.as_str(), v.as_str()), ("x1*x2", "exact"));
    let t: String = "x,x,y\n".to_string()
        + &(0..20)
            .map(|_| {
                let (a, b) = (0.5 + 4.0 * lcg(&mut s), 0.5 + 4.0 * lcg(&mut s));
                format!("{},{},{}\n", a, b, a + b)
            })
            .collect::<String>();
    let j = run_json(&t);
    let top = j
        .get("results")
        .and_then(|r| r.as_arr())
        .and_then(|r| r.first())
        .cloned()
        .unwrap();
    assert_eq!(top.get("named").and_then(|v| v.as_str()), Some("x + x_2"));
}

#[test]
fn a_constant_of_planck_size_is_confirmed() {
    let mut s = 21u64;
    let t: String = "lambda,c,y\n".to_string()
        + &(0..40)
            .map(|_| {
                let (l, c) = (1e-9 + 1e-3 * lcg(&mut s), 2.9e8 + 2e7 * lcg(&mut s));
                format!("{:.12e},{:.12e},{:.12e}\n", l, c, 6.626e-34 * c / l)
            })
            .collect::<String>();
    let (m, _, v) = top_and_verdict(&t);
    assert_eq!(v, "exact", "{}", m);
    assert!(m.contains("6.626e-34"), "{}", m);
}

#[test]
fn a_decay_rate_is_not_rounded_off_its_rows() {
    let mut s = 33u64;
    let t: String = "t,y\n".to_string()
        + &(0..50)
            .map(|_| {
                let x = 60.0 * lcg(&mut s);
                format!("{:.12},{:.12e}\n", x, 1000.0 * (-x / 12.3).exp())
            })
            .collect::<String>();
    let (m, _, v) = top_and_verdict(&t);
    assert_eq!(v, "exact", "{}", m);
    assert!(
        same_function(&m, &[(0.0, 60.0)], |x| 1000.0 * (-x[0] / 12.3).exp()),
        "{}",
        m
    );
    assert_eq!(crate::driver::percent(9.1e-4), "0.00091");
    assert_eq!(crate::driver::percent(0.87), "0.87");
}

#[test]
fn a_wide_table_is_searched_on_its_most_related_columns() {
    let mut s = 45u64;
    let cols = 60;
    let t: String = (1..=cols).map(|i| format!("c{},", i)).collect::<String>()
        + "y\n"
        + &(0..60)
            .map(|_| {
                let x: Vec<f64> = (0..cols).map(|_| 1.0 + 9.0 * lcg(&mut s)).collect();
                x.iter().map(|v| format!("{},", v)).collect::<String>()
                    + &format!("{}\n", x[6] * x[18].sqrt())
            })
            .collect::<String>();
    let mut cfg = test_cfg();
    cfg.timeout_ms = 0;
    let j = crate::forms::json::parse(&run_text(&t, cfg).unwrap()).unwrap();
    let top = j
        .get("results")
        .and_then(|r| r.as_arr())
        .and_then(|r| r.first())
        .cloned()
        .unwrap();
    assert_eq!(
        top.get("named").and_then(|v| v.as_str()),
        Some("c7*sqrt(c19)")
    );
    assert_eq!(
        j.get("names")
            .and_then(|n| n.get("columns"))
            .and_then(|c| c.as_arr())
            .map(|c| c.len()),
        Some(crate::config::SCREEN_COLUMNS)
    );
    let w = j
        .get("warnings")
        .and_then(|w| w.as_arr())
        .cloned()
        .unwrap_or_default();
    assert!(w.iter().any(|x| x
        .as_str()
        .is_some_and(|x| x.contains("the 30 most related"))));
}

#[test]
fn a_named_constant_is_tried_before_a_longer_fraction() {
    let order = crate::forms::search::nice_near(6.285);
    let at = |x: f64| order.iter().position(|v| (v - x).abs() < 1e-12).unwrap();
    assert!(
        at(2.0 * std::f64::consts::PI) < at(19.0 / 3.0),
        "{:?}",
        &order[..12]
    );
    let mut s = 61u64;
    let sig2 = |v: f64| format!("{:.1e}", v).parse::<f64>().unwrap();
    let t: String = "L,g,T\n".to_string()
        + &(0..60)
            .map(|_| {
                let (l, g) = (0.1 + 2.9 * lcg(&mut s), 1.5 + 23.5 * lcg(&mut s));
                format!(
                    "{},{},{}\n",
                    sig2(l),
                    sig2(g),
                    sig2(2.0 * std::f64::consts::PI * (l / g).sqrt())
                )
            })
            .collect::<String>();
    for reversed in [false, true] {
        crate::forms::search::REVERSED_ORDER.with(|r| r.set(reversed));
        let mut cfg = test_cfg();
        cfg.target_col = "T".to_string();
        let j = crate::forms::json::parse(&run_text(&t, cfg).unwrap()).unwrap();
        crate::forms::search::REVERSED_ORDER.with(|r| r.set(false));
        let top = j
            .get("results")
            .and_then(|r| r.as_arr())
            .and_then(|r| r.first())
            .cloned()
            .unwrap();
        assert!(
            top.get("symbolic")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .contains("2*pi"),
            "reversed {}: {:?}",
            reversed,
            top.get("model")
        );
    }
}

#[test]
fn integer_written_laws_keep_their_offsets() {
    let t = "x,y\n1,3\n2,5\n3,7\n4,9\n5,11\n6,13\n7,15\n8,17\n";
    let (m, _, v) = top_and_verdict(t);
    assert_eq!((m.as_str(), v.as_str()), ("2*x1 + 1", "exact"));
    let j = run_json(t);
    let top = j
        .get("results")
        .and_then(|r| r.as_arr())
        .and_then(|r| r.first())
        .cloned()
        .unwrap();
    assert_eq!(top.get("holdout_r2").and_then(|v| v.as_f64()), Some(1.0));
    let mut s = 71u64;
    let t: String = "a,b,y\n".to_string()
        + &(0..30)
            .map(|_| {
                let (a, b) = (
                    1 + (9.0 * lcg(&mut s)) as i64,
                    1 + (9.0 * lcg(&mut s)) as i64,
                );
                format!("{},{},{}\n", a, b, a * b + 3)
            })
            .collect::<String>();
    let (m, _, v) = top_and_verdict(&t);
    assert_eq!((m.as_str(), v.as_str()), ("x1*x2 + 3", "exact"));
}

#[test]
fn rows_short_of_the_header_are_named_as_such() {
    let t = "a\tb\ttarget\n0\t0\n1\t2\n2\t4\n3\t6\n4\t8\n5\t10\n";
    let e = run_text(t, {
        let mut c = test_cfg();
        c.target_col = "target".to_string();
        c
    })
    .unwrap_err();
    assert!(
        e.contains("no data row has the header's 3 fields (most rows have 2)"),
        "{}",
        e
    );
}

#[test]
fn a_named_constant_needs_the_data_to_single_it_out() {
    let t = "planet,a_1e6km,T_days\nMercury,58,88\nVenus,110,220\nEarth,150,370\nMars,230,690\nJupiter,780,4300\nSaturn,1400,11000\nUranus,2900,31000\nNeptune,4500,60000\n";
    let mut cfg = test_cfg();
    cfg.target_col = "T_days".to_string();
    let j = crate::forms::json::parse(&run_text(t, cfg).unwrap()).unwrap();
    let top = j
        .get("results")
        .and_then(|r| r.as_arr())
        .and_then(|r| r.first())
        .cloned()
        .unwrap();
    let named = top
        .get("named")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    assert_eq!(named, "0.2*a_1e6km**(3/2)");
}

#[test]
fn a_target_of_tied_values_keeps_its_held_out_rows() {
    let mut s = 5u64;
    let t: String = "x,y\n".to_string()
        + &(0..80)
            .map(|_| {
                let x = lcg(&mut s);
                format!("{},{}\n", x, if x > 0.5 { 1 } else { 0 })
            })
            .collect::<String>();
    let j = run_json(&t);
    let top = j
        .get("results")
        .and_then(|r| r.as_arr())
        .and_then(|r| r.first())
        .cloned()
        .unwrap();
    assert!(
        top.get("holdout_r2")
            .and_then(|v| v.as_f64())
            .is_some_and(|r| r > 0.5),
        "{:?}",
        top.get("holdout_r2")
    );
}

fn sig_g(v: f64, sig: i32) -> String {
    let d = (sig - 1 - v.abs().log10().floor() as i32).max(0) as usize;
    let s = format!("{:.*}", d, v);
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

#[test]
fn a_decay_written_to_three_digits_is_exact() {
    let t: String = "t,y\n".to_string()
        + &(0..40)
            .map(|t| format!("{},{}\n", t, sig_g(500.0 * (-(t as f64) / 7.3).exp(), 3)))
            .collect::<String>();
    let j = run_json(&t);
    let top = j
        .get("results")
        .and_then(|r| r.as_arr())
        .and_then(|r| r.first())
        .cloned()
        .unwrap();
    assert_eq!(precision_verdict(&top), "exact", "{:?}", top.get("model"));
    assert_eq!(
        top.get("structure")
            .and_then(|v| v.as_str())
            .map(|s| s.contains("exp")),
        Some(true),
        "{:?}",
        top.get("model")
    );
}

#[test]
fn an_offset_and_a_scale_are_snapped_together() {
    let t: String = "lambda,y\n".to_string()
        + &(0..41)
            .map(|k| {
                let l = 380.0 + 10.0 * k as f64;
                format!("{},{}\n", l, sig_g(1239.84 / l, 6))
            })
            .collect::<String>();
    let j = run_json(&t);
    let top = j
        .get("results")
        .and_then(|r| r.as_arr())
        .and_then(|r| r.first())
        .cloned()
        .unwrap();
    assert_eq!(
        top.get("model").and_then(|v| v.as_str()),
        Some("1239.84*1/x1")
    );
    assert_eq!(precision_verdict(&top), "exact");
}

fn top_of(table: &str) -> (String, String, String) {
    let j = run_json(table);
    let top = j
        .get("results")
        .and_then(|r| r.as_arr())
        .and_then(|r| r.first())
        .cloned()
        .unwrap();
    let s = |k: &str| {
        top.get(k)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    (s("model"), s("named"), precision_verdict(&top))
}

#[test]
fn a_named_value_needs_more_than_noise_to_be_named() {
    let mut s = 3u64;
    let mut gauss = move || {
        let (u, v) = (lcg(&mut s).max(1e-12), lcg(&mut s));
        (-2.0 * u.ln()).sqrt() * (2.0 * std::f64::consts::PI * v).cos()
    };
    let mut s2 = 5u64;
    let c = 7.0 * 2f64.powf(0.25);
    let t: String = "n,t,v,y\n".to_string()
        + &(0..200)
            .map(|_| {
                let (n, tk, v) = (
                    0.5 + 2.5 * lcg(&mut s2),
                    250.0 + 150.0 * lcg(&mut s2),
                    1.0 + 9.0 * lcg(&mut s2),
                );
                format!(
                    "{},{},{},{}\n",
                    sig_g(n, 6),
                    sig_g(tk, 6),
                    sig_g(v, 6),
                    sig_g(c * n * tk / v * (1.0 + 0.01 * gauss()), 6)
                )
            })
            .collect::<String>();
    let (_, named, _) = top_of(&t);
    assert!(!named.contains("**(1/4)"), "{}", named);
}

#[test]
fn a_named_constant_the_data_single_out_is_named() {
    let mut s = 9u64;
    let t: String = "x,y\n".to_string()
        + &(0..60)
            .map(|_| {
                let x = 0.5 + 3.5 * lcg(&mut s);
                format!(
                    "{},{}\n",
                    sig_g(x, 4),
                    sig_g(
                        (-x * x / 2.0).exp() / (2.0 * std::f64::consts::PI).sqrt(),
                        4
                    )
                )
            })
            .collect::<String>();
    let (_, named, verdict) = top_of(&t);
    assert!(named.contains("sqrt(2*pi)"), "{}", named);
    assert_eq!(verdict, "exact");
}

#[test]
fn a_withdrawn_name_leaves_the_law_with_a_decimal() {
    let t = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/channel_capacity_r3.tsv"
    ))
    .unwrap();
    let (model, named, _) = top_of(&t);
    assert!(model.ends_with("*log(x1/x2 + 1)"), "{} ({})", model, named);
}

#[test]
fn readings_that_repeat_at_their_precision_are_not_a_design_grid() {
    let mut s = 3u64;
    let mut next = || {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        0.5 + 2.5 * ((s >> 11) as f64 / (1u64 << 53) as f64)
    };
    let dense: Vec<f64> = (0..600_000)
        .map(|_| {
            let v = next();
            let q = if v < 1.0 { 1e-6 } else { 1e-5 };
            (v / q).round() * q
        })
        .collect();
    let q: Vec<f64> = dense
        .iter()
        .map(|v| if *v < 1.0 { 1e-6 } else { 1e-5 })
        .collect();
    assert!(!crate::driver::design_grid(&dense, &q));
    let grid: Vec<f64> = (0..200).map(|i| 273.15 + 5.0 * (i % 20) as f64).collect();
    let gq = vec![0.01; grid.len()];
    assert!(crate::driver::design_grid(&grid, &gq));
}

fn real(file: &str, target: &str) -> crate::forms::json::Json {
    let t = std::fs::read_to_string(format!(
        "{}/fixtures/real/{}",
        env!("CARGO_MANIFEST_DIR"),
        file
    ))
    .unwrap();
    let mut cfg = test_cfg();
    cfg.target_col = target.to_string();
    crate::forms::json::parse(&run_text(&t, cfg).unwrap()).unwrap()
}

fn real_top(j: &crate::forms::json::Json) -> (String, String) {
    let v = j
        .get("verdict")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let top = j
        .get("results")
        .and_then(|r| r.as_arr())
        .and_then(|r| r.first());
    (
        v,
        top.and_then(|t| t.get("named"))
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string(),
    )
}

#[test]
fn real_planets_give_keplers_third_law() {
    let (v, m) = real_top(&real("planets8.csv", "T_yr"));
    assert_eq!((v.as_str(), m.as_str()), ("law", "a_AU**(3/2)"));
    for f in ["orbits26.csv", "orbits26_3dec.csv"] {
        let (v, m) = real_top(&real(f, "period_years"));
        assert_eq!(
            (v.as_str(), m.as_str()),
            ("law", "distance_AU**(3/2)"),
            "{}",
            f
        );
    }
}

#[test]
fn real_pendulum_swings_give_two_pi_root_l_over_g() {
    let (v, m) = real_top(&real("pendulum200_1pct.csv", "period_s"));
    assert_eq!(
        (v.as_str(), m.as_str()),
        ("law", "(2*pi)*sqrt(length_m)/sqrt(g)")
    );
    let (v, m) = real_top(&real("pendulum30_1pct.csv", "period_s"));
    assert_eq!(
        (v.as_str(), m.as_str()),
        ("law", "(2*pi)*sqrt(length_m)/sqrt(g)")
    );
}

fn real_summary(file: &str, target: &str) -> String {
    let t = std::fs::read_to_string(format!(
        "{}/fixtures/real/{}",
        env!("CARGO_MANIFEST_DIR"),
        file
    ))
    .unwrap();
    let mut cfg = test_cfg();
    cfg.target_col = target.to_string();
    crate::summary::render(&run_text(&t, cfg).unwrap())
}

#[test]
fn real_surface_gravity_shows_the_simpler_law() {
    let s = real_summary("surface_gravity.tsv", "g_ms2");
    let line = s
        .lines()
        .find(|l| l.starts_with("simpler law: g_ms2 = "))
        .unwrap_or_else(|| panic!("{}", s));
    assert!(
        line.contains("*mass_1e24kg/radius_km**2 (r2 = 0.98") && line.ends_with("1 constant free)"),
        "{}",
        s
    );
}

#[test]
fn real_steam_table_fit_that_misses_its_small_pressures_is_refused() {
    let t = std::fs::read_to_string(format!(
        "{}/fixtures/real/steam_pressure.tsv",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let rows: Vec<(f64, f64)> = t
        .lines()
        .skip(1)
        .map(|l| {
            let v: Vec<f64> = l.split('\t').map(|x| x.parse().unwrap()).collect();
            (v[0], v[1])
        })
        .collect();
    let f = vec![rows.iter().map(|r| r.0).collect::<Vec<f64>>()];
    let y: Vec<f64> = rows.iter().map(|r| r.1).collect();
    let q: Vec<f64> = vec![1e-4; y.len()];
    assert_eq!(
        crate::guard::badly_missed(
            "0.002229*exp(2.309401076759*x1**(1/3)) + 1",
            &f,
            &y,
            &q,
            y.len()
        ),
        Some((2, 1))
    );
    let s = real_summary("steam_pressure.tsv", "P_kPa");
    assert!(!s.starts_with("P_kPa = 0.002229*exp("), "{}", s);
    assert!(!s.contains("possibly"), "{}", s);
}

#[test]
fn a_narrow_target_with_small_misses_is_a_law() {
    let mut seed: u64 = 12345;
    let mut next = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut t = String::from("x\ty\n");
    for i in 0..60 {
        let x = 1.0 + 0.3 * i as f64 / 59.0;
        let y = 100.0 * x.powf(0.1) * (1.0 + 0.01 * (2.0 * next() - 1.0));
        t.push_str(&format!("{:.4}\t{:.3}\n", x, y));
    }
    let mut cfg = test_cfg();
    cfg.target_col = "y".to_string();
    let j = crate::forms::json::parse(&run_text(&t, cfg).unwrap()).unwrap();
    let r2 = j
        .get("results")
        .and_then(|r| r.as_arr())
        .and_then(|r| r.first())
        .and_then(|r| r.get("r2"))
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0);
    assert!(r2 < 0.9, "r2 {}", r2);
    assert_eq!(real_top(&j).0, "law");
}

#[test]
fn real_boyle_gives_pressure_inverse_to_volume() {
    let (v, m) = real_top(&real("boyle_1662.tsv", "pressure"));
    let k: f64 = m.split('*').next().unwrap_or("").parse().unwrap_or(0.0);
    assert!(
        v == "law" && m.ends_with("*1/volume") && (1390.0..1420.0).contains(&k),
        "{} {}",
        v,
        m
    );
}

#[test]
fn real_scatter_is_no_law() {
    for (f, t) in [
        ("hubble_1929.tsv", "velocity_kms"),
        ("cars_stopping.csv", "dist_ft"),
    ] {
        let j = real(f, t);
        let (v, m) = real_top(&j);
        assert_eq!(v, "no law found", "{}: {}", f, m);
        let reason = j.get("reason").and_then(|r| r.as_str()).unwrap_or("");
        assert!(
            reason.contains("explains less than 90%")
                || reason.contains("does not beat a constant"),
            "{}: {}",
            f,
            reason
        );
    }
}

#[test]
fn physical_constants_are_named_once_and_never_over_a_column() {
    use crate::forms::nf::physical_constant;
    let n = |v: f64| physical_constant(v).map(|(t, _)| t);
    assert_eq!(n(1.380_649e-23 * 6.022_140_76e23).as_deref(), Some("R"));
    assert_eq!(n(1.0 / 299_792_458.0).as_deref(), Some("1/c"));
    assert_eq!(n(2.0 * 6.674_30e-11).as_deref(), Some("2*G"));
    assert_eq!(n(-9.806_65).as_deref(), Some("-g_n"));
    assert_eq!(n(6.674_30e-11 * 299_792_458.0), None);
    assert_eq!(
        crate::output::symbolic_model("6.6743e-11*x1", &["M".to_string()]),
        "G*x1"
    );
    assert_eq!(
        crate::output::symbolic_model("6.6743e-11*x1", &["G".to_string()]),
        "6.6743e-11*x1"
    );
}

#[test]
fn a_noisy_ideal_gas_in_si_units_gives_r() {
    let mut seed: u64 = 777;
    let mut next = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut t = String::from("T\tV\tP\n");
    for _ in 0..80 {
        let (temp, vol) = (250.0 + 150.0 * next(), 0.01 + 0.04 * next());
        let p = 8.314_462_618 * temp / vol * (1.0 + 0.01 * (2.0 * next() - 1.0));
        t.push_str(&format!("{:.1}\t{:.4}\t{:.0}\n", temp, vol, p));
    }
    let mut cfg = test_cfg();
    cfg.target_col = "P".to_string();
    let j = crate::forms::json::parse(&run_text(&t, cfg).unwrap()).unwrap();
    assert_eq!(real_top(&j), ("law".to_string(), "R*T/V".to_string()));
    let free = j
        .get("results")
        .and_then(|r| r.as_arr())
        .and_then(|r| r.first())
        .and_then(|r| r.get("free_constants"))
        .and_then(|f| f.as_f64());
    assert_eq!(free, Some(0.0));
}

#[test]
fn real_jupiter_moons_give_keplers_law_over_a_longer_form() {
    let (v, m) = real_top(&real("jupiter_moons.tsv", "P_days"));
    let one_term = m
        .split_once('*')
        .is_some_and(|(k, rest)| k.parse::<f64>().is_ok() && rest == "a_km**(3/2)");
    assert!(v == "law" && one_term, "{} {}", v, m);
}

#[test]
fn cleaning_never_takes_an_answer_out_of_the_cells_precision() {
    let mut s = 5u64;
    let mut next = || {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        0.5 + 2.5 * ((s >> 11) as f64 / (1u64 << 53) as f64)
    };
    let mut t = String::from("x\ty\n");
    for _ in 0..60 {
        let x = next();
        t.push_str(&format!("{:.12}\t{:.12}\n", x, 1.0006 * x * x));
    }
    let (model, _, verdict) = top_of(&t);
    assert_eq!(
        (model.as_str(), verdict.as_str()),
        ("1.0006*x1**2", "exact")
    );
}

#[test]
fn a_named_physical_constant_is_not_free() {
    let mut t = String::from("f\tE\n");
    for i in 0..30 {
        let f = 1e14 * (1.0 + 0.3 * i as f64);
        let e = 6.62607015e-34 * f;
        t.push_str(&format!("{:.5e}\t{:.5e}\n", f, e));
    }
    let mut cfg = test_cfg();
    cfg.target_col = "E".to_string();
    let out = run_text(&t, cfg).unwrap();
    let j = crate::forms::json::parse(&out).unwrap();
    let r = j
        .get("results")
        .and_then(|r| r.as_arr())
        .and_then(|r| r.first())
        .cloned()
        .unwrap();
    let named = r
        .get("named")
        .and_then(|n| n.as_str())
        .unwrap_or("")
        .to_string();
    let free = r.get("free_constants").and_then(|n| n.as_f64());
    assert_eq!((named.as_str(), free), ("h*f", Some(0.0)), "{}", out);
}

#[test]
fn a_constant_fitted_short_of_its_best_value_is_still_named() {
    let j = real("pendulum50_5pct.tsv", "T");
    let (v, m) = real_top(&j);
    assert_eq!((v.as_str(), m.as_str()), ("law", "(2*pi)*sqrt(L)/sqrt(g)"));
    assert!(
        matches!(j.get("simpler_law"), Some(crate::forms::json::Json::Null)),
        "{:?}",
        j.get("simpler_law")
    );
}
