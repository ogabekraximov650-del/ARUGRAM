//! Emoji, GIF va stiker to'plamlari (Kotlin `MyPacksScreen` / `PackDetailScreen`): mening / obunalar / ommaviy, yaratish, rasm qo'shish, obuna.

use super::*;
use crate::api;
use crate::data::format_bytes;
use crate::packs_data::{self, Header, Item};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

const PK_ADD: &str = "M19,13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z";

#[derive(Clone, Default)]
pub struct PackInfo {
    pub(super) id: i64,
    pub(super) kind: String,
    pub(super) title: String,
    pub(super) version: i64,
    pub(super) items: i64,
    pub(super) bytes: i64,
    pub(super) owner_name: String,
    pub(super) sub: bool,
    pub(super) file: String,
}

impl PackInfo {
    pub(super) fn from(o: &Value) -> PackInfo {
        let n = |k: &str| o[k].as_i64().unwrap_or(0);
        PackInfo {
            id: n("id"),
            kind: o["kind"].as_str().unwrap_or("sticker").into(),
            title: o["title"].as_str().unwrap_or("").into(),
            version: n("version"),
            items: n("items"),
            bytes: n("bytes"),
            owner_name: o["owner_name"].as_str().filter(|v| *v != "null").unwrap_or("").into(),
            sub: o["sub"].as_bool().unwrap_or(false),
            file: o["file"].as_str().filter(|v| *v != "null").unwrap_or("").into(),
        }
    }
}

fn kind_single(k: &str) -> &'static str {
    match k {
        "emoji" => "Emoji",
        "gif" => "GIF",
        _ => "Stiker",
    }
}

fn kind_plural(k: &str) -> &'static str {
    match k {
        "emoji" => "Emojilar",
        "gif" => "GIFlar",
        _ => "Stikerlar",
    }
}

pub struct PacksPage {
    tab: State<usize>,
    mine: State<Rc<Vec<PackInfo>>>,
    subs: State<Rc<Vec<PackInfo>>>,
    ops: State<Rc<Vec<Value>>>,
    browse: State<Rc<Vec<PackInfo>>>,
    browse_kind: State<String>,
    loading: State<bool>,
    create: State<bool>,
    new_kind: State<String>,
    new_title: State<String>,
    detail: State<Option<PackInfo>>,
    hdr: State<Option<Arc<Header>>>,
    thumbs: State<Rc<HashMap<i64, Rc<Image>>>>,
    derr: State<String>,
    big: State<Option<(Item, Option<Rc<Image>>)>>,
    msg: State<String>,
    scroll: ScrollState,
}

impl PacksPage {
    pub(super) fn has_detail(&self) -> bool {
        self.detail.get().is_some()
    }
    pub(super) fn close_detail(&self) {
        if self.big.get().is_some() {
            self.big.set(None);
            return;
        }
        self.detail.set(None);
    }
}

impl Home {
    pub(super) fn open_packs(self: &Rc<Self>) {
        let p = Rc::new(PacksPage {
            tab: State::new(0),
            mine: State::new(Rc::new(Vec::new())),
            subs: State::new(Rc::new(Vec::new())),
            ops: State::new(Rc::new(Vec::new())),
            browse: State::new(Rc::new(Vec::new())),
            browse_kind: State::new(String::new()),
            loading: State::new(true),
            create: State::new(false),
            new_kind: State::new("sticker".into()),
            new_title: State::new(String::new()),
            detail: State::new(None),
            hdr: State::new(None),
            thumbs: State::new(Rc::new(HashMap::new())),
            derr: State::new(String::new()),
            big: State::new(None),
            msg: State::new(String::new()),
            scroll: ScrollState::new(),
        });
        self.packs_load(&p);
        self.push(pages::Page::Packs(p));
    }

