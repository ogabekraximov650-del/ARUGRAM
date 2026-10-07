// ══════════════════════════════════════════════════════════════
//  ANIBLA.UZ YUKLASH — kodlash botining uchinchi bo'limi
// ══════════════════════════════════════════════════════════════
//
// TALAB (foydalanuvchi): bot anibla.uz ga login/parol bilan kiradi,
// "Izlash" tugmasi; nom yoziladi — topilgan anime/filmning qismi va
// mavjud sifatlari (1080p, 720p, ...) tugma bo'lib chiqadi; tugma
// bosilganda video GitHub Actions orqali (IKKINCHI akkauntdagi `GH_REPO`)
// yuklab olinib, botga yuboriladi. Login/parol AES bilan shifrlangan
// faylda (`tool/anibla/creds.enc`).
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
const WORKFLOW: &str = "anibla.yml";
const DEFAULT_SITE: &str = "https://anibla.uz";
/// Qidiruv natijalari bir sahifada.
const SEARCH_PAGE: i64 = 8;
/// Qism tugmalari bir sahifada (5 tadan 6 qator).
const EP_PAGE: usize = 30;
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
}

impl Item {
    fn movie(&self) -> bool {
        !self.m.to_ascii_lowercase().contains("serie")
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Nav {
    n: String,
    q: String,
    p: i64,
    pages: i64,
    total: i64,
    items: Vec<Item>,
}

async fn nav_get(env: &Env) -> Nav {
    config_get(env, "anibla_nav").await
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_default()
}

fn kb(rows: Vec<Vec<Value>>) -> Value {
    json!({"inline_keyboard": rows})
}

fn btn(text: &str, data: String) -> Value {
    json!({"text": text, "callback_data": data})
}

fn menu_keyboard() -> Value {
    encbot_keyboard(vec![vec![BTN_SEARCH.to_string(), BTN_QUEUE.to_string()], vec![postbot::BTN_HOME.to_string()]])
}

// ── Bot: menyu va qidiruv ────────────────────────────────────

pub(crate) async fn menu(env: &Env, chat: i64) {
    config_put(env, "encbot_mode", "anibla").await;
    let warn = if creds(env).is_none() {
        "\n\n\u{26A0}\u{FE0F} Login/parol hali sozlanmagan: GitHub'da <code>ANIBLA_KEY</code> \
         secret'ini qo'yib, worker'ni qayta deploy qiling."
    } else { "" };
    encbot_send(env, chat, &format!(
        "\u{1F39E} <b>Anibla.uz yuklash</b>\n\n\
         \u{1F50D} Anime yoki film nomini yozing (kamida 3 harf).\n\
         Natijadan tanlang \u{2192} fasl va qism \u{2192} sifat.\n\
         Video navbatga tushadi va GitHub Actions orqali ketma-ket yuklanib, shu chatga yuboriladi.{warn}"),
        Some(menu_keyboard())).await;
}

/// Anibla rejimidagi matn — qidiruv.
pub(crate) async fn on_message(env: &Env, msg: &Value, chat: i64) {
    let text = msg["text"].as_str().unwrap_or("").trim().to_string();
    if text == BTN_QUEUE {
        queue_list(env, chat).await;
        return;
    }
    if text.is_empty() || text == BTN_SEARCH {
        encbot_send(env, chat, "\u{270D}\u{FE0F} Anime yoki film nomini yozing:", Some(menu_keyboard())).await;
        return;
    }
    if text.chars().count() < 3 {
        encbot_send(env, chat, "Kamida 3 ta harf yozing.", Some(menu_keyboard())).await;
        return;
    }
    let q: String = text.chars().take(80).collect();
    search(env, chat, 0, &q, 1).await;
}

/// Qidiruv sahifasi. `edit` > 0 — shu xabarni tahrirlaydi (sahifa almashganda).
async fn search(env: &Env, chat: i64, edit: i64, q: &str, page: i64) {
    let path = format!("media/mobile?search={}&limit={SEARCH_PAGE}&page={page}", enc(q));
    let v = match get(env, &path, false).await {
        Ok(v) => v,
        Err(e) => {
            encbot_send(env, chat, &format!("\u{274C} Qidirib bo'lmadi: {}", html_escape(&e)), None).await;
            return;
        }
    };
    let items: Vec<Item> = data_list(&v).iter().map(|x| Item {
        s: x["slug"].as_str().unwrap_or("").to_string(),
        t: title_of(x),
        m: x["mediaType"].as_str().unwrap_or("").to_string(),
        y: jint(x, "published_year"),
        e: jint(x, "available_episodes"),
        d: x["uz"]["description"].as_str().unwrap_or("").chars().take(350).collect(),
        img: x["thumbnail"].as_str().unwrap_or("").trim_start_matches('/').to_string(),
    }).filter(|i| !i.s.is_empty()).collect();
    if items.is_empty() {
        let t = format!("\u{1F937} <b>\u{00AB}{}\u{00BB}</b> bo'yicha hech narsa topilmadi.\n\
                         Boshqacha yozib ko'ring (masalan, o'zbekcha nomining bir qismi).", html_escape(q));
        encbot_send(env, chat, &t, Some(menu_keyboard())).await;
        return;
    }
    let pages = jint(&v["pagination"], "pages").max(1);
    let total = jint(&v["pagination"], "total").max(items.len() as i64);
    let nav = Nav {
        n: (now_ms() % 1_000_000).to_string(),
        q: q.to_string(), p: page, pages, total, items,
    };
    config_put(env, "anibla_nav", &serde_json::to_string(&nav).unwrap_or_default()).await;

    let mut rows: Vec<Vec<Value>> = nav.items.iter().enumerate().map(|(i, it)| {
        let mut label = format!("{} {}", if it.movie() { "\u{1F3AC}" } else { "\u{1F4FA}" }, it.t);
        if it.y > 0 { label.push_str(&format!(" \u{00B7} {}", it.y)); }
        if !it.movie() && it.e > 0 { label.push_str(&format!(" \u{00B7} {} qism", it.e)); }
        vec![btn(&label, format!("zi:{}:{i}", nav.n))]
    }).collect();
    if pages > 1 {
        let mut r = Vec::new();
        if page > 1 { r.push(btn("\u{25C0}\u{FE0F}", format!("zg:{}:{}", nav.n, page - 1))); }
        r.push(btn(&format!("{page}/{pages}"), format!("zg:{}:{page}", nav.n)));
        if page < pages { r.push(btn("\u{25B6}\u{FE0F}", format!("zg:{}:{}", nav.n, page + 1))); }
        rows.push(r);
    }
    let text = format!("\u{1F50D} <b>\u{00AB}{}\u{00BB}</b> \u{2014} {total} ta natija\n\
                        \u{1F4FA} serial \u{00B7} \u{1F3AC} film\n\nKeraklisini tanlang:", html_escape(q));
    if edit > 0 {
        let _ = encbot_api(env, "editMessageText", json!({
            "chat_id": chat, "message_id": edit, "text": text, "parse_mode": "HTML",
            "reply_markup": kb(rows),
        })).await;
    } else {
        encbot_send(env, chat, &text, Some(kb(rows))).await;
    }
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

/// Tanlangan anime/film: rasm, qisqa ma'lumot va keyingi qadam tugmalari.
async fn show_item(env: &Env, chat: i64, nav: &Nav, i: usize) {
    let Some(it) = nav.items.get(i) else { return };
    let mut head = format!("{} <b>{}</b>", if it.movie() { "\u{1F3AC}" } else { "\u{1F4FA}" }, html_escape(&it.t));
    if it.y > 0 { head.push_str(&format!(" ({})", it.y)); }
    if !it.d.is_empty() {
        head.push_str(&format!("\n\n{}\u{2026}", html_escape(&it.d)));
    }
    let markup = if it.movie() {
        match qualities_text(env, it, None).await {
            Ok((t, rows)) => {
                head.push_str(&format!("\n\n{t}"));
                kb(rows.into_iter().map(|(label, h)| vec![btn(&label, format!("zq:{}:{i}:-:-:{h}", nav.n))]).collect())
            }
            Err(e) => {
                head.push_str(&format!("\n\n\u{274C} {}", html_escape(&e)));
                kb(vec![])
            }
        }
    } else {
        match seasons(env, &it.s).await {
            Ok(ss) if ss.len() > 1 => {
                head.push_str("\n\nFaslni tanlang:");
                let rows: Vec<Vec<Value>> = ss.chunks(3).enumerate().map(|(r, ch)| ch.iter().enumerate()
                    .map(|(c, s)| btn(&season_label(s), format!("zs:{}:{i}:{}", nav.n, r * 3 + c))).collect()).collect();
                kb(rows)
            }
            Ok(ss) if ss.len() == 1 => match episodes(env, &it.s, &ss[0].slug).await.ok() {
                Some(eps) if !eps.is_empty() => {
                    head.push_str(&format!("\n\n{} \u{00B7} {} ta qism. Qismni tanlang:", season_label(&ss[0]), eps.len()));
                    episode_kb(&nav.n, i, 0, &eps, 0)
                }
                _ => {
                    head.push_str("\n\n\u{1F937} Qismlar topilmadi.");
                    kb(vec![])
                }
            },
            Ok(_) => {
                head.push_str("\n\n\u{1F937} Fasllar topilmadi.");
                kb(vec![])
            }
            Err(e) => {
                head.push_str(&format!("\n\n\u{274C} {}", html_escape(&e)));
                kb(vec![])
            }
        }
    };
    send_card(env, chat, it, &head, markup).await;
}

/// Muqova rasmi bilan (bo'lmasa oddiy matn).
async fn send_card(env: &Env, chat: i64, it: &Item, text: &str, markup: Value) {
    if !it.img.is_empty() && text.chars().count() <= 1000 {
        let r = encbot_api(env, "sendPhoto", json!({
            "chat_id": chat, "photo": format!("{}/{}", site(env), it.img),
            "caption": text, "parse_mode": "HTML", "reply_markup": markup,
        })).await;
        if r.is_ok() {
            return;
        }
    }
    encbot_send(env, chat, text, Some(markup)).await;
}

/// Qism tugmalari (5 tadan), sahifalab.
fn episode_kb(n: &str, i: usize, j: usize, eps: &[Episode], page: usize) -> Value {
    let pages = eps.len().div_ceil(EP_PAGE).max(1);
    let page = page.min(pages - 1);
    let start = page * EP_PAGE;
    let mut rows: Vec<Vec<Value>> = eps[start..(start + EP_PAGE).min(eps.len())].chunks(5).enumerate()
        .map(|(r, ch)| ch.iter().enumerate()
            .map(|(c, e)| btn(&e.num.to_string(), format!("ze:{n}:{i}:{j}:{}", start + r * 5 + c))).collect())
        .collect();
    if pages > 1 {
        let mut r = Vec::new();
        if page > 0 { r.push(btn("\u{25C0}\u{FE0F}", format!("zp:{n}:{i}:{j}:{}", page - 1))); }
        let a = eps[start].num;
        let b = eps[(start + EP_PAGE).min(eps.len()) - 1].num;
        r.push(btn(&format!("{a}\u{2013}{b} / {}", eps.len()), format!("zp:{n}:{i}:{j}:{page}")));
        if page + 1 < pages { r.push(btn("\u{25B6}\u{FE0F}", format!("zp:{n}:{i}:{j}:{}", page + 1))); }
        rows.push(r);
    }
    kb(rows)
}

// ── Video va sifatlar ────────────────────────────────────────

struct Variant {
    height: i64,
    bandwidth: i64,
    url: String,
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
    let mut out = Vec::new();
    for (k, l) in lines.iter().enumerate() {
        let Some(attrs) = l.strip_prefix("#EXT-X-STREAM-INF:") else { continue };
        let Some(uri) = lines[k + 1..].iter().find(|x| !x.is_empty() && !x.starts_with('#')) else { continue };
        let attr = |name: &str| attrs.split(',').find_map(|p| p.trim().strip_prefix(name)).unwrap_or("");
        let height = attr("RESOLUTION=").split('x').nth(1).and_then(|h| h.parse().ok()).unwrap_or(0);
        let bandwidth = attr("BANDWIDTH=").parse().unwrap_or(0);
        out.push(Variant { height, bandwidth, url: join_url(&master, uri) });
    }
    if out.is_empty() {
        // Sifatlarsiz oddiy playlist — bitta "asl" sifat.
        out.push(Variant { height: 0, bandwidth: 0, url: master });
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
    Ok((format!("{len}Sifatni tanlang:"), rows))
}

fn hms(s: i64) -> String {
    let (h, m, s) = (s / 3600, s % 3600 / 60, s % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m:02}:{s:02}") }
}

// ── Bot: tugmalar ────────────────────────────────────────────

/// Inline tugmalar (`z?:<nav>:...`). `true` — ishlandi.
pub(crate) async fn on_callback(env: &Env, chat: i64, msg_id: i64, data: &str) -> bool {
    let p: Vec<&str> = data.split(':').collect();
    let kind = p[0];
    if matches!(kind, "zx" | "zr") {
        if let Some(id) = p.get(1).and_then(|x| x.parse::<i64>().ok()) {
            queue_callback(env, chat, kind, id).await;
        }
        return true;
    }
    if !matches!(kind, "zg" | "zi" | "zs" | "zp" | "ze" | "zq") {
        return false;
    }
    let nav = nav_get(env).await;
    if p.get(1).copied() != Some(nav.n.as_str()) || nav.n.is_empty() {
        encbot_send(env, chat, "\u{267B}\u{FE0F} Bu eski qidiruv natijasi \u{2014} nomni qaytadan yozing.",
            Some(menu_keyboard())).await;
        return true;
    }
    let num = |k: usize| p.get(k).and_then(|x| x.parse::<i64>().ok());
    if kind == "zg" {
        let page = num(2).unwrap_or(1).clamp(1, nav.pages.max(1));
        if page != nav.p {
            search(env, chat, msg_id, &nav.q, page).await;
        }
        return true;
    }
    let i = num(2).unwrap_or(-1);
    let Some(it) = usize::try_from(i).ok().and_then(|i| nav.items.get(i)).cloned() else {
        return true;
    };
    let i = i as usize;
    if kind == "zi" {
        show_item(env, chat, &nav, i).await;
        return true;
    }
    // Film sifati: `zq:<n>:<i>:-:-:<h>`.
    if kind == "zq" && it.movie() {
        start(env, chat, &it, None, num(5).unwrap_or(0)).await;
        return true;
    }
    let ss = match seasons(env, &it.s).await {
        Ok(ss) => ss,
        Err(e) => {
            encbot_send(env, chat, &format!("\u{274C} {}", html_escape(&e)), None).await;
            return true;
        }
    };
    let Some(season) = num(3).and_then(|j| usize::try_from(j).ok()).and_then(|j| ss.get(j)) else {
        return true;
    };
    let j = num(3).unwrap_or(0) as usize;
    let eps = match episodes(env, &it.s, &season.slug).await {
        Ok(e) if !e.is_empty() => e,
        Ok(_) => {
            encbot_send(env, chat, "\u{1F937} Bu faslda qismlar topilmadi.", None).await;
            return true;
        }
        Err(e) => {
            encbot_send(env, chat, &format!("\u{274C} {}", html_escape(&e)), None).await;
            return true;
        }
    };
    match kind {
        "zs" => {
            encbot_send(env, chat, &format!("\u{1F4FA} <b>{}</b> \u{00B7} {} \u{00B7} {} ta qism\n\nQismni tanlang:",
                html_escape(&it.t), html_escape(&season_label(season)), eps.len()),
                Some(episode_kb(&nav.n, i, j, &eps, 0))).await;
        }
        "zp" => {
            let page = num(4).unwrap_or(0).max(0) as usize;
            let _ = encbot_api(env, "editMessageReplyMarkup", json!({
                "chat_id": chat, "message_id": msg_id, "reply_markup": episode_kb(&nav.n, i, j, &eps, page),
            })).await;
        }
        "ze" => {
            let Some(e) = num(4).and_then(|k| usize::try_from(k).ok()).and_then(|k| eps.get(k)) else { return true };
            let k = num(4).unwrap_or(0);
            let head = format!("\u{1F4FA} <b>{}</b>\n{} \u{00B7} {}-qism{}", html_escape(&it.t),
                html_escape(&season_label(season)), e.num,
                if e.title.is_empty() { String::new() } else { format!(": {}", html_escape(&e.title)) });
            match qualities_text(env, &it, Some((&season.slug, e))).await {
                Ok((t, rows)) => {
                    let rows = rows.into_iter()
                        .map(|(label, h)| vec![btn(&label, format!("zq:{}:{i}:{j}:{k}:{h}", nav.n))]).collect();
                    encbot_send(env, chat, &format!("{head}\n\n{t}"), Some(kb(rows))).await;
                }
                Err(err) => encbot_send(env, chat, &format!("{head}\n\n\u{274C} {}", html_escape(&err)), None).await,
            }
        }
        "zq" => {
            let Some(e) = num(4).and_then(|k| usize::try_from(k).ok()).and_then(|k| eps.get(k)) else { return true };
            let label = format!("{} \u{00B7} {}-qism{}", season_label(season), e.num,
                if e.title.is_empty() { String::new() } else { format!(": {}", e.title) });
            start(env, chat, &it, Some((&season.slug, e, label)), num(5).unwrap_or(0)).await;
        }
        _ => {}
    }
    true
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

/// Sifat tanlandi: variant manzilini aniqlaydi va navbatga qo'yadi.
async fn start(env: &Env, chat: i64, it: &Item, ep: Option<(&str, &Episode, String)>, height: i64) {
    let mut caption = it.t.clone();
    if let Some((_, _, label)) = &ep {
        caption.push_str(&format!("\n{label}"));
    }
    caption.push_str(&format!("\n{}", quality_name(height)));
    let head = job_head(&caption);
    let status = encbot_api(env, "sendMessage", json!({
        "chat_id": chat, "parse_mode": "HTML", "text": format!("{head}\n\n\u{23F3} tayyorlanmoqda..."),
    })).await.ok().and_then(|v| v["message_id"].as_i64()).unwrap_or(0);
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
        vec![TursoArg::text(&v.url)]).await.ok().and_then(|r| first_row(&r));
    if let Some(d) = dup {
        let _ = encbot_api(env, "editMessageText", edit(format!(
            "\u{2139}\u{FE0F} Bu video allaqachon navbatda (#{}).", jint(&d, "id")))).await;
        return;
    }
    let ins = turso_exec(env,
        "INSERT INTO anibla_jobs (url, caption, file_name, chat, status_msg, queued_at) VALUES (?,?,?,?,?,?) RETURNING id",
        vec![TursoArg::text(&v.url), TursoArg::text(&caption), TursoArg::text(&fname),
             TursoArg::int(chat), TursoArg::int(status), TursoArg::int(now_ms())]).await;
    let Some(id) = ins.ok().and_then(|r| first_row(&r)).map(|r| jint(&r, "id")) else {
        let _ = encbot_api(env, "editMessageText", edit("\u{274C} Navbatga qo'yib bo'lmadi (baza xatosi).".into())).await;
        return;
    };
    let ahead = turso_exec(env, "SELECT COUNT(*) AS n FROM anibla_jobs WHERE state IN ('queued','running') AND id<?",
        vec![TursoArg::int(id)]).await.ok().and_then(|r| first_row(&r)).map(|r| jint(&r, "n")).unwrap_or(0);
    let kick = kick(env).await;
    let place = if ahead > 0 { format!("\u{1F4CB} Navbatda #{id} \u{2014} oldinda {ahead} ta video.") }
                else { format!("\u{1F4CB} Navbatda #{id} \u{2014} birinchi.") };
    let _ = encbot_api(env, "editMessageText", edit(format!("{place}\n{kick}"))).await;
}

fn state_icon(job: &Value) -> &'static str {
    match job["state"].as_str().unwrap_or("") {
        "running" if jint(job, "lease_until") > now_ms() => "\u{2699}\u{FE0F}",
        "error" => "\u{274C}",
        _ => "\u{23F3}",
    }
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
    let mut text = format!("\u{1F4CB} <b>Yuklash navbati</b> \u{2014} {} ta\n\
                            \u{23F3} kutmoqda \u{00B7} \u{2699}\u{FE0F} yuklanmoqda \u{00B7} \u{274C} xato\n", rows.len());
    let mut kb_rows = Vec::new();
    for r in &rows {
        let id = jint(r, "id");
        let cap = r["caption"].as_str().unwrap_or("").replace('\n', " \u{00B7} ");
        text.push_str(&format!("\n{} #{id} {}", state_icon(r), html_escape(&cap)));
        if r["state"].as_str() == Some("error") {
            text.push_str(&format!("\n   <i>{}</i>", html_escape(r["error"].as_str().unwrap_or(""))));
            kb_rows.push(vec![btn(&format!("\u{1F501} #{id} qayta"), format!("zr:{id}")),
                              btn(&format!("\u{1F5D1} #{id}"), format!("zx:{id}"))]);
        } else if state_icon(r) != "\u{2699}\u{FE0F}" {
            kb_rows.push(vec![btn(&format!("\u{1F5D1} #{id} ni olib tashlash"), format!("zx:{id}"))]);
        }
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
                }
                None => encbot_send(env, chat, &format!("#{id} yo'q yoki hozir yuklanmoqda."), None).await,
            }
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
            return ok(json!({"channel": tg_channel_id(env), "job": {
                "id": id, "url": job["url"], "caption": job["caption"], "file_name": job["file_name"],
                "chat": jint(&job, "chat"), "status_msg": jint(&job, "status_msg"),
                "attempt": jint(&got, "attempts"),
            }}));
        }
        return ok(json!({"none": true}));
    }

