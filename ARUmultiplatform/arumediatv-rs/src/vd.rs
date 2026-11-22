//! Worker `vd` tizimidan rasm olish (Telegram'ga to'g'ridan-to'g'ri ulanishsiz): `imginfo` (hajm, kalit) + `img/<nom>` (shifrlangan baytlar).
//! UZ: baytlar AES-128-CTR bilan shifrlangan (IV = 0, kalit 32 hex), ilova o'zi ochadi. EN: same scheme as the Kotlin `VdImages`.

use aes::Aes128;
use ctr::cipher::{KeyIvInit, StreamCipher};
use serde_json::Value;
use std::collections::HashMap;

type Ctr = ctr::Ctr128BE<Aes128>;

fn hex16(h: &str) -> Option<[u8; 16]> {
    if h.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = u8::from_str_radix(&h[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

/// `offset` dan boshlab AES-128-CTR (IV = 0) — o'z-o'zini teskari qiladi (shifrlash = ochish).
pub fn apply(key_hex: &str, offset: u64, data: &mut [u8]) -> bool {
    let Some(key) = hex16(key_hex) else { return false };
    let mut c = Ctr::new(&key.into(), &[0u8; 16].into());
    c.seek(offset);
    c.apply_keystream(data);
    true
}

use ctr::cipher::StreamCipherSeek;

/// Rasm nomlari uchun ochilgan baytlar. Tayyor bo'lmagan / topilmagan nomlar natijada yo'q. Bloklaydi.
/// UZ: ommaviy rasmlar sessiyasiz (`imginfo`), yozishma va to'plam fayllari (`chat_`, `pki_`) — sessiya bilan (`info`), Kotlin `VdImages` kabi.
pub fn fetch_images(names: &[String], session: &str) -> HashMap<String, Vec<u8>> {
    let mut out = HashMap::new();
    if names.is_empty() {
        return out;
    }
    let (private, public): (Vec<&String>, Vec<&String>) = names.iter().partition(|n| n.starts_with("chat_") || n.starts_with("pki_"));
    let mut files = serde_json::Map::new();
    let mut ask = |path: &str, list: &[&String], bearer: Option<&str>| {
        if list.is_empty() {
            return;
        }
        let body = serde_json::json!({ "names": list });
        if let Ok((200, text)) = crate::api::request("POST", path, "", Some(&body), bearer) {
            if let Some(f) = serde_json::from_str::<Value>(&text).ok().and_then(|j| j["files"].as_object().cloned()) {
                files.extend(f);
            }
        }
    };
    ask("/api/vd/imginfo", &public, None);
    ask("/api/vd/info", &private, Some(session));
    for n in names {
        let Some(f) = files.get(n) else { continue };
        let (size, key, state) = (f["size"].as_u64().unwrap_or(0), f["key"].as_str().unwrap_or(""), f["state"].as_str().unwrap_or(""));
        if state != "ready" || size == 0 || key.len() != 32 {
            continue;
        }
        let Ok(mut enc) = crate::api::get_bytes(&format!("/api/vd/img/{n}"), "", None) else { continue };
        if enc.len() as u64 != size {
            continue;
        }
        if apply(key, 0, &mut enc) {
            out.insert(n.clone(), enc);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctr_ikki_marta_asl_baytni_qaytaradi() {
        let key = "00112233445566778899aabbccddeeff";
        let mut d = b"salom dunyo, bu sinov matni".to_vec();
        let orig = d.clone();
        assert!(apply(key, 0, &mut d));
        assert_ne!(d, orig);
        assert!(apply(key, 0, &mut d));
        assert_eq!(d, orig);
    }

    #[test]
    fn bo_lak_siljishi_mos() {
        let key = "ffeeddccbbaa99887766554433221100";
        let mut whole = vec![7u8; 100];
        apply(key, 0, &mut whole);
        let mut part = vec![7u8; 40];
        apply(key, 60, &mut part);
        assert_eq!(&whole[60..], &part[..]);
    }
}
