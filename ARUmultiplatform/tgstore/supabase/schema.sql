-- UZ: tgstore RO'YXATI (Supabase / Postgres): FAQAT BITTA JADVAL "DB_CHUNK". Har qator = bitta bo'lak fayli (db_1_v1, db_2_v1, ...) yoki yordamchi qator.
--     Bo'lak 1 MB ga to'lsa worker keyingisini (db_2_v1) yaratadi; o'zgarganda versiya oshadi (db_1_v2), eskisi 1 daqiqadan keyin Telegram'dan o'chadi.
--     Ma'lumotning o'zi YO'Q: faqat nom, Telegram file_id, ochish kaliti va kalit oralig'i.
-- EN: tgstore REGISTRY (Supabase / Postgres): ONE TABLE "DB_CHUNK". One row per chunk file (db_1_v1, db_2_v1, ...) or helper row. When a chunk reaches 1 MB the worker
--     creates the next one (db_2_v1); a change bumps the version (db_1_v2) and the old one is deleted from Telegram after a minute. No data here, only name/file_id/key/range.
-- RU: REESTR tgstore: ODNA TABLITSA "DB_CHUNK". Odna stroka = odin fayl chanka (db_1_v1, db_2_v1, ...) ili sluzhebnaya stroka.
--
-- UZ: `kind`: 'chunk' (bo'lak) | 'gc' (o'chiriladigan eski fayl) | 'mark' (kesh belgisi) | 'auth' (maxfiy kalit xeshi) | 'ping' (kunlik uyg'otish).
-- UZ: XAVFSIZLIK: RLS yoqilgan, siyosat YO'Q (ochiq kalit bilan jadvalni to'g'ridan-to'g'ri o'qib/yozib BO'LMAYDI). Hamma funksiya `security definer` va birinchi parametri `p_tok`
--     (workerdagi `STORE_KEY`); bazada faqat uning SHA-256 xeshi. service_role kaliti KERAK EMAS.
-- EN: SECURITY: RLS on, no policies. Every function is `security definer` and needs `p_tok`; only its SHA-256 hash is stored. No service_role key needed.

create table if not exists "DB_CHUNK" (
  name    text primary key,                 -- chunk: db_<no>_v<ver> | gc:<msg_id> | mark:<key> | auth | ping
  kind    text not null default 'chunk',
  no      bigint,
  ver     bigint,
  file_id text,
  msg_id  bigint,
  key     text,                             -- AES-256-GCM kaliti (hex) / decryption key
  kmin    text,
  kmax    text,
  rows    int    default 0,
  size    bigint default 0,
  due     bigint,                           -- gc: o'chirish vaqti (ms)
  val     bigint,                           -- mark / ping qiymati
  note    text,                             -- auth: sha256(STORE_KEY)
  at      timestamptz default now()
);
create index if not exists db_chunk_kind_kmin on "DB_CHUNK" (kind, kmin);
alter table "DB_CHUNK" enable row level security;

-- ── Eski jadvallardan KO'CHIRISH (bo'lsa): tirik bo'laklar saqlanadi, qolgan eski fayllar o'chirish navbatiga tushadi ──────────────────────────────
do $$
begin
  if to_regclass('public.kv_chunks') is not null then
    insert into "DB_CHUNK"(name, kind, no, ver, file_id, msg_id, key, kmin, kmax, rows, size)
      select 'db_' || no || '_v' || ver, 'chunk', no, ver, file_id, msg_id, key, kmin, kmax, rows, size from kv_chunks where tbl = 'pool'
      on conflict do nothing;
    insert into "DB_CHUNK"(name, kind, msg_id, due)
      select 'gc:' || msg_id, 'gc', msg_id, 0 from kv_chunks where tbl <> 'pool' and msg_id <> 0
      on conflict do nothing;
  end if;
  if to_regclass('public.kv_gc') is not null then
    insert into "DB_CHUNK"(name, kind, msg_id, due) select 'gc:' || msg_id, 'gc', msg_id, due from kv_gc on conflict do nothing;
  end if;
end $$;

insert into "DB_CHUNK"(name, kind, note) values ('auth', 'auth', 'f874232e514ca8c0475e091aea63bfe731c4ca6e34b6f5db04c25480251e36d0')
  on conflict (name) do update set note = excluded.note;
insert into "DB_CHUNK"(name, kind, val) values ('ping', 'ping', 0) on conflict do nothing;

