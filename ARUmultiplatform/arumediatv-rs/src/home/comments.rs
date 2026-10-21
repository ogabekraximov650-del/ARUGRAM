//! Izohlar (Kotlin `CommentsTab`): yozish, javob, layk, javoblarni ochish, o'chirish, shikoyat, saralash, jonli yangilanish.

use super::pages::{self, SeasonPage};
use super::*;
use crate::api;
use crate::data::{comments_from, fetch_comments, fetch_replies, fmt_ymd, Comment};
use serde_json::{json, Value};
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Arc;
use crate::packs_data::{self, Header};
use super::packs::PackInfo;

const C_HEART: &str = "M12,21.35l-1.45,-1.32C5.4,15.36 2,12.28 2,8.5 2,5.42 4.42,3 7.5,3c1.74,0 3.41,0.81 4.5,2.09C13.09,3.81 14.76,3 16.5,3 19.58,3 22,5.42 22,8.5c0,3.78 -3.4,6.86 -8.55,11.54L12,21.35z";
const C_HEART_O: &str = "M16.5,3c-1.74,0 -3.41,0.81 -4.5,2.09C10.91,3.81 9.24,3 7.5,3 4.42,3 2,5.42 2,8.5c0,3.78 3.4,6.86 8.55,11.54L12,21.35l1.45,-1.32C18.6,15.36 22,12.28 22,8.5 22,5.42 19.58,3 16.5,3zM12.1,18.55l-0.1,0.1 -0.1,-0.1C7.14,14.24 4,11.39 4,8.5 4,6.5 5.5,5 7.5,5c1.54,0 3.04,0.99 3.57,2.36h1.87C13.46,5.99 14.96,5 16.5,5c2,0 3.5,1.5 3.5,3.5 0,2.89 -3.14,5.74 -7.9,10.05z";
const C_SEND: &str = "M2.01,21L23,12 2.01,3 2,10l15,2 -15,2z";
const C_REPLY: &str = "M10,9V5l-7,7 7,7v-4.1c5,0 8.5,1.6 11,5.1 -1,-5 -4,-10 -11,-11z";
const C_STICKER: &str = "M21,3H3C1.9,3 1,3.9 1,5v14c0,1.1 0.9,2 2,2h13l7,-7V5C23,3.9 22.1,3 21,3zM15,19v-4c0,-0.55 0.45,-1 1,-1h4L15,19zM8.5,11C7.67,11 7,10.33 7,9.5S7.67,8 8.5,8 10,8.67 10,9.5 9.33,11 8.5,11zM15.5,11c-0.83,0 -1.5,-0.67 -1.5,-1.5S14.67,8 15.5,8 17,8.67 17,9.5 16.33,11 15.5,11z";
const C_CLOSE: &str = "M19,6.41L17.59,5 12,10.59 6.41,5 5,6.41 10.59,12 5,17.59 6.41,19 12,13.41 17.59,19 19,17.59 13.41,12z";

pub struct CommentsState {
    pub(super) items: State<Rc<Vec<Comment>>>,
    pub(super) replies: State<Rc<HashMap<String, Vec<Comment>>>>,
    pub(super) status: State<u8>,
    pub(super) input: State<String>,
    pub(super) reply_to: State<Option<(String, String)>>,
    pub(super) sort: State<usize>,
    pub(super) menu: State<Option<Comment>>,
    pub(super) report: State<Option<String>>,
    pub(super) reason: State<String>,
    pub(super) msg: State<String>,
    pub(super) busy: State<bool>,
    /// Stiker/GIF paneli.
    pub(super) panel: State<bool>,
    pub(super) panel_scroll: ScrollState,
    pub(super) lib: State<Option<Rc<Vec<PackInfo>>>>,
    pub(super) pk_sel: State<usize>,
    pub(super) pk_hdr: State<Option<Arc<Header>>>,
    pub(super) pk_thumbs: State<Rc<HashMap<i64, Rc<Image>>>>,
    mk: Cell<i64>,
    watching: Cell<bool>,
    pub(super) scroll: ScrollState,
}

