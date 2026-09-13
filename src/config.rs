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
/// Safety ceiling on disjoint-pair partitions scanned per difference-sum firing.
/// disjoint_pair_partitions emits partitions deterministically fewest-pairs-first, so
/// this ceiling (large; only bites at extreme arity) can only ever drop the
/// highest-pair-count partitions, never the low-arity partition a target needs.
pub(crate) const SUMSQ_VISIT_CAP: usize = 3000;
/// General norm-combiner composite-V generator (roadmap #2): max column-subset size
/// for Σx² / sqrt(Σx²) norms. Corpus-max + headroom: physical norms combine ≤4
/// coordinates; a searched ceiling on subset size, not a menu.
pub(crate) const NORM_MAX_ACTIVE: usize = 4;
/// Safety ceiling on norm column-subsets enumerated per firing. The subset budget is
/// DATA-KEYED via engine::norm_subset_budget(n_cols) to cover EVERY subset of size
/// 2..=NORM_MAX_ACTIVE (Σ C(n_cols,s)); this const is only the hard ceiling guarding
/// pathological arity. Subsets are emitted size-ascending, so if the ceiling ever
/// binds it drops the largest subsets first, never the small one a target needs.
pub(crate) const NORM_SUBSET_CAP: usize = 300;
/// Trig-composite argument grammar cap (E7 unified generator): max candidate
/// oscillatory arguments u (single columns, products, column·difference) enumerated
/// per firing before the trig shapes (sin/cos^p, sinc²) are applied.
pub(crate) const TRIG_ARG_CAP: usize = 96;
/// P2: trig-shape parameters are SEARCHED ranges, not hand-picked menus. The power p
/// sweeps the contiguous integer range 1..=TRIG_POW_MAX (was the tuned menu {2,4});
/// the argument scale s sweeps the subharmonic set 1/n for n in 1..=TRIG_SCALE_HARM
/// (was the tuned pair {1, ½}). composite_bake's near-exact r² gate selects which
/// (s,p) the data actually supports, so the parameter is fit-derived, not corpus-set.
/// Both ceilings are "corpus-max + headroom": p≤4 covers sin²/sin⁴ and adds odd
/// powers; the 3rd subharmonic covers half-angle (½) plus one beyond. General
/// searched-range bounds, widenable without touching the mechanism.
pub(crate) const TRIG_POW_MAX: i32 = 4;
pub(crate) const TRIG_SCALE_HARM: usize = 3;
/// Phase 2A: searched exponents for the general pow(composite, k) divide-factor
/// (over the sub/div composites). k=2 reproduces the former dedicated subsq=(a−b)²
/// and sqdiv=(a/b)² ops; 0.5 and 3 make sqrt-of-composite and cubes equally reachable
/// (a searched power set rather than the square alone). k=-1 adds the
/// reciprocal-difference factor 1/(a−b) (and 1/(a/b) = b/a): a deliberate config
/// improvement that makes forms like Feynman II_21_32 and II_11_3 reachable (they
/// solve at every seed once the holdout scorer is leverage-trimmed; see math.rs).
pub(crate) const POW_K_SET: &[f64] = &[0.5, 2.0, 3.0, -1.0];
/// P2: difference-sum norm partition counts are a SEARCHED range 1..=NORM_PARTITION_MAX
/// (capped by n_cols/2), not the tuned menu {3,2}. P=1 adds the single squared
/// difference; the composite_bake gate selects the partition the data supports.
/// Corpus-max + headroom: ≤3 disjoint difference-pairs covers the 3D distance forms.
pub(crate) const NORM_PARTITION_MAX: usize = 3;
/// Force-seed the top-K correlating atoms as guaranteed starts of the k-monomial
/// matching pursuit (so the greedy pursuit cannot miss a strong-but-not-first atom).
/// A general matching-pursuit warm-start count, capped by the term budget
/// engine::mono_terms(n_cols); size-invariant, not corpus-tuned.
pub(crate) const FSEED_TOP: usize = 3;

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
];
/// Denominators tried for the rational multiple p/q of each CONSTANTS base.
pub(crate) const SNAP_DENOMS: &[u32] = &[1, 2, 3, 4, 6, 8, 12];

