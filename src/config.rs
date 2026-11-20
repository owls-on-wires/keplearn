//! config: single source of truth for every tunable constant in the engine:
//! search-shape budgets, confirm/exit criteria, fit-quality gates, the structural
//! pre-signal thresholds, and the arity budget law. Referenced crate-wide via
//! `use crate::*`. Values are generous defaults; correctness does not depend on
//! their exact settings.

// ── Search shape / budgets ──────────────────────────────────────────────────

/// Maximum reduction-chain depth (root = depth 0).
pub(crate) const MAX_DEPTH: usize = 4;
/// Per-node frontier beam width for depths ≥1 (index d gives the cap at depth d;
/// index 0 is UNUSED; the root beam is DATA-KEYED via engine::root_beam(n_cols), the
/// grammar's exact single-step branching factor, so the root never discards a
/// candidate). Deeper levels narrow monotonically (32→16→8): a depth-d node has
/// O(branching^d) siblings, so a per-node beam that halves with depth holds total
/// frontier work roughly flat while still tunnelling the highest-priority chains.
pub(crate) const BEAM_BY_DEPTH: [usize; MAX_DEPTH] = [0, 32, 16, 8];
/// Per-depth cap on nodes actually expanded (index = depth). depth 0 = the single
/// root; depth 1 is DATA-KEYED to engine::root_beam(n_cols) (expand every root child
/// once; the eval budget that used to bound this was removed, so this is now the
/// load-bearing shallow-work budget); depths ≥2 narrow (250→120) as documented
/// latency ceilings concentrating work on the shallow, high-value reductions.
pub(crate) const DEPTH_CAPS: [usize; MAX_DEPTH + 1] = [1, 0, 400, 250, 120];
/// Pareto-by-complexity harvest tiers (indexed by full-model node count).
pub(crate) const FRONT_TIERS: usize = 64;
/// Max materialized virtual columns available to any one subtree.
pub(crate) const MAX_VCOLS: usize = 8;
/// Additive-term budget for the root monomial harvest is DATA-KEYED via
/// engine::mono_terms(n_cols) = n_cols.clamp(2, MONO_TERM_CAP): an additive
/// decomposition has at most one monomial per independent variable direction, so the
/// term count follows arity rather than the corpus's incidental k=3. MONO_TERM_CAP
/// bounds the k-monomial joint-LS search cost.
pub(crate) const MONO_TERM_CAP: usize = 4;
/// Cap on trace `harvest` decision events.
pub(crate) const HARVEST_EV_CAP: u64 = 500;
/// Searched exponents for the general pow(composite, k) divide-factor (over the
/// sub/div composites): the grammar's core exponents (`Q::CORE`: ±½, ±1, ±2)
/// other than 1, which is the composite itself (0.1 used ½, 2, 3, −1).
pub(crate) const POW_K_SET: &[f64] = &[-2.0, -1.0, -0.5, 0.5, 2.0];

// ── Confirm / exit criteria ─────────────────────────────────────────────────

/// Immediate stop only on a numerically-exact holdout-confirmed candidate. This is
/// the SOLE early-stop; near-exact (NEAR_EXIT_R2) hits do not stop the search (the
/// former eval-counted confirmation window was removed in the item-6 audit; it was
/// gated on a per-expression eval count that the pure-operator engine never
/// increments, so it never fired). Otherwise the search runs to frontier exhaustion.
pub(crate) const EXACT_EXIT_R2: f64 = 1.0 - 1e-9;
/// Near-exact band: gates candidate acceptance / best_confirmed updates (it no longer
/// arms a confirmation window; see EXACT_EXIT_R2).
pub(crate) const NEAR_EXIT_R2: f64 = 0.9999;
/// Terminal quick-probe promotion: a child near-exactly matching a derived probe
/// vector jumps the tunnel queue (its single-input gain scores ~0 by construction).
pub(crate) const TERMINAL_PROBE_R2: f64 = 0.9995;
/// Global cap on Materialize (augmented-input) expansions per dataset. Size-INVARIANT
/// cost discipline: an augmented search costs ~10x a base one, so their COUNT is
/// fixed (root Mat pin + one verified deeper mono-Mat); per-Mat work still scales with
/// arity/rows via the beam and depth caps.
pub(crate) const MAT_EXPAND_CAP: usize = 2;

