use crate::forms::fit::linear_fit;

pub(crate) struct Prep {
    pub(crate) atoms: Vec<Vec<f64>>,
    ac: Vec<Vec<f64>>,
    gram: Vec<f64>,
}

impl Prep {
    fn center(v: &[f64]) -> Vec<f64> {
        let n = v.len();
        let m = v.iter().sum::<f64>() / n as f64;
        let c: Vec<f64> = v.iter().map(|x| x - m).collect();
        let s = c.iter().map(|x| x * x).sum::<f64>().sqrt().max(1e-300);
        c.iter().map(|x| x / s).collect()
    }

    pub(crate) fn new(atoms: Vec<Vec<f64>>) -> Prep {
        Prep {
            atoms: Vec::new(),
            ac: Vec::new(),
            gram: Vec::new(),
        }
        .extend(atoms)
    }

    pub(crate) fn extend(&self, extra: Vec<Vec<f64>>) -> Prep {
        let n0 = self.atoms.len();
        let na = n0 + extra.len();
        let mut ac = self.ac.clone();
        ac.extend(extra.iter().map(|v| Prep::center(v)));
        let mut gram = vec![0.0f64; na * na];
        for i in 0..n0 {
            gram[i * na..i * na + n0].copy_from_slice(&self.gram[i * n0..(i + 1) * n0]);
        }
        for i in n0..na {
            for j in 0..=i {
                let v: f64 = ac[i].iter().zip(&ac[j]).map(|(p, q)| p * q).sum();
                gram[i * na + j] = v;
                gram[j * na + i] = v;
            }
        }
        let mut atoms = self.atoms.clone();
        atoms.extend(extra);
        Prep { atoms, ac, gram }
    }
}

pub(crate) fn pursue(
    p: &Prep,
    y: &[f64],
    max_terms: usize,
    min_terms: usize,
) -> Option<Vec<usize>> {
    pursue_masked(p, y, max_terms, min_terms, None)
}

