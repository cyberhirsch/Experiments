# Research Briefing: № 06 — Galactica [v001: Brute Force]
> Path: **Direct N-Body (O(N²) GPGPU)**

## 1. Approach
Direct calculation of all gravitational pairs. This is the most accurate method but computationally expensive.

## 2. Implementation
- **Solvers:** WebGPU Compute Shaders with tiling optimizations for shared memory.
- **Visuals:** High-precision particle paths and deep-space thermal gradients.

## 3. Design Reference
Consistent with **№ 02 — Gas Giant (S² FLIP)**.
