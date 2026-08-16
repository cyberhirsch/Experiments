# № 14 — Orogeny

**Seamless planetary erosion — a whole planet held in erosional equilibrium on S².**

Tectonic uplift raises relief, rivers cut it back down, sediment fills the basins and
builds deltas at sea level. Because uplift and erosion balance, the simulation has no
end state: it runs indefinitely rather than decaying to a smooth sphere.

*Status: **v003 built** — `Orogeny_v002.html`, single file, WebGL2, no dependencies.
v001 was droplet erosion on a noise uplift field; v002 added the four-panel layout,
Euler-pole plates and impacts; v003 adds the crust column with local Airy isostasy,
advecting plates and subduction. See [ROADMAP.md](ROADMAP.md).*

---

## v002 additions

**Four-panel layout.** Top: viewing, resolution, simulation. Left: tectonics and impacts.
Right: erosion, with a Fluids slot. Bottom: verification and readout. A pointer-transparent
CSS grid over the canvas, so the container never eats a drag and each panel does.

**Euler-pole plates** — selectable as the uplift source. Euler's rotation theorem makes a
rigid plate on a sphere *exactly* a rotation about an axis through the centre, so plate
motion is `v(p) = ω·(ω̂ × p)` and surface speed is `ω·sin Δ` from the pole. Relative motion
between two plates is itself a rotation: poles subtract as vectors. Voronoi partition, and
the two nearest seeds hand you the margin for free — half the difference of the geodesic
distances is the distance to their bisector, so no jump-flooding pass was needed.
Convergence drives orogens, divergence rifts, and per-plate crust type gives trenches with
inboard arcs at ocean–continent margins. Evaluated once at init into the same uplift
texture the noise field fills, so the per-step cost is unchanged.

**Impacts** — bowl and central peak excavated, the same volume returned as rim and an r⁻³
ejecta blanket, landing as sediment. The ejecta scale is solved numerically against
`2π·sin(a)·da` at strike time rather than analytically on a flat plane; that correction
took the drift per crater from −0.156 % to −0.0003 %.

Both are logged with their measurements in [ROADMAP.md](ROADMAP.md) and
[DESIGN_OPTIONS.md](DESIGN_OPTIONS.md).

---

## v003 additions

**The surface stopped being the primary variable.** The terrain texture is now
`(h, sed, tc, type)` and `h = uIsoK·tc` — crustal thickness is what is conserved, the
surface is wherever that column floats. Tectonics thickens the column; erosion removes
mass and only `uIsoK` of it reaches the surface. That asymmetry is isostatic rebound, and
it is why mountain ranges persist. Measured with uplift off over 4,000 steps of pure
erosion, relief retained goes **84.2 % rigid → 88.3 % at the default → 90.7 % at true
Airy (0.152)**.

**Plates drift.** The plate id is a field now, carried by its own plate's rotation, with
uplift rebuilt from it every 50 steps. 5.45 % of cells changed plate over 6,000 steps.
The first implementation stepped incrementally and never moved at all — one rebuild
backtraces 0.16 of a cell and nearest sampling rounds that away, forever. Backtracing by
the *total accumulated angle* against the immutable t=0 field is exact at any rate and
never diffuses the labels.

**Subduction consumes.** Oceanic crust thins where it meets a continent (mean oceanic
thickness 1.5e-4 → −3.8e-4 over 6,000 steps) and new crust appears at ridges.

**This changes the invariant, deliberately.** Crust created at ridges and destroyed at
trenches means crustal volume is genuinely not conserved, so the mass audit reports the
budget rather than forcing it to zero: **−0.42 %** over 6,000 steps, consumption slightly
outrunning creation — the expected regime given margins run 3.8 % convergent against
3.2 % divergent. Turning subduction off restores v002's strict conservation.

*Gotcha worth keeping:* the flat test began failing after the drifting-plate mode ran,
because the test writes a uniform uplift field and the plate rebuild then overwrote it
mid-run. The test now pins the source for its duration. My first instinct was to blame a
stale page load; re-testing showed a real ordering bug.

## v003.1 — two features that were never actually running

