//! Ko'p hisob (Kotlin `AccountStore` + `AccountSwitcher`): har bir kirish ALOHIDA hisob, shu telefonga kirilganlari shu yerda turadi.
//! Faqat sessiya tokenlari va profil nusxasi (shifrlangan). Hisoblar soni: obunasi bor hisob bo'lsa 5 tagacha, yo'q bo'lsa 3 tagacha.
//! Hisob almashtirilganda yuborilmagan navbat (`sync_queue.json`) hisob bilan birga chetga olinadi va yangi hisobniki joyiga qaytariladi.

use crate::data::Profile;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::rc::Rc;
use uzcoders::prelude::spawn_bg;

#[derive(Clone)]
pub struct Saved {
    pub id: i64,
    pub session: String,
    pub name: String,
    pub username: String,
    /// Qanday kirilgani: `TG: @user` / `TG: 123456` / Google email.
    pub ident: String,
    pub sub_until: i64,
}

pub const LIMIT_FREE: usize = 3;
pub const LIMIT_SUB: usize = 5;

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn root() -> String {
    uzcoders::android::files_dir()
}

fn file() -> String {
    format!("{}/accounts.json", root())
}

pub fn list() -> Vec<Saved> {
    let Some(t) = crate::secure::read_text(&file(), "accounts") else { return Vec::new() };
    let Ok(Value::Array(a)) = serde_json::from_str::<Value>(&t) else { return Vec::new() };
    a.iter()
        .filter_map(|o| {
            let id = o["id"].as_i64()?;
            let session = o["session"].as_str()?.to_string();
            if id <= 0 || session.is_empty() {
                return None;
            }
            Some(Saved {
                id,
                session,
                name: o["name"].as_str().unwrap_or("").into(),
                username: o["username"].as_str().unwrap_or("").into(),
                ident: o["ident"].as_str().unwrap_or("").into(),
                sub_until: o["sub"].as_i64().unwrap_or(0),
            })
        })
        .collect()
}

fn persist(v: &[Saved]) {
    let a: Vec<Value> = v.iter().map(|s| json!({"id": s.id, "session": s.session, "name": s.name, "username": s.username, "ident": s.ident, "sub": s.sub_until})).collect();
    let _ = crate::secure::write_text(&file(), "accounts", &Value::Array(a).to_string());
}

/// Kirilganda / profil yangilanganda: shu hisob yangilanadi yoki qo'shiladi; "hisob qo'shish" rejimi tugaydi.
pub fn upsert(session: &str, p: &Profile) {
    if p.id <= 0 {
        return;
    }
    let mut v = list();
    let s = Saved { id: p.id, session: session.to_string(), name: p.name.clone(), username: p.username.clone(), ident: p.ident.clone(), sub_until: p.sub_until };
    match v.iter_mut().find(|a| a.id == p.id) {
        Some(o) => *o = s,
        None => v.push(s),
    }
    persist(&v);
    set_add_return(0);
}

pub fn remove(id: i64) {
    let mut v = list();
    v.retain(|a| a.id != id);
    persist(&v);
    let _ = std::fs::remove_dir_all(format!("{}/accs/{id}", root()));
}

/// Ulangan hisoblardan birortasida faol obuna bo'lsa 5, bo'lmasa 3.
pub fn limit() -> usize {
    let now = now_ms();
    if list().iter().any(|a| a.sub_until > now) { LIMIT_SUB } else { LIMIT_FREE }
}

pub fn can_add() -> bool {
    list().len() < limit()
}

fn add_file() -> String {
    format!("{}/accounts_add_return", root())
}

/// "Hisob qo'shish" rejimida qaytish uchun hisob (0 — rejim yo'q).
pub fn add_return() -> i64 {
    std::fs::read_to_string(add_file()).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0)
}

fn set_add_return(id: i64) {
    if id > 0 {
        let _ = std::fs::write(add_file(), id.to_string());
    } else {
        let _ = std::fs::remove_file(add_file());
    }
}

const PERSONAL: [&str; 1] = ["sync_queue.json"];

/// `to_stash == true`: faol joydan `accs/<id>/` ga; `false`: `accs/<id>/` dan faol joyga.
fn move_files(to_stash: bool, id: i64) {
    let dir = format!("{}/accs/{id}", root());
    let _ = std::fs::create_dir_all(&dir);
    for n in PERSONAL {
        let (act, st) = (format!("{}/{n}", root()), format!("{dir}/{n}"));
        let (src, dst) = if to_stash { (act, st) } else { (st, act) };
        if !std::path::Path::new(&src).exists() {
            continue;
        }
        let _ = std::fs::remove_file(&dst);
        if std::fs::rename(&src, &dst).is_err() {
            let _ = std::fs::copy(&src, &dst);
            let _ = std::fs::remove_file(&src);
        }
    }
}

/// UI qatlami ulagichlari (`lib.rs` o'rnatadi): sessiya bilan bosh sahifani ochish va "hisob qo'shish" (kirish ekrani).
#[derive(Clone)]
pub struct Hooks {
    pub activate: Rc<dyn Fn(String)>,
    pub add: Rc<dyn Fn()>,
}

thread_local! {
    static HOOKS: RefCell<Option<Hooks>> = const { RefCell::new(None) };
}

pub fn set_hooks(h: Hooks) {
    HOOKS.with(|x| *x.borrow_mut() = Some(h));
}

fn hooks() -> Option<Hooks> {
    HOOKS.with(|x| x.borrow().clone())
}

/// Boshqa hisobga o'tadi (UI oqimidan): avval navbat serverga yuboriladi, so'ng fayllar almashadi va bosh sahifa qayta quriladi.
pub fn switch_to(from: i64, to: i64) {
    let Some(t) = list().into_iter().find(|a| a.id == to) else { return };
    spawn_bg(crate::sync::flush, move |_| {
        if from > 0 {
            move_files(true, from);
        }
        move_files(false, to);
        crate::api::save_session(&t.session);
        if let Some(h) = hooks() {
            (h.activate)(t.session.clone());
        }
    });
}

/// "Account qo'shish": hozirgisi saqlanadi, mehmon holatida kirish ekrani ochiladi.
pub fn begin_add(from: i64) {
    spawn_bg(crate::sync::flush, move |_| {
        if from > 0 {
            move_files(true, from);
            set_add_return(from);
        }
        crate::api::clear_session();
        if let Some(h) = hooks() {
            (h.add)();
        }
    });
}

/// Kirish ekranidan orqaga: "hisob qo'shish" bekor qilinadi va avvalgi hisobga qaytiladi (`false` — rejim yo'q).
pub fn cancel_add() -> bool {
    let back = add_return();
    let Some(t) = list().into_iter().find(|a| a.id == back) else { return false };
    if back <= 0 {
        return false;
    }
    move_files(false, back);
    crate::api::save_session(&t.session);
    set_add_return(0);
    if let Some(h) = hooks() {
        (h.activate)(t.session.clone());
    }
    true
}

/// Hisobdan chiqilgach: shu hisob ro'yxatdan olinadi; boshqa hisob qolgan bo'lsa unga o'tiladi (`true`).
pub fn after_logout(session: &str) -> bool {
    if let Some(a) = list().into_iter().find(|a| a.session == session) {
        remove(a.id);
    }
    let Some(next) = list().into_iter().next() else { return false };
    move_files(false, next.id);
    crate::api::save_session(&next.session);
    if let Some(h) = hooks() {
        (h.activate)(next.session.clone());
    }
    true
}
