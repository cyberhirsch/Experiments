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
