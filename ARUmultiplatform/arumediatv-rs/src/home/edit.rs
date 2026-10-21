//! Profilni tahrirlash (Kotlin `EditProfileScreen`): ism va username; username bo'shligi yozish paytida tekshiriladi.

use super::*;
use crate::api;
use serde_json::{json, Value};
use std::cell::RefCell;

pub struct EditPage {
    name: State<String>,
    username: State<String>,
    original_username: String,
    /// 0 — noma'lum, 1 — bo'sh, 2 — band/noto'g'ri, 3 — tekshirilmoqda.
    avail: State<u8>,
    checked: RefCell<String>,
    saving: State<bool>,
    msg: State<String>,
}

fn name_problem(s: &str) -> Option<&'static str> {
    let n = s.trim();
    if n.is_empty() {
        Some("Ism bo'sh bo'lmasin")
    } else if n.chars().count() > 20 {
        Some("Ism eng ko'pi 20 ta belgi bo'lishi mumkin")
    } else {
        None
    }
}

fn username_problem(u: &str) -> Option<&'static str> {
    let n = u.chars().count();
    if n < 3 {
        Some("Username kamida 3 ta belgidan iborat bo'lsin")
    } else if n > 15 {
        Some("Username eng ko'pi 15 ta belgi bo'lishi mumkin")
    } else if !u.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        Some("Faqat harf, raqam va pastki chiziq (_) ishlatiladi")
    } else {
        None
    }
}

impl Home {
    pub(super) fn open_edit(self: &Rc<Self>) {
        let pr = self.profile.get();
        let first = pr.name.split_whitespace().next().unwrap_or("").to_string();
        let p = Rc::new(EditPage {
            name: State::new(first),
            username: State::new(pr.username.clone()),
            original_username: pr.username.clone(),
            avail: State::new(0),
            checked: RefCell::new(pr.username.clone()),
            saving: State::new(false),
            msg: State::new(String::new()),
        });
        self.push(pages::Page::Edit(p));
    }

    pub(super) fn edit_page(self: &Rc<Self>, p: &Rc<EditPage>) -> Element {
        let name = p.name.get();
        let user = p.username.get();
        let np = name_problem(&name);
        let up = if user.is_empty() { None } else { username_problem(&user) };
        // Username bo'shligini tekshirish: so'rov tugamaguncha keyingisi ketmaydi (tabiiy debounce).
        if !user.is_empty() && user != p.original_username && up.is_none() && p.avail.get() != 3 && *p.checked.borrow() != user {
            *p.checked.borrow_mut() = user.clone();
            p.avail.set(3);
            let (p2, session, u2) = (p.clone(), self.session.clone(), user.clone());
            spawn_bg(
                move || api::get_json("/api/auth/username-check", &format!("?u={u2}"), Some(&session)),
                move |r| {
                    let v = match r {
                        Ok(j) if j["valid"].as_bool().unwrap_or(false) => {
                            if j["available"].as_bool().unwrap_or(false) { 1 } else { 2 }
                        }
                        Ok(_) => 2,
                        Err(_) => 0,
                    };
                    // Yozuv tekshiruv paytida o'zgargan bo'lsa natija eskirgan: keyingi kadrda qayta tekshiriladi.
                    if *p2.checked.borrow() == p2.username.get() {
                        p2.avail.set(v);
                    } else {
                        p2.avail.set(0);
                    }
                },
            );
        } else if user == p.original_username && p.avail.get() == 3 {
            p.avail.set(0);
        }
        let avail = p.avail.get();
        let checking = avail == 3;
        let user_err: Option<String> = up.map(String::from).or(if avail == 2 { Some("Bu username band yoki noto'g'ri".into()) } else { None });
        let user_hint = if checking {
            "Tekshirilmoqda...".to_string()
        } else if !user.is_empty() && user == p.original_username {
            "Bu — hozirgi username'ingiz.".to_string()
        } else if avail == 1 {
            "Bu username bo'sh — olsangiz bo'ladi".to_string()
        } else {
            "3-15 ta belgi. Faqat harf, raqam va _ ishlatiladi.".to_string()
        };
        let styled = |f: Element, label: &str| {
            f.color(Color::WHITE)
                .size(16.0)
                .line_height(24.0)
                .padding_each(16.0, 26.0, 16.0, 10.0)
                .label_inside()
                .glass(Color::rgba(255, 255, 255, 22))
                .hint_color(Color::rgba(255, 255, 255, 150))
                .accent(ACCENT2)
                .border(1.0, Color::TRANSPARENT)
                .radius(16.0)
                .label(label)
        };
        let can_save = np.is_none() && up.is_none() && avail != 2 && !checking && !p.saving.get();
        let (pp, me) = (p.clone(), self.clone());
        let save = column(vec![if p.saving.get() { spinner(22.0, 2.0, Color::WHITE) } else { tx("Saqlash", 16.0, Color::WHITE).bold() }])
            .center()
            .padding_xy(0.0, 16.0)
            .radius(28.0)
            .gradient(if can_save { ACCENT } else { ACCENT.with_opacity(0.4) }, if can_save { ACCENT2 } else { ACCENT2.with_opacity(0.4) })
            .on_click(move || {
                if !can_save {
                    return;
                }
                pp.saving.set(true);
                pp.msg.set(String::new());
                let body = json!({"first_name": pp.name.get().trim(), "username": pp.username.get().trim()});
                let (p2, me2, session) = (pp.clone(), me.clone(), me.session.clone());
                spawn_bg(
                    move || {
                        let (code, text) = api::request("POST", "/api/auth/profile", "", Some(&body), Some(&session)).map_err(|_| "Tarmoq xatosi — qaytadan urinib ko'ring".to_string())?;
                        if code >= 400 {
                            let j: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                            return Err(j["error"].as_str().map(String::from).unwrap_or_else(|| "Saqlab bo'lmadi".into()));
                        }
                        Ok(())
                    },
                    move |r: Result<(), String>| {
                        p2.saving.set(false);
                        match r {
                            Ok(()) => {
                                p2.msg.set("Saqlandi".into());
                                me2.load_profile();
                            }
                            Err(e) => p2.msg.set(e),
                        }
                    },
                );
            });
        let note = |t: &str, err: bool| tx(t, 12.0, if err { ERROR } else { Color::rgba(255, 255, 255, 140) }).padding_each(6.0, 6.0, 6.0, 0.0);
        let mut kids = vec![
            styled(field(&p.name), "Ism"),
            note(np.unwrap_or("Eng ko'pi 20 ta belgi. Emoji va istalgan belgi ishlatsangiz bo'ladi."), np.is_some()),
            gap_box(0.0, 14.0),
            styled(field(&p.username), "Username"),
            match &user_err {
                Some(e) => note(e, true),
                None => note(&user_hint, false),
            },
            gap_box(0.0, 20.0),
        ];
        let m = p.msg.get();
        if !m.is_empty() {
            kids.push(tx(m.clone(), 13.0, if m == "Saqlandi" { Color::rgb(0x4A, 0xDE, 0x80) } else { ERROR }));
            kids.push(gap_box(0.0, 12.0));
        }
        kids.push(save);
        self.scaffold("Profilni tahrirlash", scroll_column(&self.settings_scroll, kids).padding_each(20.0, 8.0, 20.0, 0.0))
    }
}
