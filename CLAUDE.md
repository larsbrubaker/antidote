# Claude guidance for the Antidote repo

## Architecture invariants

- 4-crate workspace: `antidote-core` (logic+widgets), `antidote-native` (winit + wgpu), `antidote-wasm` (cdylib), `demo/` (TS shell). Mirrors `agg-gui`'s demo-* pattern.
- `antidote-core` MUST stay `wasm32`-clean. No `tokio`, no `dotenvy`, no `dirs`, no `winit`, no `wgpu`. Both shells inject services through traits in `antidote_core::platform`.
- `antidote-native` and `antidote-wasm` are **platform shells only**. They wire up the OS/browser window or canvas, wgpu surface, event loop, input forwarding, and platform persistence. They contain **no game or UI content**: every game rule, widget tree, menu, layout, HUD, dialog, leaderboard, and interface the user sees is shared via `antidote-core`. Platform crates call shared builders such as `antidote_core::ui::build_antidote_app()` and forward events; they never construct screens or widgets directly.
- Platform split, copied from agg-gui's goal:
  - **Game / widget / layout code** → `antidote-core`
  - **GPU renderers (WGSL shaders, geometry, draw calls)** → `demo-wgpu` / future `agg-gui` wgpu backend
  - **Platform shell (OS window or browser canvas + event forwarding + persistence backend)** → `antidote-native` and `antidote-wasm`
- Y-up: agg-gui is Y-up first-quadrant; the JS reference is Y-down. The flip happens once at the GameWidget boundary in `antidote_core::render::scene::flip_y` — every helper inside `render::scene` works in JS-style Y-down coordinates.
- **Physics: rapier2d.** Don't replace it with a hand-rolled integrator. Body parameters (density, friction, restitution, damping, CCD) match `reference/GFG/public/games/antidote/antidote-physics.js` exactly. PIXELS_PER_METER = 30.
- **Rendering: pixel-faithful reproduction of the JS Canvas.** Every gradient stop, alpha, line width, eye offset, spike orbit, wobble, ease curve, and pop-animation timing must match `reference/GFG/public/games/antidote/antidote-rendering.js`. When in doubt, run the JS reference side-by-side and compare frames.
- DB access goes through Supabase REST (PostgREST + `/auth/v1/*`) over `reqwest`. No direct Postgres connection — wouldn't work in WASM.
- Anon key ships in the build artifact; RLS is what guards data. Never touch RLS without re-checking the policies in `db/migrations/0001_init.sql`.

## Local development uses agg-gui as a path dep — improve it as you go

When developing on a workstation that has the rust-apps superproject checked out (with the agg-gui submodule beside antidote at `../agg-gui/`), the `[patch.crates-io]` section in the workspace `Cargo.toml` redirects `agg-gui` to the local checkout. **This is the default state** — every commit assumes contributors are running with the path override active.

That means: when antidote needs an agg-gui feature that doesn't exist yet, the right move is to **add it to agg-gui itself** (in `C:\Development\rust-apps\agg-gui\agg-gui\src\…`), not to work around it inside antidote. agg-gui is being grown specifically to support games well; antidote is one of the first real callers driving that growth.

Workflow:
1. Make the change in `../agg-gui/agg-gui/src/…`.
2. Run antidote against the patched local crate (`cargo check --workspace` — Cargo picks up `../agg-gui/agg-gui` via the patch).
3. When the agg-gui changes are stable, publish a new agg-gui version to crates.io (Lars handles this manually).
4. CI continues building against the published crates.io version because the CI workflow clones `larsbrubaker/agg-gui` as a sibling so the `path = "../agg-gui/agg-gui"` patch resolves there too.

If you're checking out antidote standalone (no rust-apps superproject), clone agg-gui sibling: `git clone https://github.com/larsbrubaker/agg-gui.git ../agg-gui` from this repo's root.

## Rendering: hardware for game sprites, software for text/menus

The agg-gui rendering backend is split by widget purpose:

- **Game sprites — bubbles, viruses, pop animations, gradients, glows** → render in **hardware** via wgpu shaders. The existing halo-AA (anti-grain anti-alias) approach in agg-gui is being extended with WGSL shader equivalents so each frame's circle/gradient/halo is drawn directly to the surface. **Do not** route game-sprite drawing through the software AGG rasterizer — at 800×600 with multiple gradients per virus and 60 fps, the software path is too slow.
- **Text, menus, table widgets, anything chrome-ish** → keep using the existing software path (`GfxCtx` → `Framebuffer`). These widgets cache their software backbuffers and the GPU just composites the bitmap once. Text shaping is a software job (rustybuzz + TTF) and that's fine.

Adding new hardware paths is expected. When a new game-sprite primitive (e.g. an animated radial-gradient ring) doesn't exist yet on the hardware path, **add it to agg-gui** (a new method on a hardware-aware `DrawCtx` impl, plus the matching WGSL shader), don't reach for the software fallback. Keep the public `DrawCtx` API uniform across the two backends so widget code doesn't have to know which backend it's running on.

The native shell renders entirely on wgpu. The wasm shell does the same via WebGL/WebGPU. Software-path widgets emit their cached backbuffer as a textured quad on the GPU side.

## Schema is multi-game

Every user-facing table is keyed `(user_id, game_id)`. The `games` table is the single source of truth for `game_id`. New games add a row to `games`; nothing else in the schema needs to change.

## Reference

The original TypeScript / Canvas 2D / Planck.js game lives in `reference/GFG/`. Treat it as read-only documentation — never modify, never include in builds.

Game-design constants are in `antidote-core/src/consts.rs`, ported from `reference/GFG/public/games/antidote/antidote.js`.

## Build & test

```bash
cargo check --workspace                    # native targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings

# WASM
wasm-pack build antidote-wasm --target web --out-dir ../demo/public/pkg --no-typescript
```

`default-members` excludes `antidote-wasm` so plain `cargo build` doesn't try to drag wasm-only deps into a native build.

## Plan

Top-level plan and milestones live at `C:\Users\LarsBrubaker\.claude\plans\curious-sparking-cherny.md` (host machine). Don't duplicate it here — it's the source of truth.
