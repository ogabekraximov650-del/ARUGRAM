//! UZ: Tanishtiruv teksturalari: Telegram'ning rasmlari (`assets/tg/*.png`) va dasturda chiziladigan doira/niqob.
//! EN: Intro textures: Telegram's images (`assets/tg/*.png`) plus the circle/mask drawn in code.
//! RU: Текстуры интро: изображения Telegram (`assets/tg/*.png`) и круг/маска, рисуемые кодом.

use crate::softgl::Texture;
use uzcoders::prelude::decode_png;

macro_rules! png {
    ($n:literal) => {
        include_bytes!(concat!("../assets/tg/", $n, ".png")) as &[u8]
    };
}

const FILES: [&[u8]; 22] = [
    png!("fast_arrow_shadow"),
    png!("fast_arrow"),
    png!("fast_body"),
    png!("fast_spiral"),
    png!("ic_bubble_dot"),
    png!("ic_bubble"),
    png!("ic_cam_lens"),
    png!("ic_cam"),
    png!("ic_pencil"),
    png!("ic_pin"),
    png!("ic_smile_eye"),
    png!("ic_smile"),
    png!("ic_videocam"),
    png!("knot_down"),
    png!("knot_up"),
    png!("powerful_infinity_white"),
    png!("powerful_infinity"),
    png!("powerful_mask"),
    png!("powerful_star"),
    png!("private_door"),
    png!("private_screw"),
    png!("tg_plane"),
];

fn cover(d: f32) -> f32 {
    (0.5 - d).clamp(0.0, 1.0)
}

/// 24 ta tekstura: indekslar `intro_scene::T_*` bilan mos / 24 textures, indices match `intro_scene::T_*`.
pub fn load(bg: [f32; 3], scale_factor: f32) -> Vec<Texture> {
    let mut v = Vec::with_capacity(24);
    for (i, f) in FILES.iter().enumerate() {
        let img = decode_png(f).expect("intro png");
        let tint = if i == 17 { Some(bg) } else { None };
        v.push(Texture::from_rgba8(img.w as usize, img.h as usize, &img.rgba, tint));
    }
    // 22: Telegram logotipi rangidagi doira (diametri 150 birlik) / a circle in the logo color (diameter 150 units).
    let n = (150.0 * scale_factor).round().max(8.0) as usize;
    let mut px = Vec::with_capacity(n * n);
    let c = [0x22 as f32 / 255.0, 0x9A as f32 / 255.0, 0xF0 as f32 / 255.0];
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = (x as f32 + 0.5 - n as f32 / 2.0, y as f32 + 0.5 - n as f32 / 2.0);
            let a = cover((dx * dx + dy * dy).sqrt() - n as f32 / 2.0);
            px.push([c[0] * a, c[1] * a, c[2] * a, a]);
        }
    }
    v.push(Texture { w: n, h: n, px });
    // 23: fon rangi, o'rtasida teshik doira (200x150 birlik) / background color with a round hole (200x150 units).
    let (w, h) = ((200.0 * scale_factor).round() as usize, (150.0 * scale_factor).round() as usize);
    let mut px = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = (x as f32 + 0.5 - w as f32 / 2.0, y as f32 + 0.5 - h as f32 / 2.0);
            let a = 1.0 - cover((dx * dx + dy * dy).sqrt() - h as f32 / 2.0);
            px.push([bg[0] * a, bg[1] * a, bg[2] * a, a]);
        }
    }
    v.push(Texture { w, h, px });
    v
}
