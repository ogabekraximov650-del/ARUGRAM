// ══════════════════════════════════════════════════════════════
//  ANIBLA.UZ YUKLASH — kodlash botining uchinchi bo'limi
// ══════════════════════════════════════════════════════════════
//
// TALAB (foydalanuvchi): bot anibla.uz ga login/parol bilan kiradi,
// "Izlash" tugmasi; nom yoziladi — topilgan anime/filmning qismi va
// mavjud sifatlari (1080p, 720p, ...) tugma bo'lib chiqadi; tugma
// bosilganda video GitHub Actions orqali (IKKINCHI akkauntdagi `GH_REPO`)
// yuklab olinib, botga yuboriladi. Login/parol AES bilan shifrlangan
// faylda (`tool/anibla/creds.enc`, kalit — `ENCODE_TOKEN`).
//
// SAYT (2026-10 holatiga, sayt JS kodidan va so'rovlardan aniqlangan):
//   * API: `<sayt>/api/backend/api/v1/...` (Next.js proksi);
//   * kirish: `POST auth/login-password {phone_number, password}` ->
//     `token.accessToken` (14 kun amal qiladi). So'rovlarga cookie
//     `access_token=<token>` bilan beriladi;
//   * qidiruv: `GET media/mobile?search=&limit=&page=` (loginsiz);
//   * serial: `GET seasons/<slug>` -> fasllar, `GET episodes/<slug>/<fasl>
//     ?view=compact` -> qismlar (loginsiz, video manzilisiz);
//   * qism videosi: `GET episodes/<slug>/<fasl>/<qism-slug>` (LOGIN bilan),
//     film: `GET movies/<slug>` (LOGIN bilan) -> `video` =
//     `<sayt>/stream/<id>`;
//   * `<video>?format=api` -> `{"file": ".../index.m3u8"}` — HLS master
//     playlist, ichida sifatlar (`RESOLUTION=1920x1080` ...). Playlist va
//     bo'laklar loginsiz ochiladi.
//
// OQIM: worker faqat JSON va m3u8 matnini o'qiydi (video baytlari worker'dan
// O'TMAYDI). Sifat bosilganda video `anibla_jobs` navbatiga tushadi va
// ALOHIDA workflow `anibla.yml` (GH_REPO, shablon
// `tool/anibla/anibla.workflow.yml`) ishga tushadi. `tool/anibla/download.py`
// navbat bo'shaguncha videolarni KETMA-KET oladi (`/api/anibla/claim`):
// ffmpeg bilan HLS'ni mp4 ga yig'adi, Telegram sessiyasi bilan yopiq kanalga
// yuklaydi, `/api/anibla/done` — bot uni BOT CHATIGA `copyMessage` qiladi va
// kanal postini o'chiradi. Jarayon har video uchun bitta holat xabarida
// jonli ko'rinadi (`/api/anibla/progress`, bazaga yozuvsiz).
//
// HOLAT: `encbot_mode='anibla'` (shu rejimda yozilgan matn — qidiruv),
// `anibla_nav` — oxirgi qidiruv natijalari (tugmalarda faqat raqamlar:
// Telegram `callback_data` 64 baytdan oshmaydi), `anibla_token`.

use super::*;

pub(crate) const BTN: &str = "\u{1F39E} Anibla yuklash";
const BTN_SEARCH: &str = "\u{1F50D} Izlash";
const BTN_QUEUE: &str = "\u{1F4CB} Yuklash navbati";
const BTN_CATS: &str = "\u{1F4C2} Bo'limlar";
const BTN_YEARS: &str = "\u{1F4C5} Yil bo'yicha";
/// Yil tugmasi: "📅 2026-yil · 53 ta anime".
const YEAR_PREFIX: &str = "\u{1F4C5} ";
const WORKFLOW: &str = "anibla.yml";
const DEFAULT_SITE: &str = "https://anibla.uz";
/// Rasmlar (poster/thumbnail) anibla.uz'da EMAS, ortidagi serverda turadi
/// (sayt JS: `NEXT_PUBLIC_API_BASE_URL || "https://amediatv.up-it.uz"`).
/// `anibla.uz/uploads/...` 404 beradi — shu sabab rasm botda chiqmasdi.
const IMG_BASE: &str = "https://amediatv.up-it.uz";
/// Qidiruv natijalari bir sahifada.
/// Ro'yxat sahifasi — 100 tadan (foydalanuvchi talabi). Telegram pastki panelga
/// 150 tagacha tugmani qabul qildi, 300 ni rad etdi (sinab ko'rilgan).
const SEARCH_PAGE: i64 = 100;
/// Qismlar sahifasi: 3 tadan qatorda 150 tagacha (Telegram chegarasidan ichkarida;
/// 150 tugma sinab ko'rilgan). Qisqa seriallar bitta sahifaga sig'adi.
const EP_PAGE: usize = 150;
const UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36";

thread_local! {
    /// Kirish tokeni izolyat xotirasida — har so'rovda bazadan o'qilmasin.
    static TOKEN: std::cell::RefCell<String> = std::cell::RefCell::new(String::new());
}

// ── Sozlama: sayt, login, parol ──────────────────────────────

/// `ANIBLA_CREDS` — `tool/anibla/creds.enc` ochilgan JSON
/// (`{"site": ..., "login": ..., "password": ...}`), deployda qo'yiladi.
struct Creds {
    site: String,
    phone: i64,
    password: String,
}

fn creds(env: &Env) -> Option<Creds> {
    let v: Value = serde_json::from_str(&tg_secret(env, "ANIBLA_CREDS")).ok()?;
    let site = v["site"].as_str().unwrap_or("").trim().trim_end_matches('/').to_string();
    let phone: String = v["login"].as_str().map(String::from)
        .or_else(|| v["login"].as_i64().map(|n| n.to_string()))
        .unwrap_or_default().chars().filter(|c| c.is_ascii_digit()).collect();
    let password = v["password"].as_str().unwrap_or("").to_string();
    if phone.is_empty() || password.is_empty() {
        return None;
    }
    Some(Creds {
        site: if site.starts_with("http") { site } else { DEFAULT_SITE.to_string() },
        phone: phone.parse().ok()?,
        password,
    })
}

fn site(env: &Env) -> String {
    creds(env).map(|c| c.site).unwrap_or_else(|| DEFAULT_SITE.to_string())
}

/// `thumbnail`/`cover` maydonidan to'liq rasm manzili. Bo'sh — bo'sh;
/// http bilan boshlansa — o'zicha; aks holda `IMG_BASE` old qo'shiladi.
fn img_url(img: &str) -> String {
    let i = img.trim();
    if i.is_empty() {
        String::new()
    } else if i.starts_with("http://") || i.starts_with("https://") {
        i.to_string()
    } else {
        format!("{IMG_BASE}/{}", i.trim_start_matches('/'))
    }
}

fn api_base(env: &Env) -> String {
    format!("{}/api/backend/api/v1", site(env))
}

