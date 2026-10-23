//! Chat uchun Telegramdagidek emoji / GIF / stiker paneli (DrKLO `EmojiView` tuzilishi): tepada bo'lim qatori
//! (oddiy emoji toifalari + Telegram hisobidagi maxsus emoji to'plamlari / stiker to'plamlari), o'rtada to'r,
//! pastda `Emoji | GIF | Stikerlar` tabi, klaviatura va o'chirish tugmalari.
//! Maxsus emoji, stiker va GIF Telegram hisobidan yadro orqali olinadi (`tgmedia.rs`); chatga havola yuboriladi.

use super::*;
use crate::tgmedia::{self, TgDoc, TgSet};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

const S_BACKSPACE: &str = "M22,3H7c-0.69,0 -1.23,0.35 -1.59,0.88L0,12l5.41,8.11c0.36,0.53 0.9,0.89 1.59,0.89h15c1.1,0 2,-0.9 2,-2V5c0,-1.1 -0.9,-2 -2,-2zM19,15.59L17.59,17 14,13.41 10.41,17 9,15.59 12.59,12 9,8.41 10.41,7 14,10.59 17.59,7 19,8.41 15.41,12 19,15.59z";
const S_RECENT: &str = "M11.99,2C6.47,2 2,6.48 2,12s4.47,10 9.99,10C17.52,22 22,17.52 22,12S17.52,2 11.99,2zM12,20c-4.42,0 -8,-3.58 -8,-8s3.58,-8 8,-8 8,3.58 8,8 -3.58,8 -8,8zM12.5,7H11v6l5.25,3.15 0.75,-1.23 -4.5,-2.67z";
const S_KB: &str = "M20,5L4,5c-1.1,0 -1.99,0.9 -1.99,2L2,17c0,1.1 0.9,2 2,2h16c1.1,0 2,-0.9 2,-2L22,7c0,-1.1 -0.9,-2 -2,-2zM11,8h2v2h-2L11,8zM11,11h2v2h-2v-2zM8,8h2v2L8,10L8,8zM8,11h2v2L8,13v-2zM7,13L5,13v-2h2v2zM7,10L5,10L5,8h2v2zM8,16h8v1L8,17v-1zM16,13h-2v-2h2v2zM16,10h-2L14,8h2v2zM19,13h-2v-2h2v2zM19,10h-2L17,8h2v2z";

pub const PANEL_H: f32 = 330.0;

