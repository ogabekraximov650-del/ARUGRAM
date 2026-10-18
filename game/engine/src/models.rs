//! Kublar, sharlar, silindrlar va konuslardan yig'ilgan modellar: anime qiz va birinchi shaxs qurollari.

use crate::scene::{Instance, Lists, Mat, Mesh};
use crate::weapons::WeaponKind;
use glam::{Quat, Vec3};
use std::f32::consts::{FRAC_PI_2, PI};

fn v3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

/// Mahalliy koordinatalar (x o'ng, y tepa, -z oldinga) dan dunyoga o'tkazuvchi yordamchi.
struct Rig<'a> {
    out: &'a mut Lists,
    pos: Vec3,
    q: Quat,
    /// Bosh bog'langan nuqta va uning burilishi (bosh qismlari uchun).
    pivot: Vec3,
    head_q: Quat,
}

impl<'a> Rig<'a> {
    fn part(&mut self, mesh: Mesh, c: Vec3, size: Vec3, rgb: [f32; 3], mat: Mat, rot: Quat) {
        self.out.push(mesh, Instance::new(self.pos + self.q * c, size, rgb, mat, self.q * rot));
    }

    /// Bosh bilan birga buriladigan qism.
    fn hp(&mut self, mesh: Mesh, c: Vec3, size: Vec3, rgb: [f32; 3], mat: Mat, rot: Quat) {
        let c2 = self.pivot + self.head_q * (c - self.pivot);
        self.part(mesh, c2, size, rgb, mat, self.head_q * rot);
    }

    /// `a` dan `b` gacha silindr (qo'l-oyoq bo'g'imi).
    fn limb(&mut self, a: Vec3, b: Vec3, dia: f32, rgb: [f32; 3], mat: Mat) {
        let d = b - a;
        let len = d.length().max(1e-4);
        let rot = Quat::from_rotation_arc(Vec3::Y, d / len);
        self.part(Mesh::Cylinder, (a + b) * 0.5, v3(dia, len, dia), rgb, mat, rot);
    }
}

#[derive(Clone, Copy)]
pub struct GirlState {
    pub pos: Vec3,
    pub yaw: f32,
    pub phase: f32,
    /// 0 — turibdi, 1 — yuryapti.
    pub moving: f32,
    pub time: f32,
    /// Boshning tanaga nisbatan burilishi (o'yinchiga qarash), radian.
    pub look_yaw: f32,
}

fn dir(a: f32) -> Vec3 {
    // +a oldinga (-z) siltaydi
    v3(0.0, -a.cos(), -a.sin())
}

