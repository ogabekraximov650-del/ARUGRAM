//! Yordam chati (Kotlin `SupportChatScreen`): admin bilan yozishma. Matn, fayl/rasm yuborish, nusxalash, o'chirish, jonli yangilanish.
//! `user_id = Some(..)` — admin boshqa foydalanuvchi suhbatini ochgan.

use super::*;
use crate::api;
use crate::data::fmt_date;
use serde_json::{json, Value};

const S_SEND: &str = "M2.01,21L23,12 2.01,3 2,10l15,2 -15,2z";
const S_ATTACH: &str = "M16.5,6v11.5c0,2.21 -1.79,4 -4,4s-4,-1.79 -4,-4V5c0,-1.38 1.12,-2.5 2.5,-2.5s2.5,1.12 2.5,2.5v10.5c0,0.55 -0.45,1 -1,1s-1,-0.45 -1,-1V6H10v9.5c0,1.38 1.12,2.5 2.5,2.5s2.5,-1.12 2.5,-2.5V5c0,-2.21 -1.79,-4 -4,-4S7,2.79 7,5v12.5c0,3.04 2.46,5.5 5.5,5.5s5.5,-2.46 5.5,-5.5V6h-1.5z";
const S_COPY: &str = "M16,1H4c-1.1,0 -2,0.9 -2,2v14h2V3h12V1zM19,5H8c-1.1,0 -2,0.9 -2,2v14c0,1.1 0.9,2 2,2h11c1.1,0 2,-0.9 2,-2V7c0,-1.1 -0.9,-2 -2,-2zM19,21H8V7h11v14z";
const S_TRASH: &str = "M6,19c0,1.1 0.9,2 2,2h8c1.1,0 2,-0.9 2,-2V7H6v12zM19,4h-3.5l-1,-1h-5l-1,1H5v2h14V4z";
const S_PLAY: &str = "M8,5v14l11,-7z";
const S_REPLY: &str = "M10,9V5l-7,7 7,7v-4.1c5,0 8.5,1.6 11,5.1 -1,-5 -4,-10 -11,-11z";
const S_CLOSE: &str = "M19,6.41L17.59,5 12,10.59 6.41,5 5,6.41 10.59,12 5,17.59 6.41,19 12,13.41 17.59,19 19,17.59 13.41,12z";
const TG_PILL: Color = Color::rgba(36, 31, 28, 240);
const TG_IN: Color = Color::rgba(42, 36, 32, 255);
const TG_OUT: Color = Color::rgba(122, 74, 42, 255);
const TG_OUT_META: Color = Color::rgba(226, 190, 156, 255);
const S_BACK: &str = "M20,11H7.83l5.59,-5.59L12,4l-8,8 8,8 1.41,-1.41L7.83,13H20v-2z";
const S_EMOJI: &str = "M11.99,2C6.47,2 2,6.48 2,12s4.47,10 9.99,10C17.52,22 22,17.52 22,12S17.52,2 11.99,2zM12,20c-4.42,0 -8,-3.58 -8,-8s3.58,-8 8,-8 8,3.58 8,8 -3.58,8 -8,8zM15.5,11c0.83,0 1.5,-0.67 1.5,-1.5S16.33,8 15.5,8 14,8.67 14,9.5s0.67,1.5 1.5,1.5zM8.5,11c0.83,0 1.5,-0.67 1.5,-1.5S9.33,8 8.5,8 7,8.67 7,9.5 7.67,11 8.5,11zM12,17.5c2.33,0 4.31,-1.46 5.11,-3.5L6.89,14c0.8,2.04 2.78,3.5 5.11,3.5z";
const S_KEYBOARD: &str = "M20,5L4,5c-1.1,0 -1.99,0.9 -1.99,2L2,17c0,1.1 0.9,2 2,2h16c1.1,0 2,-0.9 2,-2L22,7c0,-1.1 -0.9,-2 -2,-2zM11,8h2v2h-2L11,8zM11,11h2v2h-2v-2zM8,8h2v2L8,10L8,8zM8,11h2v2L8,13v-2zM7,13L5,13v-2h2v2zM7,10L5,10L5,8h2v2zM8,16h8v1L8,17v-1zM16,13h-2v-2h2v2zM16,10h-2L14,8h2v2zM19,13h-2v-2h2v2zM19,10h-2L17,8h2v2z";
const S_CHECK: &str = "M9,16.17L4.83,12l-1.42,1.41L9,19 21,7l-1.41,-1.41z";

#[derive(Clone)]
pub struct ChatMsg {
    id: String,
    from_admin: bool,
    body: String,
    created_at: i64,
    media_url: String,
    media_type: String,
    media_ms: i64,
    pending: bool,
    seen: bool,
}

pub struct SupportPage {
    pub(super) user_id: Option<i64>,
    items: State<Rc<Vec<ChatMsg>>>,
    input: State<String>,
    err: State<String>,
    loaded: State<bool>,
    busy: State<bool>,
    /// Amallar menyusi ochilgan xabar identifikatori.
    menu: State<String>,
    /// Javob berilayotgan xabar identifikatori.
    reply: State<Option<String>>,
    /// Emoji paneli.
    ep: Rc<super::tgpanel::TgPanel>,
    scroll: ScrollState,
    stick: std::cell::Cell<bool>,
    ver: std::cell::Cell<i64>,
}

