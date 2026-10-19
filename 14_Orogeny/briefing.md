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

## v004h — the fold record rides the crust (bug)

Visible as combed, needle-like spikes trailing every orogen in the direction of plate motion.
`bakeFold` writes the fold into the crust as `dNew − dOld`, with `dOld` kept in `FD`. The crust
is advected between bakes; `FD` was not. Every rebake (each `PLATE_EVERY` = 50 steps) therefore
subtracted the old fold where it used to be, while the advected copy stayed in the rock. The
residues stacked up along the drift.

`FD` now makes exactly the trip `T` makes, in `advectFold`:
- the same backtrace and the same fault clamp,
- the same MacCormack pass and limiter, so the record and the rock diffuse identically,
- it is wiped where fresh seafloor replaces the column.

Patched into v003 and carried into v006. **Not yet measured**; the check is a run at high plate
speed with Pressure and Surface views compared over a few thousand steps.

## v006 — planet presets, cleaned-up panel

`Orogeny_v006.html` + `planet-presets.js`. Thirteen planet types replace the single Earth-like
default:

| | | | | |
|---|---|---|---|---|
| Terran | Young orogen | Pangaea | Waterworld | Luna |
| Mars | Venus | Io | Archipelago | Europa |
| Snowball | Dune world | Titan | | |

A planet preset is more than a slider snapshot. It names:
- **Modules.** Which of tectonics, impacts, erosion and ocean the planet uses. The others are
  switched off and their panels hidden. "Show all modules" brings them back.
- **Overrides** on top of base defaults (`P_BASE`), so switching never inherits the last
  planet's settings. View and performance settings are excluded and stay the user's.
- **A look.** Palette, altitude reference (from sea level or a fixed datum), snow band, ocean
  colours or a frozen ocean, river colour, and the atmosphere's limb and dayside tint. These are
  render uniforms; the shader's hard-coded Earth palette is gone.
- **A history.** Phases start at a step count and may change parameters or fire a burst of
  impacts:
  - Mars runs wet for 3,000 steps, then loses its water and takes 120 craters.
  - Luna starts with 500.
  - The dune world dries out at 1,500.
- **What is missing.** Types that need an unbuilt process (volcanism, glaciers, wind, lineae,
  hotspots) are marked ◐ and say what they approximate.

Panel: one Planet picker at the top, with the description beneath it. Each module is a section
with a single *Active* switch. Expert controls sit in collapsible groups (margin detail,
droplets). Grid, droplets, render quality, terrain noise and verification moved to the bottom
bar. Speed runs 1, 4 or 16 substeps per frame so phases arrive in reasonable time. The impact
radius floor drops to 0.008 rad for small lunar craters. The noise and impact designers open from
the top bar.

**Craters overprint.** Strikes used to add their profile to whatever was there, so a ridge ran
straight through a fresh bowl. Now, before the profile is stamped:
- The column inside the rim is relaxed toward its own footprint mean, crust thickness and
  sediment separately.
- The weight is full in the bowl and fades out between 0.95 R and 1.5 R.
- The mean comes from a fixed 126-sample equal-area polar stencil around the centre. It is
  identical for every fragment, so all faces agree on one level.

Relaxing toward the mean rather than the centre value keeps the step volume-neutral to sampling
accuracy: the ridge's material fills the footprint's low ground, and the bowl/rim/ejecta balance
is untouched. The **Overprint** slider (0 restores additive) controls the strength. Unmeasured:
what this does to the mass audit per strike. The Mass test after a burst would show it.

## v007 — rock and gas layers, strata, lava

`Orogeny_v007.html`. A planet is now a **rock** body, a **gas** envelope, or both: two switches in
the top bar. Rock modules need the rock layer; the Gas module appears whenever the gas layer is on.

**Gas layer.** A 2D incompressible flow on the same cubed-sphere grid: №02's subject on №14's
machinery, not a port of №02's three.js FLIP.
- **Velocity** is a world-space 3D tangent vector, so seams need no basis change.
- **Each step:**
  - vorticity
  - semi-Lagrangian midpoint advection of velocity and dye along the geodesic
  - forcing: zonal jets relaxed toward `cos(n·lat)`, Coriolis as a rotation about p̂ by
    −2Ω·sin(lat)·dt, vorticity confinement, damping
  - divergence through v004's exact finite-volume shader
  - warm-started Jacobi on ∇²p = ∇·v
  - projection
