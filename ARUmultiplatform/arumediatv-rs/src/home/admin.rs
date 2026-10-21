//! Admin paneli (Kotlin `AdminHubScreen` va bo'limlar): suhbatlar, foydalanuvchilar, shikoyatlar, umumiy statistika.
//! Har bir amal SERVERDA tekshiriladi (`admin_only`); ilova faqat so'rov yuboradi.

use super::*;
use crate::api;
use crate::data::{fmt_ymd, format_bytes, format_count, format_hours};
use serde_json::{json, Value};

const AD_PEOPLE: &str = "M16,11c1.66,0 2.99,-1.34 2.99,-3S17.66,5 16,5c-1.66,0 -3,1.34 -3,3s1.34,3 3,3zM8,11c1.66,0 2.99,-1.34 2.99,-3S9.66,5 8,5C6.34,5 5,6.34 5,8s1.34,3 3,3zM8,13c-2.33,0 -7,1.17 -7,3.5V19h14v-2.5c0,-2.33 -4.67,-3.5 -7,-3.5zM16,13c-0.29,0 -0.62,0.02 -0.97,0.05 1.16,0.84 1.97,1.97 1.97,3.45V19h6v-2.5c0,-2.33 -4.67,-3.5 -7,-3.5z";
const AD_CHAT: &str = "M21.99,4c0,-1.1 -0.89,-2 -1.99,-2H4c-1.1,0 -2,0.9 -2,2v12c0,1.1 0.9,2 2,2h14l4,4 -0.01,-18z";
const AD_FLAG: &str = "M14.4,6L14,4H5v17h2v-7h5.6l0.4,2h7V6z";
const AD_INSIGHTS: &str = "M21,8c-1.45,0 -2.26,1.44 -1.93,2.51l-3.55,3.56c-0.3,-0.09 -0.74,-0.09 -1.04,0l-2.55,-2.55C12.27,10.45 11.46,9 10,9c-1.45,0 -2.27,1.44 -1.93,2.52l-4.56,4.55C2.44,15.74 1,16.55 1,18c0,1.1 0.9,2 2,2 1.45,0 2.26,-1.44 1.93,-2.51l4.55,-4.56c0.3,0.09 0.74,0.09 1.04,0l2.55,2.55C12.73,16.55 13.54,18 15,18c1.45,0 2.27,-1.44 1.93,-2.52l3.56,-3.55c1.07,0.33 2.51,-0.48 2.51,-1.93 0,-1.1 -0.9,-2 -2,-2z";
const AD_MOVIE: &str = "M18,4l2,4h-3l-2,-4h-2l2,4h-3l-2,-4H8l2,4H7L5,4H4c-1.1,0 -1.99,0.9 -1.99,2L2,18c0,1.1 0.9,2 2,2h16c1.1,0 2,-0.9 2,-2V4h-4z";
const AD_SMILE: &str = "M11.99,2C6.47,2 2,6.48 2,12s4.47,10 9.99,10C17.52,22 22,17.52 22,12S17.52,2 11.99,2zM12,20c-4.42,0 -8,-3.58 -8,-8s3.58,-8 8,-8 8,3.58 8,8 -3.58,8 -8,8z";
const AD_CAMPAIGN: &str = "M18,11v2h4v-2h-4zM16,17.61c0.96,0.71 2.21,1.65 3.2,2.39 0.4,-0.53 0.8,-1.07 1.2,-1.6 -0.99,-0.74 -2.24,-1.68 -3.2,-2.4 -0.4,0.54 -0.8,1.08 -1.2,1.61zM20.4,5.6c-0.4,-0.53 -0.8,-1.07 -1.2,-1.6 -0.99,0.74 -2.24,1.68 -3.2,2.4 0.4,0.53 0.8,1.07 1.2,1.6 0.96,-0.72 2.21,-1.65 3.2,-2.4zM4,9c-1.1,0 -2,0.9 -2,2v2c0,1.1 0.9,2 2,2h1v4h2v-4h1l5,3V6L8,9H4z";
const AD_SECURITY: &str = "M12,1L3,5v6c0,5.55 3.84,10.74 9,12 5.16,-1.26 9,-6.45 9,-12V5l-9,-4z";
const AD_CHEVRON: &str = "M10,6L8.59,7.41 13.17,12l-4.58,4.59L10,18l6,-6z";
const AD_SEARCH: &str = "M15.5,14h-0.79l-0.28,-0.27C15.41,12.59 16,11.11 16,9.5 16,5.91 13.09,3 9.5,3S3,5.91 3,9.5 5.91,16 9.5,16c1.61,0 3.09,-0.59 4.23,-1.57l0.27,0.28v0.79l5,4.99L20.49,19l-4.99,-5zM9.5,14C7.01,14 5,11.99 5,9.5S7.01,5 9.5,5 14,7.01 14,9.5 11.99,14 9.5,14z";

/// Admin bo'limlari: (nom, izoh, belgi).
const SECTIONS: [(&str, &str, &str); 9] = [
    ("Animelarni boshqarish", "Qo'shish, tahrirlash, o'chirish", AD_MOVIE),
    ("Foydalanuvchilar", "Balans, obuna, bloklash", AD_PEOPLE),
    ("Suhbatlar", "Foydalanuvchilar yozishmalari", AD_CHAT),
    ("Shikoyatlar", "Izohlarga kelgan shikoyatlar", AD_FLAG),
    ("Emoji, GIF va stikerlar", "Yuklangan rasmlarni ko'rib chiqish", AD_SMILE),
    ("Majburiy obunalar", "Kanallar, limit va statistika", AD_CAMPAIGN),
    ("Umumiy statistika", "Kontent, foydalanuvchilar, ko'rishlar, trafik", AD_INSIGHTS),
    ("Kodlash navbati", "Kodlanayotgan va navbatdagi qismlar, log", AD_MOVIE),
    ("Ilova va xavfsizlik", "Eng past versiya, ulanish kaliti", AD_SECURITY),
];

