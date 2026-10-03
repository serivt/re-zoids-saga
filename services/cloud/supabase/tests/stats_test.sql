-- The maintainer's views of the players' progress: each player's furthest
-- save read from the summaries, the achievements counted, an unreadable
-- list counted as none, the totals, and no access for the page's roles.
-- Run with `supabase test db --workdir services/cloud`.
begin;
create extension if not exists pgtap with schema extensions;
select plan(9);

insert into auth.users (id, email) values
    ('11111111-1111-1111-1111-111111111111', 'one@example.com'),
    ('22222222-2222-2222-2222-222222222222', 'two@example.com');

insert into public.saves (user_id, rom, slot, data, summary) values
    ('11111111-1111-1111-1111-111111111111', '8bd2a7c5e3a4b0f9d1c6e2a7b3f4c5d6e7a8b9c0', 'slot-1', 'AAAA',
        'Lv 31, area 10, 4698050 G, 87:23 played'),
    ('11111111-1111-1111-1111-111111111111', '8bd2a7c5e3a4b0f9d1c6e2a7b3f4c5d6e7a8b9c0', 'slot-2', 'AAAA',
        'Lv 12, area 4, 9000 G'),
    ('11111111-1111-1111-1111-111111111111', '8bd2a7c5e3a4b0f9d1c6e2a7b3f4c5d6e7a8b9c0', 'achievements',
        'IyByZS16b2lkcy1zYWdhIGFjaGlldmVtZW50cwpjaGFwdGVyLTEKY2hhcHRlci0yCnZldGVyYW4K', null),
    ('22222222-2222-2222-2222-222222222222', '8bd2a7c5e3a4b0f9d1c6e2a7b3f4c5d6e7a8b9c0', 'autosave', 'AAAA',
        'Lv 3, area 1, 120 G, 0:45 played'),
    ('22222222-2222-2222-2222-222222222222', '8bd2a7c5e3a4b0f9d1c6e2a7b3f4c5d6e7a8b9c0', 'achievements',
        '!!! not base64 !!!', null);

select is(
    (select row(level, area, money, played_minutes, slots_used)::text
        from stats.player_progress where email = 'one@example.com'),
    '(31,10,4698050,5243,2)',
    'a player''s furthest save is read from the summaries');
select is(
    (select achievements from stats.player_progress where email = 'one@example.com'),
    3, 'the achievements are counted, the header left out');
select is(
    (select row(level, slots_used)::text from stats.player_progress where email = 'two@example.com'),
    '(3,0)', 'the autosave counts, though it is no slot');
select is(
    (select achievements from stats.player_progress where email = 'two@example.com'),
    null::integer, 'an unreadable list counts as none, and the view still answers');
select is(
    (select row(accounts, with_saves, saved_last_7_days)::text from stats.overview),
    '(2,2,2)', 'the overview counts the accounts and the players who saved');
select is(
    (select string_agg(area || ':' || players, ',' order by area) from stats.furthest_area),
    '1:1,10:1', 'each player counts once, at their furthest area');

set local role anon;
select throws_ok($$ select * from stats.player_progress $$, '42501', null,
    'the page''s anonymous role cannot read the stats');
set local role authenticated;
set local request.jwt.claims to '{"sub": "11111111-1111-1111-1111-111111111111", "role": "authenticated"}';
select throws_ok($$ select * from stats.overview $$, '42501', null,
    'nor can a signed-in player');
select throws_ok($$ select stats.achievement_count('AAAA') $$, '42501', null,
    'nor call its functions');

reset role;
select * from finish();
rollback;
