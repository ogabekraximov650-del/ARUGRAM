//! Chat uchun emoji tanlash paneli (oddiy Unicode emoji; matn tizim shriftida chiziladi — `richtext.rs`).
//! O'z emoji/GIF/stiker to'plamlari tizimi Rust ilovadan olib tashlangan.

use super::*;

const EMOJIS: &str = "😀 😃 😄 😁 😆 😅 😂 🤣 😊 😇 🙂 😉 😍 🥰 😘 😋 😛 😜 🤪 😎 🤩 🥳 😏 😒 😞 😔 😟 😕 🙁 😣 😖 😫 😩 🥺 😢 😭 😤 😠 😡 🤬 🤯 😳 🥵 🥶 😱 😨 😰 😥 😓 🤗 🤔 🤭 🤫 😶 😐 😑 😬 🙄 😯 😮 😲 🥱 😴 🤤 😷 🤒 🤕 🤢 🤮 🤧 😈 👿 💀 💩 🤡 👻 👽 🤖 🙈 🙉 🙊 💋 💌 💘 💝 💖 💗 💓 💞 💕 💔 ❤️ 🧡 💛 💚 💙 💜 🖤 🤍 🤎 💯 💢 💥 💫 💦 💨 👍 👎 👌 ✌️ 🤞 🤟 🤘 👈 👉 👆 👇 ✋ 🤚 🖖 👋 🤙 💪 🙏 🤝 👏 🙌 👐 🤲 💅 🔥 ✨ ⭐ 🌟 🎉 🎊 🎁 🏆 🥇 🎵 🎶 🍕 🍔 🍟 🍿 🍰 🍫 🍺 ☕ 🌹 🌸 🌞 🌙 ⚡ 🌈 ☔ ❄️ 🐱 🐶 🦊 🐼 🐵 🦁 🐸 🐢 🦋 🎮 ⚽ 🏀 📱 💻 📷 🎬 🎧 🚀 ✅ ❌ ❓ ❗ ⚠️ 💤 👀 🧠";

pub struct EmojiPanel {
    pub(super) open: State<bool>,
    scroll: ScrollState,
}

impl EmojiPanel {
    pub fn new() -> EmojiPanel {
        EmojiPanel { open: State::new(false), scroll: ScrollState::new() }
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
    /// Emoji to'ri: faqat ko'rinadigan qatorlar quriladi (emoji rasmlari ham shularga yasaladi).
    /// Emoji tanlanganda `on_emoji`, o'chirish tugmasida `on_back` chaqiriladi.
    pub(super) fn emoji_view(self: &Rc<Self>, ep: &Rc<EmojiPanel>, on_emoji: Rc<dyn Fn(String)>, on_back: Rc<dyn Fn()>) -> Element {
        let (w, _) = window_dp();
        let per_row = 8usize;
        let cell = ((w - 24.0 - 16.0) / per_row as f32).floor();
        let all: Vec<String> = EMOJIS.split_whitespace().map(String::from).collect();
        let nrows = (all.len() + per_row - 1) / per_row + 1; // oxirgi qator — o'chirish tugmasi
        let (me, cb, back) = (self.clone(), on_emoji, on_back);
        let list = lazy_column(&ep.scroll, nrows, cell, move |i| {
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
        .height(250.0);
        Self::pglass(column(vec![list]).padding_each(10.0, 8.0, 10.0, 4.0)).radius(24.0)
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
