//! Landis percolation on the sampled galaxy.
//!
//! Landis (1998): every colony is, with probability P, a *colonising* one
//! that sends ships to every reachable neighbour, and with probability 1-P
//! one that never does. Above a critical P the colonised region spans the
//! galaxy, riddled with voids; below it, colonisation dies in a finite
//! cluster. That is site percolation, and it has a consequence the PRD's
//! discrete-event design does not exploit:
//!
//! Whether a site is colonising is a coin that belongs to the SITE, not to
//! whoever reaches it first. So the arrival time at every system is simply
//! the shortest travel time from the seed through open sites - a single-
//! source shortest path. Compute it once and the entire history is a static
//! field; "now" is a threshold on it. No stepping, no drift, exact to the
//! definition. Hazards that kill colonies after the fact will break this and
//! need real stepping. Landis alone does not.
//!
//! Reachability is by neighbour COUNT, not distance. At 20 M sampled systems
//! the nearest sampled neighbour near the Sun is ~65 ly; a physical ship
//! range of 10-30 ly would reach nothing. "The k nearest" is scale-free, and
//! it is how Landis actually posed the problem - a lattice has a coordination
//! number, not a radius.

use kiddo::{ImmutableKdTree, SquaredEuclidean};
use rayon::prelude::*;
use std::collections::{BinaryHeap, VecDeque};
use std::num::NonZero;
use std::sync::atomic::{AtomicU32, Ordering};

pub const LY_PER_KPC: f32 = 3261.563_8;

/// The k_max nearest neighbours of every system, nearest first.
pub struct Graph {
    pub n: usize,
    pub k_max: usize,
    /// row-major, `k_max` entries per system, `u32::MAX` where fewer exist
    pub nbr: Vec<u32>,
    pub pos: Vec<[f32; 3]>,
}

impl Graph {
    /// Build with a k-d tree. `progress` is written 0..=100 as it goes so a
    /// UI thread can watch; pass a dummy if you do not care.
    pub fn build(pos: Vec<[f32; 3]>, k_max: usize, progress: &AtomicU32) -> Self {
        let n = pos.len();
        progress.store(0, Ordering::Relaxed);
        let tree: ImmutableKdTree<f32, 3> =
            ImmutableKdTree::new_from_slice(&pos).expect("kd-tree");
        progress.store(5, Ordering::Relaxed);

        // ask for k_max+1 and drop self, which comes back at distance zero.
        // A coincident duplicate would survive that filter, which is why the
        // filter is by index and not by distance.
        let want = NonZero::new(k_max + 1).expect("k_max >= 1");
        let done = AtomicU32::new(0);
        let chunk = 4096usize;
        let mut nbr = vec![u32::MAX; n * k_max];
        nbr.par_chunks_mut(chunk * k_max)
            .enumerate()
            .for_each(|(ci, out)| {
                let base = ci * chunk;
                for (j, row) in out.chunks_mut(k_max).enumerate() {
                    let i = base + j;
                    let res = tree
                        .query(&pos[i])
                        .nearest_n::<SquaredEuclidean<f32>>(want)
                        .execute();
                    let mut w = 0;
                    for r in res {
                        let idx = r.item as usize;
                        if idx == i || w == k_max {
                            continue;
                        }
                        row[w] = idx as u32;
                        w += 1;
                    }
                }
                let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                let total = n.div_ceil(chunk) as u32;
                progress.store(5 + (95 * d) / total.max(1), Ordering::Relaxed);
            });
        progress.store(100, Ordering::Relaxed);
        Self { n, k_max, nbr, pos }
    }

    /// For tests: an explicit adjacency, positions given for distances.
    pub fn from_edges(pos: Vec<[f32; 3]>, k_max: usize, nbr: Vec<u32>) -> Self {
        assert_eq!(nbr.len(), pos.len() * k_max);
        Self { n: pos.len(), k_max, nbr, pos }
    }

    /// Index of the sampled system nearest a point (kpc).
    pub fn nearest(&self, p: [f32; 3]) -> usize {
        // linear scan; called once per run, and the tree is not kept around
        let mut best = 0usize;
        let mut bd = f32::INFINITY;
        for (i, q) in self.pos.iter().enumerate() {
            let d = (q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2);
            if d < bd {
                bd = d;
                best = i;
            }
        }
        best
    }

    #[inline]
    fn dist_ly(&self, a: usize, b: usize) -> f32 {
        let p = self.pos[a];
        let q = self.pos[b];
        ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
            * LY_PER_KPC
    }

    #[inline]
    fn neighbours(&self, i: usize, k: usize) -> &[u32] {
        let k = k.min(self.k_max);
        &self.nbr[i * self.k_max..i * self.k_max + k]
    }
}