fn num(v: &Value) -> i64 {
    v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)).or_else(|| v.as_str().and_then(|s| s.parse().ok())).unwrap_or(0)
}

fn err_of(code: u16, body: &str, fallback: &str) -> String {
    let j: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    j["error"].as_str().map(String::from).unwrap_or_else(|| if code == 403 { "Bu bo'lim faqat admin uchun".into() } else { format!("{fallback} ({code})") })
}

fn ago(ms: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    let s = ((now - ms) / 1000).max(0);
    if s < 60 { "hozir".into() } else if s < 3600 { format!("{} daq", s / 60) } else if s < 86400 { format!("{} soat", s / 3600) } else { format!("{} kun", s / 86400) }
}

// ── Foydalanuvchilar ──

#[derive(Clone)]
pub struct AUser {
    id: i64,
    username: String,
    name: String,
    balance: i64,
    banned: bool,
    ban_reason: String,
    last_login_at: i64,
    sub_until: i64,
}

impl AUser {
    fn from(o: &Value) -> AUser {
        let first = o["first_name"].as_str().unwrap_or("").trim();
        let last = o["last_name"].as_str().unwrap_or("").trim();
        let username = o["username"].as_str().unwrap_or("").to_string();
        let id = num(&o["id"]);
        let name = match format!("{first} {last}").trim() {
            n if !n.is_empty() => n.to_string(),
            _ if !username.is_empty() => format!("@{username}"),
            _ => format!("Foydalanuvchi {id}"),
        };
        AUser { id, username, name, balance: num(&o["balance"]), banned: o["banned"].as_bool().unwrap_or(false), ban_reason: o["ban_reason"].as_str().unwrap_or("").into(), last_login_at: num(&o["last_login_at"]), sub_until: num(&o["sub_until"]) }
    }
    fn sub_days_left(&self) -> i64 {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
        let ms = self.sub_until - now;
        if ms <= 0 { 0 } else { (ms as f64 / 86_400_000.0).ceil() as i64 }
    }
}

#[derive(Clone)]
enum Modal {
    None,
    Menu(AUser),
    Amount(AUser),
    Days(AUser),
    Ban(AUser),
    Unban(AUser),
}

pub struct UsersPage {
    items: State<Rc<Vec<AUser>>>,
    query: State<String>,
    sort: State<u8>,
    total: State<i64>,
    page: std::cell::Cell<i64>,
    has_more: State<bool>,
    loading: State<bool>,
    error: State<String>,
    last_q: std::cell::RefCell<String>,
    modal: State<Modal>,
    num: State<String>,
    reason: State<String>,
    ban_days: State<i64>,
    msg: State<String>,
    scroll: ScrollState,
}

pub struct ListPage {
    items: State<Rc<Vec<Value>>>,
    loaded: State<bool>,
    error: State<String>,
    has_more: State<bool>,
    page: std::cell::Cell<i64>,
    scroll: ScrollState,
    confirm: State<Option<Value>>,
}

pub struct StatsPage {
    data: State<Option<Rc<Value>>>,
    error: State<String>,
    open: State<String>,
    range: State<String>,
    series: State<Option<Rc<Vec<(String, i64)>>>>,
    scroll: ScrollState,
}

impl Home {
    pub(super) fn open_admin_hub(self: &Rc<Self>) {
        self.push(pages::Page::AdminHub);
    }

    pub(super) fn admin_hub_page(self: &Rc<Self>) -> Element {
        let mut kids: Vec<Element> = Vec::new();
        for (i, (title, hint, ic)) in SECTIONS.iter().enumerate() {
            let me = self.clone();
            kids.push(
                Self::pglass(
                    row(vec![
                        column(vec![icon(ic, 26.0).color(ACCENT2)]).center().width(52.0).height(52.0).radius(16.0).bg(ACCENT.with_opacity(0.2)),
                        column(vec![tx(*title, 16.0, Color::WHITE).bold(), tx(*hint, 12.0, Color::rgba(255, 255, 255, 140))]).weight(1.0),
                        icon(AD_CHEVRON, 24.0).color(Color::rgba(255, 255, 255, 96)),
                    ])
                    .gap(16.0)
                    .align(Align::Center)
                    .padding_xy(18.0, 14.0),
                )
                .radius(26.0)
                .margin_each(0.0, 0.0, 0.0, 12.0)
                .on_click(move || me.open_admin_section(i)),
            );
        }
        kids.push(gap_box(0.0, 32.0));
        self.scaffold("Admin paneli", scroll_column(&self.settings_scroll, kids).padding_each(16.0, 4.0, 16.0, 0.0))
    }

    fn open_admin_section(self: &Rc<Self>, i: usize) {
        match i {
            1 => self.open_admin_users(),
            2 => self.open_admin_list(0),
            3 => self.open_admin_list(1),
            4 => self.open_admin_packs(),
            5 => self.open_admin_channels(),
            6 => self.open_admin_stats(),
            8 => self.open_admin_app(),
            _ => {
                self.dlg_msg.set(format!("«{}» bo'limi Rust ilovada hali tayyor emas — Kotlin ilovada mavjud.", SECTIONS[i].0));
                self.dlg.set(5);
            }
        }
    }