/// URL qismi uchun foiz-kodlash (qism slug'lari o'zbekcha matn: `ʻ`, bo'shliq).
fn enc(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

// ── Sayt bilan aloqa ─────────────────────────────────────────

async fn http(url: &str, method: Method, body: Option<Value>, cookie: Option<&str>)
    -> std::result::Result<(u16, String), String> {
    let h = Headers::new();
    let _ = h.set("User-Agent", UA);
    let _ = h.set("Accept", "application/json, text/plain, */*");
    if body.is_some() {
        let _ = h.set("Content-Type", "application/json");
    }
    if let Some(c) = cookie {
        let _ = h.set("Cookie", c);
    }
    let mut init = RequestInit::new();
    init.with_method(method).with_headers(h);
    if let Some(b) = body {
        init.with_body(Some(b.to_string().into()));
    }
    let req = Request::new_with_init(url, &init).map_err(|e| e.to_string())?;
    let mut r = Fetch::Request(req).send().await.map_err(|e| format!("tarmoq: {e}"))?;
    let code = r.status_code();
    let text = r.text().await.unwrap_or_default();
    Ok((code, text))
}

async fn login(env: &Env) -> std::result::Result<String, String> {
    let Some(c) = creds(env) else {
        return Err("login/parol sozlanmagan (ANIBLA_CREDS)".into());
    };
    let (code, text) = http(&format!("{}/api/backend/api/v1/auth/login-password", c.site), Method::Post,
        Some(json!({"phone_number": c.phone, "password": c.password})), None).await?;
    let v: Value = serde_json::from_str(&text).unwrap_or(json!({}));
    let tok = v["token"]["accessToken"].as_str()
        .or_else(|| v["data"]["token"]["accessToken"].as_str())
        .unwrap_or("").to_string();
    if tok.is_empty() {
        let why = v["code"].as_str().or_else(|| v["message"].as_str()).unwrap_or("").to_string();
        return Err(match why.as_str() {
            "invalid_password" => "parol noto'g'ri".into(),
            "user_not_found" => "bunday login yo'q".into(),
            "password_login_rate_limited" => "juda ko'p urinish — birozdan keyin".into(),
            _ => format!("saytga kirib bo'lmadi (HTTP {code} {why})"),
        });
    }
    TOKEN.with(|t| *t.borrow_mut() = tok.clone());
    config_put(env, "anibla_token", &tok).await;
    Ok(tok)
}

async fn token(env: &Env, fresh: bool) -> std::result::Result<String, String> {
    if !fresh {
        let mem = TOKEN.with(|t| t.borrow().clone());
        if !mem.is_empty() {
            return Ok(mem);
        }
        if let Some(t) = config_get(env, "anibla_token").await {
            TOKEN.with(|m| *m.borrow_mut() = t.clone());
            return Ok(t);
        }
    }
    login(env).await
}

/// API'dan JSON. `auth` — login kerak (401 bo'lsa bir marta qayta kiradi).
async fn get(env: &Env, path: &str, auth: bool) -> std::result::Result<Value, String> {
    let url = format!("{}/{}", api_base(env), path.trim_start_matches('/'));
    for attempt in 0..2 {
        let cookie = if auth { Some(format!("access_token={}", token(env, attempt > 0).await?)) } else { None };
        let (code, text) = http(&url, Method::Get, None, cookie.as_deref()).await?;
        if code == 401 && auth && attempt == 0 {
            continue;
        }
        let v: Value = serde_json::from_str(&text).map_err(|_| format!("sayt javobi tushunarsiz (HTTP {code})"))?;
        if code >= 400 || v["success"].as_bool() == Some(false) {
            return Err(format!("sayt: {} (HTTP {code})", v["message"].as_str().unwrap_or("xato")));
        }
        return Ok(v);
    }
    Err("saytga kirib bo'lmadi".into())
}

fn data_list(v: &Value) -> Vec<Value> {
    v["data"].as_array().cloned().unwrap_or_default()
}

fn title_of(v: &Value) -> String {
    let t = v["uz"]["title"].as_str().or_else(|| v["ru"]["title"].as_str()).unwrap_or("").trim().to_string();
    if t.is_empty() { v["slug"].as_str().unwrap_or("?").to_string() } else { t }
}

// ── Navigatsiya holati ───────────────────────────────────────
//
// TALAB (foydalanuvchi): ro'yxatlar PASTKI PANELDAN (reply keyboard), inline
// emas: anime nomlari — qatorga bitta, qismlar — qatorga uchta, sifatlar —
// qatorga bitta. Tugma bosilganda uning MATNI keladi; qaysi ro'yxat ochiqligi
// `anibla_nav` da (`v`: list | seasons | eps | q, tanlangan `i`/`j`/`k`).

#[derive(Default, Serialize, Deserialize, Clone)]
struct Item {
    s: String,
    t: String,
    /// `Series` yoki `Movies`.
    m: String,
    #[serde(default)]
    y: i64,
    #[serde(default)]
    e: i64,
    #[serde(default)]
    d: String,
    #[serde(default)]
    img: String,
    /// Saytdagi to'liq ma'lumot (tayyor HTML: davlat, janr, studiya, ovoz
    /// berganlar, tavsif ...) — faqat tanlangan anime uchun olinadi (`describe`).
    #[serde(default)]
    info: String,
    /// To'liq tavsif (tayyor HTML) — `info` dan alohida: rasm izohiga
    /// sig'masa keyingi xabarga o'tadi (`send_photo_card`).
    #[serde(default)]
    about: String,
}

impl Item {
    fn movie(&self) -> bool {
        !self.m.to_ascii_lowercase().contains("serie")
    }

    /// Ro'yxatdagi tugma matni (shu matn qaytib kelganda aynan shu element).
    fn label(&self) -> String {
        let mut l = format!("{} {}", if self.movie() { "\u{1F3AC}" } else { "\u{1F4FA}" }, self.t);
        if self.y > 0 { l.push_str(&format!(" \u{00B7} {}", self.y)); }
        if !self.movie() && self.e > 0 { l.push_str(&format!(" \u{00B7} {} qism", self.e)); }
        l
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Nav {
    n: String,
    q: String,
    /// Bo'lim (kategoriya) ID si; `new` — oxirgi yuklanganlar. Qidiruvda bo'sh.
    #[serde(default)]
    c: String,
    /// Bo'lim nomi (sarlavha uchun).
    #[serde(default)]
    lt: String,
    p: i64,
    pages: i64,
    total: i64,
    items: Vec<Item>,
    /// Ochiq ro'yxat: `list`, `seasons`, `eps`, `q`.
    #[serde(default)]
    v: String,
    /// Tanlangan anime (items ichida), fasl, qismlar sahifasi, qism.
    #[serde(default)]
    i: i64,
    #[serde(default)]
    j: i64,
    #[serde(default)]
    ep: i64,
    #[serde(default)]
    k: i64,
}

async fn nav_get(env: &Env) -> Nav {
    config_get(env, "anibla_nav").await
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_default()
}

async fn nav_put(env: &Env, nav: &Nav) {
    config_put(env, "anibla_nav", &serde_json::to_string(nav).unwrap_or_default()).await;
}

fn kb(rows: Vec<Vec<Value>>) -> Value {
    json!({"inline_keyboard": rows})
}

fn btn(text: &str, data: String) -> Value {
    json!({"text": text, "callback_data": data})
}

const BTN_NEW: &str = "\u{1F195} Oxirgi yuklanganlar";
const BTN_PREV: &str = "\u{25C0}\u{FE0F} Oldingi";
const BTN_NEXT: &str = "Keyingi \u{25B6}\u{FE0F}";
const BTN_BACK: &str = "\u{2B05}\u{FE0F} Orqaga";

/// Pastki panel: doimiy boshqaruv tugmalari TEPADA (foydalanuvchi talabi:
/// uzun ro'yxatda pastga aylantirmasdan ko'rinsin), keyin berilgan qatorlar.
fn panel(rows: Vec<Vec<String>>, back: bool) -> Value {
    let mut all = if back {
        vec![
            vec![BTN_BACK.to_string(), postbot::BTN_HOME.to_string()],
            vec![BTN_SEARCH.to_string(), BTN_QUEUE.to_string(), ENCBOT_BTN_STATUS.to_string()],
        ]
    } else {
        vec![
            vec![BTN_CATS.to_string(), BTN_YEARS.to_string(), BTN_SEARCH.to_string()],
            vec![BTN_QUEUE.to_string(), ENCBOT_BTN_STATUS.to_string(), postbot::BTN_HOME.to_string()],
        ]
    };
    all.extend(rows);
    encbot_keyboard(all)
}

fn menu_keyboard() -> Value {
    panel(vec![], false)
}

// ── "Ilova uchun" -> "Anibla orqali" ─────────────────────────
//
// TALAB (foydalanuvchi): "Ilova uchun" bo'limida anime va bo'lim tanlanib
// "Yangi qism qo'shish" (yoki qism tugmasi) bosilganda "Anibla orqali" tugmasi;
// bosilsa shu bo'limning o'zi (bo'limlar, qidiruv, fasl, qism) chiqadi. Qism
// tanlanganda sifat SO'RALMAYDI — eng yuqorisi navbatga tushadi, Actions uni
// yuklab yopiq kanalga qo'yadi, `done` esa uni botga yuborilgan videodek
// qismning asl videosi qiladi (`encbot_register`): kodlash navbati -> H.265,
// 4 sifat, MP4 + fMP4 (`tool/encode/run.py`).
// `anibla_target`: "anime/bo'lim" (har tanlangan qism — navbatdagi yangi qism)
// yoki "anime/bo'lim/raqam" (bitta qism almashtiriladi, keyin rejim o'chadi).

async fn app_target(env: &Env) -> Option<(String, String)> {
    let t = config_get(env, "anibla_target").await.filter(|t| !t.is_empty())?;
    let label = config_get(env, "anibla_target_label").await.unwrap_or_default();
    Some((t, label))
}

pub(crate) async fn menu_for_app(env: &Env, chat: i64, target: &str, label: &str) {
    config_put(env, "anibla_target", target).await;
    config_put(env, "anibla_target_label", label).await;
    config_put(env, "encbot_mode", "anibla").await;
    let warn = if creds(env).is_none() {
        "\n\n\u{26A0}\u{FE0F} anibla.uz login/paroli sozlanmagan (<code>tool/anibla/creds.enc</code>)."
    } else { "" };
    let panel = home_panel(env).await;
    encbot_send(env, chat, &format!(
        "\u{1F4F1} <b>Ilova uchun \u{2014} Anibla orqali</b>\n\u{1F3AF} {}\n\n\
         \u{1F4C2} Pastdagi bo'limlardan tanlang yoki anime nomini yozing. Anime \u{2192} fasl \u{2192} qism: \
         sifat so'ralmaydi \u{2014} eng yuqorisi yuklanadi, keyin H.265 da 4 sifatga (MP4 va fMP4) kodlanadi.\n\
         \u{21A9}\u{FE0F} Chiqish: \u{00AB}Bosh menyu\u{00BB}.{warn}", html_escape(label)),
        Some(panel)).await;
}

/// Ilova rejimida qism (yoki film) tanlandi: eng yuqori sifat navbatga.
async fn pick_best(env: &Env, chat: i64, mut nav: Nav, rows: &[(String, i64)]) {
    let h = rows.iter().map(|(_, h)| *h).max().unwrap_or(0);
    nav.v = "q".into();
    nav_put(env, &nav).await;
    pick_quality(env, chat, &nav, h).await;
}

// ── Bot: menyu, bo'limlar va qidiruv ─────────────────────────

/// Bosh panel: "Oxirgi yuklanganlar" va saytdagi bo'limlar (2 tadan).
async fn home_panel(env: &Env) -> Value {
    let mut rows = vec![vec![BTN_NEW.to_string()]];
    let cats = categories(env).await;
    for ch in cats.chunks(2) {
        rows.push(ch.iter().map(|(_, name)| format!("\u{1F4C2} {name}")).collect());
    }
    panel(rows, false)
}

pub(crate) async fn menu(env: &Env, chat: i64) {
    // Oddiy "Anibla yuklash" — video bot chatiga keladi (ilovaga emas).
    if app_target(env).await.is_some() {
        config_put(env, "anibla_target", "").await;
    }
    config_put(env, "encbot_mode", "anibla").await;
    let warn = if creds(env).is_none() {
        "\n\n\u{26A0}\u{FE0F} Login/parol hali sozlanmagan: <code>tool/anibla/creds.enc</code> \
         <code>ENCODE_TOKEN</code> bilan ochilmadi (deploy log'iga qarang)."
    } else { "" };
    let panel = home_panel(env).await;
    encbot_send(env, chat, &format!(
        "\u{1F39E} <b>Anibla.uz yuklash</b>\n\n\
         \u{1F4C2} Pastdagi bo'limlardan tanlang yoki anime/film nomini yozing (kamida 3 harf).\n\
         Anime \u{2192} fasl va qism \u{2192} sifat. Video navbatga tushadi, GitHub Actions orqali \
         ketma-ket yuklanib, shu chatga yuboriladi.{warn}"),
        Some(panel)).await;
}

/// Saytdagi bo'limlar: `GET categories` (Ongoing, Hamma animelar, Anime
/// filmlar, ...) — ro'yxat saytdan olinadi, kodga yozilmagan.
async fn categories(env: &Env) -> Vec<(String, String)> {
    let v = get(env, "categories?sortBy=name.uz&sortDirection=-1", false).await.unwrap_or(json!({}));
    let list = v["data"]["categories"].as_array().or_else(|| v["data"].as_array()).cloned().unwrap_or_default();
    list.iter().filter(|c| c["status"].as_bool() != Some(false)).filter_map(|c| {
        let id = c["_id"].as_str()?.to_string();
        let name = c["name"]["uz"].as_str().or_else(|| c["name"].as_str()).unwrap_or("").trim().to_string();
        (!id.is_empty() && !name.is_empty()).then_some((id, name))
    }).collect()
}

/// Pastki paneldagi tugma (yoki yozilgan matn). Anibla rejimida hamma matn shu yerga.
pub(crate) async fn on_message(env: &Env, msg: &Value, chat: i64) {
    let text = msg["text"].as_str().unwrap_or("").trim().to_string();
    if text == BTN_QUEUE {
        queue_list(env, chat).await;
        return;
    }
    if text == BTN_CATS {
        let panel = home_panel(env).await;
        encbot_send(env, chat, "\u{1F4C2} <b>Bo'limlar</b> (saytdagidek) \u{2014} pastdan tanlang:", Some(panel)).await;
        return;
    }
    if text.is_empty() || text == BTN_SEARCH {
        encbot_send(env, chat, "\u{270D}\u{FE0F} Anime yoki film nomini yozing:", None).await;
        return;
    }
    if text == BTN_NEW {
        listing(env, chat, "", "new", "Oxirgi yuklanganlar", 1).await;
        return;
    }
    if text == BTN_YEARS {
        years_panel(env, chat).await;
        return;
    }
    // "📅 2026-yil · 53 ta anime" — shu yilning animelari.
    if let Some(rest) = text.strip_prefix(YEAR_PREFIX) {
        if let Some(y) = rest.split("-yil").next().and_then(|n| n.trim().parse::<i64>().ok()) {
            if rest.contains("-yil") && (1900..=2200).contains(&y) {
                listing(env, chat, "", &format!("y:{y}"), &format!("{y}-yil"), 1).await;
                return;
            }
        }
    }
    if let Some(name) = text.strip_prefix("\u{1F4C2} ") {
        if let Some((id, n)) = categories(env).await.into_iter().find(|(_, n)| n == name.trim()) {
            listing(env, chat, "", &id, &n, 1).await;
            return;
        }
    }

    let mut nav = nav_get(env).await;
    if text == BTN_PREV || text == BTN_NEXT {
        let d = if text == BTN_NEXT { 1 } else { -1 };
        if nav.v == "eps" {
            nav.ep = (nav.ep + d).max(0);
            show_eps(env, chat, nav, false).await;
        } else if !nav.n.is_empty() {
            let page = (nav.p + d).clamp(1, nav.pages.max(1));
            listing(env, chat, &nav.q, &nav.c, &nav.lt, page).await;
        }
        return;
    }
    if text == BTN_BACK {
        let movie = usize::try_from(nav.i).ok().and_then(|i| nav.items.get(i)).map(|it| it.movie()).unwrap_or(true);
        match nav.v.as_str() {
            "q" if !movie => show_eps(env, chat, nav, false).await,
            "eps" => {
                let it = usize::try_from(nav.i).ok().and_then(|i| nav.items.get(i)).cloned();
                let many = match &it { Some(it) => seasons(env, &it.s).await.map(|s| s.len() > 1).unwrap_or(false), None => false };
                if many { show_seasons(env, chat, nav).await } else { list_view(env, chat, nav).await }
            }
            "seasons" | "q" => list_view(env, chat, nav).await,
            _ => {
                let panel = home_panel(env).await;
                encbot_send(env, chat, "\u{1F4C2} Bo'limlar:", Some(panel)).await;
            }
        }
        return;
    }
    // Ro'yxatdagi anime/film.
    if let Some(i) = nav.items.iter().position(|it| it.label() == text) {
        nav.i = i as i64;
        show_item(env, chat, nav).await;
        return;
    }
    // Fasl: "🗂 2-fasl".
    if let Some(name) = text.strip_prefix("\u{1F5C2} ") {
        if let Some(it) = usize::try_from(nav.i).ok().and_then(|i| nav.items.get(i)).cloned() {
            if let Ok(ss) = seasons(env, &it.s).await {
                if let Some(j) = ss.iter().position(|s| season_label(s) == name.trim()) {
                    nav.j = j as i64;
                    nav.ep = 0;
                    show_eps(env, chat, nav, true).await;
                    return;
                }
            }
        }
    }
    // Qism: "▶️ 12-qism".
    if let Some(rest) = text.strip_prefix("\u{25B6}\u{FE0F} ") {
        if let Some(num) = rest.strip_suffix("-qism").and_then(|n| n.trim().parse::<i64>().ok()) {
            show_qualities(env, chat, nav, num).await;
            return;
        }
    }
    // Sifat: "⬇️ 720p · ~250 MB" (`asl sifat` — 0).
    if let Some(rest) = text.strip_prefix("\u{2B07}\u{FE0F} ") {
        let h: i64 = rest.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(0);
        pick_quality(env, chat, &nav, h).await;
        return;
    }
    if text.chars().count() < 3 {
        encbot_send(env, chat, "Kamida 3 ta harf yozing.", None).await;
        return;
    }
    let q: String = text.chars().take(80).collect();
    listing(env, chat, &q, "", "", 1).await;
}

/// Saytdagi animelar yillar bo'yicha: `(yil, soni)`, yangi yil tepada.
/// Sayt ro'yxatidan (`media/mobile`, hammasi) hisoblanadi va 6 soat saqlanadi.
async fn year_counts(env: &Env) -> Vec<(i64, i64)> {
    const TTL: i64 = 6 * 3600 * 1000;
    if let Some(j) = config_get(env, "anibla_years").await.and_then(|v| serde_json::from_str::<Value>(&v).ok()) {
        if now_ms() - j["at"].as_i64().unwrap_or(0) < TTL {
            let l: Vec<(i64, i64)> = j["l"].as_array().cloned().unwrap_or_default().iter()
                .filter_map(|x| Some((x[0].as_i64()?, x[1].as_i64()?))).collect();
            if !l.is_empty() { return l; }
        }
    }
    let mut counts: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    for page in 1..=4 {
        let Ok(v) = get(env, &format!("media/mobile?limit=500&page={page}"), false).await else { break };
        for x in data_list(&v) {
            let y = jint(&x, "published_year");
            if y > 0 { *counts.entry(y).or_insert(0) += 1; }
        }
        if page >= jint(&v["pagination"], "pages").max(1) { break; }
    }
    let mut l: Vec<(i64, i64)> = counts.into_iter().collect();
    l.sort_by(|a, b| b.0.cmp(&a.0));
    if !l.is_empty() {
        config_put(env, "anibla_years", &json!({"at": now_ms(), "l": l}).to_string()).await;
    }
    l
}

/// "📅 Yil bo'yicha": saytdagi yillar va har yildagi animelar soni (tugma — shu yil ro'yxati).
async fn years_panel(env: &Env, chat: i64) {
    let l = year_counts(env).await;
    if l.is_empty() {
        encbot_send(env, chat, "\u{274C} Yillar ro'yxatini olib bo'lmadi, birozdan keyin qayta urinib ko'ring.", None).await;
        return;
    }
    let total: i64 = l.iter().map(|x| x.1).sum();
    let rows: Vec<Vec<String>> = l.chunks(2).map(|ch| {
        ch.iter().map(|(y, n)| format!("{YEAR_PREFIX}{y}-yil \u{00B7} {n} ta anime")).collect()
    }).collect();
    let text = format!("\u{1F4C5} <b>Yil bo'yicha</b> \u{2014} saytda jami {total} ta anime, {} ta yil.\n\n\
                        \u{2B07}\u{FE0F} Yilni tanlang:", l.len());
    encbot_send(env, chat, &text, Some(panel(rows, false))).await;
}

/// Ro'yxat sahifasi: qidiruv (`q`) yoki bo'lim (`cat`: ID yoki `new`).
async fn listing(env: &Env, chat: i64, q: &str, cat: &str, label: &str, page: i64) {
    let mut path = format!("media/mobile?limit={SEARCH_PAGE}&page={page}");
    if !q.is_empty() {
        path.push_str(&format!("&search={}", enc(q)));
    } else if let Some(y) = cat.strip_prefix("y:") {
        path.push_str(&format!("&years={}", enc(y)));
    } else if !cat.is_empty() && cat != "new" {
        path.push_str(&format!("&categories={}", enc(cat)));
    }
    let v = match get(env, &path, false).await {
        Ok(v) => v,
        Err(e) => {
            encbot_send(env, chat, &format!("\u{274C} Ro'yxatni olib bo'lmadi: {}", html_escape(&e)), None).await;
            return;
        }
    };
    let items: Vec<Item> = data_list(&v).iter().map(|x| Item {
        s: x["slug"].as_str().unwrap_or("").to_string(),
        t: title_of(x),
        m: x["mediaType"].as_str().unwrap_or("").to_string(),
        y: jint(x, "published_year"),
        e: jint(x, "available_episodes"),
        // Tavsif ro'yxatda SAQLANMAYDI (500 ta anime bazadagi yozuvni shishirardi):
        // anime tanlanganda alohida olinadi (`describe`).
        d: String::new(),
        info: String::new(),
        about: String::new(),
        img: x["thumbnail"].as_str().unwrap_or("").trim_start_matches('/').to_string(),
    }).filter(|i| !i.s.is_empty()).collect();
    if items.is_empty() {
        let t = if q.is_empty() {
            format!("\u{1F937} <b>{}</b> bo'limida hozircha hech narsa yo'q.", html_escape(label))
        } else {
            format!("\u{1F937} <b>\u{00AB}{}\u{00BB}</b> bo'yicha hech narsa topilmadi.\n\
                     Boshqacha yozib ko'ring (masalan, o'zbekcha nomining bir qismi).", html_escape(q))
        };
        encbot_send(env, chat, &t, None).await;
        return;
    }
    let pages = jint(&v["pagination"], "pages").max(1);
    let total = jint(&v["pagination"], "total").max(items.len() as i64);
    let nav = Nav {
        n: (now_ms() % 1_000_000).to_string(),
        q: q.to_string(), c: cat.to_string(), lt: label.to_string(), p: page, pages, total, items,
        ..Default::default()
    };
    list_view(env, chat, nav).await;
}

/// Saqlangan ro'yxatni (qayta so'ramasdan) pastki panelga chiqaradi.
async fn list_view(env: &Env, chat: i64, mut nav: Nav) {
    nav.v = "list".into();
    let mut top: Vec<Vec<String>> = Vec::new();
    let mut nr = Vec::new();
    if nav.p > 1 { nr.push(BTN_PREV.to_string()); }
    if nav.p < nav.pages { nr.push(BTN_NEXT.to_string()); }
    if !nr.is_empty() { top.push(nr); }
    let items: Vec<Vec<String>> = nav.items.iter().map(|it| vec![it.label()]).collect();
    let head = if nav.q.is_empty() {
        format!("\u{1F4C2} <b>{}</b> \u{2014} {} ta", html_escape(&nav.lt), nav.total)
    } else {
        format!("\u{1F50D} <b>\u{00AB}{}\u{00BB}</b> \u{2014} {} ta natija", html_escape(&nav.q), nav.total)
    };
    let page = if nav.pages > 1 { format!(" \u{00B7} {}/{}-sahifa", nav.p, nav.pages) } else { String::new() };
    let text = format!("{head}{page}\n\u{1F4FA} serial \u{00B7} \u{1F3AC} film\n\n\
                        \u{2B07}\u{FE0F} Pastdan tanlang:");
    nav_put(env, &nav).await;
    send_list(env, chat, &text, top, items, false, None).await;
}

// ── Bot: serial / film ───────────────────────────────────────

struct Season {
    slug: String,
    title: String,
}

async fn seasons(env: &Env, slug: &str) -> std::result::Result<Vec<Season>, String> {
    let v = get(env, &format!("seasons/{}", enc(slug)), false).await?;
    Ok(data_list(&v).iter().map(|s| Season {
        slug: s["slug"].as_str().unwrap_or("").to_string(),
        title: title_of(s),
    }).filter(|s| !s.slug.is_empty()).collect())
}

struct Episode {
    slug: String,
    num: i64,
    title: String,
}

async fn episodes(env: &Env, slug: &str, season: &str) -> std::result::Result<Vec<Episode>, String> {
    let v = get(env, &format!("episodes/{}/{}?view=compact", enc(slug), enc(season)), false).await?;
    let mut eps: Vec<Episode> = data_list(&v).iter().enumerate().map(|(i, e)| Episode {
        slug: e["slug"].as_str().unwrap_or("").to_string(),
        num: Some(jint(e, "episode_number")).filter(|n| *n > 0).unwrap_or(i as i64 + 1),
        title: e["uz"]["title"].as_str().unwrap_or("").trim().to_string(),
    }).filter(|e| !e.slug.is_empty()).collect();
    eps.sort_by_key(|e| e.num);
    Ok(eps)
}

fn season_label(s: &Season) -> String {
    if s.title.chars().all(|c| c.is_ascii_digit()) { format!("{}-fasl", s.title) } else { s.title.clone() }
}

fn cur_item(nav: &Nav) -> Option<Item> {
    usize::try_from(nav.i).ok().and_then(|i| nav.items.get(i)).cloned()
}

fn item_head(it: &Item, desc: bool) -> String {
    let mut head = format!("{} <b>{}</b>", if it.movie() { "\u{1F3AC}" } else { "\u{1F4FA}" }, html_escape(&it.t));
    if it.y > 0 { head.push_str(&format!(" ({})", it.y)); }
    if desc && !it.info.is_empty() {
        head.push_str(&format!("\n\n{}", it.info));
    }
    if desc && !it.about.is_empty() {
        head.push_str(&format!("\n\n{}", it.about));
    }
    head
}

/// Telegram hisoblaydigan uzunlik (HTML teglari va `&amp;` kabilar hisobga kirmaydi).
fn visible_len(html: &str) -> usize {
    let mut n = 0;
    let mut tag = false;
    let mut ent = false;
    for c in html.chars() {
        match c {
            '<' => tag = true,
            '>' if tag => tag = false,
            _ if tag => {}
            '&' => { ent = true; n += 1; }
            ';' if ent => ent = false,
            _ if ent => {}
            _ => n += 1,
        }
    }
    n
}

/// `[{name: {uz, ru}}]` yoki `[{name: "..."}]` ro'yxatidan nomlar.
fn names(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|x| {
        let n = &x["name"];
        n["uz"].as_str().or_else(|| n["ru"].as_str()).or_else(|| n.as_str())
            .map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
    }).collect()).unwrap_or_default()
}

/// Son yoki satr ko'rinishidagi butun son (`age`: 13 yoki "17").
fn num_of(v: &Value) -> i64 {
    v.as_i64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())).unwrap_or(0)
}

