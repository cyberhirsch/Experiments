# Corona FLIP Parameters

This is the complete parameter catalogue for the current `Corona_v001.html` solver and renderer.

Legend:

- `Live`: can be changed while the simulation runs.
- `Reset`: changing it should reseed particles, rebuild field lines, or restart the burst.
- `Rebuild`: changing it requires rebuilding GPU buffers, shader constants, or the solver.

The browser UI now exposes these as two full-screen panels: `Sim` for Solver, Emitter, Sim, and Field controls; `Look` for Rendering and View controls. Settings autosave after a delay, and the popup header includes manual `Save` / `Load` / `Reset` buttons.

## Solver / Performance

| Parameter | Current value | Mode | Notes |
|---|---:|---|---|
| GPU grid resolution | no UI cap, default `128^3` | Rebuild | Dense WebGPU grid resolution. Enter a positive whole number and press `Commit`; changing it recompiles/reallocates the solver. Very large values can freeze the tab or exhaust GPU memory before delayed autosave writes them. |
| Simulation box size | `1.0..5.0`, default `4.7` | Rebuild | Full width of the cubic FLIP domain. Adjust with the box-size slider, then press the same `Commit` button used for grid resolution. |
| CPU fallback grid resolution | `26^3` | Rebuild | Kept small so fallback does not freeze the page. |
| Particle count steps | `4200, 9000, 16000, 32000, 48000, 65536` | Reset | Values exposed by the existing `N` slider. |
| Default burst particle count | `32000` | Reset | Current default slider index. |
| Max particle count | `65536` | Rebuild | Allocated typed-array/GPU particle capacity. |
| Pressure iterations | `40` | Live | Jacobi iterations per frame. |
| FLIP/PIC blend | `0.82` | Live | `1.0` is pure FLIP, `0.0` is pure PIC. |
| Max timestep clamp | `1 / 45` | Live | Caps solver dt for stability. |
| Simulation speed / dt scale | `1.0` | Live | Multiplier before dt is sent to the solver. |
| Face velocity damping | `0.996` | Live | Applied after force integration. |
| Max particle speed | `3.2` | Live | Velocity clamp in `gridToParticle`. |
| GPU readback/update rate | `every available frame` | Live | Position/age readback for the WebGL point renderer. |
| Workgroup size | `128` | Rebuild | Compute shader dispatch group size. |

## CME Emitter

| Parameter | Current value | Mode | Notes |
|---|---:|---|---|
| Active emitter / source spot | `activeEmitterIndex` | Reset | Chooses which bipolar magnetic region emits the burst. |
| Burst radius on surface | `0.085` | Reset | Angular scatter radius for initial surface positions. |
| Surface start radius | `1.026` | Reset | Base radius just above the sun surface. |
| Puff/random depth | `0.024` | Reset | Extra random outward offset. |
| Lift base | `0.95` | Reset | Initial outward velocity. |
| Lift randomness | `0.35` | Reset | Added random lift range. |
| Shear randomness | `0.18` | Reset | Random velocity along the bipolar axis. |
| Spin randomness | `0.36` | Reset | Random tangential/twist velocity. |
| Initial magnetic velocity influence | `0.22` | Reset | Initial push along local magnetic field direction. |
| Magnetic sample radius | `1.05` | Reset | Radius used to sample the magnetic field for initial velocity. |
| Particle lifetime base | `5.5` | Reset | Base particle lifetime in seconds. |
| Particle lifetime randomness | `2.5` | Reset | Extra random lifetime range. |

## Forces

