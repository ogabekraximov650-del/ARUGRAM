//! Dunyo saqlash: bo'laklar (16x16x16), ustun ma'lumotlari keshi, tahrirlar (buzish/qo'yish) va ularni saqlash.
use crate::blocks::*;
use crate::gen::{ColInfo, Gen, CV};
use crate::planet::*;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

pub struct Chunk {
    pub b: Vec<u8>,
    used: AtomicU64,
}

impl Chunk {
    fn new(b: Vec<u8>, tick: u64) -> Chunk {
        Chunk { b, used: AtomicU64::new(tick) }
    }
    pub fn is_air(&self) -> bool {
        self.b.iter().all(|&v| v == AIR)
    }
}

pub struct World {
    pub gen: Gen,
    chunks: RwLock<HashMap<CP, Arc<Chunk>>>,
    cols: Mutex<HashMap<(u8, i32, i32), Arc<ColInfo>>>,
    dirty: Mutex<HashSet<CP>>,
    tick: AtomicU64,
}

impl World {
    pub fn new(seed: u32) -> World {
        World {
            gen: Gen::new(seed),
            chunks: RwLock::new(HashMap::new()),
            cols: Mutex::new(HashMap::new()),
            dirty: Mutex::new(HashSet::new()),
            tick: AtomicU64::new(1),
        }
    }

    fn now(&self) -> u64 {
        self.tick.fetch_add(1, Ordering::Relaxed)
    }

    /// Ustun sirt ma'lumoti (keshlanadi).
    pub fn col(&self, f: u8, cu: i32, cv: i32) -> Arc<ColInfo> {
        if let Some(c) = self.cols.lock().unwrap().get(&(f, cu, cv)) {
            return c.clone();
        }
        let v = Arc::new(self.gen.column(f, cu, cv));
        self.cols.lock().unwrap().insert((f, cu, cv), v.clone());
        v
    }

    /// Chizish kerak bo'lgan radial bo'laklar oralig'i (ikkalasi ham kiradi).
    pub fn section_range(&self, f: u8, cu: i32, cv: i32) -> (i32, i32) {
        let col = self.col(f, cu, cv);
        let (lo, hi) = col.span();
        ((lo - 48).div_euclid(16) as i32, (hi.max(col.wmax()) + 75).div_euclid(16) as i32)
    }

    pub fn chunk(&self, cp: CP) -> Arc<Chunk> {
        if let Some(c) = self.chunks.read().unwrap().get(&cp) {
            c.used.store(self.now(), Ordering::Relaxed);
            return c.clone();
        }
        let col = self.col(cp.f, cp.u, cp.v);
        let c = Arc::new(Chunk::new(self.gen.fill_chunk(cp, &col), self.now()));
        self.chunks.write().unwrap().entry(cp).or_insert(c).clone()
    }

    pub fn block(&self, c: &Cell) -> u8 {
        let (cp, i) = chunk_of(c);
        self.chunk(cp).b[i]
    }

    /// Blokni o'zgartiradi. Qayta qurish kerak bo'lgan ustunlar ((yuz, cu, cv)) ro'yxatini qaytaradi.
    pub fn set_block(&self, c: &Cell, v: u8) -> Vec<(u8, i32, i32)> {
        let (cp, i) = chunk_of(c);
        let old = self.chunk(cp);
        if old.b[i] == v {
            return Vec::new();
        }
        let mut nb = old.b.clone();
        nb[i] = v;
        let t = self.now();
        self.chunks.write().unwrap().insert(cp, Arc::new(Chunk::new(nb, t)));
        self.dirty.lock().unwrap().insert(cp);
        let mut cols: Vec<(u8, i32, i32)> = vec![(cp.f, cp.u, cp.v)];
        let (lu, lv) = (c.u.rem_euclid(16), c.v.rem_euclid(16));
        for (du, dv, on) in [(-1, 0, lu == 0), (1, 0, lu == 15), (0, -1, lv == 0), (0, 1, lv == 15), (-1, -1, lu == 0 && lv == 0), (1, 1, lu == 15 && lv == 15), (-1, 1, lu == 0 && lv == 15), (1, -1, lu == 15 && lv == 0)] {
            if on {
                let n = neighbor(c, du, dv, 0);
                let (ncp, _) = chunk_of(&n);
                let k = (ncp.f, ncp.u, ncp.v);
                if !cols.contains(&k) {
                    cols.push(k);
                }
            }
        }
        cols
    }

