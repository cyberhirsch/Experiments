# № 11 — Erosion

**Volumetric (true 3D) erosion of desert rock — mesas, hoodoos, arches.**

Classic terrain erosion is 2.5D: a heightmap `h(x,z)` with water running downhill.
It cannot produce overhangs, arches, caves, sea stacks or hoodoos — exactly the
forms that read as *erosion* rather than dunes. This experiment goes fully
volumetric: the eroding material is a 3D density field, so undercuts and
free-standing spires emerge on their own.

## Subject — Desert mesa / hoodoos

Differential erosion of layered sedimentary rock. Hard *caprock* bands resist
weathering and protect the softer strata beneath, leaving banded vertical spires
(hoodoos, à la Bryce Canyon). Lateral hardness variation decides which columns
survive, so isolated pinnacles separate out of a solid plateau.

## Three implementations (built in sequence)

| Ver | Representation | Render | Notes |
|-----|----------------|--------|-------|
| v001 | 128³ density grid | SDF raymarch | Realtime, no meshing. Differential dissolution + lateral undercut. |
| v002 | voxel density | marching cubes | Meshed surface, lit geometry, crisper silhouettes. |
| v003 | material points | particle / splat | MPM granular collapse, talus and slumping. |

## v001 model (GPU compute, ping-pong)

Per step, for each solid voxel exposed to empty neighbours:

```
soft   = 1 / hardness(p)                 // hardness = vertical strata + caprock + lateral noise
rate   = dt · soft · (rain·topEmpty + wind·0.25·sideEmpty)
dens  -= rate                            // dissolve at the exposed surface
```

- **Rain** erodes downward where sky is exposed above.
- **Wind** undercuts laterally — this is what pinches hoodoo waists.
- **Caprock** (hard thin bands) erodes ~slowly, capping the spires.
- **Bedrock** floor is held solid so pinnacles don't float away.

### Structural soundness (connectivity sweep)

Volumetric erosion happily carves rock loose into chunks connected to nothing —
visually wrong (floating islands). Fixed with an **exact connectivity sweep** over
three r32float fields:

- **reset** seeds bedrock = `1`, everything else `0`;
- **flood** does pure-max propagation through connected solid rock (no decay);
- after a full sweep (`~2.5N` iterations, spread `32`/frame across the two working
  buffers) the result is **published** to a third buffer that the erosion pass reads.

Every voxel reachable from bedrock ends up exactly `1.0`, everything else `0.0`;
the erosion pass deletes any solid voxel whose published support is `< 0.5`.
Because connectivity is recomputed from scratch each sweep, a chunk that
disconnects is simply not reached by the next sweep and is culled within **one
sweep period (~0.2–0.25s)** — independent of structure height. (An earlier
attenuation-decay version was correct but slow: gentle decay was needed to protect
tall connected spires, which made floater removal take 1–2s.) Culling is gated
until the first sweep publishes. Toggle: **Remove floating rock**.

Rendered by raymarching the trilinearly-filtered density texture; surface normal
from the field gradient, desert palette keyed to the same hardness function so the
strata you see are the strata that eroded.

### Sedimentation (talus slumping)

Mass is conserved into a second material: the density field is **rgba16float** with
`.r` = rock, `.g` = loose sediment. Eroded rock becomes sediment (`deposit`); a
**slump pass** settles it — conservative vertical gravity exchange plus an
angle-of-repose lateral spread that only acts once sediment is resting. Sediment is
soft and slowly washes away (so piles don't grow without bound), and renders as a
distinct matte sand colour vs. the banded bedrock. Controls: **Sedimentation**
toggle, **Sediment yield** (deposit), **Talus angle** (repose).

Two subtleties found via GPU readback:
- The floater cull must **keep** sediment (delete only unsupported rock) — loose
  sediment has `total < 0.5` so the flood reads it as empty; deleting it would
  annihilate every pile before it forms.
- The lateral repose must be **mass-conserving at cliff edges**. An early version
  only spread "resting" sediment, so sediment shedding off an edge was given away
  by the source but never received by the empty neighbour over the void → silently
  destroyed (exactly the talus path). Fixed with a symmetric `sign()`-odd exchange
  gated only by a symmetric rock-full test; the slump now runs 8 iters/frame so
  sediment actually reaches the slope feet. Talus piles at the **foot of eroded
  slopes** (not the intact base, which has no open space yet).

## Roadmap / ideas

- Deposition pass (sediment fills talus slopes at the angle of repose).
- Coastal-cliff and karst-cave variants (oscillating waterline / interior voids).
- Wind direction control → asymmetric undercutting, yardangs.