/// Oddiy emoji toifalari: (belgi, nom, emojilar).
const CATS: &[(&str, &str, &str)] = &[
    ("😀", "Smaylar va odamlar", "😀 😃 😄 😁 😆 😅 😂 🤣 🥲 ☺️ 😊 😇 🙂 🙃 😉 😌 😍 🥰 😘 😗 😙 😚 😋 😛 😝 😜 🤪 🤨 🧐 🤓 😎 🥸 🤩 🥳 😏 😒 😞 😔 😟 😕 🙁 ☹️ 😣 😖 😫 😩 🥺 😢 😭 😤 😠 😡 🤬 🤯 😳 🥵 🥶 😱 😨 😰 😥 😓 🫣 🤗 🫡 🤔 🫢 🤭 🤫 🤥 😶 😐 😑 😬 🫨 🙄 😯 😦 😧 😮 😲 🥱 😴 🤤 😪 😵 🤐 🥴 🤢 🤮 🤧 😷 🤒 🤕 🤑 🤠 😈 👿 👹 👺 🤡 💩 👻 💀 ☠️ 👽 👾 🤖 🎃 😺 😸 😹 😻 😼 😽 🙀 😿 😾 👋 🤚 🖐️ ✋ 🖖 👌 🤌 🤏 ✌️ 🤞 🤟 🤘 🤙 👈 👉 👆 👇 ☝️ 👍 👎 ✊ 👊 🤛 🤜 👏 🙌 👐 🤲 🤝 🙏 ✍️ 💅 🤳 💪 🦾 👂 👃 🧠 👀 👁️ 👅 👄 💋 👶 🧒 👦 👧 🧑 👨 👩 🧓 👴 👵 🙍 🙎 🙅 🙆 💁 🙋 🧏 🙇 🤦 🤷 👮 🕵️ 💂 👷 🤴 👸 👳 🧙 🧚 🧛 🧜 🧝 🧞 🧟 💆 💇 🚶 🏃 💃 🕺 👯"),
    ("🐱", "Hayvonlar va tabiat", "🐶 🐱 🐭 🐹 🐰 🦊 🐻 🐼 🐻‍❄️ 🐨 🐯 🦁 🐮 🐷 🐽 🐸 🐵 🙈 🙉 🙊 🐒 🐔 🐧 🐦 🐤 🐣 🐥 🦆 🦅 🦉 🦇 🐺 🐗 🐴 🦄 🐝 🪱 🐛 🦋 🐌 🐞 🐜 🪰 🪲 🦟 🦗 🕷️ 🦂 🐢 🐍 🦎 🦖 🦕 🐙 🦑 🦐 🦞 🦀 🐡 🐠 🐟 🐬 🐳 🐋 🦈 🐊 🐅 🐆 🦓 🦍 🐘 🦛 🦏 🐪 🐫 🦒 🦘 🐃 🐂 🐄 🐎 🐖 🐏 🐑 🦙 🐐 🦌 🐕 🐩 🐈 🐓 🦃 🦚 🦜 🦢 🦩 🕊️ 🐇 🦝 🦨 🦡 🦦 🦥 🐁 🐀 🐿️ 🦔 🌵 🎄 🌲 🌳 🌴 🌱 🌿 ☘️ 🍀 🎍 🍃 🍂 🍁 🍄 🌾 💐 🌷 🌹 🥀 🌺 🌸 🌼 🌻 🌞 🌝 🌛 🌜 🌚 🌕 🌙 ⭐ 🌟 ✨ ⚡ ☄️ 💥 🔥 🌈 ☀️ ⛅ ☁️ ☔ ❄️ ☃️ ⛄ 💧 💦 🌊"),
    ("🍔", "Ovqat va ichimlik", "🍏 🍎 🍐 🍊 🍋 🍌 🍉 🍇 🍓 🫐 🍈 🍒 🍑 🥭 🍍 🥥 🥝 🍅 🍆 🥑 🥦 🥬 🥒 🌶️ 🌽 🥕 🧄 🧅 🥔 🍠 🥐 🥯 🍞 🥖 🧀 🥚 🍳 🥞 🧇 🥓 🥩 🍗 🍖 🌭 🍔 🍟 🍕 🥪 🌮 🌯 🥗 🍝 🍜 🍲 🍛 🍣 🍱 🥟 🍤 🍙 🍚 🍘 🍥 🥠 🍢 🍡 🍧 🍨 🍦 🥧 🧁 🍰 🎂 🍮 🍭 🍬 🍫 🍿 🍩 🍪 🌰 🥜 🍯 🥛 🍼 ☕ 🍵 🧃 🥤 🧋 🍶 🍺 🍻 🥂 🍷 🥃 🍸 🍹 🍾 🧊 🥄 🍴 🍽️"),
    ("⚽", "Faoliyat", "⚽ 🏀 🏈 ⚾ 🥎 🎾 🏐 🏉 🥏 🎱 🏓 🏸 🏒 🏑 🥍 🏏 🥅 ⛳ 🏹 🎣 🥊 🥋 🎽 🛹 🛼 ⛸️ 🥌 🎿 ⛷️ 🏂 🏋️ 🤼 🤸 ⛹️ 🤺 🤾 🏌️ 🏇 🧘 🏄 🏊 🤽 🚣 🧗 🚵 🚴 🏆 🥇 🥈 🥉 🏅 🎖️ 🎫 🎟️ 🎪 🎭 🎨 🎬 🎤 🎧 🎼 🎹 🥁 🎷 🎺 🎸 🎻 🎲 ♟️ 🎯 🎳 🎮 🎰 🧩"),
    ("🚗", "Sayohat va joylar", "🚗 🚕 🚙 🚌 🚎 🏎️ 🚓 🚑 🚒 🚐 🚚 🚛 🚜 🛵 🏍️ 🚲 🛴 🚨 🚔 🚍 🚘 🚖 🚡 🚠 🚟 🚃 🚋 🚞 🚝 🚄 🚅 🚈 🚂 🚆 🚇 🚊 🚉 ✈️ 🛫 🛬 🛩️ 🚀 🛸 🚁 🛶 ⛵ 🚤 🛥️ 🛳️ ⛴️ 🚢 ⚓ ⛽ 🚧 🚦 🚥 🗺️ 🗿 🗽 🗼 🏰 🏯 🏟️ 🎡 🎢 🎠 ⛲ ⛱️ 🏖️ 🏝️ 🏜️ 🌋 ⛰️ 🏔️ 🗻 🏕️ ⛺ 🏠 🏡 🏘️ 🏢 🏬 🏣 🏤 🏥 🏦 🏨 🏪 🏫 🏩 💒 🏛️ ⛪ 🕌 🕍 🌅 🌄 🌠 🎇 🎆 🌇 🌆 🏙️ 🌃 🌌 🌉 🌁"),
    ("💡", "Buyumlar", "⌚ 📱 📲 💻 ⌨️ 🖥️ 🖨️ 🖱️ 💽 💾 💿 📀 📷 📸 📹 🎥 📞 ☎️ 📟 📠 📺 📻 🎙️ ⏱️ ⏰ 🕰️ ⌛ ⏳ 📡 🔋 🔌 💡 🔦 🕯️ 🧯 💸 💵 💴 💶 💷 💰 💳 💎 ⚖️ 🔧 🔨 ⚒️ 🛠️ ⛏️ 🔩 ⚙️ 🧱 ⛓️ 🧲 🔫 💣 🧨 🔪 🗡️ ⚔️ 🛡️ 🚬 ⚰️ ⚱️ 🏺 🔮 📿 🧿 💈 ⚗️ 🔭 🔬 🕳️ 💊 💉 🩸 🧬 🦠 🧫 🧪 🌡️ 🧹 🧺 🧻 🚽 🚰 🚿 🛁 🧼 🧽 🔑 🗝️ 🚪 🛋️ 🛏️ 🧸 🎁 🎈 🎏 🎀 🎊 🎉 🎎 🏮 ✉️ 📩 📦 🏷️ 📝 📁 📅 📈 📉 📊 📌 📍 📎 ✂️ 🔒 🔓 🔔 🔕"),
    ("❤️", "Belgilar", "❤️ 🧡 💛 💚 💙 💜 🖤 🤍 🤎 💔 ❣️ 💕 💞 💓 💗 💖 💘 💝 💟 ☮️ ✝️ ☪️ 🕉️ ☸️ ✡️ 🔯 ☯️ ♈ ♉ ♊ ♋ ♌ ♍ ♎ ♏ ♐ ♑ ♒ ♓ 🆔 ⚛️ 🉑 ☢️ ☣️ 📴 📳 🈶 🈚 🈸 🈺 🆚 💮 🉐 ㊙️ ㊗️ 🆎 🅰️ 🅱️ 🅾️ 🆑 🆘 ❌ ⭕ 🛑 ⛔ 📛 🚫 💯 💢 ♨️ 🚷 🚯 🚳 🚱 🔞 📵 ❗ ❕ ❓ ❔ ‼️ ⁉️ 🔅 🔆 ⚠️ 🚸 🔱 ⚜️ 🔰 ♻️ ✅ 🈯 💹 ❇️ ✳️ ❎ 🌐 💠 Ⓜ️ 🌀 💤 🏧 🚾 ♿ 🅿️ 🛗 🈳 🚹 🚺 🚼 🔼 🔽 ⏫ ⏬ ▶️ ⏸️ ⏯️ ⏹️ ⏺️ ⏭️ ⏮️ ➕ ➖ ➗ ✖️ ♾️ 💲 ✔️ ☑️ 🔘 🔴 🟠 🟡 🟢 🔵 🟣 ⚫ ⚪ 🟤 🔺 🔻 🔸 🔹 🔶 🔷 🔳 🔲"),
    ("🏁", "Bayroqlar", "🏁 🚩 🎌 🏴 🏳️ 🏳️‍🌈 🏴‍☠️ 🇺🇿 🇰🇿 🇰🇬 🇹🇯 🇹🇲 🇦🇿 🇦🇲 🇬🇪 🇷🇺 🇺🇦 🇧🇾 🇹🇷 🇮🇷 🇦🇫 🇵🇰 🇮🇳 🇨🇳 🇯🇵 🇰🇷 🇰🇵 🇻🇳 🇹🇭 🇮🇩 🇲🇾 🇸🇦 🇦🇪 🇶🇦 🇰🇼 🇮🇶 🇸🇾 🇯🇴 🇱🇧 🇮🇱 🇪🇬 🇲🇦 🇩🇿 🇹🇳 🇱🇾 🇪🇹 🇳🇬 🇿🇦 🇩🇪 🇫🇷 🇮🇹 🇪🇸 🇵🇹 🇬🇧 🇮🇪 🇳🇱 🇧🇪 🇨🇭 🇦🇹 🇸🇪 🇳🇴 🇫🇮 🇩🇰 🇵🇱 🇨🇿 🇭🇺 🇷🇴 🇧🇬 🇬🇷 🇨🇦 🇺🇸 🇲🇽 🇧🇷 🇦🇷 🇨🇴 🇨🇱 🇵🇪 🇨🇺 🇦🇺 🇳🇿"),
];

