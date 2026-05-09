# Antidote — outstanding work & plan phases

A living checklist of what's left and the order we plan to tackle it. The original detailed plan lives at the host machine (`C:\Users\LarsBrubaker\.claude\plans\curious-sparking-cherny.md`) and the original PR-style milestones (M1–M6) are still valid; this doc captures the *current* status and what changed after M2 shipped.

---

## What's already done

- **M0/M1** — 4-crate workspace skeleton, Supabase Postgres schema applied, multi-game DB design (`games`, `user_scores`, `user_progress`, `user_settings`), CI + Pages deploy wired.
- **M2** — Native gameplay MVP. Full game loop, rapier2d physics with JS-faithful body params, pixel-faithful render of every JS Canvas helper (`paint_bubble`, `paint_virus`, `paint_dead_virus`, `paint_dying_virus`, `paint_pop_animation`, background+grid+border), pointer event handling, level init + virus spawn, dying lifecycle, pop lifecycle, life-loss + level-complete transitions. 12 unit tests.
- **Native shell** — winit 0.30 + software AGG rasterizer + softbuffer present. `cargo run -p antidote-native` opens a 800×600 window with the playable game. After the `[profile.dev.package."*"] opt-level = 2` fix, frame time is ~25 ms (release) / ~35 ms (debug).
- **Local-dev policy** — `[patch.crates-io] agg-gui = { path = "../agg-gui/agg-gui" }` is active by default; CI clones agg-gui sibling so the patch resolves there too. agg-gui is grown in place when antidote needs new features. See [CLAUDE.md](CLAUDE.md).
- **Pages deploy** — Vite `base: "./"` and relative `runtime-config.json` fetch; `https://larsbrubaker.github.io/antidote/` loads cleanly.

---

## Phase 1 — Hardware rendering for game sprites (biggest payoff)

**Goal:** game sprites (bubbles, viruses, pop rings, gradients, glows) render entirely on the GPU via WGSL shaders. Chrome widgets (text, menus, tables) keep using AGG's software back-buffer pipeline and the GPU just composites that bitmap as a quad. agg-gui gains hardware-aware `DrawCtx` impls in lockstep.

**Why now:** software AGG at 800×600 with multiple radial gradients per virus caps us around 25–40 fps and degrades over time. Hardware-rendered sprites unlock smooth 60 fps and free us from the rasterizer's hot loop, which is also where we suspect the slow frame-time drift lives.

**Sub-steps (ordered):**

1. **Done — Swap softbuffer → wgpu in `antidote-native`.** Cribbed the wgpu setup from `agg-gui/demo-native/src/main.rs` (Gpu struct, Surface, RENDER_ATTACHMENT format pick). The existing software `Framebuffer` is now uploaded as a fullscreen RGBA8 texture and drawn with a tiny WGSL shader (`textureSample` of a screen-sized texture). Same visual output as before, but the wgpu device/surface/queue is now in our hand for steps 2+. Eliminates the softbuffer present cost (~2 ms/frame).

2. **Add a hardware `DrawCtx` impl in agg-gui.** Lives in `agg-gui/agg-gui/src/wgpu_gfx_ctx.rs` (new module) or as a sibling crate `agg-gui-wgpu`. Implements the same `DrawCtx` trait so callers don't change. Initially supports only the primitives game sprites need:
   - **filled circle** with optional outline (matches `ctx.circle(x, y, r) + fill/stroke`).
   - **radial gradient fill** with N color stops + spread mode.
   - **halo-AA outline** — port the existing AGG halo trick to a WGSL fragment shader (signed distance + smoothstep over 1.0 px).
   - **alpha blending** with global alpha matching the software pipeline byte-for-byte.

   Each primitive is one draw call (instanced quads, vertex shader expands a unit quad into a circle bounding box; fragment shader does the AA + gradient sampling). State changes (fill color, gradient, line width) batch into a uniform buffer per-frame.

3. **Route game sprites through the hardware path.** The existing `render::scene::paint_*` helpers already use the `DrawCtx` API; they just call into the new wgpu-backed impl. No changes to `paint_bubble` / `paint_virus` / `paint_dying_virus` / `paint_pop_animation` source — they paint the same calls onto a different backend.

