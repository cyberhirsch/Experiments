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
| Clouds / plasma | ✓ | fBm + a gradient map (designer, display only) |
| Fire | ○ | turbulence + vertical gradient + ramp |
| Rust / corrosion | ~ | cellular × fBm threshold — buildable now as a composite (multiply, then a threshold remap on a layer); no preset ships |

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
3. ~~**Two-stack blend**~~ — done, as the composite layer stack below (up to four stacks,
   nine blend modes, masks).
4. **Voronoise** and Voronoi centroid output — completes §2.
5. Escape-time fractals, scatter/bombing, simulated families on demand.

---

## Designer: sphere view, gradient map, compositing

Three additions to `noise-designer.html`. None of them changes what a **single** preset
compiles to: `glslForParams` output is the same string as before (the library body was
only split in two, `GLSL_PRELUDE + GLSL_LAYER`), and its cache key is unchanged.

### Sphere view (Shape → Sphere)

The noise is sampled with the **unit direction**, `evalNoise(normalize(dir), uNP)` —
exactly the call the planet tool's terrain init makes — so the sphere shows what the
planet starts from, including anything a 3D field does that the flat `z = 0` tile never
showed. *View scale* does not apply here: the radius is 1, as on the planet, and the
preset's own frequency sets the size of features.

- **Bake, then draw.** The field is evaluated once per texel into an R32F cube map
  (*Bake / face* 256² / 512² / 1024²; 512² matches the planet's default grid), then drawn
  on a 6 × 192² cube-sphere mesh. Orbiting (drag) and zoom (wheel) only re-shade; the
  bake reruns when the noise or the resolution changes.
- **Displace** moves vertices radially by `disp × value`; 0 is a smooth sphere carrying
  only shading. **Relief height** drives the shading normal (central differences one
  texel apart on the cube map), with the same meaning as on the tile.
- The cube map's texel layout is **gnomonic**; the planet's grid is **equiangular**. The
  field is the same function of direction either way — only where the texels fall
  differs, so per-texel aliasing of a very high-frequency preset will not match the
  planet texel for texel.
- Needs `EXT_color_buffer_float` (as the reaction-diffusion preview already did).
  Without `OES_texture_float_linear` the cube map is sampled NEAREST and shading shows
  texel facets.

### Gradient map (right panel)

A colour ramp over the final value, used by the *height ramp* and *relief + tint* views
on both tile and sphere (turning it on switches the grey *relief* view to *relief + tint*).
Click the bar to add a stop, drag a marker to move it, select one to recolour or delete
it (two minimum). Built-in ramps: designer blue, terrain, ice, lava, desert, grey.
*Value at left / right* sets which noise values map to the ends (default −1 … 1).

Stored on the preset only when enabled; **display only** — the planet tool ignores it.

```json
"gradient": {
  "stops": [ { "t": 0, "color": [0.03, 0.08, 0.2] }, { "t": 1, "color": [0.96, 0.97, 0.98] } ],
  "range": [-1, 1]
}
```
`t` in 0…1 along the ramp, colours in 0…1, `range` in value space.

### Compositing (Mode → Composite)

A stack of up to **4** layers, each a full noise parameter set (`np`) edited with the
usual generator panel, plus a blend mode, a weight and an optional mask. Evaluated in
**one** shader.

- **Value convention.** Each layer's `[-1,1]` output is mapped to `u ∈ [0,1]`, blended
  there, and the result mapped back to `[-1,1]` — so `evalComposite(p)` is a drop-in for
  `evalNoise`. The accumulator starts at mid-grey (signed 0); layer 0 always *mixes* onto
  it, so its weight is its contrast.
- **Blend modes:** mix (lerp), add (signed: adds `u − 0.5`, i.e. detail around the
  current value), subtract (signed), multiply, screen, overlay, max, min, difference.
  Each is `r = mix(r, blend(r, u), weight × mask)`. Nothing clamps the accumulator, so
  stacked adds can leave `[-1,1]`.
- **Masks:** none, any other layer's own `u`, or the result so far; optionally inverted.
  Every layer is evaluated at every sample, so a mask may reference a layer below it. A
  layer at weight 0 acts purely as a mask.
- **Code generation** (`glslForComposite`). Per-layer specialization uses the same
  preprocessor constants as a single noise, and those are global — so each layer is its
  own copy of the library body between its `#define`s and `#undef`s, with every function
  that body defines renamed `_L<i>` (`evalNoise_L0`, `cellular_L2`, …). The rename list
  is scraped from the source, not hand-kept. Blend, weight and mask are **uniforms**
  (`uCompC<i>` structs; each layer's params are `uCompL<i>`): their branches hold a few
  ALU ops and no noise evaluation, so they don't need specializing, and changing them
  does not recompile. The cache key is the layer count plus each layer's `glslKey`.
- **Why 4.** Cost is linear in layers — all of them run at every sample, and each is a
  full specialized noise. And `NoiseParams` has 34 members; WebGL2 guarantees only 224
  fragment uniform vectors and may spend one per scalar member, so four layers can take
  up to 136 before the host's own uniforms.
- **Rendering in the designer.** Composites never go through the direct tile program
  (which calls `evalNoise` three times per pixel for relief — three times a four-layer
  stack is the inlining multiplier that once crashed the GPU process). They are baked
  once per pixel into an R32F texture and shaded from it, the same way the sphere is.

Preset format — a composite has **no `np`**, deliberately: every consumer written before
composites filters on `p.np`, so Orogeny v003–v006 skip it rather than misread it.

```json
{
  "id": "p…", "name": "ridges over cells", "mode": "composite",
  "layers": [
    { "np": { "base": 4, "combinator": 4, "...": "..." }, "blend": "mix", "weight": 1, "mask": null },
    { "np": { "base": 1, "combinator": 3, "...": "..." }, "blend": "add", "weight": 0.5,
      "mask": { "src": "layer", "layer": 0, "invert": false } }
  ],
  "gradient": { "...": "optional, as above" }
}
```
`mask` is `null`, `{src:"layer", layer, invert}` or `{src:"result", invert}`.
`normalizeComposite` completes every `np` from defaults, applies the cap and drops masks
that point at a missing layer, so hand-edited files load.

### High detail (noise-lib + `noise-designer_v002.html`)

All opt-in; a preset with ≤ 8 octaves compiles to the same code and the same values as
before on every page.

- **Octaves to 16.** Loops run to `OG_OCTMAX`, which is 8 unless the preset asks for
  more, then 16 (`glslKey` gains `_o16` only in that case, so no existing key changes).
  The value-noise fast path gets extended copies of the host's `fbm()`/`ridged()` with the
  same constants, so going from 8 to 9 octaves adds one octave instead of switching
  normalization.
- **Band limit** (`{cut:true}` at compile time, `setNoiseCut(gl, prog, spacing)` at draw
  time, spacing in the units of `p`). Octave *i* gets weight
  `1 − smoothstep(0.25, 0.5, spacing·freq·lacunarityⁱ)` — whole at ≥ 4 samples per lattice
  cell, gone at 2. The edges are chosen from the sampling argument, not measured. Octave 0
  is never faded. The weight scales the octave and its share of the normalizer, so the
  averaging combinators (fBm, turbulence, ridged, billow, terraced) keep their expected
  value as octaves fade; hybrid MF and multifractal lose that octave's share of the mean.
  The domain/curl warp fields take the same fade. Not covered: the warp's own domain
  compression, remap sine/stripe frequencies, the white-noise base (no band at all), and
  kernel carriers. Fully faded octaves are not evaluated, which also keeps octave
  coordinates out of the range where fp32 stops resolving lattice cells.
- **fp32.** Scaling by the lacunarity keeps relative error, so a sample's position error in
  that octave's own pixels is the same for every octave — set by `|q|` at octave 0 against
  the pixel width — and folding per octave would not move it. The designer therefore limits
  zoom by the magnitude of the coordinate that varies across the screen (`precisionFloor`,
  8 ulps per pixel, a chosen margin); near the origin the limit falls with the view.
  New composite layers offset their seed along z only, which is constant on the tile.
- **Designer v002:** full device pixel ratio; wheel zooms at the cursor, drag pans, view
  height 1e-6…50 on a log slider, ⌂ back to the origin; the band limit fed with the pixel
  width (tile), the texel angle 2/R (sphere bake) or the output pixel (export); a readout
  of octaves resolved at the current spacing. Cube bake 256²…4096² per face (R32F: 4096²
  is 384 MiB), baked progressively in 512² chunks, 4 per frame. *Max detail* sets 16
  octaves on the edited noise (all layers in Composite) and turns the band limit on.
  *PNG16* exports a 16-bit greyscale heightmap (value −1…1 → 0…65535): a square of the view
  on the tile, equirectangular 2:1 on the sphere, rendered in strips and streamed through
  `CompressionStream` (encoder from № 21).

### In Orogeny v007

Composite presets appear in the **Terrain noise** selector (marked *· composite*) and
drive the starting terrain exactly as single presets do: `initProgramFor` compiles them
through `glslForAny`, which also emits an `evalNoise(p, np)` alias for the stack, so
`FS_INIT`'s `uMode == 4` branch is unchanged. A composite is used as authored — the
`P.nFreq` override applies to single presets only. The **orogen** paths (fold bake,
uplift noise) remain single-noise: they only ever read the compiled-in Mountains preset,
and `foldTargetProgramFor` / `upDivProgramFor` refuse a composite outright. The gradient
is ignored.