    fn packs_load(self: &Rc<Self>, p: &Rc<PacksPage>) {
        p.loading.set(true);
        let (p2, session) = (p.clone(), self.session.clone());
        spawn_bg(move || api::get_json("/api/packs/library", "", Some(&session)), move |r| {
            p2.loading.set(false);
            match r {
                Ok(j) => {
                    let list = |k: &str| j[k].as_array().map(|a| a.iter().map(PackInfo::from).collect::<Vec<_>>()).unwrap_or_default();
                    p2.mine.set(Rc::new(list("mine")));
                    p2.subs.set(Rc::new(list("subs")));
                    p2.ops.set(Rc::new(j["ops"].as_array().cloned().unwrap_or_default()));
                }
                Err(e) => p2.msg.set(e),
            }
        });
    }

    fn packs_browse(self: &Rc<Self>, p: &Rc<PacksPage>) {
        p.loading.set(true);
        let (p2, session, kind) = (p.clone(), self.session.clone(), p.browse_kind.get());
        spawn_bg(
            move || api::get_json("/api/packs/public", &if kind.is_empty() { String::new() } else { format!("?kind={kind}") }, Some(&session)),
            move |r| {
                p2.loading.set(false);
                if let Ok(j) = r {
                    p2.browse.set(Rc::new(j["packs"].as_array().map(|a| a.iter().map(PackInfo::from).collect()).unwrap_or_default()));
                }
            },
        );
    }

    /// Amallar 3 soniya kutib bitta paket bo'lib yuboriladi, so'ng kutubxona yangilanadi.
    fn packs_flush_soon(self: &Rc<Self>, p: &Rc<PacksPage>) {
        let (me, p2) = (self.clone(), p.clone());
        spawn_bg(|| { std::thread::sleep(std::time::Duration::from_secs(3)); crate::sync::flush(); }, move |_| me.packs_load(&p2));
    }

