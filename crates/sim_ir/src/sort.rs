//! Execution order (Kahn with min-heap) and algebraic loop detection (Tarjan).
use crate::graph::Connectivity;
use sim_core::ids::BlockId;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

/// DF edges (driver block → driven block whose input is direct feedthrough).
pub fn df_edges(conn: &Connectivity, df: &BTreeMap<BlockId, bool>) -> Vec<(BlockId, BlockId)> {
    let mut e = BTreeSet::new();
    for (inp, src) in &conn.driver {
        if df.get(&inp.block).copied().unwrap_or(false) {
            e.insert((src.block, inp.block));
        }
    }
    e.into_iter().collect()
}

/// Topological order or the list of loops.
pub fn sorted_order(
    nodes: &[BlockId],
    edges: &[(BlockId, BlockId)],
) -> Result<Vec<BlockId>, Vec<Vec<BlockId>>> {
    let set: BTreeSet<BlockId> = nodes.iter().copied().collect();
    let edges: BTreeSet<(BlockId, BlockId)> = edges
        .iter()
        .copied()
        .filter(|(a, b)| set.contains(a) && set.contains(b))
        .collect();
    let mut indeg: BTreeMap<BlockId, usize> = set.iter().map(|n| (*n, 0)).collect();
    let mut adj: BTreeMap<BlockId, Vec<BlockId>> = BTreeMap::new();
    for (a, b) in &edges {
        *indeg.get_mut(b).expect("node") += 1;
        adj.entry(*a).or_default().push(*b);
    }
    let mut heap: BinaryHeap<Reverse<BlockId>> = indeg
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(n, _)| Reverse(*n))
        .collect();
    let mut order = vec![];
    while let Some(Reverse(n)) = heap.pop() {
        order.push(n);
        for m in adj.get(&n).cloned().unwrap_or_default() {
            let d = indeg.get_mut(&m).expect("node");
            *d -= 1;
            if *d == 0 {
                heap.push(Reverse(m));
            }
        }
    }
    if order.len() == set.len() {
        return Ok(order);
    }
    let done: BTreeSet<BlockId> = order.into_iter().collect();
    let rest: BTreeSet<BlockId> = set.difference(&done).copied().collect();
    let sub: Vec<(BlockId, BlockId)> = edges
        .iter()
        .copied()
        .filter(|(a, b)| rest.contains(a) && rest.contains(b))
        .collect();
    Err(find_loops(&rest, &sub))
}

fn find_loops(nodes: &BTreeSet<BlockId>, edges: &[(BlockId, BlockId)]) -> Vec<Vec<BlockId>> {
    let mut adj: BTreeMap<BlockId, Vec<BlockId>> = BTreeMap::new();
    for (a, b) in edges {
        adj.entry(*a).or_default().push(*b);
    }
    for v in adj.values_mut() {
        v.sort();
    }
    // Iterative Tarjan.
    let mut index = 0usize;
    let mut idx: BTreeMap<BlockId, usize> = BTreeMap::new();
    let mut low: BTreeMap<BlockId, usize> = BTreeMap::new();
    let mut on: BTreeSet<BlockId> = BTreeSet::new();
    let mut stack: Vec<BlockId> = vec![];
    let mut sccs: Vec<Vec<BlockId>> = vec![];
    for &start in nodes {
        if idx.contains_key(&start) {
            continue;
        }
        let mut work: Vec<(BlockId, usize)> = vec![(start, 0)];
        while let Some((v, i)) = work.pop() {
            if i == 0 {
                idx.insert(v, index);
                low.insert(v, index);
                index += 1;
                stack.push(v);
                on.insert(v);
            }
            let succ = adj.get(&v).cloned().unwrap_or_default();
            if i < succ.len() {
                work.push((v, i + 1));
                let w = succ[i];
                if !idx.contains_key(&w) {
                    work.push((w, 0));
                } else if on.contains(&w) {
                    let lw = idx[&w];
                    let lv = low.get_mut(&v).expect("v");
                    *lv = (*lv).min(lw);
                }
                continue;
            }
            if low[&v] == idx[&v] {
                let mut comp = vec![];
                while let Some(w) = stack.pop() {
                    on.remove(&w);
                    comp.push(w);
                    if w == v {
                        break;
                    }
                }
                sccs.push(comp);
            }
            if let Some(&(parent, _)) = work.last() {
                let lv = low[&v];
                let lp = low.get_mut(&parent).expect("p");
                *lp = (*lp).min(lv);
            }
        }
    }
    let mut loops = vec![];
    for comp in sccs {
        let cs: BTreeSet<BlockId> = comp.iter().copied().collect();
        let start = *cs.iter().next().expect("non-empty");
        let self_loop = edges.contains(&(start, start));
        if cs.len() < 2 && !self_loop {
            continue;
        }
        if cs.len() == 1 {
            loops.push(vec![start]);
            continue;
        }
        // DFS for a path back to start within the component.
        let mut path = vec![start];
        let mut visited = BTreeSet::from([start]);
        fn dfs(
            v: BlockId,
            start: BlockId,
            adj: &BTreeMap<BlockId, Vec<BlockId>>,
            cs: &BTreeSet<BlockId>,
            path: &mut Vec<BlockId>,
            vis: &mut BTreeSet<BlockId>,
        ) -> bool {
            for &w in adj.get(&v).map(|x| x.as_slice()).unwrap_or(&[]) {
                if w == start {
                    return true;
                }
                if cs.contains(&w) && vis.insert(w) {
                    path.push(w);
                    if dfs(w, start, adj, cs, path, vis) {
                        return true;
                    }
                    path.pop();
                }
            }
            false
        }
        dfs(start, start, &adj, &cs, &mut path, &mut visited);
        loops.push(path);
    }
    loops.sort_by_key(|l| l[0]);
    loops
}

