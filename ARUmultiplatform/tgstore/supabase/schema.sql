-- UZ: tgstore RO'YXATI (Supabase / Postgres). FAQAT bo'lak nomi, Telegram file_id, ochish kaliti va kalit oralig'i. Ma'lumotning o'zi YO'Q.
-- EN: tgstore REGISTRY (Supabase / Postgres). ONLY chunk name, Telegram file_id, decryption key and key range. NO data.
-- RU: REESTR tgstore (Supabase / Postgres). TOLKO imya chanka, file_id, klyuch i diapazon. Dannykh NET.
--
-- UZ: Hamma jadvalda RLS yoqilgan va siyosat YO'Q: faqat `service_role` kaliti (workerda, maxfiy) kira oladi; anon/authenticated — yo'q.
-- EN: RLS is on with no policies: only the `service_role` key (secret, in the worker) can access; anon/authenticated cannot.

create table if not exists kv_tables (
  tbl      text primary key,
  rev      bigint not null default 0,
  next_no  bigint not null default 1
);

create table if not exists kv_chunks (
  tbl      text   not null,
  no       bigint not null,
  ver      bigint not null,
  name     text   not null,
  file_id  text   not null,
  msg_id   bigint not null default 0,
  key      text   not null,
  kmin     text   not null,
  kmax     text   not null,
  rows     int    not null default 0,
  size     bigint not null default 0,
  primary key (tbl, no)
);
create index if not exists kv_chunks_range on kv_chunks (tbl, kmin);

-- UZ: almashtirilgan eski fayllar (Telegram'dan keyinroq o'chiriladi) / EN: replaced files to delete later / RU: starye fayly na udalenie
create table if not exists kv_gc (
  msg_id bigint primary key,
  name   text,
  due    bigint not null
);

-- UZ: keshni bekor qilish belgilari (production `MarkHub` o'rniga) / EN: cache-invalidation marks (instead of the MarkHub DO)
create table if not exists kv_marks (
  k text primary key,
  v bigint not null default 0
);

-- UZ: kunlik "uyg'otish" (bepul reja 7 kun so'rovsiz to'xtatiladi) / EN: daily keep-alive
create table if not exists kv_ping (
  id int primary key default 1,
  at timestamptz not null default now(),
  n  bigint not null default 0
);
insert into kv_ping(id) values (1) on conflict do nothing;

alter table kv_tables enable row level security;
alter table kv_chunks enable row level security;
alter table kv_gc     enable row level security;
alter table kv_marks  enable row level security;
alter table kv_ping   enable row level security;

-- ── Funksiyalar / Functions ─────────────────────────────────────────────────────────────────────────────────────

-- UZ: bir nechta jadvalning ro'yxati bir chaqiruvda: {"tbl": {"rev":..,"next_no":..,"chunks":[...]}}
create or replace function kv_get_many(p_tbls text[]) returns jsonb
language sql stable as $$
  select coalesce(jsonb_object_agg(t,
    jsonb_build_object(
      'rev',     coalesce((select rev from kv_tables where tbl = t), 0),
      'next_no', coalesce((select next_no from kv_tables where tbl = t), 1),
      'chunks',  coalesce((select jsonb_agg(jsonb_build_object(
                    'no', c.no, 'ver', c.ver, 'name', c.name, 'file_id', c.file_id, 'msg_id', c.msg_id,
                    'key', c.key, 'min', c.kmin, 'max', c.kmax, 'rows', c.rows, 'size', c.size) order by c.kmin)
                  from kv_chunks c where c.tbl = t), '[]'::jsonb)
    )), '{}'::jsonb)
  from unnest(p_tbls) as t;
$$;

-- UZ: ATOMIK commit — bir nechta jadval bitta tranzaksiyada. p = {"now": ms, "gc_delay": ms,
--     "tables":[{"tbl","rev","remove":[no..],"add":[{no,ver,name,file_id,msg_id,key,min,max,rows,size}]}]}
--     Biror jadvalning `rev` i mos kelmasa HECH NARSA o'zgarmaydi: {"ok":false,"conflict":"<jadval>"}.
-- EN: ATOMIC multi-table commit; a stale `rev` on any table changes nothing and reports the conflict.
create or replace function kv_commit(p jsonb) returns jsonb
language plpgsql as $$
declare
  t jsonb; tb text; cur bigint; item jsonb; n bigint; res jsonb := '{}'::jsonb; order_tbls text[];
begin
  -- jadvallarni tartib bilan qulflaymiz (deadlock bo'lmasin)
  select array_agg(x->>'tbl' order by x->>'tbl') into order_tbls from jsonb_array_elements(p->'tables') x;
  foreach tb in array order_tbls loop
    insert into kv_tables(tbl) values (tb) on conflict do nothing;
    perform 1 from kv_tables where tbl = tb for update;
  end loop;
  -- 1) tekshiruv
  for t in select * from jsonb_array_elements(p->'tables') loop
    select rev into cur from kv_tables where tbl = t->>'tbl';
    if cur <> (t->>'rev')::bigint then
      return jsonb_build_object('ok', false, 'conflict', t->>'tbl');
    end if;
  end loop;
  -- 2) qo'llash
  for t in select * from jsonb_array_elements(p->'tables') loop
    tb := t->>'tbl';
    insert into kv_gc(msg_id, name, due)
      select c.msg_id, c.name, (p->>'now')::bigint + (p->>'gc_delay')::bigint
      from kv_chunks c
      where c.tbl = tb and c.msg_id <> 0
        and c.no in (select (v)::bigint from jsonb_array_elements_text(t->'remove') v)
    on conflict do nothing;
    delete from kv_chunks where tbl = tb and no in (select (v)::bigint from jsonb_array_elements_text(t->'remove') v);
    for item in select * from jsonb_array_elements(t->'add') loop
      n := (item->>'no')::bigint;
      delete from kv_chunks where tbl = tb and no = n;
      insert into kv_chunks(tbl, no, ver, name, file_id, msg_id, key, kmin, kmax, rows, size)
      values (tb, n, (item->>'ver')::bigint, item->>'name', item->>'file_id', coalesce((item->>'msg_id')::bigint, 0),
              item->>'key', item->>'min', item->>'max', coalesce((item->>'rows')::int, 0), coalesce((item->>'size')::bigint, 0));
      update kv_tables set next_no = greatest(next_no, n + 1) where tbl = tb;
    end loop;
    update kv_tables set rev = rev + 1 where tbl = tb;
  end loop;
  return jsonb_build_object('ok', true, 'metas', kv_get_many(order_tbls));
