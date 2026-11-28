//! Ilova: o'yin mantig'i + oqim bilan yuklash + GPU resurslari. Platformadan mustaqil (Android'da ham, testlarda ham shu).
use crate::game::{decode_save, encode_save, Game};
use crate::hud::Hud;
use crate::math::*;
use crate::mesh::{bounds, cell_box, mesh_column, Vertex};
use crate::planet::*;
use crate::render::{GpuMesh, Renderer, Stats, View};
use crate::sky;
use crate::stream::{Built, Key, Streamer};

/// Dunyo yaratish yoki plita kodi o'zgarsa oshiriladi (diskdagi eski kesh avtomatik o'chadi).
pub const CACHE_VERSION: u32 = 5;
/// GPU dagi keshning eng ko'p to'rtburchak soni (sifat bo'yicha).
const GPU_QUADS: [usize; 3] = [150_000, 300_000, 500_000];
use crate::text::Atlas;
use crate::ui::{self, Layout, Mode};
use crate::world::World;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

pub const SEED: u32 = 20_261_128;
const EYE_PAD: f64 = 6.4;
const FONT: &[u8] = include_bytes!("../assets/font.ttf");

pub enum TouchEv {
    Down(i32, f32, f32),
    Move(i32, f32, f32),
    Up(i32),
    Cancel,
}

pub struct App {
    pub game: Game,
    pub stats: Stats,
    renderer: Renderer,
    atlas: Atlas,
    streamer: Streamer,
    scene: HashMap<Key, GpuMesh>,
    /// Endi kerak emas, lekin o'rniga yangisi tayyor bo'lguncha ko'rinib turadigan meshlar (teshiklar bo'lmasin).
    stale: HashMap<Key, f32>,
    dir: String,
    quality_applied: usize,
    /// Oqim hisoblash nuqtasi: o'yinchi 24 blokdan ko'p uzoqlashgandagina siljiydi (halqalar tez-tez almashmasin).
    anchor: D3,
    last_anchor: D3,
    last_used: HashMap<Key, u64>,
    tick: u64,
    since_stream: f32,
    since_evict: f32,
    since_save: f32,
    started: Instant,
    pub fov: f32,
}

fn save_path(dir: &str) -> String {
    format!("{dir}/world.bin")
}