| Parameter | Current value | Mode | Notes |
|---|---:|---|---|
| Gravity/radial pull strength | `-0.30` | Live | Radial force term in `forceAt`. |
| Gravity softness | `0.18` | Live | Softening in the radial pull denominator. |
| Magnetic force scale | `0.020` | Live | Converts magnetic field strength into grid force. |
| Magnetic force cap | `1.15` | Live | Maximum magnetic push. |
| Magnetic radial lift | `0.16` | Live | Extra outward term when field points radially outward. |
| Magnetic field softening | `0.010` | Live | Avoids singular forces near poles. |
| Surface kill radius | `0.980` | Live | Particles inside this sphere are expired instead of bounced off the sun surface. |
| Outer kill radius | `2.75` | Live | Particles beyond this radius are expired. |

## Density / Clumping / Pressure Feel

| Parameter | Current value | Mode | Notes |
|---|---:|---|---|
| Density compression target | `10.0` | Live | Particle count per cell before density pressure starts. |
| Compression cap | `28.0` | Live | Maximum compression contribution. |
| Compression strength | `0.085` | Live | Strength of density-corrected pressure RHS. |
| Cohesion density low | `8.0` | Live | Lower edge for surface cohesion response. |
| Cohesion density high | `32.0` | Live | Upper edge where cohesion fades out. |
| Cohesion max pull | `4.2` | Live | Caps the density-gradient pull. |
| Cohesion gradient scale | `0.16` | Live | Multiplies local density-gradient magnitude. |
| Sparse radial confinement threshold | `8.0` | Live | Density below which particles get pulled inward. |
| Sparse radial confinement strength | `0.075` | Live | Strength of inward sparse-particle confinement. |
| Cohesive velocity scale | `1.0` | Live | Current multiplier for adding cohesion into velocity. |
| Cohesive advection offset | `0.35` | Live | Extra position offset from cohesion. |
| Fluid marker cell quantum | `4` | Rebuild | Encodes per-cell particle counts in `cellType`. |

## Magnetic Field Generation

| Parameter | Current value | Mode | Notes |
|---|---:|---|---|
| Field seed | `17` | Reset | Random seed for magnetic pole layout. |
| Bipolar pair count | `11` | Reset | Number of local bipolar regions. |
| Global positive dipole charge | `1.45` | Reset | Strength of north/global positive pole. |
| Global negative dipole charge | `-1.45` | Reset | Strength of south/global negative pole. |
| Global positive dipole position | `(0.0, 0.88, 0.18)` normalized | Reset | Direction of global positive pole. |
| Global negative dipole position | `(0.0, -0.88, -0.18)` normalized | Reset | Direction of global negative pole. |
| Bipole separation min | `0.10` | Reset | Minimum angular pole separation. |
| Bipole separation random range | `0.18` | Reset | Additional random angular separation. |
| Bipole strength min | `0.65` | Reset | Minimum local pole strength. |
| Bipole strength random range | `0.85` | Reset | Additional random pole strength. |
| Max uploaded poles | `32` | Rebuild | Uniform array capacity for WebGPU magnetic poles. |
| Field lines per positive pole | `10` | Reset | Streamline seeds per positive pole. |
| Field-line seed ring min | `0.025` | Reset | Minimum streamline seed offset around pole. |
| Field-line seed ring random range | `0.060` | Reset | Additional seed-ring offset. |
| Field-line step size | `0.018` | Reset | Integration step for visual field lines. |
| Field-line max steps | `260` | Reset | Maximum streamline integration steps. |
| Field-line outer cutoff | `2.45` | Reset | Stops streamlines that leave the corona. |
| Field-line inner cutoff | `1.012` | Reset | Stops streamlines that return to the surface. |
| Minimum line segment count | `8` | Reset | Shorter field lines are discarded. |

## Rendering

