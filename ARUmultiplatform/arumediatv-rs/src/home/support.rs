//! Yordam chati (Kotlin `SupportChatScreen`): admin bilan yozishma. Matn, fayl/rasm yuborish, nusxalash, o'chirish, jonli yangilanish.
//! `user_id = Some(..)` — admin boshqa foydalanuvchi suhbatini ochgan.

use super::*;
use crate::api;
use crate::data::{fmt_date, fmt_ymd};
use serde_json::{json, Value};

const S_SEND: &str = "M2.01,21L23,12 2.01,3 2,10l15,2 -15,2z";
const S_ATTACH: &str = "M16.5,6v11.5c0,2.21 -1.79,4 -4,4s-4,-1.79 -4,-4V5c0,-1.38 1.12,-2.5 2.5,-2.5s2.5,1.12 2.5,2.5v10.5c0,0.55 -0.45,1 -1,1s-1,-0.45 -1,-1V6H10v9.5c0,1.38 1.12,2.5 2.5,2.5s2.5,-1.12 2.5,-2.5V5c0,-2.21 -1.79,-4 -4,-4S7,2.79 7,5v12.5c0,3.04 2.46,5.5 5.5,5.5s5.5,-2.46 5.5,-5.5V6h-1.5z";
const S_COPY: &str = "M16,1H4c-1.1,0 -2,0.9 -2,2v14h2V3h12V1zM19,5H8c-1.1,0 -2,0.9 -2,2v14c0,1.1 0.9,2 2,2h11c1.1,0 2,-0.9 2,-2V7c0,-1.1 -0.9,-2 -2,-2zM19,21H8V7h11v14z";
const S_TRASH: &str = "M6,19c0,1.1 0.9,2 2,2h8c1.1,0 2,-0.9 2,-2V7H6v12zM19,4h-3.5l-1,-1h-5l-1,1H5v2h14V4z";
const S_CHECK: &str = "M9,16.17L4.83,12l-1.42,1.41L9,19 21,7l-1.41,-1.41z";

#[derive(Clone)]
pub struct ChatMsg {
    id: String,
    from_admin: bool,
    body: String,
    created_at: i64,
    media_url: String,
    media_type: String,
    pending: bool,
    seen: bool,
}

pub struct SupportPage {
    pub(super) user_id: Option<i64>,
    items: State<Rc<Vec<ChatMsg>>>,
    input: State<String>,
    err: State<String>,
    loaded: State<bool>,
    busy: State<bool>,
    /// Amallar menyusi ochilgan xabar identifikatori.
    menu: State<String>,
    scroll: ScrollState,
    stick: std::cell::Cell<bool>,
    ver: std::cell::Cell<i64>,
}

fn parse_msg(o: &Value) -> ChatMsg {
    let s = |k: &str| o[k].as_str().filter(|v| *v != "null").unwrap_or("").to_string();
    ChatMsg {
        id: s("id"),
        from_admin: o["from_admin"].as_bool().unwrap_or(false),
        body: s("body"),
        created_at: o["created_at"].as_i64().unwrap_or(0),
        media_url: s("media_url"),
        media_type: s("media_type"),
        pending: false,
        seen: o["seen"].as_bool().unwrap_or(false),
    }
}

fn media_kind(path: &str) -> (&'static str, &'static str) {
    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => ("image", "image/jpeg"),
        "png" => ("image", "image/png"),
        "webp" => ("image", "image/webp"),
        "mp4" | "mov" | "mkv" | "webm" => ("video", "video/mp4"),
        _ => ("file", "application/octet-stream"),
    }
}

impl Home {
    fn chat_url(user_id: Option<i64>) -> String {
        match user_id {
            None => "/api/chat".to_string(),
            Some(u) => format!("/api/chat/thread/{u}"),
        }
    }

