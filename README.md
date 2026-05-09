# Antidote

Bubble-trap virus puzzle game in Rust — rendered with [agg-gui](https://github.com/larsbrubaker/agg-gui), persisted to Supabase, runs natively (winit + wgpu) and in the browser (WebAssembly).

Held pointer grows an antidote bubble; bouncing viruses get trapped if they can't move for 3 seconds; lives, levels, and a multi-game leaderboard.

> Ported from the original TypeScript / Canvas 2D / Planck.js implementation, preserved under `reference/GFG/` for reference.

## Quick start

```bash
# Native
cp .env.example .env          # fill in SUPABASE_ANON_KEY
cargo run -p antidote-native
cargo install cargo-watch     # one-time install for watch mode
cargo dev                     # rebuilds and reruns antidote-native on changes

# WebAssembly
wasm-pack build antidote-wasm --target web --out-dir ../demo/public/pkg --no-typescript
cd demo && bun install && bun run dev
```

## Workspace layout

```
antidote-core/     # game logic, widgets, Supabase REST client (target-agnostic)
antidote-native/   # winit + wgpu shell with tokio runtime
antidote-wasm/     # cdylib wasm-bindgen shell
demo/              # TypeScript bundling shell for the WASM build
db/migrations/     # Supabase Postgres schema (multi-game)
reference/GFG/     # original TS/Canvas reference (read-only)
```

`antidote-core` is `wasm32`-clean — no `tokio`, no `dotenvy`. Both shells inject `Storage` and `HttpClient` impls.

## Database

Schema is multi-game by design: `games`, `user_scores (user_id, game_id) PK`, `user_progress`, `user_settings`. See [`db/README.md`](db/README.md).

Auth: Supabase email/password via REST. Tokens cached in a JSON file on native, `localStorage` in the browser. RLS enforces `auth.uid() = user_id` on user-scoped tables.

## Status

| Milestone | Description | State |
|-----------|-------------|-------|
| M1 | Skeleton + Supabase round-trip | scaffolding committed |
| M2 | Native gameplay MVP | pending |
| M3 | Auth + score sync | pending |
| M4 | Menus, multiple levels, lives | pending |
| M5 | WASM build + GitHub Pages deploy | pending |
| M6 | Polish + hero image | pending |

## License

MIT — see [LICENSE](LICENSE).
