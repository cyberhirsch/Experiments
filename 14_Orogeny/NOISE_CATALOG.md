# № 14 Orogeny — procedural texture catalog

Status: **✓** in `noise-lib.js` · **~** reachable but hidden behind a sub-parameter · **○** not implemented

The library is compositional: **base** × **combinator** × **modifiers**. Many named
techniques in the literature are compositions, not separate generators — "marble" is
`sin(x + turbulence)`, "crackle" is a thresholded `F2−F1`. Those are listed under the
family they belong to, with the composition given.

---

## 1 · Lattice & gradient noises

| | Status | Note |
|---|---|---|
| White / uniform random | ✓ | `base:white`. One hash. Building block only. |
| Value noise | ✓ | `base:value`. 8 hashes + trilinear. Cheapest useful base. |
| Value-cubic / Hermite | ○ | Smoother interpolant, no gradient cost. |
| Perlin (improved, 2002) | ✓ | `base:perlin`. Unit gradients, quintic fade. |
| Simplex 3D | ✓ | `base:simplex`. No axis bias; cheaper than Perlin above 3D. |
| OpenSimplex2 / 2S | ○ | Patent-free Simplex successor, smoother 2nd derivative. |
| Super-simplex | ○ | |
| Flow noise | ○ | Gradients rotate over time — kills the "swimming" look when animated. |
| Analytic-derivative noise | ○ | Returns value **and** gradient in one evaluation. Cheap normals; also what erosion-aware fBm needs. |
| Wavelet noise (Cook & DeRose) | ○ | Band-limited — does not alias under zoom or warp the way lattice noise does. |

## 2 · Cellular / Voronoi / distance

**Now twelve first-class dropdown entries**, not one entry hiding an F-index sub-select.
Per-output normalization constants were *measured* (same math reimplemented in JS, 60,000
samples, 1st/99th percentile mapped onto [−1,1]) rather than guessed — calibrated at
jitter 0.9 / Euclidean; other metrics shift the range and the final clamp absorbs it.

| | Status | Note |
|---|---|---|
| Worley F1 | ✓ | Nearest-point distance — organic cell mosaic. |
| Worley F2 | ✓ | |
| Worley F2−F1 | ✓ | **Cell walls / cracks.** Approximate; see edge distance below. |
| Worley F1+F2 | ✓ | |
| Worley F3 | ✓ | 4-deep sorted insert. |
| Worley F4 | ✓ | |
| F1×F2 | ✓ | |
| F1/F2 | ✓ | |
| **Voronoi cell ID** | ✓ | Flat random value per cell, *not* a distance. The shattered / stained-glass look — cannot be had by tuning F1. |
| **True Voronoi edge distance** | ✓ | Exact distance to the bisector. `F2−F1` only approximates it and is wrong near cell corners. Costs a second 27-cell pass. |
| Crackle / mud-crack | ✓ | Thresholded wall field, `crackleThr` controls crack width. |
| Smooth F1 (soft-min) | ✓ | Exponential soft-min — no derivative discontinuity at the walls, so it lights and erodes cleanly. |
| Metric: Euclidean | ✓ | round cells |
| Metric: Manhattan | ✓ | **diamond cells** |
| Metric: Chebyshev | ✓ | **square cells** |
| Metric: Euclidean² | ✓ | Skips the `sqrt` — free, and shapes falloff differently. |
| Metric: Minkowski p-norm | ✓ | Continuous morph between the three above, `minkowskiP`. |
| Jitter / regularity | ✓ | 0 = perfect grid, 1 = fully random. |
| Voronoi centroid / position | ○ | Outputs the cell's seed point — useful for scattering. |
| Voronoise (Quilez) | ○ | Continuous blend value-noise ↔ Voronoi on two axes. |
| Radial / polar Voronoi | ○ | |
| Weighted Voronoi | ○ | Per-seed radius — Apollonius / power diagram. |

## 3 · Fractal combinators

| | Status | Note |
|---|---|---|
| fBm | ✓ | |
| Turbulence | ✓ | `abs()` per octave — creases at every scale. |
| Ridged multifractal | ✓ | `1−abs()` per octave. |
| Billow | ✓ | `2·abs()−1` — puffy, cloud-like. |
| Hybrid multifractal (Musgrave) | ✓ | Ridges that flatten into valleys. |
| Multifractal (Musgrave) | ✓ | Multiplicative octaves — sharper high relief. |
| Terraced / quantized | ✓ | Mesa / plateau. |
| Swiss turbulence | ○ | Each octave warped by the previous octave's derivative. Erosion-flavoured. |
| Jordan turbulence | ○ | Same family, stronger. |
| Erosion-aware fBm (iq) | ○ | Damps octave amplitude by accumulated slope. Needs analytic derivatives. |
| Pink (1/f) / brown (1/f²) | ○ | Spectral shaping via the `gain` exponent — nearly free. |

## 4 · Warps & modifiers

| | Status | Note |
|---|---|---|
| Domain warp | ✓ | The single most valuable modifier here. |
| Iterated / recursive warp | ✓ | `warpIter` 1–4. |
| Curl warp | ✓ | Divergence-free — never creates or destroys area. |
| Anisotropic stretch | ✓ | Dunes, foliation. |
| **Fold / solarize** | ✓ | One `abs` about a pivot, 4 ALU ops, applied once. Pivot, gain, V/Λ direction. |
| Tileable period | — | **Removed.** |
| Domain repetition (`mod`) | ○ | |
| Mirror / kaleidoscope | ○ | |
| Polar / log-polar transform | ○ | Turns stripes into rings or rays. |
| Twist / bend | ○ | |

