# № 14 — Orogeny · Design Options & Decisions

Running log of every design fork raised while building this experiment, the option
chosen, and the alternatives left on the table. Kept so we can revisit and iterate.
**Bold = chosen.** ✅ = in v001, which is **built** (`Orogeny_v001.html`).

---

## 1. Subject

- **Seamless planetary erosion — a whole planet in erosional equilibrium** ✅.
  Uplift and erosion balance, so the sim never reaches an end state.
- Single continent on a plane — rejected, this is what every erosion demo already is,
  and it makes the seam problem (the actual experiment) disappear.
- Volumetric shell continuing № 11 — rejected. A 6×512²×64 shell is ~100 M voxels, and
  at planetary scale the overhangs it buys are sub-pixel. The material collapses to a
  radial heightfield and the budget goes to planform detail instead.

## 2. Manifold & storage

- **Cubed sphere, 6 faces** ✅ — no pole singularity, lifts № 02's chart machinery.
- **Equi-angular** parameterisation (`a = tan ξ`) ✅ over plain gnomonic: narrows the
  cell-area ratio across a face from **5.2×** (1 → 3^−³ᐟ² at the corner) to **√2 ≈ 1.41**
  (1 at the centre → 2/2^³ᐟ² at an edge midpoint). Reduces distortion; does not remove the
  need for metric terms.
- **Arc lengths measured, cell area analytic** ✅. Chord between adjacent cell centres is
  within 4e-7 of the arc at these sizes, so lengths are measured in 3D. Area is *not*:
  spherical excess (Van Oosterom & Strackee) is exact in exact arithmetic but cancels
  catastrophically in fp32 here — numerator ~1e-5 against a denominator ~4 — and left
  0.013 % error in Σ area. The analytic element `(1+X²)(1+Y²)·δ⁻³·Δξ²` lands within
  0.0001 %.
- **1-texel halo (ghost cells)** ✅ — *the actual fix*. Each face is stored at (N+2)² with
  a ring holding the neighbouring faces' values, refilled every pass (standard halo
  exchange from cubed-sphere climate codes). Hardware bilinear at a face edge then blends
  against true neighbour values instead of garbage, and every physics stencil neighbour is
  in-layer.
  Without it, a heightfield is bilinearly sampled everywhere (shading, raymarch, slope) and
  taps within half a texel of a tile boundary pull texels from the *adjacent tile*. In
  № 02's 3×2 layout the tile beside face 0 (+X) is face 1 (−X) — the opposite side of the
  planet. № 02 tolerates this because its bilinear path feeds a Jacobi solve that diffuses
  the error within one frame; here `h` **is** the rendered surface, the artifact is static,
  and the physics amplifies it (a notch collects flow → incises → deepens).
- **6-layer `TEXTURE_2D_ARRAY`** ✅ — *departure from № 02*, which uses a 3×2 atlas.
  The container, not the fix: filtering never crosses layers, so a stale halo degrades to a
  local edge clamp rather than a value from 180° away, and stencils are plain
  `texelFetch(src, ivec3(i, j, face), 0)` with no tile-origin arithmetic. An atlas with
  gutters between tiles would work identically — this is the tidier form of the same fix.
- **Halo lag.** Each pass writes its interior from the source's interior and its halo by
  resampling the source's *neighbour* interior, so the halo trails the interior by one
  substep. Accepted: the lag is symmetric across every seam (both faces lag each other
  equally), so it acts as slightly raised diffusion on a 1-texel ring rather than a
  one-sided bias, and it vanishes identically on a uniform field — which is exactly what
  the flat test in §4 checks.
- Lat-long grid — rejected, pole singularity is exactly what № 02 was built to avoid.
- Icosphere / geodesic grid — better isotropy, but 12 pentagon defects and no cheap 2D
  texture indexing. Reconsider only if cube-corner artifacts prove intractable.

## 3. Seam correctness (the core of the experiment)

Three requirements, all mandatory:

1. **Metric-correct operators** — real arc lengths in every gradient, Laplacian, flux and
   diffusion coefficient. Constant-`Δx` slope carves the six faces into the terrain.
2. **Area-weighted flux across seams** — receiving-cell area weighting, or mass leaks
   along the 12 cube edges and prints a ridge/trench per edge.
3. **Explicit 8-corner case** — valence-3 vertices, the grid's only irregular points.
   № 02's nearest-face-at-edges is adequate for a one-frame pressure solve and *not*
   adequate for erosion integrated over minutes.

