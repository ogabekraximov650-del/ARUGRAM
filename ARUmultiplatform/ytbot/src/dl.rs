// UZ: yt-dlp bilan ishlash: ma'lumot olish (sifatlar), diskka yuklash, oqim (stdout). EN: talking to yt-dlp: probing formats, file download, stdout streaming.
use crate::Cfg;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Bitta sifat tugmasi.
#[derive(Clone, Debug, PartialEq)]
pub struct Opt {
    pub h: i64,
    /// Taxminiy yakuniy hajm (bayt), bilinsa.
    pub size: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct Info {
    pub title: String,
    pub duration: i64,
    pub w: i64,
    pub h: i64,
    pub opts: Vec<Opt>,
    pub audio_size: Option<u64>,
    /// To'g'ridan-to'g'ri fayl havolalari (balandlik -> URL): yt-dlp sahifaga kira olmaydigan saytlar uchun (masalan Rumble embed).
    pub direct: Vec<(i64, String)>,
}

fn has_codec(v: &Value) -> bool {
    matches!(v.as_str(), Some(s) if s != "none" && !s.is_empty())
}

/// Video formatmi: `vcodec` aniq "none" bo'lmasa (noma'lum/null ham) — video.
fn is_video(f: &Value) -> bool {
    f["vcodec"].as_str() != Some("none")
}

fn size_of(f: &Value) -> Option<u64> {
    f["filesize"].as_u64().or_else(|| f["filesize_approx"].as_f64().map(|x| x as u64)).filter(|s| *s > 0)
}

/// `yt-dlp -J` javobidan sifatlar ro'yxati (balandlik bo'yicha kamayish tartibida).
pub fn parse_info(v: &Value) -> Info {
    let formats = v["formats"].as_array().cloned().unwrap_or_default();
    let mut heights: Vec<i64> = formats.iter().filter(|f| is_video(f)).filter_map(|f| f["height"].as_i64()).filter(|h| *h >= 144).collect();
    heights.sort_unstable_by(|a, b| b.cmp(a));
    heights.dedup();
    let audio_size = formats.iter().filter(|f| !has_codec(&f["vcodec"]) && has_codec(&f["acodec"])).filter_map(size_of).max();
    let mut opts = Vec::new();
    for h in heights {
        let at_h: Vec<&Value> = formats.iter().filter(|f| is_video(f) && f["height"].as_i64() == Some(h)).collect();
        let progressive = at_h.iter().filter(|f| f["acodec"].as_str() != Some("none")).filter_map(|f| size_of(f)).max();
        let video_only = at_h.iter().filter(|f| f["acodec"].as_str() == Some("none")).filter_map(|f| size_of(f)).max();
        let size = match (video_only, audio_size) {
            (Some(v), Some(a)) => Some(v + a),
            (Some(v), None) => Some(v),
            _ => progressive,
        };
        opts.push(Opt { h, size });
    }
    Info {
        title: v["title"].as_str().unwrap_or("video").to_string(),
        duration: v["duration"].as_f64().unwrap_or(0.0) as i64,
        w: v["width"].as_i64().unwrap_or(0),
        h: v["height"].as_i64().unwrap_or(0),
        opts,
        audio_size,
        direct: Vec::new(),
    }
}

/// yt-dlp format tanlovchisi: `h` balandlik ("1080") yoki "a" (faqat audio).
pub fn selector(fmt: &str) -> String {
    if fmt == "d" {
        return "b".to_string(); // to'g'ridan-to'g'ri fayl: bitta format
    }
    if fmt == "b" {
        return "bv*+ba/b".to_string(); // sifat ro'yxati yo'q: mavjud eng yaxshi fayl
    }
    match fmt.parse::<i64>() {
        Ok(h) => format!("bv*[height<={h}]+ba/b[height<={h}]/b"),
        Err(_) => "ba/b".to_string(),
    }
}

/// yt-dlp progress qatori: `  45.3%|  2.10MiB/s|00:12` (shablon turi "download:" chiqishga KIRMAYDI). Mos kelsa tozalangan qator.
pub fn progress_line(l: &str) -> Option<String> {
    let t = l.trim();
    let mut it = t.split('|');
    let pc = it.next()?.trim();
    it.next()?;
    it.next()?;
    if pc.ends_with('%') && pc.trim_end_matches('%').trim().parse::<f64>().is_ok() {
        Some(t.to_string())
    } else {
        None
    }
}

pub fn human(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1024.0 * 1024.0 * 1024.0 { format!("{:.1} GB", b / 1073741824.0) } else if b >= 1048576.0 { format!("{:.0} MB", b / 1048576.0) } else { format!("{:.0} KB", b / 1024.0) }
}

/// Fayl nomi uchun xavfsiz sarlavha.
pub fn safe_name(title: &str, ext: &str) -> String {
    let t: String = title.chars().map(|c| if c.is_alphanumeric() || " -_.()".contains(c) { c } else { '_' }).collect();
    let t: String = t.trim().chars().take(80).collect();
    format!("{}.{ext}", if t.is_empty() { "video" } else { &t })
}

fn base(cfg: &Cfg) -> Command {
    let mut c = Command::new(&cfg.ytdlp);
    c.arg("--no-playlist").arg("--no-warnings").arg("--no-colors").arg("--no-part");
    if let Some(ck) = &cfg.cookies {
        c.arg("--cookies").arg(ck);
    }
    c.kill_on_drop(true);
    c
}

pub async fn probe(cfg: &Cfg, url: &str) -> Result<Info> {
    match probe_ytdlp(cfg, url).await {
        Ok(i) => Ok(i),
        Err(e) => {
            // Rumble sahifasi serverga Cloudflare tekshiruvini beradi (403); embed sahifasi va CDN esa ochiq — to'g'ridan-to'g'ri havolalar olinadi.
            if rumble_id(url).is_some() {
                match rumble_probe(url).await {
                    Ok(i) => return Ok(i),
                    Err(e2) => return Err(anyhow!("{e}\n(Rumble embed yo'li ham ishlamadi: {e2})")),
                }
            }
            Err(e)
        }
    }
}

// ── Rumble: embed sahifasidan to'g'ridan-to'g'ri MP4 ──────────

const UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36";

/// `https://rumble.com/v74j7sy-nom.html` yoki `/embed/v74j7sy/` -> `v74j7sy`.
pub fn rumble_id(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"))?;
    let (host, path) = rest.split_once('/')?;
    if !(host == "rumble.com" || host == "www.rumble.com") {
        return None;
    }
    let seg = if let Some(e) = path.strip_prefix("embed/") { e } else { path };
    let id: String = seg.split(['-', '/', '.', '?']).next()?.to_string();
    if id.len() >= 5 && id.starts_with('v') && id.chars().all(|c| c.is_ascii_alphanumeric()) { Some(id) } else { None }
}

/// `start` dagi `{` dan boshlanuvchi JSON obyektini (qavslar muvozanati, qator ichidagi qavslarni hisobga olmay) kesib oladi.
fn json_object_at(s: &str, start: usize) -> Option<&str> {
    let b = s.as_bytes();
    if *b.get(start)? != b'{' {
        return None;
    }
    let (mut depth, mut in_str, mut esc) = (0i32, false, false);
    for (i, &c) in b.iter().enumerate().skip(start) {
        if in_str {
            if esc {
                esc = false;
            } else if c == b'\\' {
                esc = true;
            } else if c == b'"' {
                in_str = false;
            }
            continue;
        }
        match c {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return s.get(start..=i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Embed sahifasi matnidan video ma'lumoti (`p.f["<id>"]={...}`): sarlavha, davomiylik, balandlik -> havola.
pub fn parse_rumble_embed(html: &str, id: &str) -> Option<Info> {
    let key = format!("p.f[\"{id}\"]=");
    let at = html.find(&key)? + key.len();
    // Obyekt oxirida JavaScript bor (`,loaded:v()}`) — bu JSON emas; shu qismdan oldini kesib `}` bilan yopamiz.
    let mut obj = json_object_at(html, at)?.to_string();
    if let Some(i) = obj.find(",loaded:") {
        obj.truncate(i);
        obj.push('}');
    }
    let v: Value = serde_json::from_str(&obj).ok()?;
    let mut direct: Vec<(i64, String, u64)> = Vec::new();
    if let Some(m) = v["ua"]["mp4"].as_object() {
        for (k, f) in m {
            if let (Ok(h), Some(u)) = (k.parse::<i64>(), f["url"].as_str()) {
                direct.push((h, u.to_string(), f["meta"]["size"].as_u64().unwrap_or(0)));
            }
        }
    }
    if direct.is_empty() {
        let u = v["u"]["mp4"]["url"].as_str()?;
        direct.push((v["h"].as_i64().unwrap_or(0), u.to_string(), v["u"]["mp4"]["meta"]["size"].as_u64().unwrap_or(0)));
    }
    direct.sort_by(|a, b| b.0.cmp(&a.0));
    let title = html.find("\"title\":\"").map(|i| &html[i + 9..]).and_then(|r| r.split('"').next()).unwrap_or("rumble").to_string();
    let duration = html.find("\"duration\":").and_then(|i| html[i + 11..].split(|c: char| !c.is_ascii_digit()).next()?.parse::<i64>().ok()).unwrap_or(0);
    Some(Info {
        title,
        duration,
        w: v["w"].as_i64().unwrap_or(0),
        h: v["h"].as_i64().unwrap_or(0),
        opts: direct.iter().map(|(h, _, sz)| Opt { h: *h, size: Some(*sz).filter(|s| *s > 0) }).collect(),
        audio_size: None,
        direct: direct.into_iter().map(|(h, u, _)| (h, u)).collect(),
    })
}

/// oEmbed javobidan haqiqiy embed ID: `...src="https://rumble.com/embed/v72cjsq/"...` -> `v72cjsq`.
pub fn embed_id_from_oembed(v: &Value) -> Option<String> {
    let html = v["html"].as_str()?;
    let rest = &html[html.find("/embed/")? + 7..];
    let id: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
    if id.len() >= 4 { Some(id) } else { None }
}

fn pct(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// Matnni `curl` bilan oladi (Cloudflare uni oddiy brauzerdek qabul qiladi); bo'lmasa o'zimizning HTTP mijoz.
async fn fetch_text(url: &str) -> Result<String> {
    if let Ok(o) = Command::new("curl").args(["-sL", "-m", "30", "-A", UA]).arg(url).output().await {
        if o.status.success() && !o.stdout.is_empty() {
            return Ok(String::from_utf8_lossy(&o.stdout).to_string());
        }
    }
    Ok(reqwest::Client::builder().user_agent(UA).timeout(std::time::Duration::from_secs(30)).build()?.get(url).send().await?.text().await?)
}

/// Rumble: sahifa ID'si (v74j7sy) va embed ID'si (v72cjsq) HAR XIL — haqiqiy embed ID oEmbed orqali olinadi, so'ng embed sahifasidan to'g'ridan-to'g'ri MP4 havolalari.
async fn rumble_probe(url: &str) -> Result<Info> {
    let (mut embed_id, mut title, mut dur) = (None::<String>, None::<String>, 0i64);
    if let Some(rest) = url.split("rumble.com/embed/").nth(1) {
        embed_id = Some(rest.chars().take_while(|c| c.is_ascii_alphanumeric()).collect());
    } else {
        let oe = fetch_text(&format!("https://rumble.com/api/Media/oembed.json?url={}", pct(url))).await?;
        let v: Value = serde_json::from_str(&oe).map_err(|_| anyhow!("Rumble oEmbed javobi tushunarsiz"))?;
        embed_id = embed_id_from_oembed(&v);
        title = v["title"].as_str().map(|t| t.to_string());
        dur = v["duration"].as_i64().unwrap_or(0);
    }
    let id = embed_id.ok_or_else(|| anyhow!("Rumble: embed ID topilmadi"))?;
    let html = fetch_text(&format!("https://rumble.com/embed/{id}/")).await?;
    if !html.contains("p.f[") {
        return Err(anyhow!("Rumble embed sahifasi video ma'lumotini bermadi ({} bayt)", html.len()));
    }
    let mut info = parse_rumble_embed(&html, &id).ok_or_else(|| anyhow!("Rumble embed ma'lumotini o'qib bo'lmadi"))?;
    if let Some(t) = title {
        info.title = t;
    }
    if dur > 0 {
        info.duration = dur;
    }
    Ok(info)
}

async fn probe_ytdlp(cfg: &Cfg, url: &str) -> Result<Info> {
    let mut c = base(cfg);
    c.arg("-J").arg("--").arg(url).stdout(Stdio::piped()).stderr(Stdio::piped());
    let out = tokio::time::timeout(std::time::Duration::from_secs(120), c.output()).await.map_err(|_| anyhow!("yt-dlp javob bermadi (120 s)"))??;
    if !out.status.success() {
        let e = String::from_utf8_lossy(&out.stderr);
        return Err(anyhow!("{}", e.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("yt-dlp xatosi").chars().take(300).collect::<String>()));
    }
    let v: Value = serde_json::from_slice(&out.stdout)?;
    let info = parse_info(&v);
    // Sifat ro'yxati bo'lmasa ham (balandlik berilmagan: Instagram, TikTok, oddiy fayl) mavjud fayl yuklanadi; faqat umuman format bo'lmasa xato.
    let has_any = v["url"].as_str().is_some() || v["formats"].as_array().map(|a| !a.is_empty()).unwrap_or(false) || v["entries"].is_array();
    if info.opts.is_empty() && info.audio_size.is_none() && !has_any {
        return Err(anyhow!("yuklab olinadigan fayl topilmadi"));
    }
    Ok(info)
}

/// Diskka yuklash (video+audio kerak bo'lsa ffmpeg birlashtiradi). `progress` — "45.3%|2.1MiB/s|00:12" ko'rinishidagi matn.
pub async fn download_file(cfg: &Cfg, url: &str, fmt: &str, dir: &Path, mut progress: impl FnMut(String)) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let mut c = base(cfg);
    c.arg("-f").arg(selector(fmt));
    if fmt == "a" {
        c.arg("-x").arg("--audio-format").arg("mp3");
    } else {
        c.arg("--merge-output-format").arg("mp4");
    }
    c.arg("-q").arg("--progress").arg("--newline").arg("--progress-template").arg("download:%(progress._percent_str)s|%(progress._speed_str)s|%(progress._eta_str)s");
    c.arg("-o").arg(dir.join("%(id)s.%(ext)s")).arg("--").arg(url);
    c.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = c.spawn()?;
    let out = child.stdout.take().ok_or_else(|| anyhow!("stdout yo'q"))?;
    let err = child.stderr.take().ok_or_else(|| anyhow!("stderr yo'q"))?;
    let err_task = tokio::spawn(async move {
        let mut last = String::new();
        let mut lines = BufReader::new(err).lines();
        while let Ok(Some(l)) = lines.next_line().await {
            if !l.trim().is_empty() {
                last = l;
            }
        }
        last
    });
    let mut lines = BufReader::new(out).lines();
    while let Ok(Some(l)) = lines.next_line().await {
        if let Some(p) = progress_line(&l) {
            progress(p);
        }
    }
    let status = child.wait().await?;
    let last_err = err_task.await.unwrap_or_default();
    if !status.success() {
        return Err(anyhow!("{}", last_err.chars().take(300).collect::<String>()));
    }
    // Natija: papkadagi eng katta fayl.
    let mut best: Option<(u64, PathBuf)> = None;
    for e in std::fs::read_dir(dir)?.flatten() {
        let m = e.metadata()?;
        if m.is_file() && best.as_ref().map(|(s, _)| m.len() > *s).unwrap_or(true) {
            best = Some((m.len(), e.path()));
        }
    }
    best.map(|(_, p)| p).ok_or_else(|| anyhow!("fayl yaratilmadi"))
}

// ── Bo'lish: Bot API fayl limiti 50 MB ──────────────────────

/// Bitta qism uchun qattiq chegara (bayt): Bot API 50 MB dan kattasini qabul qilmaydi.
pub const PART_LIMIT: u64 = 49_900_000;
/// Bo'lishda maqsad: qismlar 45–50 MB oralig'ida chiqishi uchun chegaraga yaqin (kesish kalit kadrda bo'lgani uchun odatda 45–48,5 MB).
pub const PART_TARGET: u64 = 48_500_000;

/// Fayl hajmi va davomiyligidan har qism necha soniya bo'lishi kerak (kamida 5 s).
pub fn seg_secs(size: u64, duration: f64, target: u64) -> u64 {
    if size == 0 || duration <= 0.0 {
        return 60;
    }
    let bps = size as f64 / duration; // bayt/soniya
    ((target as f64 / bps).floor() as u64).max(5)
}

/// `ffprobe` bilan davomiylik (soniya); bilinmasa 0.
pub async fn probe_duration(path: &Path) -> f64 {
    let out = Command::new("ffprobe").args(["-v", "error", "-show_entries", "format=duration", "-of", "default=nw=1:nk=1"]).arg(path).stdout(Stdio::piped()).stderr(Stdio::null()).output().await;
    out.ok().and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<f64>().ok()).unwrap_or(0.0)
}

/// Video uchun kichik rasm (thumbnail): Telegram talabi — JPEG, eni/bo'yi 320 px dan oshmasin, hajmi 200 KB dan kichik. Natija: rasm fayli yo'li.
pub async fn make_thumb(video: &Path, out: &Path) -> Option<PathBuf> {
    let dur = probe_duration(video).await;
    // Boshidan 1 soniya (qora kadr bo'lmasin), qisqa videoda o'rtasidan.
    let at = if dur > 3.0 { 1.0 } else { dur / 2.0 };
    for q in ["4", "8", "14", "22"] {
        let ok = Command::new("ffmpeg")
            .args(["-y", "-v", "error", "-ss", &format!("{at:.2}"), "-i"])
            .arg(video)
            .args(["-frames:v", "1", "-vf", "scale='min(320,iw)':'min(320,ih)':force_original_aspect_ratio=decrease", "-q:v", q])
            .arg(out)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false);
        if ok && std::fs::metadata(out).map(|m| m.len() > 0 && m.len() < 200_000).unwrap_or(false) {
            return Some(out.to_path_buf());
        }
    }
    let _ = std::fs::remove_file(out);
    None
}

/// Bitta paket: (ko'rsatish vaqti soniyada, hajm baytda, kalit kadrmi).
pub type Pkt = (f64, u64, bool);

/// `ffprobe` bilan video (yoki audio) paketlari: vaqt, hajm, kalit kadr belgisi.
pub async fn packets(path: &Path, stream: &str) -> Result<Vec<Pkt>> {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", stream, "-show_entries", "packet=pts_time,size,flags", "-of", "csv=p=0"])
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await?;
    let mut v: Vec<Pkt> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let mut it = l.split(',');
            let t: f64 = it.next()?.parse().ok()?;
            let sz: u64 = it.next()?.parse().ok()?;
            let key = it.next().map(|f| f.contains('K')).unwrap_or(false);
            Some((t, sz, key))
        })
        .collect();
    v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    Ok(v)
}

/// Kesish vaqtlari (soniya): har qism `target` bayt atrofida, lekin undan oshmaydi; kesish FAQAT kalit kadrda — video buzilmaydi.
/// `total` — butun fayl hajmi (audio va konteyner qo'shimchasi video hajmiga ulush bilan qo'shiladi).
pub fn plan_cuts(pkts: &[Pkt], total: u64, target: u64) -> Result<Vec<f64>> {
    let vsum: u64 = pkts.iter().map(|p| p.1).sum();
    if vsum == 0 {
        return Err(anyhow!("paketlar topilmadi"));
    }
    let f = (total as f64 / vsum as f64).max(1.0);
    let target = target as f64;
    let mut cuts: Vec<f64> = Vec::new();
    let mut acc = 0.0f64;
    let mut cand: Option<(f64, f64)> = None; // (kalit kadr vaqti, shu kadrgacha yig'ilgan bayt)
    for &(t, sz, key) in pkts {
        if key && acc > 0.0 && acc <= target {
            cand = Some((t, acc));
        }
        acc += sz as f64 * f;
        if acc > target {
            match cand.take() {
                Some((ct, a)) => {
                    cuts.push(ct);
                    acc -= a;
                    if acc > target {
                        return Err(anyhow!("kalit kadrlar juda siyrak: bitta guruh 50 MB dan katta"));
                    }
                }
                None => return Err(anyhow!("kalit kadrlar juda siyrak: bitta guruh 50 MB dan katta")),
            }
        }
    }
    Ok(cuts)
}

/// Faylni VAQT bo'yicha qismlarga bo'ladi (qayta kodlamasdan, kalit kadrlarda): har qism 45–50 MB atrofida. Qism chegaradan oshsa, maqsad kamaytirilib qayta uriniladi.
/// Natija: video tartibidagi qism fayllari (asl fayl o'chiriladi).
pub async fn split(path: &Path, dir: &Path, prog: Arc<AtomicU32>) -> Result<Vec<PathBuf>> {
    let size = std::fs::metadata(path)?.len();
    if size <= PART_LIMIT {
        prog.store(100, Ordering::Relaxed);
        return Ok(vec![path.to_path_buf()]);
    }
    let total_dur = probe_duration(path).await;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("mp4").to_string();
    let audio_only = !matches!(ext.as_str(), "mp4" | "mkv" | "webm" | "mov");
    let mut pkts = packets(path, if audio_only { "a:0" } else { "v:0" }).await.unwrap_or_default();
    if pkts.is_empty() {
        pkts = packets(path, "a:0").await.unwrap_or_default();
    }
    let mut target = PART_TARGET;
    let mut last_err = anyhow!("bo'lib bo'lmadi");
    for attempt in 0..4 {
        let sub = dir.join(format!("s{attempt}"));
        let _ = std::fs::remove_dir_all(&sub);
        std::fs::create_dir_all(&sub)?;
        // Kesish vaqtlari: paketlar bo'yicha aniq reja; bo'lmasa (ffprobe bermasa) o'rtacha bitreytdan teng vaqt.
        let times: Vec<f64> = match plan_cuts(&pkts, size, target) {
            Ok(c) => c,
            Err(e) if pkts.is_empty() => {
                let dur = probe_duration(path).await;
                let seg = seg_secs(size, dur, target) as f64;
                let n = if seg > 0.0 { (dur / seg).floor() as usize } else { 0 };
                let _ = e;
                (1..=n).map(|i| i as f64 * seg).collect()
            }
            Err(e) => return Err(e),
        };
        let mut c = Command::new("ffmpeg");
        c.args(["-y", "-v", "error", "-i"]).arg(path).args(["-map", "0:v?", "-map", "0:a?", "-c", "copy", "-f", "segment"]);
        if !times.is_empty() {
            // Kalit kadr vaqtidan 1 ms oldin: segmentlovchi aynan o'sha kalit kadrdan boshlaydi.
            c.arg("-segment_times").arg(times.iter().map(|t| format!("{:.3}", (t - 0.001).max(0.0))).collect::<Vec<_>>().join(","));
        }
        c.args(["-reset_timestamps", "1"]);
        if ext == "mp4" {
            c.args(["-segment_format_options", "movflags=+faststart"]);
        }
        // Foiz: ffmpeg `-progress pipe:1` (out_time_us) / umumiy davomiylik.
        prog.store(0, Ordering::Relaxed);
        c.args(["-progress", "pipe:1", "-nostats"]);
        c.arg(sub.join(format!("p%03d.{ext}"))).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
        let mut child = c.spawn()?;
        let so = child.stdout.take();
        let se = child.stderr.take();
        let err_task = tokio::spawn(async move {
            let mut last = String::new();
            if let Some(se) = se {
                let mut lines = BufReader::new(se).lines();
                while let Ok(Some(l)) = lines.next_line().await {
                    if !l.trim().is_empty() {
                        last = l;
                    }
                }
            }
            last
        });
        if let Some(so) = so {
            let mut lines = BufReader::new(so).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                if let Some(us) = l.strip_prefix("out_time_us=").and_then(|v| v.trim().parse::<f64>().ok()) {
                    if total_dur > 0.0 {
                        prog.store(((us / 1_000_000.0 / total_dur) * 100.0).clamp(0.0, 99.0) as u32, Ordering::Relaxed);
                    }
                }
            }
        }
        let status = child.wait().await?;
        let ff_err = err_task.await.unwrap_or_default();
        if !status.success() {
            return Err(anyhow!("ffmpeg: {}", ff_err));
        }
        prog.store(100, Ordering::Relaxed);
        let mut parts: Vec<PathBuf> = std::fs::read_dir(&sub)?.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
        parts.sort();
        if parts.is_empty() {
            return Err(anyhow!("ffmpeg qism yaratmadi"));
        }
        let biggest = parts.iter().filter_map(|p| std::fs::metadata(p).ok()).map(|m| m.len()).max().unwrap_or(0);
        if biggest <= PART_LIMIT {
            let _ = std::fs::remove_file(path); // asl fayl endi kerak emas — disk to'lib qolmasin
            return Ok(parts);
        }
        last_err = anyhow!("qism {} dan katta chiqdi (kalit kadrlar siyrak)", human(biggest));
        let _ = std::fs::remove_dir_all(&sub);
        target = target * 92 / 100; // maqsad kamaytirilib qayta uriniladi
    }
    Err(last_err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> Value {
        json!({
            "title": "Test video", "duration": 125.4, "width": 1920, "height": 1080,
            "formats": [
                {"format_id": "140", "vcodec": "none", "acodec": "mp4a.40.2", "filesize": 2_000_000, "protocol": "https", "ext": "m4a"},
                {"format_id": "18", "vcodec": "avc1", "acodec": "mp4a", "height": 360, "filesize": 10_000_000, "protocol": "https", "ext": "mp4"},
                {"format_id": "137", "vcodec": "avc1", "acodec": "none", "height": 1080, "filesize": 80_000_000, "protocol": "https", "ext": "mp4"},
                {"format_id": "136", "vcodec": "avc1", "acodec": "none", "height": 720, "filesize_approx": 40_000_000.0, "protocol": "https", "ext": "mp4"},
                {"format_id": "sb0", "vcodec": "none", "acodec": "none", "height": 90}
            ]
        })
    }

    #[test]
    fn parses_heights_and_sizes() {
        let i = parse_info(&sample());
        assert_eq!(i.title, "Test video");
        assert_eq!(i.duration, 125);
        assert_eq!(i.opts.iter().map(|o| o.h).collect::<Vec<_>>(), vec![1080, 720, 360]);
        assert_eq!(i.opts[0].size, Some(82_000_000)); // video-only + audio
        assert_eq!(i.opts[1].size, Some(42_000_000)); // taxminiy hajm ham hisobga olinadi
        assert_eq!(i.audio_size, Some(2_000_000));
    }

    #[test]
    fn oembed_gives_real_embed_id() {
        let v = json!({"html": "<iframe src=\"https://rumble.com/embed/v72cjsq/\" width=\"1024\"></iframe>", "title": "Duo aura"});
        assert_eq!(embed_id_from_oembed(&v), Some("v72cjsq".to_string()));
        assert_eq!(embed_id_from_oembed(&json!({"html": "yo'q"})), None);
        assert_eq!(pct("https://rumble.com/v1-a b.html"), "https%3A%2F%2Frumble.com%2Fv1-a%20b.html");
    }

    #[test]
    fn rumble_ids_and_embed_parsing() {
        assert_eq!(rumble_id("https://rumble.com/v74j7sy-duo-aura-999999.html"), Some("v74j7sy".into()));
        assert_eq!(rumble_id("https://rumble.com/embed/v74j7sy/"), Some("v74j7sy".into()));
        assert_eq!(rumble_id("https://youtube.com/v74j7sy"), None);
        assert_eq!(rumble_id("https://rumble.com/user/x"), None);
        let html = r#"<script>p.f["v74j7sy"]={"fps":29.97,"w":1280,"h":720,"u":{"mp4":{"url":"https:\/\/c.example\/a.gaa.mp4","meta":{"size":71227293,"w":1280,"h":720}}},"ua":{"mp4":{"360":{"url":"https:\/\/c.example\/a.baa.mp4","meta":{"bitrate":636,"size":21938969,"w":640,"h":360}},"720":{"url":"https:\/\/c.example\/a.gaa.mp4","meta":{"size":71227293,"w":1280,"h":720}}},"timeline":{"180":{"url":"x"}}},"x":"}{","viewer_id":"abc",loaded:v()};</script> {"title":"Salom","duration":275}"#;
        let i = parse_rumble_embed(html, "v74j7sy").unwrap();
        assert_eq!(i.title, "Salom");
        assert_eq!(i.duration, 275);
        assert_eq!(i.opts.iter().map(|o| o.h).collect::<Vec<_>>(), vec![720, 360]);
        assert_eq!(i.direct[0], (720, "https://c.example/a.gaa.mp4".to_string()));
        assert_eq!(i.opts[1].size, Some(21938969));
    }

    /// Haqiqiy Rumble embed sahifasi bilan (RUMBLE_FIXTURE=/yo'l/emb.html RUMBLE_ID=v74j7sy); berilmasa o'tkaziladi.
    #[test]
    fn real_rumble_embed_page() {
        let (Ok(path), Ok(id)) = (std::env::var("RUMBLE_FIXTURE"), std::env::var("RUMBLE_ID")) else { return };
        let html = std::fs::read_to_string(path).unwrap();
        let i = parse_rumble_embed(&html, &id).expect("embed o'qilmadi");
        assert!(!i.direct.is_empty());
        assert!(i.direct.iter().all(|(_, u)| u.starts_with("https://") && u.ends_with(".mp4")), "{:?}", i.direct);
        eprintln!("rumble: {} | {} s | {:?}", i.title, i.duration, i.opts);
    }

    #[test]
    fn no_height_formats_still_offer_a_download() {
        // Instagram/TikTok kabi: balandlik yo'q, vcodec noma'lum — sifat tugmalari bo'sh, lekin format bor.
        let v = json!({"title": "Reel", "duration": 12.0, "formats": [{"format_id": "mp4", "url": "https://x/y.mp4", "ext": "mp4"}]});
        let i = parse_info(&v);
        assert!(i.opts.is_empty());
        assert_eq!(selector("b"), "bv*+ba/b");
        // vcodec noma'lum, lekin balandlik bor — video sifatida hisoblanadi.
        let v2 = json!({"title": "X", "formats": [{"format_id": "1", "height": 720, "filesize": 5_000_000, "ext": "mp4"}]});
        assert_eq!(parse_info(&v2).opts.iter().map(|o| o.h).collect::<Vec<_>>(), vec![720]);
    }

    #[test]
    fn parses_ytdlp_progress_lines() {
        assert_eq!(progress_line("  0.1%|  11.86MiB/s|00:02"), Some("0.1%|  11.86MiB/s|00:02".to_string()));
        assert_eq!(progress_line(" 45.3%| Unknown B/s|Unknown").is_some(), true);
        assert_eq!(progress_line("WARNING: [generic] Falling back"), None);
        assert_eq!(progress_line("[download] Destination: x.mp4"), None);
    }

    #[test]
    fn selectors_names_and_segments() {
        assert_eq!(selector("720"), "bv*[height<=720]+ba/b[height<=720]/b");
        assert_eq!(selector("a"), "ba/b");
        assert_eq!(human(1_500_000), "1 MB");
        assert_eq!(human(3 * 1024 * 1024 * 1024), "3.0 GB");
        assert_eq!(safe_name("Salom/dunyo: 1?", "mp4"), "Salom_dunyo_ 1_.mp4");
        assert_eq!(safe_name("", "mp3"), "video.mp3");
        // 500 MB / 1000 s = 0,5 MB/s -> 48,5 MB = ~97 s
        assert_eq!(seg_secs(500_000_000, 1000.0, PART_TARGET), 97);
        assert_eq!(seg_secs(0, 0.0, PART_TARGET), 60);
        assert_eq!(seg_secs(100_000_000_000, 10.0, PART_TARGET), 5); // juda yuqori bitreyt: kamida 5 s
    }

    /// 1 s li paketlar, har 2 s da kalit kadr (jami 2 MB guruh), har paket 1 MB: kesishlar kalit kadrlarda, har qism <= maqsad va maqsadga yaqin.
    #[test]
    fn cuts_land_on_keyframes_and_fit_target() {
        let pkts: Vec<Pkt> = (0..200).map(|i| (i as f64, 1_000_000u64, i % 2 == 0)).collect();
        let cuts = plan_cuts(&pkts, 200_000_000, 10_500_000).unwrap();
        assert!(!cuts.is_empty());
        let mut prev = 0.0;
        for c in &cuts {
            assert_eq!(*c as u64 % 2, 0, "kesish kalit kadrda bo'lishi kerak"); // kalit kadrlar juft soniyalarda
            let bytes = (c - prev) * 1_000_000.0;
            assert!(bytes <= 10_500_000.0 && bytes >= 8_500_000.0, "qism hajmi {bytes}");
            prev = *c;
        }
    }

    #[test]
    fn huge_gop_is_an_error() {
        // 100 s lik bitta kalit guruh (100 MB) — 48,5 MB ga sig'maydi.
        let pkts: Vec<Pkt> = (0..100).map(|i| (i as f64, 1_000_000u64, i == 0)).collect();
        assert!(plan_cuts(&pkts, 100_000_000, 48_500_000).is_err());
    }

    /// Haqiqiy ffmpeg bilan: ~130 MB video yasaladi, bo'linadi; har qism <= 49,9 MB va oxirgisidan boshqalari >= 45 MB bo'lishi kerak. ffmpeg bo'lmasa o'tkaziladi.
    #[tokio::test]
    async fn split_real_video_parts_between_45_and_50_mb() {
        if std::process::Command::new("ffmpeg").arg("-version").output().is_err() || std::process::Command::new("ffprobe").arg("-version").output().is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("ytbot-split-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("in.mp4");
        let st = std::process::Command::new("ffmpeg")
            .args(["-y", "-v", "error", "-f", "lavfi", "-i", "testsrc2=size=640x360:rate=25", "-f", "lavfi", "-i", "sine=frequency=440", "-vf", "noise=alls=70:allf=t+u", "-t", "130", "-c:v", "libx264", "-b:v", "8M", "-maxrate", "8M", "-bufsize", "2M", "-g", "50", "-c:a", "aac", "-shortest"])
            .arg(&src)
            .status()
            .unwrap();
        assert!(st.success());
        let before = std::fs::metadata(&src).unwrap().len();
        assert!(before > 100_000_000, "sinov videosi kichik: {before}");
        let parts = split(&src, &dir, Arc::new(AtomicU32::new(0))).await.unwrap();
        assert!(parts.len() >= 3, "qismlar soni {}", parts.len());
        let sizes: Vec<u64> = parts.iter().map(|p| std::fs::metadata(p).unwrap().len()).collect();
        for (i, sz) in sizes.iter().enumerate() {
            assert!(*sz <= PART_LIMIT, "{i}-qism {sz} chegaradan katta");
            if i + 1 < sizes.len() {
                assert!(*sz >= 45_000_000, "{i}-qism 45 MB dan kichik: {sz}");
            }
        }
        // Har qism ochiladigan (buzilmagan) video va undan thumbnail yasaladi (JPEG, < 200 KB).
        for p in &parts {
            assert!(probe_duration(p).await > 1.0, "{p:?} ochilmadi");
        }
        let th = make_thumb(&parts[0], &dir.join("t.jpg")).await.expect("thumbnail yasalmadi");
        let tsz = std::fs::metadata(&th).unwrap().len();
        assert!(tsz > 500 && tsz < 200_000, "thumbnail hajmi {tsz}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
