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

## 18. Margin scan reach ✅ v003.1

The uplift rebuild finds its margin by scanning outward for a differing plate id. The
first version scanned **±5 cells at stride 1**, and at 6 × 512² a cell is 0.0031 rad
while the belt is `bwidth` = 0.045 and the arc sits at `arcOff` = 0.056 — fifteen and
eighteen cells out. The window could not reach either.

❌ *What that cost.* The Gaussian only decayed from 1.000 to 0.944 before the hard edge of
the window, so the belt was a 5-cell stripe of near-constant height and **`bwidth` was
geometrically inert**; and `x = (db − arcOff)/w` never went positive, so the **volcanic
arc — the whole point of the ocean–continent asymmetry — did not exist**. *Measured:*
source coverage was 10.84 % of the sphere for `bwidth` ∈ {0.015, 0.045, 0.090} and for
`arcOff` ∈ {0.020, 0.056, 0.100} — the same number for every value of both sliders.
Both work correctly in the static-plate mode, where `db` is analytic and unbounded; the
bug arrived with the field rebuild.

- **Two-tier scan** ✅ — tier 1 stride 1 out to 5 cells, exact, and where the normal
  matters most; tier 2 strides the same 13×13 window by `K` for *direction only*, then a
  stride-1 march back along it recovers exact distance. `K = ⌈(arcOff + 2.5·bwidth)/
  cellArc / 6⌉`, so the reach follows the sliders — 10 at defaults, covering 60 cells.
  Worst case ~350 fetches against 169, at one rebuild per 50 steps. *Measured:* coverage
  now 36 / 80 / 99 % across the three `bwidth` values and 71 / 80 / 83 % across `arcOff`.
- **Seam-crossing lookup** ✅ — the old scan clamped to the layer, so it truncated against
  the face edge. Invisible at ±5, a groove along all twelve cube edges at ±60. Offsets
  that leave the face now go through `faceToDir` → `dirToFace`, nearest (bilinear would
  blend ids 3 and 8 into plate 5.5); interior offsets keep the plain `texelFetch` and pay
  nothing. *Measured:* mean-|u| at edge+0/1/2/4 cells agrees within 0.6 %, i.e. no groove.
- **True angle for the margin distance** ✅ — `acos(p·q)` rather than `best × π/2N`. The
  equiangular cell arc varies 1.41× across a face, which was below noise at 5 cells and is
  not at 60.
- ⬜ *Jump-flood distance field* — exact and unbounded, removing the caveat that tier 2 can
  miss a foreign strip narrower than `K` lying wholly outside tier 1. Costs log₂N extra
  passes and a seam-aware ping-pong. The strips only occur at triple junctions here, and
  cost an overestimated distance rather than a wrong one.
- ⬜ *Widening the halo to R texels* instead of routing out-of-face offsets through the
  direction — fewer ALU ops per sample, but the halo pass grows from 1 ring to 60 and
  every other field would carry the cost.
- ❌ *Raising the stride-1 radius to ~40* — 6,561 fetches per cell, a ~40 ms hitch every
  rebuild.

## 19. Source normalisation ✅ v003.1

Widening the belt widens every term at once, so the **total** tectonic throughput rode on
`bwidth`: fixing §18 multiplied the field's area-weighted mean-|u| by **2.74×** on its own,
and the crust budget went from −0.42 % to **+3.97 %** over 6,000 steps — the uncontrolled
inflation the mean-zero correction exists to prevent, coming back in through the
subduction gate.

- **Normalise by area-weighted mean-|u|** ✅ — folded into `uUplift`, so `bwidth` now
  redistributes uplift instead of adding it and the uplift slider keeps one meaning across
  every belt width. Mean-|u| rather than peak because the physically meaningful invariant
  is crustal flux per unit time, not maximum rate. *Measured:* budget +3.97 % → **+1.37 %**.
