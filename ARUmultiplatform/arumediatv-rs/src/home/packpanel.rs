//! Chat uchun emoji / stiker / GIF paneli (Kotlin `TgMediaPanel`): birinchi yorliq — emoji to'ri, qolganlari — o'z va obuna
//! bo'lingan stiker/GIF to'plamlari. Emoji matnga qo'shiladi, stiker/GIF darhol yuboriladi (`pk_<to'plam>_<element>`).
//! Izohlardagi panel (`comments.rs`) bilan bir xil to'plam yuklash yo'li.

use super::packs::PackInfo;
use super::*;
use crate::api;
use crate::packs_data::{self, Header};
use std::collections::HashMap;
use std::sync::Arc;

/// Eng ko'p ishlatiladigan emojilar (bo'shliq bilan ajratilgan).
const EMOJIS: &str = "😀 😃 😄 😁 😆 😅 😂 🤣 😊 😇 🙂 😉 😍 🥰 😘 😋 😛 😜 🤪 😎 🤩 🥳 😏 😒 😞 😔 😟 😕 🙁 😣 😖 😫 😩 🥺 😢 😭 😤 😠 😡 🤬 🤯 😳 🥵 🥶 😱 😨 😰 😥 😓 🤗 🤔 🤭 🤫 😶 😐 😑 😬 🙄 😯 😮 😲 🥱 😴 🤤 😷 🤒 🤕 🤢 🤮 🤧 😈 👿 💀 💩 🤡 👻 👽 🤖 🙈 🙉 🙊 💋 💌 💘 💝 💖 💗 💓 💞 💕 💔 ❤️ 🧡 💛 💚 💙 💜 🖤 🤍 🤎 💯 💢 💥 💫 💦 💨 👍 👎 👌 ✌️ 🤞 🤟 🤘 👈 👉 👆 👇 ✋ 🤚 🖖 👋 🤙 💪 🙏 🤝 👏 🙌 👐 🤲 💅 🔥 ✨ ⭐ 🌟 🎉 🎊 🎁 🏆 🥇 🎵 🎶 🍕 🍔 🍟 🍿 🍰 🍫 🍺 ☕ 🌹 🌸 🌞 🌙 ⚡ 🌈 ☔ ❄️ 🐱 🐶 🦊 🐼 🐵 🦁 🐸 🐢 🦋 🎮 ⚽ 🏀 📱 💻 📷 🎬 🎧 🚀 ✅ ❌ ❓ ❗ ⚠️ 💤 👀 🧠";

pub struct PackPanel {
    pub(super) open: State<bool>,
    /// 0 — emoji; 1.. — to'plam (`lib[tab - 1]`).
    pub(super) tab: State<usize>,
    lib: State<Option<Rc<Vec<PackInfo>>>>,
    hdr: State<Option<Arc<Header>>>,
    thumbs: State<Rc<HashMap<i64, Rc<Image>>>>,
    scroll: ScrollState,
    escroll: ScrollState,
    chips: ScrollState,
}

impl PackPanel {
    pub fn new() -> PackPanel {
        PackPanel {
            open: State::new(false),
            tab: State::new(0),
            lib: State::new(None),
            hdr: State::new(None),
            thumbs: State::new(Rc::new(HashMap::new())),
            scroll: ScrollState::new(),
            escroll: ScrollState::new(),
            chips: ScrollState::new(),
        }
    }
}

/// Matn oxiridagi bitta belgini (emoji ketma-ketligining qismlari bilan) o'chiradi.
pub(super) fn pop_last_char(s: &str) -> String {
    let mut v: Vec<char> = s.chars().collect();
    while let Some(c) = v.pop() {
        // Variatsiya belgisi / birlashtiruvchi / teri rangi — undan oldingi asosiy belgi ham ketadi.
        if matches!(c as u32, 0xFE0F | 0x200D | 0x1F3FB..=0x1F3FF) {
            continue;
        }
        if matches!(v.last().map(|x| *x as u32), Some(0x200D)) {
            v.pop();
            continue;
        }
        break;
    }
    v.into_iter().collect()
}

impl Home {
    /// Panelni ochadi/yopadi; birinchi ochilishda to'plamlar kutubxonasi yuklanadi.
    pub(super) fn pp_toggle(self: &Rc<Self>, pp: &Rc<PackPanel>) {
        let now = !pp.open.get();
        pp.open.set(now);
        if now && pp.lib.get().is_none() {
            let (pp2, me, session) = (pp.clone(), self.clone(), self.session.clone());
            spawn_bg(move || api::get_json("/api/packs/library", "", Some(&session)), move |r| {
                let mut v: Vec<PackInfo> = Vec::new();
                if let Ok(j) = r {
                    for k in ["mine", "subs"] {
                        if let Some(a) = j[k].as_array() {
                            v.extend(a.iter().map(PackInfo::from).filter(|p| (p.kind == "sticker" || p.kind == "gif") && !p.file.is_empty() && p.items > 0));
                        }
                    }
                }
                let _ = &me;
                pp2.lib.set(Some(Rc::new(v)));
            });
        }
    }

