# Research Briefing: № 18 — Galaxy Colonization

> Project Status: **M1 substrate + M4-lite Landis percolation built** — threshold reproduced
> Domain: **Galactic structure · discrete-event simulation · GPU compute**

**Modelling the spread of a civilisation across the Milky Way on real numbers:
published stellar density models, sub-light travel, and the cosmic and
civilisational hazards that stall or erase a colony. Built on the established
literature — Landis percolation, the Carroll-Nellenback settlement model,
Sandberg probe diffusion — rather than invented physics.**

---

## 1. Why this one leaves the browser

Every other experiment in this volume runs in a tab. This one cannot, and the
reason is a number rather than a preference: the substrate alone is 16 bytes
per sampled system, and a billion systems is ~15 GiB of buffer. WebGPU in a
browser will not allocate that — it is two orders of magnitude past what a tab
is given. So the stack is Rust + wgpu + WGSL + winit/egui, native, with the
RTX 3090 as the reference target.

That is the one place this experiment breaks the volume's pattern. It is not
in `index.html`, because there is nothing for a browser to launch.

```
cargo run --release                    # the simulator
cargo run --release --example limits   # headless: what this GPU will allocate
cargo test                             # WGSL validation + uniform layout check
```

---

## 2. What the hardware actually says

The PRD budgets 16 bytes per system and concludes that 10⁹ systems fit a 24 GiB
card. The arithmetic is right and the conclusion is fragile, because §6 also
requires a per-system `next_event_time` and §8 a spatial hash over the same
points — neither of which is in the 16. `examples/limits.rs` asks the driver
directly rather than reasoning about it. On the reference machine:

```
adapter                    NVIDIA GeForce RTX 3090 (Vulkan), driver 596.36
max_buffer_size                  4.00 GiB
max_storage_binding              2.00 GiB
max_compute_workgroups/dim      65535
```

Three consequences, none of them in the PRD:

**A storage binding holds 179 M systems, so 10⁹ needs six position buffers.**
The binding limit is 2 GiB, not the 4 GiB of `max_buffer_size`, and position is
the widest per-system array at 12 bytes. Every compute and render pass over the
full galaxy has to be chunked across bindings. This is an architectural
constraint on every pass that will ever be written, and it is worth absorbing
now rather than at 700 M systems.

**One dispatch dimension reaches 16.8 M invocations.** 65535 workgroups × 256.
The default 20 M systems already exceeds it — the first generation call would
have failed validation on launch. Generation now dispatches a 2-D grid and
rebuilds the linear index from `num_workgroups`.

**Whether 10⁹ fits comes down to two decisions the PRD leaves open.** §8 offers
"f64 or integer years" for time; taking f64, plus a conventional spatial hash
carrying its own index array, gives:

| layout | 10⁹ systems | |
|---|---|---|
| f64 years + indexed hash | **26.54 GiB** | over a 24 GiB card |
| u32 decades + sorted-in-place hash | **19.09 GiB** | fits |

The saving is 7.45 GiB from two choices. Event times are stored as **u32
decades**: a 10¹⁰ yr run is 10⁹ decades, u32 reaches 4.29 × 10⁹, so a decade
tick spans the whole run with four times the headroom at half the footprint of
an f64 — and 10 yr resolution is far finer than the 10²–10⁴ yr hop the fast
clock needs. The hash keeps the system arrays **sorted by cell** so there is no
separate index array to pay for; the points drift and the grid rebuilds
periodically anyway, so sorting in place costs nothing extra.

---

## 3. The substrate

Four components, each drawn by inverse transform where an exact inverse exists.
Only the spiral-arm modulation needs rejection, so generation is one pass.

- **Thin and thick disk** — Juric et al. (2008): scale lengths 2.6 and 3.6 kpc,
  scale heights 300 and 900 pc.
- **Bar / bulge** — Wegg & Gerhard (2013) axis ratios, 27° to the Sun–centre
  line, as a triaxial gaussian. The true profile (Dwek G2) is boxier; the
  difference is a slightly softer bar edge.
- **Halo** — power law index 2.77, flattening q = 0.64, between a 1 kpc core
  and a 100 kpc cutoff.
- **Spiral arms** — Reid et al. (2019) VLBI parallaxes, pitch ~12.5°, applied
  as an angular modulation so the radial profile is untouched.

**The sampling result worth keeping.** An exponential disk's radial law is
*exactly* a Gamma distribution. Surface density Σ(R) ∝ exp(−R/h) gives
P(R) ∝ R·exp(−R/h) dR, which is Gamma(shape 2, scale h) — and a Gamma(2,1)
draw is simply −ln(u₁u₂). The usual approach is Newton iteration on a CDF that
has no closed-form inverse; this is two logarithms, exact, unbiased, branchless.

Each system also draws a mass from the Kroupa (2001) broken power law, which
gives a spectral class and (later) a main-sequence lifetime — the hard
per-system clock behind the stellar-evolution hazard. Metallicity falls
outward and is poor in the halo, and gates a habitability score: F/G/K carry
it, O/B die before biology finishes, M flare and tidally lock.

