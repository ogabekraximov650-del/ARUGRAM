// ═══════════════════════════════════════════════════════════════
//  FOYDALANUVCHINING TELEGRAM HISOBIDAGI STIKER, EMOJI VA GIFLARI
// ═══════════════════════════════════════════════════════════════
//
// Faqat KO'RISH uchun: to'plamlar ro'yxati, to'plam ichidagi hujjatlar va
// saqlangan GIFlar (`messages.getAllStickers`, `getEmojiStickers`,
// `getStickerSet`, `getSavedGifs`), hamda bitta hujjat/kichik rasmni
// faylga yuklab olish (`upload.getFile`). Ilova baytlarni o'zi o'zgartiradi
// va shifrlab keshlaydi; bu yerda hech narsa chizilmaydi.
//
// Bu fayl `telegram.rs` ichiga `include!` qilinadi (uning yopiq
// funksiyalaridan foydalanadi).

/// Hujjat (stiker/GIF/emoji) -> JSON. `emoji` — to'plam jadvalidan.
fn media_doc_json(d: &tl::types::Document, emoji: Option<&str>) -> Value {
    let (mut w, mut h) = (0i32, 0i32);
    let mut alt = String::new();
    let (mut animated, mut video, mut custom, mut mask) = (false, false, false, false);
    for a in &d.attributes {
        match a {
            tl::enums::DocumentAttribute::ImageSize(x) => {
                w = x.w;
                h = x.h;
            }
            tl::enums::DocumentAttribute::Animated => animated = true,
            tl::enums::DocumentAttribute::Sticker(s) => {
                alt = s.alt.clone();
                mask = s.mask;
            }
            tl::enums::DocumentAttribute::Video(v) => {
                video = true;
                w = v.w;
                h = v.h;
            }
            tl::enums::DocumentAttribute::CustomEmoji(c) => {
                custom = true;
                if alt.is_empty() {
                    alt = c.alt.clone();
                }
            }
            _ => {}
        }
    }
    if let Some(e) = emoji {
        if alt.is_empty() {
            alt = e.to_string();
        }
    }
    // Kichik rasm: avval "m" (taxminan 128-320 px), keyin "s", keyin birinchisi.
    let mut thumb = String::new();
    let mut rank = 0;
    if let Some(list) = &d.thumbs {
        for s in list {
            let ty = match s {
                tl::enums::PhotoSize::Size(x) => x.r#type.clone(),
                tl::enums::PhotoSize::Progressive(x) => x.r#type.clone(),
                _ => continue,
            };
            let r = match ty.as_str() {
                "m" => 3,
                "s" => 2,
                _ => 1,
            };
            if r > rank {
                rank = r;
                thumb = ty;
            }
        }
    }
    let kind = match d.mime_type.as_str() {
        "application/x-tgsticker" => "tgs",
        "video/webm" => "webm",
        "image/webp" => "webp",
        "video/mp4" => "mp4",
        "image/gif" => "gif",
        other => other,
    };
    json!({
        "id": d.id,
        "hash": d.access_hash,
        "ref": hex::encode(&d.file_reference),
        "dc": d.dc_id,
        "mime": d.mime_type,
        "kind": kind,
        "size": d.size,
        "w": w,
        "h": h,
        "emoji": alt,
        "thumb": thumb,
        "animated": animated || kind == "tgs" || kind == "webm" || kind == "mp4",
        "video": video,
        "custom": custom,
        "mask": mask,
    })
}

fn media_set_json(s: &tl::types::StickerSet) -> Value {
    json!({
        "id": s.id,
        "hash": s.access_hash,
        "title": s.title,
        "short": s.short_name,
        "count": s.count,
        "emoji": s.emojis,
        "mask": s.masks,
        "official": s.official,
        "thumb_doc": s.thumb_document_id.unwrap_or(0),
        "thumb_ver": s.thumb_version.unwrap_or(0),
        "thumb_dc": s.thumb_dc_id.unwrap_or(0),
    })
}

fn media_docs_json(docs: &[tl::enums::Document], emoji_of: &std::collections::HashMap<i64, String>) -> Vec<Value> {
    docs.iter()
        .filter_map(|d| match d {
            tl::enums::Document::Document(x) => Some(media_doc_json(x, emoji_of.get(&x.id).map(|s| s.as_str()))),
            _ => None,
        })
        .collect()
}