- ❌ *`upliftFromField` never measured its own field.* `UMEAN` was whatever the noise path
  last left behind — so in drifting mode with subduction **off**, the regime that exists to
  demonstrate strict conservation, the mean-zero correction was running on a stale constant.
  It now measures. *Measured:* −0.161 % over 6,000 steps.
- **Measure on geometry change, not per rebuild** ✅ — the readback is 6 × (N+2)² and a
  pipeline stall, which would roughly triple the cost of a rebuild. Margins move 5 % of
  cells in 6,000 steps, so the statistics are near-stationary between rebuilds — the same
  amortisation argument as `PLATE_EVERY` itself.
- ⬜ *Balancing creation against consumption directly*, by splitting the two into separate
  channels of the source texture and scaling the ridge term to match the measured trench
  term. That is §16's open plate-area bookkeeping, and it is the honest fix: the residual
  +1.37 % is unenforced, and its sign is now creation-dominated where it used to be
  consumption-dominated.

## 20. Boundary shape ✅ v003.2

Spherical Voronoi boundaries are **great-circle arcs** — exactly straight — so every
margin was a polygon edge. Seed jitter does not help: it moves the seed, not the shape of
the bisector.

- **Domain-warp the lookup direction inside `nearestPlates`** ✅ — two octaves, the low one
  cutting embayments at plate scale and the high one crenulating the edge. Placed inside
  the lookup so the plate field, the initial continents and the overlay agree for free,
  and so the drifting source inherits it through its own seeding. *Measured:* boundary
  crossings along a fixed set of great circles go 2199 → 2372 → 2580 for warp 0 / 0.18 /
  0.40; a straight-edged Voronoi holds constant, so the rise is added path length.
  Side effect, kept: the bisector distance is now measured in warped space, so belt width
  stretches and pinches along a margin instead of holding one value.
- **Ridged-fBm mask on the OROGENIC branches only** ✅ — a single Gaussian runs as a
  continuous welt; real ranges are chains of massifs with gaps and offsets. Trenches and
  rifts are genuinely continuous and are left alone. *Measured:* uplift volume −24 % and
  strong-uplift area −20 % at full strength, with the negative (trench/rift) volume
  unchanged at −0.3098 to four figures.
- ❌ *Four octaves with a symmetric remap* — averaging pulls ridged fBm toward its mean, so
  the mask barely reached zero and the belt lost only 7 % of its volume. A texture, not a
  break-up. Two octaves and a threshold biased above the mean.
- ⬜ *Break-up scaled by local convergence*, so orogens crenulate more than transforms.
  Needs the convergence rate before the warp is applied, which is circular in the current
  ordering.

## 21. Motion-vector source ✅ v004

The plate field stops being a label and becomes a **velocity**. Three passes: `v = ω × p`
blended across each margin → finite-volume divergence → crust-type rules on that
divergence. Chosen over extending the scan source, because boundary *shape* and the
physics *across* a boundary are orthogonal and the scan was the wrong tool for both.

- **Divergence as the mechanism** ✅ — −∇·v is the areal strain rate, so subduction and
  spreading are one quantity from a four-tap difference. **Retires the entire §18
  apparatus for this source**: no scan for a differing id, no radius, no two-tier window,
  no quantised normal.
- **Seeds move, so the Voronoi drifts** ✅ — rigid plates are rotations, so rotating each
  seed under its own pole drifts margins and triple junctions analytically. No id field,
  no advection pass to compound, and the margin distance stays exact and unbounded.
  *Measured:* 31.2 % of sampled directions changed uplift over 6,000 steps.
  *Caveat written down:* a Voronoi cell shrinks but never empties, because a seed always
  owns the points nearest it. Oceans narrow; they do not shut.
- **Belt width stops being a magnitude** ✅ — blending v over width w gives |∇·v| ~ conv/2w,
  so total consumption is convergence × margin length *independent of w*. That is the
  physically correct invariant, and precisely what the old Gaussian band got wrong — its
  integral scaled with w, which is why SRC_NORM had to exist (§19).
