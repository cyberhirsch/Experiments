# Research Briefing: № 08 — Strange Attractors
> Project Status: **Pre-Implementation**
> Domain: **Chaotic Systems**
> 
> [!IMPORTANT]
> This study compares individual trajectory rendering with statistical density mapping. Please explore the **v-folders** to review the specific shaders and integration logic for each approach.

## 1. Objective
High-precision visualization of chaotic attractors (Lorenz, Aizawa, Thomas) using advanced integration techniques to ensure long-term path stability.

## 2. Technical Roadmap
- **[v001: RK4 Paths](v001_RK4_Paths/briefing.md):** Individual trajectory integration (Ribbons/Trails).
- **[v002: Density Field](v002_Density_Field/briefing.md):** Statistical occupancy mapping (Massive parallel sampling).
- **Tech:** Float64-precision simulation / WebGPU.

## 3. Design Reference
Following the aesthetic of **№ 02 — Gas Giant (S² FLIP)**:
- **Typography:** *Cormorant Garamond* (Italic) for headings, *JetBrains Mono* for metrics.
- **Color Palette:** Deep Obsidian (`#070809`) background with Steel Blue accents.
- **Layout:** Minimalist header with experiment volume (Vol. II) and a dedicated sidebar for attractor coefficient manipulation.
