//! UZ: Lottie/.tgs faylning bitta kadrini PNG ga chizadi: `render_png <kirish.json|.tgs> <chiqish.png> [kadr] [o'lcham]`.
//! EN: Renders one frame of a Lottie/.tgs file to PNG: `render_png <in.json|.tgs> <out.png> [frame] [size]`.
//! RU: Рисует один кадр Lottie/.tgs в PNG: `render_png <вход.json|.tgs> <выход.png> [кадр] [размер]`.

use uzcoders_lottie::{Animation, RenderConfig, Renderer};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 {
        eprintln!("usage: render_png <in.json|.tgs> <out.png> [frame] [size]");
        std::process::exit(2);
    }
    let raw = std::fs::read(&a[1]).expect("read");
    let json = uzcoders_lottie::tgs::to_json(&raw).expect("tgs/json");
    let anim = Animation::from_json_str(&json).expect("parse");
    let frame: f32 = a.get(3).and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let size: f32 = a.get(4).and_then(|v| v.parse().ok()).unwrap_or(anim.width as f32);
    let cfg = RenderConfig::new(uzcoders_lottie::Rgba8::TRANSPARENT, size / anim.width.max(1) as f32);
    let f = Renderer::default().render_frame(&anim, frame, cfg).expect("render");
    // UZ: Premultiplied -> oddiy RGBA. EN: Premultiplied -> straight RGBA. RU: Предумноженный -> обычный RGBA.
    let mut px = f.pixels.clone();
    for p in px.chunks_exact_mut(4) {
        let al = u32::from(p[3]);
        if al > 0 && al < 255 {
            for c in &mut p[..3] {
                *c = ((u32::from(*c) * 255 + al / 2) / al).min(255) as u8;
            }
        }
    }
    std::fs::write(&a[2], uzcoders_render::png::encode_rgba(f.width, f.height, &px)).expect("write");
}