/// Anime uslubidagi qiz (~1.62 m): kichik bosh-tana nisbati 1:6.3, ingichka bo'yin va bel, uzun oyoq,
/// tekis peshona sochi, uzun soch, tugun va oltin soch bezagi, dengizchi forma.
pub fn girl(out: &mut Lists, s: &GirlState) {
    let skin = [1.0, 0.86, 0.78];
    let hair = [0.10, 0.40, 0.38];
    let hair_hi = [0.30, 0.68, 0.62];
    let hair_lo = [0.06, 0.28, 0.30];
    let white = [0.97, 0.97, 1.0];
    let navy = [0.14, 0.18, 0.42];
    let red = [0.88, 0.14, 0.26];
    let gold = [1.0, 0.78, 0.25];
    let sock = [0.12, 0.13, 0.22];
    let shoe = [0.32, 0.18, 0.14];
    let m = s.moving;
    let t = s.time;
    let breathe = (t * 1.8).sin() * 0.003;
    let mut r = Rig {
        out,
        pos: s.pos,
        q: Quat::from_rotation_y(s.yaw),
        pivot: v3(0.0, 1.34, 0.0),
        head_q: Quat::from_rotation_y(s.look_yaw.clamp(-1.0, 1.0)) * Quat::from_rotation_x(0.03),
    };

    // oyoqlar: ingichka, uzun (son), paypoq tizza ostida
    for sign in [-1.0f32, 1.0] {
        let ph = s.phase + if sign > 0.0 { 0.0 } else { PI };
        let a = ph.sin() * 0.55 * m;
        let bend = 0.10 + m * 0.60 * ph.cos().max(0.0);
        let hip = v3(sign * 0.075, 0.90, 0.0);
        let knee = hip + dir(a) * 0.43;
        let ankle = knee + dir(a - bend) * 0.42;
        r.limb(hip, knee, 0.088, skin, Mat::Toon);
        r.part(Mesh::Sphere, knee, Vec3::splat(0.078), skin, Mat::Toon, Quat::IDENTITY);
        r.limb(knee, ankle, 0.066, sock, Mat::Toon);
        let foot = ankle + v3(0.0, -0.035, -0.04);
        r.part(Mesh::Sphere, foot, v3(0.07, 0.06, 0.18), shoe, Mat::Toon, Quat::from_rotation_x((a - bend) * 0.25));
    }
    // yubka (kesik konus) va oq hoshiya
    r.part(Mesh::Frustum, v3(0.0, 0.79, 0.0), v3(0.46, 0.32, 0.34), navy, Mat::Toon, Quat::IDENTITY);
    r.part(Mesh::Cylinder, v3(0.0, 0.635, 0.0), v3(0.462, 0.018, 0.342), [0.92, 0.92, 0.97], Mat::Toon, Quat::IDENTITY);
    // ingichka bel va ko'krak
    r.part(Mesh::Cylinder, v3(0.0, 1.0, 0.0), v3(0.185, 0.12, 0.135), white, Mat::Toon, Quat::IDENTITY);
    r.part(Mesh::Sphere, v3(0.0, 1.145 + breathe, 0.0), v3(0.235, 0.26, 0.17), white, Mat::Toon, Quat::IDENTITY);
    r.part(Mesh::Sphere, v3(0.0, 1.235 + breathe, 0.0), v3(0.25, 0.14, 0.155), white, Mat::Toon, Quat::IDENTITY);
    // yoqa (dengizchi), lenta
    r.part(Mesh::Cube, v3(0.0, 1.285, 0.03), v3(0.255, 0.018, 0.19), navy, Mat::Toon, Quat::from_rotation_x(-0.15));
    for sg in [-1.0f32, 1.0] {
        r.part(Mesh::Cube, v3(sg * 0.036, 1.235, -0.083), v3(0.010, 0.13, 0.025), navy, Mat::Toon, Quat::from_rotation_z(sg * 0.5));
    }
    r.part(Mesh::Sphere, v3(0.0, 1.195, -0.088), v3(0.055, 0.042, 0.03), red, Mat::Toon, Quat::IDENTITY);
    for sg in [-1.0f32, 1.0] {
        r.part(Mesh::Cube, v3(sg * 0.024, 1.15, -0.088), v3(0.028, 0.085, 0.010), red, Mat::Toon, Quat::from_rotation_z(sg * 0.15));
    }
    // ingichka bo'yin
    r.part(Mesh::Cylinder, v3(0.0, 1.335, 0.0), v3(0.052, 0.085, 0.052), skin, Mat::Toon, Quat::IDENTITY);
    // qo'llar: ingichka, kichik kaft
    for sign in [-1.0f32, 1.0] {
        let ph = s.phase + if sign > 0.0 { PI } else { 0.0 };
        let a = ph.sin() * 0.45 * m + 0.06;
        let sh = v3(sign * 0.125, 1.255, 0.0);
        let el = sh + dir(a) * 0.255;
        let wr = el + dir(a + 0.20 + 0.35 * m) * 0.235;
        r.part(Mesh::Sphere, sh, Vec3::splat(0.07), white, Mat::Toon, Quat::IDENTITY);
        r.limb(sh, el, 0.058, white, Mat::Toon);
        r.part(Mesh::Sphere, el, Vec3::splat(0.052), skin, Mat::Toon, Quat::IDENTITY);
        r.limb(el, wr, 0.045, skin, Mat::Toon);
        r.part(Mesh::Sphere, wr + dir(a + 0.2) * 0.025, v3(0.045, 0.06, 0.035), skin, Mat::Toon, Quat::IDENTITY);
    }
    // orqadagi uzun soch (belgacha), yurganda tebranadi
    let sway = (s.phase * 1.0).sin() * 0.10 * m + (t * 1.3).sin() * 0.02;
    let back_pivot = v3(0.0, 1.58, 0.10);
    let hq = Quat::from_rotation_x(sway);
    for (x, wdt, len, dep) in [(0.0f32, 0.21, 0.66, 0.10), (-0.075, 0.11, 0.60, 0.08), (0.075, 0.11, 0.60, 0.08)] {
        let c = back_pivot + hq * v3(x, -len * 0.5 - 0.02, 0.0);
        r.part(Mesh::Frustum, c, v3(wdt, len, dep), hair, Mat::ToonHair, hq * Quat::from_rotation_x(PI));
    }
    // bosh
    let hy = 1.49;
    let blink = {
        let c = (t * 0.27).fract();
        if c < 0.035 { 1.0 - ((c / 0.035) - 0.5).abs() * 2.0 } else { 0.0 }
    };
    let open = (1.0 - 0.92 * blink).max(0.08);
    r.hp(Mesh::Sphere, v3(0.0, hy, 0.0), v3(0.215, 0.235, 0.205), skin, Mat::Face, Quat::IDENTITY);
    if let Some(last) = r.out.0[Mesh::Sphere as usize].last_mut() {
        last.color[3] = open;
    }
    // uchli iyak
    r.hp(Mesh::Sphere, v3(0.0, hy - 0.075, -0.012), v3(0.165, 0.12, 0.17), skin, Mat::Toon, Quat::IDENTITY);
    r.hp(Mesh::Sphere, v3(0.0, hy - 0.122, -0.026), v3(0.085, 0.075, 0.095), skin, Mat::Toon, Quat::IDENTITY);
    // soch qalpog'i
    r.hp(Mesh::Sphere, v3(0.0, hy + 0.028, 0.042), v3(0.245, 0.27, 0.250), hair, Mat::ToonHair, Quat::IDENTITY);
    // tekis kesilgan peshona sochi (hime): beshta qalin tola
    for k in -2i32..=2 {
        let x = k as f32 * 0.043;
        let z = -(0.092 - x * x * 3.2);
        r.hp(Mesh::Cube, v3(x, hy + 0.052, z), v3(0.046, 0.095, 0.028), hair, Mat::ToonHair, Quat::from_rotation_x(-0.10));
    }
    r.hp(Mesh::Sphere, v3(0.0, hy + 0.092, -0.07), v3(0.205, 0.07, 0.10), hair_hi, Mat::ToonHair, Quat::from_rotation_x(-0.2));
    // yon sochlar: yuz yonida ko'krakkacha
    for sg in [-1.0f32, 1.0] {
        r.hp(Mesh::Frustum, v3(sg * 0.106, hy - 0.10, -0.03), v3(0.040, 0.30, 0.052), hair, Mat::ToonHair, Quat::from_rotation_x(PI));
    }
    // yon tugun va oltin soch bezagi
    r.hp(Mesh::Sphere, v3(0.075, hy + 0.135, 0.05), Vec3::splat(0.09), hair, Mat::ToonHair, Quat::IDENTITY);
    r.hp(Mesh::Cylinder, v3(0.075, hy + 0.135, 0.05), v3(0.096, 0.02, 0.096), gold, Mat::Toon, Quat::from_rotation_z(FRAC_PI_2));
    r.hp(Mesh::Sphere, v3(0.122, hy + 0.135, 0.05), Vec3::splat(0.036), gold, Mat::Toon, Quat::IDENTITY);
    r.hp(Mesh::Sphere, v3(-0.10, hy + 0.06, -0.04), v3(0.032, 0.032, 0.02), gold, Mat::Toon, Quat::IDENTITY);
}

