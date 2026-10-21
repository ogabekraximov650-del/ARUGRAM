//! Admin: majburiy kanallar, to'plamlarni ko'rib chiqish, ilova va xavfsizlik sozlamalari.

use super::*;
use crate::api;
use crate::data::format_bytes;
use serde_json::{json, Value};

fn num(v: &Value) -> i64 {
    v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)).or_else(|| v.as_str().and_then(|s| s.parse().ok())).unwrap_or(0)
}

fn post(session: &str, path: &str, body: Value) -> Result<Value, String> {
    let (code, text) = api::request("POST", path, "", Some(&body), Some(session)).map_err(|_| "Internet yo'q".to_string())?;
    let j: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if code >= 400 {
        return Err(j["error"].as_str().map(String::from).unwrap_or_else(|| if code == 403 { "Bu bo'lim faqat admin uchun".into() } else { format!("Xato ({code})") }));
    }
    Ok(j)
}

fn modal(inner: Element, close: impl Fn() + 'static) -> Element {
    let panel = inner.lens(Color::rgba(18, 18, 24, 120), GLASS_ZOOM).radius(28.0).padding(20.0).margin_each(20.0, 0.0, 20.0, 0.0);
    column(vec![panel]).center().bg(Color::rgba(0, 0, 0, 130)).on_click(close)
}

pub struct ChannelsPage {
    items: State<Rc<Vec<Value>>>,
    loaded: State<bool>,
    error: State<String>,
    limit_for: State<Option<Value>>,
    del_for: State<Option<Value>>,
    num: State<String>,
    msg: State<String>,
    // Qo'shish oynasi
    add: State<bool>,
    input: State<String>,
    info: State<Option<Rc<Value>>>,
    kind: State<String>,
    unlimited: State<bool>,
    need: State<String>,
    busy: State<bool>,
    add_err: State<String>,
    scroll: ScrollState,
}

pub struct PacksReviewPage {
    ops: State<Option<Rc<Vec<Value>>>>,
    reject_for: State<Option<Value>>,
    reason: State<String>,
    approve_all: State<bool>,
    msg: State<String>,
    scroll: ScrollState,
}

pub struct AppPage {
    cfg: State<Option<Rc<Value>>>,
    version: State<String>,
    error: State<String>,
    msg: State<String>,
}

impl Home {
    // ── Majburiy kanallar ──

    pub(super) fn open_admin_channels(self: &Rc<Self>) {
        let p = Rc::new(ChannelsPage {
            items: State::new(Rc::new(Vec::new())),
            loaded: State::new(false),
            error: State::new(String::new()),
            limit_for: State::new(None),
            del_for: State::new(None),
            num: State::new(String::new()),
            msg: State::new(String::new()),
            add: State::new(false),
            input: State::new(String::new()),
            info: State::new(None),
            kind: State::new("public".into()),
            unlimited: State::new(true),
            need: State::new(String::new()),
            busy: State::new(false),
            add_err: State::new(String::new()),
            scroll: ScrollState::new(),
        });
        self.ch_load(&p);
        self.push(pages::Page::AdminChannels(p));
    }

    fn ch_load(self: &Rc<Self>, p: &Rc<ChannelsPage>) {
        let (p2, session) = (p.clone(), self.session.clone());
        spawn_bg(move || api::get_json("/api/admin/channels", "", Some(&session)), move |r| {
            p2.loaded.set(true);
            match r {
                Ok(j) => {
                    p2.items.set(Rc::new(j["items"].as_array().cloned().unwrap_or_default()));
                    p2.error.set(String::new());
                }
                Err(e) => p2.error.set(e),
            }
        });
    }

    fn ch_op(self: &Rc<Self>, p: &Rc<ChannelsPage>, body: Value, done: &'static str) {
        let (p2, me, session) = (p.clone(), self.clone(), self.session.clone());
        spawn_bg(move || post(&session, "/api/admin/channels", body), move |r| match r {
            Ok(_) => {
                p2.msg.set(done.into());
                me.ch_load(&p2);
            }
            Err(e) => p2.msg.set(e),
        });
    }

