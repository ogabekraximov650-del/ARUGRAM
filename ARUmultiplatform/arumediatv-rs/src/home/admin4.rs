//! Admin: animelarni boshqarish (Kotlin `AdminContentScreens`): anime -> bo'limlar -> qismlar, qo'shish/tahrirlash/o'chirish,
//! rasm va video yuklash (Telegram, shifrlangan), asl videoni kodlash navbatiga qo'yish, intro vaqtlari.

use super::*;
use crate::api;
use crate::backend;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

const GENRES: [&str; 37] = [
    "Bolalar uchun", "Detektiv", "Drama", "Ekshen", "Etti", "Fantastika", "Fantaziya", "G'ayrioddiy", "Garem", "Harbiy", "Iblislar", "Ish", "Jang san'ati", "Jangari", "Komediya", "Kosmos", "Kundalik hayot", "Maktab", "Mashinalar",
    "Mexa", "Musiqiy", "O'yinlar", "O'zga dunyo", "Psixologik", "Qo'rqinchli", "Romantika", "Sarguzasht", "Sehr", "Shafqatsizlik", "Sir", "Sport", "Super kuch", "Syodze", "Syonen", "Tarixiy", "Tragediya", "Triller",
];
const QUALS: [&str; 4] = ["360p", "480p", "720p", "1080p"];
const AC_EDIT: &str = "M3,17.25V21h3.75L17.81,9.94l-3.75,-3.75L3,17.25zM20.71,7.04c0.39,-0.39 0.39,-1.02 0,-1.41l-2.34,-2.34c-0.39,-0.39 -1.02,-0.39 -1.41,0l-1.83,1.83 3.75,3.75 1.83,-1.83z";
const AC_DEL: &str = "M6,19c0,1.1 0.9,2 2,2h8c1.1,0 2,-0.9 2,-2V7H6v12zM19,4h-3.5l-1,-1h-5l-1,1H5v2h14V4z";
const AC_ADD: &str = "M19,13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z";
const AC_SEND: &str = "M2.01,21L23,12 2.01,3 2,10l15,2 -15,2z";
const AC_PHOTO: &str = "M21,19V5c0,-1.1 -0.9,-2 -2,-2H5c-1.1,0 -2,0.9 -2,2v14c0,1.1 0.9,2 2,2h14c1.1,0 2,-0.9 2,-2zM8.5,13.5l2.5,3.01L14.5,12l4.5,6H5l3.5,-4.5z";