4. **Composite chrome widgets via a software → texture path.** `agg_gui::App::paint` already paints widget sub-trees into back-buffer textures (the `BackbufferState` / `BackbufferCache` machinery). Once the chrome widget tree paints into its cached AGG framebuffer, the wgpu `DrawCtx` uploads that bitmap once per dirty frame and draws it as a textured quad alongside the game sprites. Text + menus continue to look exactly like today; they just ride on top of the GPU now.

5. **Replicate to the WASM shell.** Phase 2 picks up there.

**Risks / unknowns:**

- Halo-AA fidelity in WGSL — needs to match the AGG software path exactly, especially for animated radii (virus wobble, pop ring).
- Gradient color matching — software AGG's gradient sampler vs WGSL `mix()` may differ at the ~1/255 level. Acceptable visually; if not, we precompute a 1-D LUT.
- BackbufferCache integration — agg-gui's existing back-buffer mechanism may need a new "compositing layer" hook so the GPU side knows which sub-trees to upload. May require small changes to `agg-gui/agg-gui/src/widget.rs` (compositing_layer / backbuffer_state_mut already exposed).

---

## Phase 2 — WASM shell + GitHub Pages live game

**Goal:** `https://larsbrubaker.github.io/antidote/` runs the same fully-playable game the native shell does, on WebGPU (with WebGL fallback). Currently the WASM crate is a stub that prints a startup message.

**Sub-steps:**

1. Wire `antidote-wasm` with `wasm_bindgen` exports — `start()`, `on_pointer_down/move/up`, `request_redraw()` callback hook. Mirror what `agg-gui/demo-wasm` does.
2. Pull the same wgpu pipeline introduced in Phase 1, but built for `wgpu` features = `["wgsl", "webgl"]` so it works without WebGPU.
3. `demo/src/main.ts` imports the wasm-pack output, attaches it to `#antidote-canvas`, drives a `requestAnimationFrame` loop that calls into wasm, forwards `pointerdown/move/up/cancel` + `touchstart/move/end/cancel` events through wasm-bindgen.
4. Local TS dev path: `bun run dev` works for hot reload of the TS shell while reusing the most-recent wasm-pack output.
5. Verify the deploy artifact size — wgpu + rapier2d + agg-gui together are not small. Aim for under ~5 MB gzipped; if larger, add `wasm-opt` step in CI.

**Risks:**

- WebGPU adoption is uneven; WebGL fallback path needs to ship from day one.
- rapier2d on wasm may pull a chunk of bundle weight. Consider feature-gating components we don't use.

---

## Phase 3 — Auth + score sync (Supabase)

**Goal:** signed-in users persist their high scores and resumable progress to Supabase. The leaderboard widget reads `user_scores` for the current `game_id`. The "other games" panel reads the `games` catalog.

**Blockers (in order):**

1. **DB Session Pooler URL** — the direct host `db.edupgibalgeqfujfkwmm.supabase.co` is IPv6-only and unreachable from Lars's network. Need the "Session pooler" connection string from Supabase Dashboard → Project Settings → Database → Connection string. Once we have it, `db/migrations/0001_init.sql` and `0002_seed_games.sql` get applied.
2. Supabase publishable key already saved to gitignored `.env` as `SUPABASE_ANON_KEY=sb_publishable_…`. Auth health endpoint already verified working.

**Sub-steps once unblocked:**

1. Apply the two SQL migrations.
2. Implement `antidote-core::db::auth::AuthClient` — `sign_up` / `sign_in` / `refresh` / `sign_out` over `/auth/v1/*`.
3. Implement `antidote-core::db::client::PostgrestClient::upsert/select/rpc` (the `list_games` round-trip already works).
4. Native shell `Storage` impl: write tokens to `dirs::config_dir()/antidote/session.json`. WASM `Storage` impl: write to `localStorage["antidote_session"]`.
5. Sign-in / sign-up dialog widget (`antidote-core::ui::auth_widget`) — two `TextArea` inputs + sign-in / sign-up / continue-as-guest buttons.
6. After each level: upsert into `user_scores` (high_score, total_score, plays, last_played) and `user_progress` (current_level, lives_remaining, state JSONB).
7. Leaderboard widget — reads top N rows from `user_scores` filtered by current `game_id`.
8. "Other games" panel on the main menu — `SELECT * FROM games ORDER BY sort_order` rendered as cards with `deploy_url` links.

