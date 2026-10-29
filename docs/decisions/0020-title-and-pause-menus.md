# ADR 0020 — Client state machine: title menu and pause menu

- Status: accepted
- Date: 2026-10-29

## Context

The client booted straight into gameplay: camera, starfield, HUD, and panels
all spawned at `Startup`, and the embedded server started ticking in `First`
immediately. There was no title screen, no way to quit gracefully, no pause,
and every piece of text rendered in Bevy's default font — the weakest link in
an otherwise deliberate pixel-art identity (ADR-0016).

The owner approved (2026-10-29) a retro opening screen and menu in the same
style as the rest of the game: pixels, lights, chunky arcade typography.

## Decision

1. **`AppState { Menu, InGame }`** — the first `States` machine in `sv-client`,
   via the already-enabled `bevy_state` feature. The app starts in `Menu`.
2. **Menu phases, not states.** A `MenuPhase` resource distinguishes the
   title screen (`Title`: LAUNCH PILOT / QUIT) from the in-game pause menu
   (`Pause`: RESUME / QUIT). One set of systems serves both.
3. **Pause semantics follow ADR-0004.** Offline, `step_embedded_server` runs
   only in `InGame`, so the galaxy truly pauses behind the menu — this is
   single-player pause. Online, the remote shard keeps ticking (the ship
   idles, exactly as it does today when the pilot steps away); `send_login`
   keeps running behind the title so entering the game is instant.
4. **ESC opens the pause menu** only while no chief panel is visible; the
   opener system runs before `sync_chief_panel`, so one ESC press dismisses
   the chief panel and never also opens the menu. ESC on the pause menu
   resumes.
5. **Gameplay UI spawns on `OnEnter(InGame)`** with idempotence guards, so
   HUD/minimap/chief/galaxy-map panels stay up (dimmed behind the menu
   backdrop) across pause/resume and never double-spawn. Input polling,
   panel updates, and camera-follow all gate on `in_state(InGame)`.
6. **Press Start 2P (SIL OFL) is the UI font**, bundled at
   `assets/fonts/press-start-2p.ttf` with its license, and adopted by every
   text site: menu, HUD, chief, shops, minimap, galaxy map. Sizes stay on the
   font's 8px grid (8/16/24/40). Menu spawn waits for the font asset to
   finish loading (a few frames) instead of risking unrendered text.
   The bevy `default_font` feature stays as a fallback.
7. **Retro lights per ADR-0016, no new art.** Two over-bright, dithered
   `glow` sprites pulse behind the title and menu items and bloom through
   the existing HDR pipeline; all text stays in the 0–1 range so it never
   blooms. The backdrop is the existing starfield/nebula sky with a slow
   camera drift. UI strings are ASCII-only — the font has no em dash,
   en dash, or multiplication sign; this applies to client strings and to
   server-authored encounter text and content-pack quest strings rendered
   in the client, which were converted from `—`/`–`/`×` to `-`/`x`.
8. **`SHIP LOST` no longer flickers on first entry**: the banner only shows
   once the HUD has seen an owned ship, closing the replication gap when
   entering the game.
9. **The capture tour starts on the title screen**: a new `Title` stage
   captures `00-title.png`, then transitions state directly (no synthetic
   key presses), and the sv-mcp screenshot publisher validates seven images.

## Consequences

- The sim (sv-core/sv-server) is untouched; this is presentation-only.
- A new client gameplay flow (menus, panels) should register against
  `AppState` rather than `Startup`, following the guards added here.
- `--capture-tour` now exercises the menu → game transition, so the README
  regenerates a title screenshot as image 00.
- The ADR-0016 collision on file numbering (two 0016-* files) is
  acknowledged but not resolved here; this ADR takes 0020.
- Future work: the graphics phases 3–6 lights pass will enrich the title
  backdrop (engine flames, richer skies); a CRT toggle (ADR-0016 item 7)
  would complete the retro frame.