/// Tavsif uzunligi chegarasi: butun xabar Telegram'ning 4096 belgilik
/// chegarasiga sig'ishi uchun (sarlavha, ma'lumot va izohlarga joy qoladi).
const DESC_MAX: usize = 2600;

/// Tanlangan anime/film haqida saytdagi TO'LIQ ma'lumot (tayyor HTML).
///
/// TALAB (foydalanuvchi): anime tanlanganda rasm tagida saytdagi to'liq
/// ma'lumot chiqsin — davlat, yil, janrlar, studiya, rejissyor, ovoz
/// berganlar, yosh chegarasi, qismlar, bo'limlar, treyler va to'liq tavsif.
async fn describe(env: &Env, it: &Item) -> (String, String) {
    let path = format!("{}/{}", if it.movie() { "movies" } else { "series" }, enc(&it.s));
    let Ok(v) = get(env, &path, false).await else { return Default::default() };
    let d = if v["data"].is_array() { &v["data"][0] } else { &v["data"] };
    let mut lines: Vec<String> = Vec::new();
    let mut add = |icon: &str, key: &str, val: String| {
        if !val.trim().is_empty() {
            lines.push(format!("{icon} <b>{key}:</b> {}", html_escape(val.trim())));
        }
    };
    let alt = d["ru"]["title"].as_str().unwrap_or("").trim().to_string();
    if !alt.is_empty() && alt != it.t { add("\u{1F524}", "Boshqa nomi", alt); }
    let c = &d["country"]["name"];
    add("\u{1F30D}", "Davlat", c["uz"].as_str().or_else(|| c["ru"].as_str()).unwrap_or("").to_string());
    let year = num_of(&d["published_year"]);
    if year > 0 { add("\u{1F4C5}", "Yili", year.to_string()); }
    add("\u{1F3AD}", "Janrlar", names(&d["genres"]).join(", "));
    add("\u{1F3E2}", "Studiya", d["studio"]["name"].as_str().unwrap_or("").to_string());
    add("\u{1F3AC}", "Rejissyor", d["director"]["name"].as_str().unwrap_or("").to_string());
    add("\u{1F399}", "Ovoz berganlar", names(&d["creators"]).join(", "));
    let age = num_of(&d["age"]);
    if age > 0 { add("\u{1F51E}", "Yosh chegarasi", format!("{age}+")); }
    if it.movie() {
        let dur = num_of(&d["duration"]);
        if dur > 0 { add("\u{23F1}", "Davomiyligi", format!("{dur} daqiqa")); }
    } else {
        let total = num_of(&d["total_episodes"]);
        let eps = match (it.e, total) {
            (a, t) if a > 0 && t > 0 && a != t => format!("{a} ta chiqqan / jami {t} ta"),
            (a, _) if a > 0 => format!("{a} ta"),
            (_, t) if t > 0 => format!("{t} ta"),
            _ => String::new(),
        };
        add("\u{1F4FA}", "Qismlar", eps);
    }
    match d["type"].as_str().unwrap_or("") {
        "paid" => add("\u{1F4B0}", "Turi", "Pullik".into()),
        "free" => add("\u{1F193}", "Turi", "Bepul".into()),
        _ => {}
    }
    let cats: Vec<String> = names(&d["categories"]).into_iter()
        .filter(|n| !n.to_lowercase().starts_with("hamma")).collect();
    add("\u{1F4C2}", "Bo'limlar", cats.join(", "));
    let mut out = lines.join("\n");
    let tr = d["trailer"].as_str().unwrap_or("").trim();
    if tr.starts_with("http") {
        let tr = tr.replace("youtube.com/embed/", "youtu.be/").replace("www.youtu.be/", "youtu.be/");
        out.push_str(&format!("\n\u{25B6}\u{FE0F} <a href=\"{}\">Treyler</a>", html_escape(&tr)));
    }
    let mut about = String::new();
    let desc = d["uz"]["description"].as_str().filter(|s| !s.trim().is_empty())
        .or_else(|| d["ru"]["description"].as_str()).unwrap_or("").trim();
    if !desc.is_empty() {
        let mut t: String = desc.chars().take(DESC_MAX).collect();
        if desc.chars().count() > DESC_MAX { t.push('\u{2026}'); }
        about = format!("\u{1F4DD} <b>Tavsif:</b>\n{}", html_escape(&t));
    }
    (out, about)
}

