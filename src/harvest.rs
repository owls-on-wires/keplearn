use crate::*;

pub(crate) fn harvest_monomial_vcols(
    t: &[f64],
    features: &[Vec<f64>],
    n_cols: usize,
    sample_n: usize,
    qn: usize,
    max_terms: usize,
) -> Vec<VC> {
    let q = qn.min(sample_n);
    if q < HARVEST_MIN_ROWS || n_cols == 0 {
        return Vec::new();
    }
    let mut logcols: Vec<Vec<f64>> = Vec::with_capacity(n_cols);
    for c in 0..n_cols {
        let mut col = vec![f64::NAN; sample_n];
        for k in 0..sample_n {
            let v = features[c][k].abs();
            col[k] = if v > 1e-300 { v.ln() } else { f64::NAN };
        }
        logcols.push(col);
    }
    let mut resid: Vec<f64> = t[..sample_n].to_vec();
    let mut out: Vec<VC> = Vec::new();
    let mut used: Vec<Vec<i32>> = Vec::new();
    for _iter in 0..(max_terms + PURSUIT_EXTRA_ROUNDS) {
        if out_of_time() {
            break;
        }
        if out.len() >= max_terms {
            break;
        }
        let mut y = vec![f64::NAN; sample_n];
        for k in 0..sample_n {
            let v = resid[k].abs();
            y[k] = if resid[k].is_finite() && v > 1e-300 {
                v.ln()
            } else {
                f64::NAN
            };
        }
        let Some(sol) = solve_ls(&logcols, &y) else {
            break;
        };
        let mut exps: Vec<i32> = Vec::with_capacity(n_cols);
        let mut deg = 0i32;
        for c in 0..n_cols {
            let e = sol[c];
            let ei = round_clamp_exp(e, &MONO_HARVEST);
            deg += ei.abs();
            exps.push(ei);
        }
        if deg == 0 || deg > MONO_HARVEST.deg_cap {
            break;
        }
        if used.iter().any(|u| *u == exps) {
            break;
        }
        let mut col = vec![0.0f64; sample_n];
        for k in 0..sample_n {
            let mut p = 1.0f64;
            for c in 0..n_cols {
                if exps[c] != 0 {
                    p *= features[c][k].powi(exps[c]);
                }
            }
            col[k] = p;
        }
        let finite = col.iter().filter(|x| x.is_finite()).count();
        if finite < min_finite_rows(q) {
            break;
        }
        let (r2, a, b) = r2_score(&resid[..sample_n], &col[..sample_n]);
        let _ = b;
        if !(r2 >= MONO_MIN_R2) || !a.is_finite() || a == 0.0 {
            break;
        }
        if deg >= HARVEST_VCOL_MIN_DEG {
            let (gexpr, desc) = monomial_strings(&exps);
            out.push(VC {
                gexpr,
                desc,
                data: Rc::new(col.clone()),
            });
        }
        used.push(exps);
        for k in 0..sample_n {
            if resid[k].is_finite() && col[k].is_finite() {
                resid[k] -= a * col[k];
            }
        }
    }
    out
}

pub(crate) fn combined_r2_k(basis: &[Vec<f64>], t: &[f64]) -> (f64, Option<Vec<f64>>) {
    let n = t.len();
    let k = basis.len();
    if k == 0 {
        return (0.0, None);
    }
    let cols: Vec<Vec<f64>> = basis.iter().map(|c| c[..n].to_vec()).collect();
    let Some(sol) = solve_ls(&cols, t) else {
        return (0.0, None);
    };
    let finite_row = |r: usize| t[r].is_finite() && cols.iter().all(|c| c[r].is_finite());
    let mut sum = 0.0f64;
    let mut cnt = 0.0f64;
    for r in 0..n {
        if finite_row(r) {
            sum += t[r];
            cnt += 1.0;
        }
    }
    if cnt < KMONO_COMBINED_MIN_ROWS {
        return (0.0, None);
    }
    let mean = sum / cnt;
    let mut sse = 0.0f64;
    let mut sst = 0.0f64;
    for r in 0..n {
        if !finite_row(r) {
            continue;
        }
        let mut pred = sol[k];
        for i in 0..k {
            pred += sol[i] * cols[i][r];
        }
        sse += (t[r] - pred) * (t[r] - pred);
        sst += (t[r] - mean) * (t[r] - mean);
    }
    if sst <= 0.0 || !sse.is_finite() {
        return (0.0, None);
    }
    (1.0 - sse / sst, Some(sol))
}