/// Monomial EXPONENT snap lattice (config-driven; was a hardcoded half-integer grid).
/// The harvest snaps each fitted log-linear exponent βᵢ to the nearest rational p/q
/// with q ∈ EXP_DENOMS. Widened from halves-only {1,2} to include thirds and quarters
/// {1,2,3,4}, so allometric 3/4 / 2/3 and Cobb-Douglas-style rational exponents become
/// reachable in addition to the sqrt-heavy half-integers. Coarse (q≤4) on purpose:
/// exponents are low-complexity, and the downstream near-exact monomial refit rejects
/// any spurious snap. EXP_SNAP_TOL is the max |βᵢ − p/q| for the fit to count as clean.
pub(crate) const EXP_DENOMS: &[u32] = &[1, 2, 3, 4];
pub(crate) const EXP_SNAP_TOL: f64 = 0.06;

// ── Structural pre-signals (self-selecting capability deployment) ────────────

/// structural-invisibility pre-signal: no single-column |corr| above this.
pub(crate) const INVIS_CORR: f64 = 0.5;
/// structural-invisibility pre-signal: no single-monomial log-linear r2 above this.
pub(crate) const INVIS_MONO: f64 = 0.5;
/// residual-near-miss pre-signal band (strong-but-inexact monomial fit).
pub(crate) const NEAR_LO: f64 = 0.99;
pub(crate) const NEAR_HI: f64 = 0.9999;

// ── Fit-quality gates (composite-bake / clean-monomial / affine) ─────────────
// Behavior-preserving relocation of formerly-inline gates (configurability pass).
// Values are UNCHANGED from their prior scattered literals.