// ── Priority planes (tunnel-heap primary key) ────────────────────────────────
// The tunnel heap orders by (plane DESC, within-plane priority DESC, …). Planes
// replace the former magic-number priority BANDS (root 1e12, root-Mat 1e11,
// mono-Mat 5e10, near-exact PROMOTE_BAND 5.0) with an explicit, collision-free
// tier. The old bands worked only because the constants were spaced far enough
// apart to never overlap the gain/cost sweep floor (≤~1); an integer plane makes
// that separation structural instead of dependent on numeric luck (a strongly
// negative path-max probe r² could, in principle, inflate a non-promoted gain/cost
// priority past 5.0 and collide with the near-exact band). Within a plane the
// float priority still ranks candidates (score for promotions, gain/cost for the
// sweep, a constant for the pinned Mats so they fall through to depth/seq order).
pub(crate) const PLANE_NORMAL: u8 = 0; // gain/cost sweep + tunnel (default)
pub(crate) const PLANE_NEAREXACT: u8 = 1; // near-exact linearization promotion
pub(crate) const PLANE_MAT_MONO: u8 = 2; // verified two/k-monomial Mat pin
pub(crate) const PLANE_MAT_ROOT: u8 = 3; // root augmented-search Mat pin
pub(crate) const PLANE_ROOT: u8 = 4; // the root node itself

// ── Peel / monomial eligibility ─────────────────────────────────────────────

/// Min r2 a rounded monomial must explain to be kept as a virtual column.
pub(crate) const MONO_MIN_R2: f64 = 0.30;

// ── Integer-exponent monomial policy (F1 consolidation) ──────────────────────

/// Bounds for rounding a real exponent vector to an integer monomial: clamp each
/// exponent's magnitude to ±`max_abs_exp` and reject a total degree above
/// `deg_cap`. The engine historically inlined THREE distinct (clamp, deg_cap)
/// settings for the same kind of object across the monomial machinery; they are
/// single-sourced here (analysis finding F1). Per-site VALUES ARE UNCHANGED; this
/// only names the bounds and removes the scattered literals.
pub(crate) struct MonoPolicy {
    pub(crate) max_abs_exp: i32,
    pub(crate) deg_cap: i32,
}
/// Materialized virtual-column monomials (monomial_vc_from_sol / _col_from_sol /
/// _gexpr_expanded) and the k-monomial full-arity nudge refinement.
pub(crate) const MONO_VCOL: MonoPolicy = MonoPolicy {
    max_abs_exp: 6,
    deg_cap: 16,
};
/// Root additive-harvest + the k-monomial log-LS exponent fits.
pub(crate) const MONO_HARVEST: MonoPolicy = MonoPolicy {
    max_abs_exp: 4,
    deg_cap: 10,
};

/// Round a real exponent to the nearest integer, clamped to the policy magnitude;
/// non-finite → 0. `f64::NAN as i32` is already 0 in Rust, so this is identical to
/// the prior `(e.round() as i32).clamp(-N, N)` (with or without an is_finite guard)
/// at every call site.
#[inline]
pub(crate) fn round_clamp_exp(e: f64, p: &MonoPolicy) -> i32 {
    if e.is_finite() {
        (e.round() as i32).clamp(-p.max_abs_exp, p.max_abs_exp)
    } else {
        0
    }
}