fn parse_msg(o: &Value) -> ChatMsg {
    let s = |k: &str| o[k].as_str().filter(|v| *v != "null").unwrap_or("").to_string();
    ChatMsg {
        id: s("id"),
        from_admin: o["from_admin"].as_bool().unwrap_or(false),
        body: s("body"),
        created_at: o["created_at"].as_i64().unwrap_or(0),
        media_url: s("media_url"),
        media_type: s("media_type"),
        media_ms: o["media_ms"].as_i64().unwrap_or(0),
        pending: false,
        seen: o["seen"].as_bool().unwrap_or(false),
    }
}

/// Suhbat nusxasi diskda shifrlangan saqlanadi (Kotlin kabi): chat darhol ochiladi, internet kelgach yangilanadi.
fn chat_cache_path(user_id: Option<i64>) -> String {
    format!("{}/chat_{}.json", crate::data::cache_dir(), user_id.unwrap_or(0))
}

fn chat_cache_save(user_id: Option<i64>, rows: &[ChatMsg]) {
    let v: Vec<Value> = rows.iter().filter(|m| !m.pending).map(|m| json!({
        "id": m.id, "from_admin": m.from_admin, "body": m.body, "created_at": m.created_at,
        "media_url": m.media_url, "media_type": m.media_type, "media_ms": m.media_ms, "seen": m.seen,
    })).collect();
    let _ = crate::secure::write_text(&chat_cache_path(user_id), "chat", &Value::Array(v).to_string());
}

fn chat_cache_load(user_id: Option<i64>) -> Vec<ChatMsg> {
    crate::secure::read_text(&chat_cache_path(user_id), "chat")
        .and_then(|t| serde_json::from_str::<Vec<Value>>(&t).ok())
        .map(|a| a.iter().map(parse_msg).collect())
        .unwrap_or_default()
}

fn media_kind(path: &str) -> (&'static str, &'static str) {
    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => ("image", "image/jpeg"),
        "png" => ("image", "image/png"),
        "webp" => ("image", "image/webp"),
        "mp4" | "mov" | "mkv" | "webm" => ("video", "video/mp4"),
        _ => ("file", "application/octet-stream"),
    }
}

/// Javob: xabar matni boshida `[re:<id>]`.
fn split_reply(body: &str) -> (Option<String>, String) {
    if let Some(rest) = body.strip_prefix("[re:") {
        if let Some(end) = rest.find(']') {
            let id = &rest[..end];
            if !id.is_empty() && id.len() <= 40 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                return (Some(id.to_string()), rest[end + 1..].to_string());
            }
        }
    }
    (None, body.to_string())
}