pub(crate) fn pursue_masked(
    p: &Prep,
    y: &[f64],
    max_terms: usize,
    min_terms: usize,
    allowed: Option<&[bool]>,
) -> Option<Vec<usize>> {
    let n = y.len();
    let na = p.atoms.len();
    if na == 0 || na > 4000 {
        return None;
    }
    let (atoms, ac, gram) = (&p.atoms, &p.ac, &p.gram);
    let ym = y.iter().sum::<f64>() / n as f64;
    let yc: Vec<f64> = y.iter().map(|v| v - ym).collect();
    let yy: f64 = yc.iter().map(|v| v * v).sum();
    if !(yy > 0.0 && yy.is_finite()) {
        return None;
    }
    let cy: Vec<f64> = ac
        .iter()
        .map(|a| a.iter().zip(&yc).map(|(p, q)| p * q).sum())
        .collect();
    let ok = |k: usize| {
        allowed.map_or(true, |a| a[k]) && gram[k * na + k].is_finite() && cy[k].is_finite()
    };
    let rel = |set: &[usize]| -> f64 {
        let k = set.len();
        let mut buf = [0.0f64; 90];
        let mut heap;
        let m: &mut [f64] = if k * (k + 1) <= buf.len() {
            &mut buf[..k * (k + 1)]
        } else {
            heap = vec![0.0f64; k * (k + 1)];
            &mut heap[..]
        };
        for (r, &i) in set.iter().enumerate() {
            for (c, &j) in set.iter().enumerate() {
                m[r * (k + 1) + c] = gram[i * na + j];
            }
            m[r * (k + 1) + k] = cy[i];
        }
        for col in 0..k {
            let piv = (col..k)
                .max_by(|&a, &b| {
                    m[a * (k + 1) + col]
                        .abs()
                        .total_cmp(&m[b * (k + 1) + col].abs())
                })
                .unwrap();
            if m[piv * (k + 1) + col].abs() < 1e-14 {
                return f64::INFINITY;
            }
            for c in 0..=k {
                m.swap(col * (k + 1) + c, piv * (k + 1) + c);
            }
            for r in 0..k {
                if r != col {
                    let f = m[r * (k + 1) + col] / m[col * (k + 1) + col];
                    for c in col..=k {
                        m[r * (k + 1) + c] -= f * m[col * (k + 1) + c];
                    }
                }
            }
        }
        let fit: f64 = set
            .iter()
            .enumerate()
            .map(|(r, &i)| cy[i] * m[r * (k + 1) + k] / m[r * (k + 1) + r])
            .sum();
        ((yy - fit) / yy).max(0.0)
    };
    let exact_qr = |set: &[usize]| -> bool {
        let g: Vec<Vec<f64>> = set.iter().map(|&j| atoms[j].clone()).collect();
        let Some((a, b)) = linear_fit(&g, y) else {
            return false;
        };
        let ss: f64 = (0..n)
            .map(|r| (y[r] - b - a.iter().zip(&g).map(|(ai, gi)| ai * gi[r]).sum::<f64>()).powi(2))
            .sum();
        1.0 - ss / yy >= 1.0 - 1e-10
    };
    let rel1 = |k: usize| -> f64 {
        let gkk = gram[k * na + k];
        if gkk.abs() < 1e-14 {
            return f64::INFINITY;
        }
        ((yy - cy[k] * cy[k] / gkk) / yy).max(0.0)
    };
    let rel2 = |i: usize, j: usize| -> f64 {
        let (gii, gjj, g) = (gram[i * na + i], gram[j * na + j], gram[i * na + j]);
        let det = gii * gjj - g * g;
        if gii.abs().max(g.abs()) < 1e-14 || det.abs() < 1e-14 * gii.abs().max(g.abs()) {
            return f64::INFINITY;
        }
        let fit = (gjj * cy[i] * cy[i] + gii * cy[j] * cy[j] - 2.0 * g * cy[i] * cy[j]) / det;
        ((yy - fit) / yy).max(0.0)
    };
    let base = |r: &[usize]| -> Option<(Vec<f64>, f64)> {
        let k = r.len();
        let mut a = vec![0.0f64; k * 2 * k];
        for (p, &i) in r.iter().enumerate() {
            for (q, &j) in r.iter().enumerate() {
                a[p * 2 * k + q] = gram[i * na + j];
            }
            a[p * 2 * k + k + p] = 1.0;
        }
        for col in 0..k {
            let piv = (col..k).max_by(|&x, &y| {
                a[x * 2 * k + col]
                    .abs()
                    .total_cmp(&a[y * 2 * k + col].abs())
            })?;
            if a[piv * 2 * k + col].abs() < 1e-14 {
                return None;
            }
            for c in 0..2 * k {
                a.swap(col * 2 * k + c, piv * 2 * k + c);
            }
            let d = a[col * 2 * k + col];
            for c in 0..2 * k {
                a[col * 2 * k + c] /= d;
            }
            for rr in 0..k {
                if rr != col {
                    let f = a[rr * 2 * k + col];
                    if f != 0.0 {
                        for c in 0..2 * k {
                            a[rr * 2 * k + c] -= f * a[col * 2 * k + c];
                        }
                    }
                }
            }
        }
        let inv: Vec<f64> = (0..k)
            .flat_map(|p| a[p * 2 * k + k..p * 2 * k + 2 * k].to_vec())
            .collect();
        let mut fit = 0.0;
        for p in 0..k {
            for q in 0..k {
                fit += cy[r[p]] * inv[p * k + q] * cy[r[q]];
            }
        }
        Some((inv, fit))
    };
    let rel_add = |r: &[usize], b: &(Vec<f64>, f64), j: usize| -> f64 {
        let k = r.len();
        let (inv, fit_r) = b;
        let mut gv = [0.0f64; 16];
        let mut gw = 0.0;
        let mut cw = 0.0;
        for p in 0..k {
            let mut s = 0.0;
            for q in 0..k {
                s += inv[p * k + q] * gram[r[q] * na + j];
            }
            gv[p] = s;
        }
        for p in 0..k {
            gw += gram[r[p] * na + j] * gv[p];
            cw += cy[r[p]] * gv[p];
        }
        let d = gram[j * na + j] - gw;
        if d.abs() < 1e-14 {
            return f64::INFINITY;
        }
        let num = cy[j] - cw;
        ((yy - fit_r - num * num / d) / yy).max(0.0)
    };
    let mut singles: Vec<(f64, usize)> = (0..na).filter(|&k| ok(k)).map(|k| (rel1(k), k)).collect();
    singles.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
    for i in (0..na).filter(|&i| ok(i)) {
        for j in ((i + 1)..na).filter(|&j| ok(j)) {
            let e = rel2(i, j);
            if pairs.len() < 16 || e < pairs[pairs.len() - 1].0 {
                pairs.push((e, i, j));
                pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
                pairs.truncate(16);
            }
        }
    }
    let seeds: Vec<Vec<usize>> = pairs
        .iter()
        .map(|p| vec![p.1, p.2])
        .chain(singles.iter().take(8).map(|s| vec![s.1]))
        .collect();
    for seed in seeds {
        let locked = seed.len();
        let mut chosen = seed.clone();
        let mut cur = rel(&chosen);
        loop {
            for _ in 0..4 {
                let mut improved = false;
                for pos in locked..chosen.len() {
                    let rest: Vec<usize> = chosen
                        .iter()
                        .enumerate()
                        .filter(|&(p, _)| p != pos)
                        .map(|(_, &i)| i)
                        .collect();
                    let b = if rest.len() < 16 { base(&rest) } else { None };
                    for k in 0..na {
                        if !ok(k) || chosen.contains(&k) {
                            continue;
                        }
                        let e = match &b {
                            Some(b) => rel_add(&rest, b, k),
                            None => {
                                let mut t = chosen.clone();
                                t[pos] = k;
                                rel(&t)
                            }
                        };
                        if e < cur * (1.0 - 1e-6) {
                            cur = e;
                            chosen[pos] = k;
                            improved = true;
                        }
                    }
                }
                if !improved || cur < 1e-9 {
                    break;
                }
            }
            if cur < 1e-9 && exact_qr(&chosen) {
                let mut i = 0;
                while i < chosen.len() {
                    let mut trial = chosen.clone();
                    trial.remove(i);
                    if !trial.is_empty() && exact_qr(&trial) {
                        chosen = trial;
                    } else {
                        i += 1;
                    }
                }
                if chosen.len() < min_terms {
                    break;
                }
                return Some(chosen);
            }
            if chosen.len() >= max_terms {
                break;
            }
            let mut best = (f64::INFINITY, None);
            let b = if chosen.len() < 16 {
                base(&chosen)
            } else {
                None
            };
            for k in 0..na {
                if !ok(k) || chosen.contains(&k) {
                    continue;
                }
                let e = match &b {
                    Some(b) => rel_add(&chosen, b, k),
                    None => {
                        let mut t = chosen.clone();
                        t.push(k);
                        rel(&t)
                    }
                };
                if e < best.0 {
                    best = (e, Some(k));
                }
            }
            let Some(k) = best.1 else { break };
            chosen.push(k);
            cur = best.0;
        }
    }
    None
}
