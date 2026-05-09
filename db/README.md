# Antidote DB

Migrations for the shared Supabase Postgres backing Antidote and any future games.

## Apply

```bash
# With supabase CLI (recommended)
supabase db push

# Or manually with psql
psql "postgresql://postgres:$SUPABASE_DB_PASSWORD@db.edupgibalgeqfujfkwmm.supabase.co:5432/postgres" \
    -f db/migrations/0001_init.sql \
    -f db/migrations/0002_seed_games.sql
```

`SUPABASE_DB_PASSWORD` lives in `.env` (gitignored). The running app **never** uses this password — only the anon key + per-user JWT, gated by RLS.

## Schema overview

- `public.games` — catalog of games. Powers "other games" panel.
- `public.user_scores (user_id, game_id) PK` — high-water-mark leaderboard.
- `public.user_progress (user_id, game_id) PK` — resumable game state.
- `public.user_settings (user_id, game_id) PK` — settings; `00000000-0000-0000-0000-000000000000` for cross-game globals.

All tables enforce RLS. `games` is publicly readable; `user_scores` is publicly readable (leaderboard); `user_progress` and `user_settings` are scoped to `auth.uid()`.