    /// 18x18x18 "yostiqlangan" nusxa: bo'lak + atrofdagi bir qavat. Indeks: (u+1) + (v+1)*18 + (r+1)*324.
    pub fn padded(&self, cp: CP) -> Vec<u8> {
        let mut p = vec![AIR; 18 * 18 * 18];
        let me = self.chunk(cp);
        let (iu0, iv0, r0) = chunk_origin(cp);
        for lr in 0..16usize {
            for lv in 0..16usize {
                let src = lv * 16 + lr * 256;
                let dst = 1 + (lv + 1) * 18 + (lr + 1) * 324;
                p[dst..dst + 16].copy_from_slice(&me.b[src..src + 16]);
            }
        }
        for y in -1..=16i64 {
            for z in -1..=16i64 {
                let edge_yz = y == -1 || y == 16 || z == -1 || z == 16;
                for x in -1..=16i64 {
                    if !(edge_yz || x == -1 || x == 16) {
                        continue;
                    }
                    let c = wrap(cp.f, iu0 + x, iv0 + z, r0 + y);
                    p[(x + 1) as usize + (z + 1) as usize * 18 + (y + 1) as usize * 324] = self.block(&c);
                }
            }
        }
        p
    }

    /// Eng kam ishlatilgan, tahrirlanmagan bo'laklar va ustunlarni xotiradan chiqaradi.
    pub fn evict(&self, max_chunks: usize, max_cols: usize) {
        let dirty = self.dirty.lock().unwrap().clone();
        {
            let mut w = self.chunks.write().unwrap();
            if w.len() > max_chunks {
                let mut v: Vec<(u64, CP)> = w.iter().filter(|(k, _)| !dirty.contains(k)).map(|(k, c)| (c.used.load(Ordering::Relaxed), *k)).collect();
                v.sort_by_key(|x| x.0);
                let over = w.len() - max_chunks;
                for (_, k) in v.into_iter().take(over) {
                    w.remove(&k);
                }
            }
        }
        let mut c = self.cols.lock().unwrap();
        if c.len() > max_cols {
            let drop: Vec<_> = c.keys().take(c.len() - max_cols).copied().collect();
            for k in drop {
                c.remove(&k);
            }
        }
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.read().unwrap().len()
    }

    pub fn dirty_count(&self) -> usize {
        self.dirty.lock().unwrap().len()
    }

    /// Tahrirlangan bo'laklarni baytlarga aylantiradi.
    pub fn save_edits(&self) -> Vec<u8> {
        let dirty: Vec<CP> = self.dirty.lock().unwrap().iter().copied().collect();
        let mut out = b"GCW3".to_vec();
        out.extend_from_slice(&(dirty.len() as u32).to_le_bytes());
        let map = self.chunks.read().unwrap();
        for cp in dirty {
            if let Some(c) = map.get(&cp) {
                let r = rle_encode(&c.b);
                out.push(cp.f);
                for v in [cp.u, cp.v, cp.r] {
                    out.extend_from_slice(&v.to_le_bytes());
                }
                out.extend_from_slice(&(r.len() as u32).to_le_bytes());
                out.extend_from_slice(&r);
            }
        }
        out
    }

    /// `save_edits` natijasini qaytadan yuklaydi. Noto'g'ri bo'lsa `false`.
    pub fn load_edits(&self, d: &[u8]) -> bool {
        if d.len() < 8 || &d[0..4] != b"GCW3" {
            return false;
        }
        let rd = |o: usize| -> Option<u32> { Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?)) };
        let Some(n) = rd(4) else { return false };
        let mut o = 8;
        let mut loaded = Vec::new();
        for _ in 0..n {
            let Some(&f) = d.get(o) else { return false };
            let (Some(a), Some(b), Some(c), Some(len)) = (rd(o + 1), rd(o + 5), rd(o + 9), rd(o + 13)) else {
                return false;
            };
            o += 17;
            let Some(rle) = d.get(o..o + len as usize) else { return false };
            o += len as usize;
            let Some(blocks) = rle_decode(rle, CV) else { return false };
            if f > 5 {
                return false;
            }
            loaded.push((CP { f, u: a as i32, v: b as i32, r: c as i32 }, blocks));
        }
        let mut map = self.chunks.write().unwrap();
        let mut dirty = self.dirty.lock().unwrap();
        for (cp, b) in loaded {
            map.insert(cp, Arc::new(Chunk::new(b, 0)));
            dirty.insert(cp);
        }
        true
    }
}

/// Oddiy ketma-ket takrorlanish siqish: (qiymat, uzunlik u16) uchliklari.
pub fn rle_encode(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let v = b[i];
        let mut n = 1usize;
        while i + n < b.len() && b[i + n] == v && n < 65535 {
            n += 1;
        }
        out.push(v);
        out.extend_from_slice(&(n as u16).to_le_bytes());
        i += n;
    }
    out
}

