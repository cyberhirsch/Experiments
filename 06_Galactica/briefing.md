# Research Briefing: № 06 — N-Body Galactica
> Project Status: **Pre-Implementation**
> Domain: **Astrophysical Dynamics**
> 
> [!IMPORTANT]
> Multiple architectural approaches are defined for this simulation. Please investigate the **v-folders** to compare performance-oriented vs. accuracy-oriented implementation paths.

## 1. Objective
Simulate large-scale galaxy formation and interaction using N-body dynamics. The goal is to visualize the emergent structure of dark matter halos and stellar disk formation.

## 2. Technical Roadmap
- **[v001: Brute Force](v001_BruteForce/briefing.md):** O(N²) direct calculation for high precision.
- **[v002: Barnes-Hut](v002_BarnesHut/briefing.md):** O(N log N) hierarchical tree for 1M+ particles.
- **Tech:** WebGPU Compute Shaders.

## 3. Design Reference
Following the aesthetic of **№ 02 — Gas Giant (S² FLIP)**:
- **Typography:** *Cormorant Garamond* (Italic) for headings, *JetBrains Mono* for metrics.
- **Color Palette:** Deep Obsidian (`#070809`) background with Thermal Peach (`#ffb474`) accents for high-energy collisions.
- **Layout:** Stage-centric view with vertical readouts for gravitational potential and kinetic energy.
