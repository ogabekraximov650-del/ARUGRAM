-- UZ: tozalash va jadvalni olib tashlash funksiyalari (tgstore). / EN: wipe and drop-table functions. / RU: funktsii ochistki.
create or replace function kv_wipe_take(p_tok text, p_limit int) returns jsonb
language plpgsql security definer set search_path = public as $$
declare ids bigint[];
begin
  perform kv_check(p_tok);
  select coalesce(array_agg(m), '{}') into ids from (
    select msg_id as m from kv_chunks where msg_id <> 0
    union
    select msg_id from kv_gc where msg_id <> 0
    limit p_limit) x;
  return jsonb_build_object('ids', to_jsonb(ids));
end $$;

create or replace function kv_wipe_done(p_tok text, p_ids bigint[], p_final boolean) returns jsonb
language plpgsql security definer set search_path = public as $$
begin
  perform kv_check(p_tok);
  update kv_chunks set msg_id = 0 where msg_id = any(p_ids);
  delete from kv_gc where msg_id = any(p_ids);
  if p_final then
    delete from kv_chunks where true;
    delete from kv_tables where true;
    delete from kv_gc where true;
    delete from kv_marks where true;
  end if;
  return jsonb_build_object('ok', true);
end $$;

create or replace function kv_drop_table(p_tok text, p_tbl text) returns jsonb
language plpgsql security definer set search_path = public as $$
declare ids bigint[];
begin
  perform kv_check(p_tok);
  select coalesce(array_agg(msg_id), '{}') into ids from kv_chunks where tbl = p_tbl and msg_id <> 0;
  delete from kv_chunks where tbl = p_tbl;
  delete from kv_tables where tbl = p_tbl;
  return jsonb_build_object('ids', to_jsonb(ids));
end $$;

revoke all on function kv_wipe_take(text, int), kv_wipe_done(text, bigint[], boolean), kv_drop_table(text, text) from public;
grant execute on function kv_wipe_take(text, int), kv_wipe_done(text, bigint[], boolean), kv_drop_table(text, text) to anon, authenticated, service_role;
