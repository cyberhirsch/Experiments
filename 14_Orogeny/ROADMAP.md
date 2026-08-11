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

## v003 — crust and real tectonics

**4. Crust column** ⬜ — *the gating piece*
Per-cell thickness, density and age. Elevation from **local isostasy** rather than the
current global mean-zero subtraction, which conserves volume correctly but is a stand-in,
not the physics: a plateau should be high because the crust beneath it is doubled.

Everything below is blocked on this.

**5. Advecting plates** 🔒 — *uplift rung 3*
Plate id carried with the crust, boundaries migrating, plates splitting and merging.
A moving-boundary problem on a sphere.

**6. Subduction** 🔒
Needs 4 for the density inversion that drives it (oceanic lithosphere goes negatively
buoyant past ~20–30 Myr — slab pull, not ridge push, does the work) and 5 to consume
crust. Then the rules follow: ocean–continent gives a trench and an inboard arc,
ocean–ocean sends the older plate down, continent–continent collides and thickens.

Constraint that makes this hard: total plate area is fixed at 4πR², so destruction at
trenches must match creation at ridges or plates overlap and open gaps.

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

- **Mean-zero uplift is not isostasy.** It conserves volume globally; real isostasy is
  local. Item 4 fixes this and gates items 5 and 6.
- **Erosive throughput is bounded by droplet coverage**, not erodibility. The uplift rate
  is scaled by coverage to compensate; item 9 would change this relationship.
- **The flat test is necessary, not sufficient** — on a uniform field a wrong metric
  weight multiplies zero. The isotropy test carries the real burden.
- **Framerate is unmeasured.** Physics and render output were verified by reading buffers
  directly; interactive performance at 512² with 262 k droplets has not been timed.