**The margin scan could not reach the features it draws.** The uplift rebuild found its
margin by scanning ±5 cells at stride 1. At 6 × 512² a cell is 0.0031 rad while the belt
is 0.045 and the arc offset 0.056 — fifteen and eighteen cells out. So the Gaussian only
decayed from 1.000 to 0.944 before hitting the hard edge of the window, and the arc's peak
lay outside it entirely. Source coverage measured **10.84 % of the sphere for every value
of `bwidth` and every value of `arcOff`**: both sliders moved nothing, and the volcanic
arc that is the entire point of the ocean–continent asymmetry did not exist. Neither
failed loudly — the belt looked like a belt, so nothing suggested the width control was
disconnected from it.

Now a two-tier scan: stride 1 out to 5 cells for the exact near field, then the same
window strided for direction only and a stride-1 march back in for exact distance.
Coverage moves 36 → 80 → 99 % with `bwidth`. The scan also used to clamp to the layer,
truncating against the face edge — harmless at ±5, a groove along all twelve cube edges at
±60 — so out-of-face offsets now route through the direction, nearest.

**Ridges were trenches.** The divergent branch summed −0.55·|conv|·band of rift against
+0.30 of crust creation. The graben won, so spreading centres *thinned* the crust while
the comment above them called that term the one that legitimately adds mass. A mid-ocean
ridge is the shallowest part of an ocean basin. Rewritten with the two length scales it
needs — a broad positive swell carrying a narrow axial graben — 1,491 of 2,946 ocean–ocean
crossings now show the ridge profile, against 761 with subduction off.

**Which broke the budget, twice over.** Letting the belt reach its designed width
multiplied the source's mean-|u| by 2.74× on its own, and the crust budget went −0.42 % →
**+3.97 %** over 6,000 steps. The source is now normalised by its own mean-|u|, so belt
width sets shape and not amount: **+1.37 %**. And `upliftFromField` turned out never to
have measured its own field — `UMEAN` was inherited from the noise path, so drifting mode
with subduction *off*, the regime that exists to demonstrate strict conservation, had been
subtracting a stale constant. Measured now: **−0.161 %** over 6,000 steps.

The remaining +1.37 % is unenforced, and its sign has flipped from consumption-dominated
to creation-dominated. Closing it properly is the plate-area bookkeeping already on the
roadmap.

## v004 — the plate field becomes a velocity

Two visual complaints turned into one architectural change.

**Straight edges.** A spherical Voronoi boundary is a great-circle arc — exactly straight —
so every margin was a polygon edge, and no amount of seed jitter changes that, because
jitter moves the seed and not the shape of the bisector. Warping the lookup *direction*
bends them, and doing it inside `nearestPlates` means every consumer inherits it from one
place. A ridged mask on the orogenic branches only turns the continuous welt into a chain
of massifs; trenches and rifts really are continuous, so they are left alone.

**Motion vectors.** The plate field stops being a label and becomes `v = ω × p`, blended
across each margin, and the uplift comes from **−∇·v** — the areal strain rate, so
subduction and spreading are one quantity from a four-tap difference. `ω × p` rather than a
constant vector per plate, because rigid motion on a sphere *is* a rotation and a constant
vector would shear every plate interior.

This retires the whole margin scan for the new source, and fixes the belt-width coupling at
the root: blending over width `w` gives `|∇·v| ~ conv/2w`, so total consumption is
convergence × margin length **independent of `w`**. The old Gaussian band's integral scaled
with `w`, which is the entire reason the source needed normalising a version earlier.
Seeds move under their own pole, so the Voronoi drifts analytically — no id field, no
advection step to compound, and margin distance stays exact at any range.

*The gotcha, including a wrong turn.* ∮∇·v dA is exactly zero on a closed surface, and it
measured **−4.93 %**. My first explanation was that the edge normal sat in each cell's own
tangent plane rather than at the shared midpoint. That was wrong, and the fix changed the
number by nothing at all — which is what pushed me to build a control with an exact answer:
*one plate is a rigid rotation, and a rotation is divergence-free everywhere*. It showed a
background |∇·v| of 0.18 against a true value of zero. The real causes were that the edge
*lengths* were centre-to-centre distances rather than the cell's actual edges, and the edge
*normal* pointed centre-to-centre — which is not perpendicular to the edge, because the
equiangular grid is not orthogonal away from a face centre. Rebuilding both from the
shared corners took the rigid-plate background to **0.00057** and closure to **0.0001 %**.

