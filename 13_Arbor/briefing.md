# № 13 — Arbor

**Procedural tree generator with a root system that grows around imported objects,
exporting textured USD (`.usda` / `.usdz`) with real UV coordinates.**

Leaves are a planned second phase. This milestone delivers the branch/root skeleton,
the obstacle-aware root growth, the mesher, and the USD + texture export.

---

## Research basis

| Year | Paper | What we borrow |
|------|-------|----------------|
| 2023 | **Rhizomorph: The Coordinated Function of Shoots and Roots** — Li, Klein, Michels, Beneš, Pirk, Pałubicki (SIGGRAPH, ACM TOG 42:4). https://doi.org/10.1145/3592145 | Root growth as response to environment: gravitropism, obstacle interaction, root–shoot signaling. Our root system is a developmental model steered by tropism + attraction fields + an obstacle SDF, Rhizomorph-style. |
| 2024 | **Interactive Invigoration: Volumetric Modeling of Trees with Strands** — Li, Schwarz, Pałubicki, Pirk, Beneš (ACM TOG 43:4). https://doi.org/10.1145/3658206 | Strand-based branch volumes. Our mesher sweeps each skeleton edge as a generalized cylinder whose radii come from a pipe-model accumulation. |
| 2025 | **Autoregressive Generation of Static and Growing Trees** — Wang, Zhang, Klein, Michels, Yan, Wonka. arXiv:2502.04762 | 4D growing trees; confirms skeleton-then-grow as a sensible representation. Context for later growth-animation phase. |
| 2025 | **Controllable Modular Trees** — Takikawa, Tinchelli, Davison, Andrus, Drown, Sheffer (SIGGRAPH). https://doi.org/10.1145/3769872.3769904 | Modular branch instancing. Parked for a later "instanced twigs" optimization. |
| 2007 | **Modeling and visualization of leaf venation patterns** — Runions et al., SIGGRAPH | The attraction-field / space-colonization growth used for crowns and root fronts. |

Classic foundation: L-systems (Lindenmayer 1968, Prusinkiewicz 1990),
pipe model theory (Shinozaki 1964), and the strand/cylinder sweep geometry.

---

## Architecture (single file — Arbor_v001.html)

```
13_Arbor/
  Arbor_v001.html     self-contained experiment (all logic + UI + render + USD export)
  assets/
    rock.obj           sample obstacle
  briefing.md          this file
```

The HTML contains two layers that can run independently:

### Core (pure JS, DOM-free, testable in Node)

The script block ends with `if (typeof module !== 'undefined' && module.exports) module.exports = Arbor;`.
UI boot is guarded by `if (typeof document !== 'undefined')`. To test the core from Node,
extract the `<script>` content and pass it through `new Function('module','exports',...)`.

Core exports under the `Arbor` namespace:

| Export | Purpose |
|--------|---------|
| `mulberry32(seed)` | Deterministic PRNG |
| `v3` | Vector math (add, sub, scale, norm, len, cross, dot, lerp, randUnit) |
| `makeNoise2(seed)` | 2D value noise with fbm |
| `TriangleSoup(verts, faces, name)` | Spatial-hash triangle distance field, gradient, inside ray test |
| `CompositeObstacle(soups)` | Aggregates multiple soups; `.distance(p)`, `.gradient(p)`, `.inside(p)` |
| `parseOBJ(text, name)` | Minimal OBJ loader returning a `TriangleSoup` |
| `makeBoulder(seed)` / `makeColumn()` / `makeSlab()` | Built-in obstacles |
| `SPECIES` | Presets: oak, spruce, banyan with full parameter tables |
| `sampleCrown(cfg, rng, obstacles)` | Crown attractor field (space-colonization targets) |
| `sampleSoil(cfg, rng, obstacles)` | Sub-surface attractor field for roots |
| `newGrowState(cfg, obstacles)` | Initialize growth (nodes, tips, fields, contacts counter) |
| `growOneStep(st)` | One iteration: steer tips, obstacle response, branching, kill consumed attractors |
| `growTree(cfg, obstacles, maxSteps)` | Loop `growOneStep` until done or maxSteps |
| `finalizeTree(st)` | Pipe-model radii, classify trunk path (prefers upward children) |
| `buildMesh(st)` | Strand-sweep mesher: returns `{trunk, branch, root}` with `pos`, `nrm`, `uv`, `idx` |
| `buildUSDA(parts, obstacles, cfg, species)` | ASCII USDA with UsdPreviewSurface + UsdUVTexture materials |
| `buildUSDZ(entries)` | Stored-method zip with 64-byte alignment (extra-field subfield 0xFFFF) |
| `buildTextureSet(seed)` | `[{name, data}]` — bark, root, twig PNGs |
| `generateTexturePixels(w, h, kind, rng)` | Procedural texture (value-noise bark/root/twig) |
| `pngEncode(pixels, w, h)` | Pure-JS PNG writer (zlib-stored, CRC32) |
| `crc32(data)` | CRC-32 for zip local headers |