/// Near-exact r² a composite-V bake (target/V or target·V fitted as an affine
/// monomial) must reach to emit a finalist. Shared by composite_bake, the log-space
/// r² inside clean_monomial_fit, and trig_clean_bake's reconstruction check.
pub(crate) const COMPOSITE_R2: f64 = 0.9999;
/// clean_monomial_fit: max |eᵢ − round(eᵢ)| for the log-LS exponent vector to count
/// as a TIGHT integer monomial (the M4 discriminator separating a true
/// multiplicative trig-composite from a coincidental affine near-fit).
pub(crate) const CLEAN_MONO_INT_TOL: f64 = 0.05;
/// clean_monomial_fit: reject a recovered clean monomial whose total |degree| exceeds this.
pub(crate) const CLEAN_MONO_DEG_CAP: i32 = 12;
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
/// Convergence r² at/above which the reseed / swap / nudge refinements stop.
pub(crate) const KMONO_CONVERGE_R2: f64 = 0.99999;
/// Coordinate-descent swap and full-arity exponent-nudge sweep caps.
pub(crate) const KMONO_SWAP_SWEEPS: usize = 4;
pub(crate) const KMONO_NUDGE_SWEEPS: usize = 6;
/// Minimum best-r² for the full-arity nudge to engage (a promising but non-exact split).
pub(crate) const KMONO_NUDGE_FLOOR: f64 = 0.5;
/// top-frac first-fit keeps the top NUM/DEN of |residual| rows (min MIN) as the mask.
pub(crate) const KMONO_TOPFRAC_NUM: usize = 6;
pub(crate) const KMONO_TOPFRAC_DEN: usize = 10;
pub(crate) const KMONO_TOPFRAC_MIN: usize = 12;
/// Strict-improvement epsilon for a swap/nudge to be accepted.
pub(crate) const IMPROVE_EPS: f64 = 1e-9;
/// Verified combined r² a k-monomial decomposition must reach to force a finalist.
pub(crate) const KMONO_FINALIST_R2: f64 = 0.999;
/// Minimum per-term degree for a forced k-column joint-LS finalist (search.rs call
/// site). Lowered 3 → 2 to reach three-deg-2-product targets (e.g. I_11_19). NOTE:
/// deg-2 terms are already pairwise-reachable, so a lower floor can spawn spurious
/// k-col finalists on low-arity roots (displacement/regression risk; watch closely).
pub(crate) const KMONO_KCOL_MIN_DEG: i32 = 2;
/// Dense low-degree backward-elimination fallback (conditioning-robust additive
/// harvest): for arity <= KMONO_DENSE_MAX_VARS, fit the full positive-exponent monomial
/// basis up to total degree KMONO_DENSE_DEG, then prune to the exact sparse set. Recovers
/// sign-flipping / near-zero polynomial sums (Strogatz lv1/lv2) the greedy pursuit
/// mis-selects. Kept small so the basis (and its O(basis^2) elimination) stays cheap.
pub(crate) const KMONO_DENSE_MAX_VARS: usize = 3;
pub(crate) const KMONO_DENSE_DEG: i32 = 3;

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
/// affc∘log lookahead: min log-monomial r² to spawn the fitted-constant (affc) node.
pub(crate) const AFFC_LOOKAHEAD_R2: f64 = 0.999;
/// Divide progress epsilon: a divide must raise harvest_cheap by at least this to be
/// ordered ahead of the non-progress tail within its plane.
pub(crate) const DIVIDE_PROGRESS_EPS: f64 = 0.005;
/// Additive-factor divide: min monomial log-linear r² to promote to the near-exact plane.
pub(crate) const ADDITIVE_PROMO_R2: f64 = 0.999;
/// General additive-assembly (generators::gen_additive_finalists): max arity at which the
/// peel-a-term-then-fit-the-residual-with-a-generator pass runs (kept low so the
/// per-first-term generator sweeps stay cheap; the Strogatz/2-var additive forms it
/// targets are low-arity). Reaches heterogeneous sums (monomial + trig / rational).
pub(crate) const ADDITIVE_MAX_VARS: usize = 3;
/// Additive-assembly acceptance: a [T1, V] joint fit is emitted only at this (very tight)
/// combined r². Keeps close-fitting approximations from being credited;
/// only a near-exact heterogeneous sum clears the bar.
pub(crate) const ADDITIVE_ASSEMBLY_R2: f64 = 0.99999;

// ── Emission-time affine recalibration (driver output stage) ─────────────────
/// Minimum plain-R² gain for baking a full-data affine calibration (a·model+b)
/// into an emitted model string. The engine fits and selects models up to
/// affine equivalence; on messy data the literal emitted member of that family
/// can be badly calibrated in original units. Recalibration emits the best
/// member instead when the gain clears this floor, so exact-law outputs
/// (gain ≈ 0, a≈1/b≈0) keep their byte-identical strings.
pub(crate) const RECAL_MIN_GAIN: f64 = 1e-6;

// ── Constant snapping (math::snap_value / snap_model_constants) ───────────────
/// Reject snapping a fitted constant whose magnitude is at/above this (out of range).
pub(crate) const SNAP_MAX_MAG: f64 = 1e6;
/// Max |numerator| and |denominator| of a simple rational multiple p/q of a base.
pub(crate) const SNAP_MULT_MAX: f64 = 8.0;
/// snap tolerance = (SNAP_TOL_REL·av).max(SNAP_TOL_ABS).min(SNAP_TOL_GRIDFRAC·base/q).
pub(crate) const SNAP_TOL_REL: f64 = 0.006;
pub(crate) const SNAP_TOL_ABS: f64 = 1e-3;
pub(crate) const SNAP_TOL_GRIDFRAC: f64 = 0.35;
/// snap_model_constants: a fitted coefficient within this fraction of the max |coef|
/// magnitude is treated as a candidate zero.
pub(crate) const SNAP_ZERO_FRAC: f64 = 0.05;
/// snap_model_constants: holdout r² slack = (1−base_rh)·SNAP_HOLDOUT_SLACK + eps.
pub(crate) const SNAP_HOLDOUT_SLACK: f64 = 0.05;
/// snap_model_constants: reject a model carrying more than this many snapped constants.
pub(crate) const SNAP_MAX_COUNT: usize = 8;

