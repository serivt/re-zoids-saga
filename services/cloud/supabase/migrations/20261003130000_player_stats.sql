-- How far the players with cloud saves have come, for the project's
-- maintainer alone, read from what the cloud saves already keep: each
-- save's summary ("Lv 31, area 10, 4698050 G, 87:23 played"), when it was
-- stored, and the achievements' list. The views live in the schema
-- `stats`, which the API does not serve (it serves `public` and
-- `graphql_public`) and which neither the page's anonymous role nor the
-- signed-in players may use: they are read in Supabase's SQL editor.
--
-- Source of knowledge: this project's own design (see docs/web.md).

create schema stats;
revoke all on schema stats from public, anon, authenticated;

comment on schema stats is 'The maintainer''s views of how far the players with cloud saves have come.';

-- How many achievements the achievements' text `data` (Base64) unlocks: its
-- lines but the header; none when it does not read.
create function stats.achievement_count(data text) returns integer
    language plpgsql immutable set search_path = '' as $$
begin
    return (
        select count(*)::integer
        from regexp_split_to_table(convert_from(decode(data, 'base64'), 'UTF8'), '\n') as line
        where btrim(line) <> '' and line not like '#%'
    );
exception when others then
    return null;
end;
$$;

-- Each save's level, area, money and minutes played, read from its summary.
create view stats.save_progress with (security_invoker = true) as
select
    saves.user_id,
    saves.rom,
    saves.slot,
    saves.updated_at,
    (regexp_match(saves.summary, '^Lv (\d+)'))[1]::integer as level,
    (regexp_match(saves.summary, 'area (\d+)'))[1]::integer as area,
    (regexp_match(saves.summary, '(\d+) G'))[1]::bigint as money,
    (
        select time[1]::integer * 60 + time[2]::integer
        from regexp_match(saves.summary, '(\d+):(\d+) played') as time
    ) as played_minutes
from public.saves
where saves.slot <> 'achievements';

comment on view stats.save_progress is 'Each cloud save''s level, area, money and minutes played.';

-- Each player and ROM: the furthest their saves reach (the highest level,
-- area, money and time played among them), how many slots they use, when
-- they last saved, and how many achievements they unlocked.
create view stats.player_progress with (security_invoker = true) as
select
    users.email,
    users.created_at as signed_up,
    users.last_sign_in_at as last_sign_in,
    left(progress.rom, 8) as rom,
    max(progress.level) as level,
    max(progress.area) as area,
    max(progress.money) as money,
    max(progress.played_minutes) as played_minutes,
    count(*) filter (where progress.slot like 'slot-%') as slots_used,
    max(progress.updated_at) as last_saved,
    (
        select stats.achievement_count(achievements.data)
        from public.saves as achievements
        where achievements.user_id = progress.user_id
            and achievements.rom = progress.rom
            and achievements.slot = 'achievements'
    ) as achievements
from stats.save_progress as progress
join auth.users as users on users.id = progress.user_id
group by users.id, users.email, users.created_at, users.last_sign_in_at, progress.user_id, progress.rom
order by last_saved desc;

comment on view stats.player_progress is 'Each player''s furthest progress by ROM, newest first.';

-- How many accounts there are, how many keep saves, and how many saved in
-- the last week and month.
create view stats.overview with (security_invoker = true) as
select
    (select count(*) from auth.users) as accounts,
    (select count(distinct user_id) from public.saves) as with_saves,
    (select count(distinct user_id) from public.saves where updated_at > now() - interval '7 days') as saved_last_7_days,
    (select count(distinct user_id) from public.saves where updated_at > now() - interval '30 days') as saved_last_30_days;

comment on view stats.overview is 'Accounts, players with saves, and players who saved lately.';

-- How many players have reached each area as their furthest.
create view stats.furthest_area with (security_invoker = true) as
select furthest.area, count(*) as players
from (
    select user_id, max(area) as area
    from stats.save_progress
    group by user_id
) as furthest
group by furthest.area
order by furthest.area;

comment on view stats.furthest_area is 'How many players reached each area as their furthest.';

revoke all on all tables in schema stats from public, anon, authenticated;
revoke all on all functions in schema stats from public, anon, authenticated;
