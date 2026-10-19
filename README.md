# Experiments — Vol. I
> A laboratory for creative physics, generative geometry, and computational aesthetics.

**Live:** [cyberhirsch.github.io/Experiments](https://cyberhirsch.github.io/Experiments/) · every experiment runs in the browser, no install.

This repository serves as a research journal for exploring the intersection of mathematical rigor and visual art. Each experiment is a self-contained study in implementing complex physical phenomena or geometric systems within the browser.

---

## 🔬 Current Research (Volume I)

### [№ 01 — Flame Fractals](https://cyberhirsch.github.io/Experiments/01_Flame/Flame3d_v008.html) · [source](01_Flame/Flame3d_v008.html)
An implementation of Iterated Function Systems (IFS) based on Scott Draves' Fractal Flame algorithm.
- **Scope:** 2D and 3D density estimation rendering; realtime 3D IFS up to v008.
- **Key Features:** Nonlinear variations, logarithmic color mapping, statistical super-sampling, per-orbit hue and emissivity memory, emissive + absorbing material via weighted blended OIT, animated warp, second blend mode.
- **Exports:** Gaussian splat export (snapshots, splat size, falloff, dust, absorb threshold, Y-up / Y-down COLMAP / Z-up).

### [№ 02 — Gas Giant (S² FLIP)](https://cyberhirsch.github.io/Experiments/02_Gas%20Giant/FLIPsphere_v001.html) · [source](02_Gas%20Giant/FLIPsphere_v001.html)
Fluid dynamics on a spherical manifold using a 6-face cubemap atlas.
- **Scope:** Planetary atmosphere visualization.
- **Key Features:** Geodesic advection, Jacobi pressure solve across cube seams (Helmholtz-Hodge decomposition), and no pole singularities.

### [№ 03 — Solar Corona (MHD)](https://cyberhirsch.github.io/Experiments/03_Corona/Corona_v001.html) · [source](03_Corona/Corona_v001.html)
A Magnetohydrodynamic (MHD) simulation of the Sun's atmosphere.
- **Scope:** Plasma dynamics and magnetic topology.
- **Key Features:** Magnetic loop force fields, procedural CME (Coronal Mass Ejection) events, and temperature-based color gradients.

### [№ 04 — Black Hole (Relativity)](https://cyberhirsch.github.io/Experiments/04_Black%20Hole/Black%20Hole_v001.html) · [source](04_Black%20Hole/Black%20Hole_v001.html)
An implementation of a ray-traced Schwarzschild black hole.
- **Scope:** General Relativity approximations in real-time.
- **Key Features:** Light bending (gravitational lensing), Doppler boosting, and accretion disk dynamics.
- **Bonus:** Included [WebGPU implementation](04_Black Hole/webgpu-black-hole/index.html) for high-performance compute-based ray-marching.

### [№ 11 — Erosion](https://cyberhirsch.github.io/Experiments/11_Erosion/Erosion_v001_SDF.html) · [source](11_Erosion/Erosion_v001_SDF.html)
True 3D volumetric erosion of layered desert rock on the GPU.
- **Scope:** Differential weathering of sedimentary strata (hoodoos, arches, mesas).
- **Key Features:** SDF raymarch render, connectivity-based floater culling, talus sedimentation.
- **Method:** 128³ density field on RGBA16Float textures, rain & wind dissolution with angle-of-repose sediment transport.

### [№ 12 — Audio Visualizer (STFT Spiral)](https://cyberhirsch.github.io/Experiments/12_Audio%20Visualizer/AudioViz_v001.html) · [source](12_Audio%20Visualizer/AudioViz_v001.html)
Real-time 3D spectral spiral driven by Short-Time Fourier Transform.
- **Scope:** A geometrically faithful mapping of audio frequency spectra to 3D space.
- **Key Features:** x = f, y = |X|·cos(ϕ), z = |X|·sin(ϕ); live mic / file upload / synthetic demo.
- **Controls:** Three color modes, adjustable radius, auto-rotate camera, OrbitControls.

### [№ 13 — Arbor](https://cyberhirsch.github.io/Experiments/13_Arbor/Arbor_v001.html) · [source](13_Arbor/Arbor_v001.html)
Procedural tree generator with roots that grow around imported objects, exporting textured USD (.usda/.usdz).
- **Scope:** Space-colonization growth, obstacle SDF steering, strand-cylinder mesher, pure-JS USDZ.
- **Key Features:** Three species (oak/spruce/banyan), per-species vigor economy, thigmotropism (contact branching), root wrap around obstacles, procedural bark/root/twig textures, WebGL2 viewport, OBJ import.
- **Exports:** UsdPreviewSurface + UsdUVTexture materials, 64-byte-aligned USDZ archive, embeddable obstacles.

### [№ 14 — Orogeny (S² Stream Power)](https://cyberhirsch.github.io/Experiments/14_Orogeny/Orogeny_v003.html) · [source](14_Orogeny/Orogeny_v003.html)
Seamless planetary erosion — a whole planet held in erosional equilibrium.
- **Scope:** Tectonic uplift, fluvial incision, sediment transport, impacts and deposition on a closed sphere.
- **Key Features:** Equiangular cubed sphere with 1-texel ghost-cell halos, measured arc-length metric, Lagrangian droplets with a P2G/G2P grant cycle, drifting Euler-pole plates with subduction, a motion-vector source deriving uplift from the divergence of the plate velocity field, local Airy isostasy, mass-conserving impacts, talus, lithification, raymarched shell.
- **Method:** 6 × 512² crust-column array with the surface derived by isostasy; mass audit closes to 0.02% over 10,000 steps and 0.0003% per crater; isostatic rebound retains 90.7% of relief against 84.2% for rigid crust.
- **Verification:** Built-in flat, isotropy and mass tests — cube corners perturb a diffusing feature by ≤0.12% of its amplitude.
- **Versions:** [v001](14_Orogeny/Orogeny_v001.html) droplet erosion · [v002](14_Orogeny/Orogeny_v002.html) plates &amp; impacts · [v003](14_Orogeny/Orogeny_v003.html) crust, drift &amp; subduction · [v006](14_Orogeny/Orogeny_v006.html) planet presets · [v007](14_Orogeny/Orogeny_v007.html) rock &amp; gas layers, strata, lava · [v008](14_Orogeny/Orogeny_v008.html) crater break-up · [v012](14_Orogeny/Orogeny_v012.html) export bake · [v013](14_Orogeny/Orogeny_v013.html) base noise per planet · [v018](14_Orogeny/Orogeny_v018.html) volcanism · [v019](14_Orogeny/Orogeny_v019.html) saved planets, atmosphere, aurora · [v020](14_Orogeny/Orogeny_v020.html) typed slider values · [v021](14_Orogeny/Orogeny_v021.html) bursts obey the Impacts switch · [v022](14_Orogeny/Orogeny_v003.html) bombardment = initial strikes + rate.
- **Tools:** [noise designer](14_Orogeny/noise-designer_v002.html) · [impact designer](14_Orogeny/impact-designer_v002.html).

### [№ 18 — Galaxy Colonization](18_Galaxy/briefing.md) · *native*
The spread of a civilisation across the Milky Way, on published numbers rather than invented physics.
- **Scope:** Galactic structure, sub-light settlement percolation, and the cosmic and civilisational hazards that stall or erase a colony.
- **Key Features:** Four-component density model (Juric et al. 2008 disks and halo, Wegg &amp; Gerhard 2013 bar, Reid et al. 2019 arms), Kroupa IMF, metallicity-gated habitability, GPU generation and additive point render of 10⁷–10⁸ systems.
- **Method:** An exponential disk's radial law is *exactly* Gamma(2, h), so R = −h·ln(u₁u₂) samples it in two logarithms — no Newton iteration on an uninvertible CDF.
- **Measured:** On an RTX 3090 a storage binding holds 179 M systems, so 10⁹ needs six position buffers; one dispatch dimension reaches 16.8 M invocations; 10⁹ systems fit 24 GiB at 19.09 GiB with u32-decade event times, and do not at 26.54 GiB with f64 years.
- **Landis (1998) reproduced:** colonisation as a single-source shortest path through sites that are colonising with probability P — the whole history is a static field and "now" is a threshold on it. On a k = 6 neighbour graph the colonised fraction goes from 0.2% at P = 0.50 to 88.4% at P = 0.55: a sharp percolation threshold at P ≈ 0.53.
- **Note:** The only experiment here that is not a web page — 15 GiB of buffer is far past what a browser tab is given. Rust + wgpu + WGSL, `cargo run --release`.

### [№ 28 — CamFluid](https://cyberhirsch.github.io/Experiments/28_CamFluid/CamFluid_v013.html) · [source](28_CamFluid/CamFluid_v013.html)
The webcam's motion vectors drive a flat 2D fluid, coloured from the webcam's rgb.
- **Motion vectors:** Lucas-Kanade optical flow on the GPU at 96 px height, with a noise gate and temporal smoothing; shown as a vector field.
- **Gas:** The Orogeny atmosphere flattened — midpoint advection, motion forcing, vorticity confinement, warm-started Jacobi pressure; the dye carries camera paint plus density.
- **FLIP:** The Gas Giant solver flattened — particles in float textures, bilinear P2G, the same pressure solve, G2P with a 0.97 FLIP / PIC blend; particle colour from the camera.
---

## 🗺️ Roadmap

### Volume II: Quantum & Chaos (Q3 2026)
- **[№ 05 — Superfluid Vortices](05_Superfluid/briefing.md):** Ginzburg-Landau simulation of quantized vortices in Bose-Einstein condensates.
- **[№ 06 — N-Body Galactica](06_Galactica/briefing.md):** Barnes-Hut accelerated galaxy formation and dark matter dynamics.
- **[№ 07 — Wave Collapse](07_Wave Collapse/briefing.md):** 3D Schrödinger Equation visualization with quantum tunneling.
- **[№ 08 — Strange Attractors](08_Strange Attractors/briefing.md):** High-precision chaotic system integration (Aizawa/Thomas).

### Volume III: Morphogenesis (Q4 2026)
- **[№ 09 — Turing Manifolds](09_Turing Manifolds/briefing.md):** Reaction-Diffusion (Gray-Scott) implemented on arbitrary surface meshes.
- **[№ 10 — Crystal Growth](10_Crystal Growth/briefing.md):** 3D Diffusion Limited Aggregation (DLA) for mineral-like fractal growth.

---

## 🛠️ Technology Stack
- **Core:** Vanilla JavaScript (ES6+), HTML5 Canvas.
- **Graphics:** WebGL2 (Primary), WebGPU (Experimental).
- **Math:** GLSL Shaders for parallelized physical integration.
- **Design:** Cormorant Garamond & JetBrains Mono for the "Scientific Journal" aesthetic.

---

## 📜 License

### Source Code
The **source code** in this repository is licensed under the [MIT License](LICENSE).

### Generated Art & Media
Any images, animations, or parameter sets generated using the code in this repository are licensed under the [Creative Commons Attribution-NonCommercial 4.0 International (CC BY-NC 4.0)](https://creativecommons.org/licenses/by-nc/4.0/) license. 

---
*Created with passion for the physics of light and motion.*