/// Engine-wide validity floor: a derived/materialized column is usable only if at
/// least ~60% of its rows are finite. Returned as an integer row count using
/// `n*6/10` integer arithmetic (NOT `n as f64 * 0.6`, which rounds differently for
/// small n), so it is byte-identical to the prior scattered `n * 6 / 10` literal at
/// every call site.
#[inline]
pub(crate) fn min_finite_rows(n: usize) -> usize {
    n * 6 / 10
}

// ── Declared snap constants ──────────────────────────────────────────────────
// The SINGLE legible table of constants the engine snaps fitted literals to. Every
// snap site (math::snap_value) reads this; no inline constant literals elsewhere.
// snap_value tries a rational multiple p/q of EACH base here (q ∈ SNAP_DENOMS), so
// e.g. 2π, 4π, π/2 fall out of the "pi" base and 1/(4π), 1/(2π) out of "1/pi"; only
// bases that are NOT rational multiples of each other need their own row. This is a
// DECLARED, tunable set: add a constant here to let the engine reach target equations
// whose closed form uses it (holdout-verified per snap, so additions can't over-snap).
pub(crate) const CONSTANTS: &[(&str, f64)] = &[
    ("1", 1.0),                                            // plain simple rationals
    ("pi", std::f64::consts::PI),                          // angles, 2π/4π/π-half via p/q
    ("1/pi", 1.0 / std::f64::consts::PI),                  // 1/(4π), 1/(2π) EM prefactors
    ("pi^2", std::f64::consts::PI * std::f64::consts::PI), // radiation, II_24_17
    ("e", std::f64::consts::E),                            // Euler's number
    ("sqrt2", std::f64::consts::SQRT_2),                   // rms / geometry
    ("sqrt3", 1.732_050_807_568_877_2),                    // hexagonal / geometry
    ("1/sqrt(2pi)", 0.398_942_280_401_432_7),              // Gaussian normalisation
    ("sqrt(2pi)", 2.506_628_274_631_000_5),                // Gaussian / Stirling
    ("ln2", std::f64::consts::LN_2),                       // half-lives
    ("1/ln2", std::f64::consts::LOG2_E),                   // bits per nat
    ("ln10", std::f64::consts::LN_10),                     // decibels, decades
    ("euler_gamma", 0.577_215_664_901_532_9),              // harmonic sums
    ("golden_ratio", 1.618_033_988_749_895),               // Fibonacci growth
];
/// CONSTANTS bases snapped only as 1, 2 or 1/2 times themselves (as
/// forms::nf's few-multiplier families): their p/q multiples would cover
/// the number line.
pub(crate) const FEW_MULT_BASES: &[f64] = &[
    0.398_942_280_401_432_7,
    2.506_628_274_631_000_5,
    std::f64::consts::LN_2,
    std::f64::consts::LOG2_E,
    std::f64::consts::LN_10,
    0.577_215_664_901_532_9,
    1.618_033_988_749_895,
];
/// Denominators tried for the rational multiple p/q of each CONSTANTS base.
pub(crate) const SNAP_DENOMS: &[u32] = &[1, 2, 3, 4, 6, 8, 12];

/// Monomial exponent snapping: each fitted log-linear exponent βᵢ snaps to the
/// nearest of the grammar's exponents (`Q::ALL`) or 0 (math::snap_exp; 0.1
/// snapped to p/q with q ≤ 4). EXP_SNAP_TOL is
/// the max |βᵢ − q| for the fit to count as clean.
pub(crate) const EXP_SNAP_TOL: f64 = 0.06;

// ── Structural pre-signals (self-selecting capability deployment) ────────────

// ── Fit-quality gates (composite-bake / clean-monomial / affine) ─────────────
// Behavior-preserving relocation of formerly-inline gates (configurability pass).
// Values are UNCHANGED from their prior scattered literals.

/// affine_trivial: an affine fit within these of identity (scale≈1, offset≈0) is
/// treated as no-op, so baking it in would only add noise-induced constants.
pub(crate) const AFFINE_SCALE_TOL: f64 = 0.01;
pub(crate) const AFFINE_OFFSET_TOL: f64 = 0.05;

