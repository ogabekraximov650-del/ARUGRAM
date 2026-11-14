// UZ: Veb-sahifadagi rasmlarni topadi, yuklaydi va PDF (har bir rasm bitta sahifa) qiladi. Qo'shimcha kutubxona yo'q: PDF qo'lda yoziladi, JPEG qayta siqilmaydi.
// EN: Finds the images on a web page, downloads them and builds a PDF (one image per page). No extra crates: the PDF is written by hand, JPEGs are embedded as-is.
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;

const UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36";
/// UZ: Bitta PDF ning chegarasi (Bot API 50 MB). EN: Size limit for one PDF (Bot API 50 MB).
pub const PDF_LIMIT: u64 = 45_000_000;
const MAX_IMAGES: usize = 300;
const MAX_ONE: usize = 30_000_000;
const SKIP_WORDS: [&str; 10] = ["logo", "icon", "avatar", "sprite", "emoji", "blank.", "pixel", "spacer", "favicon", "loading"];

fn attr(tag: &str, name: &str) -> Option<String> {
    let low = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(p) = low[from..].find(name) {
        let i = from + p;
        let before_ok = i == 0 || low.as_bytes()[i - 1].is_ascii_whitespace();
        let rest = tag[i + name.len()..].trim_start();
        if before_ok && rest.starts_with('=') {
            let v = rest[1..].trim_start();
            let (q, body) = match v.chars().next()? {
                c @ ('"' | '\'') => (Some(c), &v[1..]),
                _ => (None, v),
            };
            let end = match q {
                Some(c) => body.find(c)?,
                None => body.find(|c: char| c.is_whitespace() || c == '>').unwrap_or(body.len()),
            };
            return Some(body[..end].replace("&amp;", "&").trim().to_string());
        }
        from = i + name.len();
    }
    None
}

/// UZ: `srcset="a 400w, b 1200w"` ichidan eng kattasi. EN: the largest candidate of a srcset.
fn best_srcset(s: &str) -> Option<String> {
    s.split(',')
        .filter_map(|c| {
            let mut it = c.split_whitespace();
            let u = it.next()?;
            let w = it.next().and_then(|x| x.trim_end_matches(|ch: char| !ch.is_ascii_digit()).parse::<u32>().ok()).unwrap_or(0);
            Some((w, u.to_string()))
        })
        .max_by_key(|(w, _)| *w)
        .map(|(_, u)| u)
}

fn title_of(html: &str) -> String {
    let low = html.to_ascii_lowercase();
    if let Some(a) = low.find("<title") {
        if let Some(b) = low[a..].find('>') {
            let s = a + b + 1;
            if let Some(e) = low[s..].find("</title>") {
                return html[s..s + e].replace("&amp;", "&").replace("&#39;", "'").replace("&quot;", "\"").trim().to_string();
            }
        }
    }
    String::new()
}

/// UZ: HTML ichidagi rasm havolalari (tartib saqlanadi, takrorlari va reklama/ikonkalar tashlanadi). EN: image URLs in the HTML, ordered, deduped, junk skipped.
pub fn extract_images(html: &str, base: &str) -> Vec<String> {
    let base_url = match reqwest::Url::parse(base) {
        Ok(u) => u,
        Err(_) => return vec![],
    };
    let low = html.to_ascii_lowercase();
    let mut out: Vec<String> = Vec::new();
    let mut pos = 0;
    while let Some(p) = low[pos..].find("<img") {
        let s = pos + p;
        let Some(e) = low[s..].find('>') else { break };
        let tag = &html[s..s + e];
        pos = s + e;
        let raw = ["data-src", "data-original", "data-lazy-src", "data-lazyload", "data-url"]
            .iter()
            .find_map(|a| attr(tag, a))
            .or_else(|| attr(tag, "srcset").and_then(|v| best_srcset(&v)))
            .or_else(|| attr(tag, "data-srcset").and_then(|v| best_srcset(&v)))
            .or_else(|| attr(tag, "src"));
        let Some(raw) = raw.filter(|r| !r.is_empty() && !r.starts_with("data:")) else { continue };
        let Ok(u) = base_url.join(&raw) else { continue };
        if u.scheme() != "http" && u.scheme() != "https" {
            continue;
        }
        let s = u.to_string();
        let l = s.to_ascii_lowercase();
        let path = l.split(['?', '#']).next().unwrap_or("");
        if path.ends_with(".svg") || path.ends_with(".gif") || SKIP_WORDS.iter().any(|w| path.contains(w)) {
            continue;
        }
        if !out.contains(&s) {
            out.push(s);
        }
        if out.len() >= MAX_IMAGES {
            break;
        }
    }
    out
}