end $$;

create or replace function kv_gc_take(p_now bigint) returns jsonb
language sql stable as $$
  select jsonb_build_object(
    'due', coalesce((select jsonb_agg(jsonb_build_object('msg_id', msg_id, 'name', name)) from (select * from kv_gc where due <= p_now order by due limit 50) g), '[]'::jsonb),
    'pending', (select count(*) from kv_gc));
$$;

create or replace function kv_gc_done(p_ids bigint[]) returns jsonb
language plpgsql as $$
begin
  delete from kv_gc where msg_id = any(p_ids);
  return jsonb_build_object('ok', true, 'pending', (select count(*) from kv_gc));
end $$;

-- UZ: keshni bekor qilish belgilari: p = {"bump":[k..], "at": ms, "get":[k..]} -> {"v": {k: n}}  (production `MarkHub` bilan bir xil ma'no: monoton)
create or replace function kv_marks(p jsonb) returns jsonb
language plpgsql as $$
declare k text; at_ bigint := coalesce((p->>'at')::bigint, 0);
begin
  for k in select jsonb_array_elements_text(coalesce(p->'bump', '[]'::jsonb)) loop
    insert into kv_marks(k, v) values (k, greatest(at_, 1))
    on conflict (k) do update set v = greatest(excluded.v, kv_marks.v + 1);
  end loop;
  return jsonb_build_object('v', coalesce((
    select jsonb_object_agg(m.k, m.v) from kv_marks m
    where m.k in (select jsonb_array_elements_text(coalesce(p->'get', '[]'::jsonb)))), '{}'::jsonb));
end $$;

create or replace function kv_ping() returns jsonb
language plpgsql as $$
begin
  update kv_ping set at = now(), n = n + 1 where id = 1;
  return (select jsonb_build_object('at', at, 'n', n) from kv_ping where id = 1);
end $$;

-- UZ: funksiyalarni faqat service_role chaqira oladi / EN: only service_role may call them
revoke all on function kv_get_many(text[]), kv_commit(jsonb), kv_gc_take(bigint), kv_gc_done(bigint[]), kv_marks(jsonb), kv_ping() from public, anon, authenticated;
grant execute on function kv_get_many(text[]), kv_commit(jsonb), kv_gc_take(bigint), kv_gc_done(bigint[]), kv_marks(jsonb), kv_ping() to service_role;