    // Jonli holat: holat xabarini tahrirlaydi (bazaga tegmaydi).
    if path == "/api/anibla/progress" {
        let text: String = b["text"].as_str().unwrap_or("").chars().take(3500).collect();
        let msg = jint(&b, "status_msg");
        if msg > 0 && !text.is_empty() {
            let _ = encbot_api(env, "editMessageText", json!({
                "chat_id": jint(&b, "chat"), "message_id": msg, "text": text,
            })).await;
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
            "text": if text.is_empty() { line.clone() } else { format!("{text}\n{line}") }});
        if b["ok"].as_bool() == Some(true) {
            // Tayyor video kanaldan BOT CHATIGA ko'chiriladi, kanal posti o'chadi.
            let channel = tg_channel_id(env);
            let msg = jint(&b, "channel_msg");
            let line = match encbot_api(env, "copyMessage", json!({
                "chat_id": chat, "from_chat_id": channel, "message_id": msg,
            })).await {
                Ok(_) => {
                    let _ = encbot_api(env, "deleteMessage", json!({"chat_id": channel, "message_id": msg})).await;
                    "\u{2705} Video pastda.".to_string()
                }
                Err(e) => format!("\u{26A0}\u{FE0F} Botga ko'chmadi ({e}) \u{2014} video yopiq kanalda qoldi."),
            };
            let _ = encbot_api(env, "editMessageText", status(line)).await;
            turso_exec(env, "DELETE FROM anibla_jobs WHERE id=?", vec![TursoArg::int(id)]).await?;
            return ok(json!({"ok": true}));
        }
        let err: String = b["error"].as_str().unwrap_or("").chars().take(300).collect();
        if b["cancelled"].as_bool() == Some(true) {
            turso_exec(env,
                "UPDATE anibla_jobs SET state='queued', runner='', lease_until=0, attempts=MAX(attempts-1,0) WHERE id=?",
                vec![TursoArg::int(id)]).await?;
            let _ = encbot_api(env, "editMessageText", status("\u{23F8} Run to'xtatildi \u{2014} video navbatga qaytdi.".into())).await;
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
        let _ = encbot_api(env, "editMessageText", status(line)).await;
        return ok(json!({"ok": true}));
    }

    err404("topilmadi")
}