impl App {
    /// GL konteksti joriy bo'lishi shart. `dir` — saqlash papkasi (bo'sh bo'lsa saqlanmaydi).
    pub fn new(dir: &str) -> Result<App, String> {
        let world = Arc::new(World::new(SEED));
        let mut saved = None;
        if !dir.is_empty() {
            if let Ok(bytes) = std::fs::read(save_path(dir)) {
                if let Some((s, edits)) = decode_save(&bytes) {
                    world.load_edits(edits);
                    saved = Some(s);
                }
            }
        }
        crate::audio::start();
        let atlas = Atlas::build(FONT, 48.0).ok_or("shrift atlasi qurilmadi")?;
        let renderer = Renderer::new(&atlas)?;
        let mut game = Game::new(world.clone(), saved);
        if !dir.is_empty() {
            let crash = format!("{dir}/crash.txt");
            let flag = format!("{dir}/running.flag");
            if let Ok(t) = std::fs::read_to_string(&crash) {
                let short: String = t.chars().filter(|c| c.is_ascii() && *c != '\n').take(110).collect();
                game.toast = Some((format!("OLDINGI YIQILISH: {short}"), 20.0));
                let _ = std::fs::remove_file(&crash);
            } else if std::path::Path::new(&flag).exists() {
                game.toast = Some(("OLDINGI SAFAR YIQILGAN (EHTIMOL XOTIRA)".to_string(), 10.0));
            }
            let _ = std::fs::write(&flag, b"1");
        }
        let workers = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).saturating_sub(1).clamp(1, 3);
        let disk = if dir.is_empty() {
            None
        } else {
            Some((format!("{dir}/cache"), format!("v{CACHE_VERSION}-s{SEED}")))
        };
        let streamer = Streamer::new(world, workers, disk);
        Ok(App {
            game,
            stats: Stats::default(),
            renderer,
            atlas,
            streamer,
            scene: HashMap::new(),
            stale: HashMap::new(),
            dir: dir.to_string(),
            quality_applied: usize::MAX,
            anchor: [f64::MAX; 3],
            last_anchor: [f64::MAX; 3],
            last_used: HashMap::new(),
            tick: 0,
            since_stream: 10.0,
            since_evict: 0.0,
            since_save: 0.0,
            started: Instant::now(),
            fov: 1.15,
        })
    }

    pub fn touch(&mut self, e: TouchEv) {
        match e {
            TouchEv::Down(id, x, y) => self.game.touch_down(id, x, y),
            TouchEv::Move(id, x, y) => self.game.touch_move(id, x, y),
            TouchEv::Up(id) => self.game.touch_up(id),
            TouchEv::Cancel => self.game.touch_cancel_all(),
        }
    }

    /// Orqaga tugmasi: pauza/blok oynasini ochadi yoki yopadi. Har doim ishlatiladi (`true`).
    pub fn back(&mut self) -> bool {
        if self.game.mode == Mode::Craft {
            self.game.close_craft();
            return true;
        }
        self.game.mode = match self.game.mode {
            Mode::Play => Mode::Pause,
            Mode::Dead => Mode::Dead,
            _ => Mode::Play,
        };
        self.game.touches.clear();
        true
    }

    pub fn save(&mut self) {
        if self.dir.is_empty() {
            return;
        }
        let bytes = encode_save(&self.game.saved(), &self.game.world.save_edits());
        let tmp = format!("{}.tmp", save_path(&self.dir));
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(&tmp, save_path(&self.dir));
        }
        self.since_save = 0.0;
    }

    /// Toza chiqish belgisi: pauza/yopishda olib tashlanadi, ishga tushganda qo'yiladi.
    pub fn mark(&self, running: bool) {
        if self.dir.is_empty() {
            return;
        }
        let flag = format!("{}/running.flag", self.dir);
        if running {
            let _ = std::fs::write(&flag, b"1");
        } else {
            let _ = std::fs::remove_file(&flag);
        }
    }

    pub fn pending(&self) -> usize {
        self.streamer.pending()
    }

    pub fn loaded_meshes(&self) -> usize {
        self.scene.len()
    }

    /// O'yinchi turgan va qo'shni blok ustunlari chizilganmi.
    fn ground_loaded(&self) -> bool {
        let c = cell_at(self.game.player.pos);
        let (cu, cv) = ((c.u / 16) as i32, (c.v / 16) as i32);
        let max = (N / 16) as i32;
        for (du, dv) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (u, v) = (cu + du, cv + dv);
            if u < 0 || v < 0 || u >= max || v >= max {
                continue;
            }
            if !self.scene.contains_key(&Key::Col(c.f, u, v)) {
                return false;
            }
        }
        true
    }

    /// Ufqqacha masofa (blok): tuman va uzoq kesim tekisligi shunga bog'liq.
    pub fn horizon(&self) -> f64 {
        let alt = self.game.player.altitude().max(0.0) + EYE_PAD;
        let r = SEA_R as f64;
        (2.0 * r * alt + alt * alt).sqrt()
    }

    fn drop_mesh(&mut self, k: &Key) {
        if let Some(mut m) = self.scene.remove(k) {
            m.free();
        }
    }

    fn put_mesh(&mut self, b: Built) {
        let k = b.key;
        self.stale.remove(&k);
        let m = GpuMesh::upload(&b);
        if let Some(mut old) = self.scene.insert(k, m) {
            old.free();
        }
    }

    fn update_streaming(&mut self, dt: f32) {
        self.since_stream += dt;
        self.since_evict += dt;
        let p = self.game.player.pos;
        // Gisteresis: nuqta kichik harakatlarda qotib turadi
        if self.anchor[0] == f64::MAX || d_len(d_sub(p, self.anchor)) > 24.0 {
            self.anchor = p;
        }
        if self.quality_applied != self.game.quality || self.anchor != self.last_anchor || self.since_stream > 1.5 {
            self.quality_applied = self.game.quality;
            let have: HashSet<Key> = self.scene.keys().copied().collect();
            let unload = self.streamer.update(self.anchor, self.game.quality, |k| have.contains(k));
            for k in unload {
                if self.scene.contains_key(&k) {
                    self.stale.insert(k, 0.0);
                }
            }
            // Yana kerak bo'lib qolgan keshdagi meshlar eskirgan ro'yxatdan chiqadi
            let wanted: Vec<Key> = self.stale.keys().copied().filter(|k| self.streamer.is_wanted(k)).collect();
            for k in wanted {
                self.stale.remove(&k);
            }
            self.last_anchor = self.anchor;
            self.since_stream = 0.0;
        }
        if self.since_evict > 4.0 {
            let q = self.game.quality.min(2);
            self.game.world.evict([8_000, 14_000, 22_000][q], [3_000, 5_000, 8_000][q]);
            self.since_evict = 0.0;
        }
        // GPU ga yuklash: kadr vaqtini ushlab turish uchun cheklangan
        let t0 = Instant::now();
        loop {
            let got = self.streamer.poll(1);
            if got.is_empty() {
                break;
            }
            for b in got {
                self.put_mesh(b);
            }
            if t0.elapsed().as_micros() > 4000 {
                break;
            }
        }
        // O'rniga yangisi tayyor bo'lgach eskisi chizilmaydi (lekin keshda qoladi: qaytsangiz tayyor turadi)
        let idle = self.streamer.pending() == 0;
        let mut hide = Vec::new();
        for (k, age) in self.stale.iter_mut() {
            *age += dt;
            if (idle && *age > 0.4) || *age > 6.0 {
                hide.push(*k);
            }
        }
        for k in hide {
            self.stale.remove(&k);
        }
        // GPU keshi chegarasi: eng uzoq vaqt kerak bo'lmaganlarni o'chiramiz
        self.tick += 1;
        if self.tick % 30 == 0 {
            self.trim_gpu_cache();
        }
        // Tahrirlangan ustunlarni darhol qayta quramiz
        let cols: HashSet<(u8, i32, i32)> = self.game.dirty.drain(..).collect();
        for (f, cx, cz) in cols {
            let key = Key::Col(f, cx, cz);
            if self.streamer.loaded.contains(&key) {
                let mesh = mesh_column(&self.game.world, f, cx, cz);
                let (min, max) = bounds(&mesh);
                self.put_mesh(Built { key, mesh, min, max });
            }
        }
    }

    fn is_visible(&self, k: &Key) -> bool {
        self.streamer.is_wanted(k) || self.stale.contains_key(k)
    }

    fn trim_gpu_cache(&mut self) {
        let now = self.tick;
        let keys: Vec<Key> = self.scene.keys().copied().collect();
        let mut total = 0usize;
        for k in &keys {
            if self.is_visible(k) {
                self.last_used.insert(*k, now);
            }
            total += self.scene[k].quads;
        }
        let budget = GPU_QUADS[self.game.quality.min(2)];
        if total <= budget {
            return;
        }
        let mut cand: Vec<(u64, Key)> = keys
            .iter()
            .filter(|k| !self.is_visible(k))
            .map(|k| (self.last_used.get(k).copied().unwrap_or(0), *k))
            .collect();
        cand.sort_by_key(|c| c.0);
        for (_, k) in cand {
            if total <= budget {
                break;
            }
            total = total.saturating_sub(self.scene.get(&k).map_or(0, |m| m.quads));
            self.last_used.remove(&k);
            self.streamer.loaded.remove(&k);
            self.drop_mesh(&k);
        }
    }

    pub fn cached_meshes(&self) -> usize {
        self.scene.len()
    }

    fn loading_progress(&self) -> f32 {
        let c = cell_at(self.game.player.pos);
        let (pcu, pcv) = ((c.u / 16) as i32, (c.v / 16) as i32);
        let mut have = 0;
        let mut total = 0;
        for dv in -3..=3 {
            for du in -3..=3 {
                total += 1;
                if self.scene.contains_key(&Key::Col(c.f, pcu + du, pcv + dv)) {
                    have += 1;
                }
            }
        }
        have as f32 / total as f32
    }

    /// Mantiq va oqim (chizmasdan). Testlarda tez yuklash uchun alohida.
    pub fn step(&mut self, w: i32, h: i32, dt: f32) {
        self.game.resize(w as f32, h as f32);
        if dt > 0.0 {
            self.game.fps = if self.game.fps == 0.0 { 1.0 / dt } else { self.game.fps * 0.92 + 0.08 / dt };
        }
        self.update_streaming(dt);
        if self.game.loading < 1.0 {
            let p = self.loading_progress();
            self.game.loading = if p > 0.95 || self.started.elapsed().as_secs() > 25 { 1.0 } else { p.min(0.99) };
        }
        let ready = self.game.loading >= 1.0;
        self.game.ground_ready = self.ground_loaded();
        self.game.update(dt, ready);
        for s in self.game.sfx.drain(..) {
            crate::audio::play(s);
        }
        crate::audio::set_rain(self.game.rain);
        crate::audio::set_music(self.game.mode == Mode::Play);
        self.since_save += dt;
        if self.since_save > 30.0 {
            self.save();
        }
    }

    /// Hozirgi holatni ekranga chizadi.
    pub fn draw(&mut self, w: i32, h: i32) {
        let ready = self.game.loading >= 1.0;
        let pl = &self.game.player;
        let up = d_f32(pl.up());
        let sky_state = sky::overcast(sky::state(self.game.time, up), self.game.rain);
        let alt = pl.altitude().max(0.0);
        let horizon = self.horizon();
        let (r, cu, f) = pl.camera_basis();
        let view = View {
            pos: pl.eye_render(),
            right: d_f32(r),
            up: d_f32(cu),
            fwd: d_f32(f),
            local_up: up,
            atm: (-alt / 60_000.0).exp() as f32,
            fov: self.fov + if pl.running { 0.07 } else { 0.0 },
            w,
            h,
            near: (alt * 0.004).clamp(0.4, 500_000.0) as f32,
            far: (horizon * 1.3 + 8000.0) as f32,
        };
        let meshes: Vec<&GpuMesh> = self
            .scene
            .iter()
            .filter(|(k, m)| !m.is_empty() && self.streamer.is_wanted(k))
            .map(|(_, m)| m)
            .collect();
        let stale_list: Vec<&GpuMesh> = self
            .scene
            .iter()
            .filter(|(k, m)| !m.is_empty() && !self.streamer.is_wanted(k) && self.stale.contains_key(k))
            .map(|(_, m)| m)
            .collect();
        let hl: Option<(D3, Vec<Vertex>)> = if self.game.mode == Mode::Play && ready { self.game.target.map(|t| cell_box(&t.cell)) } else { None };
        let dynm = if ready { self.game.dynamic() } else { None };
        let shadows_on = self.game.quality >= 1 && ready;
        let shadow_meshes: Vec<&GpuMesh> = if shadows_on {
            self.scene
                .iter()
                .filter(|(k, m)| matches!(k, Key::Col(..)) && !m.is_empty() && self.streamer.is_wanted(k) && d_len(d_sub(m.origin, view.pos)) < 300.0)
                .map(|(_, m)| m)
                .collect()
        } else {
            Vec::new()
        };
        self.stats = self.renderer.draw_scene(&view, &sky_state, horizon.max(6000.0) as f32, &meshes, hl.as_ref(), dynm.as_ref(), &shadow_meshes, &stale_list);
        let layout = Layout::new(w as f32, h as f32);
        let state = self.game.ui_state();
        let mut hud = Hud::new(&self.atlas);
        ui::draw(&mut hud, &layout, &state);
        self.renderer.draw_hud(&hud, w, h);
    }

    /// Bitta kadr: mantiq + oqim + chizish.
    pub fn frame(&mut self, w: i32, h: i32, dt: f32) {
        self.step(w, h, dt);
        self.draw(w, h);
    }

    pub fn read_pixels(&self, w: i32, h: i32) -> Vec<u8> {
        self.renderer.read_pixels(w, h)
    }
}