async fn show_item(env: &Env, chat: i64, mut nav: Nav) {
    if let Some(i) = usize::try_from(nav.i).ok().filter(|i| *i < nav.items.len()) {
        if nav.items[i].info.is_empty() {
            let (info, about) = describe(env, &nav.items[i]).await;
            nav.items[i].info = info;
            nav.items[i].about = about;
        }
    }
    let Some(it) = cur_item(&nav) else { return };
    if it.movie() {
        nav.j = -1;
        nav.k = -1;
        let head = item_head(&it, true);
        match qualities_text(env, &it, None).await {
            Ok((_, rows)) if app_target(env).await.is_some() => pick_best(env, chat, nav, &rows).await,
            Ok((t, rows)) => {
                nav.v = "q".into();
                nav_put(env, &nav).await;
                let rows = rows.into_iter().map(|(l, _)| vec![l]).collect();
                send_card(env, chat, &it, &format!("{head}\n\n{t}"), panel(rows, true)).await;
            }
            Err(e) => encbot_send(env, chat, &format!("{head}\n\n\u{274C} {}", html_escape(&e)), None).await,
        }
        return;
    }
    match seasons(env, &it.s).await {
        Ok(ss) if ss.len() > 1 => {
            // Rasm, tavsif va fasl tugmalari BITTA xabarda (foydalanuvchi
            // talabi: "anime ustiga bosilganda rasmi bilan chiqsin").
            show_seasons(env, chat, nav).await;
        }
        Ok(ss) if ss.len() == 1 => {
            nav.j = 0;
            nav.ep = 0;
            show_eps(env, chat, nav, true).await;
        }
        Ok(_) => encbot_send(env, chat, &format!("{}\n\n\u{1F937} Fasllar topilmadi.", item_head(&it, false)), None).await,
        Err(e) => encbot_send(env, chat, &format!("{}\n\n\u{274C} {}", item_head(&it, false), html_escape(&e)), None).await,
    }
}

async fn show_seasons(env: &Env, chat: i64, mut nav: Nav) {
    if let Some(i) = usize::try_from(nav.i).ok().filter(|i| *i < nav.items.len()) {
        if nav.items[i].info.is_empty() {
            let (info, about) = describe(env, &nav.items[i]).await;
            nav.items[i].info = info;
            nav.items[i].about = about;
        }
    }
    let Some(it) = cur_item(&nav) else { return };
    let ss = seasons(env, &it.s).await.unwrap_or_default();
    nav.v = "seasons".into();
    nav_put(env, &nav).await;
    let rows: Vec<Vec<String>> = ss.chunks(2)
        .map(|ch| ch.iter().map(|s| format!("\u{1F5C2} {}", season_label(s))).collect()).collect();
    let text = format!("{}\n\n{} ta fasl \u{2014} pastdan tanlang:", item_head(&it, true), ss.len());
    send_card(env, chat, &it, &text, panel(rows, true)).await;
}

/// Qismlar: qatorga 3 tadan, `EP_PAGE` tadan sahifa. `card` — muqova bilan.
async fn show_eps(env: &Env, chat: i64, mut nav: Nav, card: bool) {
    let Some(it) = cur_item(&nav) else { return };
    let ss = match seasons(env, &it.s).await {
        Ok(ss) => ss,
        Err(e) => { encbot_send(env, chat, &format!("\u{274C} {}", html_escape(&e)), None).await; return; }
    };
    let Some(season) = usize::try_from(nav.j).ok().and_then(|j| ss.get(j)) else { return };
    let mut eps = match episodes(env, &it.s, &season.slug).await {
        Ok(e) if !e.is_empty() => e,
        Ok(_) => { encbot_send(env, chat, "\u{1F937} Bu faslda qismlar topilmadi.", None).await; return; }
        Err(e) => { encbot_send(env, chat, &format!("\u{274C} {}", html_escape(&e)), None).await; return; }
    };
    // Teskari tartib (foydalanuvchi talabi): eng yangi qism birinchi — 10, 9, 8 ...
    eps.reverse();
    let pages = eps.len().div_ceil(EP_PAGE).max(1) as i64;
    nav.ep = nav.ep.clamp(0, pages - 1);
    nav.v = "eps".into();
    nav_put(env, &nav).await;
    let start = nav.ep as usize * EP_PAGE;
    let part = &eps[start..(start + EP_PAGE).min(eps.len())];
    let mut top: Vec<Vec<String>> = Vec::new();
    let mut nr = Vec::new();
    if nav.ep > 0 { nr.push(BTN_PREV.to_string()); }
    if nav.ep + 1 < pages { nr.push(BTN_NEXT.to_string()); }
    if !nr.is_empty() { top.push(nr); }
    let rows: Vec<Vec<String>> = part.chunks(3)
        .map(|ch| ch.iter().map(|e| format!("\u{25B6}\u{FE0F} {}-qism", e.num)).collect::<Vec<_>>()).collect();
    let text = format!("{}\n{} \u{00B7} {} ta qism{}\n\n\u{2B07}\u{FE0F} Qismni pastdagi paneldan tanlang (eng yangisi birinchi):",
        item_head(&it, card), html_escape(&season_label(season)), eps.len(),
        if pages > 1 { format!(" \u{00B7} {}\u{2013}{}-qismlar", part[0].num, part[part.len() - 1].num) } else { String::new() });
    send_list(env, chat, &text, top, rows, true, if card { Some(&it) } else { None }).await;
}

/// Qism tanlandi: sifatlar (qatorga bitta).
async fn show_qualities(env: &Env, chat: i64, mut nav: Nav, num: i64) {
    let Some(it) = cur_item(&nav) else { return };
    let Ok(ss) = seasons(env, &it.s).await else { return };
    let Some(season) = usize::try_from(nav.j).ok().and_then(|j| ss.get(j)) else { return };
    let eps = episodes(env, &it.s, &season.slug).await.unwrap_or_default();
    let Some(k) = eps.iter().position(|e| e.num == num) else {
        encbot_send(env, chat, &format!("\u{1F937} {num}-qism topilmadi."), None).await;
        return;
    };
    let e = &eps[k];
    let head = format!("\u{1F4FA} <b>{}</b>\n{} \u{00B7} {}-qism{}", html_escape(&it.t),
        html_escape(&season_label(season)), e.num,
        if e.title.is_empty() { String::new() } else { format!(": {}", html_escape(&e.title)) });
    match qualities_text(env, &it, Some((&season.slug, e))).await {
        Ok((_, rows)) if app_target(env).await.is_some() => {
            nav.k = k as i64;
            pick_best(env, chat, nav, &rows).await;
        }
        Ok((t, rows)) => {
            nav.k = k as i64;
            nav.v = "q".into();
            nav_put(env, &nav).await;
            let rows = rows.into_iter().map(|(l, _)| vec![l]).collect();
            encbot_send(env, chat, &format!("{head}\n\n{t}"), Some(panel(rows, true))).await;
        }
        Err(err) => encbot_send(env, chat, &format!("{head}\n\n\u{274C} {}", html_escape(&err)), None).await,
    }
}