    pub(super) fn open_support(self: &Rc<Self>, user_id: Option<i64>) {
        let p = Rc::new(SupportPage {
            user_id,
            items: State::new(Rc::new(Vec::new())),
            input: State::new(String::new()),
            err: State::new(String::new()),
            loaded: State::new(false),
            busy: State::new(false),
            menu: State::new(String::new()),
            scroll: ScrollState::new(),
            stick: std::cell::Cell::new(true),
            ver: std::cell::Cell::new(0),
        });
        self.chat_load(&p);
        self.chat_watch(&p);
        self.push(pages::Page::Support(p));
    }

    fn chat_open(&self, p: &Rc<SupportPage>) -> bool {
        matches!(self.pages.get().last(), Some(pages::Page::Support(q)) if Rc::ptr_eq(q, p))
    }

    fn chat_load(self: &Rc<Self>, p: &Rc<SupportPage>) {
        let (p2, me, session) = (p.clone(), self.clone(), self.session.clone());
        let url = Self::chat_url(p.user_id);
        spawn_bg(move || api::get_json(&url, "", Some(&session)), move |r| match r {
            Ok(j) => {
                let rows: Vec<ChatMsg> = j["items"].as_array().map(|a| a.iter().map(parse_msg).collect()).unwrap_or_default();
                // Rasm/video xabarlarining fayllari poster oqimi orqali (Telegram -> kesh) olinadi.
                {
                    let have = me.posters.borrow();
                    let mut q = me.queue.borrow_mut();
                    for m in rows.iter().filter(|m| m.media_type == "image" && !m.media_url.is_empty()) {
                        if !have.contains_key(&m.media_url) && !q.contains(&m.media_url) {
                            q.push(m.media_url.clone());
                        }
                    }
                }
                p2.items.set(Rc::new(rows));
                p2.loaded.set(true);
                p2.err.set(String::new());
                p2.stick.set(true);
                me.next_posters();
                if p2.user_id.is_none() {
                    me.unread.set(0);
                }
            }
            Err(e) => {
                if !p2.loaded.get() {
                    p2.err.set(e);
                }
            }
        });
    }

    /// `/api/chat/wait`: server ~19 soniya ushlab turadi va suhbat o'zgarganda `new` qaytaradi.
    fn chat_watch(self: &Rc<Self>, p: &Rc<SupportPage>) {
        let (me, p2, session) = (self.clone(), p.clone(), self.session.clone());
        let q = {
            let last = p.items.get().iter().filter(|m| !m.pending).map(|m| m.created_at).max().unwrap_or(0);
            format!("?ver={}&mk=-1&at=0&since={last}{}", p.ver.get(), p.user_id.map(|u| format!("&user_id={u}")).unwrap_or_default())
        };
        spawn_bg(
            move || api::get_json("/api/chat/wait", &q, Some(&session)).map_err(|_| std::thread::sleep(std::time::Duration::from_secs(3))),
            move |r| {
                if !me.chat_open(&p2) {
                    return;
                }
                if let Ok(j) = r {
                    if let Some(v) = j["ver"].as_i64() {
                        p2.ver.set(v);
                    }
                    if j["new"].as_bool().unwrap_or(false) {
                        me.chat_load(&p2);
                    }
                }
                me.chat_watch(&p2);
            },
        );
    }