Creation and destruction now balance to machine precision by construction. That was the
argument for the architecture; it is now a property of it rather than a claim about it.

## v004b — the crust rides the plate

The rock moves now. A crater drifts with its plate instead of sitting still in space while
the margins slide past it. Semi-Lagrangian backtrace along the local flow, bilinear —
thickness is continuous, and nearest sampling would freeze sub-cell motion exactly the way
the v003 plate advection locked up. Against a stationary copy the field differs by 11.65 %;
against its own analytically backtraced self, by 3.50 %. With advection off it does not
move at all (0.01 %), which is also the proof that it never did before.

Three wrong turns, none of which inspection would have caught:

**Banking motion helps cost, not accuracy.** I asserted a sub-cell resample smears as badly
as a full one. Bilinear error goes as t(1−t), so small hops are individually gentle: 80
passes retained 99.52 % of the field's spread against 99.56 % for 17. Banking stayed, for
4.7× fewer GPU passes.

**Pure backtracing loses 56 % of the crust.** It moves crust without compressing it, so
converging plates pack it into less area at unchanged thickness. The rigid-plate control
again: under a divergence-free flow the same code conserved to 0.000 %, which located the
fault in the flow rather than the scheme.

**And the textbook fix for that was also wrong.** Scaling the column by exp(−∇·v·Δt) is the
conservative form, and it too measured 0.000 % under rigid rotation — but `tc` is a *signed
deviation from a datum*, not an absolute thickness, and oceanic crust is negative.
Multiplying a negative column by a factor below one moves it toward zero, which *adds*
signed volume. Thinning oceanic crust at a trench made the planet gain mass: +14.4 % at
default drift, +45.5 % at maximum. Gating by crust type cannot repair a sign error in what
the variable means. The term sits in the shader, disabled, with the reasoning next to it.

Shipped with compression off, leaving `tc·∇·v` to the divergence source where it already
lived: relief 0.0347 against 0.0348 with advection disabled, budget +0.386 % at drift 0.5
and −0.112 % at drift 2.0. Doing it properly wants `tc` measured up from a datum thickness,
which is what a crust-age field would supply.

---

## Where it sits in the collection

| | Domain | Material | Method |
|---|---|---|---|
| № 02 Gas Giant | S² cubemap atlas | fluid *above* the surface | PIC/FLIP |
| № 11 Erosion | 128³ box with a floor | rock, volumetric | SDF raymarch + dissolution |
| **№ 14 Orogeny** | **S² cubed sphere** | **rock, as a radial shell** | **droplet + grid arbitration** |

№ 14 is the Gas Giant's manifold carrying № 11's subject. A closed manifold is not
merely a different shape — it removes the domain walls, so every drainage basin is
interior, water has nowhere to leave, and mass is exactly conserved on the sphere. That
last property is the whole reason the mass audit is worth having: on a closed sphere it
is a real invariant, not a bookkeeping exercise.

At planetary scale the overhangs that motivated № 11's volumetric field are invisible,
so the material collapses to a radial heightfield `h(dir)` and the resolution budget
buys planform detail instead.

---

## The seamless problem

"Seamless" here is a property of the **operators**, not the renderer.

### 1. The metric

On a cubed sphere with face coordinates `(a,b)`, the area element carries a factor
`(1 + a² + b²)^(-3/2)`. For a plain **gnomonic** grid (`a` uniform) that means cell area
varies **5.2×** across one face — 1 at the face centre, `3^(-3/2)` = 0.192 at the corner.
We use the **equiangular** grid instead (`a = tan ξ`, `ξ` uniform on `[-π/4, π/4]`), whose
area element is `(1+X²)(1+Y²)·δ⁻³`. Its extremes are 1 at the centre and `2/2^(3/2)` =
0.707 at an edge midpoint, so the ratio falls to **√2 ≈ 1.41**.