/// Paket tokenlari (`[pe:..:..:X]`, `[ce:..:..:X]`) o'rniga ularning zaxira belgisi `X` qo'yiladi.
fn plain(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("[pe:").into_iter().chain(rest.find("[ce:")).min() {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        match tail.find(']') {
            Some(end) => {
                let tok = &tail[1..end];
                out.push_str(tok.rsplit(':').next().unwrap_or(""));
                rest = &tail[end + 1..];
            }
            None => {
                out.push_str(tail);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

fn snippet(m: &ChatMsg) -> String {
    let body = crate::tgmedia::plain_text(&plain(&split_reply(&m.body).1));
    if !m.media_type.is_empty() {
        let label = match m.media_type.as_str() {
            "voice" => "Ovozli xabar",
            "round" => "Video xabar",
            "sticker" => "Stiker",
            "gif" => "GIF",
            "image" => "Rasm",
            "video" => "Video",
            _ => "Fayl",
        };
        return if body.is_empty() { label.to_string() } else { format!("{label} · {body}") };
    }
    body
}

/// "Bugun" / "Kecha" / "12-sentyabr" (UTC+5).
fn day_label(ms: i64) -> String {
    const MONTHS: [&str; 12] = ["yanvar", "fevral", "mart", "aprel", "may", "iyun", "iyul", "avgust", "sentyabr", "oktyabr", "noyabr", "dekabr"];
    let day = |t: i64| (t / 1000 + 5 * 3600).div_euclid(86400);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    let diff = day(now) - day(ms);
    if diff == 0 {
        return "Bugun".into();
    }
    if diff == 1 {
        return "Kecha".into();
    }
    let ymd = crate::data::fmt_ymd(ms);
    let p: Vec<&str> = ymd.split('.').collect();
    if p.len() < 3 {
        return ymd;
    }
    let (d, m) = (p[0].trim_start_matches('0'), p[1].parse::<usize>().unwrap_or(1).clamp(1, 12));
    let cur = crate::data::fmt_ymd(now);
    let same_year = cur.rsplit('.').next() == p.get(2).copied();
    if same_year { format!("{d}-{}", MONTHS[m - 1]) } else { format!("{d}-{}, {}", MONTHS[m - 1], p[2]) }
}

/// Sarlavha sig'masa oxiriga "…" qo'yadi (Kotlin `maxLines = 1, overflow = Ellipsis`): kapsula balandligi qat'iy, matn o'ralmasin.
fn ellipsize(s: &str, size: f32, max_w: f32) -> String {
    let max_chars = (max_w / (size * 0.54)).floor().max(3.0) as usize;
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut o: String = s.chars().take(max_chars.saturating_sub(1)).collect();
    o.push('\u{2026}');
    o
}

fn url_hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

impl Home {
    /// Chatdagi video uchun boshidagi kadr (Kotlin `ChatVideoFrame`): diskdagi shifrlangan nusxa yoki fonda yasaladi.
    /// Xato bo'lsa sababi qaytadi va 15 soniyadan keyin (3 martagacha) qayta uriniladi.
    fn chat_video_frame(self: &Rc<Self>, url: &str) -> (Option<Rc<Image>>, Option<String>) {
        if url.is_empty() {
            return (None, None);
        }
        let key = format!("chat{:016x}_100", url_hash(url));
        let _ = self.gate_rev.get();
        if let Some(v) = self.hframes.borrow().get(&key) {
            return (v.clone(), None);
        }
        let tries = match self.hframe_fail.borrow().get(&key) {
            Some((why, at, n)) if *n >= 3 || at.elapsed().as_secs() < 15 => return (None, Some(why.clone())),
            Some((_, _, n)) => *n,
            None => 0,
        };
        self.hframes.borrow_mut().insert(key.clone(), None);
        let (me, session, u) = (self.clone(), self.session.clone(), url.to_string());
        spawn_bg(move || crate::downloads::history_frame_r(&session, &u, "", 100, &key).map(|i| (key.clone(), i)).map_err(|e| (key, e)), move |r| {
            match r {
                Ok((k, img)) => {
                    me.hframe_fail.borrow_mut().remove(&k);
                    me.hframes.borrow_mut().insert(k, Some(Rc::new(img)));
                }
                Err((k, why)) => {
                    me.hframes.borrow_mut().remove(&k);
                    me.hframe_fail.borrow_mut().insert(k, (why, std::time::Instant::now(), tries + 1));
                    let me2 = me.clone();
                    spawn_bg(|| std::thread::sleep(std::time::Duration::from_secs(16)), move |_| me2.gate_rev.set(me2.gate_rev.get() + 1));
                }
            }
            me.gate_rev.set(me.gate_rev.get() + 1);
        });
        (None, None)
    }

    fn chat_url(user_id: Option<i64>) -> String {
        match user_id {
            None => "/api/chat".to_string(),
            Some(u) => format!("/api/chat/thread/{u}"),
        }
    }

    pub(super) fn open_support(self: &Rc<Self>, user_id: Option<i64>) {
        let cached = chat_cache_load(user_id);
        let have_cache = !cached.is_empty();
        let p = Rc::new(SupportPage {
            user_id,
            items: State::new(Rc::new(cached)),
            input: State::new(String::new()),
            err: State::new(String::new()),
            loaded: State::new(have_cache),
            busy: State::new(false),
            menu: State::new(String::new()),
            reply: State::new(None),
            ep: Rc::new(super::tgpanel::TgPanel::new()),
            scroll: ScrollState::new(),
            stick: std::cell::Cell::new(true),
            ver: std::cell::Cell::new(0),
        });
        self.chat_load(&p);
        self.chat_watch(&p);
        self.push(pages::Page::Support(p));
    }

    fn chat_open(&self, p: &Rc<SupportPage>) -> bool {
        matches!(self.pages.get().last(), Some(pages::Page::Support(q)) if Rc::ptr_eq(q, p))
    }

    fn chat_load(self: &Rc<Self>, p: &Rc<SupportPage>) {
        let (p2, me, session) = (p.clone(), self.clone(), self.session.clone());
        let url = Self::chat_url(p.user_id);
        spawn_bg(move || api::get_json(&url, "", Some(&session)), move |r| match r {
            Ok(j) => {
                let rows: Vec<ChatMsg> = j["items"].as_array().map(|a| a.iter().map(parse_msg).collect()).unwrap_or_default();
                // Rasm/video xabarlarining fayllari poster oqimi orqali (Telegram -> kesh) olinadi.
                {
                    let have = me.posters.borrow();
                    let mut q = me.queue.borrow_mut();
                    for m in rows.iter().filter(|m| m.media_type == "image" && !m.media_url.is_empty()) {
                        if !have.contains_key(&m.media_url) && !q.contains(&m.media_url) {
                            q.push(m.media_url.clone());
                        }
                    }
                }
                chat_cache_save(p2.user_id, &rows);
                p2.items.set(Rc::new(rows));
                p2.loaded.set(true);
                p2.err.set(String::new());
                p2.stick.set(true);
                me.next_posters();
                if p2.user_id.is_none() {
                    me.unread.set(0);
                }
            }
            Err(e) => {
                if !p2.loaded.get() {
                    p2.err.set(e);
                }
            }
        });
    }

    /// `/api/chat/wait`: server ~19 soniya ushlab turadi va suhbat o'zgarganda `new` qaytaradi.
    fn chat_watch(self: &Rc<Self>, p: &Rc<SupportPage>) {
        let (me, p2, session) = (self.clone(), p.clone(), self.session.clone());
        let q = {
            let last = p.items.get().iter().filter(|m| !m.pending).map(|m| m.created_at).max().unwrap_or(0);
            format!("?ver={}&mk=-1&at=0&since={last}{}", p.ver.get(), p.user_id.map(|u| format!("&user_id={u}")).unwrap_or_default())
        };
        spawn_bg(
            move || api::get_json("/api/chat/wait", &q, Some(&session)).map_err(|_| std::thread::sleep(std::time::Duration::from_secs(3))),
            move |r| {
                if !me.chat_open(&p2) {
                    return;
                }
                if let Ok(j) = r {
                    if let Some(v) = j["ver"].as_i64() {
                        p2.ver.set(v);
                    }
                    if j["new"].as_bool().unwrap_or(false) {
                        me.chat_load(&p2);
                    }
                }
                me.chat_watch(&p2);
            },
        );
    }

    fn chat_send(self: &Rc<Self>, p: &Rc<SupportPage>, body: String, media: Option<(String, String)>) {
        let text = body.trim().to_string();
        if text.is_empty() && media.is_none() {
            return;
        }
        let shown = text.clone();
        let text = match p.reply.get() {
            Some(id) => format!("[re:{id}]{text}"),
            None => text,
        };
        p.reply.set(None);
        if media.is_none() {
            let mut v: Vec<ChatMsg> = (*p.items.get()).clone();
            v.push(ChatMsg { id: format!("tmp{}", v.len()), from_admin: p.user_id.is_some(), body: shown, created_at: 0, media_url: String::new(), media_type: String::new(), media_ms: 0, pending: true, seen: false });
            p.items.set(Rc::new(v));
            p.stick.set(true);
        }
        let mut b = json!({"body": text});
        if let Some(u) = p.user_id {
            b["user_id"] = json!(u);
        }
        if let Some((f, t)) = &media {
            b["media_file"] = json!(f);
            b["media_type"] = json!(t);
        }
        let (p2, me, session) = (p.clone(), self.clone(), self.session.clone());
        spawn_bg(
            move || {
                let (code, text) = api::request("POST", "/api/chat", "", Some(&b), Some(&session)).map_err(|_| "Internet yo'q — qaytadan urinib ko'ring".to_string())?;
                if code >= 400 {
                    let j: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                    return Err(j["error"].as_str().map(String::from).unwrap_or_else(|| "Yuborilmadi".into()));
                }
                Ok(())
            },
            move |r: Result<(), String>| {
                p2.busy.set(false);
                if let Err(e) = r {
                    p2.err.set(e);
                    let v: Vec<ChatMsg> = p2.items.get().iter().filter(|m| !m.pending).cloned().collect();
                    p2.items.set(Rc::new(v));
                }
                me.chat_load(&p2);
            },
        );
    }

    fn chat_attach(self: &Rc<Self>, p: &Rc<SupportPage>) {
        let (me, pc) = (self.clone(), p.clone());
        self.pick("*/*", move |path| {
            let (kind, mime) = media_kind(&path);
            let ext = path.rsplit('.').next().unwrap_or("bin").to_lowercase();
            let uid = me.profile.get().id;
            let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
            let name = format!("chat_{uid}_{ts}.{ext}");
            let disp = path.rsplit('/').next().unwrap_or("fayl").to_string();
            pc.busy.set(true);
            pc.err.set(String::new());
            let (p2, me2, session, name2) = (pc.clone(), me.clone(), me.session.clone(), name.clone());
            spawn_bg(move || crate::backend::upload_file(&path, &name2, mime, &session, &|_, _| {}), move |r| match r {
                Ok(()) => me2.chat_send(&p2, if kind == "file" { disp } else { String::new() }, Some((name, kind.to_string()))),
                Err(e) => {
                    p2.busy.set(false);
                    p2.err.set(e);
                }
            });
        });
    }

    pub(super) fn support_page(self: &Rc<Self>, p: &Rc<SupportPage>) -> Element {
        let items = p.items.get();
        let (w, _) = window_dp();
        // Pastga yopishish: ko'rinish balandligi ma'lum bo'lgach oxirgi xabarga suriladi.
        if p.stick.get() && p.scroll.max_offset() > 0.0 {
            if (p.scroll.offset() - p.scroll.max_offset()).abs() > 1.0 {
                p.scroll.set_offset(p.scroll.max_offset());
            } else {
                p.stick.set(false);
            }
        }
        // Xabarlar to'liq ekranga yoyiladi: sarlavha va pastki panel ularning USTIDA shisha qatlam, xabarlar ularning ostidan o'tadi.
        let ins = safe_insets();
        let header_h = 12.0 + ins.top + 56.0 + 8.0;
        let bar_h = 62.0
            + if p.reply.get().is_some() { 52.0 } else { 0.0 }
            + if super::richtext::needs_rich(&crate::tgmedia::plain_text(&p.input.get())) { 42.0 } else { 0.0 }
            + if p.ep.open.get() { super::tgpanel::PANEL_H } else { 0.0 }
            + if p.err.get().is_empty() { 0.0 } else { 26.0 };
        let mut kids: Vec<Element> = Vec::new();
        let start = items.len().saturating_sub(120);
        let visible = &items[start..];
        super::tgcache::chat_anim_mark(visible.iter().rev().filter(|m| m.media_type == "sticker").map(|m| m.id.clone()), 4);
        let same_day = |a: i64, b: i64| crate::data::fmt_ymd(a) == crate::data::fmt_ymd(b);
        for (i, m) in visible.iter().enumerate() {
            let prev = if i > 0 { visible.get(i - 1) } else { None };
            let next = visible.get(i + 1);
            let new_day = prev.map_or(true, |q| !same_day(q.created_at, m.created_at));
            let last_of_group = next.map_or(true, |q| q.from_admin != m.from_admin || !same_day(q.created_at, m.created_at));
            let top_near = !new_day && prev.map_or(false, |q| q.from_admin == m.from_admin);
            if new_day && m.created_at > 0 {
                kids.push(row(vec![spacer(), tx(day_label(m.created_at), 13.0, Color::WHITE).bold().bg(Color::rgba(0, 0, 0, 90)).radius(14.0).padding_xy(12.0, 4.0), spacer()]).padding_xy(0.0, 8.0));
            }
            kids.push(self.chat_bubble(p, m, w, top_near, !last_of_group));
        }
        kids.push(gap_box(0.0, 8.0));
        let body: Element = if !p.loaded.get() {
            if p.err.get().is_empty() { column(vec![spinner(32.0, 3.0, ACCENT2)]).center() } else { column(vec![tx(p.err.get(), 14.0, ERROR)]).center() }
        } else if items.is_empty() {
            Self::empty("M21.99,4c0,-1.1 -0.89,-2 -1.99,-2H4c-1.1,0 -2,0.9 -2,2v12c0,1.1 0.9,2 2,2h14l4,4 -0.01,-18z", "Hozircha xabar yo'q\nSavol yoki taklifingizni yozing")
        } else {
            scroll_column(&p.scroll, kids).padding_each(12.0, 4.0 + header_h, 12.0, 4.0 + bar_h + ins.bottom)
        };
        // Pastdagi yozish paneli: shisha (lupa) — tepadagi sarlavha bilan bir xil uslubda.
        let (pa, ps) = (p.clone(), p.clone());
        let me = self.clone();
        let me2 = self.clone();
        let can_send = !p.input.get().trim().is_empty() && !p.busy.get();
        let input = field(&p.input).color(Color::WHITE).size(15.0).hint("Xabar").hint_color(Color::rgba(255, 255, 255, 96)).accent(ACCENT2).padding_xy(4.0, 14.0).no_border().weight(1.0);
        let pe = p.ep.clone();
        let emo = column(vec![icon(if p.ep.open.get() { S_KEYBOARD } else { S_EMOJI }, 24.0).color(Color::rgba(255, 255, 255, 170))])
            .center()
            .width(46.0)
            .height(48.0)
            .on_click(move || pe.open.set(!pe.open.get()));
        let attach = column(vec![if p.busy.get() { spinner(20.0, 2.0, Color::WHITE) } else { icon(S_ATTACH, 23.0).color(Color::rgba(255, 255, 255, 170)) }])
            .center()
            .width(46.0)
            .height(48.0)
            .on_click(move || {
                if !pa.busy.get() {
                    me.chat_attach(&pa);
                }
            });
        let pill = Self::pglass(row(vec![emo, input, attach]).align(Align::Center)).radius(24.0).weight(1.0);
        let send = column(vec![icon(S_SEND, 22.0).color(Color::WHITE)])
            .center()
            .width(48.0)
            .height(48.0)
            .lens(if can_send { ACCENT.with_opacity(0.55) } else { Color::rgba(255, 255, 255, 8) }, GLASS_ZOOM)
            .radius(24.0)
            .on_click(move || {
                if !ps.input.get().trim().is_empty() && !ps.busy.get() {
                    let t = ps.input.get();
                    ps.input.set(String::new());
                    me2.chat_send(&ps, t, None);
                }
            });
        let mut bar_col: Vec<Element> = Vec::new();
        if let Some(rid) = p.reply.get() {
            let quoted = p.items.get().iter().find(|m| m.id == rid).cloned();
            let (name, snip) = quoted.map(|m| (if m.from_admin { "Admin".to_string() } else { "Siz".to_string() }, snippet(&m))).unwrap_or(("Xabar".into(), "o'chirilgan".into()));
            let pc = p.clone();
            bar_col.push(
                Self::pglass(
                    row(vec![
                        column(vec![]).width(3.0).height(34.0).radius(2.0).bg(ACCENT2),
                        column(vec![tx(name, 14.0, ACCENT2).bold(), tx(snip, 14.0, Color::WHITE)]).weight(1.0).margin_each(10.0, 0.0, 10.0, 0.0),
                        column(vec![icon(S_CLOSE, 20.0).color(Color::rgba(255, 255, 255, 150))]).center().width(40.0).height(40.0).on_click(move || pc.reply.set(None)),
                    ])
                    .align(Align::Center)
                    .padding_each(14.0, 6.0, 4.0, 6.0),
                )
                .radius(18.0)
                .margin_each(10.0, 6.0, 10.0, 0.0),
            );
        }
        // Yozilayotgan matnda emoji/CJK bo'lsa — maydon ularni chiza olmaydi, shuning uchun ko'rinishi shu yerda.
        let typed = crate::tgmedia::plain_text(&p.input.get());
        if super::richtext::needs_rich(&typed) {
            bar_col.push(
                Self::pglass(self.rich_el(&typed, 15.0, w - 64.0, 0xFFFFFFFF, false).padding_xy(14.0, 8.0))
                    .radius(18.0)
                    .margin_each(10.0, 6.0, 10.0, 0.0),
            );
        }
        bar_col.push(row(vec![pill, send]).gap(8.0).align(Align::End).margin_each(10.0, 6.0, 10.0, 8.0));
        if p.ep.open.get() {
            let (pi, pb, ps2, pe2) = (p.clone(), p.clone(), p.clone(), p.ep.clone());
            let me_s = self.clone();
            let on_emoji: Rc<dyn Fn(String)> = Rc::new(move |e| pi.input.set(format!("{}{}", pi.input.get(), e)));
            let on_back: Rc<dyn Fn()> = Rc::new(move || pb.input.set(super::tgpanel::pop_last_char(&pb.input.get())));
            let on_sticker: Rc<dyn Fn(String, &'static str)> = Rc::new(move |r, kind| {
                if !ps2.busy.get() {
                    me_s.chat_send(&ps2, String::new(), Some((r, kind.to_string())));
                }
            });
            let on_close: Rc<dyn Fn()> = Rc::new(move || pe2.open.set(false));
            bar_col.push(self.tg_panel_view(&p.ep, on_emoji, on_sticker, on_back, on_close).margin_each(10.0, 0.0, 10.0, 8.0));
        }
        let bar = column(bar_col);
        let mut low = Vec::new();
        if !p.err.get().is_empty() && p.loaded.get() {
            low.push(tx(p.err.get(), 12.5, ERROR).padding_xy(18.0, 4.0));
        }
        low.push(bar);
        let title = match p.user_id {
            None => "Admin bilan bog'lanish".to_string(),
            Some(u) => format!("Suhbat · {u}"),
        };
        let me_b = self.clone();
        let round_btn = Self::pglass(column(vec![icon(S_BACK, 24.0).color(Color::WHITE)]).center().width(56.0).height(56.0)).radius(28.0).on_click(move || {
            me_b.pop();
        });
        let admin_side = p.user_id.is_some();
        let letter: String = title.trim_start_matches("Suhbat · ").chars().take(1).flat_map(|c| c.to_uppercase()).collect();
        let avatar = column(vec![tx(if admin_side { letter } else { "A".to_string() }, 18.0, ACCENT2).bold()]).center().width(44.0).height(44.0).radius(22.0).bg(if admin_side { ACCENT.with_opacity(0.25) } else { Color::rgba(0, 0, 0, 150) });
        // Sarlavha kapsulasi balandligi qat'iy (56): matn bir qatorda, sig'masa "…" bilan.
        let name_w = (w - 84.0 - 24.0 - 44.0 - 10.0).max(80.0);
        let sub = if admin_side { format!("ID: {}", p.user_id.unwrap_or(0)) } else { "yordam xizmati".to_string() };
        let capsule = Self::pglass(
            row(vec![
                avatar,
                column(vec![tx(ellipsize(&title, 16.0, name_w), 16.0, Color::WHITE).bold(), tx(ellipsize(&sub, 13.0, name_w), 13.0, Color::rgba(255, 255, 255, 150))]).justify(Justify::Center).weight(1.0),
            ])
            .gap(10.0)
            .align(Align::Center)
            .padding_each(6.0, 0.0, 18.0, 0.0)
            .height(56.0),
        )
        .radius(28.0)
        .weight(1.0);
        let header = row(vec![round_btn, capsule]).gap(8.0).align(Align::Center).margin_each(10.0, 12.0 + ins.top, 10.0, 8.0);
        // Sarlavha ro'yxat USTIDA chiziladi (xabarlar shisha ostida suriladi, hech qachon ustiga chiqmaydi).
        let page = stack(vec![
            Self::glass_backdrop(),
            column(vec![body.grow(1.0)]),
            column(vec![spacer(), column(low)]).padding_each(0.0, 0.0, 0.0, ins.bottom),
            column(vec![header]),
        ]);
        let mut layers = vec![page];
        if !p.menu.get().is_empty() {
            layers.push(self.chat_menu(p));
        }
        // Stiker/GIF/maxsus emojini ushlab turganda butun oyna ustida katta ko'rinish (Telegramdagidek).
        if let Some(ov) = self.tg_preview_view(&p.ep) {
            layers.push(ov);
        }
        stack(layers)
    }

    fn chat_bubble(self: &Rc<Self>, p: &Rc<SupportPage>, m: &ChatMsg, w: f32, top_near: bool, bottom_near: bool) -> Element {
        // Admin ko'rinishida admin xabarlari o'ngda, oddiy foydalanuvchida — o'z xabarlari o'ngda.
        let mine = if p.user_id.is_some() { m.from_admin } else { !m.from_admin };
        let hhmm: String = fmt_date(m.created_at).chars().take(5).collect();
        let (rid, body) = split_reply(&m.body);
        let body = crate::tgmedia::plain_text(&plain(&body));
        let meta_col = if mine { TG_OUT_META } else { Color::rgba(255, 255, 255, 140) };
        let mut inner: Vec<Element> = Vec::new();
        if let Some(rid) = rid {
            let q = p.items.get().iter().find(|x| x.id == rid).cloned();
            let (name, snip) = q.map(|x| (if x.from_admin { "Admin".to_string() } else { "Siz".to_string() }, snippet(&x))).unwrap_or(("Xabar".into(), "o'chirilgan".into()));
            inner.push(
                row(vec![
                    column(vec![]).width(3.0).height(36.0).bg(if mine { Color::WHITE } else { ACCENT2 }),
                    column(vec![tx(name, 14.0, if mine { Color::WHITE } else { ACCENT2 }).bold(), tx(snip, 14.0, Color::WHITE)]).margin_each(8.0, 3.0, 8.0, 3.0),
                ])
                .bg(Color::rgba(255, 255, 255, 20))
                .radius(6.0)
                .margin_each(0.0, 0.0, 0.0, 4.0),
            );
        }
        let maxw = (w - 24.0 - 40.0).min(300.0);
        // Fayl / ovoz — tashqi ko'ruvchida; rasm va video — ilova ichidagi ko'ruvchida (`mediaview.rs`).
        let open = |e: Element| {
            let (me, u) = (self.clone(), m.media_url.clone());
            e.on_click(move || me.chat_open_media(&u))
        };
        let view = |e: Element, kind: &'static str| {
            let (me, u) = (self.clone(), m.media_url.clone());
            e.on_click(move || me.open_media(&u, kind))
        };
        match m.media_type.as_str() {
            "image" => {
                let img = self.posters.borrow().get(&m.media_url).cloned();
                match img {
                    Some(i) => {
                        // Butun rasm nisbati saqlanadi (avval 16:9 ga kesilib, buzilib ko'rinardi).
                        let k = (230.0 / i.w as f32).min(230.0 / i.h as f32);
                        inner.push(view(stack(vec![image_fill(&i)]).width(i.w as f32 * k).height(i.h as f32 * k).radius(12.0), "image"));
                    }
                    None => inner.push(column(vec![spinner(22.0, 2.0, Color::WHITE)]).center().width(230.0).height(230.0).radius(12.0).bg(Color::rgba(0, 0, 0, 64))),
                }
            }
            "video" | "round" => {
                let round = m.media_type == "round";
                let (bw, bh) = if round { (200.0, 200.0) } else { (230.0, 150.0) };
                let mut layers: Vec<Element> = Vec::new();
                let (frame, why) = self.chat_video_frame(&m.media_url);
                if let Some(f) = frame {
                    layers.push(stack(vec![image_fill(&f)]));
                }
                if let Some(why) = why {
                    layers.push(column(vec![tx(format!("Kadr: {why}"), 10.0, Color::rgba(255, 255, 255, 150))]).padding(6.0));
                }
                layers.push(column(vec![column(vec![icon(S_PLAY, 30.0).color(Color::WHITE)]).center().width(48.0).height(48.0).radius(24.0).bg(Color::rgba(0, 0, 0, 128))]).center());
                if m.media_ms > 0 && !round {
                    let s = m.media_ms / 1000;
                    layers.push(column(vec![spacer(), row(vec![spacer(), tx(format!("{}:{:02}", s / 60, s % 60), 11.0, Color::WHITE).bg(Color::rgba(0, 0, 0, 115)).radius(8.0).padding_xy(6.0, 2.0)]).padding(8.0)]));
                }
                inner.push(view(stack(layers).width(bw).height(bh).radius(if round { bh / 2.0 } else { 12.0 }).bg(Color::BLACK), if round { "round" } else { "video" }));
            }
            "voice" => inner.push(open(row(vec![icon(S_PLAY, 24.0).color(Color::WHITE), tx(format!("Ovozli xabar · {}:{:02}", m.media_ms / 60000, (m.media_ms / 1000) % 60), 14.0, Color::WHITE)]).gap(10.0).align(Align::Center).padding_xy(0.0, 6.0))),
            "file" => inner.push(open(row(vec![icon(S_ATTACH, 24.0).color(Color::WHITE), tx(if m.body.is_empty() { "Fayl".to_string() } else { m.body.clone() }, 14.0, Color::WHITE)]).gap(10.0).align(Align::Center).padding_xy(0.0, 6.0))),
            "sticker" | "gif" => {
                let r = crate::data::file_name_of(&m.media_url);
                match self.tg_ref_el(&r, if m.media_type == "gif" { 190.0 } else { 150.0 }, super::tgcache::chat_anim_ok(&m.id)) {
                    Some(e) => inner.push(e),
                    None => inner.push(tx(if m.media_type == "gif" { "GIF" } else { "Stiker" }, 13.5, Color::WHITE).bold()),
                }
            }
            _ => {}
        }
        if m.media_type != "file" && !body.is_empty() {
            // Emoji / koreys / xitoy kabi belgilar Roboto'da yo'q — tizim shriftida rasmga chiziladi (`richtext.rs`).
            if super::richtext::needs_rich(&body) {
                inner.push(self.rich_el(&body, 15.0, maxw.max(120.0) - 16.0, 0xFFFFFFFF, false));
            } else {
                inner.push(tx(body, 15.0, Color::WHITE));
            }
        }
        let mut meta = vec![spacer(), tx(hhmm, 11.0, meta_col)];
        if m.pending {
            meta.push(tx("...", 11.0, TG_OUT_META));
        } else if mine {
            meta.push(icon(S_CHECK, 12.0).color(TG_OUT_META));
            if m.seen {
                meta.push(icon(S_CHECK, 12.0).color(TG_OUT_META).margin_each(-9.0, 0.0, 0.0, 0.0));
            }
        }
        inner.push(row(meta).gap(3.0).align(Align::Center).padding_each(0.0, 2.0, 0.0, 0.0));
        let (pc, id) = (p.clone(), m.id.clone());
        let mut bubble = column(inner);
        bubble = bubble.padding(8.0).radius(if top_near || bottom_near { 12.0 } else { 18.0 }).bg(if mine { TG_OUT } else { TG_IN });
        let bubble = bubble.on_click(move || pc.menu.set(id.clone()));
        let bubble = column(vec![bubble]).width(maxw.max(120.0)).align(if mine { Align::End } else { Align::Start });
        let r = if mine { row(vec![spacer(), bubble]) } else { row(vec![bubble, spacer()]) };
        r.padding_each(0.0, if top_near { 2.0 } else { 6.0 }, 0.0, 0.0)
    }

    /// Video/ovoz/faylni mahalliy Telegram serveri manzili orqali tashqi ko'ruvchida ochadi (Kotlin `playUrl`).
    fn chat_open_media(self: &Rc<Self>, url: &str) {
        uzcoders::android::toast("Ochilmoqda...");
        let (session, u) = (self.session.clone(), url.to_string());
        spawn_bg(move || crate::data::tg_play_url(&session, &u), |r| match r {
            Ok(link) => uzcoders::android::open_url(&link),
            Err(e) => uzcoders::android::toast(&format!("Ochilmadi: {e}")),
        });
    }

    fn chat_menu(self: &Rc<Self>, p: &Rc<SupportPage>) -> Element {
        let id = p.menu.get();
        let msg = p.items.get().iter().find(|m| m.id == id).cloned();
        let Some(m) = msg else {
            p.menu.set(String::new());
            return gap_box(0.0, 0.0);
        };
        let item = |ic: &'static str, label: &str, danger: bool, f: Box<dyn Fn()>| {
            row(vec![icon(ic, 20.0).color(if danger { ERROR } else { Color::WHITE }), tx(label, 15.0, if danger { ERROR } else { Color::WHITE }).bold()]).gap(14.0).align(Align::Center).padding_xy(18.0, 14.0).on_click(f)
        };
        let (p1, body) = (p.clone(), m.body.clone());
        let (p2, me, id2) = (p.clone(), self.clone(), m.id.clone());
        let can_delete = !m.pending && ((p.user_id.is_some()) || !m.from_admin);
        let (p3, id3) = (p.clone(), m.id.clone());
        let mut opts: Vec<Element> = Vec::new();
        if !m.pending {
            opts.push(item(S_REPLY, "Javob berish", false, Box::new(move || {
                p3.reply.set(Some(id3.clone()));
                p3.menu.set(String::new());
            })));
        }
        if m.media_type.is_empty() && !body.is_empty() {
            opts.push(item(S_COPY, "Nusxa olish", false, Box::new(move || {
                uzcoders::android::copy_text(&plain(&split_reply(&body).1));
                uzcoders::android::toast("Nusxa olindi");
                p1.menu.set(String::new());
            })));
        }
        if can_delete {
            opts.push(column(vec![]).height(1.0).bg(Color::rgba(255, 255, 255, 30)));
            opts.push(item(S_TRASH, "O'chirish", true, Box::new(move || {
                p2.menu.set(String::new());
                let v: Vec<ChatMsg> = p2.items.get().iter().filter(|x| x.id != id2).cloned().collect();
                p2.items.set(Rc::new(v));
                let (session, ids) = (me.session.clone(), vec![id2.clone()]);
                spawn_bg(move || api::request("POST", "/api/chat/messages/delete", "", Some(&json!({"ids": ids})), Some(&session)), |_| {});
            })));
        }
        let pc = p.clone();
        let panel = Self::pglass(column(opts)).radius(24.0).margin_each(40.0, 0.0, 40.0, 0.0);
        column(vec![panel]).center().bg(Color::rgba(0, 0, 0, 45)).on_click(move || pc.menu.set(String::new()))
    }
}

#[cfg(test)]
impl ChatMsg {
    pub(super) fn test(id: &str, from_admin: bool, body: &str, created_at: i64) -> ChatMsg {
        ChatMsg { id: id.into(), from_admin, body: body.into(), created_at, media_url: String::new(), media_type: String::new(), media_ms: 0, pending: false, seen: true }
    }
}

#[cfg(test)]
impl SupportPage {
    pub(super) fn items_set(&self, v: Vec<ChatMsg>) {
        self.items.set(Rc::new(v));
        self.loaded.set(true);
    }
}