fn sv(v: &Value, k: &str) -> String {
    match &v[k] {
        Value::String(s) if s != "null" => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

fn iv(v: &Value, k: &str) -> i64 {
    v[k].as_i64().or_else(|| v[k].as_f64().map(|f| f as i64)).or_else(|| v[k].as_str().and_then(|s| s.parse().ok())).unwrap_or(0)
}

fn file_of_url(u: &str) -> String {
    match u.find("/api/image/") {
        Some(i) => u[i + "/api/image/".len()..].to_string(),
        None => u.to_string(),
    }
}

fn call(method: &str, path: &str, body: Option<Value>, session: &str) -> Result<Value, String> {
    let (code, text) = api::request(method, path, "", body.as_ref(), Some(session)).map_err(|_| "Internet yo'q".to_string())?;
    let j: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if code >= 400 {
        return Err(j["error"].as_str().map(String::from).unwrap_or_else(|| if code == 403 { "Bu amal faqat admin uchun".into() } else { format!("Xato ({code})") }));
    }
    Ok(j)
}

fn cache_file(key: &str) -> String {
    let k: String = key.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
    format!("{}/admin_cache_{k}.json", uzcoders::android::files_dir())
}

fn read_cache(key: &str) -> Option<Vec<Value>> {
    let t = std::fs::read_to_string(cache_file(key)).ok()?;
    serde_json::from_str::<Vec<Value>>(&t).ok()
}

fn key_file() -> String {
    format!("{}/admin_tgkeys.json", uzcoders::android::files_dir())
}

fn key_put(name: &str, key: &str) {
    let mut m: serde_json::Map<String, Value> = std::fs::read_to_string(key_file()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    m.insert(name.to_string(), Value::String(key.to_string()));
    let _ = std::fs::write(key_file(), Value::Object(m).to_string());
}

fn key_get(name: &str) -> String {
    std::fs::read_to_string(key_file()).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).and_then(|m| m[name].as_str().map(String::from)).unwrap_or_default()
}

fn normalize_intro(raw: &str) -> String {
    let t = raw.trim();
    if t.is_empty() || t.contains(':') {
        return t.to_string();
    }
    let digits: String = t.chars().filter(|c| c.is_ascii_digit()).collect();
    match digits.parse::<u64>() {
        Ok(0) | Err(_) => String::new(),
        Ok(_) if digits.len() <= 2 => format!("0:{digits:0>2}"),
        Ok(_) => format!("{}:{}", &digits[..digits.len() - 2], &digits[digits.len() - 2..]),
    }
}

fn intro_text(raw: &str) -> String {
    let t = raw.trim();
    if t.is_empty() || t == "0" || t == "null" {
        return String::new();
    }
    if t.contains(':') {
        return t.to_string();
    }
    match t.parse::<i64>() {
        Ok(s) if s > 0 => format!("{}:{:02}", s / 60, s % 60),
        _ => String::new(),
    }
}

fn mb(b: u64) -> String {
    format!("{:.2} MB", b as f64 / 1_048_576.0)
}

// ── Holat turlari ──

#[derive(Clone)]
pub enum Content {
    List(Rc<ListPg>),
    Anime(Rc<AnimeForm>),
    Season(Rc<SeasonForm>),
    Episode(Rc<EpForm>),
}

/// level: 0 — animelar, 1 — bo'limlar, 2 — qismlar.
pub struct ListPg {
    level: u8,
    anime: Value,
    season: Value,
    items: State<Rc<Vec<Value>>>,
    loaded: State<bool>,
    error: State<String>,
    del: State<Option<Value>>,
    post: State<Option<Value>>,
    msg: State<String>,
    scroll: ScrollState,
}

pub struct AnimeForm {
    initial: Value,
    name: State<String>,
    davlat: State<String>,
    studiya: State<String>,
    janri: State<String>,
    tavsif: State<String>,
    photo: State<String>,
    busy: State<bool>,
    err: State<String>,
    scroll: ScrollState,
}

pub struct SeasonForm {
    anime_id: i64,
    initial: Value,
    bolim: State<String>,
    nomi: State<String>,
    studio: State<String>,
    tarjimon: State<String>,
    davlat: State<String>,
    rejissyor: State<String>,
    yili: State<String>,
    yosh: State<String>,
    tavsif: State<String>,
    genres: State<Vec<String>>,
    turi: State<String>,
    holati: State<String>,
    photo: State<String>,
    busy: State<bool>,
    err: State<String>,
    scroll: ScrollState,
}

#[derive(Clone)]
struct Q {
    url: String,
    size: u64,
    key: String,
}

pub struct EpForm {
    anime_id: i64,
    season_id: i64,
    initial: Value,
    ep_id: i64,
    number: State<String>,
    name: State<String>,
    intro: Vec<State<String>>,
    quals: State<Vec<Q>>,
    origin: State<String>,
    origin_key: std::cell::RefCell<String>,
    origin_new: State<bool>,
    job: State<Option<Rc<Value>>>,
    up_target: State<String>,
    up_prog: State<(u64, u64)>,
    sent: Arc<AtomicU64>,
    total: Arc<AtomicU64>,
    saving: State<bool>,
    err: State<String>,
    msg: State<String>,
    del_q: State<Option<usize>>,
    del_origin: State<bool>,
    scroll: ScrollState,
}

// ── Umumiy UI ──

fn gfield(f: Element, label: &str) -> Element {
    f.color(Color::WHITE)
        .size(16.0)
        .line_height(24.0)
        .padding_each(16.0, 26.0, 16.0, 10.0)
        .label_inside()
        .glass(Color::rgba(255, 255, 255, 22))
        .hint_color(Color::rgba(255, 255, 255, 150))
        .accent(ACCENT2)
        .border(1.0, Color::TRANSPARENT)
        .radius(16.0)
        .label(label)
}

fn modal(inner: Element, close: impl Fn() + 'static) -> Element {
    let panel = inner.lens(Color::rgba(18, 18, 24, 120), GLASS_ZOOM).radius(28.0).padding(20.0).margin_each(20.0, 0.0, 20.0, 0.0);
    column(vec![panel]).center().bg(Color::rgba(0, 0, 0, 130)).on_click(close)
}

fn round_btn(path: &'static str, color: Color, f: impl Fn() + 'static) -> Element {
    column(vec![icon(path, 20.0).color(color)]).center().width(40.0).height(40.0).on_click(f)
}

fn fab(f: impl Fn() + 'static) -> Element {
    column(vec![spacer(), row(vec![spacer(), column(vec![icon(AC_ADD, 28.0).color(Color::WHITE)]).center().width(60.0).height(60.0).radius(30.0).gradient(ACCENT, ACCENT2).on_click(f)]).padding_each(0.0, 0.0, 20.0, 24.0)])
}

fn note(t: String, err: bool) -> Element {
    tx(t, 13.0, if err { ERROR } else { Color::hex(0x69F0AE) }).margin_each(0.0, 0.0, 0.0, 10.0)
}

fn glass_card(kids: Vec<Element>) -> Element {
    Home::pglass(column(kids).padding(14.0)).radius(22.0).margin_each(0.0, 0.0, 0.0, 12.0)
}

fn pill_btn(label: &str, primary: bool, busy: bool, f: impl Fn() + 'static) -> Element {
    let inner = if busy { spinner(20.0, 2.0, Color::WHITE) } else { tx(label, 15.0, Color::WHITE).bold() };
    let b = column(vec![inner]).center().padding_xy(0.0, 14.0).radius(26.0).weight(1.0);
    if primary { b.gradient(ACCENT, ACCENT2).on_click(f) } else { Home::pglass(b).radius(26.0).on_click(f) }
}

fn thumb(name: &str, size: f32) -> Element {
    let (a, b) = name_colors(name);
    let ch = name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".into());
    column(vec![tx(ch, size * 0.4, Color::WHITE).bold()]).center().width(size).height(size * 1.2).radius(size * 0.2).gradient(a, b)
}

impl Home {
    pub(super) fn open_admin_content(self: &Rc<Self>) {
        self.open_list(0, Value::Null, Value::Null);
    }

    fn open_list(self: &Rc<Self>, level: u8, anime: Value, season: Value) {
        let cache_key = match level {
            0 => "admin_anime".to_string(),
            1 => format!("admin_seasons_{}", iv(&anime, "id")),
            _ => format!("eps_{}_{}", iv(&anime, "id"), iv(&season, "season_id")),
        };
        let cached = read_cache(&cache_key);
        let has = cached.is_some();
        let mut first = cached.unwrap_or_default();
        if level == 2 {
            first.reverse();
        }
        let p = Rc::new(ListPg { level, anime, season, items: State::new(Rc::new(first)), loaded: State::new(has), error: State::new(String::new()), del: State::new(None), post: State::new(None), msg: State::new(String::new()), scroll: ScrollState::new() });
        self.clist_load(&p);
        self.push(pages::Page::AdminContent(Content::List(p)));
    }

    fn clist_load(self: &Rc<Self>, p: &Rc<ListPg>) {
        let (p2, session) = (p.clone(), self.session.clone());
        let (level, aid, sid) = (p.level, iv(&p.anime, "id"), iv(&p.season, "season_id"));
        spawn_bg(
            move || -> Result<Vec<Value>, String> {
                let (path, key) = match level {
                    0 => ("/api/anime".to_string(), "admin_anime".to_string()),
                    1 => (format!("/api/seasons/anime/{aid}"), format!("admin_seasons_{aid}")),
                    _ => (format!("/api/epizods/{aid}/{sid}?all=1"), format!("eps_{aid}_{sid}")),
                };
                let (pth, q) = match path.split_once('?') {
                    Some((a, b)) => (a.to_string(), format!("?{b}")),
                    None => (path.clone(), String::new()),
                };
                let (code, text) = api::request("GET", &pth, &q, None, Some(&session)).map_err(|e| e)?;
                if code != 200 {
                    return Err(format!("Yuklab bo'lmadi ({code})"));
                }
                let _ = std::fs::write(cache_file(&key), &text);
                serde_json::from_str::<Vec<Value>>(&text).map_err(|_| "Javob noto'g'ri".to_string())
            },
            move |r| {
                match r {
                    Ok(mut v) => {
                        if p2.level == 2 {
                            v.reverse();
                        }
                        p2.items.set(Rc::new(v));
                        p2.error.set(String::new());
                    }
                    Err(e) => {
                        if p2.items.get().is_empty() {
                            p2.error.set(e);
                        }
                    }
                }
                p2.loaded.set(true);
            },
        );
    }

    /// Ro'yxatni (orqada turgan sahifada) qayta yuklaydi: forma saqlangach chaqiriladi.
    fn content_reload_top(self: &Rc<Self>) {
        for pg in self.pages.get().iter().rev() {
            if let pages::Page::AdminContent(Content::List(p)) = pg {
                self.clist_load(p);
                break;
            }
        }
    }

    pub(super) fn admin_content_page(self: &Rc<Self>, c: &Content) -> Element {
        match c {
            Content::List(p) => self.content_list_page(p),
            Content::Anime(f) => self.anime_form_page(f),
            Content::Season(f) => self.season_form_page(f),
            Content::Episode(f) => self.ep_form_page(f),
        }
    }

    // ── Ro'yxat ──

    fn content_list_page(self: &Rc<Self>, p: &Rc<ListPg>) -> Element {
        let title = match p.level {
            0 => "Animelarni boshqarish".to_string(),
            1 => sv(&p.anime, "name"),
            _ => {
                let b = Some(iv(&p.season, "bolim_id")).filter(|b| *b > 0).unwrap_or_else(|| iv(&p.season, "season_id"));
                format!("{b}-bo'lim")
            }
        };
        let items = p.items.get();
        let body: Element = if !p.loaded.get() && items.is_empty() {
            column(vec![spinner(32.0, 3.0, ACCENT2)]).center()
        } else if !p.error.get().is_empty() && items.is_empty() {
            let (me, pp) = (self.clone(), p.clone());
            column(vec![tx(p.error.get(), 14.0, ERROR), gap_box(0.0, 12.0), self.chip("Qayta urinish", true, move || me.clist_load(&pp))]).center()
        } else if items.is_empty() {
            column(vec![tx(match p.level { 0 => "Hali anime qo'shilmagan", 1 => "Hali bo'lim qo'shilmagan", _ => "Hali qism qo'shilmagan" }, 15.0, Color::WHITE)]).center()
        } else {
            let mut kids: Vec<Element> = Vec::new();
            for it in items.iter() {
                kids.push(self.list_row(p, it));
            }
            kids.push(gap_box(0.0, 120.0));
            scroll_column(&p.scroll, kids).padding_each(16.0, 4.0, 16.0, 0.0)
        };
        let mut layers = vec![self.scaffold(&title, body)];
        let (me, pp) = (self.clone(), p.clone());
        layers.push(fab(move || me.list_add(&pp)));
        let m = p.msg.get();
        if !m.is_empty() {
            let pm = p.clone();
            layers.push(column(vec![spacer(), tx(m, 14.0, Color::WHITE).bold().bg(Color::rgba(30, 30, 36, 235)).radius(20.0).padding_xy(18.0, 10.0).margin_each(0.0, 0.0, 0.0, 100.0).on_click(move || pm.msg.set(String::new()))]).align(Align::Center));
        }
        if let Some(d) = p.del.get() {
            let (what, label) = match p.level {
                0 => ("Animeni o'chirish?", sv(&d, "name")),
                1 => ("Bo'limni o'chirish?", sv(&d, "nomi")),
                _ => ("Qismni o'chirish?", format!("{}-qism {}", sv(&d, "epizod_number"), sv(&d, "epizod_name"))),
            };
            let (pc, pd, me) = (p.clone(), p.clone(), self.clone());
            layers.push(modal(
                column(vec![
                    tx(what, 18.0, Color::WHITE).bold(),
                    tx(format!("«{label}» rostanxam o'chirilsinmi? Bu qaytarilmaydi."), 14.0, Color::WHITE).padding_each(0.0, 8.0, 0.0, 16.0),
                    row(vec![
                        { let pc = pc.clone(); self.chip("Bekor qilish", false, move || pc.del.set(None)) },
                        self.chip("Ha, o'chirish", true, move || me.list_delete(&pd)),
                    ])
                    .gap(10.0),
                ]),
                move || pc.del.set(None),
            ));
        }
        if let Some(e) = p.post.get() {
            let (pc, pd, me) = (p.clone(), p.clone(), self.clone());
            layers.push(modal(
                column(vec![
                    tx("Kanalga joylash?", 18.0, Color::WHITE).bold(),
                    tx(format!("{}-qism kanalga post qilib joylansinmi?", sv(&e, "epizod_number")), 14.0, Color::WHITE).padding_each(0.0, 8.0, 0.0, 16.0),
                    row(vec![
                        { let pc = pc.clone(); self.chip("Bekor qilish", false, move || pc.post.set(None)) },
                        self.chip("Ha, joylash", true, move || {
                            pd.post.set(None);
                            let (aid, sid, eid) = (iv(&pd.anime, "id"), iv(&pd.season, "season_id"), iv(&e, "epizod_id"));
                            let (p3, session) = (pd.clone(), me.session.clone());
                            spawn_bg(move || call("POST", &format!("/api/epizods/{aid}/{sid}/{eid}/announce"), None, &session), move |r| p3.msg.set(match r { Ok(_) => "Kanalga joylandi".into(), Err(e) => format!("Joylashda xato: {e}") }));
                        }),
                    ])
                    .gap(10.0),
                ]),
                move || pc.post.set(None),
            ));
        }
        stack(layers)
    }

    fn list_row(self: &Rc<Self>, p: &Rc<ListPg>, it: &Value) -> Element {
        let (title, sub, tag) = match p.level {
            0 => (sv(it, "name"), sv(it, "janri"), String::new()),
            1 => {
                let b = Some(iv(it, "bolim_id")).filter(|b| *b > 0).unwrap_or_else(|| iv(it, "season_id"));
                (format!("{b}-bo'lim — {}", sv(it, "nomi")), "Qismlarni ko'rish →".to_string(), [sv(it, "turi"), sv(it, "holati")].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · "))
            }
            _ => {
                let n = if it.get("epizod_number").is_some() { sv(it, "epizod_number") } else { "?".into() };
                let name = sv(it, "epizod_name");
                (if name.is_empty() { format!("Qism {n}") } else { name }, ep_qualities_line(it), String::new())
            }
        };
        let lead: Element = if p.level == 2 {
            column(vec![tx(if it.get("epizod_number").is_some() { sv(it, "epizod_number") } else { "?".into() }, 15.0, ACCENT2).black()]).center().width(44.0).height(44.0).radius(12.0).bg(ACCENT.with_opacity(0.18))
        } else {
            thumb(&title, 48.0)
        };
        let mut info = vec![tx(title.clone(), 14.0, Color::WHITE).bold()];
        if !tag.is_empty() {
            info.push(tx(tag, 11.0, ACCENT2));
        }
        if !sub.is_empty() {
            info.push(tx(sub, 12.0, Color::WHITE));
        }
        let (me, pp, item) = (self.clone(), p.clone(), it.clone());
        let open = move || match pp.level {
            0 => me.open_list(1, item.clone(), Value::Null),
            1 => me.open_list(2, pp.anime.clone(), item.clone()),
            _ => me.open_ep_form(iv(&pp.anime, "id"), iv(&pp.season, "season_id"), item.clone()),
        };
        let (me2, pp2, item2) = (self.clone(), p.clone(), it.clone());
        let edit = round_btn(AC_EDIT, Color::WHITE, move || me2.list_edit(&pp2, item2.clone()));
        let (pp3, item3) = (p.clone(), it.clone());
        let del = round_btn(AC_DEL, Color::hex(0xFF5252), move || pp3.del.set(Some(item3.clone())));
        let mut actions = vec![];
        if p.level == 2 {
            let (pp4, item4) = (p.clone(), it.clone());
            actions.push(round_btn(AC_SEND, Color::WHITE, move || pp4.post.set(Some(item4.clone()))));
        }
        actions.push(edit);
        actions.push(del);
        Self::pglass(row(vec![lead, column(info).weight(1.0), row(actions)]).gap(12.0).align(Align::Center).padding_xy(12.0, 12.0)).radius(22.0).margin_each(0.0, 0.0, 0.0, 10.0).on_click(open)
    }

    fn list_add(self: &Rc<Self>, p: &Rc<ListPg>) {
        match p.level {
            0 => self.open_anime_form(Value::Null),
            1 => self.open_season_form(iv(&p.anime, "id"), Value::Null),
            _ => self.open_ep_form(iv(&p.anime, "id"), iv(&p.season, "season_id"), Value::Null),
        }
    }

    fn list_edit(self: &Rc<Self>, p: &Rc<ListPg>, it: Value) {
        match p.level {
            0 => self.open_anime_form(it),
            1 => self.open_season_form(iv(&p.anime, "id"), it),
            _ => self.open_ep_form(iv(&p.anime, "id"), iv(&p.season, "season_id"), it),
        }
    }

    fn list_delete(self: &Rc<Self>, p: &Rc<ListPg>) {
        let Some(it) = p.del.get() else { return };
        p.del.set(None);
        let path = match p.level {
            0 => format!("/api/anime/{}", iv(&it, "id")),
            1 => format!("/api/seasons/{}/{}", iv(&p.anime, "id"), iv(&it, "season_id")),
            _ => format!("/api/epizods/{}/{}/{}", iv(&p.anime, "id"), iv(&p.season, "season_id"), iv(&it, "epizod_id")),
        };
        let (p2, me, session) = (p.clone(), self.clone(), self.session.clone());
        spawn_bg(move || call("DELETE", &path, None, &session), move |r| match r {
            Ok(_) => {
                p2.msg.set("O'chirildi".into());
                me.clist_load(&p2);
            }
            Err(e) => p2.msg.set(format!("O'chirishda xato: {e}")),
        });
    }

    // ── Rasm yuklash (anime/bo'lim) ──

    fn pick_photo(self: &Rc<Self>, prefix: &'static str, photo: State<String>, busy: State<bool>, err: State<String>) {
        let session = self.session.clone();
        self.pick("image/*", move |path| {
            busy.set(true);
            err.set(String::new());
            let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
            let name = format!("{prefix}_{ts}.jpg");
            let (n2, s2) = (name.clone(), session.clone());
            let (photo, busy, err) = (photo.clone(), busy.clone(), err.clone());
            spawn_bg(
                move || -> Result<(), String> {
                    let bytes = std::fs::read(&path).map_err(|_| "Rasmni o'qib bo'lmadi".to_string())?;
                    let img = crate::webp::decode_any(&bytes).ok_or("Rasm noto'g'ri (JPEG/PNG kerak)".to_string())?.resized(1280, 1280);
                    let out = format!("{}/{n2}", std::path::Path::new(&path).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default());
                    std::fs::write(&out, img.to_png()).map_err(|_| "Rasm tayyorlanmadi".to_string())?;
                    let r = backend::upload_file(&out, &n2, "image/jpeg", &s2, &|_, _| {});
                    let _ = std::fs::remove_file(&out);
                    let _ = std::fs::remove_file(&path);
                    r
                },
                move |r| {
                    busy.set(false);
                    match r {
                        Ok(()) => photo.set(name),
                        Err(e) => err.set(format!("Rasm yuklashda xato: {e}")),
                    }
                },
            );
        });
    }

    fn photo_box(self: &Rc<Self>, photo: &State<String>, busy: &State<bool>, h: f32, prefix: &'static str, err: &State<String>) -> Element {
        let (me, ph, bz, er) = (self.clone(), photo.clone(), busy.clone(), err.clone());
        let label = if busy.get() { "Yuklanmoqda...".to_string() } else if photo.get().is_empty() { "Rasm tanlash".to_string() } else { format!("Rasm: {}  (almashtirish uchun bosing)", photo.get()) };
        let inner = column(vec![if busy.get() { spinner(28.0, 3.0, Color::WHITE) } else { icon(AC_PHOTO, 40.0).color(Color::WHITE) }, gap_box(0.0, 8.0), tx(label, 13.0, Color::WHITE)]).center();
        Self::pglass(inner).radius(22.0).height(h).margin_each(0.0, 0.0, 0.0, 16.0).on_click(move || {
            if !bz.get() {
                me.pick_photo(prefix, ph.clone(), bz.clone(), er.clone());
            }
        })
    }

    // ── Anime formasi ──

    fn open_anime_form(self: &Rc<Self>, initial: Value) {
        let st = |k: &str| State::new(sv(&initial, k));
        let f = Rc::new(AnimeForm {
            name: st("name"),
            davlat: st("davlat"),
            studiya: st("studiya"),
            janri: st("janri"),
            tavsif: st("tavsif"),
            photo: State::new(file_of_url(&sv(&initial, "photo_url"))),
            busy: State::new(false),
            err: State::new(String::new()),
            scroll: ScrollState::new(),
            initial,
        });
        self.push(pages::Page::AdminContent(Content::Anime(f)));
    }

    fn anime_form_page(self: &Rc<Self>, f: &Rc<AnimeForm>) -> Element {
        let new = f.initial.is_null();
        let mut kids = vec![self.photo_box(&f.photo, &f.busy, 190.0, "anime", &f.err)];
        if !f.err.get().is_empty() {
            kids.push(note(f.err.get(), true));
        }
        for (st, l) in [(&f.name, "Anime nomi"), (&f.davlat, "Davlat"), (&f.studiya, "Studiya"), (&f.janri, "Janri"), (&f.tavsif, "Tavsif")] {
            kids.push(gfield(field(st), l));
            kids.push(gap_box(0.0, 12.0));
        }
        let (fc, me) = (f.clone(), self.clone());
        let save = pill_btn(if new { "Qo'shish" } else { "Saqlash" }, true, f.busy.get(), move || {
            if fc.busy.get() {
                return;
            }
            fc.busy.set(true);
            fc.err.set(String::new());
            let body = json!({"photo_url": fc.photo.get(), "name": fc.name.get(), "davlat": fc.davlat.get(), "studiya": fc.studiya.get(), "janri": fc.janri.get(), "tavsif": fc.tavsif.get()});
            let path = if fc.initial.is_null() { "/api/anime".to_string() } else { format!("/api/anime/{}", iv(&fc.initial, "id")) };
            let method = if fc.initial.is_null() { "POST" } else { "PUT" };
            let (f2, me2, session) = (fc.clone(), me.clone(), me.session.clone());
            spawn_bg(move || call(method, &path, Some(body), &session), move |r| {
                f2.busy.set(false);
                match r {
                    Ok(_) => {
                        me2.pop();
                        me2.content_reload_top();
                    }
                    Err(e) => f2.err.set(format!("Xato: {e}")),
                }
            });
        });
        let me3 = self.clone();
        kids.push(row(vec![pill_btn("Bekor", false, false, move || { me3.pop(); }), save]).gap(12.0));
        kids.push(gap_box(0.0, 40.0));
        self.scaffold(if new { "Anime qo'shish" } else { "Animeni tahrirlash" }, scroll_column(&f.scroll, kids).padding_each(16.0, 4.0, 16.0, 0.0))
    }

    // ── Bo'lim formasi ──

    fn open_season_form(self: &Rc<Self>, anime_id: i64, initial: Value) {
        let st = |k: &str| State::new(sv(&initial, k));
        let num = |k: &str| State::new(Some(iv(&initial, k)).filter(|n| *n > 0).map(|n| n.to_string()).unwrap_or_default());
        let genres: Vec<String> = sv(&initial, "janri").split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        let turi = Some(sv(&initial, "turi")).filter(|t| ["TV", "FILM", "OVA"].contains(&t.as_str())).unwrap_or_else(|| "TV".into());
        let holati = Some(sv(&initial, "holati")).filter(|t| ["Davom etmoqda", "Tugallangan"].contains(&t.as_str())).unwrap_or_else(|| "Davom etmoqda".into());
        let f = Rc::new(SeasonForm {
            anime_id,
            bolim: num("bolim_id"),
            nomi: st("nomi"),
            studio: st("studio"),
            tarjimon: st("tarjimon"),
            davlat: st("davlat"),
            rejissyor: st("rejissyor"),
            yili: st("yili"),
            yosh: num("yosh"),
            tavsif: st("tavsif"),
            genres: State::new(genres),
            turi: State::new(turi),
            holati: State::new(holati),
            photo: State::new(file_of_url(&sv(&initial, "photo_url"))),
            busy: State::new(false),
            err: State::new(String::new()),
            scroll: ScrollState::new(),
            initial,
        });
        self.push(pages::Page::AdminContent(Content::Season(f)));
    }

    /// Chiplarni sig'imiga qarab qatorlarga joylaydi.
    fn chip_flow(self: &Rc<Self>, names: &[&str], is_on: impl Fn(&str) -> bool, on_tap: impl Fn(String) + Clone + 'static) -> Element {
        let (w, _) = window_dp();
        let avail = w - 32.0;
        let mut rows: Vec<Vec<Element>> = vec![Vec::new()];
        let mut used = 0.0;
        for n in names {
            let cw = Self::chip_w(n);
            if used + cw > avail && !rows.last().map(|r| r.is_empty()).unwrap_or(true) {
                rows.push(Vec::new());
                used = 0.0;
            }
            used += cw + 8.0;
            let (t, name) = (on_tap.clone(), n.to_string());
            if let Some(r) = rows.last_mut() {
                r.push(self.chip(n, is_on(n), move || t(name.clone())));
            }
        }
        column(rows.into_iter().map(|r| row(r).gap(8.0).height(40.0).margin_each(0.0, 0.0, 0.0, 8.0)).collect())
    }

    fn segmented(self: &Rc<Self>, opts: &[&str], st: &State<String>) -> Element {
        row(opts
            .iter()
            .map(|o| {
                let (s2, v) = (st.clone(), o.to_string());
                let on = st.get() == *o;
                let b = column(vec![tx(*o, 13.0, Color::WHITE).bold()]).center().padding_xy(0.0, 12.0).radius(14.0).weight(1.0);
                if on { b.gradient(ACCENT, ACCENT2).on_click(move || s2.set(v.clone())) } else { Self::pglass(b).radius(14.0).on_click(move || s2.set(v.clone())) }
            })
            .collect())
        .gap(8.0)
    }

    fn season_form_page(self: &Rc<Self>, f: &Rc<SeasonForm>) -> Element {
        let new = f.initial.is_null();
        let mut kids = vec![self.photo_box(&f.photo, &f.busy, 170.0, "season", &f.err)];
        if !f.err.get().is_empty() {
            kids.push(note(f.err.get(), true));
        }
        let num = |e: Element| e.keyboard(KeyboardKind::Number);
        kids.push(gfield(num(field(&f.bolim)), "Bo'lim raqami (N-bo'lim)"));
        kids.push(gap_box(0.0, 12.0));
        for (st, l) in [(&f.nomi, "Nomi"), (&f.studio, "Studiya"), (&f.tarjimon, "Tarjimon"), (&f.davlat, "Davlat"), (&f.rejissyor, "Rejissyor")] {
            kids.push(gfield(field(st), l));
            kids.push(gap_box(0.0, 12.0));
        }
        kids.push(gfield(num(field(&f.yili)), "Yili"));
        kids.push(gap_box(0.0, 12.0));
        kids.push(gfield(num(field(&f.yosh)), "Yosh chegarasi (masalan 18)"));
        kids.push(gap_box(0.0, 16.0));
        kids.push(tx("Janri", 14.0, Color::WHITE).bold().margin_each(0.0, 0.0, 0.0, 8.0));
        let gs = f.genres.clone();
        let g2 = f.genres.clone();
        kids.push(self.chip_flow(&GENRES, move |n| gs.get().iter().any(|x| x == n), move |n| {
            let mut v = g2.get();
            if let Some(i) = v.iter().position(|x| *x == n) {
                v.remove(i);
            } else {
                v.push(n);
            }
            g2.set(v);
        }));
        kids.push(gap_box(0.0, 8.0));
        kids.push(tx("Turi", 14.0, Color::WHITE).bold().margin_each(0.0, 0.0, 0.0, 8.0));
        kids.push(self.segmented(&["TV", "FILM", "OVA"], &f.turi));
        kids.push(gap_box(0.0, 16.0));
        kids.push(tx("Holat", 14.0, Color::WHITE).bold().margin_each(0.0, 0.0, 0.0, 8.0));
        kids.push(self.segmented(&["Davom etmoqda", "Tugallangan"], &f.holati));
        kids.push(gap_box(0.0, 16.0));
        kids.push(gfield(field(&f.tavsif), "Tavsif"));
        kids.push(gap_box(0.0, 24.0));
        let (fc, me) = (f.clone(), self.clone());
        let save = pill_btn(if new { "Qo'shish" } else { "Saqlash" }, true, f.busy.get(), move || {
            if fc.busy.get() {
                return;
            }
            fc.busy.set(true);
            fc.err.set(String::new());
            let mut g = fc.genres.get();
            g.sort();
            let body = json!({
                "anime_id": fc.anime_id, "bolim_id": fc.bolim.get().parse::<i64>().unwrap_or(0), "photo_url": fc.photo.get(), "nomi": fc.nomi.get(), "studio": fc.studio.get(),
                "tarjimon": fc.tarjimon.get(), "yili": fc.yili.get(), "janrlar": g, "janri": g.join(", "), "turi": fc.turi.get(), "holati": fc.holati.get(),
                "tavsif": fc.tavsif.get(), "davlat": fc.davlat.get().trim(), "rejissyor": fc.rejissyor.get().trim(), "yosh": fc.yosh.get().trim().parse::<i64>().unwrap_or(0),
            });
            let path = if fc.initial.is_null() { "/api/seasons".to_string() } else { format!("/api/seasons/{}/{}", fc.anime_id, iv(&fc.initial, "season_id")) };
            let method = if fc.initial.is_null() { "POST" } else { "PUT" };
            let (f2, me2, session) = (fc.clone(), me.clone(), me.session.clone());
            spawn_bg(move || call(method, &path, Some(body), &session), move |r| {
                f2.busy.set(false);
                match r {
                    Ok(_) => {
                        me2.pop();
                        me2.content_reload_top();
                    }
                    Err(e) => f2.err.set(format!("Xato: {e}")),
                }
            });
        });
        let me3 = self.clone();
        kids.push(row(vec![pill_btn("Bekor", false, false, move || { me3.pop(); }), save]).gap(12.0));
        kids.push(gap_box(0.0, 40.0));
        self.scaffold(if new { "Yangi bo'lim" } else { "Bo'limni tahrirlash" }, scroll_column(&f.scroll, kids).padding_each(16.0, 4.0, 16.0, 0.0))
    }

    // ── Qism formasi ──

    fn open_ep_form(self: &Rc<Self>, anime_id: i64, season_id: i64, initial: Value) {
        let origin = sv(&initial, "origin_video");
        let quals: Vec<Q> = QUALS
            .iter()
            .map(|q| {
                let u = file_of_url(&sv(&initial, &format!("url_{q}")));
                // Kodlanguncha worker asl videoni bo'sh sifat o'rnida beradi: u sifat emas.
                if !origin.is_empty() && u == origin {
                    Q { url: String::new(), size: 0, key: String::new() }
                } else {
                    Q { url: u, size: iv(&initial, &format!("size_{q}")).max(0) as u64, key: String::new() }
                }
            })
            .collect();
        let intro: Vec<State<String>> = (1..=10).map(|i| State::new(intro_text(&sv(&initial, &format!("intro_{i}"))))).collect();
        let f = Rc::new(EpForm {
            anime_id,
            season_id,
            ep_id: iv(&initial, "epizod_id"),
            number: State::new(if initial.get("epizod_number").is_some() { sv(&initial, "epizod_number") } else { String::new() }),
            name: State::new(sv(&initial, "epizod_name")),
            intro,
            quals: State::new(quals),
            origin: State::new(origin),
            origin_key: std::cell::RefCell::new(String::new()),
            origin_new: State::new(false),
            job: State::new(None),
            up_target: State::new(String::new()),
            up_prog: State::new((0, 0)),
            sent: Arc::new(AtomicU64::new(0)),
            total: Arc::new(AtomicU64::new(0)),
            saving: State::new(false),
            err: State::new(String::new()),
            msg: State::new(String::new()),
            del_q: State::new(None),
            del_origin: State::new(false),
            scroll: ScrollState::new(),
            initial,
        });
        self.ep_status(&f);
        self.push(pages::Page::AdminContent(Content::Episode(f)));
    }

    fn ep_status(self: &Rc<Self>, f: &Rc<EpForm>) {
        if f.ep_id <= 0 {
            return;
        }
        let (aid, sid, session) = (f.anime_id, f.season_id, self.session.clone());
        spawn_bg(move || api::get_json("/api/encode/status", &format!("?anime_id={aid}&season_id={sid}"), Some(&session)), {
            let f = f.clone();
            move |r: Result<Value, String>| {
                if let Ok(j) = r {
                    let job = j["items"].as_array().and_then(|a| a.iter().find(|x| iv(x, "epizod_id") == f.ep_id).cloned());
                    f.job.set(job.map(Rc::new));
                }
            }
        });
    }

    /// Yuklash jarayonini ~0.5 s da yangilab turadi.
    fn ep_progress_watch(self: &Rc<Self>, f: &Rc<EpForm>) {
        let (f2, me) = (f.clone(), self.clone());
        spawn_bg(|| std::thread::sleep(std::time::Duration::from_millis(500)), move |_| {
            if f2.up_target.get().is_empty() {
                return;
            }
            f2.up_prog.set((f2.sent.load(Ordering::SeqCst), f2.total.load(Ordering::SeqCst)));
            me.ep_progress_watch(&f2);
        });
    }

    fn ep_upload(self: &Rc<Self>, f: &Rc<EpForm>, target: &'static str) {
        if !f.up_target.get().is_empty() {
            return;
        }
        let (fc, me) = (f.clone(), self.clone());
        self.pick("video/*", move |path| {
            let ext = std::path::Path::new(&path).extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).filter(|e| !e.is_empty() && e.len() <= 4).unwrap_or_else(|| "mp4".into());
            let mime = if ext == "mkv" { "video/x-matroska" } else { "video/mp4" };
            let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
            let uid = me.profile.get().id;
            let fname = if target == "origin" { format!("orig_{uid}_{}_{}_{ts}.{ext}", fc.anime_id, fc.season_id) } else { format!("ep_{}_{}_{target}_{ts}.{ext}", fc.anime_id, fc.season_id) };
            fc.up_target.set(target.to_string());
            fc.up_prog.set((0, 0));
            fc.sent.store(0, Ordering::SeqCst);
            fc.total.store(std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0), Ordering::SeqCst);
            fc.err.set(String::new());
            me.ep_progress_watch(&fc);
            let (sent, total, session, fn2, mime) = (fc.sent.clone(), fc.total.clone(), me.session.clone(), fname.clone(), mime.to_string());
            let (f2, p2) = (fc.clone(), path.clone());
            spawn_bg(
                move || {
                    let size = std::fs::metadata(&p2).map(|m| m.len()).unwrap_or(0);
                    let r = backend::upload_file_key(&p2, &fn2, &mime, &session, &|s, t| {
                        sent.store(s, Ordering::SeqCst);
                        if t > 0 {
                            total.store(t, Ordering::SeqCst);
                        }
                    });
                    let _ = std::fs::remove_file(&p2);
                    (r, size)
                },
                move |(r, size)| {
                    f2.up_target.set(String::new());
                    match r {
                        Ok(key) => {
                            key_put(&fname, &key);
                            if target == "origin" {
                                f2.origin.set(fname);
                                *f2.origin_key.borrow_mut() = key;
                                f2.origin_new.set(true);
                                f2.job.set(None);
                                f2.msg.set("Asl video yuklandi — «Saqlash»dan keyin kodlash navbatiga qo'yiladi".into());
                            } else if let Some(i) = QUALS.iter().position(|q| *q == target) {
                                let mut v = f2.quals.get();
                                v[i] = Q { url: fname, size, key };
                                f2.quals.set(v);
                                f2.msg.set(format!("{target} Telegram kanaliga yuklandi — endi «Saqlash»ni bosing"));
                            }
                        }
                        Err(e) => f2.err.set(format!("Yuklanmadi: {e}")),
                    }
                },
            );
        });
    }

    fn encode_text(job: Option<Rc<Value>>, origin_new: bool) -> Option<String> {
        let Some(j) = job else { return if origin_new { Some("Saqlangach kodlash navbatiga qo'yiladi".into()) } else { None } };
        let done = sv(&j, "done").split(',').filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ");
        match sv(&j, "state").as_str() {
            "queued" => Some(if done.is_empty() { "Navbatda".into() } else { format!("Navbatda (tayyor: {done})") }),
            "running" => Some(if done.is_empty() { "Kodlanmoqda...".into() } else { format!("Kodlanmoqda... tayyor: {done}") }),
            "done" => Some(format!("Tayyor: {done}")),
            "error" => Some(format!("Xato: {}", sv(&j, "error"))),
            _ => None,
        }
    }

    fn up_bar(f: &Rc<EpForm>) -> Element {
        let (s, t) = f.up_prog.get();
        let frac = if t > 0 { (s as f32 / t as f32).clamp(0.0, 1.0) } else { 0.0 };
        column(vec![
            stack(vec![column(vec![]).height(6.0).radius(3.0).bg(Color::rgba(255, 255, 255, 30)), row(vec![column(vec![]).height(6.0).radius(3.0).bg(ACCENT2).weight(frac.max(0.001)), column(vec![]).weight((1.0 - frac).max(0.001))])]),
            tx(format!("{:.1}% ({} / {})", if t > 0 { s as f64 * 100.0 / t as f64 } else { 0.0 }, mb(s), mb(t)), 12.0, Color::WHITE).padding_each(0.0, 8.0, 0.0, 0.0),
        ])
    }

    fn upload_btn(self: &Rc<Self>, label: &str, f: impl Fn() + 'static) -> Element {
        Self::pglass(column(vec![tx(label, 13.0, ACCENT2).bold()]).center().padding_xy(0.0, 14.0)).radius(16.0).on_click(f)
    }

    fn ep_form_page(self: &Rc<Self>, f: &Rc<EpForm>) -> Element {
        let new = f.initial.is_null();
        let mut kids: Vec<Element> = Vec::new();
        if !f.err.get().is_empty() {
            kids.push(note(f.err.get(), true));
        }
        if !f.msg.get().is_empty() {
            kids.push(note(f.msg.get(), false));
        }
        kids.push(gfield(field(&f.number).keyboard(KeyboardKind::Number), "Qism raqami (0 dan boshlash)"));
        kids.push(gap_box(0.0, 12.0));
        kids.push(gfield(field(&f.name), "Qism nomi (ixtiyoriy)"));
        kids.push(gap_box(0.0, 20.0));

        // Asl video.
        let origin = f.origin.get();
        let status = Self::encode_text(f.job.get(), f.origin_new.get());
        let failed = f.job.get().map(|j| sv(&j, "state") == "error").unwrap_or(false);
        let mut ob = vec![tx("Asl video (avto-kodlash)", 14.0, Color::WHITE).bold(), tx("Bitta asl video yuklang — 1080p/720p/480p/360p (manbadan baland emas) H.265 bilan o'zi tayyorlanadi va yoziladi.", 12.0, Color::WHITE).padding_each(0.0, 4.0, 0.0, 10.0)];
        if f.up_target.get() == "origin" {
            ob.push(Self::up_bar(f));
        } else {
            if !origin.is_empty() {
                let fc = f.clone();
                ob.push(
                    row(vec![
                        tx(if failed { "!" } else { "✓" }, 16.0, if failed { ERROR } else { Color::hex(0x69F0AE) }).width(22.0),
                        tx(status.unwrap_or_else(|| "Asl video yuklangan".into()), 13.0, Color::WHITE).weight(1.0),
                        round_btn(AC_DEL, Color::hex(0xFF5252), move || fc.del_origin.set(true)),
                    ])
                    .align(Align::Center)
                    .margin_each(0.0, 0.0, 0.0, 10.0),
                );
            }
            let (fc, me) = (f.clone(), self.clone());
            ob.push(self.upload_btn(if origin.is_empty() { "Original video yuklash" } else { "Boshqa asl video yuklash" }, move || me.ep_upload(&fc, "origin")));
            if failed && !f.origin_new.get() && f.ep_id > 0 {
                let (fc, me) = (f.clone(), self.clone());
                ob.push(tx("Qayta urinish", 14.0, ACCENT2).bold().padding_xy(0.0, 10.0).on_click(move || me.ep_requeue(&fc)));
            }
        }
        kids.push(glass_card(ob));

        // Sifatlar.
        let quals = f.quals.get();
        for (i, q) in QUALS.iter().enumerate() {
            let mut c = vec![tx(format!("{q} sifat"), 14.0, Color::WHITE).bold(), gap_box(0.0, 10.0)];
            if f.up_target.get() == *q {
                c.push(Self::up_bar(f));
            } else if !quals[i].url.is_empty() {
                let fc = f.clone();
                c.push(
                    row(vec![tx("✓", 16.0, Color::hex(0x69F0AE)).width(22.0), tx(if quals[i].size > 0 { mb(quals[i].size) } else { "Yuklangan".into() }, 13.0, Color::WHITE).weight(1.0), round_btn(AC_DEL, Color::hex(0xFF5252), move || fc.del_q.set(Some(i)))])
                        .align(Align::Center),
                );
            } else {
                let (fc, me) = (f.clone(), self.clone());
                let qq: &'static str = q;
                c.push(self.upload_btn(&format!("{q} fayl yuklash"), move || me.ep_upload(&fc, qq)));
            }
            kids.push(glass_card(c));
        }

        // Intro.
        let mut ib = vec![tx("O'tkazib yuboriladigan joylar", 14.0, Color::WHITE).bold(), tx("Chapga boshlanish, o'ngga tugash vaqti: 5:14 va 6:44. Bo'sh qatorlar e'tiborga olinmaydi.", 11.5, Color::WHITE).padding_each(0.0, 4.0, 0.0, 12.0)];
        for r in 0..5 {
            ib.push(row(vec![tx(format!("{}", r + 1), 12.0, Color::WHITE).width(20.0), gfield_small(&f.intro[r * 2], "boshi"), gfield_small(&f.intro[r * 2 + 1], "oxiri")]).gap(10.0).align(Align::Center).margin_each(0.0, 0.0, 0.0, 8.0));
        }
        kids.push(glass_card(ib));

        let (fc, me) = (f.clone(), self.clone());
        let save = pill_btn(if new { "Qo'shish" } else { "Saqlash" }, true, f.saving.get(), move || me.ep_save(&fc));
        let me3 = self.clone();
        kids.push(row(vec![pill_btn("Bekor", false, false, move || { me3.pop(); }), save]).gap(12.0));
        kids.push(gap_box(0.0, 40.0));

        let mut layers = vec![self.scaffold(if new { "Qism qo'shish" } else { "Qismni tahrirlash" }, scroll_column(&f.scroll, kids).padding_each(16.0, 4.0, 16.0, 0.0))];
        if let Some(i) = f.del_q.get() {
            let (fc, fd, me) = (f.clone(), f.clone(), self.clone());
            layers.push(modal(
                column(vec![
                    tx(format!("{} faylni o'chirish?", QUALS[i]), 18.0, Color::WHITE).bold(),
                    tx("Fayl kanaldan ham, bazadan ham o'chiriladi. Davom etasizmi?", 14.0, Color::WHITE).padding_each(0.0, 8.0, 0.0, 16.0),
                    row(vec![{ let fc = fc.clone(); self.chip("Bekor qilish", false, move || fc.del_q.set(None)) }, self.chip("Ha, o'chirish", true, move || me.ep_del_quality(&fd, i))]).gap(10.0),
                ]),
                move || fc.del_q.set(None),
            ));
        }
        if f.del_origin.get() {
            let (fc, fd, me) = (f.clone(), f.clone(), self.clone());
            layers.push(modal(
                column(vec![
                    tx("Asl videoni o'chirish?", 18.0, Color::WHITE).bold(),
                    tx("Asl video kanaldan ham, bazadan ham o'chiriladi va kodlash navbatdan olinadi. Tayyor sifatlar qoladi.", 14.0, Color::WHITE).padding_each(0.0, 8.0, 0.0, 16.0),
                    row(vec![{ let fc = fc.clone(); self.chip("Bekor qilish", false, move || fc.del_origin.set(false)) }, self.chip("Ha, o'chirish", true, move || me.ep_del_origin(&fd))]).gap(10.0),
                ]),
                move || fc.del_origin.set(false),
            ));
        }
        stack(layers)
    }

    fn ep_requeue(self: &Rc<Self>, f: &Rc<EpForm>) {
        let (f2, me, session) = (f.clone(), self.clone(), self.session.clone());
        let body = json!({"anime_id": f.anime_id, "season_id": f.season_id, "epizod_id": f.ep_id, "origin": f.origin.get(), "key": key_get(&f.origin.get())});
        spawn_bg(move || call("POST", "/api/encode/queue", Some(body), &session), move |r| {
            f2.msg.set(match r { Ok(_) => "Qayta navbatga qo'yildi".into(), Err(e) => format!("Xato: {e}") });
            me.ep_status(&f2);
        });
    }

    fn ep_del_quality(self: &Rc<Self>, f: &Rc<EpForm>, i: usize) {
        f.del_q.set(None);
        let clear = {
            let f = f.clone();
            move || {
                let mut v = f.quals.get();
                v[i] = Q { url: String::new(), size: 0, key: String::new() };
                f.quals.set(v);
            }
        };
        if f.ep_id <= 0 {
            clear();
            return;
        }
        let (f2, session) = (f.clone(), self.session.clone());
        let path = format!("/api/epizods/{}/{}/{}/{}", f.anime_id, f.season_id, f.ep_id, QUALS[i]);
        spawn_bg(move || call("DELETE", &path, None, &session), move |r| match r {
            Ok(_) => clear(),
            Err(e) => f2.err.set(format!("O'chirishda xato: {e}")),
        });
    }

    fn ep_del_origin(self: &Rc<Self>, f: &Rc<EpForm>) {
        f.del_origin.set(false);
        let (f2, session) = (f.clone(), self.session.clone());
        let body = json!({"anime_id": f.anime_id, "season_id": f.season_id, "epizod_id": f.ep_id, "origin": f.origin.get()});
        spawn_bg(move || call("POST", "/api/encode/delete", Some(body), &session), move |r| match r {
            Ok(_) => {
                f2.origin.set(String::new());
                f2.origin_new.set(false);
                f2.job.set(None);
                f2.msg.set("Asl video o'chirildi".into());
            }
            Err(e) => f2.err.set(format!("Asl video o'chirilmadi: {e}")),
        });
    }

    fn ep_save(self: &Rc<Self>, f: &Rc<EpForm>) {
        if f.saving.get() || !f.up_target.get().is_empty() {
            return;
        }
        f.saving.set(true);
        f.err.set(String::new());
        let mut body = json!({"anime_id": f.anime_id, "season_id": f.season_id, "epizod_number": f.number.get().parse::<i64>().unwrap_or(0), "epizod_name": f.name.get()});
        for (i, q) in QUALS.iter().enumerate() {
            let c = f.quals.get()[i].clone();
            body[format!("url_{q}")] = json!(c.url);
            body[format!("size_{q}")] = json!(c.size);
            body[format!("key_{q}")] = json!(c.key);
        }
        for (i, st) in f.intro.iter().enumerate() {
            body[format!("intro_{}", i + 1)] = json!(normalize_intro(&st.get()));
        }
        let (origin, origin_new, origin_key) = (f.origin.get(), f.origin_new.get(), f.origin_key.borrow().clone());
        let (aid, sid, ep_id, new) = (f.anime_id, f.season_id, f.ep_id, f.initial.is_null());
        let (f2, me, session) = (f.clone(), self.clone(), self.session.clone());
        spawn_bg(
            move || -> Result<Option<String>, String> {
                let (method, path) = if new { ("POST", "/api/epizods".to_string()) } else { ("PUT", format!("/api/epizods/{aid}/{sid}/{ep_id}")) };
                let j = call(method, &path, Some(body), &session)?;
                if origin_new && !origin.is_empty() {
                    let id = if ep_id > 0 { ep_id } else { iv(&j, "epizod_id") };
                    if id == 0 {
                        return Ok(Some("Qism saqlandi, lekin kodlash navbatiga qo'yilmadi: qism raqami olinmadi".into()));
                    }
                    let key = if origin_key.is_empty() { key_get(&origin) } else { origin_key };
                    let q = json!({"anime_id": aid, "season_id": sid, "epizod_id": id, "origin": origin, "key": key});
                    return Ok(Some(match call("POST", "/api/encode/queue", Some(q), &session) {
                        Ok(_) => "Kodlash navbatiga qo'yildi".into(),
                        Err(e) => format!("Qism saqlandi, lekin kodlash navbatiga qo'yilmadi: {e}"),
                    }));
                }
                Ok(None)
            },
            move |r| {
                f2.saving.set(false);
                match r {
                    Ok(_) => {
                        me.pop();
                        me.content_reload_top();
                    }
                    Err(e) => f2.err.set(format!("Xato: {e}")),
                }
            },
        );
    }
}

fn gfield_small(st: &State<String>, label: &str) -> Element {
    field(st)
        .color(Color::WHITE)
        .size(14.0)
        .padding_xy(12.0, 10.0)
        .glass(Color::rgba(255, 255, 255, 22))
        .hint(label)
        .hint_color(Color::rgba(255, 255, 255, 150))
        .accent(ACCENT2)
        .border(1.0, Color::TRANSPARENT)
        .radius(12.0)
        .weight(1.0)
}

fn ep_qualities_line(e: &Value) -> String {
    let o = sv(e, "origin_video");
    let is_origin = |u: &str| !o.is_empty() && (u == o || u.ends_with(&format!("/{o}")));
    let mut q: Vec<String> = QUALS.iter().filter(|k| { let u = sv(e, &format!("url_{k}")); !u.is_empty() && !is_origin(&u) }).map(|k| k.to_string()).collect();
    if !o.is_empty() {
        q.push("Original — tayyorlanmoqda".into());
    }
    if q.is_empty() { "Fayl yo'q".into() } else { q.join(" • ") }
}