    pub(super) fn admin_channels_page(self: &Rc<Self>, p: &Rc<ChannelsPage>) -> Element {
        let items = p.items.get();
        let body: Element = if !p.loaded.get() {
            column(vec![spinner(32.0, 3.0, ACCENT2)]).center()
        } else if items.is_empty() {
            column(vec![tx(if p.error.get().is_empty() { "Kanal yo'q".to_string() } else { p.error.get() }, 14.0, Color::rgba(255, 255, 255, 140))]).center()
        } else {
            let mut kids: Vec<Element> = Vec::new();
            for c in items.iter() {
                let need = num(&c["need"]);
                let (pl, pd, c1, c2) = (p.clone(), p.clone(), c.clone(), c.clone());
                kids.push(
                    Self::pglass(
                        column(vec![
                            tx(c["title"].as_str().unwrap_or("").to_string(), 15.0, Color::WHITE).bold(),
                            tx(
                                format!("{} · {} / {} · {}", if c["kind"] == "private" { "Yopiq" } else { "Ochiq" }, num(&c["joined"]), if need > 0 { need.to_string() } else { "cheksiz".into() }, if c["active"].as_bool().unwrap_or(false) { "faol" } else { "to'ldi" }),
                                12.0,
                                Color::rgba(255, 255, 255, 150),
                            ),
                            tx(c["url"].as_str().unwrap_or("").to_string(), 11.0, Color::rgba(255, 255, 255, 110)),
                            row(vec![
                                self.chip("Limit", false, move || {
                                    pl.num.set(if need > 0 { need.to_string() } else { String::new() });
                                    pl.limit_for.set(Some(c1.clone()));
                                }),
                                self.chip("O'chirish", false, move || pd.del_for.set(Some(c2.clone()))),
                            ])
                            .gap(8.0)
                            .padding_each(0.0, 10.0, 0.0, 0.0),
                        ])
                        .padding(14.0),
                    )
                    .radius(22.0)
                    .margin_each(0.0, 0.0, 0.0, 10.0),
                );
            }
            kids.push(gap_box(0.0, 90.0));
            scroll_column(&p.scroll, kids)
        };
        let mut col = vec![body.grow(1.0)];
        if !p.msg.get().is_empty() {
            let pm = p.clone();
            col.insert(0, tx(p.msg.get(), 13.0, Color::rgba(255, 255, 255, 230)).bg(Color::rgba(0, 0, 0, 120)).radius(14.0).padding_xy(14.0, 8.0).margin_each(0.0, 0.0, 0.0, 8.0).on_click(move || pm.msg.set(String::new())));
        }
        let pa = p.clone();
        col.push(row(vec![spacer(), column(vec![tx("+ Kanal qo'shish", 14.0, Color::WHITE).bold()]).center().padding_xy(22.0, 14.0).radius(26.0).gradient(ACCENT, ACCENT2).on_click(move || pa.add.set(true)), spacer()]).padding_each(0.0, 8.0, 0.0, 12.0));
        let mut layers = vec![self.scaffold("Majburiy kanallar", column(col).padding_each(16.0, 4.0, 16.0, 0.0))];
        if let Some(c) = p.limit_for.get() {
            let (pc, ps, me) = (p.clone(), p.clone(), self.clone());
            let id = num(&c["id"]);
            layers.push(modal(
                column(vec![
                    tx("Limit (0 — cheksiz)", 18.0, Color::WHITE).bold(),
                    gap_box(0.0, 10.0),
                    field(&p.num).color(Color::WHITE).size(16.0).keyboard(KeyboardKind::Number).accent(ACCENT2).padding_xy(14.0, 12.0).glass(Color::rgba(255, 255, 255, 22)).border(1.0, Color::TRANSPARENT).radius(14.0),
                    gap_box(0.0, 14.0),
                    row(vec![spacer(), self.chip("Bekor", false, { let pc = pc.clone(); move || pc.limit_for.set(None) }), self.chip("Saqlash", true, move || {
                        ps.limit_for.set(None);
                        me.ch_op(&ps, json!({"op": "set", "id": id, "need": ps.num.get().parse::<i64>().unwrap_or(0)}), "Saqlandi");
                    })])
                    .gap(8.0),
                ]),
                move || pc.limit_for.set(None),
            ));
        }
        if let Some(c) = p.del_for.get() {
            let (pc, pd, me) = (p.clone(), p.clone(), self.clone());
            let id = num(&c["id"]);
            layers.push(modal(
                column(vec![
                    tx("Kanalni o'chirish", 18.0, Color::WHITE).bold(),
                    gap_box(0.0, 10.0),
                    tx(format!("«{}» ro'yxatdan olib tashlanadi.", c["title"].as_str().unwrap_or("")), 14.0, Color::rgba(255, 255, 255, 200)),
                    gap_box(0.0, 14.0),
                    row(vec![spacer(), self.chip("Bekor", false, { let pc = pc.clone(); move || pc.del_for.set(None) }), self.chip("O'chirish", true, move || {
                        pd.del_for.set(None);
                        me.ch_op(&pd, json!({"op": "del", "id": id}), "O'chirildi");
                    })])
                    .gap(8.0),
                ]),
                move || pc.del_for.set(None),
            ));
        }
        if p.add.get() {
            layers.push(self.ch_add_dialog(p));
        }
        stack(layers)
    }