/// "algebraic loop: Gain(2) -> Sum(3) -> Gain(2)".
pub fn loop_message(cycle: &[BlockId], names: &BTreeMap<BlockId, String>) -> String {
    let f = |b: &BlockId| format!("{}({})", names.get(b).cloned().unwrap_or_default(), b.0);
    let mut parts: Vec<String> = cycle.iter().map(f).collect();
    if let Some(first) = cycle.first() {
        parts.push(f(first));
    }
    format!("algebraic loop: {}", parts.join(" -> "))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn b(v: &[u32]) -> Vec<BlockId> {
        v.iter().map(|x| BlockId(*x)).collect()
    }
    fn e(v: &[(u32, u32)]) -> Vec<(BlockId, BlockId)> {
        v.iter().map(|(a, c)| (BlockId(*a), BlockId(*c))).collect()
    }
    #[test]
    fn table() {
        assert_eq!(
            sorted_order(&b(&[1, 2, 3]), &e(&[(1, 2), (2, 3)])),
            Ok(b(&[1, 2, 3]))
        );
        assert_eq!(
            sorted_order(&b(&[1, 2, 3]), &e(&[(2, 3)])),
            Ok(b(&[1, 2, 3]))
        );
        assert_eq!(
            sorted_order(&b(&[1, 2, 3, 4]), &e(&[(1, 2), (3, 2), (3, 4)])),
            Ok(b(&[1, 3, 2, 4]))
        );
        assert_eq!(sorted_order(&b(&[5, 2, 9]), &[]), Ok(b(&[2, 5, 9])));
        assert_eq!(
            sorted_order(&b(&[1, 2, 3]), &e(&[(1, 3), (3, 2), (2, 3)])),
            Err(vec![b(&[2, 3])])
        );
        assert_eq!(sorted_order(&b(&[1, 2]), &e(&[(2, 2)])), Err(vec![b(&[2])]));
        assert_eq!(
            sorted_order(&b(&[1, 2, 3, 4, 5]), &e(&[(2, 3), (3, 4), (4, 2), (5, 5)])),
            Err(vec![b(&[2, 3, 4]), b(&[5])])
        );
        assert_eq!(
            sorted_order(&b(&[1, 2, 3]), &e(&[(1, 2), (1, 2)])),
            Ok(b(&[1, 2, 3]))
        );
        assert_eq!(sorted_order(&[], &[]), Ok(vec![]));
        let n: Vec<BlockId> = (1..=1000).map(BlockId).collect();
        let ed: Vec<(BlockId, BlockId)> = (1..1000).map(|i| (BlockId(i), BlockId(i + 1))).collect();
        assert_eq!(sorted_order(&n, &ed).unwrap().len(), 1000);
        let mut ring = ed.clone();
        ring.push((BlockId(1000), BlockId(1)));
        assert_eq!(sorted_order(&n, &ring).unwrap_err()[0].len(), 1000);
        let names = BTreeMap::from([
            (BlockId(2), "Gain".to_string()),
            (BlockId(3), "Sum".to_string()),
        ]);
        assert_eq!(
            loop_message(&b(&[2, 3]), &names),
            "algebraic loop: Gain(2) -> Sum(3) -> Gain(2)"
        );
        let names_g = BTreeMap::from([(BlockId(5), "G".to_string())]);
        assert_eq!(
            loop_message(&b(&[5]), &names_g),
            "algebraic loop: G(5) -> G(5)"
        );

        // Edges with nodes not in nodes are ignored
        assert_eq!(
            sorted_order(&b(&[1, 2]), &e(&[(1, 2), (999, 1), (2, 888)])),
            Ok(b(&[1, 2]))
        );
    }

    #[test]
    fn df_edges_fixture() {
        let p = format!(
            "{}/../../tests/fixtures/models/step_gain_scope.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let m: sim_core::model::Model =
            sim_core::json::from_json_str(&std::fs::read_to_string(p).unwrap()).unwrap();
        let conn = crate::graph::build_connectivity(&m);
        // Gain and Scope are direct-feedthrough; Step is not
        let df = BTreeMap::from([(BlockId(1), false), (BlockId(2), true), (BlockId(3), true)]);
        let edges = df_edges(&conn, &df);
        assert_eq!(
            edges,
            vec![(BlockId(1), BlockId(2)), (BlockId(2), BlockId(3))]
        );
    }
}
