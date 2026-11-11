// ══════════════════════════════════════════════════════════════
//  PANEL BOT ADMINLARI — "👥 Adminlar" bo'limi (faqat ASOSIY admin)
// ══════════════════════════════════════════════════════════════
//
// TALAB (foydalanuvchi): asosiy admin ID'si panel botga boshqa adminlarni shu bo'lim orqali qo'shsin/olib tashlasin.
// Adminlar ro'yxati `app_config.encbot_admins` da (vergul bilan Telegram ID'lar). Asosiy admin (`ADMIN_TELEGRAM_ID`) har doim
// admin va ro'yxatdan o'chirilmaydi. Ro'yxat hali yozilmagan bo'lsa — boshlang'ich qiymat `ENCBOT_DEFAULT_ADMINS`.
// Qo'shilgan admin panel botni ishlata oladi, lekin shu bo'limni (adminlarni boshqarish) FAQAT asosiy admin ko'radi.

use super::*;

pub(crate) const BTN: &str = "\u{1F465} Adminlar";
const BTN_ADD: &str = "\u{2795} Admin qo'shish";
const BTN_DEL: &str = "\u{2796} Adminni olib tashlash";
const BTN_CANCEL: &str = "\u{274C} Bekor qilish";
/// Ro'yxat hali yozilmagan bo'lsa ishlatiladigan qo'shimcha adminlar (avval kodda yozilgan).
const ENCBOT_DEFAULT_ADMINS: [i64; 1] = [8_827_179_336];

thread_local! {
    static CACHE: std::cell::RefCell<(i64, Vec<i64>)> = const { std::cell::RefCell::new((0, Vec::new())) };
}

fn parse(csv: &str) -> Vec<i64> {
    csv.split(',').filter_map(|x| x.trim().parse::<i64>().ok()).filter(|x| *x > 0).collect()
}

/// Qo'shimcha adminlar (asosiy adminsiz). 20 soniya xotirada.
pub(crate) async fn extra_admins(env: &Env) -> Vec<i64> {
    let now = now_ms();
    let (at, v) = CACHE.with(|c| c.borrow().clone());
    if now - at < 20_000 && at > 0 {
        return v;
    }
    let list = match config_get(env, "encbot_admins").await {
        Some(csv) => parse(&csv),
        None => ENCBOT_DEFAULT_ADMINS.to_vec(),
    };
    CACHE.with(|c| *c.borrow_mut() = (now, list.clone()));
    list
}

/// Panel botni ishlatish huquqi: asosiy admin yoki qo'shilgan admin.
pub(crate) async fn is_admin(env: &Env, id: i64) -> bool {
    id == ADMIN_TELEGRAM_ID || (id > 0 && extra_admins(env).await.contains(&id))
}

async fn save(env: &Env, list: &[i64]) {
    let csv = list.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",");
    config_put(env, "encbot_admins", if csv.is_empty() { "-" } else { &csv }).await;
    CACHE.with(|c| *c.borrow_mut() = (now_ms(), list.to_vec()));
}

fn keyboard() -> Value {
    encbot_keyboard(vec![
        vec![BTN_ADD.to_string(), BTN_DEL.to_string()],
        vec![postbot::BTN_HOME.to_string()],
    ])
}

async fn show(env: &Env, chat: i64, note: &str) {
    let list = extra_admins(env).await;
    let rows = if list.is_empty() {
        "\u{2014} qo'shilgan admin yo'q".to_string()
    } else {
        list.iter().map(|id| format!("\u{2022} <code>{id}</code>")).collect::<Vec<_>>().join("\n")
    };
    encbot_send(env, chat, &format!(
        "\u{1F465} <b>Panel bot adminlari</b>\n\n\u{1F451} Asosiy admin: <code>{ADMIN_TELEGRAM_ID}</code>\n{rows}\n\n{note}\n\
         Qo'shilgan admin panel botni to'liq ishlata oladi, lekin adminlarni faqat siz boshqarasiz."),
        Some(keyboard())).await;
}

/// "👥 Adminlar" bosildi (faqat asosiy admin).
pub(crate) async fn start(env: &Env, chat: i64) {
    if chat != ADMIN_TELEGRAM_ID {
        return;
    }
    config_put(env, "encbot_mode", "adm").await;
    config_put(env, "adm_state", "").await;
    show(env, chat, "").await;
}

/// Adminlar rejimidagi xabar. `true` — xabar shu yerda qayta ishlandi.
pub(crate) async fn on_message(env: &Env, msg: &Value, chat: i64) -> bool {
    if chat != ADMIN_TELEGRAM_ID {
        return false;
    }
    let text = msg["text"].as_str().unwrap_or("").trim().to_string();
    let state = config_get(env, "adm_state").await.unwrap_or_default();
    if text == BTN_CANCEL {
        config_put(env, "adm_state", "").await;
        show(env, chat, "\u{274C} Bekor qilindi.").await;
        return true;
    }
    if text == BTN_ADD || text == BTN_DEL {
        config_put(env, "adm_state", if text == BTN_ADD { "add" } else { "del" }).await;
        encbot_send(env, chat, if text == BTN_ADD {
            "\u{2795} Qo'shiladigan odamning <b>Telegram ID</b> raqamini yuboring (masalan <code>8827179336</code>).\n\
             ID ni bilmasa, u @userinfobot ga /start bosib bilib oladi."
        } else {
            "\u{2796} Olib tashlanadigan adminning <b>Telegram ID</b> raqamini yuboring."
        }, Some(encbot_keyboard(vec![vec![BTN_CANCEL.to_string()], vec![postbot::BTN_HOME.to_string()]]))).await;
        return true;
    }
    if state != "add" && state != "del" {
        show(env, chat, "Pastdagi tugmalardan birini tanlang.").await;
        return true;
    }
    let id: i64 = text.chars().filter(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(0);
    if id <= 0 || id.to_string().len() < 5 {
        encbot_send(env, chat, "\u{26A0}\u{FE0F} ID noto'g'ri. Faqat raqam yuboring (masalan <code>8827179336</code>).", None).await;
        return true;
    }
    let mut list = extra_admins(env).await;
    config_put(env, "adm_state", "").await;
    if state == "add" {
        if id == ADMIN_TELEGRAM_ID || list.contains(&id) {
            show(env, chat, "\u{2139}\u{FE0F} Bu ID allaqachon admin.").await;
        } else {
            list.push(id);
            save(env, &list).await;
            show(env, chat, &format!("\u{2705} <code>{id}</code> admin qilindi. U botda /start bosishi kerak.")).await;
        }
    } else if id == ADMIN_TELEGRAM_ID {
        show(env, chat, "\u{26D4} Asosiy adminni olib tashlab bo'lmaydi.").await;
    } else if let Some(p) = list.iter().position(|x| *x == id) {
        list.remove(p);
        save(env, &list).await;
        show(env, chat, &format!("\u{2705} <code>{id}</code> adminlikdan olindi.")).await;
    } else {
        show(env, chat, "\u{2139}\u{FE0F} Bu ID ro'yxatda yo'q.").await;
    }
    true
}
