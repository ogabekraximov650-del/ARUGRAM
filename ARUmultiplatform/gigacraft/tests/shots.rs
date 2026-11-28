//! Ko'rinmas (pbuffer) OpenGL ES kontekstida haqiqiy kadrlarni chizadi va rasmga oladi (CI'da Mesa llvmpipe).
use gigacraft::{app::App, egl::Egl, png::encode_rgba, sky, ui::Mode};
use std::time::{Duration, Instant};

const W: i32 = 1280;
const H: i32 = 576;

fn save(name: &str, px: &[u8]) {
    if let Ok(d) = std::env::var("GIGA_SNAP_DIR") {
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(format!("{d}/{name}.png"), encode_rgba(W as u32, H as u32, px)).unwrap();
    }
}

fn px(img: &[u8], x: i32, y: i32) -> [f32; 3] {
    let i = ((y * W + x) * 4) as usize;
    [img[i] as f32, img[i + 1] as f32, img[i + 2] as f32]
}

fn luma(c: [f32; 3]) -> f32 {
    0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn wait_loaded(app: &mut App, secs: u64) {
    let t0 = Instant::now();
    let mut quiet = 0;
    while t0.elapsed() < Duration::from_secs(secs) {
        app.step(W, H, 0.016);
        std::thread::sleep(Duration::from_millis(3));
        if app.pending() == 0 && app.game.loading >= 1.0 {
            quiet += 1;
            if quiet > 5 {
                return;
            }
        } else {
            quiet = 0;
        }
    }
}

fn teleport(app: &mut App, lat: f64, lon: f64, alt: f64) {
    let d = gigacraft::earth::latlon_dir(lat, lon);
    app.game.player.pos = gigacraft::math::d_mul(d, gigacraft::planet::SEA_R as f64 + alt);
}

fn set_alt(app: &mut App, alt: f64) {
    let up = gigacraft::math::d_norm(app.game.player.pos);
    app.game.player.pos = gigacraft::math::d_mul(up, gigacraft::planet::SEA_R as f64 + alt);
}

#[test]
fn ekran_rasmlari() {
    let mut egl = match Egl::new(true) {
        Ok(e) => e,
        Err(e) => {
            if std::env::var("GIGA_REQUIRE_GL").is_ok() {
                panic!("EGL ishga tushmadi: {e}");
            }
            eprintln!("EGL yo'q ({e}) — rasm testi o'tkazib yuborildi");
            return;
        }
    };
    egl.make_pbuffer(W, H).expect("pbuffer");
    let mut app = App::new("").expect("ilova");
    app.game.quality = 0;
    let t0 = Instant::now();
    wait_loaded(&mut app, 300);
    eprintln!("yuklash: {:?}, meshlar: {}", t0.elapsed(), app.loaded_meshes());
    assert!(app.loaded_meshes() > 100, "mesh kam: {}", app.loaded_meshes());

    // 1. Kun
    app.game.time = sky::PHASE_DAY;
    app.game.player.pitch = 0.5;
    app.step(W, H, 0.016);
    app.draw(W, H);
    let day = app.read_pixels(W, H);
    save("1-kun", &day);
    eprintln!("kun: chizildi {} / kesildi {} / uchburchak {}", app.stats.drawn, app.stats.culled, app.stats.tris);
    assert!(app.stats.drawn > 20 && app.stats.tris > 10_000);

    // 2. Kech va 3. Tun
    app.game.time = sky::PHASE_SUNSET + 0.02;
    app.draw(W, H);
    save("2-kech", &app.read_pixels(W, H));
    app.game.time = sky::PHASE_NIGHT;
    app.draw(W, H);
    let night = app.read_pixels(W, H);
    save("3-tun", &night);

    // Osmon (yuqori markaz) kunduzi tundan ancha yorqin
    let top_day = px(&day, W / 2, 8);
    let top_night = px(&night, W / 2, 8);
    assert!(luma(top_day) > luma(top_night) * 2.0, "kun/tun farqi yo'q: {:?} {:?}", top_day, top_night);
    let mut distinct = std::collections::HashSet::new();
    for y in (0..H).step_by(9) {
        for x in (0..W).step_by(9) {
            let c = px(&day, x, y);
            distinct.insert(((c[0] / 8.0) as i32, (c[1] / 8.0) as i32, (c[2] / 8.0) as i32));
        }
    }
    assert!(distinct.len() > 40, "rasm bir xil: {} rang", distinct.len());
    // HUD: "Qo'yish" tugmasi to'q sariq
    let s = H as f32 / 1080.0;
    let (bx, by) = (W as f32 - 164.0 * s - 100.0 * s, H as f32 - 204.0 * s);
    let b = px(&day, bx as i32, by as i32);
    assert!(b[0] > 170.0 && b[2] < 80.0, "HUD tugmasi yo'q: {:?}", b);

    // 4. Baland uchish (uzoq relyef)
    app.game.time = sky::PHASE_DAY;
    app.game.player.flying = true;
    let ground = app.game.player.altitude();
    set_alt(&mut app, ground + 450.0);
    app.game.player.pitch = -0.42;
    wait_loaded(&mut app, 120);
    app.draw(W, H);
    save("4-balandlik", &app.read_pixels(W, H));

    // 5. Pauza menyusi va 6. blok oynasi
    app.game.mode = Mode::Pause;
    app.draw(W, H);
    save("5-pauza", &app.read_pixels(W, H));
    app.game.mode = Mode::Inv;
    app.draw(W, H);
    save("6-bloklar", &app.read_pixels(W, H));
    app.game.mode = Mode::Play;

    // 7. Yuqori sifat
    app.game.quality = 2;
    wait_loaded(&mut app, 300);
    app.draw(W, H);
    save("7-yuqori-sifat", &app.read_pixels(W, H));
    eprintln!("yuqori sifat: chizildi {} / uchburchak {}", app.stats.drawn, app.stats.tris);

    // 7b. Soyalar: yerga yaqin, quyosh past
    app.game.quality = 1;
    app.game.player.flying = false;
    set_alt(&mut app, ground + 1.0);
    app.game.player.pitch = -0.35;
    app.game.time = 0.08;
    wait_loaded(&mut app, 200);
    app.step(W, H, 0.016);
    app.draw(W, H);
    save("10-soya", &app.read_pixels(W, H));

    // 7c. Ko'l (Gyuron) va daryo (Nil) ustidan
    app.game.player.flying = true;
    teleport(&mut app, 45.0, 280.0, 700.0);
    app.game.player.pitch = -0.5;
    wait_loaded(&mut app, 200);
    app.draw(W, H);
    save("11-kol", &app.read_pixels(W, H));
    teleport(&mut app, 26.0, 32.0, 500.0);
    wait_loaded(&mut app, 200);
    app.draw(W, H);
    save("12-nil", &app.read_pixels(W, H));

    // 7d. Qishloq uyi (g'isht, oyna, tom, bug'doy), jonzotlar, yomg'ir va omon qolish inventari
    {
        use gigacraft::gen::{Gen, EDGE_MARGIN, HOUSE_CELL};
        use gigacraft::{items, math::*, planet::*};
        let g = Gen::new(1);
        let mut found = None;
        'a: for f in 0..6u8 {
            for cz in (EDGE_MARGIN / HOUSE_CELL + 2)..40 {
                for cx in (EDGE_MARGIN / HOUSE_CELL + 2)..40 {
                    if let Some(h) = g.house_in_cell(f, cx * 5, cz * 5) {
                        found = Some(h);
                        break 'a;
                    }
                }
            }
        }
        let h = found.expect("uy topilmadi");
        let c = dir(h.f, (h.x + 8) as f64 + 0.5, (h.z + 8) as f64 + 0.5);
        // uydan 45 blok narida, ustida 14 blok balandlikda, uyga qarab
        let side = d_norm(d_cross(c, [0.0, 0.0, 1.0]));
        let p = d_add(d_mul(c, (h.base + 15) as f64), d_mul(side, 45.0));
        app.game.player.pos = p;
        app.game.player.flying = true;
        let to = d_norm(d_sub(d_mul(c, (h.base + 3) as f64), p));
        app.game.player.heading = d_norm(d_sub(to, d_mul(d_norm(p), d_dot(to, d_norm(p)))));
        app.game.player.pitch = -0.3;
        app.game.time = sky::PHASE_DAY;
        app.game.survival = false;
        wait_loaded(&mut app, 200);
        app.game.player.flying = false;
        let fwd = app.game.player.camera_basis_flat().2;
        for k in 0..3 {
            let kind = [gigacraft::entity::COW, gigacraft::entity::SHEEP, gigacraft::entity::ZOMBIE][k];
            let at = d_add(app.game.player.pos, d_mul(fwd, 22.0 + k as f64 * 5.0));
            let at = d_mul(d_norm(at), d_len(app.game.player.pos) - 12.0);
            app.game.mobs.push(gigacraft::entity::Mob::new(kind, at, fwd));
        }
        app.game.rain = 0.8;
        app.draw(W, H);
        save("13-qishloq", &app.read_pixels(W, H));
        app.game.survival = true;
        app.game.inv.add(items::tool(0, 1) as u16, 1);
        app.game.inv.add(gigacraft::blocks::LOG as u16, 12);
        app.game.inv.add(items::COAL, 5);
        app.game.inv.add(items::BREAD, 3);
        app.game.inv.add(gigacraft::blocks::IRON_ORE as u16, 4);
        app.game.open_inv();
        app.game.grid[0] = items::Stack::new(gigacraft::blocks::LOG as u16, 1);
        app.draw(W, H);
        let inv = app.read_pixels(W, H);
        save("14-inventar", &inv);
        app.game.close_craft();
        app.game.survival = false;
        app.game.rain = 0.0;
        app.game.mobs.clear();
    }

    // 8. Yuqori atmosfera (20 km) va 9. kosmosdan sayyora (10 000 km)
    app.game.quality = 0;
    app.game.player.fly_level = 7;
    set_alt(&mut app, 80_000.0);
    app.game.player.pitch = -0.7;
    wait_loaded(&mut app, 200);
    app.draw(W, H);
    save("8-stratosfera", &app.read_pixels(W, H));
    set_alt(&mut app, 40_000_000.0);
    app.game.player.pitch = -1.55;
    wait_loaded(&mut app, 200);
    app.draw(W, H);
    let space = app.read_pixels(W, H);
    save("9-kosmos", &space);
    // kosmosda yuqori burchak qora, markazda sayyora (qora emas)
    let corner = px(&space, 4, 4);
    let mid = px(&space, W / 2, H / 2);
    assert!(luma(corner) < 40.0, "kosmos qora bo'lishi kerak: {:?}", corner);
    assert!(luma(mid) > 25.0, "sayyora ko'rinmayapti: {:?}", mid);
}
