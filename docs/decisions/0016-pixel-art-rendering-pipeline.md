# ADR 0016 — Pixel-art rendering pipeline: nearest sampling, HDR bloom, painted glows

- Status: accepted
- Date: 2026-10-24

## Context

The client renders through Bevy's default 2D pipeline: a single LDR
`Camera2d`, linear texture filtering everywhere, no tonemapping, no
post-processing. The result reads as "programmer art": placeholder PNGs
authored at 128×128 are linearly downscaled to 36–92 px world sizes
(soft mush instead of crisp texels), every "glow" is a plain alpha
falloff, and the hit-flash is already authored over-bright
(`srgb(2.0, 0.45, 0.35)`) but silently clamps in the LDR target.

The owner wants the retro pixel-art identity kept, but richer: more
lights, better sprites. Constraints that shape the design:

- Bevy 0.19.1 is pinned (ADR-0005 discipline); no third-party render
  dependency is desired.
- Presentation lives in `evr-client` only (ADR-0004); the sim does not
  change.
- Content sprites are drop-in files under `assets/textures/`
  (art-guide contract); replacement art must stay easy.
- Real world sizes: ships 36/48/68/92 px (`ShipSize`), planets exactly
  180 px (`PLANET_RADIUS = 90`), stations `radius * 2.2` (79–97 px),
  suns `radius * 3.75` (~900 px, corona calibrated to the 1.8× heat
  zone), projectiles 9 px, meteorites `radius * 2.2`.

## Decision

1. **Nearest-neighbor sampling by default** via
   `ImagePlugin::default_nearest()`. Per-image linear samplers are the
   exception, allowed only where heavy downscale dominates readability:
   shop/commodity icons shown at ~20 px and chief portraits in the UI.
2. **Native texel standard — one texture texel equals one world pixel.**
   Ships are drawn at 36/48/68/92 px content footprints inside the
   128×128 canvas and render at `custom_size = 128`; planet PNGs move
   to 180×180 (rendering at 180 px, 1:1); procedural fallback textures
   are generated at their native world sizes (per-`ShipSize` ship
   fallbacks, meteorite variants at 62/38/22 px, a 16 px projectile
   bolt, 8 px debris, a 64 px nebula puff).
3. **HDR pipeline.** The camera spawns with `Hdr`,
   `Tonemapping::TonyMcMapface`, and `Bloom` in additive composite mode
   with a prefilter threshold of 1.0: only deliberately over-bright
   (>1.0) colors bloom; normal art and UI text stay clean.
4. **Painted-glow lighting.** "More lights" means over-bright sprite
   colors composited by bloom — the hand-painted 1990s approach: the
   sun sprite tints ×~1.6, projectile bolts ×~1.9, explosion debris
   over-bright, the existing ×2.0 hit flash now actually reads.
   Dynamic 2D point lights (`bevy_light_2d` 0.10 targets our Bevy
   0.19) were considered and deferred: shadowed lights fight the pixel
   aesthetic and add a pinned dependency; can be spiked later.
5. **Stepped, dithered glow textures.** Corona, accretion rings,
   projectile/debris puffs and nebulae are authored as discrete falloff
   bands with Bayer dithering, not smooth gradients, so nearest
   sampling renders them as intentional pixel art at any scale.
6. **Generator v2.** `ops/generate_placeholder_art.py` becomes a real
   pixel-art generator: limited faction palettes (terran, vellari,
   sylph, korath, auroran), 1 px outlines, dithered hull shading,
   engine nozzles, cockpit glints. The drop-in file contract is kept:
   same names, same canvas per kind, content footprints per the
   art-guide table.
7. **CRT/scanline post-processing is deferred** and will ship as an
   off-by-default toggle when built.

## Consequences

- The look becomes crisp nearest-sampled pixel art with selective
  glow: suns, weapon bolts, explosions, and hit flashes bloom; the UI
  never does.
- Sprite `custom_size` for ships/stations changes from world-size
  scaling to canvas-native; replacement art must respect the art-guide
  footprint table to stay 1:1.
- `Tonemapping::TonyMcMapface` requires the `bevy/tonemapping_luts`
  feature; because evr-client builds bevy with `default-features =
  false`, the feature list must also carry `bevy/zstd_rust` (the LUTs
  are zstd-compressed ktx2). Dropping either flag silently renders the
  magenta tonemapping fallback fullscreen.
- HDR + bloom adds GPU cost on a single 2D camera; `make bench` guards
  the frame budget.
- Effects follow-ups are queued, not decided here: engine-flame
  animation, trails, muzzle flash, jump flash, richer generated skies,
  CRT toggle (graphics plan phases 3–6; each records its own decision
  when built).