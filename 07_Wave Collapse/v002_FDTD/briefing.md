# Research Briefing: № 07 — Wave Collapse [v002: FDTD]
> Path: **Finite-Difference Time-Domain (Grid)**

## 1. Approach
Direct integration of the Schrödinger equation on a spatial grid. Better for complex, non-periodic boundary conditions and varying potentials.

## 2. Implementation
- **Solvers:** Finite Difference kernels (WebGL2/WebGPU).
- **Visuals:** Isosurface rendering of the probability density $|\psi|^2$.

## 3. Design Reference
Consistent with **№ 02 — Gas Giant (S² FLIP)**.
