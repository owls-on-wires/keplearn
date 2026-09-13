use crate::*;

pub(crate) struct TunnelKey {
    plane: u8,
    priority: f64,
    depth: usize,
    complexity: usize,
    seq: u64,
    idx: usize,
}

impl PartialEq for TunnelKey {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == CmpOrd::Equal
    }
}

impl Eq for TunnelKey {}

impl PartialOrd for TunnelKey {
    fn partial_cmp(&self, o: &Self) -> Option<CmpOrd> {
        Some(self.cmp(o))
    }
}

impl Ord for TunnelKey {
    fn cmp(&self, o: &Self) -> CmpOrd {
        match self.plane.cmp(&o.plane) {
            CmpOrd::Equal => {}
            c => return c,
        }
        match self.priority.partial_cmp(&o.priority) {
            Some(CmpOrd::Equal) | None => {}
            Some(c) => return c,
        }
        match o.depth.cmp(&self.depth) {
            CmpOrd::Equal => {}
            c => return c,
        }
        match o.complexity.cmp(&self.complexity) {
            CmpOrd::Equal => o.seq.cmp(&self.seq),
            c => c,
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct SweepKey {
    depth: usize,
    kind: u8,
    complexity: usize,
    seq: u64,
    idx: usize,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct StructKey {
    depth: usize,
    complexity: usize,
    seq: u64,
    idx: usize,
}

pub(crate) fn is_structural(r: Option<&Red>) -> bool {
    match r {
        Some(Red::Shell { .. }) | Some(Red::Affc { .. }) => true,
        Some(Red::Div { cost, .. }) => *cost >= 3,
        _ => false,
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Lane {
    Tunnel,
    Sweep,
    Structural,
}

pub(crate) struct Frontier {
    nodes: Vec<Option<Node>>,
    tunnel: BinaryHeap<TunnelKey>,
    sweep: BinaryHeap<std::cmp::Reverse<SweepKey>>,
    structural: BinaryHeap<std::cmp::Reverse<StructKey>>,
}

impl Frontier {
    pub(crate) fn new() -> Self {
        Frontier {
            nodes: Vec::new(),
            tunnel: BinaryHeap::new(),
            sweep: BinaryHeap::new(),
            structural: BinaryHeap::new(),
        }
    }

    pub(crate) fn push(&mut self, n: Node) {
        let idx = self.nodes.len();
        self.tunnel.push(TunnelKey {
            plane: n.plane,
            priority: n.priority,
            depth: n.depth,
            complexity: n.complexity,
            seq: n.seq,
            idx,
        });
        let kind = if matches!(
            n.chain.last(),
            Some(Red::Shell { .. }) | Some(Red::Affc { .. })
        ) {
            1u8
        } else {
            0u8
        };
        self.sweep.push(std::cmp::Reverse(SweepKey {
            depth: n.depth,
            kind,
            complexity: n.complexity,
            seq: n.seq,
            idx,
        }));
        if is_structural(n.chain.last()) {
            self.structural.push(std::cmp::Reverse(StructKey {
                depth: n.depth,
                complexity: n.complexity,
                seq: n.seq,
                idx,
            }));
        }
        self.nodes.push(Some(n));
    }

    pub(crate) fn pop_from(&mut self, tunnel: bool) -> Option<Node> {
        if tunnel {
            while let Some(k) = self.tunnel.pop() {
                if let Some(n) = self.nodes[k.idx].take() {
                    return Some(n);
                }
            }
        } else {
            while let Some(std::cmp::Reverse(k)) = self.sweep.pop() {
                if let Some(n) = self.nodes[k.idx].take() {
                    return Some(n);
                }
            }
        }
        None
    }

    pub(crate) fn pop_structural(&mut self) -> Option<Node> {
        while let Some(std::cmp::Reverse(k)) = self.structural.pop() {
            if let Some(n) = self.nodes[k.idx].take() {
                return Some(n);
            }
        }
        None
    }

    pub(crate) fn pop_lane(&mut self, lane: Lane) -> Option<Node> {
        match lane {
            Lane::Tunnel => self.pop_from(true).or_else(|| self.pop_from(false)),
            Lane::Sweep => self.pop_from(false).or_else(|| self.pop_from(true)),
            Lane::Structural => self
                .pop_structural()
                .or_else(|| self.pop_from(false))
                .or_else(|| self.pop_from(true)),
        }
    }
}
