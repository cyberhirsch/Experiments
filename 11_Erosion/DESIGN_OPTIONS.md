# № 11 — Erosion · Design Options & Decisions

Running log of every design fork raised while building this experiment, the option
chosen, and the alternatives left on the table. Kept so we can revisit and iterate.
**Bold = currently chosen / implemented.**

---

## 1. Subject (what erodes)

- **Desert mesa / hoodoos** ✅ — layered sedimentary rock, hard caprock protecting soft
  strata → banded vertical spires. Biggest visual payoff.
- Coastal cliff — oscillating waterline undercuts a cliff; sea arches & stacks calve off.
- Karst caves — interior dissolution carving void networks, revealed via cutaway.
- Asteroid ablation — abstract space-rock erosion (fits the Black Hole / Galactica family).

## 2. Representation & render (the three planned versions)

| Ver  | Representation        | Render          | Status |
|------|-----------------------|-----------------|--------|
| **v001** | dense density grid | **SDF raymarch** | ✅ built |
| v002 | voxel density         | marching cubes  | planned |
| v003 | material points       | particle / splat (MPM) | planned |

Decided to build **all three in sequence**, SDF first as the realtime base.

Alternative representations considered: multi-layered heightmaps (cheaper arches, what
the 2024 SOTA papers use), implicit construction trees (memory-efficient). We chose
**dense voxels brute-forced "because we can"** at experiment scale — uncommon as a live
web toy, which is the novelty (see literature note below).

## 3. Hardness field — noise type

The hardness field decides which rock resists erosion (→ what survives as spires).
Selectable noise type:

- **Strata** ✅ default — layered beds + hard caprock bands (sin in y).
- Value fBm — blobby mounds / knolls.
- Cellular "boulders" — Worley F1 inverted; hard clusters at cell centres → rounded
  goblins (closest to the "Goblins by Spheroidal Weathering" look).
- Cellular "cracked ridges" — Worley F2−F1; hard along cell borders → polygonal fins.

Future noise ideas: ridged fBm, domain-warped Worley, billow, hybrid multifractal.

## 4. Hardness field — controls

- **Noise scale** — feature frequency (resolution-independent, referenced to 128³).
- **Octaves** — fBm detail layers (1 smooth → 6 rugged).
- **Hardness contrast** — soft↔hard spread (controls how cleanly spires separate).
- **Hard threshold** (`1.05`, internal) — rock above this is effectively permanent so
  spires *stand* rather than dissolving to bedrock. This is what makes hoodoos a stable
  state instead of a transient.

## 5. Domain warp (distorts the hardness noise)

A 3D vector fBm that warps the noise domain → folded / tilted / swirled strata.

- **Domain warp** — overall amplitude (0 = flat beds).
- **Warp scale** — warp-noise frequency (broad folds ↔ tight turbulence).
- **Warp octaves** — fBm detail in the warp itself (smooth folds ↔ fractal contortion).

## 6. Resolution

- **Grid resolution** — 64³ … 512³ (default **192³**). Memory-safe rebuild: out-of-memory
  falls back to last good size instead of black-screening.
- **Render quality** — Low / **Medium** / High / Ultra = pixel scale (0.5×–1.5×) +
  raymarch step count (130–440). Pair high grids with High/Ultra so thin spires aren't
  skipped by the marcher.

Noise is resolution-independent, so raising the grid sharpens the *same* forms rather
than shrinking them.

## 7. Erosion model (dissolution)

Per exposed solid voxel: `rate = dt · erodibility · (1/hardness) · (rain·topEmpty +
wind·sideEmpty + …)`. Rain erodes top-down; wind undercuts laterally (stronger higher up
the column → pinched hoodoo waists). Bedrock floor held solid.

- **Erosion speed** — **float** ✅ (scales `dt`; fixed 2 sub-steps/frame). At speed 0
  erosion freezes but culling keeps running.