    fn chat_send(self: &Rc<Self>, p: &Rc<SupportPage>, body: String, media: Option<(String, String)>) {
        let text = body.trim().to_string();
        if text.is_empty() && media.is_none() {
            return;
        }
        if media.is_none() {
            let mut v: Vec<ChatMsg> = (*p.items.get()).clone();
            v.push(ChatMsg { id: format!("tmp{}", v.len()), from_admin: p.user_id.is_some(), body: text.clone(), created_at: 0, media_url: String::new(), media_type: String::new(), pending: true, seen: false });
            p.items.set(Rc::new(v));
            p.stick.set(true);
        }
        let mut b = json!({"body": text});
        if let Some(u) = p.user_id {
            b["user_id"] = json!(u);
        }
        if let Some((f, t)) = &media {
            b["media_file"] = json!(f);
            b["media_type"] = json!(t);
        }
        let (p2, me, session) = (p.clone(), self.clone(), self.session.clone());
        spawn_bg(
            move || {
                let (code, text) = api::request("POST", "/api/chat", "", Some(&b), Some(&session)).map_err(|_| "Internet yo'q — qaytadan urinib ko'ring".to_string())?;
                if code >= 400 {
                    let j: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                    return Err(j["error"].as_str().map(String::from).unwrap_or_else(|| "Yuborilmadi".into()));
                }
                Ok(())
            },
            move |r: Result<(), String>| {
                p2.busy.set(false);
                if let Err(e) = r {
                    p2.err.set(e);
                    let v: Vec<ChatMsg> = p2.items.get().iter().filter(|m| !m.pending).cloned().collect();
                    p2.items.set(Rc::new(v));
                }
                me.chat_load(&p2);
            },
        );
    }

    fn chat_attach(self: &Rc<Self>, p: &Rc<SupportPage>) {
        let (me, pc) = (self.clone(), p.clone());
        self.pick("*/*", move |path| {
            let (kind, mime) = media_kind(&path);
            let ext = path.rsplit('.').next().unwrap_or("bin").to_lowercase();
            let uid = me.profile.get().id;
            let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
            let name = format!("chat_{uid}_{ts}.{ext}");
            let disp = path.rsplit('/').next().unwrap_or("fayl").to_string();
            pc.busy.set(true);
            pc.err.set(String::new());
            let (p2, me2, session, name2) = (pc.clone(), me.clone(), me.session.clone(), name.clone());
            spawn_bg(move || crate::backend::upload_file(&path, &name2, mime, &session, &|_, _| {}), move |r| match r {
                Ok(()) => me2.chat_send(&p2, if kind == "file" { disp } else { String::new() }, Some((name, kind.to_string()))),
                Err(e) => {
                    p2.busy.set(false);
                    p2.err.set(e);
                }
            });
        });
    }