## 5 · Structured & periodic

| | Status | Note |
|---|---|---|
| Stripes (sine) | ○ | |
| Square / triangle / sawtooth wave | ○ | |
| Checkerboard | ○ | |
| Brick / tile | ○ | |
| Hex grid | ○ | |
| Triangle grid | ○ | |
| Truchet tiles | ○ | Random-rotated tile arcs — joint and fault patterns. |
| Weave / fabric | ○ | |
| Halftone / dot screen | ○ | |
| Moiré / interference | ○ | |
| Quasicrystal | ✓ | Summed rotated cosines. |
| Penrose tiling | ○ | |
| Phyllotaxis / sunflower | ○ | |

## 6 · Classic composites (Perlin's originals)

These are compositions, not generators — a **remap** stage over an ordinary fBm. The remap
takes both the value *and* the position, because marble is a sine of a *coordinate*
displaced by noise, not a sine of the noise.

| | Status | Composition |
|---|---|---|
| Marble / veining | ✓ | remap `marble`: `sin((q.x·f + v·k)·2π)` |
| Wood grain | ✓ | remap `rings`: `sin((‖q‖·f + v·k)·2π)` |
| Strata / banding | ✓ | remap `strata`: `fract(q.y·f + v·k)` |
| Threshold (hard / soft) | ✓ | remap `step` / `smoothstep` |
| Camouflage | ✓ | remap `posterize` over fBm |
| Clouds / plasma | ✓* | fBm — *needs only a colour ramp at display time |
| Fire | ○ | turbulence + vertical gradient + ramp |
| Rust / corrosion | ○ | cellular × fBm threshold — needs the two-stack blend |

## 7 · Spectral / band-limited

| | Status | Note |
|---|---|---|
| Gabor | ✓ | Bounded-impulse approximation, ≤4 impulses/cell. |
| Phasor | ✓ | Randomised per-impulse orientation. |
| Sparse convolution | ✓ | Isotropic, no carrier. |
| Spot noise | ○ | Gabor's ancestor. |
| Blue noise / Poisson-disk | ✓ | Special preview mode (point scatter). |
| Anisotropic oriented noise | ~ | via the `aniso` modifier. |

## 8 · Simulated (no closed form — need their own pass)

| | Status | Note |
|---|---|---|
| Reaction-diffusion (Gray-Scott) | ✓ | Special mode, chunked ping-pong. |
| Cellular automata / caves | ○ | |
| Diffusion-limited aggregation | ○ | Lightning, dendrites, frost. |
| Hydraulic erosion | ✓* | *Exists in the main tool, not the noise library. |
| Thermal erosion / talus | ✓* | *Same. |
| Fluid advection | ○ | |
| Perlin worms | ○ | Tunnel networks. |

## 9 · Recursive / precompute

| | Status | Note |
|---|---|---|
| Diamond-square | ✓ | Special mode, CPU grid. |
| Midpoint displacement (1D) | ○ | |
| Fault fractal | ○ | Repeated random great-circle cuts — the classic *spherical* terrain method, directly relevant to this project. |
| Poisson-disk / dart throwing | ✓ | Special mode. |
| Wang tiles / histogram-preserving tiling | ○ | Stub. Retiles an **input bitmap**; a workflow, not a generator. |

## 10 · Escape-time & IFS fractals

| | Status |
|---|---|
| Mandelbrot / Julia | ○ |
| Burning ship / tricorn | ○ |
| Newton fractal | ○ |
| Fractal flame / IFS | ○ |
| KIFS (kaleidoscopic IFS) | ○ |
| Apollonian gasket | ○ |
| Menger / Sierpinski | ○ |

## 11 · Scatter & bombing

| | Status |
|---|---|
| Texture bombing / splatter | ○ |
| Star field | ○ |
| Bubbles / foam | ○ |
| Scratches / fibers | ○ |
| Craquelure | ○ |
| Crystal / shatter | ○ |

## 12 · Terrain-domain generators

| | Status | Note |
|---|---|---|
| River networks / flow accumulation | ○ | |
| Ridge networks | ○ | |
| Strata / sedimentary banding | ✓ | remap `strata`. |
| Dune fields | ✓ | anisotropy + ridged. |
| Scree / talus | ○ | |

---

## Tally

- **✓ implemented:** **20 base variants** (9 GLSL bases, the cellular one exposing 12
  outputs), 8 combinators, 4 modifiers, **7 remap modes**, 3 special preview modes
- **○ not implemented:** ~65 entries
- **removed:** tileable period

## Shader cost

Base, cellular output, combinator, modifiers and remap are all **preprocessor constants**,
so exactly one of each survives compilation. Verified by expanding every combination
offline with a preprocessor evaluator:

- **4,480 combinations swept, zero failures**
- exactly one `evalBaseUnit` body per program; only the selected base's generator compiled
- largest specialized shader: **170 lines**
- the bit-identical `fbm`/`ridged` fast path for the main tool's terrain init is intact

For contrast, the original runtime-dispatch version generated ~54,000 unrolled kernel
bodies in a single shader and took down the GPU process.

## Recommended order from here

1. **Structured patterns** (§5) — checkerboard, brick, hex, Truchet, weave, halftone.
   Each is a short standalone function; they are the largest remaining visual gap.
2. **Fault fractal** (§9) — spherical by construction, unusually relevant to this project.
3. **Two-stack blend** — mix two full base+combinator stacks by weight. Unlocks
   Perlin–Worley hybrids and rust, and is more generally useful than either.
4. **Voronoise** and Voronoi centroid output — completes §2.
5. Escape-time fractals, scatter/bombing, simulated families on demand.