/// Buyruqlar: `sets:stickers`, `sets:emoji`, `set:<id>:<hash>`, `gifs`.
async fn media_list(client: &Client, arg: &str) -> Result<Value, String> {
    match arg {
        "sets:stickers" | "sets:emoji" => {
            let r = if arg == "sets:emoji" {
                client.invoke(&tl::functions::messages::GetEmojiStickers { hash: 0 }).await
            } else {
                client.invoke(&tl::functions::messages::GetAllStickers { hash: 0 }).await
            }
            .map_err(|e| inv_err(&e))?;
            let mut extra: Vec<Value> = Vec::new();
            if arg == "sets:emoji" {
                // O'rnatilgan to'plam bo'lmasa ham emoji chiqishi uchun: Telegram tavsiya qilgan (featured) emoji to'plamlari.
                if let Ok(tl::enums::messages::FeaturedStickers::Stickers(f)) =
                    client.invoke(&tl::functions::messages::GetFeaturedEmojiStickers { hash: 0 }).await
                {
                    for c in &f.sets {
                        let tl::enums::StickerSet::Set(set) = c.set();
                        extra.push(media_set_json(&set));
                    }
                }
            }
            let mut sets = match r {
                tl::enums::messages::AllStickers::Stickers(x) => x
                    .sets
                    .iter()
                    .map(|s| match s {
                        tl::enums::StickerSet::Set(x) => x,
                    })
                    .filter(|s| !s.masks)
                    .map(media_set_json)
                    .collect::<Vec<_>>(),
                tl::enums::messages::AllStickers::NotModified => Vec::new(),
            };
            for e in extra {
                if !sets.iter().any(|x| x["id"] == e["id"]) {
                    sets.push(e);
                }
            }
            Ok(json!({"ok": true, "sets": sets}))
        }
        "gifs" => {
            let r = client
                .invoke(&tl::functions::messages::GetSavedGifs { hash: 0 })
                .await
                .map_err(|e| inv_err(&e))?;
            let docs = match r {
                tl::enums::messages::SavedGifs::Gifs(x) => media_docs_json(&x.gifs, &Default::default()),
                tl::enums::messages::SavedGifs::NotModified => Vec::new(),
            };
            Ok(json!({"ok": true, "docs": docs}))
        }
        _ if arg.starts_with("emoji:") => {
            // Maxsus emoji hujjati faqat raqami bo'yicha (boshqa foydalanuvchi yuborgan matndagi emoji uchun).
            let id: i64 = arg[6..].parse().map_err(|_| "emoji raqami noto'g'ri".to_string())?;
            let docs = client
                .invoke(&tl::functions::messages::GetCustomEmojiDocuments { document_id: vec![id] })
                .await
                .map_err(|e| inv_err(&e))?;
            Ok(json!({"ok": true, "docs": media_docs_json(&docs, &Default::default())}))
        }
        _ if arg.starts_with("set:") || arg.starts_with("setname:") => {
            // `set:<id>:<hash>` — o'z to'plamimiz; `setname:<qisqa_nom>` — ochiq nom bo'yicha (boshqa foydalanuvchi yuborgan stiker uchun).
            let stickerset = if let Some(short) = arg.strip_prefix("setname:") {
                if short.is_empty() || short.len() > 64 || !short.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    return Err("to'plam nomi noto'g'ri".to_string());
                }
                tl::enums::InputStickerSet::ShortName(tl::types::InputStickerSetShortName { short_name: short.to_string() })
            } else {
                let mut it = arg[4..].split(':');
                let id: i64 = it.next().and_then(|v| v.parse().ok()).ok_or("to'plam raqami noto'g'ri")?;
                let hash: i64 = it.next().and_then(|v| v.parse().ok()).ok_or("to'plam kaliti noto'g'ri")?;
                tl::enums::InputStickerSet::Id(tl::types::InputStickerSetId { id, access_hash: hash })
            };
            let r = client
                .invoke(&tl::functions::messages::GetStickerSet { stickerset, hash: 0 })
                .await
                .map_err(|e| inv_err(&e))?;
            match r {
                tl::enums::messages::StickerSet::Set(x) => {
                    // hujjat -> emoji jadvali (birinchi mos kelgani)
                    let mut emoji_of = std::collections::HashMap::new();
                    for p in &x.packs {
                        let tl::enums::StickerPack::Pack(p) = p;
                        for id in &p.documents {
                            emoji_of.entry(*id).or_insert_with(|| p.emoticon.clone());
                        }
                    }
                    let tl::enums::StickerSet::Set(info) = &x.set;
                    Ok(json!({"ok": true, "set": media_set_json(info), "docs": media_docs_json(&x.documents, &emoji_of)}))
                }
                tl::enums::messages::StickerSet::NotModified => Ok(json!({"ok": true, "docs": []})),
            }
        }
        _ => Err("noma'lum buyruq".to_string()),
    }
}

/// To'plamlar/GIF ro'yxatini oladi (BLOKLAYDI — faqat fon oqimidan chaqiring). Javob: `{"ok":true,...}` yoki `{"error":...}`.
#[no_mangle]
pub extern "C" fn rust_tg_media_list(arg_ptr: *const c_char) -> *mut c_char {
    let arg = unsafe { cstr_to_str(arg_ptr) }.unwrap_or("").trim().to_string();
    string_to_cptr(with_client(|t, client| {
        if !t.authorized.load(Ordering::SeqCst) {
            return Err("Telegram ulanmagan".to_string());
        }
        let v = run_tmo(t, 40, media_list(&client, &arg))?;
        Ok(v.to_string())
    }))
}

