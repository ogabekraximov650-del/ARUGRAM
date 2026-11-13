// UZ: yt-dlp bilan ishlash: ma'lumot olish (sifatlar), diskka yuklash, oqim (stdout). EN: talking to yt-dlp: probing formats, file download, stdout streaming.
use crate::Cfg;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

/// Bitta sifat tugmasi.
#[derive(Clone, Debug, PartialEq)]
pub struct Opt {
    pub h: i64,
    /// Taxminiy yakuniy hajm (bayt), bilinsa.
    pub size: Option<u64>,
    /// Oqim uchun nomzod: bitta tayyor fayl (video+audio birga), hajmi ANIQ ma'lum — (format_id, bayt, kengaytma).
    pub stream: Option<(String, u64, String)>,
}

#[derive(Clone, Debug, Default)]
pub struct Info {
    pub title: String,
    pub duration: i64,
    pub w: i64,
    pub h: i64,
    pub opts: Vec<Opt>,
    pub audio_size: Option<u64>,
}

fn has_codec(v: &Value) -> bool {
    matches!(v.as_str(), Some(s) if s != "none" && !s.is_empty())
}

fn size_of(f: &Value) -> Option<u64> {
    f["filesize"].as_u64().or_else(|| f["filesize_approx"].as_f64().map(|x| x as u64)).filter(|s| *s > 0)
}

/// `yt-dlp -J` javobidan sifatlar ro'yxati (balandlik bo'yicha kamayish tartibida).
pub fn parse_info(v: &Value) -> Info {
    let formats = v["formats"].as_array().cloned().unwrap_or_default();
    let mut heights: Vec<i64> = formats.iter().filter(|f| has_codec(&f["vcodec"])).filter_map(|f| f["height"].as_i64()).filter(|h| *h >= 144).collect();
    heights.sort_unstable_by(|a, b| b.cmp(a));
    heights.dedup();
    let audio_size = formats.iter().filter(|f| !has_codec(&f["vcodec"]) && has_codec(&f["acodec"])).filter_map(size_of).max();
    let mut opts = Vec::new();
    for h in heights {
        let at_h: Vec<&Value> = formats.iter().filter(|f| has_codec(&f["vcodec"]) && f["height"].as_i64() == Some(h)).collect();
        let progressive = at_h.iter().filter(|f| has_codec(&f["acodec"])).filter_map(|f| size_of(f)).max();
        let video_only = at_h.iter().filter(|f| !has_codec(&f["acodec"])).filter_map(|f| size_of(f)).max();
        let size = match (video_only, audio_size) {
            (Some(v), Some(a)) => Some(v + a),
            (Some(v), None) => Some(v),
            _ => progressive,
        };
        // Oqim nomzodi: aniq hajmli (`filesize`), oddiy http(s) yuklanadigan, video+audio birga fayl.
        let stream = at_h
            .iter()
            .filter(|f| has_codec(&f["acodec"]) && f["filesize"].as_u64().unwrap_or(0) > 0)
            .filter(|f| f["protocol"].as_str().map(|p| p == "https" || p == "http").unwrap_or(false))
            .max_by_key(|f| f["filesize"].as_u64().unwrap_or(0))
            .map(|f| (f["format_id"].as_str().unwrap_or("").to_string(), f["filesize"].as_u64().unwrap_or(0), f["ext"].as_str().unwrap_or("mp4").to_string()))
            .filter(|(id, _, _)| !id.is_empty());
        opts.push(Opt { h, size, stream });
    }
    Info {
        title: v["title"].as_str().unwrap_or("video").to_string(),
        duration: v["duration"].as_f64().unwrap_or(0.0) as i64,
        w: v["width"].as_i64().unwrap_or(0),
        h: v["height"].as_i64().unwrap_or(0),
        opts,
        audio_size,
    }
}