    pub(super) fn support_page(self: &Rc<Self>, p: &Rc<SupportPage>) -> Element {
        let items = p.items.get();
        let (w, _) = window_dp();
        // Pastga yopishish: ko'rinish balandligi ma'lum bo'lgach oxirgi xabarga suriladi.
        if p.stick.get() && p.scroll.max_offset() > 0.0 {
            if (p.scroll.offset() - p.scroll.max_offset()).abs() > 1.0 {
                p.scroll.set_offset(p.scroll.max_offset());
            } else {
                p.stick.set(false);
            }
        }
        let mut kids: Vec<Element> = Vec::new();
        let start = items.len().saturating_sub(120);
        let mut last_day = String::new();
        for m in items[start..].iter() {
            let day = fmt_ymd(m.created_at);
            if !day.is_empty() && day != last_day {
                kids.push(row(vec![spacer(), tx(day.clone(), 12.0, Color::WHITE).bold().bg(Color::rgba(0, 0, 0, 90)).radius(14.0).padding_xy(12.0, 4.0), spacer()]).padding_xy(0.0, 8.0));
                last_day = day;
            }
            kids.push(self.chat_bubble(p, m, w));
        }
        kids.push(gap_box(0.0, 8.0));
        let body: Element = if !p.loaded.get() {
            if p.err.get().is_empty() { column(vec![spinner(32.0, 3.0, ACCENT2)]).center() } else { column(vec![tx(p.err.get(), 14.0, ERROR)]).center() }
        } else if items.is_empty() {
            Self::empty("M21.99,4c0,-1.1 -0.89,-2 -1.99,-2H4c-1.1,0 -2,0.9 -2,2v12c0,1.1 0.9,2 2,2h14l4,4 -0.01,-18z", "Hozircha xabar yo'q\nSavol yoki taklifingizni yozing")
        } else {
            scroll_column(&p.scroll, kids).padding_each(12.0, 4.0, 12.0, 4.0)
        };
        // Pastdagi yozish paneli: shisha.
        let (pa, ps, pm) = (p.clone(), p.clone(), p.clone());
        let me = self.clone();
        let me2 = self.clone();
        let can_send = !p.input.get().trim().is_empty() && !p.busy.get();
        let input = field(&p.input)
            .color(Color::WHITE)
            .size(16.0)
            .hint("Xabar yozing...")
            .hint_color(Color::rgba(255, 255, 255, 120))
            .accent(ACCENT2)
            .padding_xy(0.0, 10.0)
            .no_border()
            .weight(1.0);
        let bar = Self::pglass(
            row(vec![
                column(vec![if p.busy.get() { spinner(20.0, 2.0, Color::WHITE) } else { icon(S_ATTACH, 22.0).color(Color::WHITE) }]).center().width(40.0).height(40.0).on_click(move || {
                    if !pa.busy.get() {
                        me.chat_attach(&pa);
                    }
                }),
                input,
                column(vec![icon(S_SEND, 20.0).color(Color::WHITE)])
                    .center()
                    .width(42.0)
                    .height(42.0)
                    .radius(21.0)
                    .bg(if can_send { ACCENT } else { Color::rgba(255, 255, 255, 25) })
                    .on_click(move || {
                        if !ps.input.get().trim().is_empty() && !ps.busy.get() {
                            let t = ps.input.get();
                            ps.input.set(String::new());
                            me2.chat_send(&ps, t, None);
                        }
                    }),
            ])
            .gap(6.0)
            .align(Align::Center)
            .padding_each(10.0, 4.0, 6.0, 4.0),
        )
        .radius(30.0)
        .margin_each(12.0, 6.0, 12.0, 10.0);
        let mut col = vec![body.grow(1.0)];
        if !p.err.get().is_empty() && p.loaded.get() {
            col.push(tx(p.err.get(), 12.5, ERROR).padding_xy(18.0, 4.0));
        }
        col.push(bar);
        let title = match p.user_id {
            None => "Admin bilan bog'lanish".to_string(),
            Some(u) => format!("Suhbat · {u}"),
        };
        let page = self.scaffold(&title, column(col));
        let mut layers = vec![page];
        if !pm.menu.get().is_empty() {
            layers.push(self.chat_menu(&pm));
        }
        stack(layers)
    }

    fn chat_bubble(self: &Rc<Self>, p: &Rc<SupportPage>, m: &ChatMsg, w: f32) -> Element {
        // Admin ko'rinishida admin xabarlari o'ngda, oddiy foydalanuvchida — o'z xabarlari o'ngda.
        let mine = if p.user_id.is_some() { m.from_admin } else { !m.from_admin };
        let hhmm: String = fmt_date(m.created_at).chars().take(5).collect();
        let mut inner: Vec<Element> = Vec::new();
        if m.media_type == "image" {
            if let Some(f) = self.frame(&m.media_url) {
                inner.push(stack(vec![image_fill(&f)]).height((w * 0.6) * 9.0 / 16.0).width(w * 0.6));
                inner.push(gap_box(0.0, 6.0));
            } else {
                inner.push(tx("Rasm yuklanmoqda...", 13.0, Color::WHITE));
            }
        } else if !m.media_type.is_empty() {
            let label = match m.media_type.as_str() {
                "video" => "Video",
                "voice" => "Ovozli xabar",
                "round" => "Video xabar",
                "sticker" => "Stiker",
                "gif" => "GIF",
                _ => "Fayl",
            };
            inner.push(tx(label, 13.5, Color::WHITE).bold());
        }
        if !m.body.is_empty() {
            inner.push(tx(m.body.clone(), 15.0, Color::WHITE));
        }
        let mut meta = vec![spacer(), tx(hhmm, 10.5, Color::WHITE)];
        if m.pending {
            meta.push(tx("…", 10.5, Color::WHITE));
        } else if mine {
            meta.push(icon(S_CHECK, 12.0).color(Color::WHITE));
            if m.seen {
                meta.push(icon(S_CHECK, 12.0).color(Color::WHITE).margin_each(-8.0, 0.0, 0.0, 0.0));
            }
        }
        inner.push(row(meta).gap(3.0).align(Align::Center).padding_each(0.0, 2.0, 0.0, 0.0));
        let (pc, id) = (p.clone(), m.id.clone());
        let mut bubble = column(inner).padding_xy(12.0, 8.0).radius(18.0).on_click(move || pc.menu.set(id.clone()));
        bubble = if mine {
            bubble.gradient(ACCENT, ACCENT2).margin_each(52.0, 0.0, 0.0, 0.0)
        } else {
            bubble.glass(Color::rgba(255, 255, 255, 30)).border(1.0, Color::rgba(255, 255, 255, 40)).margin_each(0.0, 0.0, 52.0, 0.0)
        };
        let r = if mine { row(vec![spacer(), bubble]) } else { row(vec![bubble, spacer()]) };
        r.padding_each(0.0, 3.0, 0.0, 3.0)
    }