Neighbour lookup: **reuse № 02's `neighborAtlas()` 3D round-trip** ✅ rather than
hand-coding 24 edge-adjacency cases — it handles corners for free. Costs a normalize +
re-project per out-of-face fetch. Hand-coded adjacency tables stay available if that
fetch becomes the bottleneck.

## 4. Verification — how "seamless" gets proved

Modelled on the GPU-readback checks that caught the sediment bugs in № 11. Not optional.

- **Flat test** ✅ — uniform shell + uniform uplift → stays uniform. **Measured: spread
  0.000e+0 %.** *Necessary, not sufficient*: on a uniform field every gradient is zero, so
  a wrong metric coefficient is multiplied by zero and stays invisible. It guards
  indexing, halo and sign errors only.
- **Isotropy test** ✅ — twin Gaussian bumps at a cube corner and a face centre, spread by
  **creep only**. **Measured: peak Δ 0.05 %, ring Δmean 0.04–0.12 %, azimuthal σ 0.02–0.08 %
  at the corner vs 0.00 % at the face centre.** This is the test that actually exercises
  the metric, since diffusion is driven directly by the weights.
  - ❌ First version eroded the bumps with droplets and normalised by a ring mean that
    passes through zero. It reported 271–405 % "anisotropy" at the *face centre* — it was
    measuring the stochasticity of drainage networks, which are azimuthally structured by
    nature. Rewritten as diffusion-only, normalised by bump amplitude.
- **Mass audit** ✅ — area-weighted total volume, including the load in flight, against
  `Σ|h|dA`. **Measured: area error 0.0001 %, drift −0.02 % over 10,000 steps.**
  - Denominator matters: bedrock and sediment nearly cancel, so `ΔV/|V|` reports nonsense
    about a near-zero baseline.

## 5. Erosion solver

- **Hybrid, staged** ✅ chosen. Mirrors FLIP's own P2G/G2P structure (the house method,
  № 02), and stage A validates the seamless atlas before any flow-accumulation solve
  exists.
  - **Stage A — droplets** ✅ v001. Particles as unit vectors with tangent velocity,
    geodesic advection reused verbatim from № 02, capacity `C = K_c·S·|v|·w`. Seam-free
    by construction — a droplet is a 3D direction and does not know faces exist.
  - **Stage B — grid incision.** Droplets splat discharge into `A` (this *is* P2G); the
    grid applies `K·A^m·S^n` + `D·∇²h`. Not in v001.

### 5b. Grant cycle — the grid arbitrates ✅ *(added during build, not foreseen)*

Droplets **declare** demand rather than taking material. Hundreds funnel through one
channel cell per step; each acting on the same stale height cut deeper than the whole
relief between them, the pit steepened, and the next step was worse. Measured before the
fix: 3 radii deep within 100 steps, and two runs from the same seed diverging 3.5×
because blend order had become the dominant input.

1. droplet declares demand or deposit; 2. splat scatters both (additive, PointSize 1 —
nothing spills into a halo, so seams conserve bit-for-bit); 3. apply grants a fraction of
the total, capped at 60 % of the drop to the lowest neighbour (zero in a pit, so pits fill
rather than dig); 4. a G2P pass hands each droplet its share.

The granted fraction is the **effective** one, hardness included, so the ground's loss and
the droplets' gain are the same number rather than two nearly-equal numbers. This turns
the solver into a genuine P2G/G2P cycle — the § 5 "hybrid" arrived a version early, and
by necessity.

### 5c. Numerical format of the splat ✅

- **RGBA32F via `EXT_float_blend`**, 16F + gain as fallback. A single droplet's demand is
  ~3e-5, under half-float's smallest normal (6.1e-5), so it fell in the sub-normal range
  the GPU flushes to zero: **63 % of eroding droplets registered no demand** yet still
  collected their grant. Mass out of nothing.
- **The no-demand case fails to ZERO, never to one.** Granting in full hands out material
  the ground never gave up — an unbounded source. Granting nothing is a bounded loss.
  Together these took the leak from **+31 % to −0.01 %**.
- Lagrangian droplets only — fastest to a good look, but less rigorous and prone to
  stippling. This is stage A without stage B, i.e. a subset of what we chose.
- Eulerian stream-power only — canonical, but needs the depression problem solved on the
  GPU up front. See §6.