Distortion is reduced, not removed, so every gradient, flux and diffusion coefficient
still uses real lengths. Arc lengths are **measured** — the chord between adjacent cell
centres in 3D, since `θ ≈ π/(2N) ≲ 3e-3 rad` makes the chord/arc error `θ²/24 ≲ 4e-7`.

**Cell area is not measured.** The obvious route — spherical excess over the cell's four
corners (Van Oosterom & Strackee) — is exact in exact arithmetic and useless in fp32 at
these sizes: the numerator is ~1e-5 while the denominator is ~4, and the cancellation
left a **0.013%** error in total area. The analytic area element above is used instead
and the same sum lands within **0.0001%** of 4πR².

### 2. Conservative flux across seams

Water and sediment leaving one face must arrive on the next weighted by the receiving
cell's area, or mass leaks along the 12 cube edges. Everything moves as **volume**:
droplets carry volume, the splat scatters volume, the apply pass divides by the local
cell area. Talus and creep exchange volume through fluxes with a **symmetric**
coefficient (`√(A_i·A_j)/d` as the edge-length proxy), which is what makes them
conservative regardless of how the two cells differ in size.

### 3. The 8 corners

A cubed sphere trades the 2 pole singularities of a lat-long grid for 8 valence-3
vertices. They are the only irregular points on the grid and they do not average away.

### The halo

The mechanism that makes all of this work is a **1-texel halo of ghost cells**: each face
is stored at (N+2)² and every pass rewrites the ring by resampling the neighbouring face.
Two consequences:

- every physics stencil neighbour is in-layer, so all of them are plain
  `texelFetch(src, ivec3(i, j, face), 0)` — no tile arithmetic, no special cases, corners
  included;
- hardware bilinear at a face edge blends against true neighbour values.

Without it, bilinear taps within half a texel of a boundary pull texels from whatever is
adjacent in memory. № 02 tolerates that because its bilinear path feeds a Jacobi solve
that diffuses the error within one frame; here `h` **is** the rendered surface, the
artifact is static, and the physics amplifies it — a notch collects flow, flow incises,
the notch deepens. Twelve edges of self-amplifying scar.

The halo trails the interior by one substep (each pass fills its halo from the *source's*
interior). The lag is symmetric across every seam, so it acts as slightly raised
diffusion on a 1-texel ring rather than a one-sided bias, and it vanishes identically on
a uniform field.

Fields live in 6-layer `TEXTURE_2D_ARRAY`s rather than № 02's 3×2 atlas. That is the
container, not the fix — an atlas with gutters would work identically — but filtering
never crosses layers, so a stale halo degrades to a local edge clamp instead of a value
from 180° away.

---

## Verification

Three tests ship in the panel. Measured on the default 6 × 512² grid:

| Test | Result |
|---|---|
| **Flat** — uniform shell, uniform uplift, 240 steps | spread **0.000e+0 %** of mean |
| **Isotropy** — twin bumps at a cube corner and a face centre, creep only, 400 steps | peak Δ **0.05 %**; ring Δmean **0.04–0.12 %**; azimuthal σ **0.02–0.08 %** (corner) vs **0.00 %** (face) |
| **Mass** — Σ area, and ΔV against Σ\|h\|dA | area error **0.0001 %**; drift **−0.02 %** over 10,000 steps |

Two honest notes on what these do and do not prove:

- **The flat test is necessary, not sufficient.** On a uniform field every gradient is
  zero, so a wrong metric coefficient is multiplied by zero and stays invisible. It
  catches indexing, halo and sign errors that would inject asymmetry — nothing more.
- **The isotropy test had to be rewritten.** The first version eroded the bumps with
  droplets and reported 271–405 % "anisotropy" at the *face centre* — worse than the
  corner. It was measuring the stochasticity of drainage networks, which are azimuthally
  structured by nature, and normalising by a ring mean that passes through zero. Running
  it with **creep only** isolates the geometry: diffusion is driven directly by the metric
  weights, so a wrong one distorts the bump into a visibly non-circular shape. The
  face-centre bump comes out perfectly axisymmetric (σ = 0.00 %, as it must at a symmetry
  point) and the corner bump within 0.08 % of it. That number *is* the seam claim.