- ❌ **Centre-to-centre direction as the edge normal.** The equiangular grid is *not*
  orthogonal — ξ and η curves meet well off 90° away from a face centre — so it is not
  perpendicular to the edge. Caught by a control that has an exact answer: **one plate is
  a rigid rotation, which is divergence-free everywhere**. Background |∇·v| was 0.18
  against a true value of 0, and global closure −4.93 %. The normal is now p̂ × t̂ from the
  edge tangent: **0.00057 and 0.0001 %**.
- ❌ *Centre-to-centre distances as edge lengths* (`uM`) — right kind of quantity, wrong
  one: the east face's flux needs the length of that face, which runs north–south. They
  differ by the local metric anisotropy, worst at cube corners, so the error was
  structured and did not cancel.
- ❌ *Edge normal at the cell centre rather than the edge midpoint* — my first diagnosis of
  the −4.93 % closure, and it was **wrong**: the fix changed the number by nothing at all,
  which is what sent me to the rigid-plate control that found the real cause.
- ⬜ *Strike-slip* — the antisymmetric part of the same four-tap, free (task #4).
- ⬜ *Crust advection along VF* (task #2) and *crust age* (task #3). VF exists precisely so
  the advection pass has something to read.

## 22. Crust advection ✅ v004b

- **Semi-Lagrangian, bilinear, backtraced along the local flow** ✅ — `p·cos a − v̂·sin a`,
  exact wherever the field is a rigid rotation, which is everywhere but the shear zones.
  Bilinear rather than the id field's nearest: thickness is continuous, and nearest would
  freeze sub-cell motion exactly the way the v003 plate advection locked up.
  *Measured:* 11.65 % from a stationary copy, 3.50 % from its own backtraced self.
- **`h` recomputed, not carried** ✅ — it is derived from `tc` by isostasy, so transporting
  both would let them desync.
- **Bank the motion, spend it in one hop** ✅ — for **cost**, not accuracy. ❌ My stated
  reason was that a sub-cell resample smears as badly as a full one; bilinear error goes as
  t(1−t), so that is false. 80 passes retained 99.52 % of the field's spread against
  99.56 % for 17; a 1 → 16 cell threshold moved retention only 89.2 → 90.6 %. Banking buys
  4.7× fewer GPU passes and four hundredths of a percent.
- ❌ **Multiplicative compression exp(−∇·v·Δt)** — the textbook conservative form, measured
  at 0.000 % under a divergence-free flow, and **wrong for this variable**: `tc` is a signed
  deviation from a datum, so oceanic crust is negative and scaling it toward zero *adds*
  signed volume. Advection alone went +14.4 % at default drift, +45.5 % at maximum. Coded,
  disabled (`uComp = 0`), documented in place.
- ❌ *Gating compression by crust type* — tried both skipping oceanic thickening (budget
  +1.9 %) and actively subducting it (+2.98 %). Neither helps, because the fault is in the
  variable's definition and not in which cells the term is applied to.
- **Ship with compression off** ✅ — `tc·∇·v` stays with the divergence-driven uplift source,
  where it already lived. Relief 0.0347 against 0.0348 with advection off; budget +0.386 %
  at drift 0.5, −0.112 % at drift 2.0.
- ⬜ *An absolute column* — `tc` measured up from a datum thickness rather than about zero,
  which is what compression needs and what crust age (task #3) would supply.
- ⬜ *MacCormack / BFECC* to cut the residual smearing, at 2–3× the passes.

## 23. Tearing, sharpness and folds ✅ v004c

**Transport uses the RIGID per-plate velocity** ✅, not `VF.xyz`. That blend is C1 across
every margin — right for a finite divergence, wrong for moving rock, and with a 15-cell
shear zone against an 8-cell crater radius it pulled craters like taffy. A backtrace that
would cross a fault is halved until it stays on its own side. *Measured:* the height step
**across** a fault is now 7.13× the step inside a plate.

- ❌ *"The membership test fires precisely at divergent margins."* It does not. Only 53.1 %
  of the cells it fired on were divergent — a coin flip — with mean divergence −0.123,
  convergent-leaning. Plate velocity at a margin is mostly TANGENTIAL, so the trace slides
  along a curved boundary and crosses it whichever way the margin moves. And a Voronoi
  partition never has a literal gap. It is a fault clamp; new crust needs its own test.
- **New crust from areal expansion** ✅ — `∇·v·Δt`, no threshold to tune. *Measured:* mean
  `tc` by divergence bucket: −0.0168 convergent, −0.0081 interior, **−0.0227 / −0.0225** at
  5…20 and >100, against a target of −0.0229.
- ❌ *Ungated injection* also fired on 780,000 weakly-divergent cells, mostly continental
  interior, each stepping toward seafloor thickness every pass. Cost 1.4 points of budget.
  Gated to oceanic crust.
- **`NEW_CRUST_H` measured, not chosen** ✅ — −0.0063, the mass-neutral value, which lands
  above the ambient abyssal −0.0117: a crest shallower than the plain, which is what a ridge
  is. It leaves the uplift source as the single authority on ridge topography.
- ❌ **`advectCrust` never called `setPlates`**, so `plateOmega` used seed 0 and the crust
  moved on a different random draw than `VF`. Invisible to the scarp test, which only cares
  that the field is discontinuous per plate. Cost most of a −0.6 % budget regression.

**MacCormack with a limiter** ✅ — reverse trace computed inline, so one scratch texture
instead of two. The limiter is mandatory: the tear deliberately creates discontinuities and
unlimited MacCormack overshoots hardest there.

| crater rim amplitude retained, 6,000 steps | first order | MacCormack |
|---|---|---|
| default drift, 26 passes | 95.2 % | **99.3 %** |
| 8× drift, 206 passes, 65 cells travelled | 76.4 % | **95.3 %** |

- ❌ *Raising the resample threshold to 4 cells* — planned, then dropped. MacCormack made it
  pointless at 99.3 % retention, and it would have cost stair-stepping and misregistration.

**Carried shortening strain and folds** ✅ — new `SF` RGBA32F holding (ε, fold axis xyz).
Strain accumulates as convergence × displacement, the axis is captured from the divergence
gradient at formation, both advect with the crust, and everything is evaluated at the
MATERIAL position so the pattern is nailed to the rock. `beltRough` moved into the same
frame — it had the identical world-fixed flaw.

- The axis is a **3-vector in material space**, not an angle in a local basis: angles wrap,
  and blending two of them while advecting is nonsense wherever the basis flips.
- ❌ *Multiplicative on the orogenic uplift* — the obvious form, and wrong. Strain is carried
  so it spreads over 15.8 % of the sphere; `u` is concentrated where `|∇·v|` is large. The
  product is non-zero only at the margin, so folds appeared as a thin line: source
  high-frequency content +39 %, terrain roughness +3 %. Additive and driven by strain alone.
- ⚠️ **`foldAmp` is in the SOURCE's units**, where belt values run to ~3. 1.6 was invisible;
  the default is 14. *Measured:* belt-over-surroundings roughness 3.345 → 4.306 at amplitude
  30, 2.995 at the default.
- ⬜ **Anisotropy is UNVERIFIED.** The domain elongation along the stored axis is implemented
  at their tuned ≈0.85/0.15 ratio, but no measurement I ran could show the folds are
  directional: 1.521 vs 1.515 at active margins with amplitude 30. Either the instrument
  (one-cell differences against the divergence gradient, on a field dominated by drainage
  and belt slope) is too crude, or the elongation is not surviving. **Do not claim
  directional folds until this is measured properly.** Phasor noise (§ deferred) would make
  the direction explicit rather than emergent and would be testable.

## 24. The blend profile IS the divergence ✅ v004d

The velocity blend across a margin was `smoothstep(0, w, db)`. Divergence is the GRADIENT
of that blend, and **smoothstep has zero derivative at its endpoints** — so there was a dead
line of zero divergence running along the axis of every margin, with the real peak displaced
to `db = w/2` on both sides. Visually: a double band with a neutral stripe down the middle.

*Measured*, mean |∇·v| against distance from the margin, 14.7-cell belt:

| cells from margin | smoothstep | tanh |
|---|---|---|
| 0 (axis) | **1.81** | **5.53** |
| 4 | 5.13 | 5.56 |
| 8 | **6.40** ← peak | 4.82 |
| 15 | 4.32 | 3.50 |

The axis carried **less than a third** of what its own flanks did. Not merely ugly: it
starved the ridge axis of the extension that drives crust creation and thinning, and is a
likely contributor to §12's stalled thinning.

- **tanh** ✅ — maximum slope at zero, so divergence peaks where the plates actually meet.
  Still continuous across the margin (db → 0 gives ½(v1+v2) from either side) and smoother
  than smoothstep was, since sign(s)·tanh(|s|/w) is analytic where sign(s)·S(|s|/w) is only
  C¹. Asymptotes rather than saturating at w, which is harmless — tanh(3) = 0.995.
  Compression/extension area split also went symmetric, 5.6/5.0 % → 4.9/4.9 %.
- ⚠️ The irony worth keeping: the C¹ blend was chosen deliberately in §21 to avoid a delta
  function in the divergence. It worked, and the cost was a zero exactly where the signal
  should have been strongest. **Choosing a blend profile is choosing the divergence profile**
  — they are the same decision, and the first one was made looking only at the blend.

## 25. Fold texture: warped, not bare ridged ✅ v004d

Domain-warped ridged noise instead of `ridged(x, 3)`: warping the sample position by a
second noise field before taking the ridge is what produces long sinuous branching crease
lines rather than lumps. *Measured:* belt-over-surroundings roughness **2.995 → 4.065** at
identical amplitude. The generator was not new — the v001 noise-belt source has used it
since the beginning, with its own comment reading "long sinuous belts rather than isotropic
blobs". Reaching for the unwarped one is very likely why the first fold attempt read as
roughness rather than folding.

## 26. Extension thins toward ocean floor ⚠️ v004d — implemented, ineffective

Divergent margins relax `tc` toward the abyssal datum at a rate set by the extension, so a
rift becomes a basin becomes ocean, all on one continuous scale with no hole for isostasy to
divide by. **It does not reach its target.** Extensional ground settles at −0.0030 against
−0.0117; raising the rate 500× moved it to −0.00304 while dropping belt roughness
4.065 → 3.215 and flipping the budget +0.512 % → −0.409 %. Reverted to the low rate.
Cause and the measurement bug behind it are recorded in task #12.

## 27. Orogen width: blend → echo → directional propagation ✅ v004e

Three attempts at giving a sharp margin a wide orogen, two dead:

- **Wide velocity blends** ✗ — per-type widths (`blendC`/`blendD`) corrupt plate
  interiors: overlapping transition zones make a rigid rotation (exactly divergence-free
  by construction) read |∇·v| up to 0.31. The control experiment that caught it: interiors
  must be silent, measured 0.00 → 0.31 as blendC went 0.05 → 1.95. Both widths now 0.
- **Echo (decay at read time)** ✗ — `exp(-db/L)` applied in FS_UPDIV never touches the
  fields, so interiors stay clean, but it has no inherent direction; a bolted-on
  "faster plate wins" test faked sidedness poorly.
- **Directional propagation** ✅ — seed the compression spike sharp, then march it along
  the velocity field with step-doubling (6 passes reach 63 cells), gated by BUOYANCY:
  continental overrides oceanic, faster wins between like types. Direction of travel alone
  cannot pick the side — at a convergent margin BOTH plates move toward it; measured
  symmetric (1.01–1.08×) until the explicit gate went in. True asymmetry 4.9–6.0×, but
  only after fixing the instrument: averaging the two sides across ~800 crossings before
  taking max/min hides asymmetry whose sign flips per crossing — classify strong/weak PER
  CROSSING first. The earlier echo's "1.4×" was measured with the broken instrument and
  was never a real number.

`smoothstep` → `tanh` for the residual blend profile: S′(0)=0 put a divergence dead-zone
exactly on the margin axis (the "neutral pressure ridge in the middle"); tanh has maximum
slope at zero. Axis mean|div| 1.81 → 5.53.

## 28. Two pressure channels, two clocks ✅ v004e

- **PG is RG32F: .x compression (propagated), .y extension (computed at seed, carried
  through every propagation pass untouched)** — verified bit-identical across reach
  sweeps. Extension stays a thin sharp line; compression gets the wide gradient. One
  channel per phenomenon, so work on one cannot disturb the other.
- **`plateAngle` vs `crustAngle`** — seed migration (Voronoi geometry deforming, "Motion
  evolution", default 0) is a different clock from crust transport ("Plate speed").
  Conflating them made the landmass speed and the boundary-deformation speed one knob.
  Material-frame functions follow `crustAngle` — the rock, not the seeds.

## 29. Noise library: specialization, not dispatch ✅ v004f

One shared GLSL library (`noise-lib.js`) feeds the designer page and the injected shaders
in the main tool. The first version dispatched 9 bases × 8 combinators with runtime `if`s
in one shader; GLSL compilers inline every branch, so `evalBaseUnit` appeared 83 times and
each instance unrolled the 27-cell × 8-impulse kernel scaffold — ~5×10⁴ inner bodies.
**It crashed the GPU process and took every later WebGL2 context with it.** Base,
cellular output, combinator, modifiers and remap are now preprocessor defines; exactly one
of each survives. Swept all 4,480 combinations offline through a preprocessor evaluator:
one base body each, largest shader 167 lines. Two supporting rules: the domain warp uses a
fixed cheap value-fBm rather than the selected base (a warp wants smooth and cheap), and
the finite-difference derivative is compiled only on request (it is a silent 7× on
everything).

Companion architecture: `noise-designer.html` (standalone page, tile + relief views,
special simulated modes), `impact-designer.js` (in-document modal; 9 crater-shape params
threaded through FS_IMPACT *and* the CPU mass-balance integral together, or the ejecta
solve silently diverges from what is rendered), `preset-store.js` (File System Access API
folder of .json files with localStorage fallback; the filesystem has no change events, so
writes bump a localStorage doorbell that fires `storage` in other tabs), and
`presets-builtin.js` (compiled-in presets that work from file:// with no picker).

## 30. Folding is baked, not grown ✅ v004g — replaces §23's fold texture

The user's requirement: ranges exist the moment the pressure does. Uplift is a RATE — it
can only accumulate, and measured it grew the peak 9 % in 1,200 steps. So the orogen's
fold displacement is now a FUNCTION of the current pressure field, written into crustal
thickness as a DELTA against what the previous bake wrote (the FD texture pair records
it). Delta, not absolute: 100 consecutive re-bakes drift the peak by exactly 0.0.
Setting the height to 0 removes precisely what was added. Isostasy stays sole owner of
the surface (`h = isoH(tc)` throughout), and erosion sees the ranges as real crust.

Thresholds are measured, not chosen: the propagated pressure is violently long-tailed
(240k samples: p90 = 0, p99 = 14.6, max = 300), so the bake gates on
`smoothstep(1, 25, PG.x)` — the top ~2–3 % of the sphere, which is the orogen. A linear
map showed nothing.

The noise itself is the compiled-in Mountains preset (cellular F1×F2, Chebyshev metric,
billow, warped, folded), hardcoded — not a dropdown — sampled at the material position so
the pattern rides the crust.

**Retired with this:** §23's strain-driven `foldTerm` texture, and the entire carried
strain/fold-axis field (SF pair, `strainK`, the second render target on the advection
pass). Three mechanisms were answering "what does compression look like", two of them
sampling the same noise; the bake is immediate, reversible and mass-accounted, so it won.
The advection MRT reverts to a single output.

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
