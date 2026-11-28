//! Dunyoni oqim bilan yuklash: o'yinchi atrofida kerakli ustunlar va uzoq relyef plitalarini aniqlaydi,
//! mesh qurishni fon oqimlariga beradi (o'yin kadrlarida qotish bo'lmasin), tayyor meshlarni qaytaradi.
use crate::lod::{self, Leaf, K_LOD};
use crate::math::D3;
use crate::mesh::{bounds, mesh_column, push_quad, MeshData};
use crate::planet::*;
use crate::tilecache::TileCache;
use crate::world::World;
use std::collections::HashSet;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Condvar, Mutex};

pub const QUALITY_NAMES: [&str; 3] = ["PAST", "O'RTA", "YUQORI"];

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub enum Key {
    /// Yaqin bloklar ustuni: (yuz, cu, cv).
    Col(u8, i32, i32),
    /// Uzoq relyef plitasi.
    Tile(Leaf),
}

pub struct Built {
    pub key: Key,
    pub mesh: MeshData,
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Key {
    /// Mesh boshlanish nuqtasi (dunyo koordinatalarida).
    pub fn origin(&self) -> D3 {
        match *self {
            Key::Col(f, cu, cv) => col_anchor(f, cu, cv),
            Key::Tile(l) => tile_anchor(l.f, l.l, l.tu, l.tv),
        }
    }
}

struct Shared {
    jobs: Vec<(f32, Key)>,
    inflight: HashSet<Key>,
    quit: bool,
}

struct Queue {
    m: Mutex<Shared>,
    cv: Condvar,
}

pub struct Streamer {
    q: Arc<Queue>,
    rx: Receiver<Built>,
    want: HashSet<Key>,
    pub loaded: HashSet<Key>,
}

/// Kamera nuqtasi va sifat uchun kerakli kalitlar (kalit, masofa).
pub fn wanted(cam: D3, quality: usize) -> Vec<(Key, f32)> {
    let (k1, k2) = K_LOD[quality.min(2)];
    lod::leaves(cam, k1, k2)
        .into_iter()
        .map(|(l, d)| {
            let k = if l.l == 0 { Key::Col(l.f, l.tu as i32, l.tv as i32) } else { Key::Tile(l) };
            (k, d as f32)
        })
        .collect()
}

fn build(world: &World, cache: &TileCache, key: Key) -> Built {
    match key {
        Key::Col(f, cu, cv) => {
            let mesh = mesh_column(world, f, cu, cv);
            let (min, max) = bounds(&mesh);
            Built { key, mesh, min, max }
        }
        Key::Tile(lf) => {
            let verts = cache.get(&world.gen, lf);
            let mut mesh = MeshData::default();
            for q in verts.chunks_exact(4) {
                push_quad(&mut mesh.opaque, [q[0], q[1], q[2], q[3]]);
            }
            let (min, max) = bounds(&mesh);
            Built { key, mesh, min, max }
        }
    }
}

impl Streamer {
    /// `disk` — (kesh papkasi, versiya) yoki `None` (faqat xotira keshi).
    pub fn new(world: Arc<World>, workers: usize, disk: Option<(String, String)>) -> Streamer {
        let cache = Arc::new(TileCache::new(disk));
        let q = Arc::new(Queue {
            m: Mutex::new(Shared {
                jobs: Vec::new(),
                inflight: HashSet::new(),
                quit: false,
            }),
            cv: Condvar::new(),
        });
        let (tx, rx) = sync_channel::<Built>(48);
        for i in 0..workers.max(1) {
            let (q, w, tx, tc): (Arc<Queue>, Arc<World>, SyncSender<Built>, Arc<TileCache>) = (q.clone(), world.clone(), tx.clone(), cache.clone());
            let _ = std::thread::Builder::new().name(format!("mesh{i}")).spawn(move || loop {
                let key = {
                    let mut g = q.m.lock().unwrap();
                    loop {
                        if g.quit {
                            return;
                        }
                        // Eng yaqin ish (kichik masofa) oldin
                        if let Some(best) = g
                            .jobs
                            .iter()
                            .enumerate()
                            .min_by(|a, b| a.1 .0.partial_cmp(&b.1 .0).unwrap_or(std::cmp::Ordering::Equal))
                            .map(|(i, _)| i)
                        {
                            let (_, k) = g.jobs.swap_remove(best);
                            g.inflight.insert(k);
                            break k;
                        }
                        g = q.cv.wait(g).unwrap();
                    }
                };
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| build(&w, &tc, key)));
                q.m.lock().unwrap().inflight.remove(&key);
                if let Ok(b) = res {
                    if tx.send(b).is_err() {
                        return;
                    }
                }
            });
        }
        Streamer {
            q,
            rx,
            want: HashSet::new(),
            loaded: HashSet::new(),
        }
    }

    /// Kerakli ishlar ro'yxatini yangilaydi. Endi kerak bo'lmagan yuklangan kalitlar qaytariladi (GPU dan o'chirish uchun).
    /// `have` — GPU keshida allaqachon bor kalitlar: ular qayta qurilmaydi, darhol "yuklangan" hisoblanadi.
    pub fn update(&mut self, cam: D3, quality: usize, have: impl Fn(&Key) -> bool) -> Vec<Key> {
        let list = wanted(cam, quality);
        self.want = list.iter().map(|(k, _)| *k).collect();
        for (k, _) in &list {
            if have(k) {
                self.loaded.insert(*k);
            }
        }
        let unload: Vec<Key> = self.loaded.iter().filter(|k| !self.want.contains(k)).copied().collect();
        for k in &unload {
            self.loaded.remove(k);
        }
        let mut g = self.q.m.lock().unwrap();
        let infl = g.inflight.clone();
        g.jobs = list
            .into_iter()
            .filter(|(k, _)| !self.loaded.contains(k) && !infl.contains(k))
            .map(|(k, d)| (d, k))
            .collect();
        drop(g);
        self.q.cv.notify_all();
        unload
    }

    /// Tayyor bo'lgan va hali kerak bo'lgan meshlar (eng ko'pi bilan `max`).
    pub fn poll(&mut self, max: usize) -> Vec<Built> {
        let mut out = Vec::new();
        while out.len() < max {
            match self.rx.try_recv() {
                Ok(b) => {
                    if self.want.contains(&b.key) {
                        self.loaded.insert(b.key);
                        out.push(b);
                    }
                }
                Err(_) => break,
            }
        }
        out
    }

    /// Navbatda va bajarilayotgan ishlar soni.
    pub fn pending(&self) -> usize {
        let g = self.q.m.lock().unwrap();
        g.jobs.len() + g.inflight.len()
    }

    pub fn is_wanted(&self, k: &Key) -> bool {
        self.want.contains(k)
    }
}