---

## Physics

```
∂h/∂t = U − (fluvial incision) + D·∇²h + talus + lithification
```

- **Uplift `U`** — a domain-warped ridged-fBm belt field, **mean-zero**. See below.
- **Fluvial incision** — Lagrangian droplets, arbitrated by the grid. See below.
- **Hillslope creep `D·∇²h`** — volume flux with a symmetric coefficient. The `dx²`
  normaliser lives in the coefficient so the slider stays inside the explicit-diffusion
  stability limit at every resolution.
- **Talus** — antisymmetric volume exchange above an angle of repose; both cells of a
  pair compute an identical magnitude from identical inputs, and the giver's stock is
  capped at ⅛ (not ½ — a cell can be the giver to all four neighbours at once, and
  clamping a negative result afterwards invents sediment).
- **Lithification** — sediment buried past a threshold compacts back to bedrock.
- **Base level** — a sea-level radius; deposition below it builds shelves and deltas.
- **Two materials** — `.r` bedrock, `.g` sediment, as in № 11.

### Droplets, and why the grid arbitrates them

Each droplet takes one step per substep, so it traces a continuous path over many
frames — a persistent rain rather than a burst. Gradients are measured in world arc
length along a tangent basis at the droplet's position, which makes the step
metric-correct on every face and across every seam without touching the metric texture
at all. A droplet is a 3D direction and does not know faces exist.

Droplets **declare** rather than act. Hundreds can funnel through one channel cell in a
step, and if each acted on the same stale height they would between them cut deeper than
the entire relief — the pit then steepens and the next step is worse. Measured before
the fix: a single cell reached 3 radii deep within 100 steps, and two runs from the same
seed diverged by 3.5× because floating-point blend order had become the dominant input.

So the pipeline is a P2G/G2P cycle — the same structure as № 02's FLIP:

1. the droplet declares a **demand** (volume it wants) or a **deposit**;
2. the splat scatters both into the grid (additive, PointSize 1, so nothing spills into a
   halo and mass is conserved bit-for-bit at seams);
3. the apply pass grants a fraction `g` of the total demand, capped so no cell is cut
   more than 60 % of the way down to its lowest neighbour in one step — in a pit the
   allowance is zero, so pits fill instead of digging, which is also what water does;
4. a G2P pass hands each droplet `g ×` its own demand.

`g` is the **effective** fraction, hardness included, so what the ground loses and what
the droplets gain are the same number rather than two nearly-equal numbers.

---

## Gotchas found while building

Recorded in the spirit of № 11's sediment traps — each of these cost a debugging pass.

**Half-float sub-normals silently created mass.** A single droplet's demand is ~3e-5,
below half-float's smallest normal (6.1e-5), so it landed in the sub-normal range that
the GPU flushes to zero. **63 %** of eroding droplets registered no demand at all, while
still reading the grant field and collecting in full. Two fixes, both needed: accumulate
the splat in **RGBA32F** (via `EXT_float_blend`, with a gained 16F fallback), and make
the no-demand case **fail to zero, never to one** — granting in full hands out material
the ground never gave up, an unbounded source; granting nothing is a bounded loss. Leak
went from +31 % to −0.01 %.

**A closed sphere with a source and no sink can only inflate.** Mean radius grew at
exactly the uplift rate, forever. Subtracting the area-weighted mean from the uplift
field fixes it and is also the more physical model — real uplift is isostatic, crust
rearranged rather than created. It also promotes total volume to a conserved quantity,
which is what the mass audit now checks.

**Nothing removes sediment on a closed sphere**, so the planet slowly disappeared under
its own debris (mean sediment still climbing after 13,000 steps). Lithification gives it
a return path, mass-conserving, and the blanket now plateaus.

**Erosive throughput is bounded by droplet coverage, not by erodibility.** Tripling
erodibility, and later raising the capacity constant 240×, changed the equilibrium relief
by less than 0.1 % — only a cell a droplet visits can be cut, so ~`PD²/6N²` of the grid is
eligible per step. Uplift has no such bound. Tying the uplift rate to coverage makes the
balance hold at every grid and droplet count instead of needing a re-tune for each.