    // ── Foydalanuvchilar ──

    fn open_admin_users(self: &Rc<Self>) {
        let p = Rc::new(UsersPage {
            items: State::new(Rc::new(Vec::new())),
            query: State::new(String::new()),
            sort: State::new(0),
            total: State::new(0),
            page: std::cell::Cell::new(0),
            has_more: State::new(false),
            loading: State::new(false),
            error: State::new(String::new()),
            last_q: std::cell::RefCell::new(String::new()),
            modal: State::new(Modal::None),
            num: State::new(String::new()),
            reason: State::new(String::new()),
            ban_days: State::new(0),
            msg: State::new(String::new()),
            scroll: ScrollState::new(),
        });
        self.users_load(&p, 0);
        self.push(pages::Page::AdminUsers(p));
    }

    fn users_load(self: &Rc<Self>, p: &Rc<UsersPage>, page: i64) {
        if p.loading.get() {
            return;
        }
        p.loading.set(true);
        let (p2, session) = (p.clone(), self.session.clone());
        let q = p.query.get().trim().to_string();
        *p.last_q.borrow_mut() = q.clone();
        let sort = if p.sort.get() == 0 { "new" } else { "online" };
        spawn_bg(
            move || {
                let query = format!("?sort={sort}&page={page}&q={}", crate::backend::urlenc(&q));
                let (code, body) = api::request("GET", "/api/admin/users", &query, None, Some(&session))?;
                if code >= 400 {
                    return Err(err_of(code, &body, "Yuklanmadi"));
                }
                serde_json::from_str::<Value>(&body).map_err(|_| "Javob noto'g'ri".to_string())
            },
            move |r: Result<Value, String>| {
                p2.loading.set(false);
                match r {
                    Ok(j) => {
                        let rows: Vec<AUser> = j["items"].as_array().map(|a| a.iter().map(AUser::from).collect()).unwrap_or_default();
                        let mut all: Vec<AUser> = if page == 0 { Vec::new() } else { (*p2.items.get()).clone() };
                        for r in rows {
                            if !all.iter().any(|x| x.id == r.id) {
                                all.push(r);
                            }
                        }
                        p2.items.set(Rc::new(all));
                        p2.total.set(num(&j["total"]));
                        p2.has_more.set(j["has_more"].as_bool().unwrap_or(false));
                        p2.page.set(page);
                        p2.error.set(String::new());
                    }
                    Err(e) => p2.error.set(e),
                }
            },
        );
    }

    fn user_act(self: &Rc<Self>, p: &Rc<UsersPage>, uid: i64, action: &'static str, amount: i64, days: i64, reason: String, done: String) {
        let mut b = json!({"action": action, "days": days});
        if amount != 0 {
            b["amount"] = json!(amount);
        }
        if !reason.is_empty() {
            b["reason"] = json!(reason);
        }
        let (p2, session) = (p.clone(), self.session.clone());
        spawn_bg(
            move || {
                let (code, body) = api::request("POST", &format!("/api/admin/user/{uid}"), "", Some(&b), Some(&session))?;
                if code >= 400 {
                    return Err(err_of(code, &body, "Bajarilmadi"));
                }
                serde_json::from_str::<Value>(&body).map_err(|_| "Javob noto'g'ri".to_string())
            },
            move |r: Result<Value, String>| match r {
                Ok(j) => {
                    let v: Vec<AUser> = p2
                        .items
                        .get()
                        .iter()
                        .map(|u| {
                            if u.id != uid {
                                return u.clone();
                            }
                            let mut u = u.clone();
                            u.balance = j["balance"].as_i64().unwrap_or(u.balance);
                            if j.get("banned").is_some() {
                                u.banned = j["banned"].as_bool().unwrap_or(false);
                                u.ban_reason = j["ban_reason"].as_str().unwrap_or("").into();
                            }
                            if j.get("subscription_until").is_some() {
                                u.sub_until = num(&j["subscription_until"]);
                            }
                            u
                        })
                        .collect();
                    p2.items.set(Rc::new(v));
                    p2.msg.set(done);
                }
                Err(e) => p2.msg.set(e),
            },
        );
    }

