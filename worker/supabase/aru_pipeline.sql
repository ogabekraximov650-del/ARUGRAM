-- ARUMEDIATV: worker -> Supabase "SQL funksiya" (Hrana /v2/pipeline bilan mos javob qaytaradi).
-- UZ: worker SQLite tilidagi buyruqlarni (`?`, `?N`, INSERT OR IGNORE, GROUP_CONCAT ...) shu funksiyaga yuboradi; u Postgres tiliga o'giradi va bajaradi.
-- EN: translates SQLite-dialect statements to Postgres and returns a Hrana-compatible JSON result. Called through PostgREST RPC with a shared secret.
-- RU: perevodit SQL SQLite v Postgres i vozvrashchaet otvet, sovmestimyi s Hrana pipeline.

create schema if not exists arumediatv;
create table if not exists arumediatv.aru_auth (h text primary key);
alter table arumediatv.aru_auth enable row level security;

create or replace function arumediatv.instr(a text, b text) returns int language sql immutable as $$ select strpos(a, b) $$;

do $$ begin
  if not exists (select 1 from pg_aggregate a join pg_proc p on p.oid=a.aggfnoid where p.proname='total' and p.pronamespace='arumediatv'::regnamespace) then
    create aggregate arumediatv.total(double precision) (sfunc = float8pl, stype = float8, initcond = '0');
  end if;
end $$;

-- strftime formati (SQLite) -> to_char formati (Postgres)
create or replace function arumediatv.tr_fmt(f text) returns text language sql immutable as $$
  select replace(replace(replace(replace(replace(replace(replace(f,'%Y','YYYY'),'%m','MM'),'%d','DD'),'%H','HH24'),'%M','MI'),'%S','SS'),'%w','D')
$$;


-- sqlite_master o'rnini bosuvchi ko'rinish (faqat name, type)
create or replace view arumediatv.sqlite_master as
  select table_name::text as name, case when table_type = 'VIEW' then 'view' else 'table' end::text as type
  from information_schema.tables where table_schema = 'arumediatv';