// ── Additive constant-offset grid (const_offsets_fit) ────────────────────────
/// Fractional multipliers of min(target) used as candidate additive offsets before
/// the golden-section refine (the retired monomial_const_offsets grid).
pub(crate) const OFFSET_FRAC_GRID: &[f64] = &[0.5, 0.9, 0.99, 0.999];
/// Coarse-grid step count bracketing the offset golden-section search.
pub(crate) const OFFSET_COARSE_STEPS: usize = 6;

// ── Fitted positive-constant search (fit_shell_const) ────────────────────────
/// 1-2-5 decade coarse sweep for a positive fitted constant: mantissas × 10^e for
/// e in CONST_DECADE_EXP.0..=CONST_DECADE_EXP.1.
pub(crate) const CONST_DECADE_MANTISSA: &[f64] = &[1.0, 2.0, 5.0];
pub(crate) const CONST_DECADE_EXP: (i32, i32) = (-3, 3);
/// Golden-section refine: half-width of the log10 bracket and the iteration count
/// (the iteration count is shared with the offset-fit refine).
pub(crate) const GOLDEN_BRACKET: f64 = 0.5;
pub(crate) const GOLDEN_ITERS: usize = 8;
/// Near-tie snap of the fitted constant to a simple value: the relative band and the
/// score-slack under which the snap is accepted.
pub(crate) const NICE_SNAP_BAND: f64 = 0.02;
pub(crate) const NICE_TIE_SLACK: f64 = 1e-6;

// ── k-monomial joint-LS harvest gates (harvest_k_monomial) ───────────────────

// ── Monomial / pair-column emission floors ───────────────────────────────────
/// mono_fit: minimum arity (number of active inputs) for the log-linear power-law
/// terminal to run. Lowered 2 → 1 so single-variable monomials are reachable (e.g.
/// I_6_2a's Gaussian, where the residual after the exp/log strip is a 1-var power).
pub(crate) const MONO_FIT_MIN_ARITY: usize = 1;
/// mono_fit: minimum r² for a rounded-exponent monomial to be emitted as a finalist.
pub(crate) const MONO_FIT_MIN_R2: f64 = 0.995;
/// Materialized pair-column (mat_pair_ops) minimum |r| to keep as a virtual column.
pub(crate) const PAIR_VCOL_MIN_R2: f64 = 0.15;

// ── invert-off-target progress / commit gates ────────────────────────────────
/// Near-exact commit r² for a target-side shell / harvest (harvest_la already exact).
pub(crate) const IO_NEAREXACT_R2: f64 = 0.9999;
/// Progress margin: a spawned shell must raise harvest_la by at least this to spawn.
pub(crate) const SHELL_PROGRESS_MARGIN: f64 = 0.02;
/// Divide progress epsilon: a divide must raise harvest_cheap by at least this to be
/// ordered ahead of the non-progress tail within its plane.
pub(crate) const DIVIDE_PROGRESS_EPS: f64 = 0.005;
/// Additive-factor divide: min monomial log-linear r² to promote to the near-exact plane.
pub(crate) const ADDITIVE_PROMO_R2: f64 = 0.999;

// ── Emission-time affine recalibration (driver output stage) ─────────────────

// ── Constant snapping (math::snap_value / snap_model_constants) ───────────────
/// Reject snapping a fitted constant whose magnitude is at/above this (out of range).
pub(crate) const SNAP_MAX_MAG: f64 = 1e6;
/// Max |numerator| and |denominator| of a simple rational multiple p/q of a base.
pub(crate) const SNAP_MULT_MAX: f64 = 8.0;
/// snap tolerance = (SNAP_TOL_REL·av).max(SNAP_TOL_ABS).min(SNAP_TOL_GRIDFRAC·base/q).
pub(crate) const SNAP_TOL_REL: f64 = 0.006;
pub(crate) const SNAP_TOL_ABS: f64 = 1e-3;
pub(crate) const SNAP_TOL_GRIDFRAC: f64 = 0.35;

