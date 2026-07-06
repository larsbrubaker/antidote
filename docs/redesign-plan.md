# Petri Pop redesign — implementation plan

Source of truth for the visuals: [`docs/New Design/Antidote Redesign.dc.html`](New%20Design/Antidote%20Redesign.dc.html)
(10 screen mockups + design tokens + rationale + animation notes) and
[`docs/New Design/Antidote Frame.dc.html`](New%20Design/Antidote%20Frame.dc.html) (the reusable in-game frame).
View them by serving `docs/New Design/` over HTTP (they fetch `support.js`).

Approved 2026-07-06. Scope decisions:

- **Full palette adoption** — lime/violet/coral everywhere, including gameplay objects.
  The CLAUDE.md "pixel-faithful to the JS reference" invariant is retired for colors
  and layout; physics body parameters still match the JS reference exactly.
- **Virus keeps its character** — existing eyes, orbiting spikes, and wobble stay;
  only the gradient is recolored to coral. The mockup's eyeless virus is rejected.
- **All four new features in the first pass** — cure-timer arc, first-run hints,
  new-best celebration, mute placeholder button (no audio exists yet).
- **Full animation sheet** — everything in the design's animation notes.

## Geometry

| Region | Units | Notes |
|---|---|---|
| App canvas | 1280×720 | Fixed 16:9 virtual canvas, scaled uniformly into the window (letterbox). Replaces 800×600. |
| Left rail | x 0–120 | Pause button, LEVEL, LIVES pips, vertical ANTIDOTE meter |
| Playfield panel | x 120–1160 (1040×720) | Dish background + 40-unit grid |
| Right rail | x 1160–1280 | Fullscreen, mute, SCORE, BEST |
| Arena (live physics area) | 1016×696 | Playfield panel inset by 12; stroke 2px violet-400 @0.35, radius 20 (corner rounding is cosmetic — walls are the rect) |

`VIRTUAL_WIDTH/HEIGHT` becomes 1016×696 and keeps its meaning (the physics/game
coordinate space); rendering offsets it by (12,12) inside the playfield panel.
The old adaptive HUD layouts (TopStrip/LeftStrip/SideColumns) are deleted — rails
are part of the fixed canvas. RotateOverlay remains the only portrait variant.

## Design tokens

Land as `antidote_core::theme`. Key colors (full set in the mockup's token sheet):
ink-900 `(15,11,26)` canvas · ink-800 `(20,15,36)` dish · ink-700 `(28,21,48)` rails/panels ·
ink-600 `(38,29,64)` raised · edge-950 `(8,5,16)` hard shadows · lime-500 `(178,255,66)` ·
lime-700 `(98,178,22)` · coral-500 `(255,92,72)` · coral-800 `(150,28,40)` · amber-500 `(255,196,54)` ·
gold-400 `(255,206,84)` · violet-400 `(158,120,255)` · text-hi `(240,236,255)` ·
text-mid `(178,168,210)` · text-low `(122,112,152)` · hairline `rgba(233,226,255,0.10)` ·
scrim `rgba(10,7,20,0.78)` (0.45 first-run / 0.82 game over).

Font: Exo 2 (SIL OFL) replaces Cascadia Code. Static instances if the text stack
can't drive a variable font's weight axis (expect: Medium 500, ExtraBold 800,
ExtraBold Italic 800).

Buttons: rounded rect (radius 16) over a solid edge rect offset 4–6u down
(no blur — it's two rounded rects). States per the token sheet
(hover raise −1u, pressed +4u with 2u edge, focus double ring, disabled @0.18/0.35).

## Task breakdown (tracked in the session task list)

1. **Foundations** — layout consts, theme module, Exo 2, fixed-canvas scale root.
2. **agg-gui primitives** — whatever the capability survey says is missing
   (expected: arc stroke for the cure timer; possibly rounded-rect/pill and
   gradient work on the hardware path). Done in `../agg-gui` per the sibling
   workflow.
3. **Playfield resize** — 1016×696 physics space, tests updated.
4. **HUD rails** — one fixed layout, per the Frame mockup.
5. **Playfield reskin** — dish, grid, arena stroke, lime bubbles, coral virus
   (eyes kept), lime/white pop particles.
6. **Menus & overlays** — all nine screens.
7. **New features** — cure arc + chip, first-run hints (persisted flag),
   new-best gold takeover + confetti, mute placeholder (persisted flag).
8. **Animations** — full sheet, transform/alpha only, time from the frame clock
   (never wall-clock into physics).
9. **Verify + docs** — tests/clippy/wasm, side-by-side against mockups,
   CLAUDE.md invariant updates.

## Known deviations from the mockups

- Virus rendering keeps eyes/spike-orbit/wobble (decision above).
- Help screen repo URL: `github.com/larsbrubaker/antidote`.
- Arena corner radius is visual only; physics corners stay square.
