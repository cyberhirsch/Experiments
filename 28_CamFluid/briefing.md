# № 28 — CamFluid

The webcam's motion vectors drive a flat 2D fluid; gas or particle colour comes from the webcam rgb.
Sources: the Orogeny atmosphere (№ 14, v007 / v010) and the Gas Giant FLIP solver (№ 02), both
taken off the sphere onto one closed rectangular grid with square cells.

## v001

`CamFluid_v001.html`, one file, WebGL2 (needs `EXT_color_buffer_float`). Keys: H settings, V vectors,
C camera underlay, G / F solver, R reset, Space pause. Settings persist in localStorage.

**Motion vectors.** Lucas-Kanade on the GPU at 96 px height: mirrored, cover-fitted luminance; Ix, Iy
from the mean of two frames, It from their difference; the tensor products blurred with a separable
9-tap Gaussian; a 2×2 solve with a ridge term (2e-4); noise gate, clamp at 6 px, temporal smoothing.
Flow (px per camera frame) becomes grid cells per second.

**Forcing.** v += (flow − v) · clamp(push · |flow| · 6 dt), then vorticity confinement and damping.

**Gas.** Midpoint semi-Lagrangian advection of velocity, forcing, divergence, warm-started Jacobi
(30), projection with closed walls. The dye (2× grid) carries rgb paint plus density: where there is
motion, camera colour and density flow in; density fades; optional ambient camera paint.

**FLIP.** Particles (16k–524k) in RGBA32F textures: position, velocity, colour. P2G as 4 bilinear
point splats per particle, additive; normalise; forcing; keep the pre-projection grid; pressure;
G2P mix(PIC, FLIP, 0.97); RK2 advection; wall bounce; random respawn with fresh camera colour;
colour drifts towards the camera colour under the particle (Live colour).

## v002 — settings in view

`CamFluid_v002.html`. A Settings button under the solver switch (and H) shows the panel, open by
default. **Amount:** particles 16k … 2.1M (1448² texture; about 200 MB of particle textures at the top
step). **Resolution:** fluid grid 48–512 cells high, dye detail 1–4× the grid, motion-vector grid
48–240 px, render scale 0.25–1 of the device pixels, pressure iterations 4–120. **Forces:** strength
(scales the flow → velocity target), push (how fast the fluid locks onto the flow), swirl (vorticity
confinement), damping. Double-click a slider to reset it. Settings are stored under a new key.

## v003 — settings per solver, GPU presets, colour, velocity gradients

`CamFluid_v003.html`. **Per solver:** Gas and FLIP each keep their own full set (resolution, forces,
motion, colour, view); switching solver switches the set and rebuilds the grids. Stored under
`camfluid.v003` as `{ mode, gas, flip }`.

**GPU presets** (Low GPU / Default / High GPU) set only the cost keys, never forces or colour; the
active one lights up and shows in the stats.

| | grid | dye | vectors | render | iter | particles |
|---|---|---|---|---|---|---|
| Gas low / high | 96 / 288 | 1× / 3× | 64 / 160 | 0.5 / 1 | 16 / 60 | — |
| FLIP low / high | 96 / 256 | — | 64 / 160 | 0.5 / 1 | 16 / 50 | 65k / 1M |

**Colour.** The scene renders linear into a half-float target; a post pass applies exposure (EV),
temperature, saturation, hue shift (rotation about the grey axis), tone map (clamp, Reinhard, ACES),
contrast and gamma. The motion vectors are drawn after grading.

**Velocity gradients.** 14 palettes (FUI cyan, Inferno, Magma, Plasma, Viridis, Turbo, Fire, Ice,
Neon, Toxic, Sunset, Ocean, Copper, Mono) in one 256 × N texture, plus Direction (hue from the
flow angle, value from the speed). The speed |v| / speed range (cells per second) picks the colour;
Gradient mix blends it with the camera colour. Gas samples the grid velocity per pixel and Field
shows the gradient outside the dye too; FLIP colours each particle by its own velocity.

## v004 — your own presets

`CamFluid_v004.html`. Under the GPU presets, *My presets* (per solver): Save stores the solver's
whole current set under a name (asks before overwriting another name), Load restores it on top of
the defaults (so presets from older versions with fewer keys still load), Delete removes it. Export
writes every preset of both solvers to `camfluid-presets.json`; Import merges a file in by name. Kept
in localStorage under `camfluid.presets`, independent of the version key.

## v005 — HEIC snapshots, soft dissolve

`CamFluid_v005.html` + `coi-sw.js`.