-- ── Funksiyalar / Functions ─────────────────────────────────────────────────────────────────────────────────────

create or replace function kv_check(p_tok text) returns void
language plpgsql stable security definer set search_path = public as $$
begin
  if p_tok is null or encode(sha256(convert_to(p_tok, 'utf8')), 'hex') is distinct from (select note from "DB_CHUNK" where name = 'auth') then
    raise exception 'unauthorized' using errcode = '28000';
  end if;
end $$;

-- UZ: hamma tirik bo'laklar (kalit oralig'i bo'yicha tartiblangan) va keyingi bo'lak raqami / all live chunks and the next chunk number
create or replace function kv_list(p_tok text) returns jsonb
language plpgsql security definer set search_path = public as $$
begin
  perform kv_check(p_tok);
  return jsonb_build_object(
    'rev', 0,
    'next_no', coalesce((select max(no) from "DB_CHUNK" where kind = 'chunk'), 0) + 1,
    'chunks', coalesce((select jsonb_agg(jsonb_build_object(
                'no', no, 'ver', ver, 'name', name, 'file_id', file_id, 'msg_id', msg_id,
                'key', key, 'min', kmin, 'max', kmax, 'rows', rows, 'size', size) order by kmin)
              from "DB_CHUNK" where kind = 'chunk'), '[]'::jsonb));
end $$;

-- UZ: bo'laklarni ATOMIK almashtirish. p = {"now","gc_delay","read":[nom..],"remove":[nom..],"add":[{no,ver,name,file_id,msg_id,key,min,max,rows,size}]}
--     `read` va `remove` dagi bo'laklar hali tirik bo'lishi shart, yangi nomlar band bo'lmasligi shart; aks holda HECH NARSA o'zgarmaydi: {"ok":false}.
-- EN: ATOMIC chunk swap with a read-set check (a stale read/remove or a taken new name changes nothing).
create or replace function kv_commit(p_tok text, p jsonb) returns jsonb
language plpgsql security definer set search_path = public as $$
declare names text[]; rem text[]; item jsonb;
begin
  perform kv_check(p_tok);
  perform pg_advisory_xact_lock(424201);
  rem := array(select jsonb_array_elements_text(coalesce(p->'remove', '[]'::jsonb)));
  names := array(select jsonb_array_elements_text(coalesce(p->'read', '[]'::jsonb)) union select unnest(rem));
  if coalesce(array_length(names, 1), 0) > 0
     and (select count(*) from "DB_CHUNK" where kind = 'chunk' and name = any(names)) <> array_length(names, 1) then
    return jsonb_build_object('ok', false, 'conflict', 'stale');
  end if;
  if exists (select 1 from "DB_CHUNK" c where c.kind = 'chunk' and c.name in (select a->>'name' from jsonb_array_elements(coalesce(p->'add', '[]'::jsonb)) a)
             and not (c.name = any(rem))) then
    return jsonb_build_object('ok', false, 'conflict', 'name');
  end if;
  insert into "DB_CHUNK"(name, kind, msg_id, due)
    select 'gc:' || msg_id, 'gc', msg_id, (p->>'now')::bigint + (p->>'gc_delay')::bigint
    from "DB_CHUNK" where kind = 'chunk' and msg_id <> 0 and name = any(rem)
    on conflict do nothing;
  delete from "DB_CHUNK" where kind = 'chunk' and name = any(rem);
  for item in select * from jsonb_array_elements(coalesce(p->'add', '[]'::jsonb)) loop
    insert into "DB_CHUNK"(name, kind, no, ver, file_id, msg_id, key, kmin, kmax, rows, size)
    values (item->>'name', 'chunk', (item->>'no')::bigint, (item->>'ver')::bigint, item->>'file_id', coalesce((item->>'msg_id')::bigint, 0),
            item->>'key', item->>'min', item->>'max', coalesce((item->>'rows')::int, 0), coalesce((item->>'size')::bigint, 0));
  end loop;
  return jsonb_build_object('ok', true, 'list', kv_list(p_tok));
end $$;

create or replace function kv_gc_take(p_tok text, p_now bigint) returns jsonb
language plpgsql security definer set search_path = public as $$
begin
  perform kv_check(p_tok);
  return jsonb_build_object(
    'due', coalesce((select jsonb_agg(jsonb_build_object('msg_id', msg_id)) from (select msg_id from "DB_CHUNK" where kind = 'gc' and due <= p_now order by due limit 100) g), '[]'::jsonb),
    'pending', (select count(*) from "DB_CHUNK" where kind = 'gc'));
