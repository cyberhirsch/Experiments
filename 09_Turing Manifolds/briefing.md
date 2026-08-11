# Research Briefing: № 09 — Turing Manifolds
> Project Status: **Pre-Implementation**
> Domain: **Biological Morphogenesis**
> 
> [!IMPORTANT]
> The implementation of reaction-diffusion on manifolds can be achieved via mesh-based or meshless methods. Examine the **v-folders** for detailed technical specs on both paths.

## 1. Objective
Implementation of the Gray-Scott reaction-diffusion model on arbitrary 3D surface meshes, exploring how pattern formation is influenced by curvature.

## 2. Technical Roadmap
- **[v001: FEM](v001_FEM/briefing.md):** Finite Element Method on fixed triangle meshes.
- **[v002: Local Neighborhood](v002_LocalNeighborhood/briefing.md):** Meshless diffusion on unstructured point sets.
- **Tech:** WebGL2 / WebGPU (Manifold advection).

## 3. Design Reference
Following the aesthetic of **№ 02 — Gas Giant (S² FLIP)**:
- **Typography:** *Cormorant Garamond* (Italic) for headings, *JetBrains Mono* for metrics.
- **Color Palette:** Deep Obsidian (`#070809`) background with Muted Slate patterns.
- **Layout:** Corner annotations detailing feed/kill rates and surface topology descriptors.