// ── Dataset holdout split / output policy (io.rs) ─────────────────────────────
/// Fixed holdout-shuffle seed. The internal 75/25 MDL-holdout split ALWAYS uses this
/// constant, so final selection is fully deterministic and the CLI `--seed` is a benign
/// no-op; SRBench's random_state cannot perturb which model is chosen. (Combined with
/// the leverage-trimmed holdout scorer in math.rs, near-exact models with measure-zero
/// poles score ~1.0 on any split, so no seed can displace the true form.) Value is the
/// golden-ratio odd constant that was the former per-run default.
pub(crate) const HOLDOUT_SHUFFLE_SEED: u64 = 0x9E3779B97F4A7C15;
/// Minimum row count for a MDL holdout split to activate; the split takes 1/FRAC_DEN.
pub(crate) const HOLDOUT_MIN_ROWS: usize = 40;
pub(crate) const HOLDOUT_FRAC_DEN: usize = 4;
/// Minimum holdout size to actually use the split, and the absolute holdout cap.
pub(crate) const HOLDOUT_MIN_ACTIVE: usize = 20;
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
/// Cap on the monomial-exponent dictionary size (harvest k-monomial atom enum).
pub(crate) const DICT_ATOM_CAP: usize = 4000;
/// Max total |degree| (Σ|exp|) of a dictionary atom kept in the k-monomial enum.
pub(crate) const DICT_ATOM_MAX_DEG: i32 = 10;
/// Extra deflation rounds beyond the term budget in the root additive-vcol harvest
/// (harvest_monomial_vcols iterates max_terms + PURSUIT_EXTRA_ROUNDS times).
pub(crate) const PURSUIT_EXTRA_ROUNDS: usize = 3;
/// Min per-term degree emitted by the root additive-vcol harvest (deg >= this).
/// COUPLED to KMONO_KCOL_MIN_DEG; both gate additive-monomial degree; if you sweep
/// one, consider the other. Kept at its current value here.
pub(crate) const HARVEST_VCOL_MIN_DEG: i32 = 3;
/// Min finite rows for the k-column joint-LS verification (combined_r2_k; f64 count).
pub(crate) const KMONO_COMBINED_MIN_ROWS: f64 = 8.0;
/// Max nonzero exponents kept in a mono_fit monomial (rejects overly dense fits).
pub(crate) const MONO_FIT_EXP_CAP: usize = 8;
/// Max valid rows a masked-divide residual may retain and still spawn a divide child
/// (search caps min_finite_rows at this before gating).
pub(crate) const DIV_MIN_VALID_ROWS_CAP: usize = 50;
/// Min rows for the offset log-linear fit (fit::loglin_r2_at).
pub(crate) const LOGLIN_MIN_ROWS: usize = 8;
/// Min rows for the log-linear / affine monomial fits (fit: monomial_loglin_r2,
/// affine_monomial_r2).
pub(crate) const MONOMIAL_FIT_MIN_ROWS: usize = 16;
/// Min rows for the monomial / k-monomial harvesters, mono_fit, and clean_monomial_fit.
pub(crate) const HARVEST_MIN_ROWS: usize = 24;

// ── Input limits (driver ingest) ─────────────────────────────────────────────
/// Stdin byte cap: a runaway or absurdly large pipe errors cleanly instead of
/// exhausting memory. 1 GiB covers every corpus in scope by a wide margin.
pub(crate) const INPUT_BYTE_CAP: u64 = 1 << 30;