-- fname(a, b, ...) chaqiruvlarini qavslar chuqurligini hisobga olib qayta yozadi (max/min — skalyar bo'lsa greatest/least; group_concat — string_agg)
create or replace function arumediatv.rewrite_fn(s text, fname text) returns text language plpgsql immutable as $$
declare
  res text := ''; rest text := s; m int; o int; depth int; i int; n int; ch text; cur text; args text[]; k int; rep text;
begin
  loop
    m := regexp_instr(rest, '\m' || fname || '\s*\(', 1, 1, 0, 'i');
    exit when m = 0;
    o := m + position('(' in substr(rest, m)) - 1;
    depth := 1; i := o + 1; cur := ''; args := '{}'; n := length(rest);
    while i <= n loop
      ch := substr(rest, i, 1);
      if ch = '(' then depth := depth + 1; cur := cur || ch;
      elsif ch = ')' then
        depth := depth - 1;
        exit when depth = 0;
        cur := cur || ch;
      elsif ch = ',' and depth = 1 then args := args || cur; cur := '';
      else cur := cur || ch; end if;
      i := i + 1;
    end loop;
    exit when depth <> 0;
    args := args || cur;
    for k in 1..array_length(args, 1) loop args[k] := arumediatv.rewrite_fn(args[k], fname); end loop;
    if lower(fname) in ('max', 'min') then
      if array_length(args, 1) >= 2 then rep := (case when lower(fname) = 'max' then 'greatest(' else 'least(' end) || array_to_string(args, ',') || ')';
      else rep := fname || '(' || args[1] || ')'; end if;
    else
      if array_length(args, 1) = 1 then rep := 'string_agg((' || args[1] || ')::text, '','')';
      else rep := 'string_agg((' || args[1] || ')::text,' || args[2] || ')'; end if;
    end if;
    res := res || substr(rest, 1, m - 1) || rep;
    rest := substr(rest, i + 1);
  end loop;
  return res || rest;
end $$;

-- ON CONFLICT ... DO UPDATE SET a=expr: ifoda ichidagi yalang'och ustun nomlari jadval nomi bilan to'ldiriladi (Postgres'da bir ma'noli bo'lishi uchun)
create or replace function arumediatv.qualify_set(setpart text, tbl text) returns text language plpgsql stable as $$
declare
  res text := ''; depth int := 0; i int; ch text; cur text := ''; parts text[] := '{}'; p text; eq int; lhs text; rhs text; c text; first boolean := true;
begin
  for i in 1..length(setpart) loop
    ch := substr(setpart, i, 1);
    if ch = '(' then depth := depth + 1; elsif ch = ')' then depth := depth - 1; end if;
    if ch = ',' and depth = 0 then parts := parts || cur; cur := ''; else cur := cur || ch; end if;
  end loop;
  parts := parts || cur;
  foreach p in array parts loop
    eq := position('=' in p);
    if eq = 0 then lhs := ''; rhs := p; else lhs := substr(p, 1, eq); rhs := substr(p, eq + 1); end if;
    for c in select a.attname from pg_attribute a join pg_class cl on cl.oid = a.attrelid join pg_namespace ns on ns.oid = cl.relnamespace
              where ns.nspname = 'arumediatv' and cl.relname = tbl and a.attnum > 0 and not a.attisdropped loop
      rhs := regexp_replace(rhs, '(?<![.\w"])' || c || '(?![\w(])', tbl || '.' || c, 'gi');
    end loop;
    res := res || case when first then '' else ',' end || lhs || rhs;
    first := false;
  end loop;
  return res;
end $$;

-- DDL (SQLite) -> Postgres
create or replace function arumediatv.tr_ddl(sql_in text) returns text language plpgsql immutable as $$
declare s text := regexp_replace(sql_in, '--[^\n]*', '', 'g');
begin
  s := regexp_replace(s, '\mINTEGER\s+PRIMARY\s+KEY\s+AUTOINCREMENT\M', 'BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY', 'gi');
  s := regexp_replace(s, '\mINTEGER\s+PRIMARY\s+KEY\M', 'BIGINT PRIMARY KEY', 'gi');
  s := regexp_replace(s, '\mINTEGER\M', 'BIGINT', 'gi');
  s := regexp_replace(s, '\mREAL\M', 'DOUBLE PRECISION', 'gi');
  s := regexp_replace(s, '\mBLOB\M', 'BYTEA', 'gi');
  s := regexp_replace(s, '\mWITHOUT\s+ROWID\M', '', 'gi');
  s := regexp_replace(s, '\mADD\s+COLUMN\s+(?!IF\s)', 'ADD COLUMN IF NOT EXISTS ', 'gi');
  return s;
end $$;

-- SQLite SQL -> Postgres SQL (literal'lar avval olib qo'yiladi, so'ng qaytariladi)
create or replace function arumediatv.tr(sql_in text, args json default '[]'::json) returns text language plpgsql stable as $$
declare
  s text := sql_in; lits text[] := '{}'; m text[]; i int := 0;
  ign boolean := false; rep boolean := false; tbl text; cols text; pk text; sets text; k int; parts text[]; av json; lit text; idx int; nargs int;
begin
  s := regexp_replace(s, 'NOT\s+GLOB\s+''\*\[\^0-9\]\*''', '!~ ''[^0-9]''', 'gi');
  loop
    m := regexp_match(s, '''(?:[^'']|'''')*''');
    exit when m is null;
    i := i + 1; lits := lits || m[1];
    s := regexp_replace(s, '''(?:[^'']|'''')*''', E'\u0001' || i || E'\u0001');
  end loop;
  s := regexp_replace(s, 'strftime\(\s*\x01(\d+)\x01\s*,\s*((?:[^()]|\([^()]*\))+?)\s*,\s*\x01\d+\x01\s*\)', E'to_char(to_timestamp(\\2) at time zone ''UTC'', \u0001F\\1\u0001)', 'gi');
  s := regexp_replace(s, '\mINSERT\s+OR\s+IGNORE\s+INTO\M', 'INSERT INTO', 'gi');
  if sql_in ~* 'INSERT\s+OR\s+IGNORE' then ign := true; end if;
  if sql_in ~* '(INSERT\s+OR\s+REPLACE\s+INTO|^\s*REPLACE\s+INTO)' then
    rep := true;
    s := regexp_replace(s, '\m(INSERT\s+OR\s+REPLACE\s+INTO|REPLACE\s+INTO)\M', 'INSERT INTO', 'gi');
  end if;
  s := arumediatv.rewrite_fn(s, 'group_concat');
  s := arumediatv.rewrite_fn(s, 'max');
  s := arumediatv.rewrite_fn(s, 'min');
  s := regexp_replace(s, '\(\s*instr\(([^()]*)\)\s*>\s*0\s*\)', '(CASE WHEN strpos(\1)>0 THEN 1 ELSE 0 END)', 'gi');
  s := regexp_replace(s, '\mLIKE\M', 'ILIKE', 'gi');
  s := regexp_replace(s, '\mAS\s+INTEGER\M', 'AS BIGINT', 'gi');
  s := regexp_replace(s, '\mAS\s+REAL\M', 'AS DOUBLE PRECISION', 'gi');
  if ign then
    if s ~* '\mON\s+CONFLICT\M' then null;
    elsif s ~* '\mRETURNING\M' then s := regexp_replace(s, '\mRETURNING\M', 'ON CONFLICT DO NOTHING RETURNING', 'i');
    else s := rtrim(s, '; ') || ' ON CONFLICT DO NOTHING'; end if;
  end if;
  if rep then
    tbl := (regexp_match(s, 'INSERT\s+INTO\s+"?(\w+)"?\s*\(([^)]*)\)', 'i'))[1];
    cols := (regexp_match(s, 'INSERT\s+INTO\s+"?(\w+)"?\s*\(([^)]*)\)', 'i'))[2];
    select string_agg(quote_ident(at.attname), ',' order by x.ord) into pk
      from pg_index i2 join pg_class c on c.oid = i2.indrelid join pg_namespace ns on ns.oid = c.relnamespace
      cross join lateral unnest(i2.indkey) with ordinality x(attnum, ord)
      join pg_attribute at on at.attrelid = c.oid and at.attnum = x.attnum
     where i2.indisprimary and ns.nspname = 'arumediatv' and c.relname = tbl;
    select string_agg(format('%1$s=excluded.%1$s', trim(c2)), ',') into sets
      from unnest(string_to_array(cols, ',')) c2 where trim(c2) <> all (string_to_array(coalesce(replace(pk, '"', ''), ''), ','));
    if pk is not null then
      s := rtrim(s, '; ') || ' ON CONFLICT (' || pk || ') DO ' || case when sets is null then 'NOTHING' else 'UPDATE SET ' || sets end;
    end if;
  end if;
  -- argumentlar: `?N` va `?` o'rniga literal'lar (matn: tur Postgres'ga qoldiriladi). Literal'lar hali olib qo'yilgan holatda — ichidagi `?` tegilmaydi.
  -- ON CONFLICT ... DO UPDATE SET: yalang'och ustun nomlari bir ma'noli bo'lsin
  if s ~* '\mDO\s+UPDATE\s+SET\M' then
    tbl := coalesce(tbl, (regexp_match(s, 'INSERT\s+INTO\s+"?(\w+)"?', 'i'))[1]);
    declare setpart text; tail text; pre text; begin
      pre := (regexp_match(s, '^(.*?\mDO\s+UPDATE\s+SET\M)', 'is'))[1];
      setpart := substr(s, length(pre) + 1);
      tail := coalesce((regexp_match(setpart, '(\m(?:WHERE|RETURNING)\M.*)$', 'is'))[1], '');
      setpart := substr(setpart, 1, length(setpart) - length(tail));
      s := pre || arumediatv.qualify_set(setpart, tbl) || ' ' || tail;
    end;
  end if;
  nargs := coalesce(json_array_length(args), 0);
  s := regexp_replace(s, '\?(\d+)', E'\u0002\\1\u0002', 'g');
  parts := string_to_array(s, '?');
  s := parts[1];
  for idx in 2..coalesce(array_length(parts,1),1) loop
    av := args->(idx-2);
    lit := case when av is null or (av->>'type') = 'null' then 'NULL' else quote_literal(av->>'value') end;
    s := s || lit || parts[idx];
  end loop;
  for idx in 1..nargs loop
    av := args->(idx-1);
    lit := case when av is null or (av->>'type') = 'null' then 'NULL' else quote_literal(av->>'value') end;
    s := replace(s, E'\u0002' || idx || E'\u0002', lit);
  end loop;
  -- literal'larni qaytarish (strftime formatlari ham)
  for k in reverse i..1 loop
    s := replace(s, E'\u0001F' || k || E'\u0001', '''' || arumediatv.tr_fmt(trim(both '''' from lits[k])) || '''');
    s := replace(s, E'\u0001' || k || E'\u0001', lits[k]);
  end loop;
  return s;
end $$;

create or replace function public.aru_pipeline(k text, req json) returns json
language plpgsql security definer set search_path = arumediatv, pg_catalog, public as $$
declare
  r json; stmt json; q_sql text; args json; q_out text;
  results json[] := '{}'; res json; rec record; js json; cols text[]; rowsj json[]; firstrow boolean; cnt bigint; ret boolean;
begin
  if not exists (select 1 from aru_auth where h = encode(sha256(convert_to(coalesce(k,''),'UTF8')),'hex')) then
    raise exception 'forbidden' using errcode = '28000';
  end if;
  for r in select * from json_array_elements(req->'requests') loop
    if r->>'type' <> 'execute' then results := results || json_build_object('type','ok','response', json_build_object('type', r->>'type')); continue; end if;
    stmt := r->'stmt'; q_sql := stmt->>'sql'; args := coalesce(stmt->'args', '[]'::json);
    begin
      if q_sql ~* '^\s*(create\s+index|create\s+unique\s+index)' then
        begin execute arumediatv.tr_ddl(q_sql); exception when others then null; end;
        res := json_build_object('cols','[]'::json,'rows','[]'::json,'affected_row_count',0);
      elsif q_sql ~* '^\s*(create|alter)\M' then
        execute arumediatv.tr_ddl(q_sql);
        res := json_build_object('cols','[]'::json,'rows','[]'::json,'affected_row_count',0);
      elsif q_sql ~* '^\s*(drop|pragma)\M' then
        res := json_build_object('cols','[]'::json,'rows','[]'::json,'affected_row_count',0);
      else
        q_out := arumediatv.tr(q_sql, args);
        ret := q_out ~* '^\s*(select|with)\M' or q_out ~* '\mreturning\M';
        if ret then
          cols := '{}'; rowsj := '{}'; firstrow := true;
          for rec in execute q_out loop
            js := row_to_json(rec);
            if firstrow then cols := array(select key from json_each(js)); firstrow := false; end if;
            rowsj := rowsj || (select json_agg(case json_typeof(v)
                when 'null' then json_build_object('type','null')
                when 'number' then case when v::text ~ '[.eE]' then json_build_object('type','float','value', v) else json_build_object('type','integer','value', v::text) end
                when 'boolean' then json_build_object('type','integer','value', case when v::text='true' then '1' else '0' end)
                else json_build_object('type','text','value', v #>> '{}') end order by o)
              from json_each(js) with ordinality as t(key, v, o));
          end loop;
          get diagnostics cnt = row_count;
          res := json_build_object('cols', (select coalesce(json_agg(json_build_object('name', c)), '[]'::json) from unnest(cols) c), 'rows', coalesce(to_json(rowsj), '[]'::json), 'affected_row_count', case when q_out ~* '^\s*(select|with)' then 0 else cnt end);
        else
          execute q_out; get diagnostics cnt = row_count;
          res := json_build_object('cols','[]'::json,'rows','[]'::json,'affected_row_count',cnt);
        end if;
      end if;
      results := results || json_build_object('type','ok','response', json_build_object('type','execute','result', res));
    exception when others then
      results := results || json_build_object('type','error','error', json_build_object('message', SQLERRM));
    end;
  end loop;
  return json_build_object('results', to_json(results));
end $$;

revoke all on function public.aru_pipeline(text, json) from public, authenticated;
grant execute on function public.aru_pipeline(text, json) to anon, service_role;