#[derive(Clone)]
enum TImg {
    Loading,
    Ready(Rc<Image>),
    Failed(Instant),
}

thread_local! {
    static IMGS: RefCell<HashMap<String, TImg>> = RefCell::new(HashMap::new());
    static RECENT: RefCell<Vec<String>> = RefCell::new(Vec::new());
}

enum Loaded {
    EmojiSets(Vec<TgSet>),
    StickerSets(Vec<TgSet>),
    Gifs(Vec<TgDoc>),
    Docs(i64, Vec<TgDoc>),
}

#[derive(Default)]
struct Data {
    emoji_sets: Option<Vec<TgSet>>,
    sticker_sets: Option<Vec<TgSet>>,
    gifs: Option<Vec<TgDoc>>,
    docs: HashMap<i64, Vec<TgDoc>>,
    pending: HashSet<String>,
}

pub struct TgPanel {
    pub(super) open: State<bool>,
    tab: State<u8>,
    cat: State<usize>,
    sset: State<usize>,
    scroll: ScrollState,
    strip: ScrollState,
    data: RefCell<Data>,
    font_tried: std::cell::Cell<bool>,
}

impl TgPanel {
    pub fn new() -> TgPanel {
        TgPanel {
            open: State::new(false),
            tab: State::new(0),
            cat: State::new(1),
            sset: State::new(0),
            scroll: ScrollState::new(),
            strip: ScrollState::new(),
            data: RefCell::new(Data::default()),
            font_tried: std::cell::Cell::new(false),
        }
    }
}

