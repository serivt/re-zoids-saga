-- The players' saves in the cloud: for each account, each ROM (by its
-- SHA-1, never its bytes) and each of the game's saves (the four slots,
-- the autosave and the achievements), the save as the page keeps it
-- (Base64 text), what it holds for the lists, and when it was stored. Each
-- player sees and changes their own alone; the last versions of each save
-- are kept to go back to; and a player can delete their account, saves and
-- all.
--
-- Source of knowledge: this project's own design (see docs/web.md).

create table public.saves (
    user_id uuid not null default auth.uid() references auth.users (id) on delete cascade,
    rom text not null check (rom ~ '^[0-9a-f]{40}$'),
    slot text not null check (slot in ('slot-1', 'slot-2', 'slot-3', 'slot-4', 'autosave', 'achievements')),
    -- The save: 32 KiB of save memory as Base64, or the achievements' text.
    data text not null check (length(data) <= 65536),
    -- What it holds, as the page shows it ("Lv 31, area 10, ...").
    summary text check (length(summary) <= 200),
    updated_at timestamptz not null default now(),
    primary key (user_id, rom, slot)
);

comment on table public.saves is 'Each player''s saves by ROM and slot, as the web page keeps them.';

-- The versions a save had before, the last ten of each.
create table public.save_history (
    id bigint generated always as identity primary key,
    user_id uuid not null references auth.users (id) on delete cascade,
    rom text not null,
    slot text not null,
    data text not null,
    summary text,
    updated_at timestamptz not null
);

create index save_history_by_save on public.save_history (user_id, rom, slot, updated_at desc);

comment on table public.save_history is 'The last versions of each save, to go back to.';

alter table public.saves enable row level security;
alter table public.save_history enable row level security;

create policy "own saves are read" on public.saves
    for select to authenticated using (user_id = auth.uid());
create policy "own saves are added" on public.saves
    for insert to authenticated with check (user_id = auth.uid());
create policy "own saves are changed" on public.saves
    for update to authenticated using (user_id = auth.uid()) with check (user_id = auth.uid());
create policy "own saves are removed" on public.saves
    for delete to authenticated using (user_id = auth.uid());

create policy "own history is read" on public.save_history
    for select to authenticated using (user_id = auth.uid());

-- A save's time is the server's when it is stored, and its version before
-- goes to the history, of which the last ten are kept.
create function public.keep_save_history() returns trigger
    language plpgsql security definer set search_path = '' as $$
begin
    new.updated_at := now();
    if tg_op = 'UPDATE' and old.data is distinct from new.data then
        insert into public.save_history (user_id, rom, slot, data, summary, updated_at)
            values (old.user_id, old.rom, old.slot, old.data, old.summary, old.updated_at);
        delete from public.save_history
            where id in (
                select id from public.save_history
                where user_id = old.user_id and rom = old.rom and slot = old.slot
                order by updated_at desc
                offset 10
            );
    end if;
    return new;
end;
$$;

create trigger saves_keep_history
    before insert or update on public.saves
    for each row execute function public.keep_save_history();

-- Deletes the account of the player who asks, and with it their saves and
-- their history.
create function public.delete_my_account() returns void
    language plpgsql security definer set search_path = '' as $$
begin
    if auth.uid() is null then
        raise exception 'not signed in';
    end if;
    delete from auth.users where id = auth.uid();
end;
$$;

revoke all on function public.delete_my_account() from public, anon;
grant execute on function public.delete_my_account() to authenticated;
revoke all on function public.keep_save_history() from public, anon, authenticated;