    fn ch_add_dialog(self: &Rc<Self>, p: &Rc<ChannelsPage>) -> Element {
        let styled = |f: Element, hint: &str| f.color(Color::WHITE).size(15.0).hint(hint).hint_color(Color::rgba(255, 255, 255, 110)).accent(ACCENT2).padding_xy(14.0, 12.0).glass(Color::rgba(255, 255, 255, 22)).border(1.0, Color::TRANSPARENT).radius(14.0);
        let mut kids = vec![tx("Kanal qo'shish", 18.0, Color::WHITE).bold(), gap_box(0.0, 10.0), styled(field(&p.input), "@username, -100... ID yoki t.me/+havola"), gap_box(0.0, 10.0)];
        let (pk, me) = (p.clone(), self.clone());
        kids.push(row(vec![self.chip(if p.busy.get() { "Tekshirilmoqda..." } else { "Tekshirish" }, true, move || {
            let arg = pk.input.get().trim().to_string();
            if arg.is_empty() || pk.busy.get() {
                return;
            }
            pk.busy.set(true);
            pk.add_err.set(String::new());
            pk.info.set(None);
            let p2 = pk.clone();
            let _ = &me;
            spawn_bg(move || crate::backend::channel_info(&arg), move |j| {
                p2.busy.set(false);
                if j["ok"].as_bool().unwrap_or(false) {
                    p2.kind.set(if j["username"].as_str().map(|u| !u.is_empty() && u != "null").unwrap_or(false) { "public".into() } else { "private".into() });
                    p2.info.set(Some(Rc::new(j)));
                } else {
                    p2.add_err.set(j["error"].as_str().filter(|e| !e.is_empty()).unwrap_or("Kanal topilmadi").to_string());
                }
            });
        })]));
        if !p.add_err.get().is_empty() {
            kids.push(tx(p.add_err.get(), 13.0, ERROR).padding_each(0.0, 8.0, 0.0, 0.0));
        }
        if let Some(j) = p.info.get() {
            kids.push(gap_box(0.0, 12.0));
            kids.push(tx(j["title"].as_str().unwrap_or("").to_string(), 15.0, Color::WHITE).bold());
            let un = j["username"].as_str().filter(|u| !u.is_empty() && *u != "null").map(|u| format!("@{u} · ")).unwrap_or_default();
            kids.push(tx(format!("{un}{} a'zo", num(&j["members"])), 12.0, Color::rgba(255, 255, 255, 160)));
            kids.push(gap_box(0.0, 10.0));
            let kinds: Vec<Element> = [("public", "Ochiq"), ("private", "Yopiq")].iter().map(|(k, l)| { let (pk, k) = (p.kind.clone(), k.to_string()); let on = p.kind.get() == k; self.chip(l, on, move || pk.set(k.clone())) }).collect();
            kids.push(row(kinds).gap(8.0));
            let pu = p.unlimited.clone();
            kids.push(row(vec![tx("Cheksiz", 14.0, Color::WHITE).weight(1.0), Self::switch(p.unlimited.get(), move || pu.set(!pu.get()))]).align(Align::Center).padding_xy(2.0, 10.0));
            if !p.unlimited.get() {
                kids.push(styled(field(&p.need).keyboard(KeyboardKind::Number), "Limit (masalan 1000)"));
                kids.push(gap_box(0.0, 8.0));
            }
            let (pa, me2, j2) = (p.clone(), self.clone(), j.clone());
            kids.push(row(vec![self.chip(if p.busy.get() { "Qo'shilmoqda..." } else { "Qo'shish" }, true, move || {
                if pa.busy.get() {
                    return;
                }
                let n = if pa.unlimited.get() { 0 } else { pa.need.get().parse::<i64>().unwrap_or(0) };
                if !pa.unlimited.get() && n <= 0 {
                    pa.add_err.set("Limitni yozing yoki «Cheksiz»ni yoqing".into());
                    return;
                }
                pa.busy.set(true);
                pa.add_err.set(String::new());
                let (p3, m3, session, kind, input) = (pa.clone(), me2.clone(), me2.session.clone(), pa.kind.get(), pa.input.get().trim().to_string());
                let id = num(&j2["id"]);
                spawn_bg(
                    move || {
                        let mut target = if id != 0 { id.to_string() } else { input };
                        let mut admin_err = String::new();
                        let a = crate::backend::make_bot_admin(&target);
                        if a["ok"].as_bool().unwrap_or(false) && a.get("chat_id").is_some() {
                            target = num(&a["chat_id"]).to_string();
                        } else {
                            admin_err = a["error"].as_str().unwrap_or("").to_string();
                        }
                        post(&session, "/api/admin/channels", json!({"op": "add", "kind": kind, "input": target, "need": n})).map_err(|e| if admin_err.is_empty() { e } else { format!("{e}\n(Botni admin qilib bo'lmadi: {admin_err})") })
                    },
                    move |r| {
                        p3.busy.set(false);
                        match r {
                            Ok(_) => {
                                p3.add.set(false);
                                p3.info.set(None);
                                p3.input.set(String::new());
                                m3.ch_load(&p3);
                            }
                            Err(e) => p3.add_err.set(e),
                        }
                    },
                );
            })]).padding_each(0.0, 10.0, 0.0, 0.0));
        }
        let pc = p.clone();
        kids.push(row(vec![spacer(), self.chip("Yopish", false, { let pc = pc.clone(); move || pc.add.set(false) })]).padding_each(0.0, 10.0, 0.0, 0.0));
        let pc2 = p.clone();
        modal(column(kids), move || pc2.add.set(false))
    }