impl Drop for Streamer {
    fn drop(&mut self) {
        self.q.m.lock().unwrap().quit = true;
        self.q.cv.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn cam(f: u8, u: f64, v: f64, alt: f64) -> D3 {
        crate::math::d_mul(dir(f, u, v), SEA_R as f64 + alt)
    }

    #[test]
    fn kalitlar_soni_va_takrorsiz() {
        for q in 0..3 {
            let w = wanted(cam(0, 30_000_000.0, 30_000_000.0, 8.0), q);
            let cols = w.iter().filter(|(k, _)| matches!(k, Key::Col(..))).count();
            assert!(cols > 100 && cols < 3000, "ustunlar {cols}");
            let tiles = w.len() - cols;
            assert!(tiles > 50 && tiles < 6000, "plitalar {tiles}");
            let uniq: HashSet<_> = w.iter().map(|(k, _)| *k).collect();
            assert_eq!(uniq.len(), w.len(), "takroriy kalitlar");
        }
    }

    #[test]
    fn oqim_mesh_quradi_va_olib_tashlaydi() {
        let w = Arc::new(World::new(5));
        let mut s = Streamer::new(w, 2, None);
        let c = cam(0, 30_000_000.0, 30_000_000.0, 8.0);
        let unload = s.update(c, 0, |_| false);
        assert!(unload.is_empty());
        let t0 = Instant::now();
        let mut got = Vec::new();
        while got.len() < 30 && t0.elapsed() < Duration::from_secs(120) {
            got.extend(s.poll(10));
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(got.len() >= 30, "mesh kam keldi: {}", got.len());
        assert!(got.iter().any(|b| matches!(b.key, Key::Col(..)) && b.mesh.quads() > 0));
        // sayyoraning narigi tomoniga o'tsak eskilari olib tashlanadi
        let unload = s.update(cam(1, 30_000_000.0, 30_000_000.0, 8.0), 0, |_| false);
        assert!(!unload.is_empty());
        assert!(s.loaded.is_empty());
    }

    #[test]
    fn plita_kesh_orqali_quriladi() {
        let w = World::new(5);
        let lf = Leaf { f: 2, l: 3, tu: 100_000, tv: 200_000 };
        let b = build(&w, &TileCache::new(None), Key::Tile(lf));
        assert!(b.mesh.quads() > 0);
        assert!(b.min.iter().chain(b.max.iter()).all(|v| v.is_finite()));
    }

    #[test]
    fn keshdagi_kalitlar_qayta_qurilmaydi() {
        let w = Arc::new(World::new(5));
        let mut s = Streamer::new(w, 1, None);
        let c = cam(0, 30_000_000.0, 30_000_000.0, 8.0);
        let cached: HashSet<Key> = wanted(c, 0).into_iter().map(|(k, _)| k).collect();
        let unload = s.update(c, 0, |k| cached.contains(k));
        assert!(unload.is_empty());
        assert_eq!(s.pending(), 0, "keshdagilar uchun ish bo'lmasligi kerak");
        assert_eq!(s.loaded.len(), cached.len());
        let unload = s.update(cam(1, 30_000_000.0, 30_000_000.0, 8.0), 0, |_| false);
        assert_eq!(unload.len(), cached.len());
    }
}