// ── Dataset holdout split / output policy (io.rs) ─────────────────────────────
/// Fixed holdout-shuffle seed. The internal 75/25 MDL-holdout split ALWAYS uses this
/// constant, so final selection is fully deterministic and the CLI `--seed` is a benign
/// no-op; SRBench's random_state cannot perturb which model is chosen. (Combined with
/// the leverage-trimmed holdout scorer in math.rs, near-exact models with measure-zero
/// poles score ~1.0 on any split, so no seed can displace the true form.) Value is the
/// golden-ratio odd constant that was the former per-run default.
pub(crate) const HOLDOUT_SHUFFLE_SEED: u64 = 0x9E3779B97F4A7C15;
/// The holdout takes 1/FRAC_DEN of the rows, at least HOLDOUT_MIN_ACTIVE and at
/// most HOLDOUT_MAX of them; below HOLDOUT_MIN_ROWS (the row count at which a
/// quarter reaches HOLDOUT_MIN_ACTIVE) there is no holdout, every row is a
/// training row, and answers are scored by leave-one-out. (0.1 set a quarter
/// of the rows aside from 40 rows but scored on them only from 80, so tables
/// of 40 to 79 rows lost a quarter of their rows.)
pub(crate) const HOLDOUT_FRAC_DEN: usize = 4;
pub(crate) const HOLDOUT_MIN_ACTIVE: usize = 20;
pub(crate) const HOLDOUT_MIN_ROWS: usize = HOLDOUT_FRAC_DEN * HOLDOUT_MIN_ACTIVE;
pub(crate) const HOLDOUT_MAX: usize = 2000;
/// Holdout magnitude-outlier trim: only trim when ≥ this many finite holdout rows,
/// trimming the top 1/TRIM_DEN of magnitudes (min TRIM_MIN rows).
pub(crate) const OUTLIER_TRIM_MIN_M: usize = 20;
/// leverage_trimmed_r2 pole gate: a holdout row is dropped from the MDL score only if
/// its residual exceeds POLE_RESID_MULT × the median absolute residual (i.e. a real
/// measure-zero pole spike, e.g. 1/(a−b) at a≈b), capped at the top 1/OUTLIER_TRIM_DEN.
/// Smooth approximants have no such rows, so the trim is a NO-OP for them: it removes
/// the seed-fragile pole artifact for true forms without boosting approximants into
/// displacing them.
pub(crate) const POLE_RESID_MULT: f64 = 50.0;
pub(crate) const OUTLIER_TRIM_DEN: usize = 50;
pub(crate) const OUTLIER_TRIM_MIN: usize = 2;
/// Holdout coverage requirement: a candidate must explain ≥ NUM/DEN of kept holdout
/// rows (min COVER_MIN) or it is unscored (harness-parity 90% rule).
pub(crate) const HOLDOUT_COVER_NUM: usize = 9;
pub(crate) const HOLDOUT_COVER_DEN: usize = 10;
pub(crate) const HOLDOUT_COVER_MIN: usize = 3;
/// --dump-candidates: max candidates written to the debug dump.
pub(crate) const DUMP_CANDIDATES_CAP: usize = 200;

/// Divide-by-near-zero guards for the binary reduction operators: raw features mask
/// |b| below DIV_GUARD_RAW, materialized virtual columns below DIV_GUARD_MAT (the one
/// distinction between the pair and mat operator registries).
pub(crate) const DIV_GUARD_RAW: f64 = 1e-10;
pub(crate) const DIV_GUARD_MAT: f64 = 1e-30;

// ── Autonomous-loop sweep knobs (behavior-preserving extraction) ──────────────
// KNOB-class literals surfaced by the engine audit so the loop can sweep them.
// Values are UNCHANGED from their prior inline literals; a pure relocation, no
// behavior change. (Distinct from the INVARIANT structural literals in the NOTE
// below, which stay inline.)

