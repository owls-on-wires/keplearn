use crate::*;

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

thread_local! {
    static DEADLINE : std::cell::Cell < Option < Instant >> = const {
    std::cell::Cell::new(None) };
}

pub(crate) fn arm_deadline_at(d: Option<Instant>) {
    DEADLINE.with(|c| c.set(d));
}

#[inline]
pub(crate) fn out_of_budget() -> bool {
    crate::heap::over_soft()
        || DEADLINE
            .with(|c| c.get())
            .is_some_and(|d| Instant::now() >= d)
}
