//! UZ: Chat uchun Telegramdagidek emoji / GIF / stiker paneli (DrKLO `EmojiView` tuzilishi):
//! uch sahifa (yon tomonga surib yoki pastdagi tab bilan almashtiriladi), har sahifada uzluksiz ro'yxat va sarlavhali bo'limlar,
//! tepada bo'lim qatori (bosilsa ro'yxat o'sha joyga sirpanadi, surilganda faol bo'lim o'zgaradi), pastda suzuvchi qator:
//! klaviatura | Emoji GIF Stikerlar | o'chirish (ushlab turilsa takrorlanadi va tezlashadi).
//! Uzoq bosish: emoji — teri rangi tanlash; stiker/GIF — katta ko'rinish, tepaga surilsa "Yuborish" menyusi; maxsus emoji — katta ko'rinish.
//! `.tgs` stikerlar va maxsus emoji sof Rust Lottie chizuvchisi (`uzcoders-lottie`) bilan animatsiyali ko'rsatiladi (`tgcache.rs`).
//! EN: A Telegram-style emoji / GIF / sticker panel for the chat (the DrKLO `EmojiView` structure):
//! three pages (switched by a sideways swipe or the bottom tab), each a continuous list with titled sections,
//! a section strip on top (a tap slides the list there, scrolling changes the active section) and a floating bottom bar:
//! keyboard | Emoji GIF Stickers | backspace (repeats and accelerates while held).
//! Long press: emoji — skin tone picker; sticker/GIF — a large preview, with a "Send" menu when swiped up; custom emoji — a large preview.
//! `.tgs` stickers and custom emoji are shown animated by the pure Rust Lottie renderer (`uzcoders-lottie`, see `tgcache.rs`).
//! RU: Панель эмодзи / GIF / стикеров для чата в стиле Telegram (структура DrKLO `EmojiView`):
//! три страницы (переключаются свайпом вбок или нижней вкладкой), на каждой — сплошной список с заголовками разделов,
//! сверху полоса разделов (нажатие плавно прокручивает список, при прокрутке меняется активный раздел), снизу плавающая панель:
//! клавиатура | Эмодзи GIF Стикеры | удаление (при удержании повторяется и ускоряется).
//! Долгое нажатие: эмодзи — выбор оттенка кожи; стикер/GIF — крупный просмотр, при свайпе вверх меню «Отправить»; пользовательское эмодзи — крупный просмотр.
//! Стикеры `.tgs` и пользовательские эмодзи анимируются чистым Rust-рендерером Lottie (`uzcoders-lottie`, см. `tgcache.rs`).

use super::tgcache::load_doc;
use super::*;
use crate::tgmedia::{self, TgDoc, TgSet};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

const S_BACKSPACE: &str = "M22,3H7c-0.69,0 -1.23,0.35 -1.59,0.88L0,12l5.41,8.11c0.36,0.53 0.9,0.89 1.59,0.89h15c1.1,0 2,-0.9 2,-2V5c0,-1.1 -0.9,-2 -2,-2zM19,15.59L17.59,17 14,13.41 10.41,17 9,15.59 12.59,12 9,8.41 10.41,7 14,10.59 17.59,7 19,8.41 15.41,12 19,15.59z";
const S_RECENT: &str = "M11.99,2C6.47,2 2,6.48 2,12s4.47,10 9.99,10C17.52,22 22,17.52 22,12S17.52,2 11.99,2zM12,20c-4.42,0 -8,-3.58 -8,-8s3.58,-8 8,-8 8,3.58 8,8 -3.58,8 -8,8zM12.5,7H11v6l5.25,3.15 0.75,-1.23 -4.5,-2.67z";
const S_KB: &str = "M20,5L4,5c-1.1,0 -1.99,0.9 -1.99,2L2,17c0,1.1 0.9,2 2,2h16c1.1,0 2,-0.9 2,-2L22,7c0,-1.1 -0.9,-2 -2,-2zM11,8h2v2h-2L11,8zM11,11h2v2h-2v-2zM8,8h2v2L8,10L8,8zM8,11h2v2L8,13v-2zM7,13L5,13v-2h2v2zM7,10L5,10L5,8h2v2zM8,16h8v1L8,17v-1zM16,13h-2v-2h2v2zM16,10h-2L14,8h2v2zM19,13h-2v-2h2v2zM19,10h-2L17,8h2v2z";

/// UZ: Panel balandligi (dp, pastki bo'sh joy bilan). EN: Panel height (dp, with the bottom margin). RU: Высота панели (dp, с нижним отступом).
pub const PANEL_H: f32 = 330.0;
const STRIP_H: f32 = 40.0;
const BAR_H: f32 = 52.0;
const HEADER_H: f32 = 32.0;
const PREVIEW_MENU_DY: f32 = -55.0;

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

/// UZ: Teri rangi o'zgaradigan asosiy emojilar (Unicode `Emoji_Modifier_Base`ning keng tarqalgan qismi). EN: Emoji that accept skin tones (a common part of Unicode `Emoji_Modifier_Base`). RU: Эмодзи с оттенками кожи (распространённая часть Unicode `Emoji_Modifier_Base`).
const TONE_BASES: &str = "✋ 🤚 🖐 🖖 👋 🤏 ✌ 🤞 🤟 🤘 🤙 👈 👉 👆 👇 ☝ 👍 👎 ✊ 👊 🤛 🤜 👏 🙌 👐 🤲 🙏 ✍ 💅 🤳 💪 👂 👃 👶 🧒 👦 👧 🧑 👨 👩 🧓 👴 👵 🙍 🙎 🙅 🙆 💁 🙋 🧏 🙇 🤦 🤷 👮 💂 👷 🤴 👸 👳 🧙 🧚 🧛 🧜 🧝 💆 💇 🚶 🏃 💃 🕺 🤰 👼 🎅 🤶 🦸 🦹 🏂 🏌 🏄 🚣 🏊 ⛹ 🏋 🚴 🚵 🤸 🤽 🤾 🤹 🧘 🛀 🛌";

fn tone_base(e: &str) -> Option<String> {
    let first = e.chars().next()?;
    let s: String = first.to_string();
    TONE_BASES.split_whitespace().any(|b| b == s).then_some(s)
}

/// UZ: Emojiga teri rangi (1..=5) qo'shadi; 0 — o'zgarmaydi. EN: Applies a skin tone (1..=5); 0 leaves it unchanged. RU: Применяет оттенок кожи (1..=5); 0 — без изменений.
fn with_tone(e: &str, tone: usize) -> String {
    let Some(m) = (tone >= 1 && tone <= 5).then(|| char::from_u32(0x1F3FA + tone as u32)).flatten() else { return e.to_string() };
    let mut it = e.chars();
    let Some(first) = it.next() else { return String::new() };
    let rest: String = it.collect();
    let rest = rest.strip_prefix('\u{FE0F}').unwrap_or(&rest);
    format!("{first}{m}{rest}")
}

enum AtlasSt {
    Loading,
    Ready(Rc<Image>, u32),
    Failed(Instant),
}

thread_local! {
    /// UZ: Emoji atlaslari: (toifa, shrift yo'li, katak px) -> holat. EN: Emoji atlases: (category, font path, cell px) -> state. RU: Атласы эмодзи: (категория, путь шрифта, ячейка px) -> состояние.
    static ATLAS: RefCell<HashMap<(usize, String, u32), AtlasSt>> = RefCell::new(HashMap::new());
    static CAT_POS: std::cell::OnceCell<HashMap<&'static str, (usize, usize)>> = const { std::cell::OnceCell::new() };
}

/// Emojining (toifa, o'rin) i — atlasdagi katak.
fn cat_pos(e: &str) -> Option<(usize, usize)> {
    CAT_POS.with(|c| {
        c.get_or_init(|| {
            let mut m = HashMap::new();
            for (ci, (_, _, list)) in CATS.iter().enumerate() {
                for (ix, e) in list.split_whitespace().enumerate() {
                    m.entry(e).or_insert((ci, ix));
                }
            }
            m
        })
        .get(e)
        .copied()
    })
}

thread_local! {
    static RECENT: RefCell<Vec<String>> = RefCell::new(Vec::new());
}