## 6. Depressions / lakes — the main risk

Grid flow routing stalls in endorheic pits; the drainage network downstream never forms.

- Priority-flood depression filling (Barnes et al. 2014) — serial, hostile to a GPU.
- Iterative lake-level relaxation — Jacobi passes propagating a flat lake surface to the
  lowest spill point. Plausible on GPU, unproven here.
- **Droplets sidestep it** ✅ — a droplet reaching a pit deposits and dies, which is also
  what water actually does. **This is the reason the solver is staged droplets-first.**
  Revisit when stage B lands.

## 7. Uplift — ladder (rung 1 in v001)

Without a source term the planet flattens and the experiment dies.

1. **Noise uplift field** ✅ **v001** — static ridged-fBm orogenic mask on the sphere,
   domain-warped. Cheapest relief that persists. *Honest limitation:* ranges look
   plausible but arbitrary — they do not read as tectonics, because nothing put them
   where they are.
   - **Mean-zero (isostatic)** ✅ *(added during build)*. A closed sphere with a source and
     no sink can only inflate — mean radius grew at exactly the uplift rate, forever.
     Subtracting the area-weighted mean makes belts rise and everything else subside,
     which is also the more physical model: real uplift rearranges crust rather than
     creating it. It promotes total volume to a conserved quantity, which is what the
     mass audit now checks.
   - **Rate tied to droplet coverage** ✅ *(added during build)*. Erosive throughput is
     bounded by coverage — only a cell a droplet visits can be cut, so ~`PD²/6N²` of the
     grid is eligible per step — while uplift has no such bound. Tripling erodibility, and
     separately raising the capacity constant **240×**, moved equilibrium relief by under
     0.1 %; the balance is set by coverage alone. Scaling the uplift rate by it makes the
     balance hold at every grid and droplet count instead of needing a per-setting re-tune.
2. **Euler-pole plates, static boundaries** ✅ **v002**. Plate motion *is* a rigid rotation
   about an Euler pole, so this is the model that only exists on a sphere. Voronoi plates
   with angular velocity `ω`; boundary uplift = convergence rate `Δv·n̂`. Convergent →
   orogeny, divergent → rift. Plates do not advect, boundaries do not migrate: ~90% of the
   payoff for ~10% of the cost of rung 3.
   - **Margins for free** ✅ — the two nearest Voronoi seeds give distance-to-bisector as
     `(d₂−d₁)/2`, so no jump-flooding distance pass was needed.
   - **Per-plate crust type** ✅ — asymmetric response at ocean–continent margins (trench
     on the oceanic side, arc offset inboard), collision plateau continent–continent.
     The signature of subduction, not the mechanism.
   - **Continental base elevation** ✅ — continents start above ocean floor, standing in
     for the isostasy this model does not have.
   - *Measured:* margins 7.0 % of the sphere, 3.8 % convergent / 3.2 % divergent; field
     mean −0.002 before the mean-zero correction, i.e. convergence and divergence nearly
     balance unaided. Relief settles ~0.039 radii over 10,000 steps, drift −0.054 %.
   - ❌ **Hashed RGB for the plate diagnostic view** — clustered around mid-grey and read
     as a single colour across the whole globe. Replaced with a hue sweep plus a
     warm/cool continental–oceanic split, and margins widened from 0.014 to 0.030 rad
     because at 1 px they vanished under any downsampling.
3. **Full tectonics with advection** — v003+. Crust carried by plates, boundaries
   migrating, plates splitting/merging, crust age and subduction (cf. Cortial 2019).
   Genuinely a second experiment's worth of work.

## 8. Render — ladder (rung 1 in v001)

1. **Raymarched shell** ✅ **v001** — sphere-trace the displaced shell (`radius` vs
   `R + h(dir)`). No mesh, no LOD, no crack stitching; reuses № 11's marcher and shading
   structure. Costs: slower than a mesh, and thin ridges get skipped unless step count is
   high — same quality/step-count pairing № 11 exposes.
2. **Displaced cubed-sphere mesh, quadtree LOD** — v002. Vertex shader samples the height
   array; scales orbit → valley floor; shares the atlas with the sim directly. Needs
   crack-free stitching (skirts or edge-clamped tessellation).
