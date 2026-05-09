-- 0004_handles_and_leaderboard_view.sql
--
-- Hide user_id UUIDs from the public leaderboard. Adds a per-user `handle`
-- mirror table, an auto-handle trigger on auth.users, and a `leaderboard`
-- view that joins handles to scores. Drops the public read on
-- user_scores so anon can ONLY read scores through the view (which
-- hides user_id).

create extension if not exists "uuid-ossp";

-- ─── user_handles ─────────────────────────────────────────────────────────

create table if not exists public.user_handles (
    user_id    uuid primary key references auth.users(id) on delete cascade,
    handle     text not null unique check (length(handle) between 1 and 32),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

drop trigger if exists trg_user_handles_updated_at on public.user_handles;
create trigger trg_user_handles_updated_at
    before insert or update on public.user_handles
    for each row execute function public.touch_updated_at();

alter table public.user_handles enable row level security;

drop policy if exists own_handle on public.user_handles;
create policy own_handle on public.user_handles
    using (user_id = auth.uid()) with check (user_id = auth.uid());

drop policy if exists handles_public_read on public.user_handles;
create policy handles_public_read on public.user_handles
    for select using (true);

-- ─── Auto-create handle on user signup ─────────────────────────────────────

-- Trigger fires on every new auth.users insert (email/password OR OAuth).
-- The default handle is `player-XXXXXXXX` derived from the user's UUID,
-- which is unique by construction. The user can change it later via an
-- update against `public.user_handles` (RLS lets them write their own row).
create or replace function public.create_default_handle()
returns trigger
language plpgsql
security definer
set search_path = public
as $fn$
begin
    insert into public.user_handles (user_id, handle)
    values (
        new.id,
        'player-' || substr(replace(new.id::text, '-', ''), 1, 8)
    )
    on conflict do nothing;
    return new;
end
$fn$;

drop trigger if exists on_auth_user_created on auth.users;
create trigger on_auth_user_created
    after insert on auth.users
    for each row execute function public.create_default_handle();

-- Backfill handles for any users who already exist (idempotent — `on
-- conflict do nothing` skips users who already have a handle).
insert into public.user_handles (user_id, handle)
select
    u.id,
    'player-' || substr(replace(u.id::text, '-', ''), 1, 8)
from auth.users u
on conflict do nothing;

-- ─── leaderboard view ─────────────────────────────────────────────────────
--
-- Owned by `postgres` (the role that runs migrations). With pg15's default
-- view behavior (security_invoker = false), the view executes with the
-- creator's privileges and bypasses RLS on the underlying tables — anon
-- can read the view even though we drop the direct anon-read policy on
-- user_scores below. Columns intentionally omit user_id.

create or replace view public.leaderboard as
select
    g.slug         as game_slug,
    h.handle       as handle,
    s.high_score   as high_score,
    s.total_score  as total_score,
    s.plays        as plays,
    s.last_played  as last_played
from public.user_scores s
join public.games        g on g.id = s.game_id
join public.user_handles h on h.user_id = s.user_id;

grant select on public.leaderboard to anon, authenticated;

-- Now that the view exists, drop the policy that let anon read user_scores
-- directly. After this, anon gets no rows from `/rest/v1/user_scores` and
-- must go through `/rest/v1/leaderboard?game_slug=eq.<slug>`.
drop policy if exists public_leaderboard_read on public.user_scores;
