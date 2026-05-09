-- 0003_achievements_and_triggers.sql
-- Adds the achievements catalog + per-user unlock log, server-side
-- timestamp triggers so clients don't need to send `last_played` /
-- `updated_at`, and a transactional RPC for atomic score upserts
-- (max-high-score / accumulate-total / +1-plays without a round-trip
-- read). Apply after 0001_init.sql + 0002_seed_games.sql.

create extension if not exists "uuid-ossp";

-- ─── Achievements ──────────────────────────────────────────────────────────

create table if not exists public.achievements (
    id           uuid primary key default uuid_generate_v4(),
    game_id      uuid not null references public.games(id) on delete cascade,
    code         text not null,
    name         text not null,
    description  text,
    icon         text,
    points       int  not null default 0,
    sort_order   int  not null default 100,
    created_at   timestamptz not null default now(),
    unique (game_id, code)
);
create index if not exists achievements_game_sort
    on public.achievements (game_id, sort_order);

create table if not exists public.user_achievements (
    user_id        uuid not null references auth.users(id) on delete cascade,
    achievement_id uuid not null references public.achievements(id) on delete cascade,
    unlocked_at    timestamptz not null default now(),
    metadata       jsonb not null default '{}'::jsonb,
    primary key (user_id, achievement_id)
);
create index if not exists user_achievements_user
    on public.user_achievements (user_id);

alter table public.achievements         enable row level security;
alter table public.user_achievements    enable row level security;

drop policy if exists achievements_read_all on public.achievements;
create policy achievements_read_all on public.achievements
    for select using (true);

drop policy if exists own_achievements on public.user_achievements;
create policy own_achievements on public.user_achievements
    using (user_id = auth.uid()) with check (user_id = auth.uid());

drop policy if exists user_achievements_public_read on public.user_achievements;
create policy user_achievements_public_read on public.user_achievements
    for select using (true);

-- ─── Server-side timestamp triggers ───────────────────────────────────────

create or replace function public.touch_last_played()
returns trigger
language plpgsql
as $fn$
begin
    new.last_played = now();
    return new;
end
$fn$;

create or replace function public.touch_updated_at()
returns trigger
language plpgsql
as $fn$
begin
    new.updated_at = now();
    return new;
end
$fn$;

drop trigger if exists trg_user_scores_last_played on public.user_scores;
create trigger trg_user_scores_last_played
    before insert or update on public.user_scores
    for each row execute function public.touch_last_played();

drop trigger if exists trg_user_progress_updated_at on public.user_progress;
create trigger trg_user_progress_updated_at
    before insert or update on public.user_progress
    for each row execute function public.touch_updated_at();

drop trigger if exists trg_user_settings_updated_at on public.user_settings;
create trigger trg_user_settings_updated_at
    before insert or update on public.user_settings
    for each row execute function public.touch_updated_at();

-- ─── Atomic score-upsert RPC ───────────────────────────────────────────────
--
-- Called from the client at level-complete / game-over to record the
-- session's score delta. Uses `auth.uid()` so callers can't write a row
-- for another user, and atomically:
--   - inserts (user_id, game_id) if missing, with the delta as both
--     high_score and total_score
--   - on conflict, sets high_score = greatest(stored, delta),
--     total_score += delta, plays += 1
-- The `last_played` trigger above stamps the timestamp for free.

create or replace function public.add_game_score(
    p_game_id      uuid,
    p_session_score int
)
returns void
language plpgsql
security definer
set search_path = public
as $fn$
declare uid uuid;
begin
    uid := auth.uid();
    if uid is null then
        raise exception 'not signed in';
    end if;
    if p_session_score < 0 then
        raise exception 'session score must be non-negative';
    end if;

    insert into public.user_scores (user_id, game_id, high_score, total_score, plays)
    values (uid, p_game_id, p_session_score, p_session_score, 1)
    on conflict (user_id, game_id) do update set
        high_score  = greatest(public.user_scores.high_score, p_session_score),
        total_score = public.user_scores.total_score + p_session_score,
        plays       = public.user_scores.plays + 1;
end
$fn$;

revoke all on function public.add_game_score(uuid, int) from public;
grant execute on function public.add_game_score(uuid, int) to authenticated;
