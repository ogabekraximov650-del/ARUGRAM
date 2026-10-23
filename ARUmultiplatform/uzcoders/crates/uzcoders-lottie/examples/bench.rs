//! UZ: Tezlik o'lchovi: `bench <fayl> <o'lcham> [kadr_soni]` — barcha kadrlarni chizib o'rtacha vaqtni chiqaradi.
//! EN: Speed benchmark: `bench <file> <size> [frames]` — renders all frames and prints the mean time.
//! RU: Замер скорости: `bench <файл> <размер> [кадров]` — рисует все кадры и печатает среднее время.

use std::time::Instant;
use uzcoders_lottie::{Animation, RenderConfig, Renderer, Rgba8};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let raw = std::fs::read(&a[1]).expect("read");
    let anim = Animation::from_json_str(&uzcoders_lottie::tgs::to_json(&raw).expect("json")).expect("parse");
    let size: f32 = a[2].parse().expect("size");
    let (ip, op) = (anim.in_point, anim.out_point);
    let n: usize = a.get(3).and_then(|v| v.parse().ok()).unwrap_or((op - ip).max(1.0) as usize);
    let cfg = RenderConfig::new(Rgba8::TRANSPARENT, size / anim.width.max(1) as f32);
    let r = Renderer::default();
    let prepared = r.prepare(&anim).expect("prepare");
    let t = Instant::now();
    for i in 0..n {
        let fr = ip + (op - ip) * i as f32 / n as f32;
        prepared.render_frame(fr, cfg).expect("render");
    }
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    println!("{} frames at {}px: total {:.0} ms, {:.2} ms/frame", n, size, ms, ms / n as f64);
}