// ---------------- birinchi shaxs qurollari ----------------

#[derive(Clone, Copy)]
pub struct VmState {
    pub kind: WeaponKind,
    /// O'q uzgandan keyingi tepish (0..1).
    pub recoil: f32,
    pub walk_phase: f32,
    pub moving: f32,
    /// Qayta o'qlash jarayoni: 0 — emas; (0..1] — davomida.
    pub reload: f32,
    /// Qurol olish animatsiyasi: 1 — to'liq ko'tarilgan.
    pub raise: f32,
    /// Og'iz chaqnashi (0..1).
    pub flash: f32,
    pub time: f32,
}

struct Gun {
    anchor: Vec3,
    q: Quat,
}

impl Gun {
    fn p(&self, l: Vec3) -> Vec3 {
        self.anchor + self.q * l
    }
}

fn gp(out: &mut Lists, g: &Gun, mesh: Mesh, c: Vec3, size: Vec3, rgb: [f32; 3], mat: Mat, rot: Quat) {
    out.push(mesh, Instance::new(g.p(c), size, rgb, mat, g.q * rot));
}

/// O'qi Z bo'ylab yotgan silindr (barrel, durbin).
fn barrel(out: &mut Lists, g: &Gun, c: Vec3, dia: f32, len: f32, rgb: [f32; 3]) {
    gp(out, g, Mesh::Cylinder, c, v3(dia, len, dia), rgb, Mat::Metal, Quat::from_rotation_x(FRAC_PI_2));
}

