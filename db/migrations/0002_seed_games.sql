-- 0002_seed_games.sql — initial games catalog row for Antidote.
insert into public.games (slug, display_name, description, deploy_url, sort_order)
values (
    'antidote',
    'Antidote',
    'Bubble-trap virus puzzle game.',
    'https://larsbrubaker.github.io/antidote/',
    10
)
on conflict (slug) do update set
    display_name = excluded.display_name,
    description  = excluded.description,
    deploy_url   = excluded.deploy_url;
