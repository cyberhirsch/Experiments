# № 14 — Orogeny · Roadmap

Ordered by what unblocks what, not by appeal. Each entry says what it needs before it
can start, because several of the interesting features are gated on the same two pieces
of missing state: a **crust column** and an **advected plate field**.

Status legend: ✅ built · 🔨 in progress · ⬜ planned · 🔒 blocked on something above.

---

## v001 — droplet erosion on S² ✅

Cubed-sphere heightfield, ghost-cell halos, measured metric, droplet erosion with a
P2G/G2P grant cycle, talus, creep, lithification, mean-zero uplift, raymarched shell,
three verification tests. See [briefing.md](briefing.md).

---

## v002 — instrument panel, plates, impacts ✅

**1. Four-panel UI** ✅
Top: viewing, resolution, simulation. Left: tectonics and impacts. Right: erosion and
fluids. Bottom: verification and readout. Pure layout — no physics touched, all control
ids preserved so the wiring survived unchanged. Verified single-row top bar at 1440×900;
at 1280×720 the top wraps to two rows and the side panels scroll.

**2. Euler-pole plates** ✅ — *uplift rung 2*
Voronoi plates on the sphere, each with an angular velocity `ω·ω̂`; boundary uplift from
the convergence rate `−(Δv·n̂)`, widened into a belt by distance from the boundary.
Boundaries are static and plates do not advect.

Slots in behind the existing uplift texture as built: evaluated once at init, zero
per-step cost, nothing in the erosion, halo, metric or mass ledger changed.

The two nearest Voronoi seeds give the margin for free — half the difference of the
geodesic distances is the distance to their bisector — so no jump-flooding pass was
needed for distance-to-boundary.

*Measured:* margins occupy 7.0 % of the sphere, split 3.8 % convergent / 3.2 % divergent,
and the field's area-weighted mean comes out at −0.002 before the mean-zero correction —
convergence and divergence very nearly balance on their own, which is the right answer.

Static per-plate crust **type** gives the asymmetric response across a convergent margin:
trench on the oceanic side, arc offset inboard on the continental side, collision plateau
where two continents meet. Continents also start ~26 mm (in radius units) above ocean
floor, standing in for isostasy. The *signature* of subduction without the machinery —
nothing is consumed, arcs do not migrate, oceans never close.

Relief with plates settles at ~0.039 radii over 10,000 steps (against ~0.018 for noise
belts, the difference being the continent/ocean base offset), volume drift −0.054 %.

**3. Asteroid impacts** ✅
Bowl plus central peak excavated, the identical volume laid back down as a raised rim and
an r⁻³ ejecta blanket. Excavation takes sediment before rock; ejecta lands as sediment,
which is what a breccia blanket is.

*Gotcha:* balancing the volumes analytically on a locally-flat plane lost **0.04 % of the
planet per strike**, because the outer taper cuts blanket that the balance assumed was
there. The ejecta height is now solved numerically at strike time against the sphere's own
area element `2π·sin(a)·da`, taper included. Drift per crater went from −0.156 % to
**−0.0003 %**, and −0.0006 % after 40 strikes.

*Resolution floor:* at 6 × 512² a cell is ~18 km and a crater needs ~8 cells to read, so
the smallest honest impact is ~150 km — basin-scale. A cratered surface needs 1024²+.

Note: after a heavy bombardment the *erosion* loop drifts more than usual — −0.54 % over
the next 2,000 steps against −0.02 % / 10,000 on undamaged terrain — because fresh crater
rims are steep enough to drive the talus and grant clamps hard. A loss, not a source.

---

## v003 — crust and real tectonics ✅

**4. Crust column** ✅ — *the gating piece*
The terrain texture became `(h, sed, tc, type)` and the **surface h is now derived**:
`h = uIsoK · tc`. Crustal thickness `tc` is the primary, conserved quantity; the surface
is wherever that column floats.

`uIsoK` is the fraction of a thickness change that reaches the surface. True Airy is
`1 − ρc/ρm ≈ 0.152`: erode a kilometre off a range and it rebounds 850 m. Tectonics
thickens the column, erosion removes mass from it and only `uIsoK` of that shows — the
asymmetry between those two is the entire point.

*Measured*, uplift off, 4,000 steps of pure erosion, relief retained:
**84.2 % rigid (K=1) → 88.3 % at the K=0.35 default → 90.7 % at true Airy.** Isostatic
rebound preserves relief, in the right direction and by a measurable amount.

