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