### Growth model

One plant graph rooted at the trunk base. First-order shoots form the trunk/crown,
first-order roots form the root system. Growth is iterative:

1. **Attraction fields** — crown attractors sampled in a species-shaped volume
   (ellipsoid for oak, cone for spruce); soil attractors in a hemispherical shell
   below ground, augmented with **hug-field points** placed just outside obstacle
   surfaces so roots actively grow toward and wrap around objects.
2. **Steering** — each tip's direction blends inertia, the space-colonization pull
   toward nearby attractors, a tropism term (phototropism +Y / gravitropism −Y),
   order-scaled for subordinate branches, and noise.
3. **Obstacle avoidance** — the tip samples the obstacle SDF; inside a margin it
   separates into surface-normal push and tangent slide, so roots **hug and wrap**
   around the object. Contact triggers thigmotropism: extra lateral roots at the
   surface.
4. **Vigor economy** — each tip carries `{vigor, stepCost, minVigor}`. Vigor
   decreases per step; branching requires `sinceBranch >= branchDelay` and
   `vigor > 0.3`. Lateral tips inherit `vigor * branchVigor`. Tips below
   `minVigor` or idle too long are culled. Hard cap at `maxNodes`.
5. **Radii** — post-growth pipe-model (`r_parent² = Σ r_child²`). Trunk path
   follows the child with maximum `score = r * (0.3 + 0.7 * upness)`.

### Mesher

Each skeleton edge becomes a strand: a quad tube between a start ring (branch-point
radius) and end ring (node radius), 8 segments around. Per-ring UVs: `u = k/8`
around, `v = arclength/tile` along. Radial ring normals give smooth cylinders.

### USD export

- **USDA** (readable ASCII): Y-up, `defaultPrim = /World`. `/World/Tree/{Trunk,Branches,Roots}` —
  `UsdGeom.Mesh` with per-vertex normals, `primvars:st`, `extent`. Materials:
  `UsdPreviewSurface` + `UsdUVTexture` (sRGB) + `UsdPrimvarReader_float2`.
  Obstacles embedded under `/World/Obstacles/`.
- **USDZ** (packaged archive): Stored-method zip, first entry 64-byte aligned,
  embedded `textures/{bark,root,twig}.png` resolvable via USD's package resolver.
  All built in pure JS — no external tools required.

### UI / WebGL2

Left panel matching the repo aesthetic (Erosion-style): species select, seed slider,
crown/root geometry controls, obstacle picker (boulder/column/slab/rockobj/custom/none),
OBJ file upload or drag-drop. Grow (animated), Instant, Regrow, Clear buttons.
Export .usda / .usdz / textures. WebGL2 renderer with orbit camera, grid, field points,
autorotate.

---

## Roadmap

- **Leaves:** leaf distribution on twig tips, `UsdGeom.PointInstancer` or
  instanced leaf meshes, leaf textures with alpha.
- Basal flare, bark normal maps, per-species secondary thickening over time.
- Crown attractors as volume-density fields (lighter crowns).
- Strand packing at branch joints instead of per-edge tubes.
- Growth animation (grow tree step-by-step with time playback).

---

## Open

Open `Arbor_v001.html` in any modern browser. WebGL2 required for the 3D viewport;
Node.js and `usd-core` optional for headless / CLI validation.

```powershell
# Headless test (Node)
node -e "const Arbor=require('./Arbor_v001.html'...)"   # extract script block
```