    fn chat_menu(self: &Rc<Self>, p: &Rc<SupportPage>) -> Element {
        let id = p.menu.get();
        let msg = p.items.get().iter().find(|m| m.id == id).cloned();
        let Some(m) = msg else {
            p.menu.set(String::new());
            return gap_box(0.0, 0.0);
        };
        let item = |ic: &'static str, label: &str, danger: bool, f: Box<dyn Fn()>| {
            row(vec![icon(ic, 20.0).color(if danger { ERROR } else { Color::WHITE }), tx(label, 15.0, if danger { ERROR } else { Color::WHITE }).bold()]).gap(14.0).align(Align::Center).padding_xy(18.0, 14.0).on_click(f)
        };
        let (p1, body) = (p.clone(), m.body.clone());
        let (p2, me, id2) = (p.clone(), self.clone(), m.id.clone());
        let can_delete = !m.pending && ((p.user_id.is_some()) || !m.from_admin);
        let mut opts = vec![item(S_COPY, "Nusxalash", false, Box::new(move || {
            uzcoders::android::copy_text(&body);
            uzcoders::android::toast("Nusxalandi");
            p1.menu.set(String::new());
        }))];
        if can_delete {
            opts.push(column(vec![]).height(1.0).bg(Color::rgba(255, 255, 255, 30)));
            opts.push(item(S_TRASH, "O'chirish", true, Box::new(move || {
                p2.menu.set(String::new());
                let v: Vec<ChatMsg> = p2.items.get().iter().filter(|x| x.id != id2).cloned().collect();
                p2.items.set(Rc::new(v));
                let (session, ids) = (me.session.clone(), vec![id2.clone()]);
                spawn_bg(move || api::request("POST", "/api/chat/messages/delete", "", Some(&json!({"ids": ids})), Some(&session)), |_| {});
            })));
        }
        let pc = p.clone();
        let panel = Self::pglass(column(opts)).radius(24.0).margin_each(40.0, 0.0, 40.0, 0.0);
        column(vec![panel]).center().bg(Color::rgba(0, 0, 0, 120)).on_click(move || pc.menu.set(String::new()))
    }
}

#[cfg(test)]
impl ChatMsg {
    pub(super) fn test(id: &str, from_admin: bool, body: &str, created_at: i64) -> ChatMsg {
        ChatMsg { id: id.into(), from_admin, body: body.into(), created_at, media_url: String::new(), media_type: String::new(), pending: false, seen: true }
    }
}

#[cfg(test)]
impl SupportPage {
    pub(super) fn items_set(&self, v: Vec<ChatMsg>) {
        self.items.set(Rc::new(v));
        self.loaded.set(true);
    }
}
