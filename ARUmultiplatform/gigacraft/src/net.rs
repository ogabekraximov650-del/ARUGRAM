//! Mahalliy tarmoq (bitta Wi-Fi) ko'p o'yinchi rejimi: UDP orqali o'yinchi holati va blok o'zgarishlari uzatiladi.
//! Server kerak emas: har bir qurilma hamma narsani tarmoqqa tarqatadi (broadcast), boshqalar ularni qabul qiladi.
use crate::math::*;
use crate::planet::Cell;
use std::collections::{HashMap, HashSet};
use std::net::{SocketAddr, UdpSocket};

pub const PORT: u16 = 47321;
const MAGIC: &[u8; 4] = b"GCN1";
const T_STATE: u8 = 1;
const T_EDIT: u8 = 2;
/// Boshqa o'yinchi shuncha soniya xabar bermasa, yo'qoladi.
const PEER_TTL: f32 = 3.0;

#[derive(Clone, Copy, Debug)]
pub struct Peer {
    pub dim: u8,
    pub pos: D3,
    pub fwd: D3,
    pub age: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edit {
    pub dim: u8,
    pub cell: Cell,
    pub block: u8,
}

#[derive(Clone, Copy)]
struct Own {
    seq: u32,
    e: Edit,
}

pub struct Net {
    sock: UdpSocket,
    targets: Vec<SocketAddr>,
    pub id: u32,
    pub peers: HashMap<u32, Peer>,
    seq: u32,
    seen: HashSet<(u32, u32)>,
    recent: Vec<Own>,
    resend_i: usize,
    state_t: f32,
    resend_t: f32,
}

fn new_id() -> u32 {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
    let x = t ^ (std::process::id() as u64) << 20 ^ 0x9E37_79B9_7F4A_7C15;
    ((x ^ (x >> 29)).wrapping_mul(0xBF58_476D_1CE4_E5B9) >> 32) as u32 | 1
}

impl Net {
    /// `listen` portida tinglaydi va `targets` manzillariga yuboradi.
    pub fn new(listen: u16, targets: Vec<SocketAddr>) -> std::io::Result<Net> {
        let sock = UdpSocket::bind(("0.0.0.0", listen))?;
        sock.set_nonblocking(true)?;
        let _ = sock.set_broadcast(true);
        Ok(Net {
            sock,
            targets,
            id: new_id(),
            peers: HashMap::new(),
            seq: 0,
            seen: HashSet::new(),
            recent: Vec::new(),
            resend_i: 0,
            state_t: 0.0,
            resend_t: 0.0,
        })
    }

    /// Wi-Fi tarmog'idagi hamma qurilmalarga.
    pub fn lan() -> std::io::Result<Net> {
        Net::new(PORT, vec![SocketAddr::from(([255, 255, 255, 255], PORT))])
    }

    /// Ko'rinayotgan (shu o'lchamdagi, yangi) o'yinchilar soni.
    pub fn visible(&self, dim: u8) -> usize {
        self.peers.values().filter(|p| p.dim == dim).count()
    }

    fn send(&self, buf: &[u8]) {
        for t in &self.targets {
            let _ = self.sock.send_to(buf, t);
        }
    }

    fn header(&self, ty: u8) -> Vec<u8> {
        let mut b = MAGIC.to_vec();
        b.push(ty);
        b.extend_from_slice(&self.id.to_le_bytes());
        b
    }

    fn edit_packet(&self, o: &Own) -> Vec<u8> {
        let mut b = self.header(T_EDIT);
        b.push(o.e.dim);
        b.extend_from_slice(&o.seq.to_le_bytes());
        b.push(o.e.cell.f);
        for x in [o.e.cell.u, o.e.cell.v, o.e.cell.r] {
            b.extend_from_slice(&x.to_le_bytes());
        }
        b.push(o.e.block);
        b
    }

    /// O'z blok o'zgarishimizni tarqatamiz (UDP ishonchsiz: keyin takror yuboriladi).
    pub fn send_edit(&mut self, dim: u8, cell: Cell, block: u8) {
        self.seq = self.seq.wrapping_add(1);
        let o = Own { seq: self.seq, e: Edit { dim, cell, block } };
        self.send(&self.edit_packet(&o));
        self.recent.push(o);
        if self.recent.len() > 512 {
            self.recent.remove(0);
        }
    }

    fn handle(&mut self, d: &[u8], out: &mut Vec<Edit>) {
        if d.len() < 9 || &d[0..4] != MAGIC {
            return;
        }
        let id = u32::from_le_bytes([d[5], d[6], d[7], d[8]]);
        if id == self.id {
            return;
        }
        let f64_at = |o: usize| d.get(o..o + 8).map(|b| f64::from_le_bytes(b.try_into().unwrap()));
        match d[4] {
            T_STATE if d.len() >= 9 + 1 + 48 => {
                let dim = d[9];
                let v: Option<Vec<f64>> = (0..6).map(|k| f64_at(10 + k * 8)).collect();
                let Some(v) = v else { return };
                if v.iter().any(|x| !x.is_finite()) {
                    return;
                }
                self.peers.insert(id, Peer { dim, pos: [v[0], v[1], v[2]], fwd: [v[3], v[4], v[5]], age: 0.0 });
            }
            T_EDIT if d.len() >= 9 + 1 + 4 + 1 + 24 + 1 => {
                let dim = d[9];
                let seq = u32::from_le_bytes([d[10], d[11], d[12], d[13]]);
                if !self.seen.insert((id, seq)) {
                    return;
                }
                if self.seen.len() > 60_000 {
                    self.seen.clear();
                }
                let f = d[14];
                let i = |o: usize| i64::from_le_bytes(d[o..o + 8].try_into().unwrap());
                if f > 5 {
                    return;
                }
                out.push(Edit { dim, cell: Cell { f, u: i(15), v: i(23), r: i(31) }, block: d[39] });
            }
            _ => {}
        }
    }