    // ── To'plamlarni ko'rib chiqish ──

    pub(super) fn open_admin_packs(self: &Rc<Self>) {
        let p = Rc::new(PacksReviewPage { ops: State::new(None), reject_for: State::new(None), reason: State::new(String::new()), approve_all: State::new(false), msg: State::new(String::new()), scroll: ScrollState::new() });
        self.pr_load(&p);
        self.push(pages::Page::AdminPacks(p));
    }

    fn pr_load(self: &Rc<Self>, p: &Rc<PacksReviewPage>) {
        let (p2, me, session) = (p.clone(), self.clone(), self.session.clone());
        spawn_bg(move || api::get_json("/api/packs/admin/pending", "", Some(&session)), move |r| {
            let list: Vec<Value> = r.ok().and_then(|j| j["ops"].as_array().cloned()).unwrap_or_default();
            // Rasmlar (tasdiqlashdan oldin ko'rish uchun) Telegram orqali olinadi.
            {
                let have = me.posters.borrow();
                let mut q = me.queue.borrow_mut();
                for o in &list {
                    if let Some(f) = o["file"].as_str() {
                        if !f.is_empty() && !have.contains_key(f) && !q.contains(&f.to_string()) {
                            q.push(f.to_string());
                        }
                    }
                }
            }
            p2.ops.set(Some(Rc::new(list)));
            me.next_posters();
        });
    }