fn push_recent(e: &str) {
    RECENT.with(|r| {
        let mut r = r.borrow_mut();
        r.retain(|x| x != e);
        r.insert(0, e.to_string());
        r.truncate(40);
    });
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

#[derive(Clone, Copy, PartialEq)]
pub(super) enum PKind {
    Sticker,
    Gif,
    Emoji,
}

/// UZ: Uzoq bosishdagi katta ko'rinish. EN: The large preview of a long press. RU: Крупный просмотр при долгом нажатии.
#[derive(Clone)]
pub(super) struct Preview {
    kind: PKind,
    doc: TgDoc,
    set: Option<TgSet>,
    /// UZ: Yuboriladigan havola (stiker/GIF). EN: The ref to send (sticker/GIF). RU: Ссылка для отправки (стикер/GIF).
    rf: String,
    /// UZ: "Yuborish" menyusi ochiqmi (tepaga surilgan). EN: Is the "Send" menu open (swiped up). RU: Открыто ли меню «Отправить» (свайп вверх).
    menu: bool,
}

/// UZ: Teri rangi oynasi: emoji va uning panel ichidagi o'rni (dp). EN: The skin tone popup: the emoji and its place inside the panel (dp). RU: Окно выбора оттенка: эмодзи и его место в панели (dp).
#[derive(Clone)]
pub(super) struct ToneMenu {
    base: String,
    x: f32,
    y: f32,
}

pub struct TgPanel {
    pub(super) open: State<bool>,
    tab: State<u8>,
    scroll: [ScrollState; 3],
    strip: ScrollState,
    data: RefCell<Data>,
    font_tried: Cell<bool>,
    last_off: Cell<f32>,
    last_move: Cell<Instant>,
    timer: Cell<bool>,
    play_left: Cell<u32>,
    settled: Cell<bool>,
    /// UZ: Sirpanib scroll: (boshlang'ich, maqsad px, boshlanish vaqti). EN: A smooth scroll: (start, target px, start time). RU: Плавная прокрутка: (начало, цель px, время начала).
    glide: Cell<Option<(f32, f32, Instant)>>,
    pub(super) preview: State<Option<Rc<Preview>>>,
    tone_menu: State<Option<Rc<ToneMenu>>>,
    tones: RefCell<HashMap<String, usize>>,
    rep: Rc<Cell<u32>>,
    /// UZ: Stiker/GIF yuborish (so'nggi qurilishdan). EN: The sticker/GIF send callback (from the latest build). RU: Обработчик отправки стикера/GIF (из последней сборки).
    send_cb: RefCell<Option<Rc<dyn Fn(String, &'static str)>>>,
    /// UZ: Yuklanayotgan ro'yxatlar boshlangan vaqti (kalit -> vaqt). EN: When the lists being loaded started (key -> time). RU: Когда начата загрузка списков (ключ -> время).
    load_start: RefCell<HashMap<String, Instant>>,
}

impl TgPanel {
    pub fn new() -> TgPanel {
        TgPanel {
            open: State::new(false),
            tab: State::new(0),
            scroll: [ScrollState::new(), ScrollState::new(), ScrollState::new()],
            strip: ScrollState::new(),
            data: RefCell::new(Data::default()),
            load_start: RefCell::new(HashMap::new()),
            font_tried: Cell::new(false),
            last_off: Cell::new(0.0),
            last_move: Cell::new(Instant::now()),
            timer: Cell::new(false),
            play_left: Cell::new(0),
            settled: Cell::new(true),
            glide: Cell::new(None),
            preview: State::new(None),
            tone_menu: State::new(None),
            tones: RefCell::new(HashMap::new()),
            rep: Rc::new(Cell::new(0)),
            send_cb: RefCell::new(None),
        }
    }

    fn cur(&self) -> &ScrollState {
        &self.scroll[self.tab.get().min(2) as usize]
    }
}

thread_local! {
    static CE_REG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

const PUA0: u32 = 0xE000;
const PUA_MAX: usize = 0x1800;

/// UZ: Maxsus emoji tokenini (`[ce:nom:id:😀]`) yozish maydoniga bitta belgi sifatida joylaydi (maydonda uzun matn ko'rinmasin): shaxsiy belgi (PUA) qaytaradi; yuborishda `ce_expand` bilan tokenga qaytariladi.
/// EN: Puts a custom emoji token (`[ce:name:id:😀]`) into the input field as a single character (so no long text shows in the field): returns a private-use (PUA) character; `ce_expand` turns it back into the token on send.
/// RU: Помещает токен пользовательского эмодзи (`[ce:имя:id:😀]`) в поле ввода одним символом (чтобы в поле не было длинного текста): возвращает символ частного использования (PUA); при отправке `ce_expand` возвращает токен.
pub(super) fn ce_compact(tok: &str) -> String {
    CE_REG.with(|r| {
        let mut r = r.borrow_mut();
        let i = match r.iter().position(|t| t == tok) {
            Some(i) => i,
            None if r.len() < PUA_MAX => {
                r.push(tok.to_string());
                r.len() - 1
            }
            None => return tok.to_string(),
        };
        char::from_u32(PUA0 + i as u32).map(String::from).unwrap_or_else(|| tok.to_string())
    })
}

fn ce_map(s: &str, f: impl Fn(&str) -> String) -> String {
    CE_REG.with(|r| {
        let r = r.borrow();
        s.chars()
            .map(|c| match (c as u32).checked_sub(PUA0).and_then(|i| r.get(i as usize)) {
                Some(t) if (PUA0..PUA0 + PUA_MAX as u32).contains(&(c as u32)) => f(t),
                _ => c.to_string(),
            })
            .collect()
    })
}

/// UZ: Shaxsiy belgilarni to'liq tokenlarga qaytaradi (yuborish uchun). EN: Turns the private-use characters back into full tokens (for sending). RU: Возвращает символы PUA в полные токены (для отправки).
pub(super) fn ce_token(c: &str) -> Option<String> {
    let mut it = c.chars();
    let ch = it.next()?;
    if it.next().is_some() {
        return None;
    }
    CE_REG.with(|r| (ch as u32).checked_sub(PUA0).and_then(|i| r.borrow().get(i as usize).cloned()))
}

/// UZ: Shaxsiy belgilarni to'liq tokenlarga qaytaradi (yuborish uchun). EN: Turns the private-use characters back into full tokens (for sending). RU: Возвращает символы PUA в полные токены (для отправки).
pub(super) fn ce_expand(s: &str) -> String {
    ce_map(s, |t| t.to_string())
}

/// UZ: Shaxsiy belgilarni zaxira emojiga almashtiradi (ko'rinish uchun). EN: Replaces the private-use characters with their fallback emoji (for display). RU: Заменяет символы PUA запасным эмодзи (для отображения).
pub(super) fn ce_plain(s: &str) -> String {
    ce_map(s, |t| crate::tgmedia::plain_text(t))
}

/// UZ: Matn oxiridagi bitta belgini (maxsus emoji belgisi yoki emoji ketma-ketligi) o'chiradi. EN: Removes the last character (a custom emoji token or an emoji sequence). RU: Удаляет последний символ (токен пользовательского эмодзи или последовательность эмодзи).
pub(super) fn pop_last_char(s: &str) -> String {
    if let Some(i) = s.rfind("[ce:") {
        if s[i..].find(']') == Some(s.len() - i - 1) {
            return s[..i].to_string();
        }
    }
    let mut v: Vec<char> = s.chars().collect();
    while let Some(c) = v.pop() {
        // UZ: Variatsiya belgisi / birlashtiruvchi / teri rangi — undan oldingi asosiy belgi ham ketadi. EN: A variation selector / joiner / skin tone: the base character before it goes too. RU: Селектор вариации / соединитель / оттенок кожи — основной символ перед ним тоже удаляется.
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

// ───────────────────────── Ro'yxat tuzilmasi / List layout ─────────────────────────

enum RowKind {
    Header(String),
    Std(Vec<String>),
    /// UZ: To'plam hujjatlari: to'plam raqami (ro'yxatda), boshlang'ich indeks, soni. EN: Set docs: set index, start index, count. RU: Документы набора: индекс набора, начальный индекс, количество.
    Docs(usize, usize, usize),
}

enum SecIcon {
    Recent,
    Cat(usize),
    Set(TgSet),
}

struct Layout {
    rows: Vec<RowKind>,
    heights: Vec<f32>,
    tops: Vec<f32>,
    secs: Vec<(usize, SecIcon)>,
}

impl Layout {
    fn new() -> Self {
        Layout { rows: Vec::new(), heights: Vec::new(), tops: Vec::new(), secs: Vec::new() }
    }

    fn push(&mut self, r: RowKind, h: f32) {
        self.tops.push(self.heights.iter().sum());
        self.rows.push(r);
        self.heights.push(h);
    }

    /// UZ: Faol bo'lim: tepasi ko'rinish yuqori chegarasidan oshmagan oxirgisi. EN: The active section: the last whose top is not past the viewport top. RU: Активный раздел: последний, чей верх не ниже верхней границы окна.
    fn active(&self, off_dp: f32) -> usize {
        let mut a = 0;
        for (i, (row, _)) in self.secs.iter().enumerate() {
            if self.tops.get(*row).copied().unwrap_or(f32::MAX) <= off_dp + 24.0 {
                a = i;
            }
        }
        a
    }
}

fn gif_rows(docs: &[TgDoc], width: f32) -> Vec<(Vec<usize>, f32)> {
    const TARGET: f32 = 96.0;
    const GAP: f32 = 2.0;
    let (mut rows, mut cur, mut asum) = (Vec::new(), Vec::new(), 0.0f32);
    for (i, d) in docs.iter().enumerate() {
        let a = if d.w > 0 && d.h > 0 { d.w as f32 / d.h as f32 } else { 1.0 }.clamp(0.4, 2.5);
        cur.push(i);
        asum += a;
        let gaps = (cur.len() - 1) as f32 * GAP;
        if asum * TARGET + gaps >= width {
            rows.push((std::mem::take(&mut cur), ((width - gaps) / asum).min(TARGET * 1.5)));
            asum = 0.0;
        }
    }
    if !cur.is_empty() {
        rows.push((cur, TARGET));
    }
    rows
}

impl Home {
    // ───────────────────────── Ma'lumot yuklash / Data loading ─────────────────────────

    fn tgp_load(self: &Rc<Self>, p: &Rc<TgPanel>, key: &str, work: impl FnOnce() -> Loaded + Send + 'static) {
        if !p.data.borrow_mut().pending.insert(key.to_string()) {
            return;
        }
        let (me, p2) = (self.clone(), p.clone());
        p.load_start.borrow_mut().insert(key.to_string(), Instant::now());
        let k = key.to_string();
        // UZ: Ro'yxat yuklash tezkor yo'lda: og'ir rasm/animatsiya ishlari navbatida kutib qolmasin.
        spawn_bg_fast(work, move |l| {
            p2.load_start.borrow_mut().remove(&k);
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

    /// UZ: Joriy tab ro'yxatlarini (bo'lmasa) yuklaydi; yangi emoji shriftini bir marta tekshiradi. EN: Loads the current tab's lists (if missing); checks the emoji font once. RU: Загружает списки текущей вкладки (если нет); один раз проверяет шрифт эмодзи.
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
        match p.tab.get() {
            1 => {
                if p.data.borrow().gifs.is_none() {
                    self.tgp_load(p, "gifs", || Loaded::Gifs(tgmedia::gifs(false)));
                }
            }
            0 => {
                if p.data.borrow().emoji_sets.is_none() {
                    self.tgp_load(p, "sets_e", || Loaded::EmojiSets(tgmedia::sets(true)));
                }
            }
            _ => {
                if p.data.borrow().sticker_sets.is_none() {
                    self.tgp_load(p, "sets_s", || Loaded::StickerSets(tgmedia::sets(false)));
                }
            }
        }
    }

    /// UZ: To'plam hujjatlari kerak (qatorlari ko'rinmoqda): yuklanmagan bo'lsa fonda yuklaydi. EN: The set's docs are needed (its rows are visible): loads them in the background if missing. RU: Нужны документы набора (его строки видны): при отсутствии загружает в фоне.
    fn tgp_need_docs(self: &Rc<Self>, p: &Rc<TgPanel>, s: &TgSet) {
        if p.data.borrow().docs.contains_key(&s.id) {
            return;
        }
        let (id, s2) = (s.id, s.clone());
        self.tgp_load(p, &format!("set_{id}"), move || Loaded::Docs(id, tgmedia::set_docs(&s2, false)));
    }

    // ───────────────────────── Surish / Scrolling ─────────────────────────

    /// UZ: Hozir animatsiya o'ynashi mumkinmi: ro'yxat to'xtagan va o'ynayotganlar soni chegaradan oshmagan (Kotlindagi `rememberSettled`). EN: May an animation play now: the list is still and the count of playing ones is within the limit (Kotlin's `rememberSettled`). RU: Можно ли сейчас анимировать: список неподвижен и число играющих в пределах лимита (`rememberSettled` в Kotlin).
    fn tgp_may_play(&self, p: &Rc<TgPanel>) -> bool {
        if !p.settled.get() || p.play_left.get() == 0 {
            return false;
        }
        p.play_left.set(p.play_left.get() - 1);
        true
    }

    /// UZ: Ro'yxat surilmoqdami: surilayotganda animatsiyalar to'xtaydi, to'xtagach 0,25 s dan keyin qaytadi; sirpanib scroll qadami. EN: Is the list moving: animations pause while it scrolls and resume 0.25 s after it stops; the glide step. RU: Прокручивается ли список: анимации паузятся и возобновляются через 0,25 с после остановки; шаг плавной прокрутки.
    fn tgp_track_scroll(self: &Rc<Self>, p: &Rc<TgPanel>) {
        if let Some((from, to, t0)) = p.glide.get() {
            let x = (t0.elapsed().as_millis() as f32 / 280.0).min(1.0);
            let e = 1.0 - (1.0 - x).powi(3);
            p.cur().set_offset(from + (to - from) * e);
            if x >= 1.0 {
                p.glide.set(None);
            } else {
                let me = self.clone();
                spawn_bg(|| std::thread::sleep(std::time::Duration::from_millis(16)), move |_| me.gate_rev.set(me.gate_rev.get() + 1));
            }
        }
        let off = p.cur().offset();
        if (off - p.last_off.get()).abs() > 0.5 {
            p.last_off.set(off);
            p.last_move.set(Instant::now());
        }
        let moving = p.last_move.get().elapsed().as_millis() < 250;
        p.settled.set(!moving);
        p.play_left.set(6);
        if moving && !p.timer.replace(true) {
            let (me, pp) = (self.clone(), p.clone());
            spawn_bg(|| std::thread::sleep(std::time::Duration::from_millis(300)), move |_| {
                pp.timer.set(false);
                me.gate_rev.set(me.gate_rev.get() + 1);
            });
        }
    }

    /// UZ: Joriy sahifani `top_dp` (dp) joyiga silliq suradi. EN: Smoothly scrolls the current page to `top_dp` (dp). RU: Плавно прокручивает текущую страницу к `top_dp` (dp).
    fn tgp_glide_to(&self, p: &Rc<TgPanel>, top_dp: f32) {
        let to = (top_dp * density()).clamp(0.0, p.cur().max_offset().max(0.0));
        p.glide.set(Some((p.cur().offset(), to, Instant::now())));
        self.gate_rev.set(self.gate_rev.get() + 1);
    }

    fn tgp_switch(&self, p: &Rc<TgPanel>, tab: i32) {
        let t = tab.clamp(0, 2) as u8;
        if p.tab.get() != t {
            uzcoders::prelude::breadcrumb(&format!("panel varag'i {t}"));
            p.tab.set(t);
            p.glide.set(None);
            p.strip.set_offset(0.0);
            p.last_off.set(p.cur().offset());
        }
    }

    // ───────────────────────── Ko'rinish / View ─────────────────────────

    fn strip_btn(inner: Element, selected: bool, f: impl Fn() + 'static) -> Element {
        let b = column(vec![inner]).center().width(40.0).height(34.0).radius(12.0);
        (if selected { b.bg(Color::rgba(255, 255, 255, 40)) } else { b }).on_click(f)
    }

    /// UZ: Ro'yxat yuklanmoqda: aylanuvchi ko'rsatkich; 20 soniyadan oshsa sabab va "Qayta urinish" tugmasi (jurnalga ham yoziladi).
    /// EN: A list is loading: a spinner; after 20 seconds a reason and a "Retry" button (also written to the error log).
    /// RU: Список загружается: индикатор; через 20 секунд — причина и кнопка «Повторить» (также пишется в журнал ошибок).
    fn tgp_loading(self: &Rc<Self>, p: &Rc<TgPanel>, key: &str) -> Element {
        let started = p.load_start.borrow().get(key).copied();
        let late = started.is_some_and(|t| t.elapsed().as_secs() >= 20);
        if !late {
            if started.is_some() {
                let me = self.clone();
                spawn_bg(|| std::thread::sleep(std::time::Duration::from_secs(21)), move |_| me.gate_rev.set(me.gate_rev.get() + 1));
            }
            return column(vec![spinner(28.0, 3.0, ACCENT2)]).center();
        }
        thread_local! {
            static LATE: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
        }
        if LATE.with(|l| l.borrow_mut().insert(format!("{key}{started:?}"))) {
            uzcoders::prelude::report_error("panel", &format!("{key} 20 soniyada yuklanmadi (Telegram yoki tarmoq sekin)"));
        }
        let (me, pp, k) = (self.clone(), p.clone(), key.to_string());
        let retry = column(vec![tx("Qayta urinish", 13.0, Color::WHITE).bold()]).center().padding_xy(18.0, 10.0).bg(ACCENT.with_opacity(0.5)).radius(12.0).on_click(move || {
            pp.data.borrow_mut().pending.remove(&k);
            pp.load_start.borrow_mut().remove(&k);
            me.gate_rev.set(me.gate_rev.get() + 1);
        });
        column(vec![tx("Telegram javob bermayapti", 13.5, Color::rgba(255, 255, 255, 170)), gap_box(0.0, 12.0), retry]).center().padding(20.0)
    }

    fn tgp_empty(msg: &str) -> Element {
        column(vec![tx(msg.to_string(), 13.5, Color::rgba(255, 255, 255, 150))]).center().padding(20.0)
    }

    fn tgp_header(title: &str) -> Element {
        column(vec![tx(title.to_string(), 13.5, Color::rgba(255, 255, 255, 150)).bold()]).justify(Justify::End).padding_each(10.0, 0.0, 6.0, 6.0)
    }

    fn tgp_placeholder(w: f32, h: f32, r: f32) -> Element {
        column(vec![]).width(w).height(h).radius(r).bg(Color::rgba(255, 255, 255, 14))
    }

    /// UZ: Panelning o'zi. `on_emoji` — emoji matni (oddiy yoki `[ce:…]`), `on_sticker(havola, tur)` — stiker/GIF yuborish, `on_back` — o'chirish, `on_close` — klaviaturaga qaytish.
    /// EN: The panel itself. `on_emoji` — emoji text (plain or `[ce:…]`), `on_sticker(ref, kind)` — sends a sticker/GIF, `on_back` — backspace, `on_close` — back to the keyboard.
    /// RU: Сама панель. `on_emoji` — текст эмодзи (обычный или `[ce:…]`), `on_sticker(ссылка, вид)` — отправка стикера/GIF, `on_back` — удаление, `on_close` — возврат к клавиатуре.
    pub(super) fn tg_panel_view(
        self: &Rc<Self>,
        p: &Rc<TgPanel>,
        on_emoji: Rc<dyn Fn(String)>,
        on_sticker: Rc<dyn Fn(String, &'static str)>,
        on_back: Rc<dyn Fn()>,
        on_close: Rc<dyn Fn()>,
    ) -> Element {
        let _ = self.gate_rev.get();
        *p.send_cb.borrow_mut() = Some(on_sticker.clone());
        self.tgp_track_scroll(p);
        self.tgp_prepare(p);
        let tab = p.tab.get();
        let (w, _) = window_dp();
        let pw = w - 20.0;
        let sc = p.cur().clone();
        let off_dp = sc.offset() / density();

        let (layout, strip_items, body): (Layout, Vec<(SecIcon, bool)>, Element) = match tab {
            0 => self.tgp_emoji_page(p, pw, off_dp, on_emoji.clone()),
            1 => self.tgp_gif_page(p, pw, on_sticker.clone()),
            _ => self.tgp_sticker_page(p, pw, off_dp, on_sticker.clone()),
        };

        // ── Tepadagi bo'lim qatori (suzuvchi) ──
        let mut strip: Vec<Element> = Vec::new();
        for (i, (icon_kind, active)) in strip_items.into_iter().enumerate() {
            let top = layout.secs.get(i).and_then(|(r, _)| layout.tops.get(*r)).copied().unwrap_or(0.0);
            let (me, pp) = (self.clone(), p.clone());
            let go = move || me.tgp_glide_to(&pp, top);
            let inner: Element = match icon_kind {
                SecIcon::Recent => icon(S_RECENT, 22.0).color(Color::rgba(255, 255, 255, 190)),
                SecIcon::Cat(c) => self.rich_el(CATS[c].0, 22.0, 30.0, 0xFFFFFFFF, false),
                SecIcon::Set(s) => {
                    let s2 = s.clone();
                    match self.tg_img(&format!("cv{:x}_{}", s.id, s.thumb_ver), move || tgmedia::set_cover(&s2, 64)) {
                        Some(c) => image(&c, 26.0, 26.0),
                        None => column(vec![]).width(26.0).height(26.0).radius(13.0).bg(Color::rgba(255, 255, 255, 30)),
                    }
                }
            };
            strip.push(Self::strip_btn(inner, active, go));
        }

        // ── Pastdagi suzuvchi qator: klaviatura | Emoji GIF Stikerlar | o'chirish ──
        let tab_btn = |label: &'static str, t: u8| {
            let (me, pp) = (self.clone(), p.clone());
            let on = tab == t;
            let e = column(vec![tx(label, 13.5, if on { Color::WHITE } else { Color::rgba(255, 255, 255, 170) }).bold()]).center().padding_xy(14.0, 8.0).radius(16.0);
            (if on { e.bg(Color::rgba(255, 255, 255, 40)) } else { e }).on_click(move || me.tgp_switch(&pp, t as i32))
        };
        let kb = column(vec![icon(S_KB, 22.0).color(Color::rgba(255, 255, 255, 190))]).center().width(46.0).height(44.0).on_click({
            let oc = on_close.clone();
            move || oc()
        });
        let bs = self.tgp_backspace(p, on_back.clone());
        let tabs = Self::pglass(row(vec![tab_btn("Emoji", 0), tab_btn("GIF", 1), tab_btn("Stikerlar", 2)]).gap(2.0).padding(3.0)).radius(20.0);
        let bar = row(vec![kb, spacer(), tabs, spacer(), bs]).align(Align::Center).height(BAR_H).padding_xy(4.0, 0.0);

        let top_pad = if strip.is_empty() { 4.0 } else { STRIP_H };
        let page = body.padding_each(5.0, top_pad, 5.0, BAR_H).grow(1.0);
        let me_sw = self.clone();
        let pp = p.clone();
        let page = column(vec![page]).grow(1.0).on_swipe(move |d| me_sw.tgp_switch(&pp, pp.tab.get() as i32 + d));
        let mut layers: Vec<Element> = vec![page];
        if !strip.is_empty() {
            layers.push(column(vec![row(strip).scroll(&p.strip).gap(2.0).align(Align::Center).height(STRIP_H).padding_xy(4.0, 0.0).bg(Color::rgba(0, 0, 0, 70))]));
        }
        layers.push(column(vec![spacer(), bar]));
        if let Some(t) = p.tone_menu.get() {
            layers.push(self.tgp_tone_popup(p, &t, on_emoji));
        }
        Self::pglass(stack(layers)).radius(24.0).height(PANEL_H - 8.0)
    }

    /// UZ: O'chirish tugmasi: bosilsa bitta belgi; 350 ms ushlansa takrorlanadi va tezlashadi (350 → 250 → 150 → 50 ms). EN: Backspace: one character on a tap; repeats and accelerates when held for 350 ms (350 → 250 → 150 → 50 ms). RU: Удаление: один символ по нажатию; при удержании 350 мс повторяется и ускоряется (350 → 250 → 150 → 50 мс).
    fn tgp_backspace(self: &Rc<Self>, p: &Rc<TgPanel>, on_back: Rc<dyn Fn()>) -> Element {
        let (ob, ob2, rep, me) = (on_back.clone(), on_back, p.rep.clone(), self.clone());
        column(vec![icon(S_BACKSPACE, 22.0).color(Color::rgba(255, 255, 255, 190))])
            .center()
            .width(46.0)
            .height(44.0)
            .on_click(move || ob())
            .on_hold(350, move |h| match h {
                Hold::Start => {
                    let id = rep.get().wrapping_add(1);
                    rep.set(id);
                    ob2();
                    Self::tgp_repeat(me.clone(), ob2.clone(), rep.clone(), id, 250);
                }
                Hold::Move(_) => {}
                Hold::End(_) => rep.set(rep.get().wrapping_add(1)),
            })
    }

    fn tgp_repeat(me: Rc<Home>, f: Rc<dyn Fn()>, rep: Rc<Cell<u32>>, id: u32, delay: u64) {
        spawn_bg(move || std::thread::sleep(std::time::Duration::from_millis(delay)), move |_| {
            if rep.get() != id {
                return;
            }
            f();
            me.gate_rev.set(me.gate_rev.get() + 1);
            Self::tgp_repeat(me, f, rep, id, delay.saturating_sub(100).max(50));
        });
    }

    // ───────────────────────── Emoji sahifasi / Emoji page ─────────────────────────

    fn tgp_emoji_page(self: &Rc<Self>, p: &Rc<TgPanel>, pw: f32, off_dp: f32, on_emoji: Rc<dyn Fn(String)>) -> (Layout, Vec<(SecIcon, bool)>, Element) {
        let cols = (((pw - 10.0) / 45.0).floor() as usize).clamp(6, 10);
        let cell = ((pw - 10.0) / cols as f32).floor();
        let sets = p.data.borrow().emoji_sets.clone().unwrap_or_default();
        let mut lay = Layout::new();
        let recent: Vec<String> = RECENT.with(|r| r.borrow().clone());
        if !recent.is_empty() {
            lay.secs.push((lay.rows.len(), SecIcon::Recent));
            lay.push(RowKind::Header("So'nggi ishlatilganlar".into()), HEADER_H);
            for ch in recent.chunks(cols) {
                lay.push(RowKind::Std(ch.to_vec()), cell);
            }
        }
        for (ci, (_, name, list)) in CATS.iter().enumerate() {
            lay.secs.push((lay.rows.len(), SecIcon::Cat(ci)));
            lay.push(RowKind::Header((*name).to_string()), HEADER_H);
            let items: Vec<String> = list.split_whitespace().map(String::from).collect();
            for ch in items.chunks(cols) {
                lay.push(RowKind::Std(ch.to_vec()), cell);
            }
        }
        for (si, s) in sets.iter().enumerate() {
            lay.secs.push((lay.rows.len(), SecIcon::Set(s.clone())));
            lay.push(RowKind::Header(if s.title.is_empty() { s.short.clone() } else { s.title.clone() }), HEADER_H);
            let n = p.data.borrow().docs.get(&s.id).map_or(s.count.max(0) as usize, |d| d.len());
            let mut start = 0;
            while start < n {
                let k = cols.min(n - start);
                lay.push(RowKind::Docs(si, start, k), cell);
                start += k;
            }
        }
        let active = lay.active(off_dp);
        let strip: Vec<(SecIcon, bool)> = lay
            .secs
            .iter()
            .enumerate()
            .map(|(i, (_, k))| {
                (
                    match k {
                        SecIcon::Recent => SecIcon::Recent,
                        SecIcon::Cat(c) => SecIcon::Cat(*c),
                        SecIcon::Set(s) => SecIcon::Set(s.clone()),
                    },
                    i == active,
                )
            })
            .collect();
        let (me, pp, heights) = (self.clone(), p.clone(), lay.heights.clone());
        let tops = lay.tops.clone();
        let rows = Rc::new(std::mem::take(&mut lay.rows));
        let r2 = rows.clone();
        let list = lazy_column_var(&p.scroll[0], &heights, move |i| match &r2[i] {
            RowKind::Header(t) => Self::tgp_header(t),
            RowKind::Std(items) => {
                let cells: Vec<Element> = items
                    .iter()
                    .map(|e| {
                        let (cb, em) = (on_emoji.clone(), e.clone());
                        let (me2, pp2) = (me.clone(), pp.clone());
                        let top = tops[i];
                        let tone = pp.tones.borrow().get(e).copied().unwrap_or(0);
                        let shown = with_tone(e, tone);
                        let col = items.iter().position(|x| x == e).unwrap_or(0);
                        let base = tone_base(e);
                        let glyph = match cat_pos(&shown).filter(|_| tone == 0) {
                            Some((ci, ix)) => me.tgp_atlas_emoji(ci, ix),
                            None => me.rich_el(&shown, 28.0, cell, 0xFFFFFFFF, false),
                        };
                        let mut c = column(vec![glyph]).center().width(cell).height(cell);
                        let sh = shown.clone();
                        c = c.on_click(move || {
                            push_recent(&em);
                            cb(sh.clone())
                        });
                        if let Some(b) = base {
                            c = c.on_hold(300, move |h| {
                                if let Hold::Start = h {
                                    let off = pp2.scroll[0].offset() / density();
                                    let x = (5.0 + col as f32 * cell).min(pw - 6.0 * 40.0 - 20.0).max(2.0);
                                    let y = (STRIP_H + top - off - 52.0).max(2.0);
                                    pp2.tone_menu.set(Some(Rc::new(ToneMenu { base: b.clone(), x, y })));
                                    me2.gate_rev.set(me2.gate_rev.get() + 1);
                                }
                            });
                        }
                        c
                    })
                    .collect();
                row(cells)
            }
            RowKind::Docs(si, start, n) => {
                let sets = me.tgp_sets_of(&pp, 0);
                let Some(set) = sets.get(*si).cloned() else { return row(vec![]) };
                me.tgp_need_docs(&pp, &set);
                let docs = pp.data.borrow().docs.get(&set.id).cloned();
                let cells: Vec<Element> = (*start..*start + *n)
                    .map(|k| match docs.as_ref().and_then(|d| d.get(k)) {
                        Some(d) => me.tgp_custom_cell(&pp, d, &set, cell, on_emoji.clone()),
                        None => column(vec![Self::tgp_placeholder(cell - 12.0, cell - 12.0, 8.0)]).center().width(cell).height(cell),
                    })
                    .collect();
                row(cells)
            }
        });
        let body = if p.data.borrow().emoji_sets.is_none() && false { column(vec![spinner(28.0, 3.0, ACCENT2)]).center() } else { list };
        let lay2 = Layout { rows: Vec::new(), heights, tops: lay.tops, secs: lay.secs };
        (lay2, strip, body)
    }

    /// UZ: Atlasdagi emoji: atlas tayyor bo'lsa bitta katak (qo'shimcha dekodlashsiz), bo'lmasa fonda tayyorlanadi va shu orada bo'sh joy.
    /// EN: An emoji from the atlas: one cell once the atlas is ready (no extra decoding), otherwise it is prepared in the background and a blank space is shown meanwhile.
    /// RU: Эмодзи из атласа: одна ячейка, когда атлас готов (без лишнего декодирования), иначе он готовится в фоне, а пока показывается пустое место.
    fn tgp_atlas_emoji(self: &Rc<Self>, ci: usize, ix: usize) -> Element {
        match self.tgp_atlas_image(ci, ix) {
            Some((img, n)) => sprite_frame(&img, crate::emojifont::ATLAS_COLS, n, ix as u32, 32.0, 32.0),
            None => column(vec![]).width(32.0).height(32.0),
        }
    }

    /// UZ: Atlas rasmi (tayyor bo'lsa) va undagi emoji soni; tayyor bo'lmasa fonda tayyorlaydi. EN: The atlas image (when ready) and its emoji count; prepares it in the background otherwise. RU: Изображение атласа (если готово) и число эмодзи в нём; иначе готовит в фоне.
    fn tgp_atlas_image(self: &Rc<Self>, ci: usize, _ix: usize) -> Option<(Rc<Image>, u32)> {
        let _ = self.gate_rev.get();
        let dens = density().max(0.5);
        let (cell_px, glyph_px) = ((32.0 * dens).round() as u32, 28.0 * dens);
        let font = crate::emojifont::path();
        let key = (ci, font, cell_px);
        let st = ATLAS.with(|a| match a.borrow().get(&key) {
            Some(AtlasSt::Ready(i, n)) => Some(Some((i.clone(), *n))),
            Some(AtlasSt::Failed(t)) if t.elapsed().as_secs() < 30 => Some(None),
            Some(AtlasSt::Loading) => Some(None),
            _ => None,
        });
        match st {
            Some(Some((img, n))) => return Some((img, n)),
            Some(None) => {}
            None => {
                ATLAS.with(|a| a.borrow_mut().insert(key.clone(), AtlasSt::Loading));
                let (me, k, list) = (self.clone(), key, CATS[ci].2);
                spawn_bg(move || crate::emojifont::atlas(ci, list, cell_px, glyph_px), move |r| {
                    let st = match r {
                        Ok(img) => AtlasSt::Ready(Rc::new(img), list.split_whitespace().count() as u32),
                        Err(e) => {
                            uzcoders::prelude::report_error("emoji atlasi", &e);
                            AtlasSt::Failed(Instant::now())
                        }
                    };
                    ATLAS.with(|a| a.borrow_mut().insert(k, st));
                    me.gate_rev.set(me.gate_rev.get() + 1);
                });
            }
        }
        None
    }

    /// UZ: Yozish maydonidagi emoji uchun rasm: maxsus emoji (shaxsiy belgi) -> uning rasmi; oddiy emoji -> atlasdagi katak; boshqasi -> tizim shriftidan chizilgan rasm.
    /// EN: The image for an emoji in the input field: a custom emoji (private-use char) -> its picture; a plain emoji -> a cell of the atlas; anything else -> an image drawn with the system font.
    /// RU: Изображение эмодзи в поле ввода: пользовательское эмодзи (символ PUA) -> его картинка; обычное эмодзи -> ячейка атласа; остальное -> картинка, нарисованная системным шрифтом.
    pub(super) fn input_emoji_image(self: &Rc<Self>, cluster: &str) -> Option<(Rc<Image>, Option<(u32, u32, u32, u32)>)> {
        if let Some(tok) = ce_token(cluster) {
            // [ce:nom:id:zaxira]
            let id = tok.trim_end_matches(']').split(':').nth(2).and_then(|s| s.parse::<i64>().ok());
            if let Some(id) = id {
                let img = self.tg_img(&format!("ce{id:x}"), move || {
                    let d = tgmedia::emoji_doc(id).ok_or_else(|| "maxsus emoji hujjati topilmadi".to_string())?;
                    tgmedia::image(&d, 96)
                });
                if let Some(i) = img {
                    return Some((i, None));
                }
            }
            // Rasm tayyor bo'lguncha zaxira emoji.
            let fb = crate::tgmedia::plain_text(&tok);
            return self.input_emoji_image(&fb);
        }
        let dens = density().max(0.5);
        if let Some((ci, ix)) = cat_pos(cluster) {
            let cell = (32.0 * dens).round() as u32;
            let (img, _n) = self.tgp_atlas_image(ci, ix)?;
            let cols = crate::emojifont::ATLAS_COLS;
            return Some((img, Some(((ix as u32 % cols) * cell, (ix as u32 / cols) * cell, cell, cell))));
        }
        self.rich_poll(cluster, 26.0, 40.0, 0xFFFFFFFF, false).map(|(i, _, _)| (i, None))
    }



    fn tgp_sets_of(&self, p: &Rc<TgPanel>, tab: u8) -> Vec<TgSet> {
        let d = p.data.borrow();
        if tab == 0 { d.emoji_sets.clone().unwrap_or_default() } else { d.sticker_sets.clone().unwrap_or_default() }
    }

    /// UZ: Maxsus emoji katagi: statik rasm, `.tgs` bo'lsa animatsiya; bosilsa matnga `[ce:…]`, ushlansa katta ko'rinish. EN: A custom emoji cell: a static image, animated if `.tgs`; a tap inserts `[ce:…]`, a hold opens the large preview. RU: Ячейка пользовательского эмодзи: статичная картинка, при `.tgs` — анимация; нажатие вставляет `[ce:…]`, удержание — крупный просмотр.
    fn tgp_custom_cell(self: &Rc<Self>, p: &Rc<TgPanel>, d: &TgDoc, set: &TgSet, cell: f32, on_emoji: Rc<dyn Fn(String)>) -> Element {
        let (dd, s2) = (d.clone(), set.clone());
        let img = self.tg_img(&format!("d{:x}", d.id), move || load_doc(Some(s2), false, dd, 96));
        let inner = match img {
            Some(im) => image(&im, cell - 10.0, cell - 10.0),
            None => Self::tgp_placeholder(cell - 10.0, cell - 10.0, 8.0),
        };
        let play = d.kind == "tgs" && self.tgp_may_play(p);
        let px = ((cell - 10.0) * density()).clamp(48.0, 96.0) as u32;
        let inner = self.tg_anim_el(&format!("d{:x}", d.id), d, Some(set.clone()), px, 36, 24.0, cell - 10.0, cell - 10.0, play, inner);
        let (cb, tok) = (on_emoji, tgmedia::emoji_token(&set.short, d));
        let hold = self.tgp_hold(p, PKind::Emoji, d.clone(), Some(set.clone()), String::new());
        column(vec![inner]).center().width(cell).height(cell).on_click(move || cb(tok.clone())).on_hold(200, hold)
    }

    /// UZ: Teri rangi oynasi: asosiy emoji + 5 rang; tanlansa saqlanadi va matnga qo'shiladi. EN: The skin tone popup: the base emoji + 5 tones; the choice is remembered and inserted. RU: Окно оттенков кожи: базовое эмодзи + 5 оттенков; выбор запоминается и вставляется.
    fn tgp_tone_popup(self: &Rc<Self>, p: &Rc<TgPanel>, t: &ToneMenu, on_emoji: Rc<dyn Fn(String)>) -> Element {
        let mut cells: Vec<Element> = Vec::new();
        for tone in 0..=5usize {
            let e = with_tone(&t.base, tone);
            let (me, pp, base, cb) = (self.clone(), p.clone(), t.base.clone(), on_emoji.clone());
            cells.push(column(vec![self.rich_el(&e, 26.0, 40.0, 0xFFFFFFFF, false)]).center().width(40.0).height(40.0).radius(10.0).on_click(move || {
                pp.tones.borrow_mut().insert(base.clone(), tone);
                push_recent(&base);
                cb(with_tone(&base, tone));
                pp.tone_menu.set(None);
                me.gate_rev.set(me.gate_rev.get() + 1);
            }));
        }
        let pp = p.clone();
        let scrim = column(vec![]).grow(1.0).on_click(move || pp.tone_menu.set(None));
        let popup = Self::pglass(row(cells).gap(2.0).padding(6.0)).radius(18.0).margin_each(t.x, t.y, 0.0, 0.0);
        stack(vec![scrim, column(vec![row(vec![popup])])])
    }

    // ───────────────────────── Stikerlar sahifasi / Stickers page ─────────────────────────

    fn tgp_sticker_page(self: &Rc<Self>, p: &Rc<TgPanel>, pw: f32, off_dp: f32, on_send: Rc<dyn Fn(String, &'static str)>) -> (Layout, Vec<(SecIcon, bool)>, Element) {
        let sets = p.data.borrow().sticker_sets.clone();
        let Some(sets) = sets else {
            return (Layout::new(), Vec::new(), self.tgp_loading(p, "sets_s"));
        };
        if sets.is_empty() {
            return (Layout::new(), Vec::new(), Self::tgp_empty("Telegram hisobingizda stiker to'plami yo'q"));
        }
        let cols = ((pw / 72.0).floor() as usize).clamp(3, 6);
        let cell = ((pw - 10.0) / cols as f32).floor();
        let mut lay = Layout::new();
        for (si, s) in sets.iter().enumerate() {
            lay.secs.push((lay.rows.len(), SecIcon::Set(s.clone())));
            lay.push(RowKind::Header(if s.title.is_empty() { s.short.clone() } else { s.title.clone() }), HEADER_H);
            let n = p.data.borrow().docs.get(&s.id).map_or(s.count.max(0) as usize, |d| d.len());
            let mut start = 0;
            while start < n {
                let k = cols.min(n - start);
                lay.push(RowKind::Docs(si, start, k), cell);
                start += k;
            }
        }
        let active = lay.active(off_dp);
        let strip: Vec<(SecIcon, bool)> = lay.secs.iter().enumerate().map(|(i, (_, k))| (if let SecIcon::Set(s) = k { SecIcon::Set(s.clone()) } else { SecIcon::Recent }, i == active)).collect();
        let (me, pp, heights) = (self.clone(), p.clone(), lay.heights.clone());
        let rows = std::mem::take(&mut lay.rows);
        let sets2 = sets.clone();
        let list = lazy_column_var(&p.scroll[2], &heights, move |i| match &rows[i] {
            RowKind::Header(t) => Self::tgp_header(t),
            RowKind::Docs(si, start, n) => {
                let set = sets2[*si].clone();
                me.tgp_need_docs(&pp, &set);
                let docs = pp.data.borrow().docs.get(&set.id).cloned();
                let cells: Vec<Element> = (*start..*start + *n)
                    .map(|k| match docs.as_ref().and_then(|d| d.get(k)) {
                        Some(d) => me.tgp_sticker_cell(&pp, d, &set, cell, on_send.clone()),
                        None => column(vec![Self::tgp_placeholder(cell - 16.0, cell - 16.0, 10.0)]).center().width(cell).height(cell),
                    })
                    .collect();
                row(cells)
            }
            _ => row(vec![]),
        });
        (Layout { rows: Vec::new(), heights, tops: lay.tops, secs: lay.secs }, strip, list)
    }

    fn tgp_sticker_cell(self: &Rc<Self>, p: &Rc<TgPanel>, d: &TgDoc, set: &TgSet, cell: f32, on_send: Rc<dyn Fn(String, &'static str)>) -> Element {
        let (dd, s2) = (d.clone(), set.clone());
        let img = self.tg_img(&format!("d{:x}", d.id), move || load_doc(Some(s2), false, dd, 160));
        let inner = match img {
            Some(im) => {
                let k = ((cell - 8.0) / im.w as f32).min((cell - 8.0) / im.h as f32);
                image(&im, im.w as f32 * k, im.h as f32 * k)
            }
            None => Self::tgp_placeholder(cell - 16.0, cell - 16.0, 10.0),
        };
        let play = d.kind == "tgs" && self.tgp_may_play(p);
        let px = ((cell - 8.0) * density()).clamp(96.0, 200.0) as u32;
        let inner = self.tg_anim_el(&format!("d{:x}", d.id), d, Some(set.clone()), px, 45, 24.0, cell - 8.0, cell - 8.0, play, inner);
        let rf = tgmedia::sticker_ref(&set.short, d);
        let hold = self.tgp_hold(p, PKind::Sticker, d.clone(), Some(set.clone()), rf.clone());
        let cb = on_send;
        column(vec![inner]).center().width(cell).height(cell).on_click(move || cb(rf.clone(), "sticker")).on_hold(200, hold)
    }

    // ───────────────────────── GIF sahifasi / GIF page ─────────────────────────

    fn tgp_gif_page(self: &Rc<Self>, p: &Rc<TgPanel>, pw: f32, on_send: Rc<dyn Fn(String, &'static str)>) -> (Layout, Vec<(SecIcon, bool)>, Element) {
        let gifs = p.data.borrow().gifs.clone();
        let Some(gifs) = gifs else {
            return (Layout::new(), Vec::new(), self.tgp_loading(p, "gifs"));
        };
        if gifs.is_empty() {
            return (Layout::new(), Vec::new(), Self::tgp_empty("Telegram'da saqlangan GIF yo'q"));
        }
        let rows = gif_rows(&gifs, pw - 10.0);
        let heights: Vec<f32> = rows.iter().map(|(_, h)| *h + 2.0).collect();
        let (me, pp) = (self.clone(), p.clone());
        let gifs2 = gifs.clone();
        let list = lazy_column_var(&p.scroll[1], &heights, move |i| {
            let (idx, h) = &rows[i];
            let a_sum: f32 = idx.iter().map(|k| Self::gif_aspect(&gifs2[*k])).sum();
            let gaps = (idx.len() - 1) as f32 * 2.0;
            let cells: Vec<Element> = idx
                .iter()
                .map(|k| {
                    let d = &gifs2[*k];
                    let w = if idx.len() == 1 { Self::gif_aspect(d) * *h } else { (Self::gif_aspect(d) / a_sum) * (pw - 10.0 - gaps) };
                    let dd = d.clone();
                    let img = me.tg_img(&format!("d{:x}", d.id), move || load_doc(None, true, dd, 320));
                    let inner: Element = match img {
                        Some(im) => stack(vec![image_fill(&im)]).width(w).height(*h).radius(8.0),
                        None => Self::tgp_placeholder(w, *h, 8.0),
                    };
                    let play = me.tgp_may_play(&pp);
                    let px = (w.max(*h) * density()).clamp(96.0, 256.0) as u32;
                    let inner = me.tg_anim_el(&format!("d{:x}", d.id), d, None, px, 24, 12.0, w, *h, play, inner);
                    let rf = tgmedia::gif_ref(d);
                    let hold = me.tgp_hold(&pp, PKind::Gif, d.clone(), None, rf.clone());
                    let cb = on_send.clone();
                    inner.on_click(move || cb(rf.clone(), "gif")).on_hold(200, hold)
                })
                .collect();
            column(vec![row(cells).gap(2.0)]).padding_each(0.0, 0.0, 0.0, 2.0)
        });
        (Layout::new(), Vec::new(), list)
    }

    fn gif_aspect(d: &TgDoc) -> f32 {
        if d.w > 0 && d.h > 0 { d.w as f32 / d.h as f32 } else { 1.0 }.clamp(0.4, 2.5)
    }

    // ───────────────────────── Uzoq bosish: katta ko'rinish / Long press preview ─────────────────────────

    /// UZ: Uzoq bosish ishlovchisi: katta ko'rinish; tepaga 55 dp surilsa "Yuborish" menyusi. EN: The long-press handler: a large preview; a 55 dp swipe up opens the "Send" menu. RU: Обработчик долгого нажатия: крупный просмотр; свайп вверх на 55 dp открывает меню «Отправить».
    fn tgp_hold(self: &Rc<Self>, p: &Rc<TgPanel>, kind: PKind, doc: TgDoc, set: Option<TgSet>, rf: String) -> impl Fn(Hold) + 'static {
        let (me, pp) = (self.clone(), p.clone());
        move |h| match h {
            Hold::Start => {
                pp.preview.set(Some(Rc::new(Preview { kind, doc: doc.clone(), set: set.clone(), rf: rf.clone(), menu: false })));
                me.gate_rev.set(me.gate_rev.get() + 1);
            }
            Hold::Move(dy) => {
                if let Some(cur) = pp.preview.get() {
                    let menu = kind != PKind::Emoji && dy < PREVIEW_MENU_DY;
                    if cur.menu != menu {
                        let mut n = (*cur).clone();
                        n.menu = menu;
                        pp.preview.set(Some(Rc::new(n)));
                        me.gate_rev.set(me.gate_rev.get() + 1);
                    }
                }
            }
            Hold::End(dy) => {
                let cur = pp.preview.get();
                pp.preview.set(None);
                me.gate_rev.set(me.gate_rev.get() + 1);
                if let Some(c) = cur {
                    if c.kind != PKind::Emoji && dy < PREVIEW_MENU_DY {
                        let cb = pp.send_cb.borrow().clone();
                        if let Some(cb) = cb {
                            cb(c.rf.clone(), if c.kind == PKind::Gif { "gif" } else { "sticker" });
                        }
                    }
                }
            }
        }
    }

    /// UZ: Katta ko'rinish (butun oyna ustida): qorong'i fon, markazda katta stiker/GIF (stikerda tepasida emoji), tepaga surilganda "Yuborish". EN: The large preview (over the whole window): a dark scrim, a big sticker/GIF in the centre (with its emoji above a sticker), a "Send" pill when swiped up. RU: Крупный просмотр (поверх всего окна): тёмный фон, по центру большой стикер/GIF (над стикером — эмодзи), при свайпе вверх — «Отправить».
    pub(super) fn tg_preview_view(self: &Rc<Self>, p: &Rc<TgPanel>) -> Option<Element> {
        let pv = p.preview.get()?;
        let (w, h) = window_dp();
        let size = (w * 0.72).min(h * 0.5).min(320.0);
        let d = pv.doc.clone();
        let still = {
            let dd = d.clone();
            match self.tg_img(&format!("pv{:x}", d.id), move || tgmedia::image(&dd, 512)) {
                Some(im) => {
                    let k = (size / im.w as f32).min(size / im.h as f32);
                    image(&im, im.w as f32 * k, im.h as f32 * k)
                }
                None => column(vec![spinner(28.0, 3.0, Color::WHITE)]).center().width(size).height(size),
            }
        };
        let big = self.tg_anim_el(&format!("pv{:x}", d.id), &d, pv.set.clone(), (size * density()).clamp(128.0, 288.0) as u32, 60, 30.0, size, size, true, still);
        let mut col: Vec<Element> = Vec::new();
        if pv.kind != PKind::Gif && !d.emoji.is_empty() {
            col.push(self.rich_el(&d.emoji, 30.0, 60.0, 0xFFFFFFFF, false).margin_each(0.0, 0.0, 0.0, 14.0));
        }
        col.push(big);
        if pv.kind != PKind::Emoji {
            let on = pv.menu;
            col.push(
                column(vec![tx("Yuborish", 15.0, Color::WHITE).bold()])
                    .center()
                    .padding_xy(26.0, 12.0)
                    .radius(22.0)
                    .bg(if on { ACCENT.with_opacity(0.9) } else { Color::rgba(255, 255, 255, 30) })
                    .margin_each(0.0, 18.0, 0.0, 0.0),
            );
        }
        Some(stack(vec![column(vec![]).bg(Color::rgba(14, 10, 8, 235)), column(vec![column(col).align(Align::Center)]).center()]))
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
        assert_eq!(pop_last_char("a👍\u{1F3FD}"), "a");
    }

    #[test]
    fn toifalar_bosh_emas() {
        for (_, _, list) in CATS {
            assert!(list.split_whitespace().count() > 20);
        }
    }

    #[test]
    fn teri_rangi_qoshiladi() {
        assert_eq!(with_tone("👍", 0), "👍");
        assert_eq!(with_tone("👍", 1), "👍\u{1F3FB}");
        assert_eq!(with_tone("👍", 5), "👍\u{1F3FF}");
        assert_eq!(with_tone("✌\u{FE0F}", 3), "✌\u{1F3FD}");
        assert!(tone_base("👍").is_some());
        assert!(tone_base("😀").is_none());
    }

    #[test]
    fn gif_qatorlari_kenglikni_toldiradi() {
        let d = |w, h| TgDoc { w, h, ..Default::default() };
        let docs = vec![d(200, 100), d(100, 100), d(100, 200), d(300, 100), d(100, 100)];
        let rows = gif_rows(&docs, 300.0);
        assert!(rows.len() >= 2);
        for (idx, h) in &rows[..rows.len() - 1] {
            let a: f32 = idx.iter().map(|k| docs[*k].w as f32 / docs[*k].h as f32).sum();
            let gaps = (idx.len() - 1) as f32 * 2.0;
            assert!((a * h + gaps - 300.0).abs() < 1.0 || *h >= 144.0, "{idx:?} {h}");
        }
        assert_eq!(rows.iter().map(|(i, _)| i.len()).sum::<usize>(), 5);
    }

    fn doc(id: i64, kind: &str) -> TgDoc {
        TgDoc { id, hash: id, fref: "ab".into(), dc: 2, kind: kind.into(), thumb: "m".into(), emoji: "😀".into(), custom: false, w: 512, h: 512 }
    }

    fn set(id: i64, n: i64) -> TgSet {
        TgSet { id, hash: id, title: format!("To'plam {id}"), short: format!("set{id}"), count: n, thumb_ver: 0 }
    }

    /// UZ: Panel har uch tabda xatosiz quriladi (soxta ma'lumot bilan), uzoq bosish ko'rinishi ham. EN: The panel builds on all three tabs without errors (fake data), and so does the long-press preview. RU: Панель строится на всех трёх вкладках без ошибок (на фиктивных данных), как и просмотр при долгом нажатии.
    #[test]
    fn maxsus_emoji_maydonda_bitta_belgi() {
        let tok = "[ce:pack_1:123:😀]";
        let c = ce_compact(tok);
        assert_eq!(c.chars().count(), 1);
        assert_eq!(ce_compact(tok), c);
        let s = format!("salom {c}!");
        assert_eq!(ce_expand(&s), "salom [ce:pack_1:123:😀]!");
        assert_eq!(ce_plain(&s), "salom 😀!");
        assert_eq!(pop_last_char(&format!("a{c}")), "a");
    }

    #[test]
    fn emoji_atlas_oryni_topiladi() {
        assert_eq!(cat_pos("😀"), Some((0, 0)));
        let (ci, ix) = cat_pos("🐱").unwrap();
        assert_eq!(CATS[ci].2.split_whitespace().nth(ix), Some("🐱"));
        assert!(cat_pos("yo'q").is_none());
        assert!(CATS.iter().all(|c| c.2.split_whitespace().count() as u32 <= 10 * 40));
    }

    #[test]
    fn panel_uch_tabda_quriladi() {
        let home = Home::new("test".into(), Rc::new(|| {}));
        let panel = Rc::new(TgPanel::new());
        {
            let mut d = panel.data.borrow_mut();
            d.emoji_sets = Some(vec![set(1, 20), set(2, 7)]);
            d.sticker_sets = Some(vec![set(3, 33), set(4, 5)]);
            d.gifs = Some((10..30).map(|i| doc(i, "mp4")).collect());
            d.docs.insert(1, (100..120).map(|i| doc(i, "tgs")).collect());
            d.docs.insert(3, (200..233).map(|i| doc(i, "webp")).collect());
            d.pending.insert("sets_e".into());
            d.pending.insert("sets_s".into());
            d.pending.insert("gifs".into());
        }
        let font = Rc::new(uzcoders::prelude::Font::parse(include_bytes!("../../assets/fonts/Roboto-Regular.ttf")).unwrap());
        let (h2, p2) = (home.clone(), panel.clone());
        let mut ui = Ui::new(font, move || {
            let nop: Rc<dyn Fn(String)> = Rc::new(|_| {});
            let nop2: Rc<dyn Fn(String, &'static str)> = Rc::new(|_, _| {});
            let nop3: Rc<dyn Fn()> = Rc::new(|| {});
            let mut layers = vec![h2.tg_panel_view(&p2, nop, nop2, nop3.clone(), nop3)];
            if let Some(o) = h2.tg_preview_view(&p2) {
                layers.push(o);
            }
            stack(layers)
        });
        for tab in 0..3u8 {
            panel.tab.set(tab);
            for _ in 0..3 {
                ui.frame(Size::new(400.0, 800.0));
            }
            // UZ: Ko'z bilan tekshirish uchun: TGPANEL_PNG_DIR berilsa, har tab PNG ga chiziladi. EN: For eyeballing: with TGPANEL_PNG_DIR set, each tab is drawn to a PNG. RU: Для визуальной проверки: при заданном TGPANEL_PNG_DIR каждая вкладка рисуется в PNG.
            if let Ok(dir) = std::env::var("TGPANEL_PNG_DIR") {
                let f = Rc::new(uzcoders::prelude::Font::parse(include_bytes!("../../assets/fonts/Roboto-Regular.ttf")).unwrap());
                let mut c = Canvas::new(400, 800, f);
                c.clear(Color::rgba(40, 32, 28, 255));
                c.execute(&ui.frame(Size::new(400.0, 800.0)));
                let _ = std::fs::write(format!("{dir}/panel_tab{tab}.png"), c.to_png());
            }
        }
        // Teri rangi oynasi va uzoq bosish ko'rinishi.
        panel.tone_menu.set(Some(Rc::new(ToneMenu { base: "👍".into(), x: 10.0, y: 50.0 })));
        panel.preview.set(Some(Rc::new(Preview { kind: PKind::Sticker, doc: doc(200, "webp"), set: Some(set(3, 33)), rf: "tgs.set3.200".into(), menu: true })));
        let cmds = ui.frame(Size::new(400.0, 800.0));
        assert!(!cmds.is_empty());
        // Ro'yxat pastga surilganda ham xatosiz.
        panel.tab.set(2);
        panel.scroll[2].set_offset(900.0);
        ui.frame(Size::new(400.0, 800.0));
        ui.frame(Size::new(400.0, 800.0));
        eprintln!("JOYLASHUV:\n{}", uzcoders::prelude::errors_text());
    }
}