/// yt-dlp format tanlovchisi: `h` balandlik ("1080") yoki "a" (faqat audio).
pub fn selector(fmt: &str) -> String {
    match fmt.parse::<i64>() {
        Ok(h) => format!("bv*[height<={h}]+ba/b[height<={h}]/b"),
        Err(_) => "ba/b".to_string(),
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
    let mut c = base(cfg);
    c.arg("-J").arg("--").arg(url).stdout(Stdio::piped()).stderr(Stdio::piped());
    let out = tokio::time::timeout(std::time::Duration::from_secs(120), c.output()).await.map_err(|_| anyhow!("yt-dlp javob bermadi (120 s)"))??;
    if !out.status.success() {
        let e = String::from_utf8_lossy(&out.stderr);
        return Err(anyhow!("{}", e.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("yt-dlp xatosi").chars().take(300).collect::<String>()));
    }
    let v: Value = serde_json::from_slice(&out.stdout)?;
    let info = parse_info(&v);
    if info.opts.is_empty() && info.audio_size.is_none() && v["url"].as_str().is_none() {
        return Err(anyhow!("sifatlar topilmadi"));
    }
    Ok(info)
}

/// Diskka yuklash (video+audio kerak bo'lsa ffmpeg birlashtiradi). `progress` — "45.3%|2.1MiB/s|00:12" ko'rinishidagi matn.
pub async fn download_file(cfg: &Cfg, url: &str, fmt: &str, dir: &Path, mut progress: impl FnMut(String)) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let mut c = base(cfg);
    c.arg("-f").arg(selector(fmt));
    if fmt.parse::<i64>().is_ok() {
        c.arg("--merge-output-format").arg("mp4");
    } else {
        c.arg("-x").arg("--audio-format").arg("mp3");
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
        if let Some(p) = l.strip_prefix("download:") {
            progress(p.trim().to_string());
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

/// Oqim: yt-dlp videoni stdout'ga yozadi (diskka emas).
pub fn spawn_stream(cfg: &Cfg, url: &str, format_id: &str) -> Result<Child> {
    let mut c = base(cfg);
    c.arg("-f").arg(format_id).arg("-q").arg("-o").arg("-").arg("--").arg(url);
    c.stdout(Stdio::piped()).stderr(Stdio::null());
    Ok(c.spawn()?)
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
                {"format_id": "hls-360", "vcodec": "avc1", "acodec": "mp4a", "height": 360, "filesize": 11_000_000, "protocol": "m3u8_native", "ext": "mp4"},
                {"format_id": "sb0", "vcodec": "none", "acodec": "none", "height": 90}
            ]
        })
    }

    #[test]
    fn parses_heights_sizes_and_stream_candidates() {
        let i = parse_info(&sample());
        assert_eq!(i.title, "Test video");
        assert_eq!(i.duration, 125);
        assert_eq!(i.opts.iter().map(|o| o.h).collect::<Vec<_>>(), vec![1080, 720, 360]);
        assert_eq!(i.opts[0].size, Some(82_000_000)); // video-only + audio
        assert_eq!(i.opts[1].size, Some(42_000_000)); // taxminiy hajm ham hisobga olinadi
        assert!(i.opts[0].stream.is_none()); // alohida video+audio — oqim emas
        // 360p: tayyor fayl (https, aniq hajm); m3u8 nomzod emas.
        assert_eq!(i.opts[2].stream, Some(("18".to_string(), 10_000_000, "mp4".to_string())));
        assert_eq!(i.audio_size, Some(2_000_000));
    }

    #[test]
    fn selectors_and_names() {
        assert_eq!(selector("720"), "bv*[height<=720]+ba/b[height<=720]/b");
        assert_eq!(selector("a"), "ba/b");
        assert_eq!(human(1_500_000), "1 MB");
        assert_eq!(human(3 * 1024 * 1024 * 1024), "3.0 GB");
        assert_eq!(safe_name("Salom/dunyo: 1?", "mp4"), "Salom_dunyo_ 1_.mp4");
        assert_eq!(safe_name("", "mp3"), "video.mp3");
    }
}
