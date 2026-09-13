use crate::*;

fn binom(n: usize, k: usize) -> usize {
    if k > n {
        return 0;
    }
    let k = k.min(n - k);
    let mut r = 1usize;
    for i in 0..k {
        r = r * (n - i) / (i + 1);
    }
    r
}

pub(crate) fn root_beam(n_cols: usize) -> usize {
    let n_unary = unary_factor_ops().len();
    let n_pair: usize = pair_factor_ops()
        .iter()
        .map(|&(_, _, _, swap)| if swap { 2 } else { 1 })
        .sum();
    let n_pow = 2 * 2 * POW_K_SET.len();
    let n_shells = shells().len();
    let pairs = n_cols * n_cols.saturating_sub(1) / 2;
    n_cols * n_unary + pairs * (n_pair + n_pow) + n_shells + ROOT_BEAM_SLACK
}

pub(crate) fn beam_at(depth: usize, n_cols: usize) -> usize {
    if depth == 0 {
        return root_beam(n_cols);
    }
    let flat = BEAM_BY_DEPTH[depth.min(MAX_DEPTH - 1)];
    flat.max(root_beam(n_cols) >> (BEAM_DEPTH_SHIFT * depth).min(24))
}

pub(crate) fn depth_cap_at(depth: usize, n_cols: usize) -> usize {
    match depth {
        0 => DEPTH_CAPS[0],
        1 => root_beam(n_cols),
        d => {
            let dd = d.min(MAX_DEPTH);
            DEPTH_CAPS[dd].max(root_beam(n_cols) >> (dd - 1).min(24))
        }
    }
}

pub(crate) fn mono_terms(n_cols: usize) -> usize {
    n_cols.clamp(2, MONO_TERM_CAP)
}

pub(crate) fn norm_subset_budget(n_cols: usize) -> usize {
    let maxsz = NORM_MAX_ACTIVE.min(n_cols);
    let total: usize = (2..=maxsz).map(|s| binom(n_cols, s)).sum();
    total.min(NORM_SUBSET_CAP)
}

static DEADLINE_MS: AtomicU64 = AtomicU64::new(0);

static CLOCK_START: OnceLock<Instant> = OnceLock::new();

pub(crate) fn arm_deadline(timeout_ms: u64) {
    let start = *CLOCK_START.get_or_init(Instant::now);
    if timeout_ms > 0 {
        let now_ms = start.elapsed().as_millis() as u64;
        DEADLINE_MS.store(now_ms + timeout_ms, AtomicOrdering::Relaxed);
    } else {
        DEADLINE_MS.store(0, AtomicOrdering::Relaxed);
    }
}

#[inline]
pub(crate) fn out_of_time() -> bool {
    let d = DEADLINE_MS.load(AtomicOrdering::Relaxed);
    if d == 0 {
        return false;
    }
    match CLOCK_START.get() {
        Some(s) => s.elapsed().as_millis() as u64 >= d,
        None => false,
    }
}