3. **Mesh + procedural detail below texel size** — v003+. Fractal displacement beneath sim
   resolution so zoom never hits a floor; "seamless" in the zoom sense too. Caveat: the
   invented detail is not simulated and will not erode, so it must stay strictly below the
   scale at which the eye can check it against the drainage network.

## 9. Materials & shading

- **Two materials** ✅ — `.r` bedrock (detachment-limited), `.g` sediment
  (transport-limited, re-erodes readily). Same split as № 11, including its two
  mass-conservation traps: the floater/cull analogue must not annihilate loose sediment,
  and lateral repose must be symmetric at cliff edges.
- **Palette keyed to the physics fields** ✅ — bedrock/sediment, altitude, slope, wetness,
  as in № 11, so what you see is what eroded.
- **Lithification** ✅ *(added during build)* — sediment buried past a threshold compacts
  back to bedrock. Nothing removes sediment on a closed sphere, so without a return path
  the planet disappears under its own debris (mean sediment still climbing at 13,000
  steps). Mass-conserving; the blanket now plateaus.
- **Rivers from discharge** ✅ — water drawn where discharge exceeds a threshold, so rivers
  *are* the drainage network rather than a texture. The most direct visual proof the
  routing works. Discharge is stored in **multiples of average traffic**, so the threshold
  survives changes of grid and droplet count; at defaults it paints ≈4 % of cells partially
  and 0.4 % fully. (First calibration flagged **24 %** of the planet as river — the
  threshold had been set against an unnormalised scale.)
- Snowline, sea surface, atmospheric rim.

## 10. Base level

- **Global sea level `R_sea`** ✅ — incision stops at base level, deposition below it builds
  deltas, fans and shelves; coastlines emerge rather than being drawn. Absorbs the
  *coastal cliff* item from № 11's backlog at planetary scale.
- Oscillating sea level (transgression/regression cycles) — later; would cut marine
  terraces.

## 11. Resolution

| Grid | Cells | At Earth radius | RG32F/target |
|---|---|---|---|
| 6 × 128² | 98 k | ≈ 72 km/cell | 0.8 MB |
| 6 × 256² | 393 k | ≈ 36 km/cell | 3.1 MB |
| **6 × 512²** ✅ default | **1.57 M** | **≈ 18 km/cell** | **12.6 MB** |
| 6 × 1024² | 6.3 M | ≈ 9 km/cell | 50 MB |

Default dropped from 1024² to **512²** so first run is comfortable; 262 k droplets (512²)
by default. Ping-pong doubles the field cost. Memory-safe rebuild from № 11: out-of-memory
falls back to the last good size rather than black-screening.

---

## 12. Asteroid impacts ✅ v002

- **Mass-conserving crater** ✅ — parabolic bowl + central peak above the
  simple-to-complex size, the identical volume returned as a raised rim and an r⁻³ ejecta
  blanket out to 3R. Excavation takes sediment before rock; ejecta lands as sediment,
  which is what a breccia blanket is. Depth/diameter ≈ 1:9.
- **Ejecta scale solved numerically** ✅ — at strike time, on the sphere's own area element
  `2π·sin(a)·da`, with the outer taper included.
  - ❌ First version balanced the volumes analytically on a locally-flat plane. The taper
    cuts blanket the balance assumed was there, so every strike lost **0.04 % of the
    planet**: −0.156 % drift for one large crater, −1.04 % after 25. The numeric solve
    took that to **−0.0003 %** per crater and −0.0006 % after 40.
- **Bombardment rate** ✅ as a slider, with a power-law size distribution (small common,
  large rare) and a "strike now" button.
- *Known:* after heavy bombardment the erosion loop itself drifts more — −0.54 % over the
  next 2,000 steps against −0.02 % / 10,000 on undamaged terrain — because fresh rims are
  steep enough to drive the talus and grant clamps hard. A loss, not a source.
- *Resolution floor:* ~18 km cells at 6 × 512² and ~8 cells needed to read as a crater put
  the smallest honest impact at ~150 km. Basin-scale, not a cratered surface.

## 14. Crust column and isostasy ✅ v003

- **Terrain texture widened to (h, sed, tc, type); surface derived** ✅ — `h = uIsoK·tc`.
  Crustal thickness is primary and conserved; the surface is where the column floats.
  *Measured:* relief retained after 4,000 steps of pure erosion 84.2 % (rigid) → 88.3 %
  (default 0.35) → 90.7 % (true Airy 0.152).