/// Sifat tanlandi — navbatga.
///
/// TALAB (foydalanuvchi): sifat bosilganda bot DARHOL javob bersin — navbatga
/// qo'shildimi yoki nima xato bo'ldi. Shu sabab birinchi ish — oddiy matnli
/// (HTML'siz, rad etilmaydigan) "qabul qilindi" xabari; keyingi har qanday
/// natija (navbat raqami yoki xato sababi) shu xabarni tahrirlaydi.
async fn pick_quality(env: &Env, chat: i64, nav: &Nav, height: i64) {
    let status = encbot_api(env, "sendMessage", json!({
        "chat_id": chat,
        "text": format!("\u{23F3} {} qabul qilindi \u{2014} navbatga qo'shilmoqda...", quality_name(height)),
    })).await.ok().and_then(|v| v["message_id"].as_i64()).unwrap_or(0);
    let fail = |why: &str| {
        let why = why.to_string();
        async move {
            let body = json!({"chat_id": chat, "message_id": status,
                "text": format!("\u{274C} Navbatga qo'shilmadi: {why}")});
            if status == 0 || encbot_api(env, "editMessageText", body.clone()).await.is_err() {
                let _ = encbot_api(env, "sendMessage", json!({"chat_id": chat, "text": body["text"]})).await;
            }
        }
    };
    let Some(it) = cur_item(nav) else {
        fail("tanlangan anime topilmadi \u{2014} ro'yxatdan qaytadan tanlang.").await;
        return;
    };
    if nav.v != "q" {
        fail("avval anime va qismni tanlang.").await;
        return;
    }
    if it.movie() {
        start(env, chat, status, &it, None, height).await;
        return;
    }
    let ss = match seasons(env, &it.s).await {
        Ok(ss) => ss,
        Err(e) => { fail(&format!("fasllarni olib bo'lmadi ({e}).")).await; return; }
    };
    let Some(season) = usize::try_from(nav.j).ok().and_then(|j| ss.get(j)) else {
        fail("fasl topilmadi \u{2014} qaytadan tanlang.").await;
        return;
    };
    let eps = match episodes(env, &it.s, &season.slug).await {
        Ok(e) => e,
        Err(e) => { fail(&format!("qismlarni olib bo'lmadi ({e}).")).await; return; }
    };
    let Some(e) = usize::try_from(nav.k).ok().and_then(|k| eps.get(k)) else {
        fail("qism topilmadi \u{2014} qismni qaytadan tanlang.").await;
        return;
    };
    let label = format!("{} \u{00B7} {}-qism{}", season_label(season), e.num,
        if e.title.is_empty() || e.title == e.num.to_string() { String::new() } else { format!(": {}", e.title) });
    start(env, chat, status, &it, Some((&season.slug, e, label)), height).await;
}

/// Rasm izohiga sig'masa: (izoh — sarlavha + ma'lumotlar, qolgan matn).
/// Sig'sa — `None`.
fn card_split(it: &Item, text: &str) -> Option<(String, String)> {
    if visible_len(text) <= 1024 {
        return None;
    }
    let mut cap = item_head(&Item { about: String::new(), ..it.clone() }, true);
    if !text.starts_with(&cap) || visible_len(&cap) > 1024 {
        cap = item_head(it, false);
    }
    let rest = text.strip_prefix(cap.as_str()).unwrap_or(text).trim_start().to_string();
    Some((cap, rest))
}

/// Muqovali xabar — HAQIQIY rasm (`sendPhoto`, galereyaga saqlasa bo'ladi).
///
/// Rasm izohi (caption) Telegram'da 1024 belgigacha. Sig'sa — hammasi bitta
/// xabarda. Sig'masa: rasm izohida sarlavha va ma'lumotlar, tavsif va qolgan
/// matn (tugmalar bilan) darhol keyingi xabarda. Muvaffaqiyatini qaytaradi.
async fn send_photo_card(env: &Env, chat: i64, it: &Item, text: &str, markup: Option<&Value>) -> bool {
    if it.img.is_empty() {
        return false;
    }
    let photo = |caption: &str, markup: Option<&Value>| {
        let mut body = json!({"chat_id": chat, "photo": img_url(&it.img), "caption": caption, "parse_mode": "HTML"});
        if let Some(m) = markup {
            body["reply_markup"] = m.clone();
        }
        body
    };
    let Some((cap, rest)) = card_split(it, text) else {
        return encbot_api(env, "sendPhoto", photo(text, markup)).await.is_ok();
    };
    if encbot_api(env, "sendPhoto", photo(&cap, None)).await.is_err() {
        return false;
    }
    let mut body = json!({"chat_id": chat, "text": rest, "parse_mode": "HTML", "disable_web_page_preview": true});
    if let Some(m) = markup {
        body["reply_markup"] = m.clone();
    }
    // Rasm ketdi — ikkinchi xabar o'tmasa ham `true` (aks holda chaqiruvchi
    // hammasini rasmsiz qayta yuborib, takror chiqarardi).
    let _ = encbot_api(env, "sendMessage", body).await;
    true
}

/// Xabar (muqova bilan yoki oddiy) yuboradi; muvaffaqiyatini qaytaradi.
async fn try_send(env: &Env, chat: i64, text: &str, markup: &Value, card: Option<&Item>) -> bool {
    if let Some(it) = card {
        if send_photo_card(env, chat, it, text, Some(markup)).await {
            return true;
        }
    }
    encbot_api(env, "sendMessage", json!({
        "chat_id": chat, "text": text, "parse_mode": "HTML",
        "disable_web_page_preview": true, "reply_markup": markup,
    })).await.is_ok()
}

/// Uzun ro'yxatni pastki panelga chiqaradi. Telegram panelga tugmalar sonini
/// cheklaydi (aniq chegarasi hujjatlashtirilmagan) — rad etsa, avtomatik
/// kamroq tugma bilan qayta urinadi va qolganini nom yozib qidirishni aytadi.
async fn send_list(env: &Env, chat: i64, text: &str, top: Vec<Vec<String>>, items: Vec<Vec<String>>,
                   back: bool, card: Option<&Item>) {
    let total: usize = items.iter().map(|r| r.len()).sum();
    // Rasm izohiga sig'maydigan uzun karta: rasm (ma'lumotlar bilan) BIR MARTA
    // oldin ketadi, panel esa keyingi xabarda — Telegram tugmalarni rad etsa,
    // pastdagi qayta urinishlar rasmni takror yubormaydi.
    let mut text = text.to_string();
    let mut card = card;
    if let Some(it) = card {
        if let Some((c, rest)) = card_split(it, &text) {
            let r = encbot_api(env, "sendPhoto", json!({
                "chat_id": chat, "photo": img_url(&it.img), "caption": c, "parse_mode": "HTML",
            })).await;
            if r.is_ok() {
                text = rest;
                card = None;
            }
        }
    }
    let text = text.as_str();
    for cap in [usize::MAX, 150, 100, 60] {
        let mut rows = top.clone();
        let mut shown = 0usize;
        for r in &items {
            if shown + r.len() > cap { break; }
            shown += r.len();
            rows.push(r.clone());
        }
        let t = if shown < total {
            format!("{text}\n\n\u{26A0}\u{FE0F} Telegram paneli cheklovi: {total} tadan faqat birinchi {shown} tasi \
                     ko'rsatildi. Qolganini nom yozib qidiring.")
        } else { text.to_string() };
        if try_send(env, chat, &t, &panel(rows, back), card).await {
            return;
        }
    }
    encbot_send(env, chat, "\u{274C} Ro'yxatni yuborib bo'lmadi. Nom yozib qidiring.", None).await;
}

/// Muqova rasmi bilan (bo'lmasa oddiy matn).
async fn send_card(env: &Env, chat: i64, it: &Item, text: &str, markup: Value) {
    let markup = if markup["remove_keyboard"].is_boolean() { None } else { Some(markup) };
    if send_photo_card(env, chat, it, text, markup.as_ref()).await {
        return;
    }
    encbot_send(env, chat, text, markup).await;
}

// ── Video va sifatlar ────────────────────────────────────────

struct Variant {
    height: i64,
    bandwidth: i64,
    url: String,
    /// Alohida ovoz yo'li (`#EXT-X-MEDIA:TYPE=AUDIO`) — bo'lsa, video
    /// bo'laklaridagi ichki ovoz EMAS, shu ishlatiladi.
    audio: Option<String>,
    /// Ovoz nomi ("Ўзбек").
    audio_name: String,
}

impl Variant {
    /// Navbatga yoziladigan manzil: `video` yoki `video\naudio`.
    fn job_url(&self) -> String {
        match &self.audio {
            Some(a) => format!("{}\n{a}", self.url),
            None => self.url.clone(),
        }
    }
}

/// `#EXT-X-MEDIA` qatoridagi atribut (qo'shtirnoq ichidagi vergul bilan ham).
fn m3u_attr(attrs: &str, name: &str) -> String {
    let key = format!("{name}=");
    let mut rest = attrs;
    while let Some(pos) = rest.find(&key) {
        let ok = pos == 0 || rest[..pos].ends_with(',');
        let after = &rest[pos + key.len()..];
        if ok {
            return if let Some(q) = after.strip_prefix('"') {
                q.split('"').next().unwrap_or("").to_string()
            } else {
                after.split(',').next().unwrap_or("").trim().to_string()
            };
        }
        rest = after;
    }
    String::new()
}

/// Nisbiy manzilni to'liqqa aylantiradi (m3u8 ichidagi `./720/index.m3u8`).
fn join_url(base: &str, rel: &str) -> String {
    let rel = rel.trim();
    if rel.starts_with("http://") || rel.starts_with("https://") {
        return rel.to_string();
    }
    let base = base.split('?').next().unwrap_or(base);
    if let Some(path) = rel.strip_prefix('/') {
        let host_end = base.find("://").map(|p| p + 3).and_then(|p| base[p..].find('/').map(|q| p + q)).unwrap_or(base.len());
        return format!("{}/{}", &base[..host_end], path);
    }
    let dir = base.rsplit_once('/').map(|(d, _)| d).unwrap_or(base);
    format!("{dir}/{}", rel.trim_start_matches("./"))
}

