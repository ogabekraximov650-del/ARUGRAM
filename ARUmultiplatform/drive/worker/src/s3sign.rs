//! Cloudflare R2 (S3 mos) uchun VAQTINCHALIK imzolangan havola (AWS Signature V4, so'rov parametrlari bilan).
//! Ilova fayl baytlarini worker'dan O'TKAZMASDAN to'g'ridan-to'g'ri R2 ga yuboradi (worker faqat havola yasaydi).

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn hmac(key: &[u8], data: &str) -> Vec<u8> {
    let mut m = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("hmac kalit");
    m.update(data.as_bytes());
    m.finalize().into_bytes().to_vec()
}

/// RFC 3986: faqat A-Z a-z 0-9 - _ . ~ o'z holicha qoladi (yo'l uchun `/` ham).
fn enc(s: &str, keep_slash: bool) -> String {
    let mut o = String::new();
    for c in s.bytes() {
        if c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.' | b'~') || (keep_slash && c == b'/') {
            o.push(c as char);
        } else {
            o.push_str(&format!("%{c:02X}"));
        }
    }
    o
}

/// `method` — "PUT"/"GET"; `host`/`path` — masalan `<acc>.r2.cloudflarestorage.com` va `/bucket/key`;
/// `extra` — qo'shimcha so'rov parametrlari (`partNumber`, `uploadId`); `amz_date` — `YYYYMMDDTHHMMSSZ`.
pub(crate) fn presign(
    method: &str,
    host: &str,
    path: &str,
    extra: &[(&str, &str)],
    access_key: &str,
    secret: &str,
    region: &str,
    amz_date: &str,
    expires: u64,
) -> String {
    let date = &amz_date[..8];
    let scope = format!("{date}/{region}/s3/aws4_request");
    let mut q: Vec<(String, String)> = vec![
        ("X-Amz-Algorithm".into(), "AWS4-HMAC-SHA256".into()),
        ("X-Amz-Credential".into(), format!("{access_key}/{scope}")),
        ("X-Amz-Date".into(), amz_date.into()),
        ("X-Amz-Expires".into(), expires.to_string()),
        ("X-Amz-SignedHeaders".into(), "host".into()),
    ];
    for (k, v) in extra {
        q.push((k.to_string(), v.to_string()));
    }
    let mut enc_q: Vec<(String, String)> = q.iter().map(|(k, v)| (enc(k, false), enc(v, false))).collect();
    enc_q.sort();
    let canon_q = enc_q.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("&");
    let canon_path = enc(path, true);
    let canonical = format!("{method}\n{canon_path}\n{canon_q}\nhost:{host}\n\nhost\nUNSIGNED-PAYLOAD");
    let to_sign = format!("AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}", hex(&Sha256::digest(canonical.as_bytes())));
    let k_date = hmac(format!("AWS4{secret}").as_bytes(), date);
    let k_region = hmac(&k_date, region);
    let k_service = hmac(&k_region, "s3");
    let k_signing = hmac(&k_service, "aws4_request");
    let sig = hex(&hmac(&k_signing, &to_sign));
    format!("https://{host}{canon_path}?{canon_q}&X-Amz-Signature={sig}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AWS hujjatidagi rasmiy namuna (Signature V4, so'rov parametrlari bilan GET).
    #[test]
    fn aws_namunasi() {
        let url = presign(
            "GET",
            "examplebucket.s3.amazonaws.com",
            "/test.txt",
            &[],
            "AKIAIOSFODNN7EXAMPLE",
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
            "us-east-1",
            "20130524T000000Z",
            86400,
        );
        assert!(
            url.ends_with("X-Amz-Signature=aeeed9bbccd4d02ee5c0109b86d86835f995330da4c265957d157751f604d404"),
            "{url}"
        );
    }

    #[test]
    fn qism_parametrlari_tartiblanadi() {
        let u = presign("PUT", "h.r2.cloudflarestorage.com", "/b/bd/up/x", &[("uploadId", "a/b"), ("partNumber", "3")], "AK", "SK", "auto", "20260101T000000Z", 60);
        assert!(u.contains("partNumber=3&uploadId=a%2Fb"), "{u}");
        assert!(u.contains("/b/bd/up/x?"), "{u}");
    }
}