**Out-of-scope until later:** Google OAuth (deep-link-callback complexity on native), anonymous accounts.

---

## Phase 4 — Menus, level progression, lives polish

**Goal:** the gameplay loop has real beginning / middle / end states, not just an infinite single-level grind.

**Sub-steps:**

1. **Main menu** — sign-in panel + "Play" button + "Other games" panel + leaderboard for the current user.
2. **Pause overlay** — Esc / P key toggles Phase::Playing ↔ Phase::Paused. Resume button + back-to-menu button.
3. **Game-over screen** — final score, "Play again" + back-to-menu, plus an upsert if the user is signed in.
4. **Level-complete screen** — short pause showing score gained + "Next level" button.
5. **Life-lost animation** — JS reference floats the new lives counter from canvas center to the HUD lives icon. Needs `antidote-core::ui::menu_widget` + agg-gui transient overlay support.
6. **HUD as a real widget tree** — currently the HUD is drawn inside the GameWidget paint. Split into a top bar widget for lives + level + antidote bar + score that lays out around the game widget.

---

## Phase 5 — Investigate and fix the slow frame-time drift

Ongoing observation: even with profile-fix in place, frame time creeps from ~25 ms to ~30 ms over 12 s of play. Suspected culprits in priority order:

1. agg-rust internal cache (path / gradient / span generators) growing.
2. Rapier `QueryPipeline` rebuilding spatial structures every step despite low body count.
3. State stack inside `GfxCtx` accumulating between paints (should be balanced — verify).
4. softbuffer surface re-config — re-checks size each frame.

After Phase 1 ships, much of this becomes irrelevant — game sprites won't go through the software rasterizer at all. Defer detailed investigation until after Phase 1 lands.

---

## Phase 6 — Polish

- Hero screenshot for `rust-apps/README.md` antidote section.
- SEO + Open Graph meta tags in `demo/index.html`.
- Settings persistence: master volume, difficulty, theme — write to `user_settings` (per-user, per-game; sentinel `00000000-0000-0000-0000-000000000000` for cross-game globals).
- Audio? The JS reference has no sound effects. Adding subtle bubble-pop / virus-trap / level-complete sounds is a stretch goal; needs a tiny audio crate (`rodio` on native, `web_sys::HtmlAudioElement` on wasm).
- README quick-start + GIF / live-demo link.
- Anonymous handle column (`anon_handle text`) on a public profile mirror so the leaderboard exposes nicknames instead of UUIDs. Make sure RLS policies match before going public.

---

## Standalone follow-ups (not on the main critical path)

- **Public leaderboard exposure** — `public_leaderboard_read` policy currently allows anyone to SELECT `user_scores`, which leaks raw `user_id` UUIDs. Before sharing the live URL widely, switch to a SQL view that joins a public-handles table.
- **CI Node 20 deprecation warning** — GitHub Actions warns `actions/checkout@v4` will require Node.js 24 by 2026-09. Bump action versions or set `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24=true` before the deadline.
- **Cargo edition 2024** — let-chains (`if let X = … && let Y = … { … }`) are nicer than the nested `if let` blocks we're using now. Bump `edition = "2024"` once the rest of the toolchain catches up.

---

## Conventions

- New agg-gui features land in `../agg-gui/agg-gui/src/…` directly, not as workarounds in antidote (see CLAUDE.md "Local development uses agg-gui as a path dep").
- `antidote-core` stays target-agnostic — no `tokio`, no `dotenvy`, no `winit`, no `wgpu` direct deps. Both shells inject services through the `platform::Storage` trait family.
- The JS reference at `reference/GFG/` is read-only documentation. Constants in `antidote-core/src/consts.rs` are the canonical Rust copies.
- Commit straight to the antidote repo's `main` — no feature branches, no worktrees. (Same convention as rust-apps superproject.)