    pub(super) fn admin_users_page(self: &Rc<Self>, p: &Rc<UsersPage>) -> Element {
        // Izlash: yozish tugagach (so'rov yo'q paytda) yangi so'rov ketadi.
        if !p.loading.get() && *p.last_q.borrow() != p.query.get().trim() {
            self.users_load(p, 0);
        }
        let search = Self::pglass(
            row(vec![
                icon(AD_SEARCH, 22.0).color(Color::rgba(255, 255, 255, 190)),
                field(&p.query).color(Color::WHITE).size(16.0).hint("ID yoki username").hint_color(Color::rgba(255, 255, 255, 120)).accent(ACCENT2).padding_xy(0.0, 10.0).no_border().weight(1.0),
            ])
            .gap(10.0)
            .align(Align::Center)
            .padding_xy(16.0, 4.0),
        )
        .radius(26.0);
        let sorts: Vec<Element> = ["Yangi qo'shilgan", "Oxirgi onlayn"]
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let (ps, pl, me) = (p.sort.clone(), p.clone(), self.clone());
                let on = p.sort.get() as usize == i;
                let e = column(vec![tx(*l, 12.0, if on { Color::WHITE } else { Color::rgba(255, 255, 255, 170) }).bold()]).center().padding_xy(14.0, 8.0).radius(18.0).on_click(move || {
                    ps.set(i as u8);
                    me.users_load(&pl, 0);
                });
                if on { e.gradient(ACCENT, ACCENT2) } else { e.bg(Color::rgba(255, 255, 255, 18)) }
            })
            .collect();
        let items = p.items.get();
        let mut head = vec![search, gap_box(0.0, 10.0), row(sorts.into_iter().chain([spacer(), tx(format!("{} ta", p.total.get()), 12.0, Color::rgba(255, 255, 255, 130))]).collect()).gap(8.0).align(Align::Center), gap_box(0.0, 10.0)];
        if !p.msg.get().is_empty() {
            let pm = p.clone();
            head.push(tx(p.msg.get(), 13.0, Color::rgba(255, 255, 255, 230)).bg(Color::rgba(0, 0, 0, 120)).radius(14.0).padding_xy(14.0, 8.0).margin_each(0.0, 0.0, 0.0, 8.0).on_click(move || pm.msg.set(String::new())));
        }
        let body: Element = if items.is_empty() {
            if p.loading.get() { column(vec![spinner(32.0, 3.0, ACCENT2)]).center() } else { column(vec![tx(if p.error.get().is_empty() { "Hech kim topilmadi".to_string() } else { p.error.get() }, 14.0, Color::rgba(255, 255, 255, 140))]).center() }
        } else {
            let mut kids: Vec<Element> = Vec::new();
            for u in items.iter() {
                let (pc, uc) = (p.clone(), u.clone());
                let mut sub = vec![format!("Balans {}", super::billing::format_sum(u.balance))];
                if u.sub_days_left() > 0 {
                    sub.push(format!("Obuna {} kun", u.sub_days_left()));
                }
                if u.last_login_at > 0 {
                    sub.push(format!("onlayn {}", ago(u.last_login_at)));
                }
                let letter: String = u.name.trim_start_matches('@').chars().take(1).flat_map(|c| c.to_uppercase()).collect();
                let mut r = vec![
                    column(vec![tx(letter, 16.0, ACCENT2).bold()]).center().width(44.0).height(44.0).radius(22.0).bg(ACCENT.with_opacity(0.22)),
                    column(vec![
                        tx(u.name.clone(), 14.0, Color::WHITE).bold(),
                        tx(format!("ID {}{}", u.id, if u.username.is_empty() { String::new() } else { format!(" · @{}", u.username) }), 12.0, Color::rgba(255, 255, 255, 160)),
                        tx(sub.join(" · "), 11.0, Color::rgba(255, 255, 255, 115)),
                    ])
                    .weight(1.0),
                ];
                if u.banned {
                    r.push(tx("BLOK", 10.0, ERROR).black().bg(ERROR.with_opacity(0.18)).radius(8.0).padding_xy(8.0, 4.0));
                }
                kids.push(Self::pglass(row(r).gap(12.0).align(Align::Center).padding(12.0)).radius(22.0).margin_each(0.0, 0.0, 0.0, 8.0).on_click(move || pc.modal.set(Modal::Menu(uc.clone()))));
            }
            if p.has_more.get() {
                let (pc, me) = (p.clone(), self.clone());
                kids.push(self.chip(if p.loading.get() { "Yuklanmoqda..." } else { "Yana yuklash" }, false, move || me.users_load(&pc, pc.page.get() + 1)));
            }
            kids.push(gap_box(0.0, 28.0));
            scroll_column(&p.scroll, kids)
        };
        let page = self.scaffold("Foydalanuvchilar", column(vec![column(head), body.grow(1.0)]).padding_each(16.0, 4.0, 16.0, 0.0));
        let mut layers = vec![page];
        if !matches!(p.modal.get(), Modal::None) {
            layers.push(self.users_modal(p));
        }
        stack(layers)
    }

    fn users_modal(self: &Rc<Self>, p: &Rc<UsersPage>) -> Element {
        let close = {
            let p = p.clone();
            move || {
                p.modal.set(Modal::None);
                p.num.set(String::new());
                p.reason.set(String::new());
            }
        };
        let mut kids: Vec<Element> = Vec::new();
        let item = |label: &str, danger: bool, f: Box<dyn Fn()>| tx(label, 15.0, if danger { ERROR } else { Color::WHITE }).padding_xy(18.0, 13.0).on_click(f);
        let styled_num = |hint: &str| {
            field(&p.num).color(Color::WHITE).size(16.0).hint(hint).hint_color(Color::rgba(255, 255, 255, 110)).accent(ACCENT2).keyboard(KeyboardKind::Number).padding_xy(14.0, 12.0).glass(Color::rgba(255, 255, 255, 22)).border(1.0, Color::TRANSPARENT).radius(14.0)
        };
        match p.modal.get() {
            Modal::None => {}
            Modal::Menu(u) => {
                kids.push(tx(u.name.clone(), 17.0, Color::WHITE).bold().padding_xy(18.0, 12.0));
                let (me, u1) = (self.clone(), u.clone());
                let (me0, u0, c0) = (self.clone(), u.clone(), close.clone());
                kids.push(item("Profilini ko'rish", false, Box::new(move || { c0(); me0.open_public_profile(u0.id); })));
                kids.push(item("Yozishma", false, { let c = close.clone(); Box::new(move || { c(); me.open_support(Some(u1.id)); }) }));
                let (pc, u2) = (p.clone(), u.clone());
                kids.push(item("Balansni o'zgartirish", false, Box::new(move || pc.modal.set(Modal::Amount(u2.clone())))));
                let (pc, u3) = (p.clone(), u.clone());
                kids.push(item("Obuna muddati", false, Box::new(move || pc.modal.set(Modal::Days(u3.clone())))));
                let (pc, u4) = (p.clone(), u.clone());
                if u.banned {
                    kids.push(item("Blokdan chiqarish", false, Box::new(move || pc.modal.set(Modal::Unban(u4.clone())))));
                } else {
                    kids.push(item("Bloklash", true, Box::new(move || pc.modal.set(Modal::Ban(u4.clone())))));
                }
                kids.push(row(vec![spacer(), self.chip("Yopish", false, close.clone())]).padding_each(0.0, 8.0, 12.0, 8.0));
            }
            Modal::Amount(u) => {
                kids.push(tx("Balansni o'zgartirish", 18.0, Color::WHITE).bold());
                kids.push(tx(format!("{} · hozir {}", u.name, super::billing::format_sum(u.balance)), 13.0, Color::rgba(255, 255, 255, 160)).padding_each(0.0, 6.0, 0.0, 10.0));
                kids.push(styled_num("Masalan 10000"));
                let n = p.num.get().parse::<i64>().unwrap_or(0);
                let (m1, p1, u1, c1) = (self.clone(), p.clone(), u.clone(), close.clone());
                let (m2, p2, u2, c2) = (self.clone(), p.clone(), u.clone(), close.clone());
                kids.push(
                    row(vec![
                        self.chip("Ayirish", false, move || if n > 0 { c2(); m2.user_act(&p2, u2.id, "balance", -n, 0, String::new(), format!("{} yechib olindi", super::billing::format_sum(n))); }),
                        self.chip("Qo'shish", true, move || if n > 0 { c1(); m1.user_act(&p1, u1.id, "balance", n, 0, String::new(), format!("{} qo'shildi", super::billing::format_sum(n))); }),
                    ])
                    .gap(8.0)
                    .padding_each(0.0, 14.0, 0.0, 0.0),
                );
            }
            Modal::Days(u) => {
                let left = u.sub_days_left();
                kids.push(tx("Obuna muddati", 18.0, Color::WHITE).bold());
                kids.push(tx(if left > 0 { format!("{} · hozir {left} kun qolgan", u.name) } else { format!("{} · obunasi yo'q", u.name) }, 13.0, Color::rgba(255, 255, 255, 160)).padding_each(0.0, 6.0, 0.0, 10.0));
                kids.push(styled_num("Necha kun"));
                let n = p.num.get().parse::<i64>().unwrap_or(0);
                let (m1, p1, u1, c1) = (self.clone(), p.clone(), u.clone(), close.clone());
                let (m2, p2, u2, c2) = (self.clone(), p.clone(), u.clone(), close.clone());
                kids.push(
                    row(vec![
                        self.chip("Kun ayirish", false, move || if n > 0 { c2(); m2.user_act(&p2, u2.id, "sub", 0, -n, String::new(), format!("{n} kun olib tashlandi")); }),
                        self.chip("Kun qo'shish", true, move || if n > 0 { c1(); m1.user_act(&p1, u1.id, "sub", 0, n, String::new(), format!("{n} kun qo'shildi")); }),
                    ])
                    .gap(8.0)
                    .padding_each(0.0, 14.0, 0.0, 0.0),
                );
                if left > 0 {
                    let (m3, p3, u3, c3) = (self.clone(), p.clone(), u.clone(), close.clone());
                    kids.push(tx("Obunani butunlay bekor qilish", 13.0, ERROR).padding_each(0.0, 12.0, 0.0, 0.0).on_click(move || { c3(); m3.user_act(&p3, u3.id, "sub_clear", 0, 0, String::new(), "Obuna bekor qilindi".into()); }));
                }
            }
            Modal::Ban(u) => {
                kids.push(tx(format!("{} — bloklash", u.name), 18.0, Color::WHITE).bold());
                let chips: Vec<Element> = [(0, "Muddatsiz"), (1, "1 kun"), (3, "3 kun"), (7, "7 kun"), (30, "30 kun")]
                    .iter()
                    .map(|(d, l)| {
                        let pb = p.ban_days.clone();
                        let d = *d;
                        self.chip(l, p.ban_days.get() == d && p.num.get().is_empty(), move || pb.set(d))
                    })
                    .collect();
                kids.push(row(chips).gap(6.0).scroll(&self.chips_scroll2).height(44.0).padding_each(0.0, 10.0, 0.0, 0.0));
                kids.push(gap_box(0.0, 10.0));
                kids.push(styled_num("Boshqa muddat (kun)"));
                kids.push(gap_box(0.0, 8.0));
                kids.push(field(&p.reason).color(Color::WHITE).size(15.0).hint("Sabab (foydalanuvchi ko'radi)").hint_color(Color::rgba(255, 255, 255, 110)).accent(ACCENT2).padding_xy(14.0, 12.0).glass(Color::rgba(255, 255, 255, 22)).border(1.0, Color::TRANSPARENT).radius(14.0));
                let (m1, p1, u1, c1) = (self.clone(), p.clone(), u.clone(), close.clone());
                kids.push(
                    row(vec![
                        spacer(),
                        self.chip("Bekor qilish", false, close.clone()),
                        self.chip("Bloklash", true, move || {
                            let days = p1.num.get().parse::<i64>().unwrap_or(p1.ban_days.get());
                            let reason = p1.reason.get().trim().to_string();
                            c1();
                            m1.user_act(&p1, u1.id, "ban", 0, days, reason, if days > 0 { format!("Bloklandi — {days} kun") } else { "Muddatsiz bloklandi".into() });
                        }),
                    ])
                    .gap(8.0)
                    .padding_each(0.0, 14.0, 0.0, 0.0),
                );
            }
            Modal::Unban(u) => {
                kids.push(tx(format!("{} blokdan chiqarilsinmi?", u.name), 17.0, Color::WHITE).bold());
                if !u.ban_reason.trim().is_empty() {
                    kids.push(tx(format!("Sabab: {}", u.ban_reason), 13.0, Color::rgba(255, 255, 255, 170)).padding_each(0.0, 8.0, 0.0, 0.0));
                }
                let (m1, p1, u1, c1) = (self.clone(), p.clone(), u.clone(), close.clone());
                kids.push(row(vec![spacer(), self.chip("Yo'q", false, close.clone()), self.chip("Ochish", true, move || { c1(); m1.user_act(&p1, u1.id, "unban", 0, 0, String::new(), "Blokdan chiqarildi".into()); })]).gap(8.0).padding_each(0.0, 14.0, 0.0, 0.0));
            }
        }
        let panel = column(kids).lens(Color::rgba(18, 18, 24, 120), GLASS_ZOOM).radius(28.0).padding(18.0).margin_each(20.0, 0.0, 20.0, 0.0);
        column(vec![panel]).center().bg(Color::rgba(0, 0, 0, 130)).on_click(close)
    }

    // ── Suhbatlar (0) va Shikoyatlar (1) ──

    fn open_admin_list(self: &Rc<Self>, kind: u8) {
        let p = Rc::new(ListPage {
            items: State::new(Rc::new(Vec::new())),
            loaded: State::new(false),
            error: State::new(String::new()),
            has_more: State::new(false),
            page: std::cell::Cell::new(0),
            scroll: ScrollState::new(),
            confirm: State::new(None),
        });
        self.list_load(&p, kind, 0);
        self.push(if kind == 0 { pages::Page::AdminThreads(p) } else { pages::Page::AdminReports(p) });
    }

    fn list_load(self: &Rc<Self>, p: &Rc<ListPage>, kind: u8, page: i64) {
        let (p2, session) = (p.clone(), self.session.clone());
        spawn_bg(
            move || {
                let (path, query) = if kind == 0 { ("/api/chat/threads".to_string(), String::new()) } else { ("/api/admin/reports".to_string(), format!("?page={page}")) };
                let (code, body) = api::request("GET", &path, &query, None, Some(&session))?;
                if code >= 400 {
                    return Err(err_of(code, &body, "Yuklanmadi"));
                }
                serde_json::from_str::<Value>(&body).map_err(|_| "Javob noto'g'ri".to_string())
            },
            move |r: Result<Value, String>| {
                p2.loaded.set(true);
                match r {
                    Ok(j) => {
                        let rows: Vec<Value> = j["items"].as_array().cloned().unwrap_or_default();
                        let mut all: Vec<Value> = if page == 0 { Vec::new() } else { (*p2.items.get()).clone() };
                        all.extend(rows);
                        p2.items.set(Rc::new(all));
                        p2.has_more.set(j["has_more"].as_bool().unwrap_or(false));
                        p2.page.set(page);
                        p2.error.set(String::new());
                    }
                    Err(e) => p2.error.set(e),
                }
            },
        );
    }

    pub(super) fn admin_threads_page(self: &Rc<Self>, p: &Rc<ListPage>) -> Element {
        let items = p.items.get();
        let body: Element = if !p.loaded.get() {
            column(vec![spinner(32.0, 3.0, ACCENT2)]).center()
        } else if items.is_empty() {
            column(vec![tx(if p.error.get().is_empty() { "Suhbatlar yo'q".to_string() } else { p.error.get() }, 14.0, Color::rgba(255, 255, 255, 140))]).center()
        } else {
            let mut kids: Vec<Element> = Vec::new();
            for t in items.iter() {
                let uid = num(&t["user_id"]);
                let first = t["first_name"].as_str().unwrap_or("").trim();
                let user = t["username"].as_str().unwrap_or("");
                let name = if !first.is_empty() { first.to_string() } else if !user.is_empty() { format!("@{user}") } else { format!("Foydalanuvchi {uid}") };
                let last = t["last_body"].as_str().unwrap_or("");
                let from_admin = t["last_from_admin"].as_bool().unwrap_or(false);
                let unread = num(&t["unread"]);
                let letter: String = name.trim_start_matches('@').chars().take(1).flat_map(|c| c.to_uppercase()).collect();
                let mut right = vec![tx(ago(num(&t["last_at"])), 10.0, Color::rgba(255, 255, 255, 110))];
                if unread > 0 {
                    right.push(tx(unread.to_string(), 11.0, Color::WHITE).bold().bg(ACCENT).radius(10.0).padding_xy(8.0, 2.0).margin_each(0.0, 4.0, 0.0, 0.0));
                }
                let (me, pc, t2) = (self.clone(), p.clone(), t.clone());
                kids.push(
                    Self::pglass(
                        row(vec![
                            column(vec![tx(letter, 16.0, ACCENT2).bold()]).center().width(44.0).height(44.0).radius(22.0).bg(ACCENT.with_opacity(0.22)),
                            column(vec![tx(name, 14.0, Color::WHITE).bold(), tx(format!("{}{}", if from_admin { "Siz: " } else { "" }, if last.is_empty() { "Fayl" } else { last }), 12.0, Color::rgba(255, 255, 255, 150))]).weight(1.0),
                            column(right).align(Align::End),
                        ])
                        .gap(12.0)
                        .align(Align::Center)
                        .padding(12.0),
                    )
                    .radius(22.0)
                    .margin_each(0.0, 0.0, 0.0, 8.0)
                    .on_click(move || {
                        let _ = (&pc, &t2);
                        me.open_support(Some(uid));
                    }),
                );
            }
            kids.push(gap_box(0.0, 28.0));
            scroll_column(&p.scroll, kids)
        };
        self.scaffold("Suhbatlar", body.padding_each(16.0, 4.0, 16.0, 0.0))
    }

    pub(super) fn admin_reports_page(self: &Rc<Self>, p: &Rc<ListPage>) -> Element {
        let items = p.items.get();
        let body: Element = if !p.loaded.get() {
            column(vec![spinner(32.0, 3.0, ACCENT2)]).center()
        } else if items.is_empty() {
            column(vec![tx(if p.error.get().is_empty() { "Shikoyat yo'q".to_string() } else { p.error.get() }, 14.0, Color::rgba(255, 255, 255, 140))]).center()
        } else {
            let mut kids: Vec<Element> = Vec::new();
            let person = |o: &Value| {
                let f = o["first_name"].as_str().unwrap_or("").trim();
                let u = o["username"].as_str().unwrap_or("");
                if !f.is_empty() { f.to_string() } else if !u.is_empty() { format!("@{u}") } else { "Foydalanuvchi".into() }
            };
            for r in items.iter() {
                let kind = r["kind"].as_str().unwrap_or("comment");
                let source = match kind { "comment" => "Izohdan", "dm" => "Shaxsiy yozishmadan", k => k };
                let (pc, r2) = (p.clone(), r.clone());
                kids.push(
                    Self::pglass(
                        column(vec![
                            row(vec![tx(source, 11.5, ACCENT2).bold(), spacer(), tx(fmt_ymd(num(&r["created_at"])), 11.0, Color::rgba(255, 255, 255, 115))]),
                            tx(format!("{}: {}", person(&r["target_user"]), r["target_body"].as_str().unwrap_or("")), 14.0, Color::WHITE).padding_each(0.0, 6.0, 0.0, 4.0),
                            tx(format!("Sabab ({}): {}", person(&r["reporter"]), r["reason"].as_str().unwrap_or("—")), 12.5, Color::rgba(255, 255, 255, 165)),
                            row(vec![spacer(), self.chip("Tozalash", false, move || pc.confirm.set(Some(r2.clone())))]).padding_each(0.0, 8.0, 0.0, 0.0),
                        ])
                        .padding(14.0),
                    )
                    .radius(22.0)
                    .margin_each(0.0, 0.0, 0.0, 10.0),
                );
            }
            if p.has_more.get() {
                let (pc, me) = (p.clone(), self.clone());
                kids.push(self.chip("Yana yuklash", false, move || me.list_load(&pc, 1, pc.page.get() + 1)));
            }
            kids.push(gap_box(0.0, 28.0));
            scroll_column(&p.scroll, kids)
        };
        let mut layers = vec![self.scaffold("Shikoyatlar", body.padding_each(16.0, 4.0, 16.0, 0.0))];
        if let Some(r) = p.confirm.get() {
            let id = r["id"].as_str().unwrap_or("").to_string();
            let (pc, pd, me) = (p.clone(), p.clone(), self.clone());
            let panel = column(vec![
                tx("Shikoyatni tozalash", 18.0, Color::WHITE).bold(),
                gap_box(0.0, 10.0),
                tx("Shikoyat ro'yxatdan olib tashlanadi.", 14.0, Color::rgba(255, 255, 255, 200)),
                gap_box(0.0, 18.0),
                row(vec![
                    spacer(),
                    self.chip("Bekor qilish", false, move || pc.confirm.set(None)),
                    self.chip("Tozalash", true, move || {
                        pd.confirm.set(None);
                        let v: Vec<Value> = pd.items.get().iter().filter(|x| x["id"].as_str() != Some(&id)).cloned().collect();
                        pd.items.set(Rc::new(v));
                        let (session, id2) = (me.session.clone(), id.clone());
                        spawn_bg(move || api::request("DELETE", &format!("/api/admin/report/{id2}"), "", None, Some(&session)), |_| {});
                    }),
                ])
                .gap(8.0),
            ])
            .lens(Color::rgba(18, 18, 24, 120), GLASS_ZOOM)
            .radius(28.0)
            .padding(22.0)
            .margin_each(24.0, 0.0, 24.0, 0.0);
            let pc = p.clone();
            layers.push(column(vec![panel]).center().bg(Color::rgba(0, 0, 0, 130)).on_click(move || pc.confirm.set(None)));
        }
        stack(layers)
    }

    // ── Umumiy statistika ──

    fn open_admin_stats(self: &Rc<Self>) {
        let p = Rc::new(StatsPage { data: State::new(None), error: State::new(String::new()), open: State::new(String::new()), range: State::new("7d".into()), series: State::new(None), scroll: ScrollState::new() });
        let (p2, session) = (p.clone(), self.session.clone());
        spawn_bg(move || api::get_json("/api/stats", "", Some(&session)), move |r| match r {
            Ok(j) => p2.data.set(Some(Rc::new(j))),
            Err(e) => p2.error.set(e),
        });
        self.push(pages::Page::AdminStats(p));
    }

    fn stats_series(self: &Rc<Self>, p: &Rc<StatsPage>) {
        p.series.set(None);
        let (p2, session, metric, range) = (p.clone(), self.session.clone(), p.open.get(), p.range.get());
        spawn_bg(
            move || api::get_json("/api/stats/series", &format!("?metric={}&range={}", crate::backend::urlenc(&metric), range), Some(&session)),
            move |r| {
                let pts: Vec<(String, i64)> = r.ok().and_then(|j| j["points"].as_array().map(|a| a.iter().map(|o| (o["t"].as_str().unwrap_or("").to_string(), num(&o["v"]))).collect())).unwrap_or_default();
                p2.series.set(Some(Rc::new(pts)));
            },
        );
    }

    pub(super) fn admin_stats_page(self: &Rc<Self>, p: &Rc<StatsPage>) -> Element {
        let Some(d) = p.data.get() else {
            return self.scaffold("Umumiy statistika", if p.error.get().is_empty() { column(vec![spinner(32.0, 3.0, ACCENT2)]).center() } else { column(vec![tx(p.error.get(), 14.0, ERROR)]).center() });
        };
        let blk = |k: &str| (num(&d[k]["daily"]), num(&d[k]["weekly"]), num(&d[k]["monthly"]), num(&d[k]["total"]));
        let c = &d["content"];
        let mut kids: Vec<Element> = vec![Self::pglass(
            column(vec![
                tx("Kontent", 15.0, Color::WHITE).bold(),
                tx(format!("Anime: {} · Bo'lim: {} · Qism: {}", num(&c["anime"]), num(&c["seasons"]), num(&c["episodes"])), 13.0, Color::rgba(255, 255, 255, 170)).padding_each(0.0, 6.0, 0.0, 0.0),
                tx(format!("Yangi qismlar: bugun {} · hafta {} · oy {}", num(&c["episodes_daily"]), num(&c["episodes_weekly"]), num(&c["episodes_monthly"])), 12.0, Color::rgba(255, 255, 255, 130)).padding_each(0.0, 4.0, 0.0, 0.0),
            ])
            .padding(16.0),
        )
        .radius(24.0)
        .margin_each(0.0, 0.0, 0.0, 12.0)];
        type Fmt = fn(i64) -> String;
        let cards: [(&str, &str, Fmt); 6] = [
            ("Foydalanuvchilar", "users", |v| format!("{} ta", format_count(v))),
            ("Anime ko'rishlar", "anime_views", |v| format!("{} ta", format_count(v))),
            ("Bo'lim ko'rishlar", "season_views", |v| format!("{} ta", format_count(v))),
            ("Qism ko'rishlar", "views", |v| format!("{} ta", format_count(v))),
            ("Ko'rish vaqti", "watch", |v| format!("{} soat", format_hours(v))),
            ("Trafik sarfi", "traffic", |v| format_bytes(v)),
        ];
        let metric_of = |k: &str| match k { "watch" => "watch_ms", other => other }.to_string();
        for (title, key, fmt) in cards {
            let (day, week, month, total) = blk(key);
            let open = p.open.get() == metric_of(key);
            let (pc, me, m) = (p.clone(), self.clone(), metric_of(key));
            let mut col = vec![
                tx(title, 15.0, Color::WHITE).bold(),
                tx(fmt(total), 24.0, ACCENT2).black(),
                row(vec![("Bugun", day), ("Hafta", week), ("Oy", month)].into_iter().map(|(l, v)| column(vec![tx(fmt(v), 13.0, Color::WHITE).bold(), tx(l, 11.0, Color::rgba(255, 255, 255, 115))]).weight(1.0)).collect()).padding_each(0.0, 6.0, 0.0, 0.0),
            ];
            if open {
                let ranges = [("24h", "24 soat"), ("7d", "7 kun"), ("30d", "30 kun"), ("all", "Hammasi")];
                col.push(row(ranges.iter().map(|(k, l)| { let (pr, me2) = (p.clone(), self.clone()); let k = k.to_string(); self.chip(l, p.range.get() == k, move || { pr.range.set(k.clone()); me2.stats_series(&pr); }) }).collect()).gap(6.0).padding_each(0.0, 12.0, 0.0, 0.0));
                match p.series.get() {
                    None => col.push(column(vec![spinner(24.0, 2.0, ACCENT2)]).center().height(120.0)),
                    Some(pts) if pts.is_empty() => col.push(column(vec![tx("Ma'lumot yo'q", 12.0, Color::rgba(255, 255, 255, 115))]).center().height(120.0)),
                    Some(pts) => {
                        let max = pts.iter().map(|x| x.1).max().unwrap_or(1).max(1) as f32;
                        let bars: Vec<Element> = pts.iter().map(|(_, v)| column(vec![spacer(), column(vec![]).height((100.0 * *v as f32 / max).max(1.0)).bg(ACCENT2).radius(2.0)]).weight(1.0).height(100.0).padding_each(1.0, 0.0, 1.0, 0.0)).collect();
                        col.push(row(bars).height(100.0).padding_each(0.0, 10.0, 0.0, 0.0));
                        col.push(tx(format!("Eng yuqori: {}", fmt(max as i64)), 11.0, Color::rgba(255, 255, 255, 130)).padding_each(0.0, 6.0, 0.0, 0.0));
                    }
                }
            }
            kids.push(
                Self::pglass(column(col).padding(16.0)).radius(24.0).margin_each(0.0, 0.0, 0.0, 12.0).on_click(move || {
                    if pc.open.get() == m {
                        pc.open.set(String::new());
                    } else {
                        pc.open.set(m.clone());
                        me.stats_series(&pc);
                    }
                }),
            );
        }
        kids.push(tx("Vaqt mintaqasi: UTC+5 (Toshkent)", 11.0, Color::rgba(255, 255, 255, 110)).padding_each(4.0, 0.0, 0.0, 32.0));
        self.scaffold("Umumiy statistika", scroll_column(&p.scroll, kids).padding_each(16.0, 4.0, 16.0, 0.0))
    }
}