- Rain / Wind — separate strengths.
- **Open edges** ✅ — lateral domain walls treated as empty instead of solid, so the
  four side faces erode and sediment flows out of the volume (open boundary, like a
  fluid sim outlet). Bedrock floor stays closed. Toggle, default on. (Closed = the
  old locked-wall behaviour.)

## 8. Structural soundness (floating-rock removal)

Volumetric erosion carves loose chunks connected to nothing. Two approaches tried:

- ❌ **Attenuation-decay flood** (first attempt): persistent r32float support, `1` at
  bedrock, decays `×exp2(-1/2N)` per step. Correct but **slow** (1–2 s) — gentle decay was
  needed to protect tall connected spires, which made floater removal laggy.
- **Exact connectivity sweep** ✅ (current): three r32float fields — *reset* seeds bedrock,
  *flood* does pure-max propagation (no decay), result *published* to a third buffer the
  cull reads. Binary 0/1 grounded-ness, no false deletions. Floater removal latency =
  **one sweep (~0.2–0.25 s)**, independent of height. Tuning knob: `FLOOD_ITERS` (32).
- Toggle: **Remove floating rock** (on by default).

## 9. Sedimentation / deposition  ← talus slumping IMPLEMENTED ✅

Mass is currently only *removed*. Sedimentation catches eroded material and piles it.

- **Talus slumping** ✅ chosen — eroded rock becomes loose sediment that falls (gravity)
  and spreads to an angle of repose → scree cones at hoodoo bases, debris filling valleys.
  GPU sandpile, mass-conserving. Sediment is itself soft → re-erodes; some washes away to
  avoid infinite buildup.
- Hydraulic transport — sediment carried by simulated water flow, deposited where flow
  slows (fans, deltas, layered fill). Richer, heavier, harder to tune. *(not chosen)*
- Both — hydraulic feeding talus. Most realistic, most expensive. *(not chosen)*

**Material look:** **distinct sand material** ✅ — sediment renders as a separate matte
sand/scree colour, clearly different from the banded bedrock. (Alt: same palette.)

Implementation note: density field moved to **rgba16float** (`.r` = rock, `.g` =
sediment) so both materials are float (mass-conserving slump) and still filterable for
the raymarch. This also improves erosion precision over the old 8-bit field. Costs ~2×
density memory — 512³ may fall back to a lower grid.

**Gotcha learned:** the floater cull must KEEP sediment and delete only unsupported
rock. Loose sediment has `total < 0.5`, so the connectivity flood reads it as empty →
unsupported → the cull would annihilate every pile before it can build. Verified via
GPU readback: with the naive cull, total sediment collapsed to 0; keeping sediment,
per-voxel sediment reaches ~0.9 (visible talus) and persists.

Open tuning (future iterations): deposit vs. wash balance sets steady-state pile
thickness; lateral slump rate vs. repose bias sets cone steepness; consider exposing a
wash slider; sediment currently washes uniformly (could make it flow-direction aware).

## 10. Rendering / look

- Desert palette keyed to the *same* hardness function that drives erosion, so carved
  strata and coloured strata stay registered.
- Sun + soft shadow + cheap AO + distance fog + tonemap.
- Sediment (§9) shades as a distinct matte material.

Future look ideas: better sky/atmosphere, coloured strata presets (Bryce / Zion /
Painted Desert), wetness darkening near sediment, ambient dust.

---

## Backlog / future iterations

- Wind **direction** control → asymmetric undercutting, yardangs.
- Coastal-cliff and karst-cave variants (oscillating waterline / interior voids, cutaway).
- Hydraulic transport (§9) once talus is dialled in.
- v002 marching-cubes mesh; v003 MPM granular.
- Cross-section / cutaway view to show internal strata & caves.
- Save/load parameter presets.

## Literature note (has this been done?)

Yes in research — voxel hoodoos via spheroidal weathering (Beardall/Jones, 2007),
volumetric/multi-layer hydraulic erosion with arches (VMV 2024), implicit-feature terrain
(SIGGRAPH 2019). Not common as a **realtime dense-voxel SDF web** experiment — that
framing is the fresh part. See chat history for paper links.
