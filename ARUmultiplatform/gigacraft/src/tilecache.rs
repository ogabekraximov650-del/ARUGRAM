//! Uzoq relyef plitalari keshi: xotirada (LRU) va diskda. Bir marta qurilgan plita qayta hisoblanmaydi:
//! guruh tarkibi o'zgarganda yoki orqaga qaytilganda tayyor ma'lumot olinadi.
use crate::gen::Gen;
use crate::lod::{self, Leaf};
use crate::mesh::Vertex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Xotiradagi keshning eng ko'p cho'qqisi (20 bayt/cho'qqi ~ 30 MB).
const MEM_VERTS: usize = 1_500_000;
/// Diskdagi kesh chegarasi (bayt): oshsa eng eskilari o'chiriladi.
const DISK_MAX: u64 = 256 * 1024 * 1024;
const DISK_KEEP: u64 = 192 * 1024 * 1024;

type Key = Leaf;

struct Mem {
    map: HashMap<Key, (Arc<Vec<Vertex>>, u64)>,
    verts: usize,
    tick: u64,
    writes: u32,
}

pub struct TileCache {
    mem: Mutex<Mem>,
    dir: Option<PathBuf>,
    cap: usize,
}

fn encode(v: &[Vertex]) -> Vec<u8> {
    let mut o = Vec::with_capacity(v.len() * 20);
    for x in v {
        for f in x.p {
            o.extend_from_slice(&f.to_le_bytes());
        }
        o.extend_from_slice(&x.c);
        o.extend(x.n.iter().map(|&b| b as u8));
    }
    o
}

fn decode(b: &[u8]) -> Option<Vec<Vertex>> {
    if b.len() % 20 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(b.len() / 20);
    for c in b.chunks_exact(20) {
        let f = |o: usize| f32::from_le_bytes([c[o], c[o + 1], c[o + 2], c[o + 3]]);
        let p = [f(0), f(4), f(8)];
        if p.iter().any(|v| !v.is_finite()) {
            return None;
        }
        out.push(Vertex {
            p,
            c: [c[12], c[13], c[14], c[15]],
            n: [c[16] as i8, c[17] as i8, c[18] as i8, c[19] as i8],
        });
    }
    Some(out)
}

fn trim_dir(dir: &PathBuf) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("bin") {
            continue;
        }
        if let Ok(m) = e.metadata() {
            files.push((m.modified().unwrap_or(std::time::UNIX_EPOCH), m.len(), p));
        }
    }
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    if total <= DISK_MAX {
        return;
    }
    files.sort_by_key(|f| f.0);
    for (_, len, p) in files {
        if total <= DISK_KEEP {
            break;
        }
        if std::fs::remove_file(&p).is_ok() {
            total = total.saturating_sub(len);
        }
    }
}

impl TileCache {
    /// `disk` — (papka, versiya). Versiya o'zgarsa (dunyo yaratish kodi yangilangan bo'lsa) eski fayllar o'chiriladi.
    pub fn new(disk: Option<(String, String)>) -> TileCache {
        let dir = disk.and_then(|(d, ver)| {
            let dir = PathBuf::from(d);
            std::fs::create_dir_all(&dir).ok()?;
            let vf = dir.join("ver.txt");
            if std::fs::read_to_string(&vf).ok().as_deref() != Some(ver.as_str()) {
                if let Ok(rd) = std::fs::read_dir(&dir) {
                    for e in rd.flatten() {
                        if e.path().extension().and_then(|x| x.to_str()) == Some("bin") {
                            let _ = std::fs::remove_file(e.path());
                        }
                    }
                }
                std::fs::write(&vf, ver).ok()?;
            }
            trim_dir(&dir);
            Some(dir)
        });
        TileCache {
            mem: Mutex::new(Mem {
                map: HashMap::new(),
                verts: 0,
                tick: 0,
                writes: 0,
            }),
            dir,
            cap: MEM_VERTS,
        }
    }

