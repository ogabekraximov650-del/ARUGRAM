//! Telegram hisobi uchun grammers sessiyasini yaratadi (telefon + kod + 2 bosqichli parol).
//!
//! Muhit: TG_API_ID, TG_API_HASH, TG_PHONE, TG_PASSWORD (ixtiyoriy), CODE_FILE (kod shu faylga yoziladi), SENT_FLAG (kod yuborilgach
//! yaratiladi), OUT (chiqish JSON). Parol va kodlar HECH QACHON chop etilmaydi.
//!
//! Chiqish formati ARUGRAM yadrosidagi `SealedSession` bilan bir xil: {"home_dc":N,"dcs":[DcOption..]} (+ "user": {id, phone, first_name}).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use grammers_client::session::types::{DcOption, PeerId, PeerInfo, UpdateState, UpdatesState};
use grammers_client::session::{BoxFuture, Session, SessionData};
use grammers_client::{tl, Client, InvocationError, SenderPool};

#[derive(Debug)]
struct SessionError;
impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sessiya qulfi buzilgan")
    }
}
impl std::error::Error for SessionError {}

struct MemSession {
    data: Mutex<SessionData>,
}

impl Session for MemSession {
    type Error = SessionError;

    fn home_dc_id(&self) -> Result<i32, SessionError> {
        Ok(self.data.lock().map_err(|_| SessionError)?.home_dc)
    }
    fn set_home_dc_id(&self, dc_id: i32) -> BoxFuture<'_, Result<(), SessionError>> {
        Box::pin(async move {
            self.data.lock().map_err(|_| SessionError)?.home_dc = dc_id;
            Ok(())
        })
    }
    fn dc_option(&self, dc_id: i32) -> Result<Option<DcOption>, SessionError> {
        Ok(self.data.lock().map_err(|_| SessionError)?.dc_options.get(&dc_id).cloned())
    }
    fn set_dc_option(&self, dc_option: &DcOption) -> BoxFuture<'_, Result<(), SessionError>> {
        let dc_option = dc_option.clone();
        Box::pin(async move {
            self.data.lock().map_err(|_| SessionError)?.dc_options.insert(dc_option.id, dc_option);
            Ok(())
        })
    }
    fn peer(&self, peer: PeerId) -> BoxFuture<'_, Result<Option<PeerInfo>, SessionError>> {
        Box::pin(async move { Ok(self.data.lock().map_err(|_| SessionError)?.peer_infos.get(&peer).cloned()) })
    }
    fn cache_peer(&self, peer: &PeerInfo) -> BoxFuture<'_, Result<(), SessionError>> {
        let peer = peer.clone();
        Box::pin(async move {
            self.data.lock().map_err(|_| SessionError)?.peer_infos.insert(peer.id(), peer);
            Ok(())
        })
    }
    fn updates_state(&self) -> BoxFuture<'_, Result<UpdatesState, SessionError>> {
        Box::pin(async move { Ok(self.data.lock().map_err(|_| SessionError)?.updates_state.clone()) })
    }
    fn set_update_state(&self, _update: UpdateState) -> BoxFuture<'_, Result<(), SessionError>> {
        Box::pin(async move { Ok(()) })
    }
}

fn env(k: &str) -> String {
    std::env::var(k).unwrap_or_default()
}

fn die(msg: &str) -> ! {
    eprintln!("XATO: {msg}");
    std::process::exit(1)
}