    pub(super) fn packs_page(self: &Rc<Self>, p: &Rc<PacksPage>) -> Element {
        if let Some(d) = p.detail.get() {
            return self.pack_detail(p, &d);
        }
        let tab = p.tab.get();
        let tabs: Vec<Element> = ["Mening", "Obunalar", "Ommaviy"]
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let (pt, me, pc) = (p.tab.clone(), self.clone(), p.clone());
                let e = column(vec![tx(*l, 13.0, if i == tab { Color::WHITE } else { Color::rgba(255, 255, 255, 170) }).bold()]).center().weight(1.0).padding_xy(4.0, 11.0).radius(20.0).on_click(move || {
                    pt.set(i);
                    if i == 2 {
                        me.packs_browse(&pc);
                    }
                });
                if i == tab { e.gradient(ACCENT, ACCENT2) } else { e }
            })
            .collect();
        let mut head: Vec<Element> = vec![Self::pglass(row(tabs).gap(4.0).padding_xy(6.0, 6.0)).radius(28.0).margin_each(0.0, 0.0, 0.0, 10.0)];
        if tab == 2 {
            let chips: Vec<Element> = ["", "sticker", "emoji", "gif"]
                .iter()
                .map(|k| {
                    let (pk, me, pc, k) = (p.browse_kind.clone(), self.clone(), p.clone(), k.to_string());
                    let on = p.browse_kind.get() == k;
                    self.chip(if k.is_empty() { "Hammasi" } else { kind_plural(&k) }, on, move || {
                        pk.set(k.clone());
                        me.packs_browse(&pc);
                    })
                })
                .collect();
            head.push(row(chips).gap(8.0).scroll(&self.chips_scroll2).height(44.0).margin_each(0.0, 0.0, 0.0, 8.0));
        }
        if !p.msg.get().is_empty() {
            let pm = p.clone();
            head.push(tx(p.msg.get(), 13.0, Color::WHITE).bg(Color::rgba(0, 0, 0, 120)).radius(14.0).padding_xy(14.0, 8.0).margin_each(0.0, 0.0, 0.0, 8.0).on_click(move || pm.msg.set(String::new())));
        }
        let list: Rc<Vec<PackInfo>> = match tab {
            0 => p.mine.get(),
            1 => p.subs.get(),
            _ => p.browse.get(),
        };
        let body: Element = if list.is_empty() {
            if p.loading.get() {
                column(vec![spinner(32.0, 3.0, ACCENT2)]).center()
            } else {
                Self::empty(PK_ADD, match tab { 0 => "Hali to'plam yo'q — pastdagi tugma bilan yarating", 1 => "Obuna bo'lgan to'plam yo'q", _ => "Ommaviy to'plam topilmadi" })
            }
        } else {
            let mut kids: Vec<Element> = Vec::new();
            for pk in list.iter() {
                let (pc, pk2, me) = (p.clone(), pk.clone(), self.clone());
                let mut r = vec![
                    column(vec![tx(kind_single(&pk.kind).chars().take(1).collect::<String>(), 18.0, ACCENT2).black()]).center().width(52.0).height(52.0).radius(14.0).bg(ACCENT.with_opacity(0.2)),
                    column(vec![
                        tx(pk.title.clone(), 15.0, Color::WHITE).bold(),
                        tx(format!("{} · {} ta{}{}", kind_single(&pk.kind), pk.items, if pk.bytes > 0 { format!(" · {}", format_bytes(pk.bytes)) } else { String::new() }, if pk.owner_name.is_empty() { String::new() } else { format!(" · {}", pk.owner_name) }), 12.0, Color::WHITE),
                    ])
                    .weight(1.0),
                ];
                if pk.sub {
                    r.push(tx("Obuna", 11.0, GOLD).bold());
                }
                kids.push(Self::pglass(row(r).gap(12.0).align(Align::Center).padding(12.0)).radius(22.0).margin_each(0.0, 0.0, 0.0, 10.0).on_click(move || me.open_pack(&pc, pk2.clone())));
            }
            kids.push(gap_box(0.0, 90.0));
            scroll_column(&p.scroll, kids)
        };
        let mut col = vec![column(head), body.grow(1.0)];
        if tab == 0 {
            let pc = p.clone();
            col.push(
                row(vec![spacer(), column(vec![tx("+ Yangi to'plam", 14.0, Color::WHITE).bold()]).center().padding_xy(22.0, 14.0).radius(26.0).gradient(ACCENT, ACCENT2).on_click(move || pc.create.set(true)), spacer()]).padding_each(0.0, 8.0, 0.0, 12.0),
            );
        }
        let mut layers = vec![self.scaffold("Emoji, GIF va stikerlar", column(col).padding_each(16.0, 4.0, 16.0, 0.0))];
        if p.create.get() {
            layers.push(self.pack_create_dialog(p));
        }
        stack(layers)
    }

    fn pack_create_dialog(self: &Rc<Self>, p: &Rc<PacksPage>) -> Element {
        let kinds: Vec<Element> = ["sticker", "emoji", "gif"]
            .iter()
            .map(|k| {
                let (pk, k) = (p.new_kind.clone(), k.to_string());
                let on = p.new_kind.get() == k;
                self.chip(kind_single(&k), on, move || pk.set(k.clone()))
            })
            .collect();
        let (pc, pd, me) = (p.clone(), p.clone(), self.clone());
        let can = !p.new_title.get().trim().is_empty() && p.mine.get().len() < 30;
        let panel = column(vec![
            tx("Yangi to'plam", 18.0, Color::WHITE).bold(),
            gap_box(0.0, 12.0),
            row(kinds).gap(8.0),
            gap_box(0.0, 10.0),
            field(&p.new_title).color(Color::WHITE).size(16.0).hint("Nomi").hint_color(Color::rgba(255, 255, 255, 110)).accent(ACCENT2).padding_xy(14.0, 12.0).glass(Color::rgba(255, 255, 255, 22)).border(1.0, Color::TRANSPARENT).radius(14.0),
            gap_box(0.0, 16.0),
            row(vec![
                spacer(),
                self.chip("Bekor qilish", false, move || pc.create.set(false)),
                self.chip("Yaratish", can, move || {
                    if !can {
                        return;
                    }
                    pd.create.set(false);
                    let id = {
                        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(1);
                        ((t % 0x7fff_ffff) as i64 + 1) * 2_097_152 + (t / 7 % 2_097_152) as i64
                    };
                    crate::sync::put_pack(format!("p:new:{id}"), json!({"op": "new", "id": id, "kind": pd.new_kind.get(), "title": pd.new_title.get().trim()}));
                    pd.new_title.set(String::new());
                    me.packs_flush_soon(&pd);
                    let mut v: Vec<PackInfo> = (*pd.mine.get()).clone();
                    v.insert(0, PackInfo { id, kind: pd.new_kind.get(), title: "…".into(), ..Default::default() });
                    pd.mine.set(Rc::new(v));
                }),
            ])
            .gap(8.0),
        ])
        .lens(Color::rgba(255, 255, 255, 14), GLASS_ZOOM)
        .radius(28.0)
        .padding(20.0)
        .margin_each(20.0, 0.0, 20.0, 0.0);
        let pc = p.clone();
        column(vec![panel]).center().bg(Color::rgba(0, 0, 0, 45)).on_click(move || pc.create.set(false))
    }

    fn pack_detail(self: &Rc<Self>, p: &Rc<PacksPage>, d: &PackInfo) -> Element {
        let mine = p.mine.get().iter().any(|m| m.id == d.id);
        let subscribed = p.subs.get().iter().any(|m| m.id == d.id) || d.sub;
        let mut kids: Vec<Element> = vec![
            Self::pglass(column(vec![
                tx(d.title.clone(), 20.0, Color::WHITE).black(),
                tx(format!("{} · {} ta{}", kind_single(&d.kind), d.items, if d.bytes > 0 { format!(" · {}", format_bytes(d.bytes)) } else { String::new() }), 13.0, Color::WHITE),
            ]).padding(18.0)).radius(24.0).margin_each(0.0, 0.0, 0.0, 12.0),
        ];
        if mine {
            let (me, pc, dd) = (self.clone(), p.clone(), d.clone());
            kids.push(
                column(vec![row(vec![icon(PK_ADD, 20.0).color(Color::WHITE), tx(format!("{} qo'shish", kind_single(&d.kind)), 14.0, Color::WHITE).bold()]).gap(8.0).align(Align::Center)])
                    .center()
                    .padding_xy(0.0, 14.0)
                    .radius(26.0)
                    .gradient(ACCENT, ACCENT2)
                    .margin_each(0.0, 0.0, 0.0, 10.0)
                    .on_click(move || {
                        let (me2, pc2, dd2) = (me.clone(), pc.clone(), dd.clone());
                        me.pick("*/*", move |path| {
                            pc2.msg.set("Fayl yuklanmoqda...".into());
                            let uid = me2.profile.get().id;
                            let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
                            let name = format!("pki_{uid}_{ts}_{:x}.bin", ts % 0xffff);
                            let (p3, m3, session, n3, pack) = (pc2.clone(), me2.clone(), me2.session.clone(), name.clone(), dd2.id);
                            spawn_bg(
                                move || -> Result<(), String> {
                                    let len = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                                    if len == 0 {
                                        return Err("Fayl bo'sh".into());
                                    }
                                    if len > 5 << 20 {
                                        return Err(format!("Fayl 5 MB dan katta ({:.1} MB)", len as f64 / 1048576.0));
                                    }
                                    crate::backend::upload_file(&path, &n3, "application/octet-stream", &session, &|_, _| {})?;
                                    crate::sync::put_pack(format!("p:add:{n3}"), json!({"op": "add", "pack": pack, "file": n3, "emoji": "", "size": len}));
                                    let _ = std::fs::remove_file(&path);
                                    Ok(())
                                },
                                move |r| match r {
                                    Ok(()) => {
                                        p3.msg.set("Yuborildi — admin ko'rib chiqadi".into());
                                        m3.packs_flush_soon(&p3);
                                    }
                                    Err(e) => p3.msg.set(e),
                                },
                            );
                        });
                    }),
            );
        } else {
            let (me, pc, dd) = (self.clone(), p.clone(), d.clone());
            kids.push(
                column(vec![tx(if subscribed { "Obunani bekor qilish" } else { "Obuna bo'lish" }, 14.0, Color::WHITE).bold()])
                    .center()
                    .padding_xy(0.0, 14.0)
                    .radius(26.0)
                    .gradient(ACCENT, ACCENT2)
                    .margin_each(0.0, 0.0, 0.0, 10.0)
                    .on_click(move || {
                        crate::sync::put_pack(format!("p:sub:{}", dd.id), json!({"op": "sub", "pack": dd.id, "on": !subscribed}));
                        pc.detail.set(None);
                        me.packs_flush_soon(&pc);
                    }),
            );
        }
        // Shu to'plamdagi amallar (kutayotgan / tasdiqlangan / rad etilgan).
        for o in p.ops.get().iter().filter(|o| o["pack"].as_i64() == Some(d.id)) {
            let state = o["state"].as_str().unwrap_or("");
            let (label, c) = match state {
                "pending" => ("Ko'rib chiqilmoqda", GOLD),
                "approved" => ("Tasdiqlandi", Color::rgb(0x4A, 0xDE, 0x80)),
                "rejected" => ("Rad etildi", ERROR),
                _ => ("Yuborilmoqda", Color::rgba(255, 255, 255, 170)),
            };
            let reason = o["reason"].as_str().filter(|r| *r != "null" && !r.is_empty()).map(|r| format!(" · {r}")).unwrap_or_default();
            kids.push(Self::pglass(row(vec![tx(format!("{} · {}", o["op"].as_str().unwrap_or("add"), label), 13.0, c).bold(), tx(reason, 12.0, Color::WHITE)]).gap(6.0).padding(12.0)).radius(18.0).margin_each(0.0, 0.0, 0.0, 8.0));
        }
        if mine {
            let (me, pc, dd) = (self.clone(), p.clone(), d.clone());
            kids.push(tx("To'plamni o'chirish", 14.0, ERROR).bold().padding_xy(6.0, 16.0).on_click(move || {
                crate::sync::put_pack(format!("p:del:{}", dd.id), json!({"op": "delete", "pack": dd.id}));
                pc.detail.set(None);
                me.packs_flush_soon(&pc);
            }));
        }
        kids.push(self.pack_grid(p, d));
        kids.push(gap_box(0.0, 28.0));
        let back_p = p.clone();
        // Orqaga: ro'yxatga (sahifadan chiqmaydi).
        let title = d.title.clone();
        let page = self.scaffold(&title, scroll_column(&p.scroll, kids).padding_each(16.0, 4.0, 16.0, 0.0));
        let _ = back_p;
        match p.big.get() {
            Some((it, img)) => {
                let pc = p.clone();
                let body: Element = match img.or_else(|| p.thumbs.get().get(&it.id).cloned()) {
                    Some(i) => {
                        let (w, _) = window_dp();
                        let side = (w - 64.0).min(360.0);
                        let k = (side / i.w as f32).min(side / i.h as f32);
                        image(&i, i.w as f32 * k, i.h as f32 * k)
                    }
                    None => spinner(32.0, 3.0, Color::WHITE),
                };
                let mut col = vec![body];
                if !it.emoji.is_empty() {
                    col.push(tx(it.emoji.clone(), 24.0, Color::WHITE).padding_each(0.0, 12.0, 0.0, 0.0));
                }
                if it.anim == 2 {
                    col.push(tx("Video-GIF (bu yerda birinchi kadr)", 12.0, Color::WHITE).padding_each(0.0, 8.0, 0.0, 0.0));
                } else if it.anim == 1 {
                    col.push(tx("Animatsiyali (bu yerda birinchi kadr)", 12.0, Color::WHITE).padding_each(0.0, 8.0, 0.0, 0.0));
                }
                let panel = column(col).center().lens(Color::rgba(255, 255, 255, 14), GLASS_ZOOM).radius(28.0).padding(20.0);
                stack(vec![page, column(vec![panel]).center().bg(Color::rgba(0, 0, 0, 45)).on_click(move || pc.big.set(None))])
            }
            None => page,
        }
    }

    fn open_pack(self: &Rc<Self>, p: &Rc<PacksPage>, pk: PackInfo) {
        p.detail.set(Some(pk.clone()));
        p.hdr.set(None);
        p.thumbs.set(Rc::new(HashMap::new()));
        p.derr.set(String::new());
        if pk.file.is_empty() || pk.items == 0 {
            p.derr.set("To'plam hali tayyor emas".into());
            return;
        }
        let (p2, session) = (p.clone(), self.session.clone());
        spawn_bg(
            move || -> Result<(Arc<Header>, Vec<(i64, Image)>), String> {
                let h = packs_data::header(&session, &pk.file, pk.id, pk.version)?;
                let t = packs_data::thumbs(&session, &pk.file, &h);
                Ok((h, t))
            },
            move |r| match r {
                Ok((h, t)) => {
                    p2.thumbs.set(Rc::new(t.into_iter().map(|(i, im)| (i, Rc::new(im))).collect()));
                    p2.hdr.set(Some(h));
                }
                Err(e) => p2.derr.set(e),
            },
        );
    }

    fn pack_grid(self: &Rc<Self>, p: &Rc<PacksPage>, d: &PackInfo) -> Element {
        let Some(h) = p.hdr.get() else {
            return if p.derr.get().is_empty() { column(vec![spinner(28.0, 3.0, ACCENT2)]).center().padding(24.0) } else { tx(p.derr.get(), 13.0, ERROR).padding_xy(6.0, 12.0) };
        };
        let thumbs = p.thumbs.get();
        let (w, _) = window_dp();
        let cols = if d.kind == "emoji" { 6 } else { 4 };
        let gap = 8.0;
        let tile = ((w - 32.0 - gap * (cols as f32 - 1.0)) / cols as f32).floor();
        let mut rows: Vec<Element> = Vec::new();
        for ch in h.items.chunks(cols) {
            let mut r: Vec<Element> = Vec::new();
            for it in ch {
                let inner: Element = match thumbs.get(&it.id) {
                    Some(i) => {
                        let k = ((tile - 12.0) / i.w as f32).min((tile - 12.0) / i.h as f32);
                        image(i, i.w as f32 * k, i.h as f32 * k)
                    }
                    None => column(vec![tx(if it.emoji.is_empty() { "·".to_string() } else { it.emoji.clone() }, 20.0, Color::WHITE)]),
                };
                let (pc, me, item) = (p.clone(), self.clone(), it.clone());
                r.push(Self::pglass(column(vec![inner]).center().width(tile).height(tile)).radius(16.0).on_click(move || me.pack_view_item(&pc, item.clone())));
            }
            rows.push(row(r).gap(gap).margin_each(0.0, 0.0, 0.0, gap));
        }
        column(rows)
    }

    fn pack_view_item(self: &Rc<Self>, p: &Rc<PacksPage>, it: Item) {
        p.big.set(Some((it.clone(), None)));
        let (Some(h), Some(d)) = (p.hdr.get(), p.detail.get()) else { return };
        let (p2, session) = (p.clone(), self.session.clone());
        spawn_bg(move || packs_data::data_image(&session, &d.file, &h, &it).map(|im| (it, im)), move |r| {
            if let Some((it, im)) = r {
                if p2.big.get().map(|(b, _)| b.id) == Some(it.id) {
                    p2.big.set(Some((it, Some(Rc::new(im)))));
                }
            }
        });
    }
}
