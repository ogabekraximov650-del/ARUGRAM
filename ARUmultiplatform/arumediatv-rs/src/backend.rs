//! Telegram orqali kirish backend'i. Hozircha SINOV rejimi: haqiqiy Telegram yadrosi (Rust `grammers`) keyingi bosqichda ulanadi.

pub enum CodeResult {
    Sent(&'static str),
    Error(String),
}

pub enum SignIn {
    NeedPassword(&'static str),
    Done,
    Error(String),
}

pub trait Backend {
    fn request_code(&self, phone: &str) -> CodeResult;
    fn sign_in(&self, code: &str) -> SignIn;
    fn check_password(&self, password: &str) -> Result<(), String>;
}

/// Sinov rejimi: tarmoqsiz. Kod `12345` bo'lsa ikki bosqichli parol so'raladi.
pub struct StubBackend;

impl Backend for StubBackend {
    fn request_code(&self, _phone: &str) -> CodeResult {
        CodeResult::Sent("Kod yuborildi (sinov rejimi: 12345 kiriting)")
    }

    fn sign_in(&self, code: &str) -> SignIn {
        if code == "12345" {
            SignIn::NeedPassword("sinov")
        } else {
            SignIn::Error("Kod noto'g'ri".into())
        }
    }

    fn check_password(&self, _password: &str) -> Result<(), String> {
        Err("Sinov rejimi: Telegram yadrosi hali ulanmagan".into())
    }
}