#[tokio::main]
async fn main() {
    let api_id: i32 = env("TG_API_ID").trim().parse().unwrap_or_else(|_| die("TG_API_ID yo'q"));
    let api_hash = env("TG_API_HASH");
    let phone = env("TG_PHONE");
    let password = env("TG_PASSWORD");
    let code_file = PathBuf::from(env("CODE_FILE"));
    let sent_flag = PathBuf::from(env("SENT_FLAG"));
    let out = PathBuf::from(env("OUT"));
    if api_hash.is_empty() || phone.is_empty() {
        die("TG_API_HASH / TG_PHONE yo'q");
    }

    let session = Arc::new(MemSession { data: Mutex::new(SessionData::default()) });
    let pool = SenderPool::with_configuration(Arc::clone(&session), api_id, grammers_mtsender::ConnectionParams::default());
    let SenderPool { runner, handle, mut updates } = pool;
    let client = Client::new(handle);
    tokio::spawn(runner.run());
    tokio::spawn(async move { while updates.recv().await.is_some() {} });

    let req = tl::functions::auth::SendCode {
        phone_number: phone.clone(),
        api_id,
        api_hash: api_hash.clone(),
        settings: tl::types::CodeSettings {
            allow_flashcall: false,
            current_number: false,
            allow_app_hash: false,
            allow_missed_call: false,
            allow_firebase: false,
            logout_tokens: None,
            token: None,
            app_sandbox: None,
            unknown_number: false,
        }
        .into(),
    };
    let sent = match client.invoke(&req).await {
        Err(InvocationError::Rpc(e)) if e.code == 303 => {
            let dc = e.value.unwrap_or(2) as i32;
            session.set_home_dc_id(dc).await.unwrap_or_else(|_| die("sessiya"));
            client.invoke(&req).await
        }
        other => other,
    };
    let hash = match sent {
        Ok(tl::enums::auth::SentCode::Code(c)) => c.phone_code_hash,
        Ok(_) => die("kutilmagan javob (kod emas)"),
        Err(e) => die(&format!("kod yuborilmadi: {e}")),
    };
    std::fs::write(&sent_flag, b"1").ok();
    println!("KOD_YUBORILDI");

    // Kodni kutish (boshqa jarayon uni Telegram xizmat chatidan o'qib, CODE_FILE ga yozadi).
    let mut code = String::new();
    for _ in 0..180 {
        if let Ok(c) = std::fs::read_to_string(&code_file) {
            let c = c.trim().to_string();
            if !c.is_empty() {
                code = c;
                break;
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    if code.is_empty() {
        die("kod kelmadi");
    }

    let r = client
        .invoke(&tl::functions::auth::SignIn {
            phone_number: phone.clone(),
            phone_code_hash: hash,
            phone_code: Some(code),
            email_verification: None,
        })
        .await;
    let auth = match r {
        Ok(a) => a,
        Err(e) if e.is("SESSION_PASSWORD_NEEDED") => {
            if password.is_empty() {
                die("ikki bosqichli parol kerak (TG_PASSWORD yo'q)");
            }
            check_password(&client, &password).await
        }
        Err(e) => die(&format!("kirish: {e}")),
    };
    let user = match auth {
        tl::enums::auth::Authorization::Authorization(a) => match a.user {
            tl::enums::User::User(u) => serde_json::json!({
                "id": u.id, "phone": u.phone.unwrap_or_default(), "first_name": u.first_name.unwrap_or_default(),
            }),
            _ => serde_json::json!({}),
        },
        _ => die("hisob yo'q (ro'yxatdan o'tish talab qilinadi)"),
    };

    let (home, dcs) = {
        let d = session.data.lock().unwrap_or_else(|_| die("sessiya"));
        (d.home_dc, d.dc_options.values().cloned().collect::<Vec<DcOption>>())
    };
    let body = serde_json::json!({"home_dc": home, "dcs": dcs, "user": user});
    std::fs::write(&out, body.to_string()).unwrap_or_else(|e| die(&format!("yozilmadi: {e}")));
    println!("SESSIYA_YOZILDI dc={home} dcs={}", dcs.len());
}

async fn check_password(client: &Client, password: &str) -> tl::enums::auth::Authorization {
    use grammers_crypto::two_factor_auth::{calculate_2fa, check_p_and_g};
    use tl::enums::PasswordKdfAlgo;
    let get = || async {
        let tl::enums::account::Password::Password(pw) = client
            .invoke(&tl::functions::account::GetPassword {})
            .await
            .unwrap_or_else(|e| die(&format!("parol ma'lumoti: {e}")));
        pw
    };
    let mut pw = get().await;
    let algo = |pw: &tl::types::account::Password| match &pw.current_algo {
        Some(PasswordKdfAlgo::Sha256Sha256Pbkdf2Hmacsha512iter100000Sha256ModPow(a)) => Some(a.clone()),
        _ => None,
    };
    let mut alg = algo(&pw).unwrap_or_else(|| die("parol turi qo'llanmaydi"));
    if !check_p_and_g(&alg.p, &alg.g) {
        pw = get().await;
        alg = algo(&pw).unwrap_or_else(|| die("parol ma'lumoti noto'g'ri"));
    }
    let srp_b = pw.srp_b.clone().unwrap_or_else(|| die("srp_b yo'q"));
    let srp_id = pw.srp_id.unwrap_or_else(|| die("srp_id yo'q"));
    let (m1, g_a) = calculate_2fa(&alg.salt1, &alg.salt2, &alg.p, &alg.g, srp_b, pw.secure_random.clone(), password);
    let req = tl::functions::auth::CheckPassword {
        password: tl::enums::InputCheckPasswordSrp::Srp(tl::types::InputCheckPasswordSrp {
            srp_id,
            a: g_a.to_vec(),
            m1: m1.to_vec(),
        }),
    };
    match client.invoke(&req).await {
        Ok(a) => a,
        Err(e) if e.is("PASSWORD_HASH_INVALID") => die("parol noto'g'ri"),
        Err(e) => die(&format!("parol tekshiruvi: {e}")),
    }
}