impl CommentsState {
    pub(super) fn new() -> CommentsState {
        CommentsState {
            items: State::new(Rc::new(Vec::new())),
            replies: State::new(Rc::new(HashMap::new())),
            status: State::new(0),
            input: State::new(String::new()),
            reply_to: State::new(None),
            sort: State::new(0),
            menu: State::new(None),
            report: State::new(None),
            reason: State::new(String::new()),
            msg: State::new(String::new()),
            busy: State::new(false),
            panel: State::new(false),
            panel_scroll: ScrollState::new(),
            lib: State::new(None),
            pk_sel: State::new(0),
            pk_hdr: State::new(None),
            pk_thumbs: State::new(Rc::new(HashMap::new())),
            mk: Cell::new(-1),
            watching: Cell::new(false),
            scroll: ScrollState::new(),
        }
    }
}

const SORTS: [(&str, &str); 3] = [("yangi", "Yangilar"), ("layk", "Layklar"), ("javob", "Javoblar")];

impl Home {
    pub(super) fn cm_load(self: &Rc<Self>, p: &Rc<SeasonPage>) {
        let (p2, session) = (p.clone(), self.session.clone());
        let (a, s) = (p.season.anime_id, p.season.season_id);
        let sort = SORTS[p.cm.sort.get()].0;
        spawn_bg(move || fetch_comments(a, s, &session, sort), move |r| match r {
            Ok((c, mk)) => {
                p2.cm.mk.set(mk);
                p2.cm.items.set(Rc::new(c));
                p2.cm.replies.set(Rc::new(HashMap::new()));
                p2.cm.status.set(1);
            }
            Err(_) => {
                if p2.cm.status.get() == 0 {
                    p2.cm.status.set(2);
                }
            }
        });
    }

    /// `/api/comments/wait?mk=`: server o'zgarishda `new` qaytaradi (jonli yangilanish).
    pub(super) fn cm_watch(self: &Rc<Self>, p: &Rc<SeasonPage>) {
        if p.cm.watching.replace(true) {
            return;
        }
        fn step(me: Rc<Home>, p: Rc<SeasonPage>) {
            let (m2, p2, session) = (me.clone(), p.clone(), me.session.clone());
            let mk = p.cm.mk.get();
            if mk < 0 {
                spawn_bg(|| std::thread::sleep(std::time::Duration::from_secs(2)), move |_| {
                    if !p2.pl.closed.get() && m2.pages.get().iter().any(|pg| matches!(pg, pages::Page::Season(q) if Rc::ptr_eq(q, &p2))) {
                        step(m2, p2);
                    } else {
                        p2.cm.watching.set(false);
                    }
                });
                return;
            }
            spawn_bg(
                move || {
                    let t0 = std::time::Instant::now();
                    let r = api::get_json("/api/comments/wait", &format!("?mk={mk}"), Some(&session));
                    // Server darhol qaytarsa (uzoq so'rov emas) — so'rovlar to'xtovsiz ketmasin.
                    if t0.elapsed().as_millis() < 1500 {
                        std::thread::sleep(std::time::Duration::from_secs(4));
                    }
                    r
                },
                move |r| {
                    if p2.pl.closed.get() || !m2.pages.get().iter().any(|pg| matches!(pg, pages::Page::Season(q) if Rc::ptr_eq(q, &p2))) {
                        p2.cm.watching.set(false);
                        return;
                    }
                    if let Ok(j) = r {
                        if j["new"].as_bool().unwrap_or(false) {
                            if let Some(m) = j["mk"].as_i64() {
                                p2.cm.mk.set(m.max(mk));
                            }
                            m2.cm_load(&p2);
                        }
                    }
                    step(m2, p2);
                },
            );
        }
        step(self.clone(), p.clone());
    }

    fn cm_send(self: &Rc<Self>, p: &Rc<SeasonPage>) {
        self.cm_send_media(p, None);
    }

