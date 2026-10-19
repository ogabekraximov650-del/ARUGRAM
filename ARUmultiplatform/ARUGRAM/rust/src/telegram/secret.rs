//! Maxfiy chatlar (end-to-end, MTProto 2.0). DrKLO `SecretChatHelper` ning Rust nusxasi:
//! Diffie-Hellman kalit almashinuvi (`messages.requestEncryption` / `acceptEncryption`), `decryptedMessageLayer` ni qo'lda
//! (de)serializatsiya qilish, AES-IGE + SHA-256 bilan shifrlash. Faqat matnli xabarlar; ommaviy media va TTL qo'llab-quvvatlanmaydi.
//!
//! Xabarlar va kalitlar faqat shu qurilmada (shifrlangan `secret.bin` faylida) turadi — Telegram serverida maxfiy chat tarixi yo'q.

use super::*;
use num_bigint::BigUint;

const LABEL_SECRET: &str = "tg-secret-v1";
/// Biz yuboradigan qatlam (`decryptedMessage#91cc4674` shu qatlamda).
const OUR_LAYER: i32 = 73;
/// Eng ko'pi bilan saqlanadigan xabarlar (har chatda).
const MAX_MSGS: usize = 1000;

// ───────────────────────── Holat (shifrlangan faylda) ─────────────────────────

fn secret_path() -> Option<PathBuf> {
    tg().map(|t| t.dir.join("secret.bin"))
}

fn state() -> &'static Mutex<Value> {
    static S: OnceLock<Mutex<Value>> = OnceLock::new();
    S.get_or_init(|| {
        let v = secret_path()
            .and_then(|p| read_sealed(&p, LABEL_SECRET))
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .unwrap_or(Value::Null);
        Mutex::new(if v.is_object() { v } else { json!({"chats": {}}) })
    })
}

/// Holatni o'zgartiradi va diskka (shifrlab) yozadi.
fn with<R>(f: impl FnOnce(&mut Value) -> R) -> R {
    let mut g = state().lock().unwrap_or_else(|e| e.into_inner());
    let r = f(&mut g);
    if let Some(p) = secret_path() {
        let _ = write_sealed(&p, LABEL_SECRET, g.to_string().as_bytes());
    }
    r
}

/// Holatni faqat o'qiydi.
fn peek<R>(f: impl FnOnce(&Value) -> R) -> R {
    let g = state().lock().unwrap_or_else(|e| e.into_inner());
    f(&g)
}

/// Chiqishda hamma maxfiy chatlar o'chadi.
pub(super) fn wipe() {
    {
        let mut g = state().lock().unwrap_or_else(|e| e.into_inner());
        *g = json!({"chats": {}});
    }
    if let Some(p) = secret_path() {
        let _ = fs::remove_file(p);
    }
}

// ───────────────────────── TL yozish / o'qish ─────────────────────────

struct W(Vec<u8>);

impl W {
    fn i32(&mut self, x: i32) {
        self.0.extend_from_slice(&x.to_le_bytes());
    }
    fn u32(&mut self, x: u32) {
        self.0.extend_from_slice(&x.to_le_bytes());
    }
    fn i64(&mut self, x: i64) {
        self.0.extend_from_slice(&x.to_le_bytes());
    }
    fn bytes(&mut self, b: &[u8]) {
        let n = b.len();
        let header = if n < 254 {
            self.0.push(n as u8);
            1
        } else {
            self.0.push(254);
            self.0.extend_from_slice(&(n as u32).to_le_bytes()[..3]);
            4
        };
        self.0.extend_from_slice(b);
        let pad = (4 - (header + n) % 4) % 4;
        self.0.extend(std::iter::repeat(0u8).take(pad));
    }
}

