use arugramrust::{intro_assets, intro_scene::Scene};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (refdir, outdir) = (a[1].clone(), a[2].clone());
    let (w, h, sf) = (600usize, 450usize, 3.0f32);
    let bg = [0x18 as f32 / 255.0, 0x18 as f32 / 255.0, 0x19 as f32 / 255.0];
    let mut sc = Scene::new(intro_assets::load(bg, sf), bg, w, h, sf);
    let ts = [3500.0f32, 7000.0, 10500.0, 14000.0, 17500.0];
    let frames = 22000 * 60 / 1000;
    let show = [0usize, 60, 120, 210, 215, 225, 240, 420, 450, 480, 600, 640, 700, 800, 1000, 1050, 1100, 1200];
    let mut total = 0.0f64;
    let mut n = 0;
    let t0 = std::time::Instant::now();
    for i in 0..frames {
        let ms = i as f32 * 1000.0 / 60.0;
        let (mut page, mut off) = (0, 0.0f32);
        for (k, t) in ts.iter().enumerate() {
            if ms >= *t {
                let x = ((ms - t) / 450.0).clamp(0.0, 1.0) - 1.0;
                page = k as i32 + 1;
                off = -(1.0 - (x * x * x * x * x + 1.0));
            }
        }
        sc.set_page(page);
        sc.set_scroll_offset(off);
        sc.set_date(1.0 + ms / 1000.0);
        sc.draw_frame(16);
        if show.contains(&i) {
            let rgba = sc.to_rgba8();
            let refimg = std::fs::read(format!("{refdir}/f{:05}.png", i + 1)).ok().and_then(|b| uzcoders::prelude::decode_png(&b));
            let mut diff = 0.0f64;
            if let Some(r) = &refimg {
                for k in 0..w * h {
                    for c in 0..3 {
                        diff += (rgba[k * 4 + c] as f64 - r.rgba[k * 4 + c] as f64).abs();
                    }
                }
                diff /= (w * h * 3) as f64;
                total += diff;
                n += 1;
            }
            // yonma-yon rasm / side by side
            let mut out = vec![0u8; w * 2 * h * 4];
            for y in 0..h {
                for x in 0..w {
                    for c in 0..4 {
                        out[(y * w * 2 + x) * 4 + c] = rgba[(y * w + x) * 4 + c];
                        out[(y * w * 2 + w + x) * 4 + c] = refimg.as_ref().map_or(0, |r| r.rgba[(y * w + x) * 4 + c]);
                    }
                }
            }
            let img = uzcoders::prelude::Image { w: (w * 2) as u32, h: h as u32, rgba: out };
            std::fs::write(format!("{outdir}/c{i:05}.png"), img.to_png()).unwrap();
            println!("kadr {i}: o'rtacha farq {diff:.2}");
        }
    }
    println!("jami {:.1} s, o'rtacha {:.2}", t0.elapsed().as_secs_f32(), total / n.max(1) as f64);
}