/// Qism/film sahifasidagi `video` -> HLS master -> sifatlar (balanddan pastga).
async fn variants(video: &str) -> std::result::Result<Vec<Variant>, String> {
    if video.is_empty() {
        return Err("bu qismda video yo'q (hali yuklanmagan yoki obuna kerak)".into());
    }
    let sep = if video.contains('?') { '&' } else { '?' };
    let (code, text) = http(&format!("{video}{sep}format=api"), Method::Get, None, None).await?;
    let master = serde_json::from_str::<Value>(&text).ok()
        .and_then(|v| v["file"].as_str().map(|f| join_url(video, f)))
        .filter(|f| !f.is_empty())
        .ok_or_else(|| format!("video manzili ochilmadi (HTTP {code})"))?;
    let (code, m3u) = http(&master, Method::Get, None, None).await?;
    if code >= 400 || !m3u.contains("#EXTM3U") {
        return Err(format!("playlist ochilmadi (HTTP {code})"));
    }
    let lines: Vec<&str> = m3u.lines().map(|l| l.trim()).collect();
    // OVOZ YO'LLARI (foydalanuvchi: "ovozi o'zbekcha emas, ruscha bo'lib
    // qolyapti"): ba'zi qismlarda (`external-hls`) o'zbekcha ovoz ALOHIDA
    // playlist (`#EXT-X-MEDIA:TYPE=AUDIO,LANGUAGE="uz"`), video bo'laklari
    // ichidagisi esa boshqa (ruscha) ovoz. Guruh bo'yicha: avval `uz`, keyin
    // DEFAULT=YES, keyin birinchisi.
    let mut groups: Vec<(String, String, String, i32)> = Vec::new(); // (guruh, uri, nom, ustunlik)
    for l in &lines {
        let Some(a) = l.strip_prefix("#EXT-X-MEDIA:") else { continue };
        if m3u_attr(a, "TYPE") != "AUDIO" { continue; }
        let uri = m3u_attr(a, "URI");
        if uri.is_empty() { continue; }
        let lang = m3u_attr(a, "LANGUAGE").to_ascii_lowercase();
        let rank = if lang.starts_with("uz") { 3 } else if m3u_attr(a, "DEFAULT") == "YES" { 2 } else { 1 };
        groups.push((m3u_attr(a, "GROUP-ID"), join_url(&master, &uri), m3u_attr(a, "NAME"), rank));
    }
    let pick_audio = |gid: &str| -> Option<(String, String)> {
        groups.iter().filter(|g| g.0 == gid).max_by_key(|g| g.3).map(|g| (g.1.clone(), g.2.clone()))
    };
    let mut out = Vec::new();
    for (k, l) in lines.iter().enumerate() {
        let Some(attrs) = l.strip_prefix("#EXT-X-STREAM-INF:") else { continue };
        let Some(uri) = lines[k + 1..].iter().find(|x| !x.is_empty() && !x.starts_with('#')) else { continue };
        let height = m3u_attr(attrs, "RESOLUTION").split('x').nth(1).and_then(|h| h.parse().ok()).unwrap_or(0);
        let bandwidth = m3u_attr(attrs, "BANDWIDTH").parse().unwrap_or(0);
        let gid = m3u_attr(attrs, "AUDIO");
        let (audio, audio_name) = match (!gid.is_empty()).then(|| pick_audio(&gid)).flatten() {
            Some((u, n)) => (Some(u), n),
            None => (None, String::new()),
        };
        out.push(Variant { height, bandwidth, url: join_url(&master, uri), audio, audio_name });
    }
    if out.is_empty() {
        // Sifatlarsiz oddiy playlist — bitta "asl" sifat.
        out.push(Variant { height: 0, bandwidth: 0, url: master, audio: None, audio_name: String::new() });
    }
    out.sort_by(|a, b| b.height.cmp(&a.height).then(b.bandwidth.cmp(&a.bandwidth)));
    out.dedup_by_key(|v| v.height);
    Ok(out)
}

/// Variant playlist'idagi bo'laklar uzunligi yig'indisi (soniya).
async fn duration(url: &str) -> f64 {
    match http(url, Method::Get, None, None).await {
        Ok((200, t)) => t.lines()
            .filter_map(|l| l.trim().strip_prefix("#EXTINF:"))
            .filter_map(|l| l.split(',').next()?.trim().parse::<f64>().ok())
            .sum(),
        _ => 0.0,
    }
}

fn quality_name(h: i64) -> String {
    if h > 0 { format!("{h}p") } else { "asl sifat".into() }
}

/// Qism (yoki film) videosi manzili.
async fn video_of(env: &Env, it: &Item, ep: Option<(&str, &Episode)>) -> std::result::Result<String, String> {
    let v = match ep {
        Some((season, e)) => {
            match get(env, &format!("episodes/{}/{}/{}", enc(&it.s), enc(season), enc(&e.slug)), true).await {
                Ok(v) => v,
                Err(_) => get(env, &format!("episodes/{}", enc(&e.slug)), true).await?,
            }
        }
        None => get(env, &format!("movies/{}", enc(&it.s)), true).await?,
    };
    let d = if v["data"].is_array() { v["data"][0].clone() } else if v["data"].is_object() { v["data"].clone() } else { v };
    Ok(["video", "video_url", "url", "stream"].iter()
        .find_map(|k| d[*k].as_str().filter(|s| !s.trim().is_empty()))
        .unwrap_or("").trim().to_string())
}

/// Sifat tugmalari: `(yozuv, balandlik)` va izoh matni.
async fn qualities_text(env: &Env, it: &Item, ep: Option<(&str, &Episode)>)
    -> std::result::Result<(String, Vec<(String, i64)>), String> {
    let video = video_of(env, it, ep).await?;
    let vs = variants(&video).await?;
    let secs = duration(&vs[0].url).await;
    let rows = vs.iter().map(|v| {
        let mut label = format!("\u{2B07}\u{FE0F} {}", quality_name(v.height));
        if secs > 0.0 && v.bandwidth > 0 {
            label.push_str(&format!(" \u{00B7} ~{:.0} MB", v.bandwidth as f64 * secs / 8.0 / 1_048_576.0));
        }
        (label, v.height)
    }).collect();
    let len = if secs > 0.0 { format!("\u{23F1} {} \u{00B7} ", hms(secs as i64)) } else { String::new() };
    let voice = if vs[0].audio_name.is_empty() { String::new() }
                else { format!("\u{1F50A} Ovoz: {}\n", html_escape(&vs[0].audio_name)) };
    Ok((format!("{voice}{len}Sifatni tanlang:"), rows))
}

fn hms(s: i64) -> String {
    let (h, m, s) = (s / 3600, s % 3600 / 60, s % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m:02}:{s:02}") }
}

// ── Bot: inline tugmalar (faqat navbat) ──────────────────────

/// Inline tugmalar: navbat (`zx`, `zxa`, `zr`). Eski xabarlardagi ro'yxat
/// tugmalari (`zg`, `zi`, ...) endi pastki panelda — ular uchun eslatma.
pub(crate) async fn on_callback(env: &Env, chat: i64, _msg_id: i64, data: &str) -> bool {
    let p: Vec<&str> = data.split(':').collect();
    let kind = p[0];
    if kind == "zxa" {
        queue_callback(env, chat, kind, 0).await;
        return true;
    }
    if matches!(kind, "zx" | "zr") {
        if let Some(id) = p.get(1).and_then(|x| x.parse::<i64>().ok()) {
            queue_callback(env, chat, kind, id).await;
        }
        return true;
    }
    if matches!(kind, "zc" | "zg" | "zi" | "zs" | "zp" | "ze" | "zq") {
        config_put(env, "encbot_mode", "anibla").await;
        let panel = home_panel(env).await;
        encbot_send(env, chat, "\u{267B}\u{FE0F} Bu eski tugma \u{2014} ro'yxatlar endi pastki panelda.", Some(panel)).await;
        return true;
    }
    false
}

// ── Yuklash navbati (Turso) va GitHub Actions ────────────────
//
// TALAB (foydalanuvchi): "alohida workflow'da ishlasin; yuklanadigan videolar
// ko'p bo'lsa bazada navbatda tursin va ketma-ket yuklab olinsin". Sifat
// bosilganda `anibla_jobs` ga bitta yozuv, `kick` — `anibla.yml` (GH_REPO)
// ishlamayotgan bo'lsa ishga tushiradi; run navbat bo'shaguncha
// `/api/anibla/claim` qilib, videolarni KETMA-KET yuklaydi. Cron ham har
// 10 daqiqada `kick` qiladi (run o'lsa yoki ishga tushmay qolsa).

pub(crate) const DDL: [&str; 2] = [
    // state: queued | running | error.
    "CREATE TABLE IF NOT EXISTS anibla_jobs (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        url TEXT NOT NULL,
        caption TEXT DEFAULT '',
        file_name TEXT DEFAULT '',
        chat INTEGER NOT NULL,
        status_msg INTEGER DEFAULT 0,
        queued_at INTEGER NOT NULL,
        state TEXT DEFAULT 'queued',
        runner TEXT DEFAULT '',
        lease_until INTEGER DEFAULT 0,
        attempts INTEGER DEFAULT 0,
        error TEXT DEFAULT ''
    )",
    "CREATE INDEX IF NOT EXISTS idx_anibla_q ON anibla_jobs(state, queued_at)",
];

/// Bitta video uchun ijara (run o'lsa shundan keyin qayta olinadi).
const LEASE_MS: i64 = 130 * 60 * 1000;
const MAX_ATTEMPTS: i64 = 3;

/// Navbatda ish bo'lsa-yu, uni hech kim ishlamayotgan bo'lsa — `anibla.yml`
/// ni ishga tushiradi. Natija matni (bot xabari uchun).
pub(crate) async fn kick(env: &Env) -> String {
    let now = now_ms();
    let res = turso_exec(env,
        "SELECT
           (SELECT COUNT(*) FROM anibla_jobs WHERE state='queued' OR (state='running' AND lease_until<=?)) AS pending,
           (SELECT COUNT(*) FROM anibla_jobs WHERE state='running' AND lease_until>?) AS active",
        vec![TursoArg::int(now), TursoArg::int(now)]).await;
    let Some(r) = res.ok().and_then(|r| first_row(&r)) else {
        return "\u{26A0}\u{FE0F} Navbatni o'qib bo'lmadi.".into();
    };
    if jint(&r, "active") > 0 {
        return "\u{2699}\u{FE0F} Yuklash ishlayapti \u{2014} navbatdagilar ketma-ket yuklanadi.".into();
    }
    if jint(&r, "pending") == 0 {
        return String::new();
    }
    let repo = tg_secret(env, "GH_REPO");
    if !repo.contains('/') {
        return "\u{26A0}\u{FE0F} GH_REPO o'rnatilmagan \u{2014} Actions ishga tushirilmadi.".into();
    }
    match gh_workflow_busy(env, &repo, WORKFLOW).await {
        Ok(true) => return "\u{23F3} GitHub Actions ishga tushmoqda \u{2014} biroz kuting.".into(),
        Ok(false) => {}
        Err(e) => return e,
    }
    // Ikki joydan bir vaqtda (cron + bot) ikkita run ochilmasin — atomik belgi.
    let claim = turso_exec(env,
        "INSERT INTO app_config (cfg_key,cfg_value) VALUES ('anibla_kicked_at', ?)
         ON CONFLICT(cfg_key) DO UPDATE SET cfg_value=excluded.cfg_value
           WHERE CAST(app_config.cfg_value AS INTEGER) < ?
         RETURNING cfg_key",
        vec![TursoArg::text(&now.to_string()), TursoArg::int(now - 3 * 60 * 1000)]).await;
    if !claim.ok().and_then(|r| first_row(&r)).is_some() {
        return "\u{23F3} GitHub Actions hozirgina ishga tushirilgan \u{2014} biroz kuting.".into();
    }
    match gh_api(env, Method::Post,
        &format!("/repos/{repo}/actions/workflows/{WORKFLOW}/dispatches"),
        Some(json!({"ref": "main"}))).await {
        Ok((204, _)) => "\u{25B6}\u{FE0F} GitHub Actions ishga tushirildi \u{2014} 1\u{2013}2 daqiqada boshlanadi.".into(),
        Ok((code, v)) => format!("\u{26A0}\u{FE0F} Actions ishga tushmadi (GitHub {code}): {}",
            html_escape(v["message"].as_str().unwrap_or(""))),
        Err(e) => format!("\u{26A0}\u{FE0F} Actions ishga tushmadi: {}", html_escape(&e.to_string())),
    }
}