Packed state is one u32 — component, class, habitability, with the top 16 bits
reserved for colonisation. Positions are `f32×3`. 16 bytes, as budgeted.

## 4. Render

Vertex pulling into a `PointList`: no vertex buffer, one vertex per system read
straight out of the storage buffer the compute pass wrote. At 10⁸ points a quad
sprite would cost six vertices each plus an overdraw bill, and a one-pixel
additive point is the physically honest mark anyway — a galaxy image *is* the
accumulated density along the line of sight. No depth buffer, for the same
reason. Colour by spectral class, component, or habitability.

## 5. Verification

`cargo build` never looks at WGSL — wgpu compiles it at `create_shader_module`,
which is to say at run time, in front of the user. `tests/shaders.rs` runs
naga's front end and validator over both modules, and checks that the uniform
block still has the 28 scalars the Rust mirror declares. A silent divergence
there would make every field past it read garbage on the GPU, and the galaxy
would come out wrong in a way that looks like a modelling mistake.

## 6. M4-lite — Landis percolation from the Sun

Skipped straight to the science, because the PRD's own success metric is
reproducing Landis (1998), and nothing built so far tested whether the model
does that. Scaling is engineering with a known answer; the science is the part
that could be wrong.

**The reduction.** Landis: each colony is, with probability P, a *colonising*
one that sends ships to every reachable neighbour, and with probability 1−P
one that never does. The coin belongs to the *site*, not to whoever reaches it
first. So the arrival time at every system is simply the shortest travel time
from the seed through open sites — a single-source shortest path. Compute it
once and the entire history is a static field; "now" is a threshold on it.
No stepping, no drift, exact to the definition. The time slider costs nothing
and can be scrubbed both ways. Hazards that kill colonies *after* the fact will
break this and need real stepping; Landis alone does not.

**Reachability is by neighbour count, not distance.** At 20 M sampled systems
the nearest sampled neighbour near the Sun is 64.7 ly; at 10⁹ it is still
17.6 ly; real stars sit ~11 ly apart. A physical ship range of 10–30 ly would
reach nothing at any sampling this project will run. "The k nearest" is
scale-free — and it is how Landis actually posed the problem: a lattice has a
coordination number, not a radius.

**Implementation.** k-d tree (kiddo) over the positions read back from the
GPU, k_max = 12 neighbours per system, Dijkstra with lazy deletion keyed on
`f32::to_bits` (order-preserving for non-negative floats, so no ordered-float
dependency). The site coin is a splitmix64 finaliser over (index, seed): a
site open at P is open at every larger P, so a sweep is a nested sequence
rather than twenty unrelated galaxies. CPU for this milestone — exact,
testable against brute force, and 20 M is within reach; GPU when scale
demands it. Result packed to one u32 per system: arrival in decades + 1
(0 = never), bit 31 = colonising. The render draws the colonised set at "now"
and burns the front — what arrived inside a window — brightest, so the wave
reads as a wave and not a stain.

**Verified.** The k-d tree's neighbour sets match brute force exactly on 3 000
points × 6. A five-site line with the middle site closed colonises exactly
three and stops, at exactly distance / speed. And on a 30 000-point random
cloud with k = 6:

```
P = 0.45   0.0014        P = 0.55   0.8840
P = 0.50   0.0021        P = 0.60   0.9336
            half-galaxy crossing at P ≈ 0.53
```

That is Landis's result, reproduced: a sharp percolation threshold with an
order parameter that goes from nothing to nearly everything across 0.05 in P.
It sits above his 0.311 for the 3-D cubic lattice because this graph is
*directed* — u's six nearest need not include u — which prunes paths. Symmetrising
the graph would pull it down toward the lattice value; left as a switch to add
when the comparison is wanted precisely.

## 7. Roadmap

- **M1 — substrate** *(done)*: density model, GPU generation, HDR point render,
  measured device limits.
- **M4-lite — Landis percolation** *(done)*: neighbour graph, exact shortest-
  path colonisation, threshold reproduced, P sweep with plot, playback.
- **M2 — chunked bindings**: the six-buffer split the 2 GiB limit forces, and
  the neighbour graph on the GPU (counting-sort spatial hash, no radix sort
  needed) — the step from 20 M to 100 M+.
- **M3 — slow clock**: per-system life timeline (abiogenesis → intelligence,
  main-sequence death) as scheduled events drawn at generation. Needed only
  for the "any system reaches intelligence" variant, not for single origin.
- **M5 — hazards**: GRB, supernova, impact, stellar death; war and AI risk on
  the fast clock.
- **M6 — galactic evolution**: star formation history, metallicity in time,
  differential rotation reshaping the percolation network.

**Open, and worth settling before M4.** The PRD's discrete-event design finds
the next event by a parallel min-reduction over every system. At 10⁹ systems
that is a 4 GiB read per step — roughly 4 ms at the 3090's bandwidth before any
work is done — and a 10¹⁰ yr run takes many thousands of steps. A calendar
queue bucketed by epoch would scan only the active bucket instead. The saving
grace is that the slow clock is almost entirely static: timelines are drawn at
generation and only the colonisation front churns, and the front is a vanishing
fraction of the galaxy. That asymmetry is probably the whole performance story.