/// The site's own coin. splitmix64 finaliser over (index, seed): decorrelated
/// bits, deterministic, and monotone in P for a fixed seed - a site that is
/// open at P stays open at every higher P, so a sweep is a nested sequence
/// and not twenty unrelated galaxies.
#[inline]
pub fn site_coin(i: u32, seed: u32) -> f32 {
    let mut x = (i as u64) ^ ((seed as u64) << 32) ^ 0x9E37_79B9_7F4A_7C15;
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x >> 40) as f32 / (1u64 << 24) as f32
}

#[derive(Clone, Copy, Debug)]
pub struct Rule {
    /// Landis P: probability a colony is a colonising one
    pub p: f32,
    /// neighbours a colonising colony reaches
    pub k: usize,
    /// ship speed as a fraction of c; travel time in years = ly / v
    pub v_c: f32,
    pub seed: u32,
}

pub struct Outcome {
    /// arrival time in years from the seed's founding; INFINITY if never
    pub arrival: Vec<f32>,
    /// which sites are colonising ones under this seed and P
    pub open: Vec<bool>,
    pub reached: usize,
    pub open_reached: usize,
    pub latest_arrival: f32,
}

/// Single-source shortest path through open sites. The seed is colonising
/// by fiat - it is the civilisation that started this. A closed site can be
/// reached, and is colonised, but relaxes nothing: it is a colony that never
/// builds a ship.
pub fn colonise(g: &Graph, src: usize, rule: Rule) -> Outcome {
    let open: Vec<bool> = (0..g.n as u32).map(|i| site_coin(i, rule.seed) < rule.p).collect();
    colonise_with(g, src, rule, open)
}

/// Same, with the coins supplied - so a test can pin exactly which sites are
/// closed instead of hunting for a seed that happens to.
pub fn colonise_with(g: &Graph, src: usize, rule: Rule, open: Vec<bool>) -> Outcome {
    let n = g.n;
    assert_eq!(open.len(), n);
    let mut arrival = vec![f32::INFINITY; n];
    // f32 non-negative bit patterns sort as the values do, so the heap can
    // key on bits without an ordered-float dependency
    let mut heap: BinaryHeap<std::cmp::Reverse<(u32, u32)>> = BinaryHeap::new();
    arrival[src] = 0.0;
    heap.push(std::cmp::Reverse((0f32.to_bits(), src as u32)));
    let inv_v = 1.0 / rule.v_c.max(1e-6);

    while let Some(std::cmp::Reverse((tb, u))) = heap.pop() {
        let u = u as usize;
        let t = f32::from_bits(tb);
        if t > arrival[u] {
            continue; // stale entry
        }
        if u != src && !open[u] {
            continue; // colonised, never colonising
        }
        for &v in g.neighbours(u, rule.k) {
            if v == u32::MAX {
                break;
            }
            let v = v as usize;
            let tv = t + g.dist_ly(u, v) * inv_v;
            if tv < arrival[v] {
                arrival[v] = tv;
                heap.push(std::cmp::Reverse((tv.to_bits(), v as u32)));
            }
        }
    }

    let mut reached = 0usize;
    let mut open_reached = 0usize;
    let mut latest = 0f32;
    for i in 0..n {
        if arrival[i].is_finite() {
            reached += 1;
            if open[i] {
                open_reached += 1;
            }
            if arrival[i] > latest {
                latest = arrival[i];
            }
        }
    }
    Outcome { arrival, open, reached, open_reached, latest_arrival: latest }
}

/// Fraction of the galaxy the seed's cluster reaches - the percolation order
/// parameter. BFS, no times, so a sweep over P is cheap.
pub fn reach_fraction(g: &Graph, src: usize, p: f32, k: usize, seed: u32) -> f64 {
    let n = g.n;
    let mut seen = vec![false; n];
    let mut q = VecDeque::new();
    seen[src] = true;
    q.push_back(src);
    let mut count = 1usize;
    while let Some(u) = q.pop_front() {
        if u != src && site_coin(u as u32, seed) >= p {
            continue;
        }
        for &v in g.neighbours(u, k) {
            if v == u32::MAX {
                break;
            }
            let v = v as usize;
            if !seen[v] {
                seen[v] = true;
                count += 1;
                q.push_back(v);
            }
        }
    }
    count as f64 / n as f64
}

/// Pack for the GPU: bits 0..31 hold (arrival in decades + 1), so 0 means
/// never reached and the seed reads 1; bit 31 marks a colonising site.
pub fn pack(out: &Outcome) -> Vec<u32> {
    out.arrival
        .iter()
        .zip(&out.open)
        .map(|(&a, &o)| {
            let t = if a.is_finite() {
                ((a / 10.0) as u32).min(0x7FFF_FFFE) + 1
            } else {
                0
            };
            t | if o { 0x8000_0000 } else { 0 }
        })
        .collect()
}
