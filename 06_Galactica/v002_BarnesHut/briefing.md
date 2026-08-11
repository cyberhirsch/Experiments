# Research Briefing: № 06 — Galactica [v002: Barnes-Hut]
> Path: **Hierarchical Tree (O(N log N))**

## 1. Approach
Approximates distant forces using a quadtree/octree. Allows for significantly higher particle counts (1M+) at the cost of slight force approximation.

## 2. Implementation
- **Solvers:** JavaScript/WASM tree construction with GPU-based force accumulation.
- **Visuals:** Visualizing the tree structure (debugging view) alongside dark matter halos.

## 3. Design Reference
Consistent with **№ 02 — Gas Giant (S² FLIP)**.