/// Additive slack on the grammar-derived root beam width (budget::root_beam).
pub(crate) const ROOT_BEAM_SLACK: usize = 4;
/// Per-depth beam-narrowing shift multiplier (budget::beam_at shifts the root beam
/// right by BEAM_DEPTH_SHIFT * depth). The kept set stays a superset of the flat
/// BEAM_BY_DEPTH floor regardless.
pub(crate) const BEAM_DEPTH_SHIFT: usize = 2;
/// Extra deflation rounds beyond the term budget in the root additive-vcol harvest
/// (harvest_monomial_vcols iterates max_terms + PURSUIT_EXTRA_ROUNDS times).
pub(crate) const PURSUIT_EXTRA_ROUNDS: usize = 3;
/// Min per-term degree emitted by the root additive-vcol harvest (deg >= this).
/// COUPLED to KMONO_KCOL_MIN_DEG; both gate additive-monomial degree; if you sweep
/// one, consider the other. Kept at its current value here.
pub(crate) const HARVEST_VCOL_MIN_DEG: i32 = 3;
/// Max nonzero exponents kept in a mono_fit monomial (rejects overly dense fits).
pub(crate) const MONO_FIT_EXP_CAP: usize = 8;
/// Max valid rows a masked-divide residual may retain and still spawn a divide child
/// (search caps min_finite_rows at this before gating).
pub(crate) const DIV_MIN_VALID_ROWS_CAP: usize = 50;
/// Rows a fit of `p` parameters needs: `p` plus the surplus that could
/// contradict it (`crate::forms::fit::fit_min_rows`; 0.1 used fixed minimums
/// of 8, 16 and 24 rows).
pub(crate) fn fit_min_rows(p: usize) -> usize {
    crate::forms::fit::fit_min_rows(p)
}

/// Rounding floor (relative to the target's variance) below which data count
/// as unrounded: 12-digit cells give about 1e-24.
pub(crate) const NOISE_FLOOR_MIN: f64 = 1e-18;
/// Finalists whose rounding floor is computed (those with a fit r² within
/// FLOOR_FIT_SLACK of the best, at most FLOOR_FINALISTS); the smallest is used.
pub(crate) const FLOOR_FINALISTS: usize = 30;
pub(crate) const FLOOR_FIT_SLACK: f64 = 0.01;
/// Best fit r² from which a rounding floor is estimated at all.
pub(crate) const FLOOR_MIN_FIT: f64 = 0.999;
/// A fit counts as exact when its unexplained variance is at most this many
/// rounding floors (measured on tables rounded to 3 and 4 significant digits).
pub(crate) const NOISE_FLOOR_K: f64 = 3.0;

/// Standard errors within which a fitted constant snaps to a simple value
/// on a fit short of exact.
pub(crate) const SNAP_Z: f64 = 1.0;

/// Share of the training rows a finalist must be defined on to be scored by
/// leave-one-out (below HOLDOUT_MIN_ROWS; as HOLDOUT_COVER_* above it).
pub(crate) const LOO_COVER: f64 = 0.9;

/// Rows from which the constant fitter scores a fit on its own seeded split.
pub(crate) const FIT_SPLIT_MIN_ROWS: usize = 40;

/// The fitting and scoring rows of the constant fitter for `n` rows: its
/// seeded split, or every row for both below FIT_SPLIT_MIN_ROWS, where the
/// split's quarter (two rows at eight) cannot score a fit.
pub(crate) fn fit_split(n: usize) -> (Vec<usize>, Vec<usize>) {
    if n < FIT_SPLIT_MIN_ROWS {
        ((0..n).collect(), (0..n).collect())
    } else {
        crate::forms::fit::split_indices(n, 1)
    }
}

// ── Input limits (driver ingest) ─────────────────────────────────────────────
/// Stdin byte cap: a runaway or absurdly large pipe errors cleanly instead of
/// exhausting memory. 1 GiB covers every corpus in scope by a wide margin.
pub(crate) const INPUT_BYTE_CAP: u64 = 1 << 30;