pub(crate) fn harvest_k_monomial(
    t: &[f64],
    features: &[Vec<f64>],
    n_cols: usize,
    sample_n: usize,
    qn: usize,
    k: usize,
    min_deg: i32,
) -> (Vec<VC>, f64, Option<Vec<f64>>) {
    let q = qn.min(sample_n);
    if q < HARVEST_MIN_ROWS || n_cols == 0 || k < 2 {
        return (Vec::new(), 0.0, None);
    }
    let mut logcols: Vec<Vec<f64>> = Vec::with_capacity(n_cols);
    for c in 0..n_cols {
        let mut col = vec![f64::NAN; sample_n];
        for kk in 0..sample_n {
            let v = features[c][kk].abs();
            col[kk] = if v > 1e-300 { v.ln() } else { f64::NAN };
        }
        logcols.push(col);
    }
    let materialize = |exps: &[i32]| -> Vec<f64> {
        let mut col = vec![0.0f64; sample_n];
        for kk in 0..sample_n {
            let mut p = 1.0f64;
            for c in 0..n_cols {
                if exps[c] != 0 {
                    p *= features[c][kk].powi(exps[c]);
                }
            }
            col[kk] = p;
        }
        col
    };
    let fit_exps = |resid: &[f64], top_frac: bool| -> Option<Vec<i32>> {
        let mut y = vec![f64::NAN; sample_n];
        for kk in 0..sample_n {
            let v = resid[kk].abs();
            y[kk] = if resid[kk].is_finite() && v > 1e-300 {
                v.ln()
            } else {
                f64::NAN
            };
        }
        if top_frac {
            let mut idx: Vec<usize> = (0..sample_n).filter(|&kk| y[kk].is_finite()).collect();
            idx.sort_by(|&a, &b| {
                resid[b]
                    .abs()
                    .partial_cmp(&resid[a].abs())
                    .unwrap_or(CmpOrd::Equal)
            });
            let keep = (idx.len() * KMONO_TOPFRAC_NUM / KMONO_TOPFRAC_DEN).max(KMONO_TOPFRAC_MIN);
            for &kk in idx.iter().skip(keep) {
                y[kk] = f64::NAN;
            }
        }
        let sol = solve_ls(&logcols, &y)?;
        let mut exps = Vec::with_capacity(n_cols);
        let mut deg = 0i32;
        for c in 0..n_cols {
            let e = sol[c];
            let ei = round_clamp_exp(e, &MONO_HARVEST);
            deg += ei.abs();
            exps.push(ei);
        }
        if deg == 0 || deg > MONO_HARVEST.deg_cap {
            return None;
        }
        Some(exps)
    };
    let attempt = |first_top: bool| -> (Vec<Vec<i32>>, Vec<Vec<f64>>) {
        let mut exps_list: Vec<Vec<i32>> = Vec::new();
        let mut cols_list: Vec<Vec<f64>> = Vec::new();
        let mut resid: Vec<f64> = t[..sample_n].to_vec();
        for i in 0..k {
            let tf = first_top && i == 0;
            let Some(e) = fit_exps(&resid, tf) else { break };
            if exps_list.iter().any(|u| *u == e) {
                break;
            }
            let col = materialize(&e);
            if col.iter().filter(|x| x.is_finite()).count() < min_finite_rows(q) {
                break;
            }
            let (_r, a, _) = r2_score(&resid[..sample_n], &col[..sample_n]);
            if !a.is_finite() || a == 0.0 {
                break;
            }
            for kk in 0..sample_n {
                if resid[kk].is_finite() && col[kk].is_finite() {
                    resid[kk] -= a * col[kk];
                }
            }
            exps_list.push(e);
            cols_list.push(col);
        }
        (exps_list, cols_list)
    };
    let mut dict: Vec<Vec<i32>> = Vec::new();
    {
        let cap = DICT_ATOM_CAP;
        let evals: [i32; 6] = [1, 2, 3, -1, -2, -3];
        'd1: for a in 0..n_cols {
            for &ea in &evals {
                let mut e = vec![0i32; n_cols];
                e[a] = ea;
                let d: i32 = e.iter().map(|x| x.abs()).sum();
                if (1..=DICT_ATOM_MAX_DEG).contains(&d) {
                    dict.push(e);
                }
                if dict.len() >= cap {
                    break 'd1;
                }
            }
        }
        if dict.len() < cap {
            'd2: for a in 0..n_cols {
                for b in (a + 1)..n_cols {
                    for &ea in &evals {
                        for &eb in &evals {
                            let mut e = vec![0i32; n_cols];
                            e[a] = ea;
                            e[b] = eb;
                            let d: i32 = e.iter().map(|x| x.abs()).sum();
                            if (1..=DICT_ATOM_MAX_DEG).contains(&d) {
                                dict.push(e);
                            }
                            if dict.len() >= cap {
                                break 'd2;
                            }
                        }
                    }
                }
            }
        }
        if dict.len() < cap {
            'd3: for a in 0..n_cols {
                for b in (a + 1)..n_cols {
                    for c in (b + 1)..n_cols {
                        for &ea in &evals {
                            for &eb in &evals {
                                for &ec in &evals {
                                    let mut e = vec![0i32; n_cols];
                                    e[a] = ea;
                                    e[b] = eb;
                                    e[c] = ec;
                                    let d: i32 = e.iter().map(|x| x.abs()).sum();
                                    if (1..=DICT_ATOM_MAX_DEG).contains(&d) {
                                        dict.push(e);
                                    }
                                    if dict.len() >= cap {
                                        break 'd3;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let dcols: Vec<Vec<f64>> = dict.iter().map(|e| materialize(e)).collect();
    let pursuit = |forced_first: Option<&Vec<i32>>| -> (Vec<Vec<i32>>, Vec<Vec<f64>>) {
        let mut chosen: Vec<Vec<i32>> = Vec::new();
        let mut chosen_cols: Vec<Vec<f64>> = Vec::new();
        let mut resid: Vec<f64> = t[..sample_n].to_vec();
        for _ in 0..k {
            if chosen.is_empty() {
                if let Some(ff) = forced_first {
                    let col = materialize(ff);
                    if col.iter().filter(|x| x.is_finite()).count() >= min_finite_rows(q) {
                        chosen.push(ff.clone());
                        chosen_cols.push(col);
                        let (_cr, sol) = combined_r2_k(&chosen_cols, &t[..sample_n]);
                        let Some(sol) = sol else { break };
                        for kk in 0..sample_n {
                            if !t[kk].is_finite() {
                                resid[kk] = f64::NAN;
                                continue;
                            }
                            let mut pred = sol[chosen_cols.len()];
                            let mut ok = true;
                            for (ci, col) in chosen_cols.iter().enumerate() {
                                if !col[kk].is_finite() {
                                    ok = false;
                                    break;
                                }
                                pred += sol[ci] * col[kk];
                            }
                            resid[kk] = if ok { t[kk] - pred } else { f64::NAN };
                        }
                        continue;
                    } else {
                        break;
                    }
                }
            }
            let mut best_i: Option<usize> = None;
            let mut best_ac = 0.0f64;
            for (di, dc) in dcols.iter().enumerate() {
                if dc.iter().filter(|x| x.is_finite()).count() < min_finite_rows(q) {
                    continue;
                }
                if chosen.iter().any(|u| *u == dict[di]) {
                    continue;
                }
                let ac = abs_corr(dc, &resid, sample_n);
                if ac.is_finite() && ac > best_ac {
                    best_ac = ac;
                    best_i = Some(di);
                }
            }
            let Some(bi) = best_i else { break };
            chosen.push(dict[bi].clone());
            chosen_cols.push(dcols[bi].clone());
            let (_cr, sol) = combined_r2_k(&chosen_cols, &t[..sample_n]);
            let Some(sol) = sol else { break };
            for kk in 0..sample_n {
                if !t[kk].is_finite() {
                    resid[kk] = f64::NAN;
                    continue;
                }
                let mut pred = sol[chosen_cols.len()];
                let mut ok = true;
                for (ci, col) in chosen_cols.iter().enumerate() {
                    if !col[kk].is_finite() {
                        ok = false;
                        break;
                    }
                    pred += sol[ci] * col[kk];
                }
                resid[kk] = if ok { t[kk] - pred } else { f64::NAN };
            }
        }
        (chosen, chosen_cols)
    };
    let mut best_exps: Vec<Vec<i32>> = Vec::new();
    let mut best_cols: Vec<Vec<f64>> = Vec::new();
    let mut best_r2 = 0.0f64;
    let mut best_sol: Option<Vec<f64>> = None;
    {
        let (pe, pc) = pursuit(None);
        if pc.len() >= 2 {
            let degs_ok = pe.iter().all(|e| {
                let d: i32 = e.iter().map(|x| x.abs()).sum();
                d >= min_deg
            });
            if degs_ok {
                let (cr2, sol) = combined_r2_k(&pc, &t[..sample_n]);
                if cr2 > best_r2 {
                    best_r2 = cr2;
                    best_exps = pe;
                    best_cols = pc;
                    best_sol = sol;
                }
            }
        }
    }
    for first_top in [false, true] {
        let (exps_list, cols_list) = attempt(first_top);
        if cols_list.len() < 2 {
            continue;
        }
        let degs_ok = exps_list.iter().all(|e| {
            let d: i32 = e.iter().map(|x| x.abs()).sum();
            d >= min_deg
        });
        if !degs_ok {
            continue;
        }
        let (cr2, sol) = combined_r2_k(&cols_list, &t[..sample_n]);
        if cr2 > best_r2 {
            best_r2 = cr2;
            best_exps = exps_list;
            best_cols = cols_list;
            best_sol = sol;
        }
    }
    if k >= 3 && best_r2 < KMONO_CONVERGE_R2 && !best_exps.is_empty() {
        let mut used = vec![false; n_cols];
        for e in &best_exps {
            for c in 0..n_cols {
                if e[c] != 0 {
                    used[c] = true;
                }
            }
        }
        'fseed: for c in 0..n_cols {
            if used[c] {
                continue;
            }
            let mut cand: Vec<(f64, usize)> = Vec::new();
            for (di, dc) in dcols.iter().enumerate() {
                if dict[di][c] == 0 {
                    continue;
                }
                let d: i32 = dict[di].iter().map(|x| x.abs()).sum();
                if d < min_deg {
                    continue;
                }
                if dc.iter().filter(|x| x.is_finite()).count() < min_finite_rows(q) {
                    continue;
                }
                let ac = abs_corr(dc, &t[..sample_n], sample_n);
                if ac.is_finite() {
                    cand.push((ac, di));
                }
            }
            cand.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(CmpOrd::Equal));
            for &(_, di) in cand.iter().take(FSEED_TOP.min(mono_terms(n_cols))) {
                let seed = dict[di].clone();
                let (pe, pc) = pursuit(Some(&seed));
                if pc.len() < 2 {
                    continue;
                }
                let degs_ok = pe.iter().all(|e| {
                    let d: i32 = e.iter().map(|x| x.abs()).sum();
                    d >= min_deg
                });
                if !degs_ok {
                    continue;
                }
                let (cr2, sol) = combined_r2_k(&pc, &t[..sample_n]);
                if cr2 > best_r2 {
                    best_r2 = cr2;
                    best_exps = pe;
                    best_cols = pc;
                    best_sol = sol;
                }
                if best_r2 >= KMONO_CONVERGE_R2 {
                    break 'fseed;
                }
            }
        }
    }
    if k >= 3
        && best_r2 < KMONO_CONVERGE_R2
        && best_exps.len() == best_cols.len()
        && best_cols.len() >= 2
    {
        let mut sweeps = 0;
        let mut improved = true;
        while improved
            && sweeps < KMONO_SWAP_SWEEPS
            && best_r2 < KMONO_CONVERGE_R2
            && !out_of_time()
        {
            improved = false;
            sweeps += 1;
            for pos in 0..best_exps.len() {
                let mut best_di: Option<usize> = None;
                let mut local_r2 = best_r2;
                for (di, de) in dict.iter().enumerate() {
                    let d: i32 = de.iter().map(|x| x.abs()).sum();
                    if d < min_deg {
                        continue;
                    }
                    if *de == best_exps[pos] {
                        continue;
                    }
                    if best_exps
                        .iter()
                        .enumerate()
                        .any(|(i, e)| i != pos && e == de)
                    {
                        continue;
                    }
                    if dcols[di].iter().filter(|x| x.is_finite()).count() < min_finite_rows(q) {
                        continue;
                    }
                    let saved = std::mem::replace(&mut best_cols[pos], dcols[di].clone());
                    let (cr2, _sol) = combined_r2_k(&best_cols, &t[..sample_n]);
                    best_cols[pos] = saved;
                    if cr2 > local_r2 + IMPROVE_EPS {
                        local_r2 = cr2;
                        best_di = Some(di);
                    }
                }
                if let Some(di) = best_di {
                    best_exps[pos] = dict[di].clone();
                    best_cols[pos] = dcols[di].clone();
                    let (cr2, sol) = combined_r2_k(&best_cols, &t[..sample_n]);
                    best_r2 = cr2;
                    best_sol = sol;
                    improved = true;
                }
            }
        }
    }
    if best_r2 > KMONO_NUDGE_FLOOR
        && best_r2 < KMONO_CONVERGE_R2
        && best_exps.len() >= 2
        && best_exps.len() == best_cols.len()
    {
        let mut sweeps = 0;
        let mut improved = true;
        while improved && sweeps < KMONO_NUDGE_SWEEPS && best_r2 < KMONO_CONVERGE_R2 {
            improved = false;
            sweeps += 1;
            for pos in 0..best_exps.len() {
                for c in 0..n_cols {
                    for &d in &[-1i32, 1] {
                        let mut cand = best_exps[pos].clone();
                        cand[c] += d;
                        if cand[c].abs() > MONO_VCOL.max_abs_exp {
                            continue;
                        }
                        let deg: i32 = cand.iter().map(|x| x.abs()).sum();
                        if deg < min_deg || deg > MONO_VCOL.deg_cap {
                            continue;
                        }
                        if best_exps
                            .iter()
                            .enumerate()
                            .any(|(i, e)| i != pos && *e == cand)
                        {
                            continue;
                        }
                        let col = materialize(&cand);
                        if col.iter().filter(|x| x.is_finite()).count() < min_finite_rows(q) {
                            continue;
                        }
                        let saved_col = std::mem::replace(&mut best_cols[pos], col);
                        let saved_exp = std::mem::replace(&mut best_exps[pos], cand);
                        let (cr2, sol) = combined_r2_k(&best_cols, &t[..sample_n]);
                        if cr2 > best_r2 + IMPROVE_EPS {
                            best_r2 = cr2;
                            best_sol = sol;
                            improved = true;
                        } else {
                            best_cols[pos] = saved_col;
                            best_exps[pos] = saved_exp;
                        }
                    }
                }
            }
        }
    }
    if n_cols >= 1 && n_cols <= KMONO_DENSE_MAX_VARS && best_r2 < KMONO_CONVERGE_R2 {
        let mut basis: Vec<Vec<i32>> = Vec::new();
        let mut e = vec![0i32; n_cols];
        loop {
            let d: i32 = e.iter().sum();
            if (1..=KMONO_DENSE_DEG).contains(&d) {
                basis.push(e.clone());
            }
            let mut c = 0usize;
            while c < n_cols {
                e[c] += 1;
                if e[c] <= KMONO_DENSE_DEG {
                    break;
                }
                e[c] = 0;
                c += 1;
            }
            if c == n_cols {
                break;
            }
        }
        if basis.len() >= 2 {
            let mut cols: Vec<Vec<f64>> = basis.iter().map(|x| materialize(x)).collect();
            let mut exps = basis;
            let (cr2, _) = combined_r2_k(&cols, &t[..sample_n]);
            if cr2 >= KMONO_FINALIST_R2 {
                while exps.len() > 2 {
                    let mut best_after = -1.0f64;
                    let mut rm = 0usize;
                    for i in 0..exps.len() {
                        let sub: Vec<Vec<f64>> = cols
                            .iter()
                            .enumerate()
                            .filter(|(j, _)| *j != i)
                            .map(|(_, c)| c.clone())
                            .collect();
                        let (c2, _) = combined_r2_k(&sub, &t[..sample_n]);
                        if c2 > best_after {
                            best_after = c2;
                            rm = i;
                        }
                    }
                    if best_after >= KMONO_FINALIST_R2 {
                        exps.remove(rm);
                        cols.remove(rm);
                    } else {
                        break;
                    }
                }
                let (fr2, sol) = combined_r2_k(&cols, &t[..sample_n]);
                if fr2 > best_r2 && cols.len() >= 2 && sol.is_some() {
                    best_r2 = fr2;
                    best_exps = exps;
                    best_cols = cols;
                    best_sol = sol;
                }
            }
        }
    }
    if std::env::var("KEPLEARN_KMONO_DBG").is_ok() {
        eprintln!(
            "KMONO-RET k={} min_deg={} best_r2={:.8} nterms={} exps={:?}",
            k,
            min_deg,
            best_r2,
            best_cols.len(),
            best_exps
        );
    }
    if best_cols.len() < 2 {
        return (Vec::new(), 0.0, None);
    }
    let mut out: Vec<VC> = Vec::new();
    for (e, col) in best_exps.iter().zip(best_cols.into_iter()) {
        let (g, d) = monomial_strings(e);
        out.push(VC {
            gexpr: g,
            desc: d,
            data: Rc::new(col),
        });
    }
    (out, best_r2, best_sol)
}

pub(crate) fn mono_fit(inputs: &[&[f64]], target: &[f64], sample_n: usize) -> Vec<Hit> {
    let eps = 1e-12;
    let sn = sample_n.min(target.len());
    let k = inputs.len();
    if !(MONO_FIT_MIN_ARITY..=12).contains(&k) || sn < HARVEST_MIN_ROWS {
        return Vec::new();
    }
    let mut basis: Vec<Vec<f64>> = Vec::with_capacity(k);
    for &col in inputs.iter() {
        let mut lc = vec![f64::NAN; sn];
        for r in 0..sn {
            let v = col[r];
            if v.is_finite() && v.abs() > eps {
                lc[r] = v.abs().ln();
            }
        }
        basis.push(lc);
    }
    let mut out: Vec<Hit> = Vec::new();
    for &off in const_offsets_fit(target, &basis, sn).iter() {
        let mut ly = vec![f64::NAN; sn];
        for r in 0..sn {
            let t = target[r] - off;
            if target[r].is_finite() && t.abs() > eps {
                ly[r] = t.abs().ln();
            }
        }
        let Some(sol) = solve_ls(&basis, &ly) else {
            continue;
        };
        let mut exps: Vec<(usize, f64)> = Vec::new();
        let mut clean = true;
        for c in 0..k {
            let b = sol[c];
            match snap_exp(b) {
                Some(snapped) => {
                    if snapped != 0.0 {
                        exps.push((c, snapped));
                    }
                }
                None => {
                    clean = false;
                    break;
                }
            }
        }
        if !clean || exps.is_empty() || exps.len() > MONO_FIT_EXP_CAP {
            continue;
        }
        let mut m = vec![f64::NAN; sn];
        for r in 0..sn {
            let mut p = 1.0f64;
            let mut ok = true;
            for &(c, e) in &exps {
                let v = inputs[c][r];
                if !v.is_finite() {
                    ok = false;
                    break;
                }
                p *= v.powf(e);
            }
            if ok && p.is_finite() {
                m[r] = p;
            }
        }
        let (r2, a, b) = r2_score(&target[..sn], &m);
        if !(r2 >= MONO_FIT_MIN_R2) || !a.is_finite() || a == 0.0 || !b.is_finite() {
            continue;
        }
        let mut expr = String::from("(");
        for (i, &(_c, e)) in exps.iter().enumerate() {
            if i > 0 {
                expr.push('*');
            }
            expr.push_str(&format!("(x{}**({}))", i + 1, fmt_exp(e)));
        }
        expr.push(')');
        let columns: Vec<usize> = exps.iter().map(|&(c, _)| c).collect();
        if out.iter().any(|h| h.expr == expr && h.columns == columns) {
            continue;
        }
        let is_raw = off == 0.0;
        out.push(Hit {
            expr,
            r2,
            scale: a,
            offset: b,
            columns,
            gate_r2: r2,
        });
        if is_raw {
            break;
        }
    }
    out
}

pub(crate) fn harvest_cheap(t: &[f64], inputs: &[&[f64]], qn: usize) -> f64 {
    affine_monomial_r2(t, inputs, qn)
        .max(monomial_loglin_r2(t, inputs, qn))
        .max(quick_probe(t, inputs, qn))
}

pub(crate) fn harvest_la(t: &[f64], inputs: &[&[f64]], qn: usize, sample_n: usize) -> f64 {
    let base = harvest_cheap(t, inputs, qn);
    if base >= IO_NEAREXACT_R2 {
        return base;
    }
    let la = fit_shell_const(t, inputs, qn, sample_n, |y, c| {
        let z = 1.0 + c * y;
        if z > 1e-30 {
            z.ln()
        } else {
            f64::NAN
        }
    })
    .1;
    base.max(la)
}