    /// Har kadr: o'z holatimizni yuboradi, kelganlarini qabul qiladi. Qaytadi: boshqalardan kelgan yangi blok o'zgarishlari.
    pub fn tick(&mut self, dt: f32, dim: u8, pos: D3, fwd: D3) -> Vec<Edit> {
        self.state_t += dt;
        if self.state_t >= 0.1 {
            self.state_t = 0.0;
            let mut b = self.header(T_STATE);
            b.push(dim);
            for x in pos.iter().chain(fwd.iter()) {
                b.extend_from_slice(&x.to_le_bytes());
            }
            self.send(&b);
        }
        // O'zgarishlarni sekin-asta takrorlaymiz: keyin ulangan qurilmalar ham oladi
        self.resend_t += dt;
        if self.resend_t >= 0.5 && !self.recent.is_empty() {
            self.resend_t = 0.0;
            for _ in 0..8 {
                self.resend_i = (self.resend_i + 1) % self.recent.len();
                let o = self.recent[self.resend_i];
                self.send(&self.edit_packet(&o));
            }
        }
        let mut out = Vec::new();
        let mut buf = [0u8; 256];
        for _ in 0..512 {
            match self.sock.recv_from(&mut buf) {
                Ok((n, _)) => {
                    let pkt = buf[..n].to_vec();
                    self.handle(&pkt, &mut out);
                }
                Err(_) => break,
            }
        }
        for p in self.peers.values_mut() {
            p.age += dt;
        }
        self.peers.retain(|_, p| p.age < PEER_TTL);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn pair(pa: u16, pb: u16) -> (Net, Net) {
        let a = Net::new(pa, vec![SocketAddr::from(([127, 0, 0, 1], pb))]).expect("a");
        let b = Net::new(pb, vec![SocketAddr::from(([127, 0, 0, 1], pa))]).expect("b");
        (a, b)
    }

    #[test]
    fn holat_va_blok_uzatiladi() {
        let (mut a, mut b) = pair(47411, 47412);
        assert_ne!(a.id, b.id);
        let cell = Cell { f: 3, u: 123_456_789, v: 42, r: 42_000_000 };
        a.send_edit(0, cell, 7);
        let mut got = Vec::new();
        for _ in 0..30 {
            let _ = a.tick(0.1, 0, [1.0, 2.0, 3.0], [0.0, 1.0, 0.0]);
            got.extend(b.tick(0.1, 0, [9.0, 9.0, 9.0], [1.0, 0.0, 0.0]));
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(got.len(), 1, "blok bir marta kelishi kerak (takror yuborishlarga qaramay): {got:?}");
        assert_eq!(got[0], Edit { dim: 0, cell, block: 7 });
        let p = b.peers.get(&a.id).expect("o'yinchi ko'rinmadi");
        assert_eq!(p.pos, [1.0, 2.0, 3.0]);
        assert_eq!(b.visible(0), 1);
        assert_eq!(b.visible(1), 0);
        // aloqa uzilsa o'yinchi yo'qoladi
        for _ in 0..40 {
            b.tick(0.1, 0, [0.0; 3], [1.0, 0.0, 0.0]);
        }
        assert!(b.peers.is_empty());
    }

    #[test]
    fn kech_ulangan_oldingi_ozgarishlarni_oladi() {
        let (mut a, mut b) = pair(47413, 47414);
        let cells: Vec<Cell> = (0..20).map(|i| Cell { f: 1, u: 1000 + i, v: 5, r: 99 }).collect();
        for c in &cells {
            a.send_edit(0, *c, 3);
        }
        // b hozir yo'q edi: faqat a takrorlaydi
        let mut got = Vec::new();
        for _ in 0..120 {
            let _ = a.tick(0.1, 0, [1.0; 3], [1.0; 3]);
            got.extend(b.tick(0.1, 0, [1.0; 3], [1.0; 3]));
        }
        assert_eq!(got.len(), 20, "ba'zilari yetib kelmadi: {}", got.len());
    }

    #[test]
    fn yaroqsiz_paketlar_etiborsiz() {
        let (mut a, mut b) = pair(47415, 47416);
        for junk in [&b"hello"[..], &[0u8; 3][..], &b"GCN1\x01"[..], &b"GCN1\x02\x01\x02\x03\x04\x00"[..]] {
            let _ = a.sock.send_to(junk, SocketAddr::from(([127, 0, 0, 1], 47416)));
        }
        std::thread::sleep(Duration::from_millis(20));
        assert!(b.tick(0.1, 0, [0.0; 3], [1.0; 3]).is_empty());
        assert!(b.peers.is_empty());
    }
}
