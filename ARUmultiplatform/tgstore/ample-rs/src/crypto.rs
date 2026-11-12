//! UZ: shifrlash (AES-256-GCM) va bo'lak konteyneri. Format Node varianti bilan BIR XIL: `iv(12) | tag(16) | shifrlangan matn`.
//! EN: AES-256-GCM sealing and the chunk container. Byte-compatible with the Node version: `iv(12) | tag(16) | ciphertext`.
//! RU: шифрование AES-256-GCM и контейнер чанка; формат совместим с версией на Node.
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{anyhow, Result};
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub fn sha256_hex(s: &str) -> String { hex::encode(Sha256::digest(s.as_bytes())) }

pub fn key32(hex_key: &str) -> Result<[u8; 32]> {
    let b = hex::decode(hex_key.trim())?;
    b.try_into().map_err(|_| anyhow!("kalit 32 bayt (64 hex) bo'lishi kerak"))
}

pub fn random_key_hex() -> String { let mut k = [0u8; 32]; rand::thread_rng().fill_bytes(&mut k); hex::encode(k) }

pub fn seal(key: &[u8; 32], plain: &[u8]) -> Vec<u8> {
    let mut iv = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut iv);
    let c = Aes256Gcm::new(key.into());
    let ct_tag = c.encrypt(Nonce::from_slice(&iv), plain).expect("aes-gcm encrypt");
    let (ct, tag) = ct_tag.split_at(ct_tag.len() - 16);
    let mut out = Vec::with_capacity(28 + ct.len());
    out.extend_from_slice(&iv);
    out.extend_from_slice(tag);
    out.extend_from_slice(ct);
    out
}

pub fn open(key: &[u8; 32], sealed: &[u8]) -> Result<Vec<u8>> {
    if sealed.len() < 28 { return Err(anyhow!("shifrlangan fayl juda qisqa")); }
    let (iv, rest) = sealed.split_at(12);
    let (tag, ct) = rest.split_at(16);
    let mut buf = Vec::with_capacity(ct.len() + 16);
    buf.extend_from_slice(ct);
    buf.extend_from_slice(tag);
    Aes256Gcm::new(key.into()).decrypt(Nonce::from_slice(iv), buf.as_slice()).map_err(|_| anyhow!("ochib bo'lmadi (kalit noto'g'ri yoki fayl buzilgan)"))
}

/// `[u32 BE sarlavha uzunligi][sarlavha JSON [{"n","l"}]][fayllar ketma-ket]` (nomlar tartiblangan). EN/RU: container of named files.
pub fn pack(files: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    let head: Vec<serde_json::Value> = files.iter().map(|(n, b)| serde_json::json!({"n": n, "l": b.len()})).collect();
    let h = serde_json::to_vec(&head).unwrap();
    let mut out = Vec::new();
    out.extend_from_slice(&(h.len() as u32).to_be_bytes());
    out.extend_from_slice(&h);
    for b in files.values() { out.extend_from_slice(b); }
    out
}

pub fn unpack(buf: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    if buf.len() < 4 { return Err(anyhow!("konteyner buzilgan")); }
    let hl = u32::from_be_bytes(buf[..4].try_into().unwrap()) as usize;
    let head: Vec<serde_json::Value> = serde_json::from_slice(buf.get(4..4 + hl).ok_or_else(|| anyhow!("konteyner buzilgan"))?)?;
    let mut off = 4 + hl;
    let mut out = BTreeMap::new();
    for h in head {
        let (n, l) = (h["n"].as_str().unwrap_or("").to_string(), h["l"].as_u64().unwrap_or(0) as usize);
        out.insert(n, buf.get(off..off + l).ok_or_else(|| anyhow!("konteyner buzilgan"))?.to_vec());
        off += l;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_open_roundtrip_and_tamper() {
        let k = key32(&random_key_hex()).unwrap();
        let s = seal(&k, b"maxfiy");
        assert_eq!(open(&k, &s).unwrap(), b"maxfiy");
        assert!(open(&key32(&random_key_hex()).unwrap(), &s).is_err());
        let mut t = s.clone(); *t.last_mut().unwrap() ^= 1;
        assert!(open(&k, &t).is_err());
    }

    #[test]
    fn container_roundtrip() {
        let mut f = BTreeMap::new();
        f.insert("a.csv".to_string(), b"id\n1\n".to_vec());
        f.insert("b.csv".to_string(), vec![]);
        assert_eq!(unpack(&pack(&f)).unwrap(), f);
    }
}
