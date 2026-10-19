//! The colonisation model, checked where it can be checked exactly.

use galaxy_colonization::colonize::{
    colonise_with, reach_fraction, site_coin, Graph, Rule, LY_PER_KPC,
};
use std::sync::atomic::AtomicU32;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

fn cloud(n: usize, seed: u64) -> Vec<[f32; 3]> {
    let mut r = Rng(seed | 1);
    (0..n).map(|_| [r.next(), r.next(), r.next()]).collect()
}

/// The k-d tree must return exactly what brute force returns, as a set.
#[test]
fn knn_matches_brute_force() {
    let pos = cloud(3000, 0xC0FFEE);
    let k = 6;
    let g = Graph::build(pos.clone(), k, &AtomicU32::new(0));
    assert_eq!(g.n, 3000);
    for i in 0..g.n {
        let p = pos[i];
        let mut all: Vec<(f32, usize)> = pos
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(j, q)| {
                let d = (q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2);
                (d, j)
            })
            .collect();
        all.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let mut want: Vec<u32> = all[..k].iter().map(|x| x.1 as u32).collect();
        let mut got: Vec<u32> = g.nbr[i * k..(i + 1) * k].to_vec();
        want.sort();
        got.sort();
        assert_eq!(got, want, "system {i}: neighbour set differs from brute force");
    }
}

/// Five systems on a line, 1 kpc apart, each linked to its two flanks. Close
/// site 2. The wave must reach 2 - it is colonised - and go no further, and
/// the arrival times must be exactly distance / speed.
#[test]
fn closed_site_is_colonised_but_does_not_propagate() {
    let pos: Vec<[f32; 3]> = (0..5).map(|i| [i as f32, 0.0, 0.0]).collect();
    let none = u32::MAX;
    // k_max = 2: [left, right]
    let nbr = vec![
        1, none, // 0
        0, 2,    // 1
        1, 3,    // 2
        2, 4,    // 3
        3, none, // 4
    ];
    let g = Graph::from_edges(pos, 2, nbr);
    let rule = Rule { p: 1.0, k: 2, v_c: 0.1, seed: 0 };
    let open = vec![true, true, false, true, true];
    let out = colonise_with(&g, 0, rule, open);

    let hop = LY_PER_KPC / 0.1; // years per kpc at 0.1c
    assert_eq!(out.arrival[0], 0.0);
    assert!((out.arrival[1] - hop).abs() < 1e-2 * hop, "{}", out.arrival[1]);
    assert!((out.arrival[2] - 2.0 * hop).abs() < 1e-2 * hop, "{}", out.arrival[2]);
    assert!(out.arrival[3].is_infinite(), "site 3 was reached through a closed site");
    assert!(out.arrival[4].is_infinite());
    assert_eq!(out.reached, 3);
    assert_eq!(out.open_reached, 2);
}

/// Percolation: the order parameter must be ~0 well below threshold, ~1 well
/// above, and monotone between - the sweep is nested by construction, so a
/// site open at P is open at every larger P.
#[test]
fn reach_fraction_shows_a_threshold() {
    let n = 30_000;
    let pos = cloud(n, 0xBEEF);
    let k = 6;
    let g = Graph::build(pos, k, &AtomicU32::new(0));
    let src = g.nearest([0.5, 0.5, 0.5]);

    let mut prev = 0.0;
    let mut curve = Vec::new();
    for step in 1..=18 {
        let p = step as f32 * 0.05;
        let f = reach_fraction(&g, src, p, k, 7);
        curve.push((p, f));
        assert!(
            f + 1e-9 >= prev,
            "not monotone: f({p:.2}) = {f:.4} < f(prev) = {prev:.4}"
        );
        prev = f;
    }
    for (p, f) in &curve {
        println!("P = {p:.2}   colonised fraction = {f:.4}");
    }
    let lo = curve[1].1; // P = 0.10
    let hi = curve[17].1; // P = 0.90
    assert!(lo < 0.02, "P=0.10 already reaches {lo:.3} of the galaxy");
    assert!(hi > 0.80, "P=0.90 reaches only {hi:.3}");

    // where does it cross a half? report it - this is the number Landis
    // computed for lattices, and here it is for a k=6 random-point graph
    let pc = curve
        .windows(2)
        .find(|w| w[0].1 < 0.5 && w[1].1 >= 0.5)
        .map(|w| w[0].0 as f64 + 0.05 * (0.5 - w[0].1) / (w[1].1 - w[0].1).max(1e-9))
        .unwrap_or(f64::NAN);
    println!("half-galaxy crossing near P = {pc:.3}");
    assert!(pc.is_finite() && pc > 0.15 && pc < 0.75, "threshold {pc} out of range");
}

/// The coin is a function of the site and the seed only, and nested in P.
#[test]
fn site_coin_is_deterministic_and_uniform() {
    let a = site_coin(12345, 9);
    let b = site_coin(12345, 9);
    assert_eq!(a, b);
    assert_ne!(site_coin(12345, 9), site_coin(12345, 10));
    let n = 200_000u32;
    let mut sum = 0.0f64;
    for i in 0..n {
        let c = site_coin(i, 3);
        assert!((0.0..1.0).contains(&c));
        sum += c as f64;
    }
    let mean = sum / n as f64;
    assert!((mean - 0.5).abs() < 0.005, "coin mean {mean} is not 0.5");
}