/// Basis-terminal calls allowed at depth 1 per run (after the first shell
/// or division), in place of 0.1's 2-term
/// monomial fit there; a basis pursuit costs far more than that fit, so the
/// first nodes reached in priority order get it.
pub(crate) const BASIS_DEPTH1_CAP: usize = 32;
/// Argument range (absolute for exp, sin, cos; relative otherwise) below
/// which a unary or group of an exact finalist counts as near its linear
/// regime, so the finalist bounds the search instead of ending it
/// (guard::exit_check). Wider than the search's 3e-2: over a range of 0.5
/// sin's cubic term is a few percent, and a pair of such unaries can cancel
/// it (sin(u) + arcsin(u) ≈ 2u).
pub(crate) const EXIT_DEGENERATE_REL: f64 = 0.5;
/// Depth-1 basis nodes whose basis terminal also screens the product
/// library (the highest ranked of the BASIS_DEPTH1_CAP; the root always
/// does): all of them.
pub(crate) const PRODUCT_DEPTH1_CAP: usize = BASIS_DEPTH1_CAP;

/// structural constants (guard::structural_constant): rationals p/q with
/// |p| and q at most this
pub(crate) const STRUCT_RATIONAL_MAX: f64 = 12.0;
/// the per-row precision check (guard::fits_each_row) applies to a target
/// of one sign whose largest magnitude is at least this times its smallest
pub(crate) const REL_SPAN: f64 = 1e3;
/// the relative error each row is allowed beyond its rounding variance
/// (full-precision cells: a true form fits to near machine precision)
pub(crate) const REL_EXACT_EPS: f64 = 1e-8;
/// conditioning (guard::well_conditioned): a term below this fraction of
/// its sum is negligible; terms whose magnitudes exceed their sum by this
/// factor cancel
pub(crate) const COND_NEGLIGIBLE: f64 = 1e-9;
pub(crate) const COND_CANCEL_MAX: f64 = 1e6;

/// the share of a --timeout budget the search may use; the rest is for
/// the answers' assembly (floor, canonical stage, scoring)
pub(crate) const SEARCH_SHARE: f64 = 0.8;

/// Rows below which nothing is searched: a law with one constant is fitted
/// with an offset (fit_min_rows(2) rows) and checked by leaving each row out,
/// one row more.
pub(crate) fn min_law_rows() -> usize {
    fit_min_rows(2) + 1
}

/// Magnitude beyond which a value's square overflows f64 (ingest warns).
pub(crate) const OVERFLOW_ABS: f64 = 1e150;

/// Candidates given a verdict before the answers are ordered by verdict:
/// this many times --top-k (distinct structures, in search order).
pub(crate) const VERDICT_POOL: usize = 3;

/// Rounding's error bound per row (guard::within_rounding): a row may miss
/// by this many times its propagated half-quanta (the derivative is
/// linearized and numerical) ...
pub(crate) const ROUND_BOUND_SLACK: f64 = 1.5;
/// ... plus this many standard deviations of the target, the per-row
/// counterpart of an exact r² of 1 − 1e-9 (whose rms residual is 3e-5 of
/// one standard deviation) on full-precision cells.
pub(crate) const ROUND_BOUND_FLOOR: f64 = 3e-4;

/// The features are read as exact settings when the rounding floor with
/// their written precision exceeds the target's own by this factor (the
/// target is written about ten times more precisely in standard deviation).
pub(crate) const FEATURE_EXACT_RATIO: f64 = 100.0;

/// Depth-1 offset shells (log(y − b) and the like) given the basis
/// terminal, on slots apart from BASIS_DEPTH1_CAP's.
pub(crate) const OFFSET_BASIS_CAP: usize = 8;