struct R<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> R<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.p + n > self.b.len() {
            return None;
        }
        let s = &self.b[self.p..self.p + n];
        self.p += n;
        Some(s)
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn i64(&mut self) -> Option<i64> {
        Some(i64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn bytes(&mut self) -> Option<Vec<u8>> {
        let first = *self.take(1)?.first()? as usize;
        let (n, header) = if first < 254 {
            (first, 1)
        } else {
            let l = self.take(3)?;
            (l[0] as usize | (l[1] as usize) << 8 | (l[2] as usize) << 16, 4)
        };
        let data = self.take(n)?.to_vec();
        let pad = (4 - (header + n) % 4) % 4;
        self.take(pad)?;
        Some(data)
    }
}

// ───────────────────────── Kriptografiya ─────────────────────────

fn sha256(parts: &[&[u8]]) -> [u8; 32] {
    let mut ctx = ring::digest::Context::new(&ring::digest::SHA256);
    for p in parts {
        ctx.update(p);
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(ctx.finish().as_ref());
    out
}

fn sha1(data: &[u8]) -> Vec<u8> {
    ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, data).as_ref().to_vec()
}

fn rand_bytes(n: usize) -> Vec<u8> {
    let mut v = vec![0u8; n];
    let _ = getrandom::getrandom(&mut v);
    v
}

fn pad256(n: &BigUint) -> Vec<u8> {
    let b = n.to_bytes_be();
    if b.len() >= 256 {
        b[b.len() - 256..].to_vec()
    } else {
        let mut v = vec![0u8; 256 - b.len()];
        v.extend(b);
        v
    }
}

/// DrKLO `Utilities.isGoodGaAndGb`: 1 < g_a < p-1, ikkala tomon ham 2^1984 dan katta.
fn good_ga(ga: &BigUint, p: &BigUint) -> bool {
    let one = BigUint::from(1u32);
    let pm1 = p - &one;
    if ga <= &one || ga >= &pm1 {
        return false;
    }
    let lim = BigUint::from(1u32) << 1984usize;
    ga >= &lim && &(p - ga) >= &lim
}

/// Kalit barmoq izi: SHA-1 ning oxirgi 8 baytı (little-endian).
fn fingerprint(key: &[u8]) -> i64 {
    let h = sha1(key);
    let mut b = [0u8; 8];
    b.copy_from_slice(&h[h.len() - 8..]);
    i64::from_le_bytes(b)
}

/// MTProto 2.0: `x` — chat yaratuvchisi yuborgan xabar uchun 0, boshqasi uchun 8.
fn kdf(key: &[u8], msg_key: &[u8], x: usize) -> ([u8; 32], [u8; 32]) {
    let a = sha256(&[msg_key, &key[x..x + 36]]);
    let b = sha256(&[&key[40 + x..40 + x + 36], msg_key]);
    let mut k = [0u8; 32];
    let mut iv = [0u8; 32];
    k[..8].copy_from_slice(&a[..8]);
    k[8..24].copy_from_slice(&b[8..24]);
    k[24..].copy_from_slice(&a[24..]);
    iv[..8].copy_from_slice(&b[..8]);
    iv[8..24].copy_from_slice(&a[8..24]);
    iv[24..].copy_from_slice(&b[24..]);
    (k, iv)
}

/// `layer` — `decryptedMessageLayer` baytlari. Natija: `fingerprint(8) | msg_key(16) | AES-IGE(len | layer | padding)`.
fn encrypt(key: &[u8], fp: i64, admin_me: bool, layer: &[u8]) -> Vec<u8> {
    let x = if admin_me { 0 } else { 8 };
    let mut data = Vec::with_capacity(layer.len() + 64);
    data.extend_from_slice(&(layer.len() as i32).to_le_bytes());
    data.extend_from_slice(layer);
    let rem = data.len() % 16;
    let mut extra = if rem != 0 { 16 - rem } else { 0 };
    extra += (2 + (rand_bytes(1)[0] % 3) as usize) * 16;
    data.extend(rand_bytes(extra));
    let full = sha256(&[&key[88 + x..88 + x + 32], &data]);
    let msg_key = &full[8..24];
    let (k, iv) = kdf(key, msg_key, x);
    grammers_crypto::aes::ige_encrypt(&mut data, &k, &iv);
    let mut out = Vec::with_capacity(24 + data.len());
    out.extend_from_slice(&fp.to_le_bytes());
    out.extend_from_slice(msg_key);
    out.extend(data);
    out
}

/// Qaytaradi: `decryptedMessageLayer` baytlari (uzunlik va to'ldirgichsiz).
fn decrypt(key: &[u8], fp: i64, admin_me: bool, bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() < 24 + 16 || (bytes.len() - 24) % 16 != 0 {
        return None;
    }
    if i64::from_le_bytes(bytes[..8].try_into().ok()?) != fp {
        return None;
    }
    let x = if admin_me { 8 } else { 0 };
    let msg_key = &bytes[8..24];
    let (k, iv) = kdf(key, msg_key, x);
    let mut d = bytes[24..].to_vec();
    grammers_crypto::aes::ige_decrypt(&mut d, &k, &iv);
    let full = sha256(&[&key[88 + x..88 + x + 32], &d]);
    if &full[8..24] != msg_key {
        return None;
    }
    let len = i32::from_le_bytes(d[..4].try_into().ok()?);
    if len <= 0 || len as usize > d.len() - 4 {
        return None;
    }
    let padding = d.len() - 4 - len as usize;
    if !(12..=1024).contains(&padding) {
        return None;
    }
    Some(d[4..4 + len as usize].to_vec())
}

// ───────────────────────── Xabar formatlari ─────────────────────────

const C_LAYER: u32 = 0x1be31789;
const C_MSG_L73: u32 = 0x91cc4674;
const C_MSG_L45: u32 = 0x36b091de;
const C_MSG_L17: u32 = 0x204d3878;
const C_SERVICE: u32 = 0x73164160;
const C_ACT_NOTIFY: u32 = 0xf3048883;
const C_ACT_DELETE: u32 = 0x65614304;
const C_MEDIA_EMPTY: u32 = 0x089f5c4a;

fn build_text(rid: i64, text: &str, reply: Option<i64>, in_seq: i32, out_seq: i32) -> Vec<u8> {
    let mut w = W(Vec::new());
    w.u32(C_LAYER);
    w.bytes(&rand_bytes(15));
    w.i32(OUR_LAYER);
    w.i32(in_seq);
    w.i32(out_seq);
    w.u32(C_MSG_L73);
    let flags: i32 = if reply.is_some() { 1 << 3 } else { 0 };
    w.i32(flags);
    w.i64(rid);
    w.i32(0); // ttl
    w.bytes(text.as_bytes());
    if let Some(r) = reply {
        w.i64(r);
    }
    w.0
}

fn build_notify(rid: i64, in_seq: i32, out_seq: i32) -> Vec<u8> {
    let mut w = W(Vec::new());
    w.u32(C_LAYER);
    w.bytes(&rand_bytes(15));
    w.i32(OUR_LAYER);
    w.i32(in_seq);
    w.i32(out_seq);
    w.u32(C_SERVICE);
    w.i64(rid);
    w.u32(C_ACT_NOTIFY);
    w.i32(OUR_LAYER);
    w.0
}

enum Item {
    Text { rid: i64, text: String, reply: i64, media: bool },
    Delete(Vec<i64>),
    Other,
}

fn parse_message(r: &mut R) -> Option<Item> {
    let c = r.u32()?;
    match c {
        C_MSG_L73 | C_MSG_L45 => {
            let flags = r.i32()?;
            let rid = r.i64()?;
            let _ttl = r.i32()?;
            let text = String::from_utf8_lossy(&r.bytes()?).into_owned();
            let media = flags & (1 << 9) != 0;
            let entities = flags & (1 << 7) != 0;
            let via = flags & (1 << 11) != 0;
            let mut reply = 0;
            // Media/entities/via_bot ni tahlil qilib bo'lmaydi — faqat ular bo'lmasa javob maydoni o'qiladi.
            if !media && !entities && !via && flags & (1 << 3) != 0 {
                reply = r.i64().unwrap_or(0);
            }
            Some(Item::Text { rid, text, reply, media })
        }
        C_MSG_L17 => {
            let rid = r.i64()?;
            let _ttl = r.i32()?;
            let text = String::from_utf8_lossy(&r.bytes()?).into_owned();
            let media = r.u32().map(|m| m != C_MEDIA_EMPTY).unwrap_or(false);
            Some(Item::Text { rid, text, reply: 0, media })
        }
        C_SERVICE => {
            let _rid = r.i64()?;
            match r.u32()? {
                C_ACT_DELETE => {
                    let _vec = r.u32()?;
                    let n = r.i32()?.clamp(0, 1000);
                    let mut ids = Vec::new();
                    for _ in 0..n {
                        ids.push(r.i64()?);
                    }
                    Some(Item::Delete(ids))
                }
                _ => Some(Item::Other),
            }
        }
        _ => Some(Item::Other),
    }
}

/// `(in_seq, out_seq, mazmun)`; qatlamsiz eski formatlar `None`.
fn parse_layer(obj: &[u8]) -> Option<(i32, i32, Item)> {
    let mut r = R { b: obj, p: 0 };
    if r.u32()? != C_LAYER {
        return None;
    }
    let rb = r.bytes()?;
    if rb.len() < 15 {
        return None;
    }
    let _layer = r.i32()?;
    let in_seq = r.i32()?;
    let out_seq = r.i32()?;
    let item = parse_message(&mut r)?;
    Some((in_seq, out_seq, item))
}

// ───────────────────────── Chat holati yordamchilari ─────────────────────────

fn chat_key(eid: i32) -> String {
    eid.to_string()
}

fn push_msg(chat: &mut Value, rid: i64, out: bool, date: i64, text: &str, reply: i64) {
    let id = chat["next"].as_i64().unwrap_or(1);
    chat["next"] = json!(id + 1);
    if !chat["msgs"].is_array() {
        chat["msgs"] = json!([]);
    }
    if let Some(a) = chat["msgs"].as_array_mut() {
        a.push(json!({"id": id, "rid": rid.to_string(), "out": out, "date": date, "text": text, "reply": reply.to_string()}));
        if a.len() > MAX_MSGS {
            let drop = a.len() - MAX_MSGS;
            a.drain(..drop);
        }
    }
}

fn has_rid(chat: &Value, rid: i64) -> bool {
    let s = rid.to_string();
    chat["msgs"].as_array().map(|a| a.iter().any(|m| m["rid"].as_str() == Some(&s))).unwrap_or(false)
}

/// Kalit (256 bayt) va barmoq izi.
fn chat_key_bytes(chat: &Value) -> Option<(Vec<u8>, i64)> {
    let key = hex::decode(chat["key"].as_str()?).ok()?;
    if key.len() != 256 {
        return None;
    }
    let fp = chat["fp"].as_str()?.parse::<i64>().ok()?;
    Some((key, fp))
}

/// Qabul qilingan shifrlangan baytlarni ochib chatga yozadi. `true` — o'zgarish bo'ldi.
fn ingest(eid: i32, date: i64, bytes: &[u8]) -> bool {
    with(|s| {
        let Some(chat) = s["chats"].get_mut(chat_key(eid)) else { return false };
        if chat["state"].as_str() != Some("ready") {
            return false;
        }
        let Some((key, fp)) = chat_key_bytes(chat) else { return false };
        let admin_me = chat["admin_me"].as_bool().unwrap_or(false);
        let Some(obj) = decrypt(&key, fp, admin_me, bytes) else { return false };
        let Some((_in_seq, out_seq, item)) = parse_layer(&obj) else { return false };
        let seq_in = chat["seq_in"].as_i64().unwrap_or(0) as i32;
        // Takroriy (allaqachon olingan) xabar.
        if out_seq <= seq_in {
            return false;
        }
        chat["seq_in"] = json!(out_seq);
        match item {
            Item::Text { rid, text, reply, media } => {
                if has_rid(chat, rid) {
                    return true;
                }
                let body = if text.is_empty() && media { "[Media — maxfiy chatda qo‘llab-quvvatlanmaydi]".to_string() } else if media { format!("{text}\n[Media]") } else { text };
                push_msg(chat, rid, false, date, &body, reply);
            }
            Item::Delete(ids) => {
                let set: HashSet<String> = ids.iter().map(|x| x.to_string()).collect();
                if let Some(a) = chat["msgs"].as_array_mut() {
                    a.retain(|m| !m["rid"].as_str().map(|r| set.contains(r)).unwrap_or(false));
                }
            }
            Item::Other => {}
        }
        true
    })
}

// ───────────────────────── Diffie-Hellman ─────────────────────────

/// `(p, g, server_random)`; p va g xotirada saqlanadi (keyingi so'rovda `version`).
async fn dh_config(client: &Client) -> Result<(BigUint, i32, Vec<u8>), String> {
    let version = peek(|s| s["dh"]["version"].as_i64().unwrap_or(0)) as i32;
    let res = client
        .invoke(&tl::functions::messages::GetDhConfig { version, random_length: 256 })
        .await
        .map_err(|e| e.to_string())?;
    match res {
        tl::enums::messages::DhConfig::Config(c) => {
            if !(2..=7).contains(&c.g) || !grammers_crypto::two_factor_auth::check_p_and_g(&c.p, &c.g) {
                return Err("Telegram yuborgan DH parametrlari yaroqsiz".to_string());
            }
            with(|s| s["dh"] = json!({"p": hex::encode(&c.p), "g": c.g, "version": c.version}));
            Ok((BigUint::from_bytes_be(&c.p), c.g, c.random))
        }
        tl::enums::messages::DhConfig::NotModified(n) => {
            let (p, g) = peek(|s| (s["dh"]["p"].as_str().and_then(|h| hex::decode(h).ok()), s["dh"]["g"].as_i64()));
            match (p, g) {
                (Some(p), Some(g)) => Ok((BigUint::from_bytes_be(&p), g as i32, n.random)),
                _ => Err("DH sozlamalari topilmadi".to_string()),
            }
        }
    }
}

/// Maxfiy eksponenta: 256 tasodifiy bayt, serverning tasodifiy qiymati bilan XOR.
fn exponent(server_random: &[u8]) -> Vec<u8> {
    let mut a = rand_bytes(256);
    for (i, b) in server_random.iter().take(256).enumerate() {
        a[i] ^= b;
    }
    a
}

/// Suhbatni boshlaydi. `user_id`/`hash` — suhbatdosh, `name` — ko'rsatiladigan ism.
async fn start(client: &Client, user_id: i64, hash: i64, name: &str) -> Result<Value, String> {
    let (p, g, random) = dh_config(client).await?;
    let a_bytes = exponent(&random);
    let a = BigUint::from_bytes_be(&a_bytes);
    let ga = BigUint::from(g as u32).modpow(&a, &p);
    if !good_ga(&ga, &p) {
        return Err("Kalit almashinuvi muvaffaqiyatsiz (g_a yaroqsiz) — qayta urinib ko‘ring".to_string());
    }
    let rid = i32::from_le_bytes(rand_bytes(4).try_into().unwrap_or([1, 2, 3, 4])) & 0x7fff_ffff;
    let res = client
        .invoke(&tl::functions::messages::RequestEncryption {
            user_id: tl::enums::InputUser::User(tl::types::InputUser { user_id, access_hash: hash }),
            random_id: rid,
            g_a: pad256(&ga),
        })
        .await
        .map_err(|e| e.to_string())?;
    let tl::enums::EncryptedChat::Waiting(w) = res else { return Err("Maxfiy chat ochilmadi".to_string()) };
    ensure_qts(client).await;
    with(|s| {
        s["chats"][chat_key(w.id)] = json!({
            "id": w.id, "access_hash": w.access_hash.to_string(), "user": user_id, "user_hash": hash.to_string(), "name": name,
            "state": "waiting", "admin_me": true, "a": hex::encode(&a_bytes), "date": w.date,
            "seq_in": -2, "seq_out": 1, "next": 1, "read": 0, "msgs": [],
        });
    });
    Ok(json!({"ok": true, "id": w.id}))
}

/// Kelgan so'rovni avtomatik qabul qiladi (DrKLO ham shunday qiladi).
async fn accept(client: &Client, r: &tl::types::EncryptedChatRequested, users: &[&tl::enums::User]) -> Result<(), String> {
    let (p, g, random) = dh_config(client).await?;
    let ga = BigUint::from_bytes_be(&r.g_a);
    if !good_ga(&ga, &p) {
        let _ = client.invoke(&tl::functions::messages::DiscardEncryption { delete_history: false, chat_id: r.id }).await;
        return Err("g_a yaroqsiz".to_string());
    }
    let b_bytes = exponent(&random);
    let b = BigUint::from_bytes_be(&b_bytes);
    let gb = BigUint::from(g as u32).modpow(&b, &p);
    let key = pad256(&ga.modpow(&b, &p));
    let fp = fingerprint(&key);
    client
        .invoke(&tl::functions::messages::AcceptEncryption {
            peer: tl::enums::InputEncryptedChat::Chat(tl::types::InputEncryptedChat { chat_id: r.id, access_hash: r.access_hash }),
            g_b: pad256(&gb),
            key_fingerprint: fp,
        })
        .await
        .map_err(|e| e.to_string())?;
    ensure_qts(client).await;
    let mut name = String::new();
    let mut uhash = 0i64;
    for u in users {
        if let tl::enums::User::User(u) = u {
            if u.id == r.admin_id {
                name = format!("{} {}", u.first_name.clone().unwrap_or_default(), u.last_name.clone().unwrap_or_default()).trim().to_string();
                uhash = u.access_hash.unwrap_or(0);
            }
        }
    }
    with(|s| {
        s["chats"][chat_key(r.id)] = json!({
            "id": r.id, "access_hash": r.access_hash.to_string(), "user": r.admin_id, "user_hash": uhash.to_string(), "name": name,
            "state": "ready", "admin_me": false, "key": hex::encode(&key), "fp": fp.to_string(), "date": r.date,
            "seq_in": -1, "seq_out": 0, "next": 1, "read": 0, "msgs": [],
        });
    });
    let _ = send_notify(client, r.id).await;
    Ok(())
}

/// Suhbatdosh qabul qildi: kalitni hisoblaymiz.
async fn accepted(client: &Client, c: &tl::types::EncryptedChat) -> Result<(), String> {
    let (p, _g, _) = dh_config(client).await?;
    let a_hex = peek(|s| {
        let ch = &s["chats"][chat_key(c.id)];
        if ch["state"].as_str() == Some("waiting") { ch["a"].as_str().map(|x| x.to_string()) } else { None }
    });
    let Some(a_hex) = a_hex else { return Ok(()) };
    let a = BigUint::from_bytes_be(&hex::decode(a_hex).map_err(|e| e.to_string())?);
    let gb = BigUint::from_bytes_be(&c.g_a_or_b);
    if !good_ga(&gb, &p) {
        let _ = client.invoke(&tl::functions::messages::DiscardEncryption { delete_history: false, chat_id: c.id }).await;
        with(|s| s["chats"][chat_key(c.id)]["state"] = json!("discarded"));
        return Err("g_b yaroqsiz".to_string());
    }
    let key = pad256(&gb.modpow(&a, &p));
    let fp = fingerprint(&key);
    if fp != c.key_fingerprint {
        let _ = client.invoke(&tl::functions::messages::DiscardEncryption { delete_history: false, chat_id: c.id }).await;
        with(|s| s["chats"][chat_key(c.id)]["state"] = json!("discarded"));
        return Err("kalit barmoq izi mos kelmadi".to_string());
    }
    with(|s| {
        let ch = &mut s["chats"][chat_key(c.id)];
        ch["state"] = json!("ready");
        ch["key"] = json!(hex::encode(&key));
        ch["fp"] = json!(fp.to_string());
        ch["a"] = Value::Null;
        ch["access_hash"] = json!(c.access_hash.to_string());
    });
    let _ = send_notify(client, c.id).await;
    Ok(())
}

// ───────────────────────── Yuborish ─────────────────────────

/// `(key, fp, admin_me, access_hash, in_seq, out_seq)`; seq_out 2 ga oshadi.
fn next_seq(eid: i32) -> Option<(Vec<u8>, i64, bool, i64, i32, i32)> {
    with(|s| {
        let ch = s["chats"].get_mut(chat_key(eid))?;
        if ch["state"].as_str() != Some("ready") {
            return None;
        }
        let (key, fp) = chat_key_bytes(ch)?;
        let admin_me = ch["admin_me"].as_bool().unwrap_or(false);
        let ah = ch["access_hash"].as_str()?.parse::<i64>().ok()?;
        let seq_in = ch["seq_in"].as_i64().unwrap_or(0) as i32;
        let seq_out = ch["seq_out"].as_i64().unwrap_or(0) as i32;
        let in_seq = if seq_in > 0 { seq_in } else { seq_in + 2 };
        ch["seq_out"] = json!(seq_out + 2);
        Some((key, fp, admin_me, ah, in_seq, seq_out))
    })
}

async fn send_notify(client: &Client, eid: i32) -> Result<(), String> {
    let (key, fp, admin_me, ah, in_seq, out_seq) = next_seq(eid).ok_or("Maxfiy chat tayyor emas")?;
    let rid = i64::from_le_bytes(rand_bytes(8).try_into().unwrap_or([1; 8]));
    let data = encrypt(&key, fp, admin_me, &build_notify(rid, in_seq, out_seq));
    client
        .invoke(&tl::functions::messages::SendEncryptedService {
            peer: tl::enums::InputEncryptedChat::Chat(tl::types::InputEncryptedChat { chat_id: eid, access_hash: ah }),
            random_id: rid,
            data,
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn send_text(client: &Client, eid: i32, text: &str, reply_rid: Option<i64>) -> Result<(), String> {
    let (key, fp, admin_me, ah, in_seq, out_seq) = next_seq(eid).ok_or("Maxfiy chat hali tayyor emas — suhbatdosh qabul qilishini kuting")?;
    let rid = i64::from_le_bytes(rand_bytes(8).try_into().unwrap_or([1; 8]));
    let data = encrypt(&key, fp, admin_me, &build_text(rid, text, reply_rid, in_seq, out_seq));
    client
        .invoke(&tl::functions::messages::SendEncrypted {
            silent: false,
            peer: tl::enums::InputEncryptedChat::Chat(tl::types::InputEncryptedChat { chat_id: eid, access_hash: ah }),
            random_id: rid,
            data,
        })
        .await
        .map_err(|e| e.to_string())?;
    let now = (now_ms() / 1000) as i64;
    with(|s| {
        if let Some(ch) = s["chats"].get_mut(chat_key(eid)) {
            push_msg(ch, rid, true, now, text, reply_rid.unwrap_or(0));
        }
    });
    Ok(())
}

// ───────────────────────── Yangilanishlar ─────────────────────────

/// qts bo'lmasa (birinchi maxfiy chat) hozirgisini saqlaydi: undan keyingi shifrlangan xabarlar `getDifference` bilan olinadi.
async fn ensure_qts(client: &Client) {
    if peek(|s| s["qts"].as_i64().is_some()) {
        return;
    }
    if let Ok(tl::enums::updates::State::State(st)) = client.invoke(&tl::functions::updates::GetState {}).await {
        with(|s| s["qts"] = json!(st.qts));
    }
}

async fn handle_chat(client: &Client, chat: &tl::enums::EncryptedChat, users: &[&tl::enums::User]) {
    match chat {
        tl::enums::EncryptedChat::Requested(r) => {
            let known = peek(|s| s["chats"].get(chat_key(r.id)).is_some());
            if !known {
                let _ = accept(client, r, users).await;
            }
        }
        tl::enums::EncryptedChat::Chat(c) => {
            let _ = accepted(client, c).await;
        }
        tl::enums::EncryptedChat::Waiting(w) => {
            with(|s| {
                if let Some(ch) = s["chats"].get_mut(chat_key(w.id)) {
                    ch["access_hash"] = json!(w.access_hash.to_string());
                }
            });
        }
        tl::enums::EncryptedChat::Discarded(d) => {
            with(|s| {
                if let Some(ch) = s["chats"].get_mut(chat_key(d.id)) {
                    ch["state"] = json!("discarded");
                }
            });
        }
        _ => {}
    }
}

fn handle_message(m: &tl::enums::EncryptedMessage) -> bool {
    match m {
        tl::enums::EncryptedMessage::Message(m) => ingest(m.chat_id, m.date as i64, &m.bytes),
        tl::enums::EncryptedMessage::Service(m) => ingest(m.chat_id, m.date as i64, &m.bytes),
    }
}

/// Ulanish ichidagi yangilanishlar tsikli chaqiradi.
pub(super) async fn on_updates(client: &Client, up: &tl::enums::Updates) {
    let (list, users): (Vec<&tl::enums::Update>, Vec<&tl::enums::User>) = match up {
        tl::enums::Updates::UpdateShort(s) => (vec![&s.update], Vec::new()),
        tl::enums::Updates::Updates(s) => (s.updates.iter().collect(), s.users.iter().collect()),
        tl::enums::Updates::Combined(s) => (s.updates.iter().collect(), s.users.iter().collect()),
        _ => return,
    };
    for u in list {
        match u {
            tl::enums::Update::Encryption(e) => handle_chat(client, &e.chat, &users).await,
            tl::enums::Update::NewEncryptedMessage(m) => {
                handle_message(&m.message);
                with(|s| {
                    let old = s["qts"].as_i64().unwrap_or(0);
                    if (m.qts as i64) > old {
                        s["qts"] = json!(m.qts);
                    }
                });
                let _ = client.invoke(&tl::functions::messages::ReceivedQueue { max_qts: m.qts }).await;
            }
            _ => {}
        }
    }
}

static LAST_SYNC: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

/// Ilova yopiq paytda kelgan shifrlangan xabarlarni oladi (`updates.getDifference`, 20 soniyada bir martadan ko'p emas).
async fn catch_up(client: &Client) {
    if peek(|s| s["chats"].as_object().map(|o| o.is_empty()).unwrap_or(true)) {
        return;
    }
    let now = now_ms();
    if now - LAST_SYNC.load(Ordering::SeqCst) < 20_000 {
        return;
    }
    LAST_SYNC.store(now, Ordering::SeqCst);
    let Ok(tl::enums::updates::State::State(st)) = client.invoke(&tl::functions::updates::GetState {}).await else { return };
    let Some(qts) = peek(|s| s["qts"].as_i64()) else {
        with(|s| s["qts"] = json!(st.qts));
        return;
    };
    let res = client
        .invoke(&tl::functions::updates::GetDifference {
            pts: st.pts,
            pts_limit: None,
            pts_total_limit: None,
            date: st.date,
            qts: qts as i32,
            qts_limit: None,
        })
        .await;
    let (msgs, new_qts) = match res {
        Ok(tl::enums::updates::Difference::Difference(d)) => {
            let tl::enums::updates::State::State(ns) = d.state;
            (d.new_encrypted_messages, Some(ns.qts))
        }
        Ok(tl::enums::updates::Difference::Slice(d)) => {
            let tl::enums::updates::State::State(ns) = d.intermediate_state;
            (d.new_encrypted_messages, Some(ns.qts))
        }
        _ => (Vec::new(), None),
    };
    for m in &msgs {
        handle_message(m);
    }
    if let Some(q) = new_qts {
        with(|s| s["qts"] = json!(q));
        let _ = client.invoke(&tl::functions::messages::ReceivedQueue { max_qts: q }).await;
    }
}

// ───────────────────────── FFI ─────────────────────────

fn chat_summary(ch: &Value) -> Value {
    let msgs = ch["msgs"].as_array();
    let last = msgs.and_then(|a| a.last());
    let read = ch["read"].as_i64().unwrap_or(0);
    let unread = msgs.map(|a| a.iter().filter(|m| !m["out"].as_bool().unwrap_or(false) && m["id"].as_i64().unwrap_or(0) > read).count()).unwrap_or(0);
    json!({
        "id": ch["id"],
        "user": ch["user"],
        "hash": ch["user_hash"],
        "name": ch["name"],
        "state": ch["state"],
        "text": last.map(|m| m["text"].clone()).unwrap_or(json!("")),
        "date": last.map(|m| m["date"].clone()).unwrap_or(ch["date"].clone()),
        "out": last.map(|m| m["out"].clone()).unwrap_or(json!(false)),
        "unread": unread,
        "top": last.map(|m| m["id"].clone()).unwrap_or(json!(0)),
    })
}

/// Maxfiy chatlar ro'yxati: `{"chats":[{"id","user","hash","name","state","text","date","out","unread","top"}]}`.
#[no_mangle]
pub extern "C" fn rust_tg_secret_list() -> *mut c_char {
    tg_call(std::ptr::null(), 40, |client, _| async move {
        catch_up(&client).await;
        let chats = peek(|s| {
            s["chats"]
                .as_object()
                .map(|o| o.values().filter(|c| c["state"].as_str() != Some("discarded")).map(chat_summary).collect::<Vec<_>>())
                .unwrap_or_default()
        });
        Ok(json!({"chats": chats}))
    })
}

/// Xabarlar (eng yangisi birinchi): `arg` = `"<chat_id>"`. Javob `{"messages":[{"id","out","date","text","reply"}],"state"}`.
#[no_mangle]
pub extern "C" fn rust_tg_secret_messages(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 40, |client, arg| async move {
        let eid = arg.trim().parse::<i32>().map_err(|_| "Chat ko'rsatilmagan".to_string())?;
        catch_up(&client).await;
        let v = peek(|s| {
            let ch = &s["chats"][chat_key(eid)];
            if ch.is_null() {
                return None;
            }
            let mut ms: Vec<Value> = ch["msgs"].as_array().cloned().unwrap_or_default();
            // `reply` — suhbatdosh yuborgan random_id; mahalliy xabar raqamiga aylantiramiz.
            let by_rid: HashMap<String, i64> = ms.iter().filter_map(|m| Some((m["rid"].as_str()?.to_string(), m["id"].as_i64()?))).collect();
            for m in ms.iter_mut() {
                let r = m["reply"].as_str().unwrap_or("0").to_string();
                m["reply"] = json!(by_rid.get(&r).copied().unwrap_or(0));
            }
            ms.reverse();
            Some(json!({"messages": ms, "state": ch["state"]}))
        });
        v.ok_or_else(|| "Maxfiy chat topilmadi".to_string())
    })
}

/// Yuborish: `arg` = `"<chat_id>\n<javob_xabar_id|0>\n<matn>"`.
#[no_mangle]
pub extern "C" fn rust_tg_secret_send(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 40, |client, arg| async move {
        let l = lines(&arg, 3);
        let eid = l[0].trim().parse::<i32>().map_err(|_| "Chat ko'rsatilmagan".to_string())?;
        let reply_id = l[1].trim().parse::<i64>().unwrap_or(0);
        let text = l[2].clone();
        if text.trim().is_empty() {
            return Err("Xabar bo'sh".to_string());
        }
        let reply_rid = if reply_id > 0 {
            peek(|s| {
                s["chats"][chat_key(eid)]["msgs"]
                    .as_array()
                    .and_then(|a| a.iter().find(|m| m["id"].as_i64() == Some(reply_id)))
                    .and_then(|m| m["rid"].as_str().and_then(|r| r.parse::<i64>().ok()))
            })
        } else {
            None
        };
        send_text(&client, eid, &text, reply_rid).await?;
        Ok(json!({"ok": true}))
    })
}

/// Yangi maxfiy chat: `arg` = `"<user_id>@<hash>\n<ism>"`.
#[no_mangle]
pub extern "C" fn rust_tg_secret_start(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 60, |client, arg| async move {
        let l = lines(&arg, 2);
        let (id, hash) = l[0].trim().split_once('@').ok_or("Foydalanuvchi ko'rsatilmagan")?;
        let user_id = id.parse::<i64>().map_err(|_| "Foydalanuvchi ko'rsatilmagan".to_string())?;
        let hash = hash.parse::<i64>().map_err(|_| "Foydalanuvchi ko'rsatilmagan".to_string())?;
        start(&client, user_id, hash, l[1].trim()).await
    })
}

/// Maxfiy chatni tugatish: `arg` = `"<chat_id>"` (serverga bildiriladi, mahalliy tarix o'chadi).
#[no_mangle]
pub extern "C" fn rust_tg_secret_discard(arg_ptr: *const c_char) -> *mut c_char {
    tg_call(arg_ptr, 30, |client, arg| async move {
        let eid = arg.trim().parse::<i32>().map_err(|_| "Chat ko'rsatilmagan".to_string())?;
        let _ = client.invoke(&tl::functions::messages::DiscardEncryption { delete_history: true, chat_id: eid }).await;
        with(|s| {
            if let Some(o) = s["chats"].as_object_mut() {
                o.remove(&chat_key(eid));
            }
        });
        Ok(json!({"ok": true}))
    })
}

/// O'qilgan deb belgilash: `arg` = `"<chat_id>"` (tarmoqsiz).
#[no_mangle]
pub extern "C" fn rust_tg_secret_read(arg_ptr: *const c_char) -> *mut c_char {
    let eid = unsafe { cstr_to_str(arg_ptr) }.unwrap_or("").trim().parse::<i32>().unwrap_or(0);
    with(|s| {
        if let Some(ch) = s["chats"].get_mut(chat_key(eid)) {
            let next = ch["next"].as_i64().unwrap_or(1);
            ch["read"] = json!(next);
        }
    });
    string_to_cptr(json!({"ok": true}).to_string())
}