fn job_head(caption: &str) -> String {
    let c = html_escape(caption);
    format!("\u{2B07}\u{FE0F} <b>{}</b>", c.replacen('\n', "</b>\n", 1))
}

/// Jonli holat xabari — "Post kodlash" holati bilan bir xil ko'rinish:
/// qalin sarlavha ("⬇️ Yuklash #4: Nomi"), ostida qism/sifat, keyin runner
/// yuborgan bosqichlar matni (oddiy matn, HTML'dan tozalanadi).
fn live_text(job: &Value, body: &str) -> String {
    let cap = job["caption"].as_str().unwrap_or("");
    let mut lines = cap.lines();
    let title = lines.next().unwrap_or("");
    let rest: Vec<&str> = lines.collect();
    let mut t = format!("\u{2B07}\u{FE0F} <b>Yuklash #{}: {}</b>", jint(job, "id"), html_escape(title));
    if !rest.is_empty() {
        t.push_str(&format!("\n{}", html_escape(&rest.join(" \u{00B7} "))));
    }
    if !body.is_empty() {
        t.push_str(&format!("\n\n{}", html_escape(body)));
    }
    t.chars().take(4000).collect()
}

/// Sifat tanlandi: variant manzilini aniqlaydi va navbatga qo'yadi.
async fn start(env: &Env, chat: i64, status: i64, it: &Item, ep: Option<(&str, &Episode, String)>, height: i64) {
    let mut caption = it.t.clone();
    if let Some((_, _, label)) = &ep {
        caption.push_str(&format!("\n{label}"));
    }
    caption.push_str(&format!("\n{}", quality_name(height)));
    let target = app_target(env).await;
    if let Some((_, label)) = &target {
        caption.push_str(&format!("\n\u{1F4F1} Ilovaga: {label}"));
    }
    let head = job_head(&caption);
    let status = if status > 0 { status } else {
        encbot_api(env, "sendMessage", json!({
            "chat_id": chat, "text": format!("\u{23F3} {} navbatga qo'shilmoqda...", quality_name(height)),
        })).await.ok().and_then(|v| v["message_id"].as_i64()).unwrap_or(0)
    };
    let _ = encbot_api(env, "editMessageText", json!({"chat_id": chat, "message_id": status, "parse_mode": "HTML",
        "text": format!("{head}\n\n\u{23F3} navbatga qo'shilmoqda (video manzili tekshirilmoqda)...")})).await;
    let edit = |line: String| json!({"chat_id": chat, "message_id": status, "parse_mode": "HTML",
        "text": format!("{head}\n\n{line}")});

    let video = match video_of(env, it, ep.as_ref().map(|(s, e, _)| (*s, *e))).await {
        Ok(v) => v,
        Err(e) => { let _ = encbot_api(env, "editMessageText", edit(format!("\u{274C} {}", html_escape(&e)))).await; return; }
    };
    let vs = match variants(&video).await {
        Ok(v) => v,
        Err(e) => { let _ = encbot_api(env, "editMessageText", edit(format!("\u{274C} {}", html_escape(&e)))).await; return; }
    };
    let Some(v) = vs.iter().find(|v| v.height == height).or(vs.first()) else { return };
    let fname: String = caption.lines().take(2).collect::<Vec<_>>().join(" ")
        .chars().filter(|c| !"\\/:*?\"<>|".contains(*c)).take(90).collect();
    let fname = format!("{} {}", fname.trim(), quality_name(v.height));

    // Bir xil video ikki marta navbatga tushmasin (tugma ikki bosilsa yoki
    // Telegram webhook'ni qayta yuborsa).
    let dup = turso_exec(env, "SELECT id FROM anibla_jobs WHERE url=? AND state IN ('queued','running') LIMIT 1",
        vec![TursoArg::text(&v.job_url())]).await.ok().and_then(|r| first_row(&r));
    if let Some(d) = dup {
        let _ = encbot_api(env, "editMessageText", edit(format!(
            "\u{2139}\u{FE0F} Bu video allaqachon navbatda (#{}).", jint(&d, "id")))).await;
        return;
    }
    let ins = turso_exec(env,
        "INSERT INTO anibla_jobs (url, caption, file_name, chat, status_msg, queued_at, target) VALUES (?,?,?,?,?,?,?) RETURNING id",
        vec![TursoArg::text(&v.job_url()), TursoArg::text(&caption), TursoArg::text(&fname),
             TursoArg::int(chat), TursoArg::int(status), TursoArg::int(now_ms()),
             TursoArg::text(target.as_ref().map(|(t, _)| t.as_str()).unwrap_or(""))]).await;
    let Some(id) = ins.ok().and_then(|r| first_row(&r)).map(|r| jint(&r, "id")) else {
        let _ = encbot_api(env, "editMessageText", edit("\u{274C} Navbatga qo'yib bo'lmadi (baza xatosi).".into())).await;
        return;
    };
    // Almashtirish — faqat bitta video; keyingi tanlov oddiy bo'ladi.
    if let Some((t, _)) = &target {
        if t.split('/').count() == 3 {
            config_put(env, "anibla_target", "").await;
        }
    }
    let ahead = turso_exec(env, "SELECT COUNT(*) AS n FROM anibla_jobs WHERE state IN ('queued','running') AND id<?",
        vec![TursoArg::int(id)]).await.ok().and_then(|r| first_row(&r)).map(|r| jint(&r, "n")).unwrap_or(0);
    let kick = kick(env).await;
    let place = if ahead > 0 { format!("oldinda {ahead} ta video bor") } else { "birinchi bo'lib yuklanadi".to_string() };
    let _ = encbot_api(env, "editMessageText", json!({"chat_id": chat, "message_id": status, "parse_mode": "HTML",
        "text": format!("\u{2705} <b>Navbatga qo'shildi</b> (#{id}, {place})\n{}\n\n{kick}\n\
                         Yuklash boshlanganda jonli holat (log bilan) pastda alohida xabarda chiqadi.",
                        html_escape(&caption))})).await;
}

fn state_icon(job: &Value) -> &'static str {
    match job["state"].as_str().unwrap_or("") {
        "running" if jint(job, "lease_until") > now_ms() => "\u{2699}\u{FE0F}",
        "error" => "\u{274C}",
        _ => "\u{23F3}",
    }
}

/// "📋 Holat" — "Anibla yuklash" bo'limida: yuklanayotgan video holati yangi
/// xabarda, faqat shu xabar yangilanib turadi (`livewatch.rs`).
pub(crate) async fn status(env: &Env, chat: i64) {
    let now = now_ms();
    let run = turso_exec(env,
        "SELECT * FROM anibla_jobs WHERE state='running' AND lease_until>? ORDER BY queued_at ASC LIMIT 1",
        vec![TursoArg::int(now)]).await.ok().and_then(|r| first_row(&r));
    let waiting = turso_exec(env, "SELECT COUNT(*) AS n FROM anibla_jobs WHERE state='queued'", vec![]).await
        .ok().and_then(|r| first_row(&r)).map(|r| jint(&r, "n")).unwrap_or(0);
    let html = match run {
        Some(j) => live_text(&j, &format!("\u{2699}\u{FE0F} yuklanmoqda \u{2014} holat bir necha soniyada shu yerda yangilanadi...\n\
                                         \u{1F4CB} Navbatda yana: {waiting} ta")),
        None => format!("\u{1F39E} <b>Anibla yuklash</b>\n\n\u{1F4A4} Hozir yuklanayotgan video yo'q.\n\
                         \u{1F4CB} Navbatda: {waiting} ta\n\nYuklash boshlansa, shu xabar o'zi yangilanadi."),
    };
    livewatch::start(env, livewatch::ANIBLA, chat, &html).await;
}

/// "📋 Yuklash navbati": har video alohida qator, o'chirish tugmasi bilan.
async fn queue_list(env: &Env, chat: i64) {
    let rows = turso_exec(env,
        "SELECT id, caption, state, lease_until, error FROM anibla_jobs ORDER BY queued_at ASC LIMIT 30", vec![]).await
        .map(|r| rows_of(&r)).unwrap_or_default();
    if rows.is_empty() {
        encbot_send(env, chat, "\u{1F4CB} Yuklash navbati bo'sh.", Some(menu_keyboard())).await;
        return;
    }
    let mut text = format!("\u{1F4CB} <b>Yuklash navbati</b> \u{2014} {} ta\n", rows.len());
    let mut kb_rows = Vec::new();
    for r in &rows {
        let id = jint(r, "id");
        let cap = r["caption"].as_str().unwrap_or("").replace('\n', " \u{00B7} ");
        let st = match state_icon(r) {
            "\u{2699}\u{FE0F}" => "\u{2699}\u{FE0F} yuklanmoqda",
            "\u{274C}" => "\u{274C} xato",
            _ => "\u{23F3} navbatda",
        };
        text.push_str(&format!("\n#{id} {} \u{2014} {st}", html_escape(&cap)));
        if r["state"].as_str() == Some("error") {
            text.push_str(&format!("\n   <i>{}</i>", html_escape(r["error"].as_str().unwrap_or(""))));
            kb_rows.push(vec![btn(&format!("\u{1F501} #{id} qayta"), format!("zr:{id}")),
                              btn(&format!("\u{1F5D1} #{id}"), format!("zx:{id}"))]);
        } else if state_icon(r) != "\u{2699}\u{FE0F}" {
            kb_rows.push(vec![btn(&format!("\u{1F5D1} #{id} ni olib tashlash"), format!("zx:{id}"))]);
        }
    }
    if rows.iter().filter(|r| state_icon(r) != "\u{2699}\u{FE0F}").count() > 1 {
        kb_rows.push(vec![btn("\u{1F9F9} Navbatdagilarning hammasini o'chirish", "zxa".into())]);
    }
    encbot_send(env, chat, &text, Some(kb(kb_rows))).await;
}

/// Navbat tugmalari: `zx:<id>` olib tashlash, `zr:<id>` qayta urinish.
async fn queue_callback(env: &Env, chat: i64, kind: &str, id: i64) {
    match kind {
        "zx" => {
            let r = turso_exec(env,
                "DELETE FROM anibla_jobs WHERE id=? AND NOT (state='running' AND lease_until>?) RETURNING status_msg, chat",
                vec![TursoArg::int(id), TursoArg::int(now_ms())]).await.ok().and_then(|r| first_row(&r));
            match r {
                Some(j) => {
                    let _ = encbot_api(env, "editMessageText", json!({
                        "chat_id": jint(&j, "chat"), "message_id": jint(&j, "status_msg"),
                        "text": format!("\u{1F5D1} #{id} navbatdan olib tashlandi."),
                    })).await;
                    encbot_send(env, chat, &format!("\u{1F5D1} #{id} olib tashlandi."), None).await;
                    queue_list(env, chat).await;
                }
                None => encbot_send(env, chat, &format!("#{id} yo'q yoki hozir yuklanmoqda."), None).await,
            }
        }
        // Navbatdagi (yuklanmayotgan) hamma videolar.
        "zxa" => {
            let gone = turso_exec(env,
                "DELETE FROM anibla_jobs WHERE NOT (state='running' AND lease_until>?) RETURNING id, status_msg, chat",
                vec![TursoArg::int(now_ms())]).await.map(|r| rows_of(&r)).unwrap_or_default();
            for j in &gone {
                let _ = encbot_api(env, "editMessageText", json!({
                    "chat_id": jint(j, "chat"), "message_id": jint(j, "status_msg"),
                    "text": format!("\u{1F5D1} #{} navbatdan olib tashlandi.", jint(j, "id")),
                })).await;
            }
            encbot_send(env, chat, &format!("\u{1F9F9} Navbatdan {} ta video o'chirildi.", gone.len()), None).await;
            queue_list(env, chat).await;
        }
        "zr" => {
            let _ = turso_exec(env,
                "UPDATE anibla_jobs SET state='queued', attempts=0, error='', runner='', lease_until=0 WHERE id=? AND state='error'",
                vec![TursoArg::int(id)]).await;
            let k = kick(env).await;
            encbot_send(env, chat, &format!("\u{1F501} #{id} yana navbatga qo'yildi.\n{k}"), None).await;
        }
        _ => {}
    }
}