    fn pr_review(self: &Rc<Self>, p: &Rc<PacksReviewPage>, ids: Vec<i64>, approve: bool, reason: String, done: &'static str) {
        let (p2, me, session) = (p.clone(), self.clone(), self.session.clone());
        spawn_bg(move || post(&session, "/api/packs/admin/review", json!({"ids": ids, "approve": approve, "reason": reason})), move |r| {
            p2.msg.set(match r { Ok(_) => done.to_string(), Err(e) => e });
            me.pr_load(&p2);
        });
    }

    pub(super) fn admin_packs_page(self: &Rc<Self>, p: &Rc<PacksReviewPage>) -> Element {
        let Some(list) = p.ops.get() else {
            return self.scaffold("To'plamlarni ko'rib chiqish", column(vec![spinner(32.0, 3.0, ACCENT2)]).center());
        };
        let body: Element = if list.is_empty() {
            column(vec![tx("Kutayotgan rasm yo'q", 14.0, Color::rgba(255, 255, 255, 140))]).center()
        } else {
            let pa = p.clone();
            let mut kids: Vec<Element> = vec![row(vec![tx(format!("{} ta kutmoqda", list.len()), 12.0, Color::rgba(255, 255, 255, 160)).weight(1.0), tx("Hammasini tasdiqlash", 12.0, ACCENT2).bold().on_click(move || pa.approve_all.set(true))]).padding_each(4.0, 0.0, 4.0, 10.0)];
            for o in list.iter() {
                let file = o["file"].as_str().unwrap_or("").to_string();
                let thumb = match self.posters.borrow().get(&file) {
                    Some(img) => stack(vec![image_fill(img)]),
                    None => column(vec![]).bg(Color::rgba(255, 255, 255, 20)).radius(12.0),
                };
                let id = num(&o["id"]);
                let (pr, me, o2) = (p.clone(), self.clone(), o.clone());
                let pa2 = p.clone();
                kids.push(
                    Self::pglass(
                        row(vec![
                            thumb.width(84.0).height(84.0),
                            column(vec![
                                tx(o["title"].as_str().unwrap_or("").to_string(), 14.0, Color::WHITE).bold(),
                                tx(format!("{} · {} · {}", o["kind"].as_str().unwrap_or(""), o["owner"].as_str().unwrap_or(""), format_bytes(num(&o["size"]))), 12.0, Color::rgba(255, 255, 255, 150)),
                                row(vec![
                                    self.chip("Tasdiqlash", true, move || me.pr_review(&pr, vec![id], true, String::new(), "Tasdiqlandi")),
                                    self.chip("Rad etish", false, move || pa2.reject_for.set(Some(o2.clone()))),
                                ])
                                .gap(8.0)
                                .padding_each(0.0, 6.0, 0.0, 0.0),
                            ])
                            .weight(1.0),
                        ])
                        .gap(12.0)
                        .padding(10.0),
                    )
                    .radius(22.0)
                    .margin_each(0.0, 0.0, 0.0, 10.0),
                );
            }
            kids.push(gap_box(0.0, 32.0));
            scroll_column(&p.scroll, kids)
        };
        let mut col = vec![body.grow(1.0)];
        if !p.msg.get().is_empty() {
            let pm = p.clone();
            col.insert(0, tx(p.msg.get(), 13.0, Color::rgba(255, 255, 255, 230)).bg(Color::rgba(0, 0, 0, 120)).radius(14.0).padding_xy(14.0, 8.0).margin_each(0.0, 0.0, 0.0, 8.0).on_click(move || pm.msg.set(String::new())));
        }
        let mut layers = vec![self.scaffold("To'plamlarni ko'rib chiqish", column(col).padding_each(16.0, 4.0, 16.0, 0.0))];
        if let Some(o) = p.reject_for.get() {
            let id = num(&o["id"]);
            let reasons: Vec<Element> = ["Nomaqbul kontent", "Sifati past", "Mualliflik huquqi buzilgan", "Noto'g'ri to'plamga yuborilgan"].iter().map(|r| { let pr = p.reason.clone(); let r = r.to_string(); self.chip(&r.clone(), false, move || pr.set(r.clone())) }).collect();
            let (pc, pd, me) = (p.clone(), p.clone(), self.clone());
            layers.push(modal(
                column(vec![
                    tx("Nima uchun rad etiladi?", 18.0, Color::WHITE).bold(),
                    gap_box(0.0, 10.0),
                    column(reasons).gap(6.0),
                    gap_box(0.0, 8.0),
                    field(&p.reason).color(Color::WHITE).size(15.0).hint("Yoki o'zingiz yozing").hint_color(Color::rgba(255, 255, 255, 110)).accent(ACCENT2).padding_xy(14.0, 12.0).glass(Color::rgba(255, 255, 255, 22)).border(1.0, Color::TRANSPARENT).radius(14.0),
                    gap_box(0.0, 14.0),
                    row(vec![spacer(), self.chip("Bekor", false, { let pc = pc.clone(); move || pc.reject_for.set(None) }), self.chip("Rad etish", true, move || {
                        pd.reject_for.set(None);
                        let reason = pd.reason.get().trim().to_string();
                        pd.reason.set(String::new());
                        me.pr_review(&pd, vec![id], false, reason, "Rad etildi");
                    })])
                    .gap(8.0),
                ]),
                move || pc.reject_for.set(None),
            ));
        }
        if p.approve_all.get() {
            let ids: Vec<i64> = list.iter().map(|o| num(&o["id"])).collect();
            let (pc, pd, me) = (p.clone(), p.clone(), self.clone());
            layers.push(modal(
                column(vec![
                    tx("Hammasini tasdiqlash", 18.0, Color::WHITE).bold(),
                    gap_box(0.0, 10.0),
                    tx(format!("Hamma {} ta rasm ko'rib chiqilib, tasdiqlansinmi?", ids.len()), 14.0, Color::rgba(255, 255, 255, 200)),
                    gap_box(0.0, 14.0),
                    row(vec![spacer(), self.chip("Yo'q", false, { let pc = pc.clone(); move || pc.approve_all.set(false) }), self.chip("Ha", true, move || {
                        pd.approve_all.set(false);
                        me.pr_review(&pd, ids.clone(), true, String::new(), "Tasdiqlandi");
                    })])
                    .gap(8.0),
                ]),
                move || pc.approve_all.set(false),
            ));
        }
        stack(layers)
    }