| Parameter | Current value | Mode | Notes |
|---|---:|---|---|
| Particle size young | `24.0` | Live | Sprite size for new CME particles. |
| Particle size old | `10.0` | Live | Sprite size for old CME particles. |
| Particle size age fade start | `0.15` | Live | Start of age-based size interpolation. |
| Particle size age fade end | `1.0` | Live | End of age-based size interpolation. |
| Particle size min | `3.4` | Live | Minimum rendered sprite size. |
| Particle size max | `38.0` | Live | Maximum rendered sprite size. |
| Sprite softness/core falloff | `6.5` | Live | Exponential falloff inside each point sprite. |
| Particle alpha | `0.34` | Live | Alpha multiplier for CME particles. |
| Age fade opaque edge | `0.08` | Live | Young-particle fade threshold. |
| Age fade transparent edge | `1.0` | Live | Old-particle fade threshold. |
| Hot color | `(1.55, 0.52, 0.10)` | Live | Low/young plasma color. |
| Pale color | `(1.20, 0.84, 0.42)` | Live | Older plasma color. |
| High/blue color | `(0.50, 0.82, 1.25)` | Live | High-altitude color. |
| Height color blend strength | `0.45` | Live | How strongly height blends toward blue. |
| Height color range | `1.4` | Live | Radius range for height color ramp. |
| Sun core radius | `0.985` | Live | Dark photosphere mesh radius. |
| Sun core color | `0x1a0500` | Live | Dark sun body color. |
| Corona glow radius | `1.32` | Live | Backside glow shell radius. |
| Corona glow color | `0xffaa33` | Live | Glow color. |
| Corona glow intensity | `0.45` | Live | Alpha multiplier in glow shader. |
| Corona glow rim exponent | `2.5` | Live | Rim falloff exponent. |
| Simulation bounds color | `0x9fcfff` | Live | Thin outline color for the FLIP volume. |
| Simulation bounds opacity | `0.28` | Live | Thin outline opacity for the FLIP volume. |
| Field-line opacity | `0.62` | Live | Magnetic streamline opacity. |
| Field-line near color | `0xffb35c` | Live | Low-altitude field-line color. |
| Field-line far color | `0x75d7ff` | Live | High-altitude field-line color. |
| Field-line color height range | `1.2` | Live | Height range for field-line color ramp. |
| Pole marker size | `0.055` | Live | Surface pole marker size. |
| Pole marker opacity | `0.9` | Live | Surface pole marker opacity. |
| Positive pole color | `0xff6f3c` | Live | Positive pole marker color. |
| Negative pole color | `0x78d9ff` | Live | Negative pole marker color. |
| Star count | `360` | Rebuild | Number of background star points. |
| Star radius min | `9.0` | Rebuild | Minimum random star radius. |
| Star radius random range | `4.0` | Rebuild | Additional random star radius. |
| Star size | `0.018` | Live | Background star point size. |
| Star opacity | `0.7` | Live | Background star opacity. |

## View / Interaction

| Parameter | Current value | Mode | Notes |
|---|---:|---|---|
| Rotate enabled | `true` | Live | Existing `Rotate` button. |
| Auto-rotation speed | `0.12` | Live | Current per-second sun rotation term. |
| Magnetic field visible | `true` | Live | Existing `Field` button. |
| Fluid visible | `true` | Live | Existing `Fluid` button. |
| Camera FOV | `35` | Live | Perspective camera field of view. |
| Camera initial distance | `4.5` | Live | Initial camera z position. |
| Orbit damping enabled | `true` | Live | OrbitControls damping. |
| Orbit damping factor | `0.08` | Live | OrbitControls damping factor. |
| Orbit rotate speed | `0.6` | Live | Mouse/touch orbit speed. |
| Orbit min distance | `1.6` | Live | Minimum camera distance. |
| Orbit max distance | `8.0` | Live | Maximum camera distance. |
| Orbit zoom speed | `0.6` | Live | Wheel/pinch zoom speed. |
| Renderer pixel ratio cap | `2.0` | Live/Rebuild | Current `Math.min(devicePixelRatio, 2)`. |

## Browser UI Layout

`Sim` popup:

- Solver / Performance
- CME Emitter
- Forces
- Density / Clumping / Pressure Feel
- Magnetic Field Generation

`Look` popup:

- Rendering
- View / Interaction