Surface relief holds equilibrium unchanged (~0.017 over 8,000 steps) and crust-volume
drift settles at −0.23 %.

*Chosen over:* (a) a bare damping factor on erosion — one line, captures the rebound, but
cannot express "continents are high because the crust is thick" and leaves subduction
fake; (b) a separate R32F thickness texture instead of widening the terrain texture —
fewer bytes, more bindings and more passes to keep in sync; (c) running at true Earth
proportions and exaggerating only in the renderer — honest units, but every erosion
constant would need re-deriving. **Still open:** flexural isostasy (an elastic plate
spreads the load over a flexural wavelength instead of responding locally), viscous
relaxation with a real time constant, and separate sediment density — a sediment layer is
lighter than crust and should float higher per unit thickness, which this model ignores.

**5. Advecting plates** ✅ — *uplift rung 3*
Plate id is now a **field**, seeded from the Voronoi and carried by its own plate's
rotation, so margins migrate. Uplift is rebuilt from that field every 50 steps — margins
move far slower than the framerate, the same amortisation № 11 uses for its connectivity
sweep. Margin distance comes from a local scan for a differing id, so no jump-flood or
blur pass was needed.

*Gotcha:* the first version stepped the field incrementally against the previous frame,
and **never moved at all**. At the default rate one rebuild backtraces 0.16 of a cell,
which nearest sampling rounds to the same cell, forever. Replaced with an exact backtrace
by the **total accumulated angle** against the immutable t=0 field: correct at any rate,
and resampling the original each time means the label field never diffuses. Nearest, not
linear, because an id is a label — blending 3 and 8 into 5.5 invents a plate.

*Measured:* 5.45 % of cells changed plate over 6,000 steps at the default drift rate.

*Chosen over:* raising the drift rate until incremental stepping clears a cell per
rebuild — works, but couples plate speed to grid resolution. **Still open:** plates
splitting, merging and vanishing (topology is fixed here, only geometry moves); ω taken
from the initial field, so a cell that changed hands is carried by its old plate; and
mantle convection actually driving ω instead of it being a fixed random draw.

**6. Subduction** ✅
With crust type as an advected field, the rules follow: ocean–continent consumes the
oceanic side and raises an arc inboard, ocean–ocean gives an island arc,
continent–continent collides and thickens, divergent margins rift and create new oceanic
crust.

*Measured:* mean oceanic crust thickness goes 1.5e-4 → −3.8e-4 over 6,000 steps — the
oceanic plates are being consumed.

**The invariant changes here, and that is correct.** Crust is created at ridges and
destroyed at trenches, so crustal volume is genuinely no longer conserved; forcing the
source term to zero mean would be wrong. The budget is reported instead of imposed:
**−0.42 %** over 6,000 steps, i.e. consumption slightly outruns creation, which is the
expected regime given margins run 3.8 % convergent against 3.2 % divergent. Switch
subduction off and the strict conservation of v002 returns.

**Still open:** true plate-area bookkeeping (total plate area is fixed at 4πR², and
nothing here enforces that destruction matches creation); slab dip setting the arc
offset from convergence rate rather than a slider; crust *age*, so oceanic lithosphere
becomes negatively buoyant past ~20–30 Myr and decides for itself which side subducts,
instead of type deciding it.

---

---

## v003.1 — the margin field actually reaches its features ✅

**A. Scan reach** ✅
The uplift rebuild scanned ±5 cells at stride 1. At 512² that is 0.015 rad against a belt
of 0.045 and an arc offset of 0.056, so it reached neither: the belt was a hard-edged
5-cell stripe whose Gaussian never decayed below 0.944, and the volcanic arc — whose peak
lies outside the window entirely — **did not exist**.

*Measured, the smoking gun:* source coverage was **10.84 % of the sphere for every value
of `bwidth` and every value of `arcOff`**. Both sliders were geometrically inert.

Replaced with a two-tier scan — stride 1 out to 5 cells for the exact near field and the
normal, then the same 13×13 window strided by `K` for direction only, then a stride-1
march back in for exact distance. `K` follows the sliders. Coverage now moves 36 → 80 →
99 % across `bwidth` and 71 → 80 → 83 % across `arcOff`.

The scan also used to clamp to the layer, truncating against the face edge — invisible at
±5, a groove along every cube edge at ±60. Out-of-face offsets now route through the
direction. *Measured:* mean-|u| at 0, 1, 2 and 4 cells from a face edge agrees within
0.6 %; flat and isotropy tests still clean (0.000 % spread, corner ≡ face centre at 0.02 %).