- ⬜ *Bare damping factor on erosion* — one line, gets the rebound, but cannot express
  "continents are high because the crust is thick" and leaves subduction fake.
- ⬜ *Separate thickness texture* instead of widening the terrain texture — fewer bytes,
  more bindings, more sync.
- ⬜ *True Earth proportions, exaggerate only in the renderer* — honest units, but every
  erosion constant needs re-deriving.
- ⬜ *Flexural isostasy* — elastic plate spreading each load over a flexural wavelength
  instead of the local column. The physically better model.
- ⬜ *Separate sediment density* — sediment is lighter than crust and should float higher
  per unit thickness; both are treated alike today.

## 15. Advecting plates ✅ v003

- **Plate id as a field, backtraced by the TOTAL accumulated angle** ✅ against the
  immutable t=0 field, nearest sampling. Exact at any drift rate, and the label field
  never diffuses. *Measured:* 5.45 % of cells changed plate over 6,000 steps.
- ❌ **Incremental stepping against the previous frame** — never moved *at all*. At the
  default rate one rebuild backtraces 0.16 of a cell; nearest rounds it to the same cell,
  forever. A silent no-op that looked like a working feature.
- ⬜ *Raise the drift rate until incremental stepping clears a cell per rebuild* — works,
  but couples plate speed to grid resolution.
- **Uplift rebuilt from the field every 50 steps** ✅, margin distance from a local scan
  for a differing id — no jump-flood, no blur, one pass.
- ⬜ *Plate topology* — splitting, merging, vanishing. Geometry moves, topology does not,
  so an ocean can never fully close.
- ⬜ *ω from the current field rather than the t=0 field* — a cell that has changed hands
  is currently carried by its old plate.
- ⬜ *Mantle convection driving ω* instead of a fixed random draw.

## 16. Subduction ✅ v003

- **Type-driven rules on the advected field** ✅ — ocean–continent consumes the oceanic
  side and raises an arc inboard; ocean–ocean gives an island arc; continent–continent
  collides; divergent margins rift and create oceanic crust. *Measured:* mean oceanic
  thickness 1.5e-4 → −3.8e-4 over 6,000 steps.
- **The invariant changes, deliberately** ✅ — crust is created at ridges and destroyed at
  trenches, so crustal volume is genuinely not conserved and forcing a zero-mean source
  term would be wrong. The budget is reported instead: −0.42 % over 6,000 steps,
  consumption slightly outrunning creation, consistent with 3.8 % convergent against
  3.2 % divergent margins. Subduction off restores strict conservation.
- ⬜ *Crust age driving buoyancy* — the real mechanism (oceanic lithosphere goes
  negatively buoyant past ~20–30 Myr), and it would let the older plate subduct at
  ocean–ocean margins instead of type deciding.
- ⬜ *Plate-area bookkeeping against 4πR²* — nothing enforces that destruction matches
  creation.
- ⬜ *Slab dip setting the arc offset* from convergence rate, rather than a slider.

## 17. Test isolation ✅ v003

- ❌ The flat test started failing once drifting plates had been used: it writes a uniform
  uplift field, then the plate rebuild overwrote it mid-run. **The failure was real and my
  first diagnosis — a stale page load — was wrong.** Tests now pin `P.source` for their
  duration and restore it after.
- ⬜ *A rebound regression test* in the panel alongside flat/isotropy/mass. The
  84.2 → 90.7 % measurement is a one-off, not a guard.

## 13. Panel layout ✅ v002

- **Four panels on a pointer-transparent CSS grid** ✅ — container never eats a drag, each
  panel does. Rows `auto / 1fr / auto` so the side panels take exactly the height between
  the top bar and the footer. Every control id preserved from v001, so the wiring survived
  the move untouched.
- Verified: single-row top bar and 960 px of clear globe at 1440×900; at 1280×720 the top
  wraps to two rows and the side panels scroll.

---

## Backlog

- Stage B grid incision (§5).
- Uplift rung 2 + render rung 2 → v002.
- **Orographic precipitation** — rain shadows behind ranges, so erosion rate depends on
  the topography it created. Strong feedback, cheap once discharge exists. High value per
  line of code.
- Glaciation above a snowline — U-shaped valleys, a visibly distinct erosion signature.
- Cutaway / cross-section exposing strata and sediment thickness.
- Oscillating sea level (§10).
- Parameter presets, save/load.