    /// Matn yoki stiker/GIF (`media`: (`pk_<to'plam>_<element>`, `sticker`/`gif`)) yuboradi.
    fn cm_send_media(self: &Rc<Self>, p: &Rc<SeasonPage>, media: Option<(String, &'static str)>) {
        let text = if media.is_some() { String::new() } else { p.cm.input.get().trim().to_string() };
        if (text.is_empty() && media.is_none()) || p.cm.busy.get() {
            return;
        }
        let parent = p.cm.reply_to.get().map(|r| r.0).unwrap_or_default();
        p.cm.busy.set(true);
        let mut b = json!({"anime_id": p.season.anime_id, "season_id": p.season.season_id, "parent_id": parent, "body": text});
        if let Some((f, t)) = &media {
            b["media_file"] = json!(f);
            b["media_type"] = json!(t);
        }
        p.cm.panel.set(false);
        let (p2, me, session) = (p.clone(), self.clone(), self.session.clone());
        spawn_bg(
            move || {
                let (code, body) = api::request("POST", "/api/comments", "", Some(&b), Some(&session)).map_err(|_| "Internet yo'q — qaytadan urinib ko'ring".to_string())?;
                let j: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
                if code >= 400 {
                    return Err(j["error"].as_str().map(String::from).unwrap_or_else(|| "Izoh yuborilmadi".into()));
                }
                Ok(Comment::from_json(&j))
            },
            move |r: Result<Comment, String>| {
                p2.cm.busy.set(false);
                match r {
                    Ok(c) => {
                        p2.cm.input.set(String::new());
                        p2.cm.reply_to.set(None);
                        if c.parent_id.is_empty() {
                            let mut v = vec![c];
                            v.extend((*p2.cm.items.get()).clone());
                            p2.cm.items.set(Rc::new(v));
                        } else {
                            let mut items = (*p2.cm.items.get()).clone();
                            for i in items.iter_mut().filter(|i| i.id == c.parent_id) {
                                i.replies += 1;
                            }
                            p2.cm.items.set(Rc::new(items));
                            let mut rep = (*p2.cm.replies.get()).clone();
                            if let Some(l) = rep.get_mut(&c.parent_id) {
                                l.push(c);
                            }
                            p2.cm.replies.set(Rc::new(rep));
                        }
                        let _ = &me;
                    }
                    Err(e) => p2.cm.msg.set(e),
                }
            },
        );
    }

    /// Izohdagi stiker/GIF (`pk_<to'plam>_<element>`) rasmi; tayyor bo'lmasa fonda yuklanadi.
    fn cm_media_el(self: &Rc<Self>, file: &str, kind: &str) -> Option<Element> {
        let rest = file.strip_prefix("pk_")?;
        let (a, b) = rest.split_once('_')?;
        let key = (a.parse::<i64>().ok()?, b.parse::<i64>().ok()?);
        let _ = self.gate_rev.get();
        let size = if kind == "sticker" { 130.0 } else { 180.0 };
        let cached = self.pk_imgs.borrow().get(&key).cloned();
        match cached {
            Some(Some(img)) => {
                let k = (size / img.w as f32).min(size / img.h as f32);
                Some(image(&img, img.w as f32 * k, img.h as f32 * k).margin_each(0.0, 4.0, 0.0, 4.0))
            }
            Some(None) => Some(tx(if kind == "gif" { "GIF" } else { "Stiker" }, 12.0, TEXT_DIM).padding_xy(0.0, 4.0)),
            None => {
                self.pk_imgs.borrow_mut().insert(key, None);
                let (me, session) = (self.clone(), self.session.clone());
                spawn_bg(move || packs_data::item_image(&session, key.0, key.1), move |r| {
                    if let Some(img) = r {
                        me.pk_imgs.borrow_mut().insert(key, Some(Rc::new(img)));
                        me.gate_rev.set(me.gate_rev.get() + 1);
                    }
                });
                Some(spinner(18.0, 2.0, ACCENT2).padding_xy(0.0, 8.0))
            }
        }
    }

    fn cm_panel_open(self: &Rc<Self>, p: &Rc<SeasonPage>) {
        if p.cm.lib.get().is_some() {
            return;
        }
        let (p2, me, session) = (p.clone(), self.clone(), self.session.clone());
        spawn_bg(move || api::get_json("/api/packs/library", "", Some(&session)), move |r| {
            let mut v: Vec<PackInfo> = Vec::new();
            if let Ok(j) = r {
                for k in ["mine", "subs"] {
                    if let Some(a) = j[k].as_array() {
                        v.extend(a.iter().map(PackInfo::from).filter(|p| (p.kind == "sticker" || p.kind == "gif") && !p.file.is_empty() && p.items > 0));
                    }
                }
            }
            let first = v.first().cloned();
            p2.cm.lib.set(Some(Rc::new(v)));
            if let Some(f) = first {
                me.cm_pick_pack(&p2, 0, f);
            }
        });
    }

    fn cm_pick_pack(self: &Rc<Self>, p: &Rc<SeasonPage>, idx: usize, pk: PackInfo) {
        p.cm.pk_sel.set(idx);
        p.cm.pk_hdr.set(None);
        p.cm.pk_thumbs.set(Rc::new(HashMap::new()));
        let (p2, session) = (p.clone(), self.session.clone());
        spawn_bg(
            move || -> Option<(Arc<Header>, Vec<(i64, Image)>)> {
                let h = packs_data::header(&session, &pk.file, pk.id, pk.version).ok()?;
                let t = packs_data::thumbs(&session, &pk.file, &h);
                Some((h, t))
            },
            move |r| {
                if let Some((h, t)) = r {
                    if p2.cm.pk_sel.get() == idx {
                        p2.cm.pk_thumbs.set(Rc::new(t.into_iter().map(|(i, im)| (i, Rc::new(im))).collect()));
                        p2.cm.pk_hdr.set(Some(h));
                    }
                }
            },
        );
    }

    /// Stiker/GIF tanlash paneli (o'z to'plamlarim va obunalarim).
    fn cm_panel(self: &Rc<Self>, p: &Rc<SeasonPage>) -> Element {
        let Some(lib) = p.cm.lib.get() else { return column(vec![spinner(22.0, 2.0, ACCENT2)]).center().padding(16.0) };
        if lib.is_empty() {
            return tx("Stiker yoki GIF to'plamingiz yo'q — Profil > To'plamlar orqali qo'shing", 12.5, Color::WHITE).padding(14.0);
        }
        let tabs: Vec<Element> = lib
            .iter()
            .enumerate()
            .map(|(i, pk)| {
                let (me, pc, pk2) = (self.clone(), p.clone(), pk.clone());
                self.chip(&pk.title, p.cm.pk_sel.get() == i, move || me.cm_pick_pack(&pc, i, pk2.clone()))
            })
            .collect();
        let mut kids = vec![row(tabs).gap(8.0).scroll(&self.chips_scroll2).height(44.0)];
        match p.cm.pk_hdr.get() {
            None => kids.push(column(vec![spinner(22.0, 2.0, ACCENT2)]).center().padding(16.0)),
            Some(h) => {
                let thumbs = p.cm.pk_thumbs.get();
                let kind = lib.get(p.cm.pk_sel.get()).map(|k| k.kind.clone()).unwrap_or_default();
                let (w, _) = window_dp();
                let tile = ((w - 24.0 - 24.0 - 8.0 * 3.0) / 4.0).floor();
                let mut rows: Vec<Element> = Vec::new();
                for ch in h.items.chunks(4).take(40) {
                    let r: Vec<Element> = ch
                        .iter()
                        .map(|it| {
                            let inner: Element = match thumbs.get(&it.id) {
                                Some(i) => {
                                    let k = ((tile - 10.0) / i.w as f32).min((tile - 10.0) / i.h as f32);
                                    image(i, i.w as f32 * k, i.h as f32 * k)
                                }
                                None => column(vec![]),
                            };
                            let (me, pc, pid, iid, kd) = (self.clone(), p.clone(), h.id, it.id, if kind == "gif" { "gif" } else { "sticker" });
                            column(vec![inner]).center().width(tile).height(tile).radius(14.0).bg(Color::rgba(255, 255, 255, 14)).on_click(move || me.cm_send_media(&pc, Some((format!("pk_{pid}_{iid}"), kd))))
                        })
                        .collect();
                    rows.push(row(r).gap(8.0).margin_each(0.0, 0.0, 0.0, 8.0));
                }
                kids.push(scroll_column(&p.cm.panel_scroll, rows).height(tile * 2.6));
            }
        }
        column(kids).padding_each(10.0, 10.0, 10.0, 4.0)
    }

    fn cm_like(self: &Rc<Self>, p: &Rc<SeasonPage>, id: String) {
        let flip = |c: &mut Comment| {
            c.liked = !c.liked;
            c.likes = if c.liked { c.likes + 1 } else { (c.likes - 1).max(0) };
        };
        let mut items = (*p.cm.items.get()).clone();
        items.iter_mut().filter(|c| c.id == id).for_each(flip);
        p.cm.items.set(Rc::new(items));
        let mut rep = (*p.cm.replies.get()).clone();
        rep.values_mut().for_each(|l| l.iter_mut().filter(|c| c.id == id).for_each(flip));
        p.cm.replies.set(Rc::new(rep));
        let (p2, session, id2) = (p.clone(), self.session.clone(), id);
        spawn_bg(
            move || {
                let (code, body) = api::request("POST", "/api/comments/like", "", Some(&json!({"id": id2})), Some(&session))?;
                if code >= 400 { Err((id2, String::from("Layk yuborilmadi"))) } else { Ok((id2, serde_json::from_str::<Value>(&body).unwrap_or(Value::Null))) }.map_err(|e: (String, String)| e.1).map(|v| v)
            },
            move |r: Result<(String, Value), String>| {
                if let Err(e) = r {
                    p2.cm.msg.set(e);
                }
            },
        );
    }

    fn cm_toggle_replies(self: &Rc<Self>, p: &Rc<SeasonPage>, id: String) {
        let mut rep = (*p.cm.replies.get()).clone();
        if rep.remove(&id).is_some() {
            p.cm.replies.set(Rc::new(rep));
            return;
        }
        let (p2, session, id2, a, s) = (p.clone(), self.session.clone(), id.clone(), p.season.anime_id, p.season.season_id);
        spawn_bg(move || fetch_replies(a, s, &id2, &session).map(|l| (id2, l)), move |r| {
            if let Ok((id, list)) = r {
                let mut rep = (*p2.cm.replies.get()).clone();
                rep.insert(id, list);
                p2.cm.replies.set(Rc::new(rep));
            }
        });
    }

    fn cm_delete(self: &Rc<Self>, p: &Rc<SeasonPage>, id: String) {
        let items: Vec<Comment> = p.cm.items.get().iter().filter(|c| c.id != id).cloned().collect();
        p.cm.items.set(Rc::new(items));
        let mut rep = (*p.cm.replies.get()).clone();
        rep.values_mut().for_each(|l| l.retain(|c| c.id != id));
        p.cm.replies.set(Rc::new(rep));
        let (p2, session) = (p.clone(), self.session.clone());
        spawn_bg(move || api::request("DELETE", &format!("/api/comments/{id}"), "", None, Some(&session)), move |r| {
            if !matches!(r, Ok((c, _)) if c < 400) {
                p2.cm.msg.set("O'chirilmadi".into());
            }
        });
    }

    fn cm_item(self: &Rc<Self>, p: &Rc<SeasonPage>, c: &Comment, reply: bool) -> Element {
        let letter: String = c.name.trim_start_matches('@').chars().take(1).flat_map(|x| x.to_uppercase()).collect();
        let body = if !c.body.is_empty() { c.body.clone() } else if !c.media.is_empty() && c.media_file.is_empty() { format!("[{}]", c.media) } else { String::new() };
        let media_el: Option<Element> = self.cm_media_el(&c.media_file, &c.media);
        let (me, pc, id) = (self.clone(), p.clone(), c.id.clone());
        let like = row(vec![icon(if c.liked { C_HEART } else { C_HEART_O }, 16.0).color(if c.liked { ACCENT2 } else { TEXT_DIM }), tx(c.likes.to_string(), 12.0, if c.liked { ACCENT2 } else { TEXT_DIM }).bold()])
            .gap(4.0)
            .align(Align::Center)
            .padding_xy(4.0, 4.0)
            .on_click(move || me.cm_like(&pc, id.clone()));
        let mut actions = vec![like];
        if !reply {
            let (pr, name, id) = (p.clone(), c.name.clone(), c.id.clone());
            actions.push(row(vec![icon(C_REPLY, 14.0).color(TEXT_DIM), tx("Javob", 12.0, TEXT_DIM).bold()]).gap(4.0).align(Align::Center).padding_xy(4.0, 4.0).on_click(move || pr.cm.reply_to.set(Some((id.clone(), name.clone())))));
            if c.replies > 0 {
                let (me, pc, id) = (self.clone(), p.clone(), c.id.clone());
                let open = p.cm.replies.get().contains_key(&c.id);
                actions.push(tx(if open { "Yopish".to_string() } else { format!("{} javob", c.replies) }, 12.0, ACCENT2).bold().padding_xy(4.0, 4.0).on_click(move || me.cm_toggle_replies(&pc, id.clone())));
            }
        }
        let (pm, cc) = (p.clone(), c.clone());
        let card = Self::pglass(
            row(vec![
                column(vec![tx(letter, 15.0, ACCENT2).bold()]).center().width(38.0).height(38.0).radius(19.0).bg(ACCENT.with_opacity(0.22)).on_click({
                    let (me, uid) = (self.clone(), c.user_id);
                    move || me.open_public_profile(uid)
                }),
                column(vec![
                    row(vec![tx(c.name.clone(), 14.0, Color::WHITE).bold().weight(1.0), tx(fmt_ymd(c.created_at), 11.0, TEXT_DIM)]),
                    gap_box(0.0, 3.0),
                    media_el.unwrap_or_else(|| column(vec![])),
                    if body.is_empty() { column(vec![]) } else { tx(body, 14.0, Color::WHITE) },
                    gap_box(0.0, 4.0),
                    row(actions).gap(14.0).align(Align::Center),
                ])
                .weight(1.0),
            ])
            .gap(10.0)
            .padding(12.0),
        )
        .radius(20.0)
        .on_click(move || pm.cm.menu.set(Some(cc.clone())));
        card.margin_each(if reply { 28.0 } else { 0.0 }, 0.0, 0.0, 8.0)
    }

    pub(super) fn season_comments_tab(self: &Rc<Self>, p: &Rc<SeasonPage>) -> Element {
        self.cm_watch(p);
        let items = p.cm.items.get();
        let sorts: Vec<Element> = SORTS
            .iter()
            .enumerate()
            .map(|(i, (_, l))| {
                let (ps, me, pc) = (p.cm.sort.clone(), self.clone(), p.clone());
                self.chip(l, p.cm.sort.get() == i, move || {
                    ps.set(i);
                    me.cm_load(&pc);
                })
            })
            .collect();
        let mut kids: Vec<Element> = vec![row(sorts).gap(8.0).padding_each(0.0, 0.0, 0.0, 10.0)];
        let replies = p.cm.replies.get();
        if items.is_empty() {
            kids.push(match p.cm.status.get() {
                0 => column(vec![spinner(28.0, 3.0, ACCENT2)]).center().padding_xy(0.0, 40.0),
                2 => tx("Izohlar yuklanmadi", 14.0, TEXT_DIM).padding_xy(0.0, 30.0),
                _ => tx("Hali izoh yo'q — birinchi bo'ling", 14.0, TEXT_DIM).padding_xy(0.0, 30.0),
            });
        }
        for c in items.iter() {
            kids.push(self.cm_item(p, c, false));
            if let Some(list) = replies.get(&c.id) {
                for r in list {
                    kids.push(self.cm_item(p, r, true));
                }
            }
        }
        kids.push(gap_box(0.0, 16.0));
        // Yozish paneli.
        let (ps, pd) = (p.clone(), p.clone());
        let me = self.clone();
        let can = !p.cm.input.get().trim().is_empty() && !p.cm.busy.get();
        let mut bar_kids: Vec<Element> = Vec::new();
        if let Some((_, name)) = p.cm.reply_to.get() {
            let pc = p.clone();
            bar_kids.push(row(vec![tx(format!("Javob: {name}"), 12.0, ACCENT2).bold().weight(1.0), icon(C_CLOSE, 16.0).color(TEXT_DIM).on_click(move || pc.cm.reply_to.set(None))]).padding_each(8.0, 4.0, 8.0, 2.0));
        }
        if p.cm.panel.get() {
            bar_kids.push(self.cm_panel(p));
        }
        let (pt, me_t) = (p.clone(), self.clone());
        bar_kids.push(
            row(vec![
                column(vec![icon(C_STICKER, 24.0).color(Color::WHITE)]).center().width(44.0).height(44.0).on_click(move || {
                    let on = !pt.cm.panel.get();
                    pt.cm.panel.set(on);
                    if on {
                        me_t.cm_panel_open(&pt);
                    }
                }),
                field(&p.cm.input).color(Color::WHITE).size(15.0).hint("Izoh yozing...").hint_color(Color::rgba(255, 255, 255, 120)).accent(ACCENT2).padding_xy(4.0, 14.0).no_border().weight(1.0),
                column(vec![if p.cm.busy.get() { spinner(18.0, 2.0, Color::WHITE) } else { icon(C_SEND, 20.0).color(Color::WHITE) }]).center().width(40.0).height(40.0).radius(20.0).bg(if can { ACCENT } else { Color::rgba(255, 255, 255, 25) }).on_click(move || me.cm_send(&ps)),
            ])
            .gap(6.0)
            .align(Align::Center)
            .padding_each(6.0, 4.0, 8.0, 4.0),
        );
        let _ = pd;
        let mut col = vec![scroll_column(&p.cm.scroll, kids).padding_each(16.0, 4.0, 16.0, 0.0).grow(1.0)];
        if !p.cm.msg.get().is_empty() {
            let pm = p.clone();
            col.push(tx(p.cm.msg.get(), 12.5, ERROR).padding_xy(18.0, 2.0).on_click(move || pm.cm.msg.set(String::new())));
        }
        col.push(column(bar_kids).lens(Color::rgba(18, 18, 24, 150), 1.0).radius(28.0).margin_each(12.0, 4.0, 12.0, 8.0));
        column(col)
    }

    /// Izoh amallari va shikoyat oynasi.
    pub(super) fn cm_overlay(self: &Rc<Self>, p: &Rc<SeasonPage>) -> Option<Element> {
        if let Some(id) = p.cm.report.get() {
            let (pc, pd, me) = (p.clone(), p.clone(), self.clone());
            let panel = column(vec![
                tx("Shikoyat", 18.0, Color::WHITE).bold(),
                gap_box(0.0, 10.0),
                field(&p.cm.reason).color(Color::WHITE).size(15.0).hint("Sababini batafsil yozing (kamida 10 belgi)").hint_color(Color::rgba(255, 255, 255, 110)).accent(ACCENT2).padding_xy(14.0, 12.0).glass(Color::rgba(255, 255, 255, 22)).border(1.0, Color::TRANSPARENT).radius(14.0),
                gap_box(0.0, 16.0),
                row(vec![
                    spacer(),
                    self.chip("Bekor qilish", false, move || pc.cm.report.set(None)),
                    self.chip("Yuborish", true, move || {
                        let reason = pd.cm.reason.get().trim().to_string();
                        if reason.chars().count() < 10 {
                            pd.cm.msg.set("Iltimos, shikoyat sababini batafsilroq yozing".into());
                            return;
                        }
                        pd.cm.report.set(None);
                        pd.cm.reason.set(String::new());
                        let (session, id2, p3) = (me.session.clone(), id.clone(), pd.clone());
                        spawn_bg(move || api::request("POST", "/api/reports", "", Some(&json!({"kind": "comment", "target_id": id2, "reason": reason})), Some(&session)), move |r| {
                            p3.cm.msg.set(if matches!(r, Ok((c, _)) if c < 400) { "Shikoyat qabul qilindi".to_string() } else { "Shikoyat yuborilmadi".to_string() });
                        });
                    }),
                ])
                .gap(8.0),
            ])
            .lens(Color::rgba(18, 18, 24, 120), GLASS_ZOOM)
            .radius(28.0)
            .padding(20.0)
            .margin_each(20.0, 0.0, 20.0, 0.0);
            let pc = p.clone();
            return Some(column(vec![panel]).center().bg(Color::rgba(0, 0, 0, 130)).on_click(move || pc.cm.report.set(None)));
        }
        let c = p.cm.menu.get()?;
        let mine = c.user_id == self.profile.get().id;
        let admin = self.profile.get().is_admin;
        let item = |label: &str, danger: bool, f: Box<dyn Fn()>| tx(label, 15.0, if danger { ERROR } else { Color::WHITE }).padding_xy(18.0, 13.0).on_click(f);
        let (p1, body) = (p.clone(), c.body.clone());
        let mut opts = vec![item("Nusxalash", false, Box::new(move || {
            uzcoders::android::copy_text(&body);
            uzcoders::android::toast("Nusxalandi");
            p1.cm.menu.set(None);
        }))];
        if !mine {
            let (p2, id) = (p.clone(), c.id.clone());
            opts.push(item("Shikoyat qilish", false, Box::new(move || {
                p2.cm.menu.set(None);
                p2.cm.report.set(Some(id.clone()));
            })));
        }
        if mine || admin {
            let (p3, me, id) = (p.clone(), self.clone(), c.id.clone());
            opts.push(item("O'chirish", true, Box::new(move || {
                p3.cm.menu.set(None);
                me.cm_delete(&p3, id.clone());
            })));
        }
        let pc = p.clone();
        let panel = Self::pglass(column(opts)).radius(24.0).margin_each(40.0, 0.0, 40.0, 0.0);
        Some(column(vec![panel]).center().bg(Color::rgba(0, 0, 0, 120)).on_click(move || pc.cm.menu.set(None)))
    }
}

#[allow(dead_code)]
fn _unused(j: &Value) -> Vec<Comment> {
    comments_from(j)
}