- **Dye** carries a band tracer (`sin(n·lat)`, so jets sit at the band edges as on Jupiter) and a
  cloud density. Both relax slowly toward their reference patterns.
- **Storms** are Gaussian vortices injected from the CPU: 75 % anticyclones that brighten, the
  rest dark cyclones.
- **Rendering:**
  - with no rock, the envelope is the body, shaded with limb darkening;
  - over rock, one cloud shell sits above the relief, with opacity and a density threshold
    ("cover", −1 = overcast).

New gas giants: Jupiter, Saturn (rings missing), Neptune. Terran, Mars and Titan get weather, and
Venus an opaque deck: switch Gas off to see its ground.

**Rock layers.** Hard and soft beds alternate in crustal thickness `tc`, which is a material
coordinate, so erosion cuts *down through* the beds rather than dragging them along. A noise warp
dips and folds them. Soft beds yield more rock per unit of erosive work in the apply pass, and
the palette colours the same function on exposed rock, so the visible bands are exactly the beds
that resist. Presets: Dune world (mesas), Mars, Young orogen.

**Lava world.** The ocean shell becomes a molten, emissive sea with drifting cooled crust.
Land glows along extension (the pressure field's low channel) and just above the lava level.
Eruptions and flows over land are not simulated yet.

**Impact designer** is a full-screen page (`impact-designer.html`) in the noise designer's
style. It shows the strike's exact profile, with the same CPU ejecta balance, as shaded relief
on a grey plane, with an optional scatter. It shares presets through the store, and the main page
re-reads the selected preset whenever the store rings. Lost compared with the popup: applying
without saving, and test strikes on the planet. Pick the preset and press Strike now instead.

**Crater realism (v007, later).** The profile moved into `CRATER_GLSL` and the solve into
`crSolve` and its helpers, each kept byte-identical in `Orogeny_v007.html` and
`impact-designer.html`. New crater-shape keys, all off by default so old presets stamp the old
crater:
- size-dependent morphology (`morphMix`, `transR` in radians): simple → complex blends the
  depth ratio, a flat floor and the peak, and a peak ring takes over above `ringR`;
- wall terraces (smoothed posterize of the wall's depth fraction);
- a central-peak cluster with the single Gaussian's volume;
- an irregular and polygonal rim (zero-mean radius modulation);
- ejecta rays (zero-mean blanket modulation) and secondary craters, each its own balanced strike;
- oblique impacts: angle fixed or drawn from sin 2θ, an area-preserving elongation below
  `elongAngle`, and a two-harmonic butterfly with an uprange forbidden zone.

Each strike carries a seed and a downrange azimuth. The hash is integer (lowbias32), so the CPU
reproduces the shader's angular terms exactly. The balance multiplies the radial integral by the
angular means ⟨ρ²⟩, ⟨ρ²B⟩ and ⟨ρ²B·m⟩, which makes it exact on the flat area element. What is left:
curvature under the angular warps, peak-cluster truncation and quadrature error. The designer
prints a polar-grid volume audit of its centre crater. The designer also previews on a **sphere**
(cube-map bake plus mesh, identical to the planet's angular crater) and on a **rounded box**,
which is only a preview approximation: craters are laid out by angle from the box centre and
displaced along the box normal. Drag orbits the view and Auto-rotate turns it.

Unmeasured: every new default and threshold (transition and ring radii, floor, terrace scarp
width, the ray fade from 1 to 2 R, the oblique ramps, secondary size), the audit's actual
numbers, and the cost of secondaries. N secondaries make a strike N + 1 full passes.

**Unmeasured, all of it.** The gas constants are first guesses:
- jet 0.0015 rad/step
- Ω scale 0.02
- confinement ε = 0.5·cell·slider
- 20 Jacobi iterations

The projection's residual divergence has not been looked at. The gas layer runs at the full grid
resolution, so 6 × 512² with gas on costs roughly 30 extra full-grid passes per step.

**Surface freshness (the brightness channel).** On an airless body the strongest visual cue is
brightness, not relief: young craters and their rays are bright, because space weathering
darkens and reddens regolith over time. A new field `AG` holds freshness f ∈ [0,1], and brightness
is derived from it in the render.
- **Every substep** f decays by exp(−weather·dt), so the half-life is ln2/weather steps. Channels
  carrying many times the average discharge are held fresh.
- **Every strike** sets f to 1 over the bowl, rim and continuous blanket (to ~2 R). It also lays
  rays that reach to ~12 R, well past the relief's own ejecta: a thresholded noise of azimuth
  alone, not ballistic paths.
- **Advection:** f rides the crust through the fold record's own advection shaders, and is wiped
  where new seafloor forms.
- **Rendering:** *Fresh contrast* scales brightness (−35 % to +55 %) and a slight
  blue-fresh / red-mature tint. A *Surface age* view shows f directly.

Presets set the contrast: Luna 1.0 with faster weathering and long rays, Europa 0.5, Mars 0.35,
Venus 0.15, Terran 0.1, Io and the lava world 0 (both resurface too fast to age). The constants
(decay, ray falloff, core radius) are untested choices, and the field is not a physical albedo.

## v008 — craters break up

`Orogeny_v008.html` and `impact-designer.html`; the shared crater blocks are byte-identical in
both, as in v007.

**Noise break-up.** All of it is one polar lattice noise (`crNoise`): cells around the crater
(periodic in φ), a radial coordinate v, two octaves, seeded per strike. Each lattice value is a
difference of two neighbouring hashes, so every ring sums to zero and the noise is exactly
zero-mean around every circle. New keys, all off at 0:
- `rimRough`: crest notches, × E, in a narrow window around R.
- `floorNoise`: hummocks, × D, faded at the centre.
- `wallNoise`: radial gullies, × D.
- `ejectaLobes`: blanket patches.
- **Radial streaks, the "zoom blur":** `streakStr`, `streakN` for the count, `streakSharp` (a
  power applied before differencing, giving a few bright streaks, each with a groove),
  `streakLen` in log-radius, and `streakFade`.

**Mass balance.** Zero mean around every ring means the noise adds no volume of its own. Its
correlation with the butterfly does change the volume, which matters because secondaries are
always oblique. `crSolve` computes that correlation exactly from the lattice (DFT × the
smoothstep kernel's transform) and folds it into E. The correlations with the rim-radius
modulation and with the rays are left as documented second-order residuals.

**Secondaries are oblique.**
- Angle uniform in `secAngleMin`–`secAngleMax` (10–25°), at any `obliqueMode`.
- Flight azimuth points outward from the primary.
- Elongation at least `secElong` (1.4).
- Asymmetry `secAsym` (1.0) and their own `secStreak` (0.5).

The sign chain (outAz → uAzDir → +q.x → φ = 0 → B = 1 + A1 − A2) puts the ejecta on the side away
from the primary and starves the side facing it. A v007 preset that already had secondaries now
gets oval ones.

**Designer.** The profile plot and readout are pinned above the scrolling sliders. The audit
adapts its angular resolution to the finest noise and also audits the first secondary.

Unmeasured: everything: cell counts, windows and masks, the 1–1.2 R fade-in, the secondary
defaults, shader cost (up to ~120 hashes per crater per fragment with every noise on), and the
audit's numbers.

## v009 — fractal crater detail, band-limited

`Orogeny_v009.html` (crater code) and `impact-designer_v002.html`. The shared blocks are still
byte-identical between the two files.

**fBm.** `crNoise` is now fBm on the polar lattice, set by one shared Detail group: `detailOct`
6, `detailLac` 2.0, `detailGain` 0.5.
- Octave k has round(N·lac^k) cells around, radial coordinate v·lac^k + 17k, and its own salt.
- Every octave is the zero-sum lattice, so each one, and their sum, is exactly zero-mean
  around every circle.
- The amplitude is normalised by Σ gain^k over all octaves, so detail is added without
  changing the coarse noise.

**Micro relief.** `microAmp` 0 (× D), `microScale` 24 cells/R, `microOct` 6. A fine grain over
the footprint out to 1.5–2 R, scaled to R. It is split into annuli (x = 0.125·2^b), each with
its own cell count, cross-faded in log₂ x, so cells stay within 2:1 of square. It is additive
and never multiplied by the butterfly.

**Band limit.** Each octave is faded by smoothstep(b, 2b, size/uCut), with `bandCells` b = 2.
Here size is the octave's smaller cell dimension, computed with the nominal R so that the weight
depends on x alone and the zero mean survives. uCut is the only thing that differs between
targets:

| Target | uCut |
|---|---|
| Planet | grid cell (π/2)/N |
| Designer plane | screen pixel |
| Designer sphere / box | bake texel |
| Designer profile | sample spacing |

So the designer at the planet's resolution is the planet. `crSolve` takes the cut it is
stamped at; the designer has a "Planet grid" setting for that.

**Mass balance.** As in v008: no first-order volume. The butterfly correlation is folded
into E per octave, with the same band weights, for octaves of at most 256 cells. Finer octaves
are left out; their harmonic content is ~2πσ·N^-1.5, under 2·10⁻³σ. The audit runs at the
planet cut, with up to 4096 angles and 400k samples.

**Designer v002.**
- Log zoom down to 1e-5 rad of view, zooming about the cursor.
- Plane centres are uploaded relative to the pan (doubles → fp32), so deep zoom does not band.
- Full devicePixelRatio.
- Bakes up to 4096², about 400 MB of R32F.
- Mesh normals use max(texel, pixel), via fwidth.
- A pinned detail readout of octaves visible in the view and on the planet.
- A profile sampled at 4 points per device pixel with its own band limit.

Unmeasured: all of it. That covers the defaults, the 2-cell band edge, the annulus layout,
shader cost (8 octaves × 5 noises + micro relief is several hundred hashes per crater per
fragment before band-limiting), the 4096 bake's memory on real GPUs, and the audit's numbers.

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

**Runtime noise dispatch is compile-time multiplication.** One shader branching over 9
noise bases × 8 combinators inlined every branch — `evalBaseUnit` 83 times, each unrolling
a 27-cell × 8-impulse kernel — and crashed the GPU process, taking every later WebGL2
context in the browser with it. Specialize through the preprocessor and cache one program
per combination; the largest of 4,480 verified specializations is 167 lines. Corollaries:
warp fields want a fixed cheap noise, not the selected base, and a finite-difference
derivative is a silent 7× on the whole shader — compile it only on request.

**A rate cannot deliver "immediately".** Uplift accumulates, so ranges built from it grew
the peak 9 % in 1,200 steps — invisible while watching. Displacement that must exist the
moment its cause does has to be a *function* of the cause, baked as a *delta* against the
previous bake (an absolute add re-applies itself every rebuild and ramps without bound).
Measured: zero drift over 100 re-bakes, exactly reversible at height 0.

**Averaging both sides of a margin before taking max/min hides real asymmetry.** Which
side is "strong" flips from crossing to crossing, so summing sides across ~800 crossings
averaged a true 4.9–6.0× one-sidedness down to a measured ~1.0–1.3×. Classify strong/weak
per crossing first, then accumulate. An earlier mechanism was judged on the broken
instrument; its published figure was never real.

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

## v010 — the gas carries colour, not a number

`Orogeny_v010.html`. Up to v009 the gas dye carried the band VALUE, and the colour ramp was
applied at render time. Advection mixes values, so a stirred boundary between two bands contains
every value in between, and every in-between value gets its ramp colour. A narrow dark stop in a
gradient therefore showed up as a sharp dark contour wherever the mixture crossed it: an
iso-line of the mixture, not material, so it could never thin or disperse.

Now the colour is applied first and the colour is advected:
- The dye holds rgb paint plus cloud density in alpha.
- Init and relaxation colour the pattern through the same ramp: the pattern's gradient, or the
  planet's four band stops.
- Storms brighten or darken the paint.

A dark edge stirred into a light zone now thins and fades. Changing the pattern restarts the
envelope in its new colours. Relaxation alone would take thousands of steps, and changing a
gradient in the designer otherwise reaches the gas only through that slow relaxation.

## v011 — every crater different

`Orogeny_v011.html`. On Luna every small crater looked identical. The per-strike seed reached
the shader correctly, but the seeded angular noises are band-limited to the grid. A crater a few
cells across keeps none of them, and its coarse profile was the same for every strike, so only
the size varied.

`varyCrater()` now scatters the coarse shape per strike, scaled by **Variation** (default 0.6):
- depth ±25 %
- rim height ±35 %
- ejecta reach ±20 % and fall-off ±15 %
- central peak ±40 %
- a low-order rim outline up to 6 % of R
- for a vertical-only preset, a random impact angle from sin 2θ

`crSolve` sees the varied shape, so each crater is still volume-balanced. The ranges are
untested. Variation 0 restores the exact preset. The designer is unaffected: it shows the preset
itself.

## v012 — export bake

`Orogeny_v012.html`. The model is to simulate at 512–1024 interactively and then bake offline at
2048–16384 per cube face. The bake is opened from **Bake…** in the bottom bar and runs in tiles.
It is not realtime, and the sim is frozen while it runs.

**Strike log.** Every stamp is recorded in stamp order, primaries and secondaries alike. Each
record is 32 Float64 values: centre, R, seed, azimuth, angle, secondary flag, frameNo, grid N,
base-shape index, uErase, crust travel at the strike, the 12 solved values, and the 7 keys
`varyCrater` touches. The base presets live in a small table, and a record costs 256 bytes.
- `setCraterUniforms()` is factored out of `strike()`, so a logged strike replays through the
  same uniform code.
- The log is cleared in `buildTerrain` (re-init wipes every crater). That covers new seed,
  rebuildWorld and grid change.
- `crustMoved` sums the crust travel that `advectCrust` actually applied.

**Height per export texel.** Everything is a function of the direction, so faces meet without
seams.
- **a) Simulated surface:** h + sed, Catmull-Rom on the face texels. Support beyond the 1-texel
  halo is read from the neighbour face.
- **b) Terrain noise:** sN · (init noise at export resolution − Catmull-Rom of the init at the
  grid texels). The grid term is produced by re-running the sim's own init program with
  snapshotted uniforms. The export term is:
  - for a noise preset: the band-limited noise-lib path (`{cut:true}`, uNoiseCut = texel angle);
  - for the built-in fBm or ridged init: FS_INIT's formula plus a zero-mean tail of extra
    octaves, faded with noise-lib's rule.
- **c) Craters:** Σ sC · (surface response at the export cut − Catmull-Rom of the
  surface response at the grid texels, at the sim cut). This runs in stamp order. Each strike's
  erase mask scales down everything older, including (b).
- **Crust motion:** the rock's position at init and at each strike is found by rotating back about
  the pole of the plate under the texel now, as matPos does.
- **Why "minus the grid's representation":** subtracting what the grid actually holds (the
  interpolant of point samples) removes interpolation blur and init-noise aliasing too. Each
  residual is zero at the sim texel centres by construction.

**Pipeline per tile** (TS² plus a halo): MAT → NOISE → per culled strike [copy, scissored GRID
pass, scissored CRATER pass] → FINAL → NORMAL → COLOUR.
- Craters are culled on the CPU: a cap test, then a conservative scissor rectangle.
- Readback goes through pixel-pack buffers behind fences.
- There is one crater per GPU flush and a fence-wait every 12.
- Bands of rows stream into PNGs through a CompressionStream, written as consecutive IDAT
  chunks. Files go to a folder via File System Access when the page is a secure context;
  otherwise they are downloaded (held in memory, which is not workable at 8192+ on file://).

**Outputs.**
- **Height:** 16-bit grey. The range is auto (sim range + 15 % margin, untested), exact (an extra
  pass) or manual. Clipped texels are counted.
- **Colour:** the Surface-view `palette()` lifted from FS_RENDER's own text, unlit and
  untonemapped, sRGB-encoded.
- **Normals:** object space, rgb = n·0.5 + 0.5.
- **Gas colour.**
- **Layouts:** UDIM (1001 + f) or a 3×2 atlas (capped at 4096 per face). An optional equirect
  carries height and colour.
- **Sidecar JSON:** the range and decode rule, R0, sea level, the faceToDir convention
  (row 0 = top, s = ((col + 0.5)/S, 1 − (row + 0.5)/S)), and the equirect convention.

**Limits.**
- Erosion, creep, talus, tectonics and the fold record are upsampled, not re-simulated.
- A crater's fine residual returns at full strength even where later erosion or deposition
  softened it.
- Excavation detail assumes bare rock (× uIsoK), so it is understated where the sim excavated
  sediment.
- The crust rewind is rigid per current plate.
- The noise residual also lands on crust minted since init.
- Catmull-Rom can overshoot steps by about 12 %.

Unmeasured: all of it. That covers bake time per resolution (shown as measured during a run),
GPU cost of the 16-sample grid pass on large craters, the auto-range margin, PNG sizes, and
whether 16384 completes on real hardware.

## v013 — a base noise per planet

`Orogeny_v013.html`. Every rocky planet in `planet-presets.js` now names a base noise
(`terrain:`), a terrain preset from `presets-builtin.js`. Seven new ones:
- Orogen ridges
- Supercontinent
- Island scatter
- Mars highlands
- Volcanic plains
- Caldera pits
- Soft lowlands

Plus the existing Continents, Lunar maria, Ice lineae, Dune sea and Lava crust.

Choosing a planet loads its base noise through the same path as picking it under *Terrain
noise*, so it shows there and can be replaced. Plate continents are still laid on top when a
plate source is active. If the preset store lists only after the planet was built (first load),
the world is rebuilt once on the base noise. Gas giants have none; their gas pattern plays that
role. The noises are first passes by reasoning, without gradients.

## v014 — every module on every planet

`Orogeny_v014.html`. A planet preset now decides what is **active**, not what **exists**:
- Every section shows on every planet. `planet.modules` is no longer read; the presets' `set`
  switches (tectonics, impacts, erosion, water, gas) do the work.
- Rock sections dim (not hide) when the Rock body is off.
- New first section, **Base noise**: the terrain-noise select (moved out of the bottom bar),
  Relief (nAmp) and Frequency (nFreq, which also overrides a single preset's own frequency), both
  rebuilding the planet on release, and a link to the noise designer.
- **Ocean** and **Gas** carry their Active switch in their headers like the other modules. The top
  bar keeps only the Rock switch; "Show all modules" is gone.

Proposed section order:

| Left: the body | Right: surface and envelopes |
|---|---|
| Base noise | Erosion |
| Tectonics | Ocean |
| Impacts | Gas |
| Volcanism *(to come)* | Ice, Wind, Climate *(to come)* |

## v015 — export bake, after the first real bakes

`Orogeny_v015.html` (bake code only). The first 4096 bakes showed halos along fault scarps, no
visible extra resolution, beaded rivers, grid-pitch bumps, and a few hairlines.

**Causes found and fixes made.**
- **Halo along scarps:** plain Catmull-Rom overshoots a step by about 12 % on both sides. Every
  cubic is now clamped to the range of its 2×2 bracketing texels, the MacCormack limiter idea.
  That covers the surface and the grid side of both residuals. Steps become monotone one-cell
  ramps, and cell-scale extrema lose the overshoot between samples.
- **Hairlines at cube edges:** dirToFace switches face, and the two faces' cubics differ. Within
  1.5 cells of an edge the sample is now blended with the neighbour face's chart (½·½ at the
  edge). The cube corners are continuous but not symmetric.
- **No visible detail:**
  - Preset octaves are already resolved at 512, so residual (b) was only interpolation error.
  - The run had 0 strikes, so (c) was zero.
  - The base noise is now evaluated with its octaves extended: auto until noise-lib's fade at the
    export texel (capped at 16), or a fixed +N.
  - For fBm, turbulence, ridged and billow the old octaves keep their exact weight, using the
    rescale (v − β)·n_ext/n_orig + β.
  - Composites and non-additive combinators are approximate.
  - The built-in inits use the same rule.
  - The panel reports the octaves added, their amplitude share, a low-gain hint and a 0-strike
    warning.
- **Grid-pitch bumps:** an optional 3×3 median on the sim grid before upsampling (Sim despeckle,
  default 1). It is the identity on locally planar ground and keeps steps; river cells are exempt.
- **Beaded rivers:** discharge membership is blurred [1 2 1]², and the surface inside that ribbon
  is pulled to a membership-weighted channel floor. The river colour uses the same ribbon.
- **Extra detail** (default 0): a warped ridged sub-grid texture, with constant slope per octave,
  stronger on slopes and weaker in sediment.

**Not addressed.**
- Staircase diagonals need edge-directed reconstruction.
- Scarps stay a one-cell ramp: the grid does not hold the sub-cell fault position.
- With crust motion, the rewind uses the ω of the plate under each texel. ω jumps at plate
  margins, so the residuals can show a line there.

Unmeasured: all thresholds (median exemption, ribbon edges, blend width, slope and sediment
ramps), and the look and cost of every change.

## v018 — volcanism

`Orogeny_v018.html` adds a Volcanism module in the left panel, after Impacts; `planet-presets.js`
gives the planets volcanism settings. It is off by default, and volcanism is a crust SOURCE.

**Sources.** All are fixed in the mantle, which here means the world frame.
- **Hotspots:** N seeded plume directions. Each pulses on or off per 300-step epoch (hashed).
  Deposition happens in `volcStep`, after the step's crust advection, at the plume's fixed
  direction. On moving plates every pulse lands on new crust, so an age-progressive island chain
  emerges; with the crust still (Mars) the pulses pile into one shield.
- **Arcs:** stratovolcano spots inboard of convergent margins:
  - ocean–continent: on the continental side at the arc offset;
  - ocean–ocean: on one side.
  They come from the analytic plate field. The spots are a jittered world-frame lattice, each
  with its own duty cycle.
- **Rifts:** weak fissure melt along divergent margins.
- Arcs and rifts are baked into a source field every PLATE_EVERY steps, and need plates
  (source ≥ 1).

**Edifices.** Part of each eruption goes straight into tc (isostasy applies) with a radial
profile, shield (1 − x²) ↔ strato ((1 − x)^2.2). The hotspot amplitude is normalised
analytically to the intended volume. A resting plume's summit occasionally collapses into a
flat-floored caldera.

**Lava.** The rest of each eruption becomes a melt-thickness field. It flows down the lava
surface with flux ∝ L³ and a Bingham yield (thickness × slope). Each face's flux is limited to
1/8 of the giver's stock. It is conservative while it moves, and freezes into tc faster where it
is thin. Under it:
- sediment is welded into the column;
- freshness goes to 0 (fresh basalt is dark);
- a basalt-cover channel darkens the palette until it weathers or is buried.

**Resurfacing** is the same model pushed: more effusion, lower yield, slower cooling, so lava
ponds in low ground and buries craters. **Heat** is set where melt lies and decays; it drives an
orange→dark-red emissive glow and a new "Lava heat" view. The melt, heat and cover ride the crust
in `advectCrust`.

**Mass.** A per-cell ledger accumulates every mantle addition: edifices, melt, calderas (−) and
the sink (−). The mass audit books the volcanic addition separately and shows the remaining drift.
Optional "Compensate" is an integral controller: every 120 steps it re-reads the ledger and
adjusts a uniform sink.

**Presets.**

| Planet | Volcanism |
|---|---|
| Mars | 3 strong plumes on a still crust (giant shields), calderas, no glow |
| Venus | 14 shields, strong resurfacing, low glow |
| Io | 16 vents, long flows, resurfacing, strong glow |
| Archipelago | hotspot chains and island arcs |
| Lava world | everything strong |
| Terran, Young orogen | modest arcs and a few hotspots |

**Limits.**
- Arcs and rifts use the analytic plate layout, which is only approximate for the advected-id
  source (2) and absent for noise belts (0).
- Lava is basalt everywhere; there is no sulphur chemistry.
- The melt field is advected first-order.
- Craters buried by lava keep their full fine residual in the export bake.
- Every rate constant is derived from a stated target size, not measured.

Unmeasured: all of it. That covers the rates, the chain spacing, flow stability at high
fluidity, the glow look, the compensation controller's convergence, and the per-step cost
(about 5 extra grid passes, plus the source bake every 50 steps).

## v019 — saved planets, atmosphere, aurora

`Orogeny_v019.html` and `planet-presets.js`. `preset-store.js` is unchanged.

**Saved planets.** There are Save, Save as… and Delete buttons under the Planet picker, and they
use a styled modal instead of window.prompt or confirm.
- **Storage:** user planets go through the shared store (kind `planet`, localStorage
  `orogeny.planetPresets`, with the same cross-tab doorbell). They merge with
  `window.OrogenyPlanets` by `B.merge`, so a saved record wins on id.
- **Save** asks before overwriting. On a built-in it stores a record with the same id, which
  replaces the built-in in the picker; deleting that record brings the built-in back.
- **Picker labels:** "· saved" marks user planets and "· modified" marks overridden built-ins.
- **Record shape:** the same as a built-in preset plus a few fields:
  - `set` = P minus VIEW_KEYS, including the volcanism, atmosphere and aurora keys;
  - `look`, including atmos and aurora;
  - `seed` and `plateSeed`;
  - `terrain` (preset id) or `set.initMode` (built-in mode), plus `baseOn`;
  - `pattern` and `colorMap`;
  - `crater` {id, params}, where the live preset is used while it exists and the snapshot
    otherwise;
  - `phases`, `modules` and `missing`, all copied.
- `setPlanet` restores all of it, including base noise off, through `applyWanted`.

**Atmosphere.** The fixed limb glow and dayside tint are replaced by single scattering through
a spherical shell (Nishita / Bruneton–Neyret form, single scattering only).
- Rayleigh uses per-planet RGB optical depths, so the λ⁻⁴ colour comes from the numbers.
- Mie uses Henyey-Greenstein with g per channel and a per-channel albedo, so a haze can absorb.
- Ozone is a tent-shaped absorbing layer.
- Coefficients are set from vertical optical depths (β = τ/H; Earth's Rayleigh τ ≈ 0.046,
  0.108, 0.265 for R, G, B). The shell itself is exaggerated: 0.1 R by default.
- Each ray takes 14 view samples × 4 sun samples, and gets no sun where the planet occludes it.
  That one loop gives the limb, sky, sunset reddening, and aerial perspective over land, sea,
  clouds and gas-giant discs.
- Surface sunlight is also reddened through the air.
- Sliders: thickness, density, scale height, haze, haze g, ozone and sun.
- Airless bodies (Luna, Io, Europa) are off.

**Aurora.** An emission term in the same loop:
- ovals at a set magnetic colatitude around a tilted magnetic axis, both poles;
- a folded centre line and ridged-noise rays, animated;
- colour by height in the shell: N₂ violet low, O green, O red high;
- night side mostly (×0.15 in daylight);
- added before the tonemap.
Presets: Terran 0.8, Jupiter 2, Saturn 1.6 (violet-pink), Neptune 0.4 (47° tilt); everything
else off.

**Export bake.** The colour map stays unlit, with no atmosphere and no aurora.

Unmeasured: all of it. That covers render cost on real GPUs (14×5 density evaluations per shell
pixel), every per-planet optical depth (first guesses), the shell fractions, the aurora mapping
and its brightness, and the save/restore round trip.

## v020 — typed slider values

Click the number next to any slider to type a value; Enter or leaving the field
commits, Esc cancels, a comma works as a decimal point. A value outside the
slider's range is kept (the slider pins at its end, the number turns amber) and
survives preset loads, phase changes and saves. Dragging the slider replaces it
with the slider's own value again. Only real (trusted) input events overwrite P
from the slider; `syncDom` and dispatched events keep a typed value. Typed
values commit with synthetic `input` and `change` events, so rebuild-on-release
hooks still fire. The export dialog's own sliders are unchanged.
`Orogeny_v020.html` only.

## v021 — bursts obey the Impacts switch

A phase `burst` (Luna's 500-strike heavy bombardment, Mars's dry-phase 120)
fired regardless of the Impacts switch, so New seed with Impacts off still
cratered the Moon. A burst is now only queued while Rock and Impacts are both
on, and switching Impacts off mid-burst drops the remaining strikes.
`Orogeny_v021.html` only.

## v022 — Bombardment = initial strikes + rate

One slider for both: a fresh world (new seed, planet change, rebuild) opens
with `round(bombard)` strikes, landed 4 per frame, and keeps the old rate after
(`bombard·0.00004` per step). It is live: `initDone` is compared with the
current slider, so raising Bombardment lands the difference; lowering it
cannot take strikes back. With Impacts (or Rock) off the opening strikes are
marked done instead of queued. Slider range is now 0–500. Phase-0 `burst`
values are ignored (bursts of later phases, Mars's dry phase, still fire);
the presets moved them into `bombard`: Luna 500, Venus 30, Europa 12. Older
versions read the same `planet-presets.js`, so their Luna/Venus/Europa
rates are higher now.
`Orogeny_v022.html`, `planet-presets.js`.