pub fn rle_decode(r: &[u8], want: usize) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(want);
    for t in r.chunks(3) {
        if t.len() != 3 {
            return None;
        }
        let n = u16::from_le_bytes([t[1], t[2]]) as usize;
        out.extend(std::iter::repeat(t[0]).take(n));
        if out.len() > want {
            return None;
        }
    }
    if out.len() == want {
        Some(out)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HIGH: i64 = SEA_R + 5000; // havo (relyef bundan past)

    fn at(f: u8, u: i64, v: i64, r: i64) -> Cell {
        Cell { f, u, v, r }
    }

    #[test]
    fn blok_yozish_va_oqish() {
        let w = World::new(1);
        let c = at(2, 30_000_000, 20_000_000, HIGH);
        assert_eq!(w.block(&c), AIR);
        w.set_block(&c, PINK);
        assert_eq!(w.block(&c), PINK);
        assert_eq!(w.block(&neighbor(&c, 1, 0, 0)), AIR);
        w.set_block(&c, AIR);
        assert_eq!(w.block(&c), AIR);
    }

    #[test]
    fn chegara_tahriri_qoshni_ustunlarni_belgilaydi() {
        let w = World::new(2);
        let base = at(1, 1600, 1600, HIGH - HIGH % 16);
        assert_eq!(w.set_block(&base, STONE).len(), 4, "burchak: o'zi + 2 yon + diagonal");
        let mid = at(1, 1608, 1608, HIGH - HIGH % 16 + 3);
        assert_eq!(w.set_block(&mid, STONE).len(), 1);
        let side = at(1, 1615, 1608, HIGH - HIGH % 16 + 3);
        assert_eq!(w.set_block(&side, STONE).len(), 2);
    }

    #[test]
    fn yostiqlangan_nusxa_blok_bilan_mos() {
        let w = World::new(4);
        for &(f, u, v) in &[(0u8, 5_000_000i64, 7_000_000i64), (3, 100, 33_000_000), (5, 16, 16)] {
            let cp = CP { f, u: (u / 16) as i32, v: (v / 16) as i32, r: ((SEA_R + 3) / 16) as i32 };
            let p = w.padded(cp);
            let (iu0, iv0, r0) = chunk_origin(cp);
            for &(x, y, z) in &[(-1i64, 0i64, 0i64), (16, 5, 5), (0, -1, 0), (7, 16, 3), (-1, -1, -1), (16, 16, 16), (5, 5, -1), (5, 5, 16), (8, 8, 8)] {
                let pi = (x + 1) as usize + (z + 1) as usize * 18 + (y + 1) as usize * 324;
                assert_eq!(p[pi], w.block(&wrap(f, iu0 + x, iv0 + z, r0 + y)), "yuz {f} ({x},{y},{z})");
            }
        }
    }

    #[test]
    fn yuz_chetida_qoshni_yuzdan_oladi() {
        let w = World::new(6);
        // yuz 0 ning eng chetidagi bo'lak; tashqaridagi blok qo'shni yuzdan olinadi
        let cp = CP { f: 0, u: (N / 16 - 1) as i32, v: (N / 32) as i32, r: ((SEA_R + 3) / 16) as i32 };
        let p = w.padded(cp);
        let (iu0, iv0, r0) = chunk_origin(cp);
        let out = wrap(0, iu0 + 16, iv0 + 4, r0 + 3);
        assert_ne!(out.f, 0);
        assert_eq!(p[17 + 5 * 18 + 4 * 324], w.block(&out));
    }

    #[test]
    fn saqlash_va_yuklash() {
        let w = World::new(9);
        let a = at(2, 5_000_000, 6_000_000, HIGH);
        let b = at(4, 40_000_000, 9_000_000, HIGH + 7);
        w.set_block(&a, CRYSTAL);
        w.set_block(&b, WHITE);
        let data = w.save_edits();
        let w2 = World::new(9);
        assert!(w2.load_edits(&data));
        assert_eq!(w2.block(&a), CRYSTAL);
        assert_eq!(w2.block(&b), WHITE);
        assert_eq!(w2.dirty_count(), 2);
        assert!(!w2.load_edits(b"xato"));
    }

    #[test]
    fn rle_aylanadi() {
        let mut v = vec![3u8; 100];
        v.extend(vec![0u8; 3996]);
        let r = rle_encode(&v);
        assert!(r.len() < 20);
        assert_eq!(rle_decode(&r, 4096).unwrap(), v);
        assert!(rle_decode(&r, 10).is_none());
    }

    #[test]
    fn evict_tahrirlanganni_saqlaydi() {
        let w = World::new(1);
        let c = at(2, 5_000_000, 5_000_000, HIGH);
        w.set_block(&c, STONE);
        for i in 0..40 {
            let _ = w.block(&at(2, 6_000_000 + i * 16, 5_000_000, HIGH));
        }
        w.evict(10, 5);
        assert!(w.chunk_count() <= 12);
        assert_eq!(w.block(&c), STONE);
    }
}