    fn pp_pick(self: &Rc<Self>, pp: &Rc<PackPanel>, idx: usize, pk: PackInfo) {
        pp.tab.set(idx + 1);
        pp.hdr.set(None);
        pp.thumbs.set(Rc::new(HashMap::new()));
        let (pp2, session) = (pp.clone(), self.session.clone());
        spawn_bg(
            move || -> Option<(Arc<Header>, Vec<(i64, Image)>)> {
                let h = packs_data::header(&session, &pk.file, pk.id, pk.version).ok()?;
                let t = packs_data::thumbs(&session, &pk.file, &h);
                Some((h, t))
            },
            move |r| {
                if let Some((h, t)) = r {
                    if pp2.tab.get() == idx + 1 {
                        pp2.thumbs.set(Rc::new(t.into_iter().map(|(i, im)| (i, Rc::new(im))).collect()));
                        pp2.hdr.set(Some(h));
                    }
                }
            },
        );
    }

    /// Panel ko'rinishi: emoji tanlanganda `on_emoji`, stiker/GIF tanlanganda `on_media(fayl, tur)` chaqiriladi,
    /// `on_back` — emoji to'ridagi o'chirish tugmasi.
    pub(super) fn pp_view(
        self: &Rc<Self>,
        pp: &Rc<PackPanel>,
        on_emoji: Rc<dyn Fn(String)>,
        on_back: Rc<dyn Fn()>,
        on_media: Rc<dyn Fn(String, &'static str)>,
    ) -> Element {
        let lib = pp.lib.get();
        let tab = pp.tab.get();
        let (w, _) = window_dp();
        let mut tabs: Vec<Element> = Vec::new();
        {
            let pc = pp.clone();
            tabs.push(self.chip("😀", tab == 0, move || pc.tab.set(0)));
        }
        if let Some(l) = &lib {
            for (i, pk) in l.iter().enumerate() {
                let (me, pc, pk2) = (self.clone(), pp.clone(), pk.clone());
                tabs.push(self.chip(&pk.title, tab == i + 1, move || me.pp_pick(&pc, i, pk2.clone())));
            }
        } else {
            tabs.push(column(vec![spinner(18.0, 2.0, ACCENT2)]).center().width(36.0).height(36.0));
        }
        let header = row(tabs).gap(8.0).scroll(&pp.chips).height(44.0);
        let body: Element = if tab == 0 {
            // Virtual ro'yxat: faqat ko'rinadigan qatorlar quriladi (emoji rasmlari ham faqat shulargagina yasaladi).
            let per_row = 8usize;
            let cell = ((w - 24.0 - 16.0) / per_row as f32).floor();
            let all: Vec<String> = EMOJIS.split_whitespace().map(String::from).collect();
            let nrows = (all.len() + per_row - 1) / per_row + 1; // oxirgi qator — o'chirish tugmasi
            let (me, cb, back) = (self.clone(), on_emoji.clone(), on_back.clone());
            lazy_column(&pp.escroll, nrows, cell, move |i| {
                if i * per_row >= all.len() {
                    let back = back.clone();
                    return row(vec![spacer(), column(vec![tx("⌫  O'chirish", 14.0, Color::WHITE).bold()]).center().padding_xy(18.0, 10.0).radius(16.0).bg(Color::rgba(255, 255, 255, 22)).on_click(move || back())]).padding_xy(0.0, 8.0);
                }
                let r: Vec<Element> = all[i * per_row..((i + 1) * per_row).min(all.len())]
                    .iter()
                    .map(|e| {
                        let (cb, em) = (cb.clone(), e.clone());
                        column(vec![me.rich_el(e, 26.0, cell, 0xFFFFFFFF, false)]).center().width(cell).height(cell).on_click(move || cb(em.clone()))
                    })
                    .collect();
                row(r)
            })
            .height(250.0)
        } else {
            match (&lib, pp.hdr.get()) {
                (Some(l), _) if l.is_empty() => tx("Stiker yoki GIF to'plamingiz yo'q — Profil > To'plamlar orqali qo'shing", 12.5, Color::WHITE).padding(14.0),
                (_, None) => column(vec![spinner(22.0, 2.0, ACCENT2)]).center().padding(16.0).height(250.0),
                (lib, Some(h)) => {
                    let thumbs = pp.thumbs.get();
                    let kind = lib.as_ref().and_then(|l| l.get(tab - 1)).map(|k| k.kind.clone()).unwrap_or_default();
                    let tile = ((w - 24.0 - 24.0 - 8.0 * 3.0) / 4.0).floor();
                    let mut rows: Vec<Element> = Vec::new();
                    for ch in h.items.chunks(4).take(60) {
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
                                let (cb, pid, iid, kd) = (on_media.clone(), h.id, it.id, if kind == "gif" { "gif" } else { "sticker" });
                                column(vec![inner]).center().width(tile).height(tile).radius(14.0).bg(Color::rgba(255, 255, 255, 14)).on_click(move || cb(format!("pk_{pid}_{iid}"), kd))
                            })
                            .collect();
                        rows.push(row(r).gap(8.0).margin_each(0.0, 0.0, 0.0, 8.0));
                    }
                    scroll_column(&pp.scroll, rows).height(250.0)
                }
            }
        };
        Self::pglass(column(vec![header, body]).padding_each(10.0, 8.0, 10.0, 4.0)).radius(24.0)
    }
}

#[cfg(test)]
mod tests {
    use super::pop_last_char;

    #[test]
    fn oxirgi_belgi_ochadi() {
        assert_eq!(pop_last_char("abc"), "ab");
        assert_eq!(pop_last_char("a😀"), "a");
        assert_eq!(pop_last_char("a❤\u{FE0F}"), "a");
        assert_eq!(pop_last_char(""), "");
    }
}
