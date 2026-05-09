# Claude guidance for the Antidote repo

## Architecture invariants

- 4-crate workspace: `antidote-core` (logic+widgets), `antidote-native` (winit/wgpu), `antidote-wasm` (cdylib), `demo/` (TS shell). Mirrors `agg-gui`'s demo-* pattern.
- `antidote-core` MUST stay `wasm32`-clean. No `tokio`, no `dotenvy`, no `dirs`, no `winit`, no `wgpu`. Both shells inject services through traits in `antidote_core::platform`.
- Y-up: agg-gui is Y-up first-quadrant; the JS reference is Y-down. The flip happens once at the GameWidget boundary in `antidote_core::render::scene::flip_y` — every helper inside `render::scene` works in JS-style Y-down coordinates.
- **Physics: rapier2d.** Don't replace it with a hand-rolled integrator. Body parameters (density, friction, restitution, damping, CCD) match `reference/GFG/public/games/antidote/antidote-physics.js` exactly. PIXELS_PER_METER = 30.
- **Rendering: pixel-faithful reproduction of the JS Canvas.** Every gradient stop, alpha, line width, eye offset, spike orbit, wobble, ease curve, and pop-animation timing must match `reference/GFG/public/games/antidote/antidote-rendering.js`. When in doubt, run the JS reference side-by-side and compare frames.
- DB access goes through Supabase REST (PostgREST + `/auth/v1/*`) over `reqwest`. No direct Postgres connection — wouldn't work in WASM.
- Anon key ships in the build artifact; RLS is what guards data. Never touch RLS without re-checking the policies in `db/migrations/0001_init.sql`.

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
