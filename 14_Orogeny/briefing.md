# № 14 — Orogeny

**Seamless planetary erosion — a whole planet held in erosional equilibrium on S².**

Tectonic uplift raises relief, rivers cut it back down, sediment fills the basins and
builds deltas at sea level. Because uplift and erosion balance, the simulation has no
end state: it runs indefinitely rather than decaying to a smooth sphere.

*Status: **v002 built** — still in `Orogeny_v001.html`, single file, WebGL2, no
dependencies. v001 was droplet erosion on a noise uplift field; v002 adds the four-panel
instrument layout, Euler-pole plate tectonics and mass-conserving impacts. See
[ROADMAP.md](ROADMAP.md).*

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