    // ── Ilova va xavfsizlik ──

    pub(super) fn open_admin_app(self: &Rc<Self>) {
        let p = Rc::new(AppPage { cfg: State::new(None), version: State::new(String::new()), error: State::new(String::new()), msg: State::new(String::new()) });
        self.app_load(&p);
        self.push(pages::Page::AdminApp(p));
    }

    fn app_load(self: &Rc<Self>, p: &Rc<AppPage>) {
        let (p2, session) = (p.clone(), self.session.clone());
        spawn_bg(move || api::get_json("/api/admin/app", "", Some(&session)), move |r| match r {
            Ok(j) => {
                p2.version.set(j["min_version"].as_str().unwrap_or("").to_string());
                p2.cfg.set(Some(Rc::new(j)));
            }
            Err(e) => p2.error.set(e),
        });
    }

    fn app_post(self: &Rc<Self>, p: &Rc<AppPage>, body: Value, done: &'static str) {
        let (p2, me, session) = (p.clone(), self.clone(), self.session.clone());
        spawn_bg(move || post(&session, "/api/admin/app", body), move |r| {
            p2.msg.set(match r { Ok(_) => done.to_string(), Err(e) => e });
            me.app_load(&p2);
        });
    }

    pub(super) fn admin_app_page(self: &Rc<Self>, p: &Rc<AppPage>) -> Element {
        let Some(c) = p.cfg.get() else {
            return self.scaffold("Ilova va xavfsizlik", if p.error.get().is_empty() { column(vec![spinner(32.0, 3.0, ACCENT2)]).center() } else { column(vec![tx(p.error.get(), 14.0, ERROR)]).center() });
        };
        let (ps, me) = (p.clone(), self.clone());
        let ver = Self::pglass(
            column(vec![
                tx("Eng kichik versiya", 15.0, Color::WHITE).bold(),
                tx("Bundan eski ilovalar yangilanmaguncha ishlamaydi. Masalan 0.1.0 yoki 0.1.0+9. Bo'sh — cheklov yo'q.", 12.0, Color::rgba(255, 255, 255, 150)).padding_each(0.0, 6.0, 0.0, 8.0),
                field(&p.version).color(Color::WHITE).size(15.0).accent(ACCENT2).padding_xy(14.0, 12.0).glass(Color::rgba(255, 255, 255, 22)).border(1.0, Color::TRANSPARENT).radius(14.0),
                gap_box(0.0, 10.0),
                row(vec![self.chip("Saqlash", true, move || {
                    let v = ps.version.get().trim().to_string();
                    me.app_post(&ps, json!({"min_version": v}), "Saqlandi");
                })]),
            ])
            .padding(16.0),
        )
        .radius(24.0)
        .margin_each(0.0, 0.0, 0.0, 12.0);
        let (p1, m1, p2, m2, p3, m3) = (p.clone(), self.clone(), p.clone(), self.clone(), p.clone(), self.clone());
        let sig = Self::pglass(
            column(vec![
                tx("Ilova imzosi", 15.0, Color::WHITE).bold(),
                tx(if c["gate_on"].as_bool().unwrap_or(false) { "Himoya YOQILGAN: faqat tanilgan imzoli ilovalar kiradi." } else { "Himoya o'chiq." }, 12.0, Color::rgba(255, 255, 255, 150)).padding_each(0.0, 6.0, 0.0, 6.0),
                tx(format!("Ishonchli imzolar: {} · bu qurilma: {}", num(&c["sig_count"]), if c["my_sig_trusted"].as_bool().unwrap_or(false) { "ishonchli" } else { "ishonchsiz" }), 12.0, Color::rgba(255, 255, 255, 120)),
                row(vec![
                    self.chip("Shu imzoga ishonish", false, move || m1.app_post(&p1, json!({"trust_me": true}), "Qo'shildi")),
                    self.chip("Faqat shu imzo", false, move || m2.app_post(&p2, json!({"only_me": true}), "Saqlandi")),
                    self.chip("Tozalash", false, move || m3.app_post(&p3, json!({"clear": true}), "Tozalandi")),
                ])
                .gap(8.0)
                .scroll(&self.chips_scroll2)
                .height(44.0)
                .padding_each(0.0, 10.0, 0.0, 0.0),
            ])
            .padding(16.0),
        )
        .radius(24.0);
        let mut kids = vec![ver, sig];
        if !p.msg.get().is_empty() {
            kids.insert(0, tx(p.msg.get(), 13.0, Color::rgba(255, 255, 255, 230)).bg(Color::rgba(0, 0, 0, 120)).radius(14.0).padding_xy(14.0, 8.0).margin_each(0.0, 0.0, 0.0, 8.0));
        }
        kids.push(gap_box(0.0, 32.0));
        self.scaffold("Ilova va xavfsizlik", scroll_column(&self.settings_scroll, kids).padding_each(16.0, 4.0, 16.0, 0.0))
    }
}