// ── Actions (`anibla.yml` -> `tool/anibla/download.py`) ──────

async fn own(env: &Env, b: &Value) -> Option<Value> {
    let runner = b["runner"].as_str().unwrap_or("");
    if runner.is_empty() {
        return None;
    }
    turso_exec(env, "SELECT * FROM anibla_jobs WHERE id=? AND runner=? AND state='running'",
        vec![TursoArg::int(jint(b, "id")), TursoArg::text(runner)]).await
        .ok().and_then(|r| first_row(&r))
}

async fn edit_status(env: &Env, job: &Value, line: &str) {
    let msg = jint(job, "status_msg");
    if msg > 0 {
        let _ = encbot_api(env, "editMessageText", json!({
            "chat_id": jint(job, "chat"), "message_id": msg, "parse_mode": "HTML",
            "text": format!("{}\n\n{line}", job_head(job["caption"].as_str().unwrap_or(""))),
        })).await;
    }
}

pub(crate) async fn route(mut req: Request, env: &Env, path: &str, method: Method) -> Result<Response> {
    if !encode_token_ok(&req, env) {
        return json_resp(&json!({"error": "unauthorized"}), 401);
    }
    if method != Method::Post {
        return err404("topilmadi");
    }
    let b: Value = req.json().await.unwrap_or(json!({}));
    let now = now_ms();

    // Navbatdagi keyingi video (eng eskisi).
    if path == "/api/anibla/claim" {
        let runner = b["runner"].as_str().unwrap_or("").trim().to_string();
        if runner.is_empty() || runner.len() > 64 {
            return json_resp(&json!({"error": "runner"}), 400);
        }
        for _ in 0..5 {
            let res = turso_exec(env,
                "SELECT * FROM anibla_jobs WHERE state='queued' OR (state='running' AND lease_until<=?)
                  ORDER BY queued_at ASC LIMIT 1", vec![TursoArg::int(now)]).await?;
            let Some(job) = first_row(&res) else {
                return ok(json!({"none": true}));
            };
            let id = jint(&job, "id");
            if jint(&job, "attempts") >= MAX_ATTEMPTS {
                let _ = turso_exec(env,
                    "UPDATE anibla_jobs SET state='error', runner='', lease_until=0, error=? WHERE id=?",
                    vec![TursoArg::text(&format!("{MAX_ATTEMPTS} marta urinildi")), TursoArg::int(id)]).await;
                edit_status(env, &job, &format!("\u{274C} {MAX_ATTEMPTS} marta urinildi \u{2014} \u{1F4CB} navbatdan qayta urinish mumkin.")).await;
                continue;
            }
            let got = turso_exec(env,
                "UPDATE anibla_jobs SET state='running', runner=?, lease_until=?, attempts=attempts+1
                  WHERE id=? AND (state='queued' OR (state='running' AND lease_until<=?))
                  RETURNING attempts",
                vec![TursoArg::text(&runner), TursoArg::int(now + LEASE_MS), TursoArg::int(id), TursoArg::int(now)]).await?;
            let Some(got) = first_row(&got) else {
                continue;
            };
            // Yuklash boshlandi: jonli holat uchun chat OXIRIGA yangi xabar (eski
            // "navbatga qo'shildi" xabari yuqorida qolib ketadi va ko'rinmaydi).
            let chat = jint(&job, "chat");
            let mut status = jint(&job, "status_msg");
            let fresh = encbot_api(env, "sendMessage", json!({
                "chat_id": chat, "parse_mode": "HTML",
                "text": live_text(&job, "\u{23F3} boshlanmoqda..."),
            })).await.ok().and_then(|v| v["message_id"].as_i64()).unwrap_or(0);
            if fresh > 0 {
                let _ = turso_exec(env, "UPDATE anibla_jobs SET status_msg=? WHERE id=?",
                    vec![TursoArg::int(fresh), TursoArg::int(id)]).await;
                if status > 0 {
                    let _ = encbot_api(env, "editMessageText", json!({
                        "chat_id": chat, "message_id": status, "parse_mode": "HTML",
                        "text": format!("{}\n\n\u{2699}\u{FE0F} Yuklanmoqda \u{2014} jonli holat pastda \u{2B07}\u{FE0F}",
                            job_head(job["caption"].as_str().unwrap_or(""))),
                    })).await;
                }
                status = fresh;
            }
            livewatch::claim(env, livewatch::ANIBLA, chat, status).await;
            return ok(json!({"channel": tg_channel_id(env), "job": {
                "id": id, "url": job["url"], "caption": job["caption"], "file_name": job["file_name"],
                "chat": chat, "status_msg": status,
                "attempt": jint(&got, "attempts"),
            }}));
        }
        return ok(json!({"none": true}));
    }

    // Jonli holat: holat xabarini tahrirlaydi (bazaga tegmaydi).
    if path == "/api/anibla/progress" {
        let text: String = b["text"].as_str().unwrap_or("").chars().take(3500).collect();
        // Faqat "Anibla yuklash" kuzatilayotgan bo'lsa (`livewatch.rs`) — o'sha
        // bitta xabar tahrirlanadi. Bazadan o'qish ham shundagina.
        if text.is_empty() || livewatch::target(env, livewatch::ANIBLA).await.is_none() {
            return ok_nostore(json!({"ok": true}));
        }
        let Some(row) = own(env, &b).await else {
            return ok_nostore(json!({"ok": false, "gone": true}));
        };
        // Telegram cheklasa ("retry after N") — N runner'ga qaytadi va u shuncha kutadi.
        if let Err(secs) = livewatch::edit(env, livewatch::ANIBLA, &live_text(&row, &text)).await {
            return ok_nostore(json!({"ok": false, "retry_after": secs}));
        }
        return ok_nostore(json!({"ok": true}));
    }

    if path == "/api/anibla/done" {
        let Some(job) = own(env, &b).await else {
            return json_resp(&json!({"error": "lost"}), 409);
        };
        let id = jint(&job, "id");
        let chat = jint(&job, "chat");
        let text: String = b["text"].as_str().unwrap_or("").chars().take(3500).collect();
        let status = |line: String| json!({"chat_id": chat, "message_id": jint(&job, "status_msg"),
            "parse_mode": "HTML",
            "text": live_text(&job, &if text.is_empty() { line.clone() } else { format!("{text}\n{line}") })});
        // Yakuniy holat "Holat" bilan ochilgan kuzatilayotgan xabarga ham.
        let watch = livewatch::target(env, livewatch::ANIBLA).await
            .filter(|(_, m)| *m != jint(&job, "status_msg"));
        let mirror = |body: Value| async move {
            if let Some((c, m)) = watch {
                let _ = livewatch::edit_msg(env, c, m, body["text"].as_str().unwrap_or("")).await;
            }
        };
        if b["ok"].as_bool() == Some(true) {
            let channel = tg_channel_id(env);
            let msg = jint(&b, "channel_msg");
            // "Ilova uchun -> Anibla orqali": kanal posti qismning asl videosi bo'ladi
            // va kodlashga navbatga tushadi (botga yuborilgan video bilan bir xil).
            let target: Vec<i64> = job["target"].as_str().unwrap_or("").split('/')
                .filter_map(|p| p.parse().ok()).collect();
            if target.len() >= 2 && msg > 0 {
                let (a, s) = (target[0], target[1]);
                let n = match target.get(2) {
                    Some(n) => *n,
                    None => encbot_numbers(env, a, s).await.iter().max().copied().unwrap_or(0) + 1,
                };
                let origin = format!("orig_bot_{a}_{s}_{n}_{}.mp4", now_ms());
                let res = if encbot_titles(env, a, s).await.is_some() {
                    encbot_register(env, a, s, n, &origin, msg, jint(&b, "size"), jint(&b, "height")).await
                } else {
                    "\u{274C} Anime yoki bo'lim topilmadi (o'chirilgan bo'lishi mumkin) \u{2014} video yopiq kanalda qoldi.".into()
                };
                let ok_line = if res.starts_with('\u{2705}') { format!("\u{2705} Ilovaga {n}-qism bo'lib qo'shildi \u{2014} kodlash navbatida.") }
                              else { "\u{26A0}\u{FE0F} Ilovaga qo'shilmadi \u{2014} sababi pastda.".to_string() };
                { let body = status(ok_line); mirror(body.clone()).await; let _ = encbot_api(env, "editMessageText", body).await; }
                encbot_send(env, chat, &res, None).await;
                turso_exec(env, "DELETE FROM anibla_jobs WHERE id=?", vec![TursoArg::int(id)]).await?;
                return ok(json!({"ok": true}));
            }
            // Tayyor video kanaldan BOT CHATIGA ko'chiriladi, kanal posti o'chadi.
            let line = match encbot_api(env, "copyMessage", json!({
                "chat_id": chat, "from_chat_id": channel, "message_id": msg,
            })).await {
                Ok(_) => {
                    let _ = encbot_api(env, "deleteMessage", json!({"chat_id": channel, "message_id": msg})).await;
                    "\u{2705} Video pastda.".to_string()
                }
                Err(e) => format!("\u{26A0}\u{FE0F} Botga ko'chmadi ({e}) \u{2014} video yopiq kanalda qoldi."),
            };
            { let body = status(line); mirror(body.clone()).await; let _ = encbot_api(env, "editMessageText", body).await; }
            turso_exec(env, "DELETE FROM anibla_jobs WHERE id=?", vec![TursoArg::int(id)]).await?;
            return ok(json!({"ok": true}));
        }
        let err: String = b["error"].as_str().unwrap_or("").chars().take(300).collect();
        if b["cancelled"].as_bool() == Some(true) {
            turso_exec(env,
                "UPDATE anibla_jobs SET state='queued', runner='', lease_until=0, attempts=MAX(attempts-1,0) WHERE id=?",
                vec![TursoArg::int(id)]).await?;
            { let body = status("\u{23F8} Run to'xtatildi \u{2014} video navbatga qaytdi.".into()); mirror(body.clone()).await; let _ = encbot_api(env, "editMessageText", body).await; }
            return ok(json!({"ok": true}));
        }
        let fatal = b["fatal"].as_bool() == Some(true) || jint(&job, "attempts") >= MAX_ATTEMPTS;
        turso_exec(env, "UPDATE anibla_jobs SET state=?, runner='', lease_until=0, error=? WHERE id=?",
            vec![TursoArg::text(if fatal { "error" } else { "queued" }), TursoArg::text(&err), TursoArg::int(id)]).await?;
        let line = if fatal {
            format!("\u{274C} Xato: {err}\n\u{1F4CB} Navbatdan qayta urinish yoki olib tashlash mumkin.")
        } else {
            format!("\u{26A0}\u{FE0F} Xato: {err}\nKeyinroq yana urinib ko'riladi.")
        };
        { let body = status(line); mirror(body.clone()).await; let _ = encbot_api(env, "editMessageText", body).await; }
        return ok(json!({"ok": true}));
    }

    err404("topilmadi")
}