/// Bitta hujjat (yoki uning kichik rasmi) baytlarini `out` fayliga yozadi.
async fn media_get(t: &'static Tg, client: &Client, v: &Value) -> Result<Value, String> {
    // To'plam muqovasi: `set_id`, `set_hash`, `set_ver` (id/hash/ref kerak emas).
    let set_id = v["set_id"].as_i64();
    let id = if set_id.is_some() { 0 } else { v["id"].as_i64().ok_or("id yo'q")? };
    let hash = if set_id.is_some() { 0 } else { v["hash"].as_i64().ok_or("kalit yo'q")? };
    let fref = hex::decode(v["ref"].as_str().unwrap_or("")).map_err(|_| "havola noto'g'ri".to_string())?;
    let mut dc = v["dc"].as_i64().unwrap_or(2) as i32;
    let thumb = v["thumb"].as_str().unwrap_or("").to_string();
    let out = v["out"].as_str().ok_or("fayl yo'li yo'q")?.to_string();
    let max: usize = v["max"].as_u64().unwrap_or(8 * 1024 * 1024) as usize;
    const LIMIT: i32 = 256 * 1024;
    let _permit = t.inflight.acquire().await.map_err(|e| e.to_string())?;
    let mut buf: Vec<u8> = Vec::new();
    let mut offset: i64 = 0;
    loop {
        let req = tl::functions::upload::GetFile {
            precise: false,
            cdn_supported: false,
            location: match set_id {
                Some(sid) => tl::enums::InputFileLocation::InputStickerSetThumb(tl::types::InputStickerSetThumb {
                    stickerset: tl::enums::InputStickerSet::Id(tl::types::InputStickerSetId { id: sid, access_hash: v["set_hash"].as_i64().unwrap_or(0) }),
                    thumb_version: v["set_ver"].as_i64().unwrap_or(0) as i32,
                }),
                None => tl::enums::InputFileLocation::InputDocumentFileLocation(tl::types::InputDocumentFileLocation {
                    id,
                    access_hash: hash,
                    file_reference: fref.clone(),
                    thumb_size: thumb.clone(),
                }),
            },
            offset,
            limit: LIMIT,
        };
        let mut got: Option<Vec<u8>> = None;
        let mut last_err = String::new();
        for _ in 0..PART_ATTEMPTS + 1 {
            let c = part_client(t, client, dc);
            match c.invoke_in_dc(dc, &req).await {
                Ok(tl::enums::upload::File::File(f)) => {
                    mark_dc_ready(t, dc);
                    got = Some(f.bytes);
                    break;
                }
                Ok(tl::enums::upload::File::CdnRedirect(_)) => return Err("CDN yo'naltirishi kutilmagan".to_string()),
                Err(e) => {
                    last_err = inv_err(&e);
                    match rpc_name(&e) {
                        Some(n) if n.starts_with("FILE_REFERENCE_") => return Err("FILE_REFERENCE_EXPIRED".to_string()),
                        Some("AUTH_KEY_UNREGISTERED") => {
                            if let Err(e) = copy_auth(t, client, dc).await {
                                check_dead(t, &e);
                                return Err(e);
                            }
                        }
                        Some("FILE_MIGRATE") => {
                            if let InvocationError::Rpc(r) = &e {
                                if let Some(x) = r.value {
                                    dc = x as i32;
                                }
                            }
                        }
                        _ => {
                            check_dead(t, &last_err);
                            return Err(last_err);
                        }
                    }
                }
            }
        }
        let Some(bytes) = got else { return Err(last_err) };
        let n = bytes.len();
        buf.extend_from_slice(&bytes);
        if buf.len() > max {
            return Err("fayl juda katta".to_string());
        }
        if (n as i32) < LIMIT {
            break;
        }
        offset += n as i64;
    }
    if buf.is_empty() {
        return Err("fayl bo'sh".to_string());
    }
    std::fs::write(&out, &buf).map_err(|e| format!("yozib bo'lmadi: {e}"))?;
    Ok(json!({"ok": true, "size": buf.len()}))
}

/// Arg: JSON `{id,hash,ref,dc,thumb,out,max}`. Javob: `{"ok":true,"size":N}` yoki `{"error":...}`.
#[no_mangle]
pub extern "C" fn rust_tg_media_get(arg_ptr: *const c_char) -> *mut c_char {
    let arg = unsafe { cstr_to_str(arg_ptr) }.unwrap_or("").to_string();
    string_to_cptr(with_client(|t, client| {
        if !t.authorized.load(Ordering::SeqCst) {
            return Err("Telegram ulanmagan".to_string());
        }
        let v: Value = serde_json::from_str(&arg).map_err(|_| "so'rov noto'g'ri".to_string())?;
        let r = run_tmo(t, 90, media_get(t, &client, &v))?;
        Ok(r.to_string())
    }))
}
