-- The cloud saves' rules: each player reaches their own saves alone, a
-- save's versions before are kept, bad rows are refused, and deleting an
-- account deletes its saves. Run with `supabase test db --workdir
-- services/cloud`.
begin;
create extension if not exists pgtap with schema extensions;
select plan(9);

insert into auth.users (id, email) values
    ('11111111-1111-1111-1111-111111111111', 'one@example.com'),
    ('22222222-2222-2222-2222-222222222222', 'two@example.com');

set local role authenticated;
set local request.jwt.claims to '{"sub": "11111111-1111-1111-1111-111111111111", "role": "authenticated"}';

insert into public.saves (rom, slot, data, summary)
    values ('8bd2a7c5e3a4b0f9d1c6e2a7b3f4c5d6e7a8b9c0', 'slot-1', 'AAAA', 'Lv 1, area 1, 0 G');
select is((select count(*)::int from public.saves), 1, 'a player sees their own save');
select is((select user_id::text from public.saves), '11111111-1111-1111-1111-111111111111',
    'the save is the signed-in player''s');

update public.saves set data = 'BBBB';
update public.saves set data = 'CCCC';
select is((select count(*)::int from public.save_history), 2, 'the versions before are kept');

select throws_ok(
    $$ insert into public.saves (rom, slot, data) values ('8bd2a7c5e3a4b0f9d1c6e2a7b3f4c5d6e7a8b9c0', 'slot-9', 'AA') $$,
    '23514', null, 'a slot the game has not is refused');
select throws_ok(
    $$ insert into public.saves (rom, slot, data) values ('not-a-sha1', 'slot-2', 'AA') $$,
    '23514', null, 'a ROM named other than by its SHA-1 is refused');

set local request.jwt.claims to '{"sub": "22222222-2222-2222-2222-222222222222", "role": "authenticated"}';
select is((select count(*)::int from public.saves), 0, 'another player sees none of them');
select is((select count(*)::int from public.save_history), 0, 'nor their history');
select throws_ok(
    $$ insert into public.saves (user_id, rom, slot, data) values ('11111111-1111-1111-1111-111111111111', '8bd2a7c5e3a4b0f9d1c6e2a7b3f4c5d6e7a8b9c0', 'slot-2', 'AA') $$,
    '42501', null, 'nor can they write one in their name');

set local request.jwt.claims to '{"sub": "11111111-1111-1111-1111-111111111111", "role": "authenticated"}';
select public.delete_my_account();
reset role;
select is((select count(*)::int from public.saves), 0, 'deleting the account deletes its saves');

select * from finish();
rollback;