/// UZ: Sahifani yuklab (sarlavha, rasm havolalari). EN: fetches the page and returns (title, image URLs).
pub async fn find_images(url: &str) -> Result<(String, Vec<String>)> {
    let html = crate::dl::fetch_text(url).await?;
    let imgs = extract_images(&html, url);
    if imgs.is_empty() {
        return Err(anyhow!("sahifada rasm topilmadi"));
    }
    let t = title_of(&html);
    Ok((if t.is_empty() { "sahifa".to_string() } else { t }, imgs))
}

/// UZ: JPEG: (kenglik, balandlik, komponentlar soni). EN: JPEG (width, height, components) from the SOF marker.
pub fn jpeg_info(b: &[u8]) -> Option<(u32, u32, u8)> {
    if b.len() < 4 || b[0] != 0xFF || b[1] != 0xD8 {
        return None;
    }
    let mut i = 2;
    while i + 4 <= b.len() {
        if b[i] != 0xFF {
            i += 1;
            continue;
        }
        let m = b[i + 1];
        if m == 0xFF {
            i += 1;
            continue;
        }
        if m == 0x01 || (0xD0..=0xD9).contains(&m) {
            i += 2;
            continue;
        }
        let len = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
        if (0xC0..=0xCF).contains(&m) && m != 0xC4 && m != 0xC8 && m != 0xCC {
            if i + 10 > b.len() {
                return None;
            }
            let h = u16::from_be_bytes([b[i + 5], b[i + 6]]) as u32;
            let w = u16::from_be_bytes([b[i + 7], b[i + 8]]) as u32;
            return Some((w, h, b[i + 9]));
        }
        i += 2 + len;
    }
    None
}

pub struct Img {
    pub w: u32,
    pub h: u32,
    pub gray: bool,
    pub data: Vec<u8>,
}

