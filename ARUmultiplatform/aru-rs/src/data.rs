//! Namuna kontent. Haqiqiy ma'lumot (bot tokeni va kanal orqali) ulangach shu tuzilmalar to'ldiriladi.

use crate::art::Art;

#[derive(Clone, Copy)]
pub struct Video {
    pub art: Art,
    pub dur: &'static str,
    pub title: &'static str,
    pub ch: &'static str,
    pub views: &'static str,
    pub ago: &'static str,
    pub likes: &'static str,
}

pub const VIDEOS: [Video; 6] = [
    Video { art: Art::Mountain, dur: "24:01", title: "Qirq kun ichida Markaziy Osiyoni bosib o'tdik", ch: "Sayohat Kundaligi", views: "1,2 mln", ago: "3 kun oldin", likes: "45 ming" },
    Video { art: Art::Food, dur: "15:42", title: "Uyda 15 daqiqada tayyorlanadigan 5 ta tez ovqat", ch: "Oshxona Sirlari", views: "486 ming", ago: "1 hafta oldin", likes: "21 ming" },
    Video { art: Art::City, dur: "18:07", title: "Yangi telefon sinovi: bir oy foydalanib ko'rdik", ch: "Texno Sharh", views: "842 ming", ago: "2 kun oldin", likes: "30 ming" },
    Video { art: Art::Dunes, dur: "12:40", title: "Qizilqum: cho'l bo'ylab yolg'iz sayohat", ch: "Sayohat Kundaligi", views: "615 ming", ago: "2 hafta oldin", likes: "18 ming" },
    Video { art: Art::Forest, dur: "09:18", title: "Tog' o'rmonida bir kecha: chodirda", ch: "Sayohat Kundaligi", views: "398 ming", ago: "1 oy oldin", likes: "12 ming" },
    Video { art: Art::Tech, dur: "21:33", title: "Eng yaxshi 7 ta foydali ilova", ch: "Texno Sharh", views: "204 ming", ago: "3 hafta oldin", likes: "9 ming" },
];

/// Vaqtga bog'langan fikr: `at` — daqiqa:soniya, `frac` — video uzunligidagi joyi (0..1).
#[derive(Clone, Copy)]
pub struct Note {
    pub at: &'static str,
    pub frac: f32,
    pub who: &'static str,
    pub letter: &'static str,
    pub text: &'static str,
    pub likes: &'static str,
}

pub const NOTES: [Note; 4] = [
    Note { at: "03:12", frac: 0.13, who: "@malika.travel", letter: "M", text: "Bu joy qayerda? Xaritada ko'rsating", likes: "384" },
    Note { at: "09:12", frac: 0.38, who: "@sardor_k", letter: "S", text: "Ayniqsa shu qism juda yoqdi, rahmat!", likes: "1,2 ming" },
    Note { at: "14:40", frac: 0.61, who: "@jasur_1995", letter: "J", text: "Montaj va musiqa a'lo darajada", likes: "97" },
    Note { at: "20:05", frac: 0.84, who: "@dilnoza_u", letter: "D", text: "Samarqanddagi joy qaysi edi?", likes: "12" },
];

/// Oqim (tik qisqa video).
#[derive(Clone, Copy)]
pub struct Reel {
    pub title: &'static str,
    pub tags: &'static str,
    pub place: &'static str,
}

pub const REELS: [Reel; 3] = [
    Reel { title: "Tog'da tong qarshilash", tags: "#sayohat #tog", place: "Chimyon, 05:40" },
    Reel { title: "Choyxonada nonushta", tags: "#ovqat #samarqand", place: "Samarqand, 08:15" },
    Reel { title: "Kechki shahar chiroqlari", tags: "#shahar #tun", place: "Toshkent, 21:30" },
];

pub const CHANNELS: [&str; 3] = ["Sayohat Kundaligi", "Oshxona Sirlari", "Texno Sharh"];
