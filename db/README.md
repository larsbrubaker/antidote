# Antidote DB

Migrations + Supabase setup notes for the shared Postgres backing Antidote and any future games.

## Apply migrations

Three options, easiest first:

1. **Via Management API** (what we use in this repo — needs `SUPABASE_ACCESS_TOKEN` in `.env`):
   ```bash
   for f in db/migrations/*.sql; do
       jq -Rs '{query: .}' "$f" \
       | curl -X POST \
           "https://api.supabase.com/v1/projects/edupgibalgeqfujfkwmm/database/query" \
           -H "Authorization: Bearer $SUPABASE_ACCESS_TOKEN" \
           -H "Content-Type: application/json" \
           --data-binary @-
   done
   ```

2. **Via Supabase Dashboard SQL Editor** — paste each `.sql` file into <https://supabase.com/dashboard/project/edupgibalgeqfujfkwmm/sql/new> and run.

3. **Via `supabase` CLI** (when on an IPv4-friendly network — direct DB host is IPv6-only, use Session Pooler in CLI config):
   ```bash
   supabase db push
   ```

## Schema overview

| Table | Purpose | Read access | Write access |
|---|---|---|---|
| `public.games` | Catalog of games (slug, display name, deploy URL) | public | none (catalog) |
| `public.user_handles` | Per-user display name (auto `player-XXXXXXXX` on signup) | public | owner only |
| `public.user_scores` | `(user_id, game_id) PK` high score / total / plays / last_played | **none direct** — go through `leaderboard` view | owner only (via `add_game_score` RPC) |
| `public.user_progress` | Resumable game state (level, lives, JSONB) | owner only | owner only |
| `public.user_settings` | Per-(user, game) JSONB settings; `00000000-…` UUID for cross-game globals | owner only | owner only |
| `public.achievements` | Per-game achievement catalog (code, name, description, points) | public | none (catalog) |
| `public.user_achievements` | `(user_id, achievement_id)` unlock log | public | owner only |
| `public.leaderboard` (view) | Public leaderboard joining `user_scores` × `user_handles` | public | n/a |

Server-side bits:

- Triggers `touch_last_played` / `touch_updated_at` auto-stamp timestamps so clients don't send them.
- `add_game_score(p_game_id uuid, p_session_score int)` RPC merges scores atomically (`high_score = greatest(...)`, `total_score += delta`, `plays += 1`). `security definer` + checks `auth.uid()` so callers can only write their own row.
- Trigger on `auth.users` insert auto-creates a default handle (`player-` + first 8 hex of UUID).

## OAuth providers (Google / Facebook / Apple)

Each provider has to be configured per Supabase project; this is a one-time setup step at <https://supabase.com/dashboard/project/edupgibalgeqfujfkwmm/auth/providers>.

The Antidote UI already shows three "Sign in with X" buttons. Until a provider is enabled in the dashboard, clicking its button will land the user on a Supabase error page ("Provider not enabled"). Email/password works regardless.

### Allowed redirect URLs (one-time)

Supabase Dashboard → Authentication → **URL Configuration** → "Redirect URLs". Add:

- `https://larsbrubaker.github.io/antidote/`  (production GitHub Pages)
- `http://localhost:5173/`                    (local `bun run dev`)
- (any future deployed origin)

Without this, OAuth round trips fail with "redirect_to is not in the allowed list".

### Google

1. <https://console.cloud.google.com/apis/credentials> → Create Credentials → OAuth client ID → Web application.
2. **Authorized redirect URIs:** `https://edupgibalgeqfujfkwmm.supabase.co/auth/v1/callback`
3. Copy **Client ID** + **Client Secret** into Supabase Dashboard → Authentication → Providers → Google.
4. Toggle Google **on**.

### Facebook

1. <https://developers.facebook.com/apps> → Create App → "Consumer".
2. Add the **Facebook Login** product → Settings.
3. **Valid OAuth Redirect URIs:** `https://edupgibalgeqfujfkwmm.supabase.co/auth/v1/callback`
4. Settings → Basic: copy **App ID** + **App Secret** into Supabase Dashboard → Authentication → Providers → Facebook.

### Apple (Sign in with Apple)

Most involved — requires an Apple Developer account ($99/yr).

1. <https://developer.apple.com/account/resources/identifiers/list/serviceId> → register a Services ID.
2. Enable **Sign in with Apple**, configure with: domain = `edupgibalgeqfujfkwmm.supabase.co`, return URL = `https://edupgibalgeqfujfkwmm.supabase.co/auth/v1/callback`.
3. Create a **Key** (Sign in with Apple key) → download `.p8`.
4. Supabase Dashboard → Authentication → Providers → Apple: paste **Services ID**, **Team ID**, **Key ID**, contents of the `.p8` private key.

### Per-platform behavior (current)

| Platform | OAuth flow today |
|---|---|
| Web (GitHub Pages, local Vite dev) | Full round trip. Page redirects to Supabase → provider → back to page. TS shell parses the `#access_token=…` hash and calls `wasm.oauth_complete(...)` to install the session. |
| Native desktop (Win/Mac/Linux) | Button opens the system browser at the Supabase auth URL. There's no localhost callback handler yet, so the redirect tokens land outside the app — sign in with email/password instead until the localhost-PKCE shell lands. Tracked in `antidote_todo.md`. |
| iOS / Android | Future. Same identity model; needs platform deep-link / app-link wiring. |

## Files

- `migrations/0001_init.sql` — base schema + RLS for games / user_scores / user_progress / user_settings.
- `migrations/0002_seed_games.sql` — seed `antidote` row in `games`.
- `migrations/0003_achievements_and_triggers.sql` — achievements catalog, per-user unlock log, timestamp triggers, `add_game_score` RPC.
- `migrations/0004_handles_and_leaderboard_view.sql` — `user_handles` table, auto-handle trigger on `auth.users`, public `leaderboard` view, drop direct anon read on `user_scores`.

`SUPABASE_ACCESS_TOKEN` (PAT) and `SUPABASE_DB_PASSWORD` live in `.env` (gitignored). The running app **never** uses either — only the publishable key + per-user JWT, all gated by RLS.