/// UZ: Rasmlar ro'yxatidan PDF baytlari (har rasm — bitta sahifa; o'lcham px*0.5 pt). EN: PDF bytes from images (one per page, page size = px*0.5 pt).
pub fn make_pdf(imgs: &[Img]) -> Vec<u8> {
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offs: Vec<usize> = Vec::new();
    let n = imgs.len();
    let obj = |out: &mut Vec<u8>, offs: &mut Vec<usize>, body: &[u8]| {
        offs.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", offs.len()).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    };
    obj(&mut out, &mut offs, b"<< /Type /Catalog /Pages 2 0 R >>");
    let kids: String = (0..n).map(|k| format!("{} 0 R", 5 + 3 * k)).collect::<Vec<_>>().join(" ");
    obj(&mut out, &mut offs, format!("<< /Type /Pages /Kids [{kids}] /Count {n} >>").as_bytes());
    // UZ: har sahifa uchun uchlik: rasm=3+3k, kontent=4+3k, sahifa=5+3k. EN: per page triple: image=3+3k, content=4+3k, page=5+3k.
    for (k, im) in imgs.iter().enumerate() {
        let (pw, ph) = (im.w as f64 * 0.5, im.h as f64 * 0.5);
        let cs = if im.gray { "/DeviceGray" } else { "/DeviceRGB" };
        let mut body = format!("<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace {cs} /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n", im.w, im.h, im.data.len()).into_bytes();
        body.extend_from_slice(&im.data);
        body.extend_from_slice(b"\nendstream");
        obj(&mut out, &mut offs, &body);
        let content = format!("q {pw:.2} 0 0 {ph:.2} 0 0 cm /Im0 Do Q");
        obj(&mut out, &mut offs, format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()).as_bytes());
        let page = format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {pw:.2} {ph:.2}] /Resources << /XObject << /Im0 {} 0 R >> >> /Contents {} 0 R >>", 3 + 3 * k, 4 + 3 * k);
        obj(&mut out, &mut offs, page.as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", offs.len() + 1).as_bytes());
    for o in &offs {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", offs.len() + 1).as_bytes());
    out
}

async fn to_jpeg(bytes: &[u8], dir: &Path, k: usize) -> Option<Vec<u8>> {
    let src = dir.join(format!("c{k}.bin"));
    let dst = dir.join(format!("c{k}.jpg"));
    tokio::fs::write(&src, bytes).await.ok()?;
    let ok = tokio::process::Command::new("ffmpeg")
        .args(["-y", "-v", "error", "-i"])
        .arg(&src)
        .args(["-frames:v", "1", "-q:v", "3", "-pix_fmt", "yuvj420p"])
        .arg(&dst)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false);
    let r = if ok { tokio::fs::read(&dst).await.ok() } else { None };
    let _ = tokio::fs::remove_file(&src).await;
    let _ = tokio::fs::remove_file(&dst).await;
    r
}

/// UZ: Rasmlarni yuklab PDF fayl(lar)ga yig'adi; har PDF `PDF_LIMIT` dan oshmaydi. `progress(i, n)` har rasmdan keyin chaqiriladi.
/// EN: Downloads the images and packs them into PDF file(s), each below `PDF_LIMIT`. `progress(i, n)` is called after each image.
pub async fn build_pdfs(urls: &[String], referer: &str, dir: &Path, name: &str, progress: impl Fn(usize, usize)) -> Result<Vec<PathBuf>> {
    tokio::fs::create_dir_all(dir).await?;
    let client = reqwest::Client::builder().user_agent(UA).timeout(Duration::from_secs(40)).build()?;
    let mut parts: Vec<Vec<Img>> = vec![vec![]];
    let mut cur = 0u64;
    for (i, u) in urls.iter().enumerate() {
        progress(i, urls.len());
        let got = async {
            let r = client.get(u).header("Referer", referer).send().await.ok()?;
            if !r.status().is_success() || r.content_length().unwrap_or(0) > MAX_ONE as u64 {
                return None;
            }
            let b = r.bytes().await.ok()?;
            (b.len() <= MAX_ONE && b.len() > 3000).then(|| b.to_vec())
        }
        .await;
        let Some(mut bytes) = got else { continue };
        let mut info = jpeg_info(&bytes).filter(|(_, _, c)| *c == 1 || *c == 3);
        if info.is_none() {
            // UZ: PNG/WebP/CMYK -> JPEG (ffmpeg). EN: convert PNG/WebP/CMYK to JPEG.
            let Some(j) = to_jpeg(&bytes, dir, i).await else { continue };
            bytes = j;
            info = jpeg_info(&bytes).filter(|(_, _, c)| *c == 1 || *c == 3);
        }
        let Some((w, h, c)) = info else { continue };
        if w.min(h) < 150 {
            continue; // ikonka/banner
        }
        if cur + bytes.len() as u64 > PDF_LIMIT && !parts.last().unwrap().is_empty() {
            parts.push(vec![]);
            cur = 0;
        }
        cur += bytes.len() as u64 + 400;
        parts.last_mut().unwrap().push(Img { w, h, gray: c == 1, data: bytes });
    }
    progress(urls.len(), urls.len());
    parts.retain(|p| !p.is_empty());
    if parts.is_empty() {
        return Err(anyhow!("rasmlarni yuklab bo'lmadi (sayt rasmlarni bermadi yoki ular juda kichik)"));
    }
    let n = parts.len();
    let mut files = Vec::new();
    for (k, p) in parts.iter().enumerate() {
        let f = dir.join(if n > 1 { format!("{name}_{}.pdf", k + 1) } else { format!("{name}.pdf") });
        tokio::fs::write(&f, make_pdf(p)).await?;
        files.push(f);
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rasm_havolalari() {
        let html = r#"<html><head><title>Bob &amp; 1</title></head><body>
          <img src="/logo.png"><img class="a" data-src='/p/1.jpg' src="blank.gif">
          <img srcset="/p/2-400.jpg 400w, /p/2-1200.jpg 1200w"><img src="https://cdn.x.com/3.webp?x=1&amp;y=2">
          <IMG SRC=/p/1.jpg><img src="data:image/png;base64,AAAA"><img src="/ikon.svg"></body></html>"#;
        let v = extract_images(html, "https://site.com/ch/1");
        assert_eq!(v, vec!["https://site.com/p/1.jpg", "https://site.com/p/2-1200.jpg", "https://cdn.x.com/3.webp?x=1&y=2"]);
        assert_eq!(title_of(html), "Bob & 1");
    }

    fn fake_jpeg(w: u16, h: u16, comps: u8) -> Vec<u8> {
        let mut b = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x4A, 0x46];
        b.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x0B, 0x08]);
        b.extend_from_slice(&h.to_be_bytes());
        b.extend_from_slice(&w.to_be_bytes());
        b.push(comps);
        b.extend_from_slice(&[0x01, 0x11, 0x00, 0xFF, 0xD9]);
        b
    }

    #[test]
    fn jpeg_olchami() {
        assert_eq!(jpeg_info(&fake_jpeg(800, 1200, 3)), Some((800, 1200, 3)));
        assert_eq!(jpeg_info(b"\x89PNG....."), None);
    }

    #[test]
    fn pdf_tuzilishi() {
        let imgs = vec![Img { w: 800, h: 1200, gray: false, data: fake_jpeg(800, 1200, 3) }, Img { w: 600, h: 900, gray: true, data: fake_jpeg(600, 900, 1) }];
        let pdf = make_pdf(&imgs);
        let s: String = pdf.iter().map(|&b| b as char).collect(); // UZ: 1 bayt = 1 belgi, siljishlar mos keladi. EN: 1 byte = 1 char so offsets match.
        assert!(s.starts_with("%PDF-1.4"));
        assert!(s.contains("/Count 2"));
        assert!(s.contains("/Kids [5 0 R 8 0 R]"));
        assert!(s.contains("/MediaBox [0 0 400.00 600.00]"));
        assert!(s.contains("/DeviceGray"));
        // xref: har bir ko'rsatilgan siljish aynan "N 0 obj" ga to'g'ri kelishi kerak.
        let xs = s.rfind("startxref\n").unwrap();
        let xref_pos: usize = s[xs + 10..].lines().next().unwrap().parse().unwrap();
        assert!(pdf[xref_pos..].starts_with(b"xref\n0 9\n"));
        let tail = String::from_utf8_lossy(&pdf[xref_pos..]).to_string();
        let lines: Vec<&str> = tail.lines().skip(3).take(8).collect();
        for (i, l) in lines.iter().enumerate() {
            let off: usize = l[..10].parse().unwrap();
            assert!(pdf[off..].starts_with(format!("{} 0 obj", i + 1).as_bytes()), "obj {} siljishi noto'g'ri", i + 1);
        }
    }

    #[tokio::test]
    async fn ffmpeg_bilan_pdf() {
        if std::process::Command::new("ffmpeg").arg("-version").output().is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("ytbot-pdf-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("a.png");
        let ok = std::process::Command::new("ffmpeg").args(["-y", "-v", "error", "-f", "lavfi", "-i", "testsrc=size=400x600:rate=1", "-frames:v", "1"]).arg(&png).status().unwrap().success();
        assert!(ok);
        let bytes = std::fs::read(&png).unwrap();
        let j = to_jpeg(&bytes, &dir, 0).await.expect("png -> jpeg");
        assert_eq!(jpeg_info(&j).map(|(w, h, _)| (w, h)), Some((400, 600)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