**S saves a HEIC** of the clean frame: read back right after the colour grade, before the motion
vectors (the HTML overlay is never in the canvas), at canvas resolution (render scale matters).
Browsers cannot encode HEIC themselves (`toBlob` does PNG / JPEG / WebP), so the encoder is
[icodec](https://github.com/Kaciras/icodec) 0.6.0 (MIT; libheif + x265, GPL) from jsDelivr, in a
module worker: quality 82, x265 preset fast, 4:2:0, sharp YUV. x265 runs threads on shared memory,
which needs a cross-origin-isolated page; `coi-sw.js` is a service worker that adds COOP same-origin
and COEP credentialless to every response (the page reloads once after it registers). Works over
http(s): GitHub Pages or localhost. On `file://`, or when anything fails, S saves a PNG and says why.
The encoder's pthread workers load the encoder script itself, so the script is fetched and run from
a blob with its self-URL patched to that blob.

**Dissolve (opacity gradients).** v003 showed hard-edged grey blobs: empty dye cells snapped to the
camera colour, and the display used max(density, field), a hard step. Now:
- Gas dye is premultiplied (rgb · density); injection adds camera colour × amount. *Softness*
  diffuses the dye a little each step; *Breakup* (with *scale*) varies the fade rate with drifting
  value noise, so it dissolves in wisps. Display: opacity = (density / *edge*)^*curve*, the dye
  composited over the field layer.
- FLIP particles have a lifetime (age in pos.z, life factor 0.5–1.5 in pos.w, life = factor /
  respawn): *fade in*, *fade out* and *curve* shape the opacity over the life, *soft points* adds
  a radial falloff. Respawn 0 = immortal particles at full opacity.

## v006 — open borders

`CamFluid_v006.html`. *Borders* (per solver): a checkbox per screen edge; checked = open. An open
edge is an outflow boundary: pressure 0 outside (Dirichlet) instead of the mirrored wall (Neumann),
the normal velocity is not zeroed, dye traced back from beyond it comes in empty, and FLIP particles
that leave through it respawn inside. All closed = the v005 box.

## v007 — fill container, emit from motion (FLIP)

`CamFluid_v007.html`. Particles can be dead (colour alpha 0: no P2G weight, not drawn). They die at
the end of their life (respawn) or through an open edge. *Fill container* rebirths them anywhere
(box stays full; off = starts empty). *Emit from motion* lets each dead particle try 4 random spots
per step, accepted with probability *emit rate* · motion · dt (motion = the gated flow magnitude);
it starts with the grid velocity and camera colour there. Both on: motion first, random fill as the
fallback. Neither: dead stays dead (R restarts). Toggling either restarts the particles.
Particle menu extends to 4.2M / 8.4M / 16.8M (2048², 2896², 4096²; 96 bytes per particle for the ping-pong state, 1.6 GB at 16.8M).

## v008 — open borders stop reflecting particles

`CamFluid_v008.html`. v006 kept the FLIP wall bounce (clamp to half a cell, reverse the normal
velocity × 0.3) on all four edges, open or not, so particles in the last half cell before an open
edge were thrown back with their velocity reversed. The bounce now runs only on closed edges; at an
open edge a particle keeps its velocity, crosses and dies. The grid side (Dirichlet pressure, no
normal-velocity zeroing, empty inflow) was already open.

## v009 — FLIP defaults

`CamFluid_v009.html`. FLIP starts with the camera underlay on (0.35) and all four borders open; Gas keeps
the closed box without underlay. New storage key, so the new defaults apply.

## v010 — Gas defaults

`CamFluid_v010.html`. Gas starts with the camera underlay on (0.35) and the High GPU set (3090): grid 288,
dye 3x, vector grid 160, render scale 1, 60 iterations. Default and High GPU coincide for Gas.

## v011 — four GPU tiers, vectors off

`CamFluid_v011.html`. The panel heading is gone; the tier buttons sit on top: Default (RTX 3090
class, also the start values), High, Medium, Low. Cost keys only.

| Gas | grid | dye | vectors | render | iter |
|---|---|---|---|---|---|
| Default | 288 | 3× | 160 | 1 | 60 |
| High | 224 | 2× | 128 | 1 | 45 |
| Medium | 160 | 2× | 96 | 0.75 | 30 |
| Low | 96 | 1× | 64 | 0.5 | 16 |

| FLIP | grid | vectors | render | iter | particles | point |
|---|---|---|---|---|---|---|
| Default | 256 | 160 | 1 | 50 | 4.2M | 1.0 |
| High | 192 | 128 | 1 | 40 | 1.0M | 1.2 |
| Medium | 128 | 96 | 0.75 | 30 | 262k | 1.6 |
| Low | 96 | 64 | 0.5 | 16 | 65k | 2.0 |

Motion vectors start off (V shows them).