end $$;

create or replace function kv_gc_done(p_tok text, p_ids bigint[]) returns jsonb
language plpgsql security definer set search_path = public as $$
begin
  perform kv_check(p_tok);
  delete from "DB_CHUNK" where kind = 'gc' and msg_id = any(p_ids);
  return jsonb_build_object('ok', true, 'pending', (select count(*) from "DB_CHUNK" where kind = 'gc'));
end $$;

-- UZ: keshni bekor qilish belgilari: p = {"bump":[k..], "at": ms, "get":[k..]} -> {"v": {k: n}} (monoton)
create or replace function kv_marks(p_tok text, p jsonb) returns jsonb
language plpgsql security definer set search_path = public as $$
declare kk text; at_ bigint := coalesce((p->>'at')::bigint, 0);
begin
  perform kv_check(p_tok);
  for kk in select jsonb_array_elements_text(coalesce(p->'bump', '[]'::jsonb)) loop
    insert into "DB_CHUNK"(name, kind, val) values ('mark:' || kk, 'mark', greatest(at_, 1))
    on conflict (name) do update set val = greatest(excluded.val, "DB_CHUNK".val + 1);
  end loop;
  return jsonb_build_object('v', coalesce((
    select jsonb_object_agg(substr(m.name, 6), m.val) from "DB_CHUNK" m
    where m.kind = 'mark' and m.name in (select 'mark:' || x from jsonb_array_elements_text(coalesce(p->'get', '[]'::jsonb)) x)), '{}'::jsonb));
end $$;

create or replace function kv_ping(p_tok text) returns jsonb
language plpgsql security definer set search_path = public as $$
begin
  perform kv_check(p_tok);
  update "DB_CHUNK" set val = coalesce(val, 0) + 1, at = now() where name = 'ping';
  return (select jsonb_build_object('at', at, 'n', val) from "DB_CHUNK" where name = 'ping');
end $$;

-- UZ: to'liq tozalash: Telegram'dan o'chiriladigan xabar raqamlari (navbatdagi p_limit ta), keyin belgilash va oxirida jadvalni bo'shatish
create or replace function kv_wipe_take(p_tok text, p_limit int) returns jsonb
language plpgsql security definer set search_path = public as $$
declare ids bigint[];
begin
  perform kv_check(p_tok);
  select coalesce(array_agg(m), '{}') into ids from (
    select msg_id as m from "DB_CHUNK" where kind in ('chunk', 'gc') and msg_id <> 0 limit p_limit) x;
  return jsonb_build_object('ids', to_jsonb(ids));
end $$;

create or replace function kv_wipe_done(p_tok text, p_ids bigint[], p_final boolean) returns jsonb
language plpgsql security definer set search_path = public as $$
begin
  perform kv_check(p_tok);
  update "DB_CHUNK" set msg_id = 0 where kind = 'chunk' and msg_id = any(p_ids);
  delete from "DB_CHUNK" where kind = 'gc' and msg_id = any(p_ids);
  if p_final then
    delete from "DB_CHUNK" where kind in ('chunk', 'gc', 'mark');
  end if;
  return jsonb_build_object('ok', true);
end $$;

revoke all on function kv_check(text) from public, anon, authenticated;
revoke all on function kv_list(text), kv_commit(text, jsonb), kv_gc_take(text, bigint), kv_gc_done(text, bigint[]), kv_marks(text, jsonb), kv_ping(text),
                       kv_wipe_take(text, int), kv_wipe_done(text, bigint[], boolean) from public;
grant execute on function kv_list(text), kv_commit(text, jsonb), kv_gc_take(text, bigint), kv_gc_done(text, bigint[]), kv_marks(text, jsonb), kv_ping(text),
                          kv_wipe_take(text, int), kv_wipe_done(text, bigint[], boolean) to anon, authenticated, service_role;

-- ── Eski jadval va funksiyalarni olib tashlash / drop the old tables and functions ────────────────────────────────────────────────────────────────────
drop function if exists kv_get_many(text, text[]);
drop function if exists kv_drop_table(text, text);
drop table if exists kv_tables, kv_chunks, kv_gc, kv_marks, kv_auth, kv_ping cascade;