fn cube(out: &mut Lists, g: &Gun, c: Vec3, size: Vec3, rgb: [f32; 3], mat: Mat, rot: Quat) {
    gp(out, g, Mesh::Cube, c, size, rgb, mat, rot);
}

pub fn viewmodel(out: &mut Lists, st: &VmState) {
    let dark = [0.20, 0.21, 0.24];
    let steel = [0.50, 0.52, 0.56];
    let wood = [0.45, 0.28, 0.14];
    let jacket = [0.22, 0.30, 0.20];
    let skin = [0.95, 0.78, 0.68];

    // harakat: yurishda tebranish, tepish, qayta o'qlash, almashtirish
    let bob = Vec3::new((st.walk_phase).sin() * 0.006, -(st.walk_phase * 2.0).cos().abs() * 0.006, 0.0) * st.moving;
    let breathe = Vec3::new(0.0, (st.time * 1.6).sin() * 0.0015, 0.0);
    let rp = (st.reload * PI).sin();
    let sw = 1.0 - st.raise.clamp(0.0, 1.0);
    let anchor = Vec3::new(0.13, -0.19, -0.40)
        + bob
        + breathe
        + Vec3::new(0.0, 0.006 * st.recoil, 0.05 * st.recoil)
        + Vec3::new(-0.03 * rp, -0.20 * rp - 0.32 * sw, 0.0);
    let q = Quat::from_rotation_x(0.10 * st.recoil - 0.55 * rp - 0.9 * sw) * Quat::from_rotation_z(0.25 * rp) * Quat::from_rotation_y(0.02 + 0.30 * rp);
    let g = Gun { anchor, q };

    let (grip, fore, muzzle): (Vec3, Vec3, Vec3);
    match st.kind {
        WeaponKind::Pistol => {
            cube(out, &g, v3(0.0, 0.034, -0.02), v3(0.032, 0.036, 0.17), dark, Mat::Metal, Quat::IDENTITY);
            cube(out, &g, v3(0.0, 0.0, -0.01), v3(0.030, 0.028, 0.15), steel, Mat::Metal, Quat::IDENTITY);
            barrel(out, &g, v3(0.0, 0.034, -0.112), 0.013, 0.03, steel);
            cube(out, &g, v3(0.0, -0.058, 0.06), v3(0.030, 0.095, 0.045), [0.07, 0.07, 0.08], Mat::Cloth, Quat::from_rotation_x(-0.2));
            cube(out, &g, v3(0.0, -0.02, 0.0), v3(0.014, 0.006, 0.05), steel, Mat::Metal, Quat::IDENTITY);
            cube(out, &g, v3(0.0, 0.056, -0.085), v3(0.008, 0.010, 0.010), [0.9, 0.9, 0.9], Mat::Flat, Quat::IDENTITY);
            cube(out, &g, v3(0.0, 0.056, 0.058), v3(0.020, 0.010, 0.010), dark, Mat::Metal, Quat::IDENTITY);
            grip = v3(0.0, -0.075, 0.062);
            fore = v3(-0.026, -0.085, 0.07);
            muzzle = v3(0.0, 0.034, -0.12);
        }
        WeaponKind::Rifle => {
            cube(out, &g, v3(0.0, 0.0, 0.0), v3(0.045, 0.065, 0.24), dark, Mat::Metal, Quat::IDENTITY);
            cube(out, &g, v3(0.0, 0.004, -0.22), v3(0.050, 0.052, 0.22), [0.16, 0.16, 0.18], Mat::Metal, Quat::IDENTITY);
            barrel(out, &g, v3(0.0, 0.012, -0.44), 0.014, 0.22, steel);
            barrel(out, &g, v3(0.0, 0.012, -0.55), 0.024, 0.05, dark);
            cube(out, &g, v3(0.0, -0.004, 0.20), v3(0.040, 0.070, 0.17), wood, Mat::Wood, Quat::IDENTITY);
            cube(out, &g, v3(0.0, -0.01, 0.29), v3(0.045, 0.090, 0.02), dark, Mat::Cloth, Quat::IDENTITY);
            cube(out, &g, v3(0.0, -0.07, 0.06), v3(0.032, 0.085, 0.04), [0.07, 0.07, 0.08], Mat::Cloth, Quat::from_rotation_x(-0.25));
            cube(out, &g, v3(0.0, -0.10, -0.06), v3(0.030, 0.13, 0.05), [0.12, 0.12, 0.14], Mat::Metal, Quat::from_rotation_x(0.15));
            cube(out, &g, v3(0.0, 0.065, -0.02), v3(0.032, 0.036, 0.07), dark, Mat::Metal, Quat::IDENTITY);
            cube(out, &g, v3(0.0, 0.065, -0.058), v3(0.024, 0.024, 0.008), [0.55, 0.15, 0.15], Mat::Glass, Quat::IDENTITY);
            cube(out, &g, v3(0.0, 0.045, -0.33), v3(0.010, 0.045, 0.010), dark, Mat::Metal, Quat::IDENTITY);
            grip = v3(0.0, -0.075, 0.062);
            fore = v3(0.0, -0.035, -0.20);
            muzzle = v3(0.0, 0.012, -0.58);
        }
        WeaponKind::Shotgun => {
            cube(out, &g, v3(0.0, 0.0, 0.02), v3(0.045, 0.06, 0.20), dark, Mat::Metal, Quat::IDENTITY);
            barrel(out, &g, v3(0.0, 0.015, -0.33), 0.026, 0.52, steel);
            barrel(out, &g, v3(0.0, -0.022, -0.30), 0.022, 0.44, dark);
            cube(out, &g, v3(0.0, -0.02, -0.25), v3(0.045, 0.040, 0.14), wood, Mat::Wood, Quat::IDENTITY);
            cube(out, &g, v3(0.0, -0.015, 0.22), v3(0.040, 0.08, 0.20), wood, Mat::Wood, Quat::from_rotation_x(0.08));
            cube(out, &g, v3(0.0, 0.040, -0.58), v3(0.008, 0.014, 0.008), [0.95, 0.8, 0.3], Mat::Metal, Quat::IDENTITY);
            cube(out, &g, v3(0.0, -0.06, 0.08), v3(0.030, 0.07, 0.04), wood, Mat::Wood, Quat::from_rotation_x(-0.3));
            grip = v3(0.0, -0.06, 0.075);
            fore = v3(0.0, -0.03, -0.25);
            muzzle = v3(0.0, 0.015, -0.60);
        }
        WeaponKind::Sniper => {
            let olive = [0.20, 0.25, 0.17];
            cube(out, &g, v3(0.0, 0.0, 0.0), v3(0.040, 0.060, 0.26), dark, Mat::Metal, Quat::IDENTITY);
            barrel(out, &g, v3(0.0, 0.012, -0.46), 0.020, 0.62, steel);
            barrel(out, &g, v3(0.0, 0.012, -0.82), 0.030, 0.10, dark);
            cube(out, &g, v3(0.0, -0.015, 0.24), v3(0.045, 0.090, 0.22), olive, Mat::Cloth, Quat::IDENTITY);
            cube(out, &g, v3(0.0, 0.040, 0.22), v3(0.040, 0.030, 0.12), olive, Mat::Cloth, Quat::IDENTITY);
            barrel(out, &g, v3(0.0, 0.082, -0.03), 0.040, 0.24, dark);
            barrel(out, &g, v3(0.0, 0.082, -0.165), 0.052, 0.05, dark);
            barrel(out, &g, v3(0.0, 0.082, 0.10), 0.048, 0.04, dark);
            gp(out, &g, Mesh::Sphere, v3(0.0, 0.082, -0.195), v3(0.044, 0.044, 0.01), [0.25, 0.45, 0.9], Mat::Glass, Quat::IDENTITY);
            cube(out, &g, v3(0.0, 0.052, -0.08), v3(0.020, 0.025, 0.030), dark, Mat::Metal, Quat::IDENTITY);
            cube(out, &g, v3(0.0, 0.052, 0.04), v3(0.020, 0.025, 0.030), dark, Mat::Metal, Quat::IDENTITY);
            gp(out, &g, Mesh::Sphere, v3(0.042, 0.015, 0.06), Vec3::splat(0.018), steel, Mat::Metal, Quat::IDENTITY);
            cube(out, &g, v3(0.0, -0.05, -0.02), v3(0.028, 0.06, 0.07), dark, Mat::Metal, Quat::IDENTITY);
            for sg in [-1.0f32, 1.0] {
                barrel(out, &g, v3(sg * 0.018, -0.03, -0.40), 0.008, 0.14, steel);
            }
            cube(out, &g, v3(0.0, -0.07, 0.08), v3(0.030, 0.08, 0.04), olive, Mat::Cloth, Quat::from_rotation_x(-0.25));
            grip = v3(0.0, -0.065, 0.085);
            fore = v3(0.0, -0.035, -0.20);
            muzzle = v3(0.0, 0.012, -0.88);
        }
    }

    // qo'llar: yengi (kurtka) va kaft; tirsak ekrandan tashqarida
    let right_wrist = g.p(grip);
    let left_wrist = g.p(fore);
    for (wrist, elbow) in [(right_wrist, Vec3::new(0.36, -0.50, 0.15)), (left_wrist, Vec3::new(-0.20, -0.46, 0.12))] {
        let d = wrist - elbow;
        let len = d.length();
        let rot = Quat::from_rotation_arc(Vec3::Y, d / len);
        out.push(Mesh::Cylinder, Instance::new((wrist + elbow) * 0.5, v3(0.066, len, 0.066), jacket, Mat::Cloth, rot));
        out.push(Mesh::Cylinder, Instance::new(wrist - d / len * 0.01, v3(0.056, 0.05, 0.056), [0.12, 0.12, 0.13], Mat::Cloth, rot));
        out.push(Mesh::Sphere, Instance::new(wrist + d / len * 0.015, v3(0.050, 0.046, 0.058), skin, Mat::Skin, g.q));
    }

    // og'iz chaqnashi
    if st.flash > 0.0 {
        let f = st.flash;
        let c = g.p(muzzle);
        out.push(Mesh::Sphere, Instance::new(c, Vec3::splat(0.035 * f + 0.01), [1.0, 0.9, 0.5], Mat::Emissive, g.q));
        out.push(Mesh::Cone, Instance::new(g.p(muzzle + v3(0.0, 0.0, -0.06)), v3(0.03 * f, 0.09 * f, 0.03 * f), [1.0, 0.7, 0.25], Mat::Emissive, g.q * Quat::from_rotation_x(FRAC_PI_2)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qiz_qismlari_va_modelda_instans_bor() {
        let mut l = Lists::default();
        girl(&mut l, &GirlState { pos: Vec3::ZERO, yaw: 0.0, phase: 1.0, moving: 1.0, time: 2.0, look_yaw: 0.3 });
        assert!(l.total() >= 30 && l.total() < 200, "{}", l.total());
        for kind in [WeaponKind::Pistol, WeaponKind::Rifle, WeaponKind::Shotgun, WeaponKind::Sniper] {
            let mut v = Lists::default();
            viewmodel(&mut v, &VmState { kind, recoil: 0.5, walk_phase: 1.0, moving: 1.0, reload: 0.5, raise: 1.0, flash: 1.0, time: 1.0 });
            assert!(v.total() > 8 && v.total() < crate::renderer::MAX_VIEWMODEL, "{kind:?}: {}", v.total());
        }
    }
}