**A drift percentage needs an honest denominator.** Bedrock and sediment very nearly
cancel, so `ΔV / |V|` reported alarming numbers about a baseline near zero. `Σ|h|dA` is
the meaningful scale. The same mistake, in a different guise, is what made the first
isotropy test useless.

---

## Budget

| Grid | Cells | At Earth radius | RG32F per target |
|---|---|---|---|
| 6 × 128² | 98 k | ≈ 72 km/cell | 0.8 MB |
| 6 × 256² | 393 k | ≈ 36 km/cell | 3.1 MB |
| **6 × 512²** default | **1.57 M** | **≈ 18 km/cell** | **12.6 MB** |
| 6 × 1024² | 6.3 M | ≈ 9 km/cell | 50 MB |

Default 262 k droplets (512²). Out-of-memory falls back to the last good size rather than
black-screening, as in № 11.

---

## Controls

Tectonics (uplift rate, belt scale, belt sharpness) · erosion (rain, erodibility,
deposition, rock hardness, inertia, evaporation, hillslope creep, talus angle, sea level,
time step) · resolution (grid, droplets, render quality) · view (five shading modes
including a discharge view and a face-id seam check, oceans & rivers, cast shadows,
auto-rotate) · verification (init mode plus the three tests).

Rivers are drawn where discharge exceeds a threshold expressed in **multiples of average
traffic**, so the threshold does not need re-tuning when the grid or droplet count
changes. At default settings that paints ≈4 % of cells partially and 0.4 % fully.

---

## Roadmap

- Stage B: replace droplet-carried incision with grid stream-power `K·A^m·S^n` on the
  droplet-supplied discharge — the droplets already produce `A`.
- v002: Euler-pole plates (uplift rung 2) + quadtree LOD mesh (render rung 2).
- v003: advecting tectonics, sub-texel procedural detail.
- Orographic precipitation — rain shadows, so erosion depends on the topography it
  created. Cheap now that discharge exists; high value per line.
- Glaciation above a snowline; oscillating sea level; cutaway view; presets.

See `DESIGN_OPTIONS.md` for the full fork log and the deferred rungs.

---

## Literature

*Author/year/venue given; DOIs to be filled in when the papers are pulled into `papers/`.*

| Year | Work | What we take |
|---|---|---|
| 1957 | Hack — *Studies of longitudinal stream profiles in Virginia and Maryland* | Hack's law; the scaling drainage networks should reproduce. |
| 1960 | Culling — *Analytical theory of erosion* | Hillslope diffusion, the `D·∇²h` term. |
| 1989 | Musgrave, Kolb & Mace — *The synthesis and rendering of eroded fractal terrains* (SIGGRAPH) | Hydraulic + thermal erosion in graphics; the talus model. |
| 1996 | Ronchi, Iacono & Paolucci — *The Cubed Sphere* (J. Comput. Phys.) | Equiangular formulation, metric terms, halo/edge treatment. |
| 1999 | Whipple & Tucker — *Dynamics of the stream-power river incision model* (JGR) | The incision law targeted by stage B. |
| 2007 | Mei, Decaudin & Hu — *Fast Hydraulic Erosion Simulation on GPU* (Pacific Graphics) | The shallow-water pipe model — the Eulerian alternative not taken. |
| 2014 | Barnes, Lehman & Mulla — *Priority-flood* | The depression problem, and why droplets avoid it. |
| 2016 | Cordonnier et al. — *Large scale terrain generation from tectonic uplift and fluvial erosion* (CGF) | Uplift-driven stream power — closest prior art, on a plane. |
| 2019 | Cortial, Peytavie, Galin & Guérin — *Procedural Tectonic Planets* (CGF) | Plate tectonics on a sphere; the model behind uplift rungs 2 and 3. |

**Has it been done?** Stream-power erosion driven by uplift is well established on a
plane, and procedural tectonics on a sphere exists offline. A **realtime, metric-correct,
seam-verified cubed-sphere erosion simulation running in a browser** is the part we have
not found — and the three tests above are what make that claim checkable rather than
decorative.