    fn path(&self, k: Key) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join(format!("t{}_{}_{}_{}.bin", k.f, k.l, k.tu, k.tv)))
    }

    /// Plita cho'qqilari: xotira -> disk -> qurish (qurilsa diskka yoziladi).
    pub fn get(&self, g: &Gen, k: Leaf) -> Arc<Vec<Vertex>> {
        {
            let mut m = self.mem.lock().unwrap();
            m.tick += 1;
            let t = m.tick;
            if let Some(e) = m.map.get_mut(&k) {
                e.1 = t;
                return e.0.clone();
            }
        }
        let from_disk = self.path(k).and_then(|p| std::fs::read(p).ok()).and_then(|b| decode(&b));
        let (v, built) = match from_disk {
            Some(v) => (v, false),
            None => (lod::build_tile(g, k), true),
        };
        if built {
            if let Some(p) = self.path(k) {
                let tmp = p.with_extension("tmp");
                if std::fs::write(&tmp, encode(&v)).is_ok() {
                    let _ = std::fs::rename(&tmp, &p);
                }
            }
        }
        let v = Arc::new(v);
        let mut m = self.mem.lock().unwrap();
        m.tick += 1;
        let t = m.tick;
        m.verts += v.len();
        if let Some(old) = m.map.insert(k, (v.clone(), t)) {
            m.verts -= old.0.len();
        }
        if built {
            m.writes += 1;
            if m.writes % 400 == 0 {
                if let Some(d) = self.dir.clone() {
                    trim_dir(&d);
                }
            }
        }
        while m.verts > self.cap && m.map.len() > 1 {
            let oldest = m.map.iter().min_by_key(|(_, e)| e.1).map(|(k, _)| *k);
            if let Some(o) = oldest {
                if let Some(e) = m.map.remove(&o) {
                    m.verts -= e.0.len();
                }
            }
        }
        v
    }

    pub fn mem_tiles(&self) -> usize {
        self.mem.lock().unwrap().map.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> String {
        let d = std::env::temp_dir().join(format!("giga_{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d.to_string_lossy().into_owned()
    }

    #[test]
    fn kodlash_aylanadi() {
        let g = Gen::new(3);
        let v = lod::build_tile(&g, Leaf { f: 2, l: 2, tu: 100, tv: 200 });
        let b = encode(&v);
        assert_eq!(b.len(), v.len() * 20);
        assert_eq!(decode(&b).unwrap(), v);
        assert!(decode(&b[..b.len() - 1]).is_none());
    }

    #[test]
    fn xotira_keshi_ayni_ob_yektni_qaytaradi() {
        let g = Gen::new(3);
        let c = TileCache::new(None);
        let lf = Leaf { f: 1, l: 3, tu: 10, tv: 20 };
        let a = c.get(&g, lf);
        let b = c.get(&g, lf);
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(c.mem_tiles(), 1);
    }

    #[test]
    fn disk_keshi_va_versiya() {
        let g = Gen::new(3);
        let d = tmp("disk");
        let c1 = TileCache::new(Some((d.clone(), "v1".into())));
        let lf = Leaf { f: 2, l: 2, tu: 2, tv: 2 };
        let a = c1.get(&g, lf);
        assert!(std::path::Path::new(&format!("{d}/t2_2_2_2.bin")).exists());
        // yangi kesh (xotira bo'sh) diskdan o'qiydi va bir xil natija beradi
        let c2 = TileCache::new(Some((d.clone(), "v1".into())));
        let b = c2.get(&g, lf);
        assert_eq!(*a, *b);
        // versiya o'zgarsa fayllar o'chadi
        let _c3 = TileCache::new(Some((d.clone(), "v2".into())));
        assert!(!std::path::Path::new(&format!("{d}/t2_2_2_2.bin")).exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn xotira_chegaradan_oshsa_eskilari_chiqadi() {
        let g = Gen::new(3);
        let mut c = TileCache::new(None);
        c.cap = 30_000;
        for i in 0..60 {
            let _ = c.get(&g, Leaf { f: 0, l: 1, tu: 50_000 + i, tv: 70_000 });
        }
        let m = c.mem.lock().unwrap();
        assert!(m.verts <= 30_000 + 20_000, "{}", m.verts);
        assert!(m.map.len() < 60);
    }
}
