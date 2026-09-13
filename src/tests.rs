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
fn literal_spans_skip_identifiers() {
    let spans = literal_spans("((x12*3.5)+0.999063000000)");
    let vals: Vec<f64> = spans.iter().map(|&(_, _, v)| v).collect();
    assert_eq!(vals, vec![3.5, 0.999063]);
    let snaps: Vec<(usize, usize, String)> = spans
        .iter()
        .filter_map(|&(a, b, v)| snap_value(v).map(|s| (a, b, fmt_const(s))))
        .collect();
    let out = apply_snaps(
        "((x12*3.5)+0.999063000000)",
        &snaps,
        &vec![true; snaps.len()],
    );
    assert_eq!(out, "((x12*3.5)+1.000000000000)");
}

#[test]
fn snap_model_constants_holdout_verified() {
    let n = 60usize;
    let pr = |i: usize, k: f64| -> f64 {
        let h = (i as f64 + 1.0) * k;
        let s = h.sin() * 43758.5453;
        s - s.floor()
    };
    let x1: Vec<f64> = (0..n).map(|i| 0.3 + 2.5 * pr(i, 12.9898)).collect();
    let x2: Vec<f64> = (0..n).map(|i| 1.1 + 1.8 * pr(i, 78.233)).collect();
    let features = vec![x1.clone(), x2.clone()];
    let (h_lo, h_hi) = (30usize, n);
    let ho_keep = vec![true; h_hi - h_lo];
    let ho_min_rows = (h_hi - h_lo) * 9 / 10;
    let mut ev = Evaluator::new(4, n);
    let target: Vec<f64> = (0..n).map(|i| x2[i] * (x1[i] + 1.0)).collect();
    let model = "(x2*(x1+0.999063000000))";
    let (base_rh, _) = holdout_model_r2(
        model,
        &features,
        &target,
        h_lo,
        h_hi,
        &ho_keep,
        ho_min_rows,
        &mut ev,
    )
    .unwrap();
    let snapped = snap_model_constants(
        model,
        base_rh,
        &features,
        &target,
        h_lo,
        h_hi,
        &ho_keep,
        ho_min_rows,
        &mut ev,
    )
    .expect("near-integer inner constant must snap");
    assert_eq!(snapped, "(x2*(x1+1.000000000000))");
    assert!(snap_model_constants(
        "(x2*(x1+1.000000000000))",
        1.0,
        &features,
        &target,
        h_lo,
        h_hi,
        &ho_keep,
        ho_min_rows,
        &mut ev
    )
    .is_none());
    let target2: Vec<f64> = (0..n).map(|i| x2[i] * (x1[i] + 0.51)).collect();
    let model2 = "(x2*(x1+0.510000000000))";
    let (rh2, _) = holdout_model_r2(
        model2,
        &features,
        &target2,
        h_lo,
        h_hi,
        &ho_keep,
        ho_min_rows,
        &mut ev,
    )
    .unwrap();
    assert!(snap_model_constants(
        model2,
        rh2,
        &features,
        &target2,
        h_lo,
        h_hi,
        &ho_keep,
        ho_min_rows,
        &mut ev
    )
    .is_none());
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
fn snap_zero_guard_keeps_load_bearing_small_coefficients() {
    let n = 60usize;
    let pr = |i: usize, k: f64| -> f64 {
        let h = (i as f64 + 1.0) * k;
        let s = h.sin() * 43758.5453;
        s - s.floor()
    };
    let x1: Vec<f64> = (0..n)
        .map(|i| {
            if i < 30 {
                10.0 + i as f64
            } else {
                0.1 + 0.01 * (i as f64 - 30.0)
            }
        })
        .collect();
    let x2: Vec<f64> = (0..n).map(|i| 1.0 + 2.0 * pr(i, 78.233)).collect();
    let features = vec![x1.clone(), x2.clone()];
    let target: Vec<f64> = (0..n)
        .map(|i| 0.001 * x1[i].powi(3) + 5.0 * x2[i] + 100.0)
        .collect();
    let model = "((0.001000000000*((x1)**3))+((5.000000000000*x2)+100.000000000000))";
    let (h_lo, h_hi) = (30usize, n);
    let ho_keep = vec![true; h_hi - h_lo];
    let ho_min_rows = (h_hi - h_lo) * 9 / 10;
    let mut ev = Evaluator::new(6, n);
    let (base_rh, _) = holdout_model_r2(
        model,
        &features,
        &target,
        h_lo,
        h_hi,
        &ho_keep,
        ho_min_rows,
        &mut ev,
    )
    .unwrap();
    let gutted = "((0.000000000000*((x1)**3))+((5.000000000000*x2)+100.000000000000))";
    let (gutted_rh, _) = holdout_model_r2(
        gutted,
        &features,
        &target,
        h_lo,
        h_hi,
        &ho_keep,
        ho_min_rows,
        &mut ev,
    )
    .unwrap();
    let slack = (1.0 - base_rh).max(0.0) * SNAP_HOLDOUT_SLACK + 1e-9;
    assert!(
        gutted_rh >= base_rh - slack,
        "test premise broken: holdout must be blind to the term ({} vs {})",
        gutted_rh,
        base_rh
    );
    let snapped = snap_model_constants(
        model,
        base_rh,
        &features,
        &target,
        h_lo,
        h_hi,
        &ho_keep,
        ho_min_rows,
        &mut ev,
    );
    match snapped {
        None => {}
        Some(m) => {
            assert!(
                m.contains("0.001000000000"),
                "load-bearing coefficient was zero-snapped: {}",
                m
            )
        }
    }
}

#[test]
fn simplify_model_drops_zeroed_terms() {
    let m = "(x4*(0.000000000000*(((x2)**0.000000000000*(x5)**0.000000000000/(x1*x4)))+0.000000000000*(((x2)**6*x5/((x1)**5*x3*x4)))+86.500000000000))";
    let s = simplify_model(m).expect("must simplify");
    assert!(
        !s.contains("0.000000000000*") && !s.contains("**(0)"),
        "zeroed terms survived: {}",
        s
    );
    let vars: Vec<Vec<f64>> = (0..5)
        .map(|j| {
            (0..8)
                .map(|i| 0.7 + 0.31 * (i as f64) + 0.17 * (j as f64))
                .collect()
        })
        .collect();
    let slices: Vec<&[f64]> = vars.iter().map(|v| v.as_slice()).collect();
    let a = eval_str(m, &slices, 8);
    let b = eval_str(&s, &slices, 8);
    for i in 0..8 {
        assert!(
            (a[i] - b[i]).abs() <= 1e-9 * (1.0 + a[i].abs()),
            "row {}: {} vs {} ({})",
            i,
            a[i],
            b[i],
            s
        );
    }
    let s2 = simplify_model("(((x1**(1))*(1.000000000000*(x2**(1)))))").expect("identities");
    let b2 = eval_str(&s2, &slices, 8);
    let a2 = eval_str("(x1*x2)", &slices, 8);
    for i in 0..8 {
        assert!((a2[i] - b2[i]).abs() < 1e-12, "{}", s2);
    }
    assert!(simplify_model("((2.500000000000*sin(x1))+0.300000000000)").is_none());
    assert!(simplify_model("(exp(x3)*(x2/(x1*x4)))").is_none());
}

#[test]
fn affine_fit_calibrates_emitted_string() {
    let n = 50usize;
    let x1: Vec<f64> = (0..n).map(|i| 0.5 + 0.1 * i as f64).collect();
    let features = vec![x1.clone()];
    let target: Vec<f64> = x1.iter().map(|v| 3.0 * v * v + 5.0).collect();
    let mut ev = Evaluator::new(4, n);
    let (a, b) =
        full_model_affine_fit("((x1)**(2))", &features, &target, &mut ev).expect("calibratable");
    assert!(
        (a - 3.0).abs() < 1e-9 && (b - 5.0).abs() < 1e-9,
        "a={} b={}",
        a,
        b
    );
    let raw = full_model_r2("((x1)**(2))", &features, &target, &mut ev);
    let baked = full_model_r2(
        "(3.000000000000*(((x1)**(2)))+5.000000000000)",
        &features,
        &target,
        &mut ev,
    );
    assert!(raw < 0.999, "raw={}", raw);
    assert!((baked - 1.0).abs() < 1e-9, "baked={}", baked);
    let (a, b) = full_model_affine_fit(
        "(3.000000000000*(((x1)**(2)))+5.000000000000)",
        &features,
        &target,
        &mut ev,
    )
    .expect("fit");
    assert!(affine_trivial(a, b), "a={} b={}", a, b);
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
