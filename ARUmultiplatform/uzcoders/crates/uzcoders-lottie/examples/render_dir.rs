//! UZ: Papkadagi barcha `.tgs`/`.json` fayllarni sinaydi: `render_dir <kirish_papka> <chiqish_papka> <o'lcham>`.
//! Har fayl uchun 3 ta kadr (0%, 33%, 66%) PNG ga chiziladi; natija qatorlari: `nom ok|FAIL <sabab> <ms>`.
//! EN: Tests every `.tgs`/`.json` in a folder: `render_dir <in_dir> <out_dir> <size>`.
//! Three frames (0%, 33%, 66%) are rendered to PNG per file; result lines: `name ok|FAIL <reason> <ms>`.
//! RU: Проверяет все `.tgs`/`.json` в папке: `render_dir <входная_папка> <выходная_папка> <размер>`.
//! Для каждого файла рисуются 3 кадра (0%, 33%, 66%) в PNG; строки результата: `имя ok|FAIL <причина> <мс>`.

use std::time::Instant;
use uzcoders_lottie::{Animation, RenderConfig, Renderer, Rgba8};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 4 {
        eprintln!("usage: render_dir <in_dir> <out_dir> <size>");
        std::process::exit(2);
    }
    let size: f32 = a[3].parse().expect("size");
    std::fs::create_dir_all(&a[2]).expect("mkdir");
    let mut names: Vec<_> = std::fs::read_dir(&a[1]).expect("read_dir").flatten().map(|e| e.path()).collect();
    names.sort();
    let (mut ok, mut bad) = (0, 0);
    for p in names {
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "tgs" && ext != "json" {
            continue;
        }
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("x").to_string();
        let t = Instant::now();
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), String> {
            let raw = std::fs::read(&p).map_err(|e| e.to_string())?;
            let json = uzcoders_lottie::tgs::to_json(&raw).ok_or("tgs/json ochilmadi")?;
            let anim = Animation::from_json_str(&json).map_err(|e| format!("parse: {e}"))?;
            let r = Renderer::default();
            let prepared = r.prepare(&anim).map_err(|e| format!("{e}"))?;
            let cfg = RenderConfig::new(Rgba8::TRANSPARENT, size / anim.width.max(1) as f32);
            let total = (anim.out_point - anim.in_point).max(1.0);
            for (k, frac) in [0.0f32, 0.33, 0.66].iter().enumerate() {
                let f = prepared.render_frame(anim.in_point + total * frac, cfg).map_err(|e| format!("{e}"))?;
                let mut px = f.pixels.clone();
                for q in px.chunks_exact_mut(4) {
                    let al = u32::from(q[3]);
                    if al > 0 && al < 255 {
                        for c in &mut q[..3] {
                            *c = ((u32::from(*c) * 255 + al / 2) / al).min(255) as u8;
                        }
                    }
                }
                std::fs::write(format!("{}/{stem}_{k}.png", a[2]), uzcoders_render::png::encode_rgba(f.width, f.height, &px)).map_err(|e| e.to_string())?;
            }
            Ok(())
        }))
        .unwrap_or_else(|_| Err("PANIC".to_string()));
        let ms = t.elapsed().as_millis();
        match res {
            Ok(()) => {
                ok += 1;
                println!("{stem} ok {ms}");
            }
            Err(e) => {
                bad += 1;
                println!("{stem} FAIL {} {ms}", e.replace('\n', " ").chars().take(200).collect::<String>());
            }
        }
    }
    println!("SUMMARY ok={ok} fail={bad}");
}
