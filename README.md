# Antidote

[![Antidote](readme_hero.png)](https://larsbrubaker.github.io/antidote/)

[Live Demo](https://larsbrubaker.github.io/antidote/) · [Repository](https://github.com/larsbrubaker/antidote)

Bubble-trap virus puzzle game in Rust — rendered with [agg-gui](https://github.com/larsbrubaker/agg-gui), physics by [rapier2d](https://rapier.rs/), persisted to Supabase. Runs natively (winit + wgpu) and in the browser (WebAssembly).

Held pointer grows an antidote bubble; bouncing viruses get trapped if they can't move for 3 seconds; lives, levels, and a cross-device leaderboard. Sign in with Google to keep scores across web, desktop, and (future) mobile.

> Ported from the original TypeScript / Canvas 2D / Planck.js implementation, preserved under `gfg/` for read-only reference.

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
gfg/               # original TS/Canvas reference (read-only; not part of builds)
```

`antidote-core` is `wasm32`-clean — no `tokio`, no `dotenvy`. Both shells inject `Storage` and `HttpClient` impls.

## Database

Schema is multi-game by design: `games`, `user_scores (user_id, game_id) PK`, `user_progress`, `user_settings`. See [`db/README.md`](db/README.md).

Auth: Supabase email/password via REST. Tokens cached in a JSON file on native, `localStorage` in the browser. RLS enforces `auth.uid() = user_id` on user-scoped tables.

## Status

| Milestone | Description | State |
|-----------|-------------|-------|
| M1 | Skeleton + Supabase round-trip | ✓ |
| M2 | Native gameplay MVP | ✓ |
| M3 | Auth + score sync (email/password + Google) | ✓ |
| M4 | Menus, multiple levels, lives | ✓ |
| M5 | WASM build + GitHub Pages deploy | ✓ |
| M6 | Polish (Facebook/Apple OAuth, persisted sessions, achievements engine, mobile shells) | in progress |

See [`antidote_todo.md`](antidote_todo.md) for the live punch list.

## License

MIT — see [LICENSE](LICENSE).