/// Feature columns read as exact settings must be written mostly with at
/// most this many significant digits (0.25, 1.5, 120): a column of
/// six-digit values is a set of measurements.
pub(crate) const SETTING_MAX_DIGITS: usize = 3;
/// The least share of the target's variance a law explains (its r² on
/// every row): below it the answer is reported as the best fit, not a law.
pub(crate) const LAW_MIN_R2: f64 = 0.9;
/// A common physical constant (pi, 2*pi, e, ...) replaces a fitted value
/// when the unexplained variance rises by at most this many times the
/// residual variance per degree of freedom: three standard errors (these
/// constants are common enough in laws to be preferred on weak evidence).
pub(crate) const COMMON_Z2: f64 = 9.0;
/// A constant is written by name only when the data pin it to this
/// relative standard error (`guard::clean_answer`).
pub(crate) const NAME_MAX_REL_SE: f64 = 0.01;
/// A law fits reasonably well when its r² is at least LAW_MIN_R2, or, for a
/// target of one sign, its median miss is at most LAW_MAX_MEDIAN_MISS of
/// the target's value.
pub(crate) const LAW_MAX_MEDIAN_MISS: f64 = 0.05;
/// A law may miss at most max(1, LAW_BAD_ROWS of the rows) by more than
/// LAW_BAD_MISS of the target's value (beyond the cell's quantum), when
/// the target keeps one sign.
pub(crate) const LAW_BAD_MISS: f64 = 0.25;
pub(crate) const LAW_BAD_ROWS: f64 = 0.05;
/// Laws short of the cells' precision are ranked with misses under this
/// fraction of the target's value not counted (`guard::practical_dl`).
pub(crate) const RANK_NOISE_FLOOR: f64 = 0.005;
/// The simpler law the summary shows under the first answer: fewer
/// constants and an r² of at least this.
pub(crate) const SIMPLER_MIN_R2: f64 = 0.98;
/// An answer's fitted constant within this fraction of a simple value (one
/// or two significant digits, or a small fraction) is written as that
/// value, and an offset under this fraction of the target's mean size is
/// dropped (`guard::clean_answer`).
pub(crate) const CLEAN_REL: f64 = 1e-3;
/// An answer within the cells' precision is not cleaned when the cleaned
/// form's rms miss would be over this many times the target's written
/// precision (its cells' median relative quantum).
pub(crate) const CLEAN_PRECISION_K: f64 = 10.0;
/// The clean answer is refused where its unexplained variance is over
/// CLEAN_LOSS_MAX times the fit's and over CLEAN_LOSS_ABS of the target's
/// variance: rounded, it would no longer fit reasonably.
pub(crate) const CLEAN_LOSS_MAX: f64 = 10.0;
pub(crate) const CLEAN_LOSS_ABS: f64 = 1e-6;
/// A design grid's distinct values lie at least this many written quanta
/// apart (median gap); densely drawn readings lie one quantum apart.
pub(crate) const GRID_MIN_GAP: f64 = 2.0;

/// A unary or square root whose output varies by less than this fraction of
/// its mean over the rows is saturated: it acts as a constant
/// (guard::saturated_nodes).
pub(crate) const SATURATION_REL: f64 = 0.02;

/// Feature columns above which the input gets a warning: the search's first
/// step grows with the column pairs, and wider tables are searched only in
/// part within a budget.
pub(crate) const WIDE_COLUMNS: usize = 20;

/// Feature columns the search uses at most: a wider table is searched on the
/// SCREEN_COLUMNS columns most related to the target one at a time
/// (driver::screen_columns). The search's first step (one-column and pair
/// divisions, each scored against every column) grows with the cube of the
/// columns; measured on 50- and 200-row tables, it completes in 7 s at 30
/// columns (within the search's 8 s of a 10 s budget) and not within 8 s at
/// 45, where no law was found, and at 10,000 columns it took 17 GB before
/// its first division was scored.
pub(crate) const SCREEN_COLUMNS: usize = 30;
