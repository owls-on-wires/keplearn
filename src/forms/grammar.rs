use crate::forms::nf::{Arg, Base, Expr, Factor, Limits, Term, Un, Q};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Slot {
    Un(Un, Q),
    Group(Q),
}

#[derive(Clone, Debug)]
pub(crate) enum Frame {
    Term {
        neg: bool,
        coef: bool,
        factors: Vec<Factor>,
    },
    Arg {
        slot: Slot,
        scale: bool,
        shift: bool,
        terms: Vec<Term>,
        opened_at: usize,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct State {
    pub(crate) top: Vec<Term>,
    pub(crate) stack: Vec<Frame>,
    pub(crate) nodes: usize,
    pub(crate) consts: usize,
    pub(crate) closing: bool,
}

pub(crate) enum Step {
    Partial(State),
    Complete(Expr),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Opt {
    Complete,
    CloseTerm,
    CloseArg,
    NewTermTop,
    NewTermArg {
        neg: bool,
        coef: bool,
    },
    Var {
        i: u8,
        q: Q,
    },
    Un {
        u: Un,
        q: Q,
        scale: bool,
        shift: bool,
    },
    Group {
        q: Q,
        shift: bool,
    },
}

impl Opt {}

pub(crate) fn remaining_budget(lim: &Limits, st: &State) -> usize {
    let mut remaining = lim.max_nodes - st.nodes;
    if let Some(opened) = st.stack.iter().find_map(|f| {
        if let Frame::Arg { opened_at, .. } = f {
            Some(*opened_at)
        } else {
            None
        }
    }) {
        let used = st.nodes.saturating_sub(opened);
        remaining = remaining.min(lim.max_arg_nodes.saturating_sub(used));
    }
    remaining
}

fn un_flags(u: Un) -> impl Iterator<Item = (bool, bool)> {
    [(false, false), (true, false), (false, true), (true, true)]
        .into_iter()
        .filter(move |&(sc, sh)| (!sc || u.takes_scale()) && (!sh || u.takes_shift()))
}

pub(crate) fn options(lim: &Limits, st: &State) -> Vec<Opt> {
    let mut out: Vec<Opt> = Vec::new();
    let remaining = remaining_budget(lim, st);
    match st.stack.last() {
        None => {
            if !st.top.is_empty() {
                out.push(Opt::Complete);
            }
            if st.top.len() < lim.max_terms {
                let sep = if st.top.is_empty() { 0 } else { 1 };
                if remaining >= sep + 1 {
                    out.push(Opt::NewTermTop);
                }
            }
        }
        Some(Frame::Term { factors, .. }) => {
            if !factors.is_empty() {
                out.push(Opt::CloseTerm);
            }
            if factors.len() >= lim.max_factors {
                return out;
            }
            let sep = if factors.is_empty() { 0 } else { 1 };
            let prev = factors.last().map(|f| &f.base);
            let min_var = match prev {
                None => 0usize,
                Some(Base::Var(j)) => *j as usize + 1,
                Some(_) => usize::MAX,
            };
            if min_var != usize::MAX {
                for i in min_var..lim.n_cols {
                    for &q in lim.exps() {
                        let cost = sep + 1 + (q != Q::P1) as usize;
                        if cost > remaining {
                            continue;
                        }
                        out.push(Opt::Var { i: i as u8, q });
                    }
                }
            }
            if depth(st) >= lim.max_nest {
                return out;
            }
            let prev_un: Option<Un> = match prev {
                Some(Base::Un(u, _)) => Some(*u),
                _ => None,
            };
            let prev_is_group = matches!(prev, Some(Base::Group(_)));
            if !prev_is_group {
                for u in Un::ALL {
                    if let Some(pu) = prev_un {
                        if u < pu {
                            continue;
                        }
                    }
                    let qs: &[Q] = if u == Un::Exp { &[Q::P1] } else { lim.exps() };
                    for &q in qs {
                        for (scale, shift) in un_flags(u) {
                            let kc = scale as usize + shift as usize;
                            if st.consts + kc > lim.max_consts {
                                continue;
                            }
                            let cost = sep + 1 + (q != Q::P1) as usize + kc;
                            if cost + 1 > remaining {
                                continue;
                            }
                            out.push(Opt::Un { u, q, scale, shift });
                        }
                    }
                }
            }
            for &q in lim.group_exps(depth(st) == 0) {
                for shift in [false, true] {
                    let kc = shift as usize;
                    if st.consts + kc > lim.max_consts {
                        continue;
                    }
                    let cost = sep + 2 + kc;
                    if cost + 1 > remaining {
                        continue;
                    }
                    out.push(Opt::Group { q, shift });
                }
            }
        }
        Some(Frame::Arg {
            slot,
            scale,
            shift,
            terms,
            ..
        }) => {
            let closable = !terms.is_empty()
                && match slot {
                    Slot::Un(Un::Exp, _) => {
                        terms.len() == 1 && !(!scale && crate::forms::nf::is_lone_log(&terms[0]))
                    }
                    Slot::Un(Un::Log, _) => {
                        terms.len() >= 2
                            || *shift
                            || (terms.len() == 1 && {
                                let t = &terms[0];
                                !t.neg
                                    && t.factors.len() == 1
                                    && t.factors[0].q == Q::P1
                                    && !matches!(
                                        t.factors[0].base,
                                        Base::Un(Un::Exp, _) | Base::Group(_)
                                    )
                            })
                    }
                    Slot::Un(_, _) => !terms.is_empty(),
                    Slot::Group(_) => terms.len() >= 2 || (terms.len() == 1 && *shift),
                };
            if closable {
                out.push(Opt::CloseArg);
            }
            let max_t = if matches!(slot, Slot::Un(Un::Exp, _)) {
                1
            } else {
                lim.max_terms
            };
            if terms.len() >= max_t {
                return out;
            }
            let sep = if terms.is_empty() { 0 } else { 1 };
            let first = terms.is_empty();
            let signs: &[bool] = match slot {
                Slot::Un(Un::Exp, _) => {
                    if *scale {
                        &[false]
                    } else {
                        &[false, true]
                    }
                }
                Slot::Un(Un::Log, _) => &[false, true],
                Slot::Un(_, _) => {
                    if first {
                        &[false]
                    } else {
                        &[false, true]
                    }
                }
                Slot::Group(q) => {
                    if first && !q.is_frac() {
                        &[false]
                    } else {
                        &[false, true]
                    }
                }
            };
            let coefs: &[bool] = if first { &[false] } else { &[false, true] };
            for &neg in signs {
                for &coef in coefs {
                    if coef && (neg || st.consts + 1 > lim.max_consts) {
                        continue;
                    }
                    let cost = sep + neg as usize + coef as usize;
                    if cost + 1 > remaining {
                        continue;
                    }
                    out.push(Opt::NewTermArg { neg, coef });
                }
            }
        }
    }
    out
}

pub(crate) fn apply(st: &State, opt: Opt) -> Option<Step> {
    let mut s = State {
        closing: false,
        ..st.clone()
    };
    match opt {
        Opt::Complete => {
            let e = Expr {
                terms: st.top.clone(),
            };
            debug_assert_eq!(e.nodes(), st.nodes);
            Some(Step::Complete(e))
        }
        Opt::NewTermTop => {
            let sep = if st.top.is_empty() { 0 } else { 1 };
            s.nodes += sep;
            s.stack.push(Frame::Term {
                neg: false,
                coef: false,
                factors: Vec::new(),
            });
            Some(Step::Partial(s))
        }
        Opt::CloseTerm => {
            let t = match st.stack.last() {
                Some(Frame::Term { neg, coef, factors }) => Term {
                    neg: *neg,
                    coef: *coef,
                    factors: factors.clone(),
                },
                _ => return None,
            };
            s.stack.pop();
            let ok = match s.stack.last_mut() {
                None => {
                    let ok = s.top.last().map_or(true, |p| p.factors < t.factors);
                    if ok {
                        s.top.push(t);
                    }
                    ok
                }
                Some(Frame::Arg { terms, .. }) => {
                    let ok = terms.last().map_or(true, |p| p.factors < t.factors);
                    if ok {
                        terms.push(t);
                    }
                    ok
                }
                Some(Frame::Term { .. }) => unreachable!(),
            };
            if ok {
                s.closing = true;
                Some(Step::Partial(s))
            } else {
                None
            }
        }
        Opt::Var { i, q } => {
            let sep = match st.stack.last() {
                Some(Frame::Term { factors, .. }) => (!factors.is_empty()) as usize,
                _ => return None,
            };
            s.nodes += sep + 1 + (q != Q::P1) as usize;
            if let Some(Frame::Term { factors, .. }) = s.stack.last_mut() {
                factors.push(Factor {
                    base: Base::Var(i),
                    q,
                });
            }
            Some(Step::Partial(s))
        }
        Opt::Un { u, q, scale, shift } => {
            let sep = match st.stack.last() {
                Some(Frame::Term { factors, .. }) => (!factors.is_empty()) as usize,
                _ => return None,
            };
            let kc = scale as usize + shift as usize;
            s.nodes += sep + 1 + (q != Q::P1) as usize + kc;
            s.consts += kc;
            s.stack.push(Frame::Arg {
                slot: Slot::Un(u, q),
                scale,
                shift,
                terms: Vec::new(),
                opened_at: s.nodes,
            });
            Some(Step::Partial(s))
        }
        Opt::Group { q, shift } => {
            let sep = match st.stack.last() {
                Some(Frame::Term { factors, .. }) => (!factors.is_empty()) as usize,
                _ => return None,
            };
            let kc = shift as usize;
            s.nodes += sep + 2 + kc;
            s.consts += kc;
            s.stack.push(Frame::Arg {
                slot: Slot::Group(q),
                scale: false,
                shift,
                terms: Vec::new(),
                opened_at: s.nodes,
            });
            Some(Step::Partial(s))
        }
        Opt::CloseArg => {
            let f = match st.stack.last() {
                Some(Frame::Arg {
                    slot,
                    scale,
                    shift,
                    terms,
                    ..
                }) => {
                    let arg = Arg {
                        terms: terms.clone(),
                        scale: *scale,
                        shift: *shift,
                    };
                    match slot {
                        Slot::Un(u, q) => Factor {
                            base: Base::Un(*u, Box::new(arg)),
                            q: *q,
                        },
                        Slot::Group(q) => Factor {
                            base: Base::Group(Box::new(arg)),
                            q: *q,
                        },
                    }
                }
                _ => return None,
            };
            s.stack.pop();
            let ok = match s.stack.last_mut() {
                Some(Frame::Term { factors, .. }) => {
                    let ok = factors.last().map_or(true, |p| p.base < f.base);
                    if ok {
                        factors.push(f);
                    }
                    ok
                }
                _ => unreachable!(),
            };
            if ok {
                s.closing = true;
                Some(Step::Partial(s))
            } else {
                None
            }
        }
        Opt::NewTermArg { neg, coef } => {
            let sep = match st.stack.last() {
                Some(Frame::Arg { terms, .. }) => (!terms.is_empty()) as usize,
                _ => return None,
            };
            s.nodes += sep + neg as usize + coef as usize;
            s.consts += coef as usize;
            s.stack.push(Frame::Term {
                neg,
                coef,
                factors: Vec::new(),
            });
            Some(Step::Partial(s))
        }
    }
}

pub(crate) fn children(lim: &Limits, st: &State) -> Vec<Step> {
    options(lim, st)
        .into_iter()
        .filter_map(|o| apply(st, o))
        .collect()
}

pub(crate) fn depth(st: &State) -> usize {
    st.stack
        .iter()
        .filter(|f| matches!(f, Frame::Arg { .. }))
        .count()
}

pub(crate) fn root() -> State {
    State {
        top: Vec::new(),
        stack: Vec::new(),
        nodes: 0,
        consts: 0,
        closing: false,
    }
}
