# Research Briefing: № 08 — Strange Attractors [v002: Density Field]
> Path: **Statistical Chaos (Density / Histogram)**

## 1. Approach
Instead of paths, this version models the statistical occupancy of phase space. Captures the "ghostly" shape of the attractor through millions of point samples.

## 2. Implementation
- **Solvers:** Massively parallel point integration + Accumulation buffer (Additive blending).
- **Visuals:** Density-based rendering (Logarithmic mapping) to reveal fine-scale attractor structure.

## 3. Design Reference
Consistent with **№ 02 — Gas Giant (S² FLIP)**.
