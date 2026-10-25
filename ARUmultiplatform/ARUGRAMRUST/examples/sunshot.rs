use arugramrust::lottie_sheet;
fn main() {
    let raw = include_str!("../assets/lottie/sun.json");
    let px = 58u32;
    let s = lottie_sheet::build(raw, px, 60.0, &[("*", [0.141, 0.604, 0.945])]).unwrap();
    println!("{}x{} cols {} count {} fps {}", s.w, s.h, s.cols, s.count, s.fps);
    // kadrlar: 0, o'rta, oxirgi / frames 0, mid, last
    let mut out = image_rgba(px * 5, px);
    for (k, f) in [s.count - 6, s.count - 4, s.count - 3, s.count - 2, s.count - 1].iter().enumerate() {
        let (cx, cy) = (f % s.cols, f / s.cols);
        for y in 0..px {
            for x in 0..px {
                let src = (((cy * px + y) * s.w + cx * px + x) * 4) as usize;
                let dst = ((y * px * 5 + k as u32 * px + x) * 4) as usize;
                let a = s.rgba[src + 3] as f32 / 255.0;
                for c in 0..3 {
                    out[dst + c] = (s.rgba[src + c] as f32 * a + 18.0 * (1.0 - a)) as u8;
                }
                out[dst + 3] = 255;
            }
        }
    }
    let img = uzcoders::prelude::Image { w: px * 5, h: px, rgba: out };
    std::fs::write("/tmp/claude-0/cmp/sun.png", img.to_png()).unwrap();
}
fn image_rgba(w: u32, h: u32) -> Vec<u8> {
    vec![0; (w * h * 4) as usize]
}