/// Matn oxiridagi bitta belgini (maxsus emoji belgisi yoki emoji ketma-ketligi) o'chiradi.
pub(super) fn pop_last_char(s: &str) -> String {
    if let Some(i) = s.rfind("[ce:") {
        if s[i..].find(']') == Some(s.len() - i - 1) {
            return s[..i].to_string();
        }
    }
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

/// Hujjat rasmini yuklaydi; havola (`file_reference`) eskirsa ro'yxatni yangilab, qayta uriniladi.
fn load_doc(set: Option<TgSet>, gifs: bool, d: TgDoc, px: u32) -> Result<Image, String> {
    match tgmedia::image(&d, px) {
        Err(e) if e.contains("FILE_REFERENCE") => {
            let fresh = if gifs { tgmedia::gifs(true) } else { set.map(|s| tgmedia::set_docs(&s, true)).unwrap_or_default() };
            match fresh.into_iter().find(|x| x.id == d.id) {
                Some(nd) => tgmedia::image(&nd, px),
                None => Err(e),
            }
        }
        r => r,
    }
}

impl Home {
    /// Telegram rasmi: tayyor bo'lsa qaytaradi, bo'lmasa fonda yuklaydi (tayyor bo'lgach oyna qayta chiziladi).
    pub(super) fn tg_img(self: &Rc<Self>, key: &str, load: impl FnOnce() -> Result<Image, String> + Send + 'static) -> Option<Rc<Image>> {
        let _ = self.gate_rev.get();
        let st = IMGS.with(|m| m.borrow().get(key).cloned());
        match st {
            Some(TImg::Ready(i)) => return Some(i),
            Some(TImg::Loading) => return None,
            Some(TImg::Failed(t)) if t.elapsed().as_secs() < 20 => return None,
            _ => {}
        }
        IMGS.with(|m| m.borrow_mut().insert(key.to_string(), TImg::Loading));
        let (me, k) = (self.clone(), key.to_string());
        spawn_bg(load, move |r| {
            let st = match r {
                Ok(img) => TImg::Ready(Rc::new(img)),
                Err(_) => TImg::Failed(Instant::now()),
            };
            IMGS.with(|m| m.borrow_mut().insert(k, st));
            me.gate_rev.set(me.gate_rev.get() + 1);
        });
        None
    }

    /// Chatdagi stiker/GIF havolasi (`tgs.…` / `tgg.…`) -> rasm. Tayyor bo'lguncha bo'sh joy.
    pub(super) fn tg_ref_el(self: &Rc<Self>, r: &str, size: f32) -> Option<Element> {
        let sticker = r.starts_with("tgs.");
        if !sticker && !r.starts_with("tgg.") {
            return None;
        }
        let rr = r.to_string();
        let img = self.tg_img(&format!("ref_{r}"), move || {
            let d = if sticker { tgmedia::resolve_sticker(&rr) } else { tgmedia::resolve_gif(&rr) }.ok_or_else(|| "havola hal bo'lmadi".to_string())?;
            tgmedia::image(&d, 512)
        });
        Some(match img {
            Some(i) => {
                let k = (size / i.w as f32).min(size / i.h as f32);
                image(&i, i.w as f32 * k, i.h as f32 * k).radius(if sticker { 0.0 } else { 14.0 })
            }
            None => column(vec![spinner(22.0, 2.0, Color::WHITE)]).center().width(size).height(size),
        })
    }

    fn tgp_load(self: &Rc<Self>, p: &Rc<TgPanel>, key: &str, work: impl FnOnce() -> Loaded + Send + 'static) {
        if !p.data.borrow_mut().pending.insert(key.to_string()) {
            return;
        }
        let (me, p2) = (self.clone(), p.clone());
        spawn_bg(work, move |l| {
            let mut d = p2.data.borrow_mut();
            match l {
                Loaded::EmojiSets(v) => d.emoji_sets = Some(v),
                Loaded::StickerSets(v) => d.sticker_sets = Some(v),
                Loaded::Gifs(v) => d.gifs = Some(v),
                Loaded::Docs(id, v) => {
                    d.docs.insert(id, v);
                }
            }
            drop(d);
            me.gate_rev.set(me.gate_rev.get() + 1);
        });
    }

    /// Joriy tab uchun kerakli ro'yxatlarni (bo'lmasa) fonda yuklaydi; yangi emoji shriftini bir marta tekshiradi.
    fn tgp_prepare(self: &Rc<Self>, p: &Rc<TgPanel>) {
        if !p.font_tried.replace(true) {
            let (me, s) = (self.clone(), self.session.clone());
            spawn_bg(move || crate::emojifont::ensure(&s), move |fresh| {
                if fresh {
                    me.rich_imgs.borrow_mut().clear();
                    me.gate_rev.set(me.gate_rev.get() + 1);
                }
            });
        }
        let tab = p.tab.get();
        if tab == 1 {
            if p.data.borrow().gifs.is_none() {
                self.tgp_load(p, "gifs", || Loaded::Gifs(tgmedia::gifs(false)));
            }
            return;
        }
        let emoji = tab == 0;
        let have = if emoji { p.data.borrow().emoji_sets.is_some() } else { p.data.borrow().sticker_sets.is_some() };
        if !have {
            if emoji {
                self.tgp_load(p, "sets_e", || Loaded::EmojiSets(tgmedia::sets(true)));
            } else {
                self.tgp_load(p, "sets_s", || Loaded::StickerSets(tgmedia::sets(false)));
            }
            return;
        }
        // Tanlangan to'plam hujjatlari.
        let set = self.tgp_current_set(p);
        if let Some(s) = set {
            if !p.data.borrow().docs.contains_key(&s.id) {
                let id = s.id;
                self.tgp_load(p, &format!("set_{id}"), move || Loaded::Docs(id, tgmedia::set_docs(&s, false)));
            }
        }
    }

    /// Tanlangan maxsus emoji yoki stiker to'plami.
    fn tgp_current_set(&self, p: &Rc<TgPanel>) -> Option<TgSet> {
        let d = p.data.borrow();
        match p.tab.get() {
            0 => {
                let i = p.cat.get().checked_sub(CATS.len() + 1)?;
                d.emoji_sets.as_ref()?.get(i).cloned()
            }
            2 => d.sticker_sets.as_ref()?.get(p.sset.get()).cloned(),
            _ => None,
        }
    }

    fn tgp_switch(&self, p: &Rc<TgPanel>, tab: u8) {
        if p.tab.get() != tab {
            p.tab.set(tab);
            p.scroll.set_offset(0.0);
            p.strip.set_offset(0.0);
        }
    }

    fn tgp_pick_cat(&self, p: &Rc<TgPanel>, tab: u8, i: usize) {
        if tab == 0 {
            p.cat.set(i);
        } else {
            p.sset.set(i);
        }
        p.scroll.set_offset(0.0);
    }

    fn strip_btn(inner: Element, selected: bool, f: impl Fn() + 'static) -> Element {
        let b = column(vec![inner]).center().width(40.0).height(36.0).radius(12.0);
        (if selected { b.bg(Color::rgba(255, 255, 255, 36)) } else { b }).on_click(f)
    }

    /// Panelning o'zi. `on_emoji` — emoji matni (oddiy yoki `[ce:…]`), `on_sticker(havola, tur)` — stiker/GIF yuborish,
    /// `on_back` — o'chirish, `on_close` — klaviaturaga qaytish.
    pub(super) fn tg_panel_view(
        self: &Rc<Self>,
        p: &Rc<TgPanel>,
        on_emoji: Rc<dyn Fn(String)>,
        on_sticker: Rc<dyn Fn(String, &'static str)>,
        on_back: Rc<dyn Fn()>,
        on_close: Rc<dyn Fn()>,
    ) -> Element {
        let _ = self.gate_rev.get();
        self.tgp_prepare(p);
        let tab = p.tab.get();
        let (w, _) = window_dp();
        let inner_w = w - 20.0 - 16.0;

        // ── Tepadagi bo'lim qatori ──
        let mut strip: Vec<Element> = Vec::new();
        let mut body: Element = column(vec![]);
        match tab {
            0 => {
                let sel = p.cat.get();
                let me = self.clone();
                strip.push(Self::strip_btn(icon(S_RECENT, 22.0).color(Color::rgba(255, 255, 255, 190)), sel == 0, {
                    let (me, p) = (me.clone(), p.clone());
                    move || me.tgp_pick_cat(&p, 0, 0)
                }));
                for (i, (ico, _, _)) in CATS.iter().enumerate() {
                    let (me, pp) = (self.clone(), p.clone());
                    strip.push(Self::strip_btn(self.rich_el(ico, 22.0, 30.0, 0xFFFFFFFF, false), sel == i + 1, move || me.tgp_pick_cat(&pp, 0, i + 1)));
                }
                let sets = p.data.borrow().emoji_sets.clone().unwrap_or_default();
                for (i, s) in sets.iter().enumerate() {
                    let idx = CATS.len() + 1 + i;
                    let (me, pp, s2) = (self.clone(), p.clone(), s.clone());
                    let cover = self.tg_img(&format!("cv{:x}_{}", s.id, s.thumb_ver), move || tgmedia::set_cover(&s2, 64));
                    let inner = match cover {
                        Some(c) => image(&c, 26.0, 26.0),
                        None => column(vec![]).width(26.0).height(26.0).radius(13.0).bg(Color::rgba(255, 255, 255, 30)),
                    };
                    strip.push(Self::strip_btn(inner, sel == idx, move || me.tgp_pick_cat(&pp, 0, idx)));
                }
                body = self.tgp_emoji_grid(p, sel, inner_w, on_emoji.clone());
            }
            1 => body = self.tgp_gif_grid(p, inner_w, on_sticker.clone()),
            _ => {
                let sel = p.sset.get();
                let sets = p.data.borrow().sticker_sets.clone();
                if let Some(sets) = &sets {
                    for (i, s) in sets.iter().enumerate() {
                        let (me, pp, s2) = (self.clone(), p.clone(), s.clone());
                        let cover = self.tg_img(&format!("cv{:x}_{}", s.id, s.thumb_ver), move || tgmedia::set_cover(&s2, 64));
                        let inner = match cover {
                            Some(c) => image(&c, 28.0, 28.0),
                            None => column(vec![]).width(28.0).height(28.0).radius(14.0).bg(Color::rgba(255, 255, 255, 30)),
                        };
                        strip.push(Self::strip_btn(inner, sel == i, move || me.tgp_pick_cat(&pp, 2, i)));
                    }
                }
                body = self.tgp_sticker_grid(p, inner_w, on_sticker.clone());
            }
        }

        // ── Pastdagi qator: klaviatura | Emoji GIF Stikerlar | o'chirish ──
        let tab_btn = |label: &'static str, t: u8| {
            let (me, pp) = (self.clone(), p.clone());
            let on = tab == t;
            let e = column(vec![tx(label, 13.5, if on { Color::WHITE } else { Color::rgba(255, 255, 255, 170) }).bold()]).center().padding_xy(14.0, 8.0).radius(16.0);
            (if on { e.bg(Color::rgba(255, 255, 255, 40)) } else { e }).on_click(move || me.tgp_switch(&pp, t))
        };
        let oc = on_close.clone();
        let ob = on_back.clone();
        let kb = column(vec![icon(S_KB, 22.0).color(Color::rgba(255, 255, 255, 190))]).center().width(44.0).height(40.0).on_click(move || oc());
        let bs = column(vec![icon(S_BACKSPACE, 22.0).color(Color::rgba(255, 255, 255, 190))]).center().width(44.0).height(40.0).on_click(move || ob());
        let tabs = Self::pglass(row(vec![tab_btn("Emoji", 0), tab_btn("GIF", 1), tab_btn("Stikerlar", 2)]).gap(2.0).padding(3.0)).radius(20.0);
        let bottom = row(vec![kb, spacer(), tabs, spacer(), bs]).align(Align::Center).height(46.0);

        let mut col: Vec<Element> = Vec::new();
        if !strip.is_empty() {
            col.push(row(strip).scroll(&p.strip).gap(2.0).align(Align::Center).height(42.0));
        }
        col.push(body.weight(1.0));
        col.push(bottom);
        Self::pglass(column(col).padding_each(8.0, 6.0, 8.0, 4.0)).radius(24.0).height(PANEL_H - 8.0)
    }

    fn tgp_empty(msg: &str) -> Element {
        column(vec![tx(msg.to_string(), 13.5, Color::rgba(255, 255, 255, 150))]).center().padding(20.0)
    }

    fn tgp_emoji_grid(self: &Rc<Self>, p: &Rc<TgPanel>, sel: usize, w: f32, on_emoji: Rc<dyn Fn(String)>) -> Element {
        let cols = 8usize;
        let cell = (w / cols as f32).floor();
        // Oddiy toifa yoki "so'nggi" yoki maxsus emoji to'plami.
        if sel == 0 || (1..=CATS.len()).contains(&sel) {
            let items: Vec<String> = if sel == 0 { RECENT.with(|r| r.borrow().clone()) } else { CATS[sel - 1].2.split_whitespace().map(String::from).collect() };
            if items.is_empty() {
                return Self::tgp_empty("Hozircha so'nggi emoji yo'q");
            }
            let nrows = (items.len() + cols - 1) / cols;
            let me = self.clone();
            return lazy_column(&p.scroll, nrows, cell, move |i| {
                let r: Vec<Element> = items[i * cols..((i + 1) * cols).min(items.len())]
                    .iter()
                    .map(|e| {
                        let (cb, em) = (on_emoji.clone(), e.clone());
                        column(vec![me.rich_el(e, 28.0, cell, 0xFFFFFFFF, false)]).center().width(cell).height(cell).on_click(move || {
                            RECENT.with(|r| {
                                let mut r = r.borrow_mut();
                                r.retain(|x| x != &em);
                                r.insert(0, em.clone());
                                r.truncate(32);
                            });
                            cb(em.clone())
                        })
                    })
                    .collect();
                row(r)
            });
        }
        let Some(set) = self.tgp_current_set(p) else {
            return if p.data.borrow().emoji_sets.is_none() { column(vec![spinner(28.0, 3.0, ACCENT2)]).center() } else { Self::tgp_empty("Telegram'da maxsus emoji to'plami topilmadi") };
        };
        let docs = p.data.borrow().docs.get(&set.id).cloned();
        let Some(docs) = docs else { return column(vec![spinner(28.0, 3.0, ACCENT2)]).center() };
        if docs.is_empty() {
            return Self::tgp_empty("To'plam bo'sh yoki ochilmadi (Telegram ulanganmi?)");
        }
        let nrows = (docs.len() + cols - 1) / cols;
        let me = self.clone();
        lazy_column(&p.scroll, nrows, cell, move |i| {
            let r: Vec<Element> = docs[i * cols..((i + 1) * cols).min(docs.len())]
                .iter()
                .map(|d| {
                    let (dd, s2) = (d.clone(), set.clone());
                    let img = me.tg_img(&format!("d{:x}", d.id), move || load_doc(Some(s2), false, dd, 96));
                    let inner = match img {
                        Some(im) => image(&im, cell - 10.0, cell - 10.0),
                        None => column(vec![]).width(cell - 10.0).height(cell - 10.0).radius(8.0).bg(Color::rgba(255, 255, 255, 14)),
                    };
                    let (cb, tok) = (on_emoji.clone(), tgmedia::emoji_token(&set.short, d));
                    column(vec![inner]).center().width(cell).height(cell).on_click(move || cb(tok.clone()))
                })
                .collect();
            row(r)
        })
    }

    fn tgp_sticker_grid(self: &Rc<Self>, p: &Rc<TgPanel>, w: f32, on_send: Rc<dyn Fn(String, &'static str)>) -> Element {
        let Some(set) = self.tgp_current_set(p) else {
            return if p.data.borrow().sticker_sets.is_none() { column(vec![spinner(28.0, 3.0, ACCENT2)]).center() } else { Self::tgp_empty("Telegram hisobingizda stiker to'plami yo'q") };
        };
        let docs = p.data.borrow().docs.get(&set.id).cloned();
        let Some(docs) = docs else { return column(vec![spinner(28.0, 3.0, ACCENT2)]).center() };
        if docs.is_empty() {
            return Self::tgp_empty("To'plam bo'sh yoki ochilmadi");
        }
        let cols = 5usize;
        let cell = (w / cols as f32).floor();
        let nrows = (docs.len() + cols - 1) / cols;
        let me = self.clone();
        lazy_column(&p.scroll, nrows, cell, move |i| {
            let r: Vec<Element> = docs[i * cols..((i + 1) * cols).min(docs.len())]
                .iter()
                .map(|d| {
                    let (dd, s2) = (d.clone(), set.clone());
                    let img = me.tg_img(&format!("d{:x}", d.id), move || load_doc(Some(s2), false, dd, 160));
                    let inner = match img {
                        Some(im) => {
                            let k = ((cell - 8.0) / im.w as f32).min((cell - 8.0) / im.h as f32);
                            image(&im, im.w as f32 * k, im.h as f32 * k)
                        }
                        None => column(vec![]).width(cell - 16.0).height(cell - 16.0).radius(10.0).bg(Color::rgba(255, 255, 255, 14)),
                    };
                    let (cb, rf) = (on_send.clone(), tgmedia::sticker_ref(&set.short, d));
                    column(vec![inner]).center().width(cell).height(cell).on_click(move || cb(rf.clone(), "sticker"))
                })
                .collect();
            row(r)
        })
    }

    fn tgp_gif_grid(self: &Rc<Self>, p: &Rc<TgPanel>, w: f32, on_send: Rc<dyn Fn(String, &'static str)>) -> Element {
        let gifs = p.data.borrow().gifs.clone();
        let Some(gifs) = gifs else { return column(vec![spinner(28.0, 3.0, ACCENT2)]).center() };
        if gifs.is_empty() {
            return Self::tgp_empty("Telegram'da saqlangan GIF yo'q");
        }
        let cols = 3usize;
        let cell = (w / cols as f32).floor();
        let nrows = (gifs.len() + cols - 1) / cols;
        let me = self.clone();
        lazy_column(&p.scroll, nrows, cell, move |i| {
            let r: Vec<Element> = gifs[i * cols..((i + 1) * cols).min(gifs.len())]
                .iter()
                .map(|d| {
                    let dd = d.clone();
                    let img = me.tg_img(&format!("d{:x}", d.id), move || load_doc(None, true, dd, 320));
                    let inner: Element = match img {
                        Some(im) => stack(vec![image_fill(&im)]).width(cell - 6.0).height(cell - 6.0).radius(10.0),
                        None => column(vec![]).width(cell - 6.0).height(cell - 6.0).radius(10.0).bg(Color::rgba(255, 255, 255, 14)),
                    };
                    let (cb, rf) = (on_send.clone(), tgmedia::gif_ref(d));
                    column(vec![inner]).center().width(cell).height(cell).on_click(move || cb(rf.clone(), "gif"))
                })
                .collect();
            row(r)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oxirgi_belgi_ochadi() {
        assert_eq!(pop_last_char("abc"), "ab");
        assert_eq!(pop_last_char("a😀"), "a");
        assert_eq!(pop_last_char("a❤\u{FE0F}"), "a");
        assert_eq!(pop_last_char(""), "");
        assert_eq!(pop_last_char("salom [ce:pack:12:😀]"), "salom ");
    }

    #[test]
    fn toifalar_bosh_emas() {
        for (_, _, list) in CATS {
            assert!(list.split_whitespace().count() > 20);
        }
    }
}
