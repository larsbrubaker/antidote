-- 0001_init.sql — Antidote (multi-game) schema for Supabase Postgres.
-- Apply via:   supabase db push
-- or:          psql "$SUPABASE_DB_URL" -f db/migrations/0001_init.sql

create extension if not exists "uuid-ossp";

-- Catalog of games hosted on this Supabase project. The "other games" panel
-- and every (user_id, game_id) row in the rest of the schema points back here.
create table if not exists public.games (
    id            uuid primary key default uuid_generate_v4(),
    slug          text not null unique,
    display_name  text not null,
    description   text,
    icon_url      text,
    deploy_url    text,
    sort_order    int  not null default 100,
    created_at    timestamptz not null default now()
);

-- Per-user, per-game high-water marks (leaderboard source).
create table if not exists public.user_scores (
    user_id      uuid not null references auth.users(id) on delete cascade,
    game_id      uuid not null references public.games(id) on delete cascade,
    high_score   int  not null default 0,
    total_score  bigint not null default 0,
    plays        int  not null default 0,
    last_played  timestamptz not null default now(),
    primary key (user_id, game_id)
);
create index if not exists user_scores_game_high on public.user_scores (game_id, high_score desc);
create index if not exists user_scores_user_last on public.user_scores (user_id, last_played desc);

-- Resumable in-progress game state (one row per game per user).
create table if not exists public.user_progress (
    user_id          uuid not null references auth.users(id) on delete cascade,
    game_id          uuid not null references public.games(id) on delete cascade,
    current_level    int  not null default 1,
    lives_remaining  int  not null default 3,
    state            jsonb not null default '{}'::jsonb,
    updated_at       timestamptz not null default now(),
    primary key (user_id, game_id)
);

-- Settings keyed by (user_id, game_id). Cross-game user settings (master volume,
-- theme, etc.) live under the all-zeros UUID sentinel.
create table if not exists public.user_settings (
    user_id     uuid not null,
    game_id     uuid not null,
    settings    jsonb not null default '{}'::jsonb,
    updated_at  timestamptz not null default now(),
    primary key (user_id, game_id),
    constraint user_settings_user_fk foreign key (user_id) references auth.users(id) on delete cascade
);

-- Row-Level Security
alter table public.games            enable row level security;
alter table public.user_scores      enable row level security;
alter table public.user_progress    enable row level security;
alter table public.user_settings    enable row level security;

-- Anyone can read the games catalog.
drop policy if exists games_read_all on public.games;
create policy games_read_all on public.games
    for select using (true);

-- Users can only modify their own scores; everyone can read the leaderboard.
drop policy if exists own_scores on public.user_scores;
create policy own_scores on public.user_scores
    using (user_id = auth.uid()) with check (user_id = auth.uid());

drop policy if exists public_leaderboard_read on public.user_scores;
create policy public_leaderboard_read on public.user_scores
    for select using (true);

drop policy if exists own_progress on public.user_progress;
create policy own_progress on public.user_progress
    using (user_id = auth.uid()) with check (user_id = auth.uid());

drop policy if exists own_settings on public.user_settings;
create policy own_settings on public.user_settings
    using (user_id = auth.uid()) with check (user_id = auth.uid());