**B. Ridges were trenches** ✅
The divergent branch summed a rift term of −0.55·|conv|·band against +0.30 for crust
creation. The graben won outright, so **spreading centres thinned the crust** and the
reported budget counted creation as a loss. A ridge is the shallowest part of an ocean
basin. Now written with the two length scales it needs: a broad positive swell of width
`w` carrying a narrow negative of width ~`w`/4. *Measured:* of 2,946 ocean–ocean margin
crossings, 1,491 now show the ridge profile (positive crest, higher rim, decaying to zero)
against 761 with subduction off — the extra ~730 are the divergent margins that used to
sink. Crust-age (item below) gives this properly, via depth ∝ √age; this is the stand-in.

**C. Source normalisation** ✅
Widening the belt widened every term at once, so total throughput rode on `bwidth` — the
scan fix alone multiplied mean-|u| by **2.74×** and drove the crust budget to **+3.97 %**
over 6,000 steps. The source is now normalised by its area-weighted mean-|u|, so belt
width changes shape and not amount. Budget **+1.37 %**.

Found on the way: `upliftFromField` never measured its own field, so `UMEAN` was inherited
from the noise path — in drifting mode with subduction **off**, the regime that exists to
demonstrate strict conservation, the mean-zero correction ran on a stale constant. Now
measured, on geometry change rather than per rebuild (the readback would triple a
rebuild's cost, and margins move only 5 % of cells in 6,000 steps). *Measured:* −0.161 %
over 6,000 steps with subduction off.

Untouched: noise belts (relief 0.0179, budget −0.059 %) and static plates (relief 0.0353,
budget +0.018 %) both unchanged. Drifting-plate relief now settles at 0.0178 — lower than
static plates, because holding crustal flux fixed while tripling the belt width
necessarily lowers the peak.

**Still open here:** the +1.37 % is unenforced. Its sign is now creation-dominated where it
was consumption-dominated, and balancing it properly means splitting creation and
consumption into separate channels and scaling one to the other — which is the plate-area
bookkeeping item below.

---

---

## v003.2 — plate boundaries stop being polygons ✅

Spherical Voronoi boundaries are great-circle arcs, i.e. exactly straight, and seed jitter
cannot fix that because it moves the seed and not the shape of the bisector. Two controls:

- **Boundary warp** — a two-octave domain warp on the lookup direction inside
  `nearestPlates`, so every consumer (plate field, initial continents, overlay, and the
  drifting source through its seeding) inherits sinuous margins from one place. *Measured:*
  boundary crossings along a fixed set of great circles 2199 → 2372 → 2580 for warp
  0 / 0.18 / 0.40, where a straight-edged Voronoi would hold constant.
- **Orogen break-up** — a ridged-fBm mask on the *orogenic* branches only, so ranges are
  chains of massifs rather than one continuous welt. Trenches and rifts really are
  continuous and are left alone. *Measured:* −24 % uplift volume and −20 % strong-uplift
  area at full strength, negative volume unchanged to four figures.

---

## v004 — the motion-vector source ✅

**13. Plate field as a velocity, not a label** ✅
Three passes: `v = ω × p` blended across each margin → finite-volume divergence →
crust-type rules applied to that divergence. −∇·v is the areal strain rate, so subduction
and spreading become one quantity read off a four-tap difference.

**This retires the entire margin-scan apparatus** for this source — no scan for a differing
id, no scan radius, no two-tier window, no quantised margin normal. It also fixes the
belt-width coupling at the root: blending v over width `w` gives `|∇·v| ~ conv/2w`, so
total consumption is convergence × margin length *independent of w*, which is the correct
invariant. The old Gaussian band's integral scaled with `w`, which is the whole reason
SRC_NORM had to be invented one version earlier.

Seeds move under their own Euler pole, so the Voronoi drifts analytically — no id field,
no advection step to compound, no possible repeat of the v003 lockup, and margin distance
stays exact at any range. *Measured:* 31.2 % of sampled directions changed uplift over
6,000 steps. Relief 0.0352 and budget +0.273 %, against 0.0353 / +0.018 % for static
plates and 0.0348 / +0.269 % for the drifting id field — the three sources agree.

*Caveat, by construction:* a Voronoi cell shrinks as neighbours close in but never empties,
because a seed always owns the points nearest it. Oceans narrow; they do not shut. Real
topology change needs retirable seeds.

**The gotcha, and a wrong diagnosis I had to back out of.** Global closure ∮∇·v dA must be
zero on a closed surface. It came out at **−4.93 %**. My first explanation — that the edge
normal was being taken in each cell's own tangent plane instead of at the shared midpoint —
was **wrong**, and the fix moved the number by nothing whatsoever, which is what forced me
to build a control with an exact answer: *a single plate is a rigid rotation, and a rotation
is divergence-free at every point*. That showed a background |∇·v| of 0.18 where the true
value is 0. Two real causes: edge *lengths* taken from centre-to-centre distances rather
than the cell's actual edges, and the edge *normal* taken along the centre-to-centre
direction — which is not perpendicular to the edge, because the equiangular grid is not
orthogonal away from a face centre. Building both from the shared cell corners fixed it:

| | before | after |
|---|---|---|
| rigid-plate background \|∇·v\| (exact: 0) | 0.18 | **0.00057** |
| global closure, 22 plates | −4.93 % | **0.0001 %** |

Creation and destruction now balance to machine precision *by construction*, which was the
claim made for this architecture and is now actually true of it.

---

## v004b — the crust rides the plate ✅

**14. Crust advection** ✅
Semi-Lagrangian, bilinear, backtraced along the local flow. A crater now drifts with its
plate instead of sitting still in space while the margins slide past it. Bilinear rather
than the plate field's nearest sampling — thickness and sediment are continuous, and
nearest would freeze sub-cell motion exactly the way the v003 advection locked up. `h` is
not carried; it is recomputed from the transported thickness, so the two cannot desync.

*Measured:* against a stationary copy the field differs by **11.65 %**, against its own
analytically backtraced self by **3.50 %** — it moved, and it moved where the velocity
field says. Advection off, the field is unchanged at 0.01 %, which also confirms crust
genuinely never moved before this.

**Three things I got wrong, all found by controls rather than by inspection:**

1. *Banking motion for accuracy.* I claimed a sub-cell resample smears as badly as a full
   one, so fewer larger hops would be much cleaner. Wrong — bilinear error goes as t(1−t),
   so small hops are individually gentle. Over the same drift, 80 passes retained 99.52 %
   of the field's spread against 99.56 % for 17, and raising the threshold from 1 cell to
   16 moved retention only 89.2 → 90.6 %. Banking stays, but for **cost** (4.7× fewer GPU
   passes), not accuracy.

2. *Volume.* Pure backtracing lost **−56 %** over 6,000 steps: it moves crust without
   compressing it, so converging plates pack crust into less area at unchanged thickness.
   Isolated with the same rigid-plate control used for the divergence — under a
   divergence-free flow the identical code conserved to **0.000 %**, proving the scheme
   sound and the flow's divergence responsible.

3. *The fix for 2 was itself wrong.* Scaling the column by exp(−∇·v·Δt) is the textbook
   conservative form and measured 0.000 % under rigid rotation — but **`tc` is a signed
   deviation from a datum, not an absolute thickness**, and oceanic crust carries negative
   `tc`. Multiplying a negative column by a factor under one moves it toward zero, which
   *adds* signed volume. Thinning oceanic crust at a trench made the planet gain mass:
   advection alone went **+14.4 %** at default drift and **+45.5 %** at maximum. No amount
   of gating by crust type fixes a sign error in the variable's definition. The term is
   coded, disabled, and documented in place.

Shipped with compression off, which leaves `tc·∇·v` to the divergence-driven uplift source
where it already lived. *Measured, 6,000 steps:* relief **0.0347** against 0.0348 with
advection disabled entirely, budget **+0.386 %** at drift 0.5 and **−0.112 %** at drift 2.0.
Transport does its job without disturbing the balance.

**Still open:** doing compression properly needs `tc` measured up from a datum thickness
rather than about zero — which changes what the variable means everywhere, and is naturally
task #3's territory, where crust age supplies a real thickness.

---

## v004c–g — sharp margins, directional pressure, companion tools, baked folding ✅

Compressed log; the forks and measurements live in DESIGN_OPTIONS §§23–30.

- **Tear at faults, MacCormack, fresh crust** ✅ — plate-membership clamp on the
  backtrace instead of blending across faults; crater rim retention 95.2 → 99.3 %; new
  seafloor injected by areal expansion (∇·v·Δt), gated oceanic.
- **Sharp margins + directional propagation** ✅ — velocity blends abandoned (they
  corrupt interiors: rigid rotation read |∇·v| 0.31 at blendC 1.95); compression spike
  marched along the velocity field with a buoyancy gate. True one-sidedness 4.9–6.0×,
  measured per crossing. tanh blend profile kills the dead-zone on the margin axis.
- **Two pressure channels** ✅ — PG.x compression (propagated), PG.y extension (sharp,
  untouched by propagation; verified bit-identical across reach sweeps).
- **Two clocks** ✅ — "Motion evolution" (seed migration, default 0) vs "Plate speed"
  (crust transport); material frame follows the crust clock.
- **Companion tools** ✅ — noise designer (compositional GLSL library, preprocessor-
  specialized after the runtime-dispatch version crashed the GPU process; 4,480
  combinations verified), impact designer (9 crater-shape params threaded through shader
  AND the CPU mass-balance solve), preset folders via the File System Access API with
  localStorage fallback, compiled-in presets for file://.
- **Baked orogen folding** ✅ — fold displacement as a function of the current pressure
  field, written to `tc` as a delta (FD pair): present at frame 1, zero drift over 100
  re-bakes, reversible, isostasy still sole owner of the surface. Replaces the carried
  strain/fold-axis field and `foldTerm`, both retired; advection back to a single target.

---

## Deferred from v003 — worth doing next

- **Plate topology**: splitting, merging, vanishing. Geometry moves today; topology does
  not, so an ocean can never fully close.
- **Crust age** as a field, driving buoyancy. Replaces the binary ocean/continent type
  with the real mechanism and lets the older plate subduct at ocean–ocean margins.
- **Plate-area bookkeeping** against 4πR².
- **Flexural isostasy** — an elastic plate spreading each load over a flexural wavelength,
  rather than the purely local Airy column used now.
- **Separate sediment density** — sediment is lighter than crust and should float higher
  per unit thickness. Currently both are treated alike.
- **A rebound verification test** in the panel, alongside flat/isotropy/mass: the
  84.2 → 90.7 % measurement is currently a one-off, not a regression guard.

---

## v004 — fluids on the same manifold

**7. 2D fluid** ⬜
Atmosphere or ocean as PIC/FLIP on the cubed-sphere atlas — № 02's method, reusing this
experiment's halo, metric texture and arc lengths unchanged. A real slot in the existing
framework rather than a rewrite.

**8. Orographic precipitation** 🔒 *(needs 7, or a cheap wind proxy)*
Rain shadows behind ranges, so erosion rate depends on the topography it created. Strong
feedback, and cheap once wind exists. Highest value per line of code on this list.

---

## Independent of the above

**9. Stage B — grid stream power** ⬜
Replace droplet-carried incision with `K·A^m·S^n` on the droplet-supplied discharge. The
droplets already produce the drainage area; this swaps the transport law for the
canonical one and should sharpen river long-profiles.

**10. Render rung 2 — quadtree LOD mesh** ⬜
Displaced cubed-sphere patches, vertex shader sampling the height array. Scales orbit to
valley floor and is much faster than the current raymarch. Needs crack-free stitching.

**11. Render rung 3 — sub-texel procedural detail** 🔒 *(needs 10)*
Fractal displacement below sim resolution so zoom never hits a floor. Caveat: invented
detail is not simulated and will not erode, so it must stay below the scale at which the
eye can check it against the drainage network.

**12. Smaller things** ⬜
Glaciation above a snowline (U-shaped valleys, a distinct erosion signature) · oscillating
sea level (marine terraces) · cutaway view exposing strata and sediment thickness ·
parameter presets and save/load · strike-slip offset of drainage across transform
boundaries.

---

## Known limitations carried forward

- **Isostasy is local Airy, not flexural**, and runs at a compromise `uIsoK` rather than
  the true 0.152, because the true value collapses this model's exaggerated relief.
- **With subduction on, crustal volume is not conserved** — by design; crust exchanges
  with the mantle. The budget is reported, not imposed. Turn subduction off for the
  strict-conservation regime.
- **Erosive throughput is bounded by droplet coverage**, not erodibility. The uplift rate
  is scaled by coverage to compensate; item 9 would change this relationship.
- **The flat test is necessary, not sufficient** — on a uniform field a wrong metric
  weight multiplies zero. The isotropy test carries the real burden.
- **Framerate is unmeasured.** Physics and render output were verified by reading buffers
  directly; interactive performance at 512² with 262 k droplets has not been timed.
