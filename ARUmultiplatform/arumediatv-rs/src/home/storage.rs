//! Xotiradan foydalanish (Kotlin `StorageScreen`): toifalar bo'yicha kesh hajmi, tanlab tozalash — shisha.

use super::*;
use crate::data::format_bytes;
use std::cell::RefCell;

const COLORS: [Color; 5] = [Color::rgb(0xE2, 0x62, 0x0F), Color::rgb(0x7C, 0x3A, 0xED), Color::rgb(0x14, 0xB8, 0xA6), Color::rgb(0xF5, 0x9E, 0x0B), Color::rgb(0x25, 0x63, 0xEB)];

pub struct StoragePage {
    /// (nom, bayt, papkalar)
    slices: RefCell<Vec<(String, u64, Vec<String>)>>,
    loaded: State<bool>,
    off: State<Vec<String>>,
    confirm: State<bool>,
    snack: State<String>,
    scroll: ScrollState,
}

fn dir_bytes(p: &std::path::Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(p) else { return std::fs::metadata(p).map(|m| m.len()).unwrap_or(0) };
    rd.flatten().map(|e| {
        let path = e.path();
        if path.is_dir() { dir_bytes(&path) } else { e.metadata().map(|m| m.len()).unwrap_or(0) }
    }).sum()
}

fn wipe(p: &str) {
    let path = std::path::Path::new(p);
    if let Ok(rd) = std::fs::read_dir(path) {
        for e in rd.flatten() {
            let q = e.path();
            let _ = if q.is_dir() { std::fs::remove_dir_all(&q) } else { std::fs::remove_file(&q) };
        }
    }
}

fn measure() -> Vec<(String, u64, Vec<String>)> {
    let files = uzcoders::android::files_dir();
    let cache = std::path::Path::new(&files).parent().map(|p| p.join("cache").to_string_lossy().into_owned()).unwrap_or_default();
    let defs: Vec<(&str, Vec<String>)> = vec![
        ("Videolar", vec![format!("{files}/video_byte_cache")]),
        ("Posterlar", vec![format!("{files}/posters")]),
        ("Emoji, GIF va stikerlar", vec![format!("{files}/aru_packs")]),
        ("Vaqtinchalik fayllar", vec![cache]),
    ];
    let mut out: Vec<(String, u64, Vec<String>)> = defs
        .into_iter()
        .map(|(n, dirs)| (n.to_string(), dirs.iter().map(|d| dir_bytes(std::path::Path::new(d))).sum::<u64>(), dirs))
        .filter(|(_, b, _)| *b > 0)
        .collect();
    out.sort_by(|a, b| b.1.cmp(&a.1));
    out
}

impl Home {
    pub(super) fn open_storage(self: &Rc<Self>) {
        let p = Rc::new(StoragePage {
            slices: RefCell::new(Vec::new()),
            loaded: State::new(false),
            off: State::new(Vec::new()),
            confirm: State::new(false),
            snack: State::new(String::new()),
            scroll: ScrollState::new(),
        });
        let p2 = p.clone();
        spawn_bg(measure, move |list| {
            *p2.slices.borrow_mut() = list;
            p2.loaded.set(true);
        });
        self.push(pages::Page::Storage(p));
    }

    pub(super) fn storage_page(self: &Rc<Self>, p: &Rc<StoragePage>) -> Element {
        if !p.loaded.get() {
            return self.scaffold("Xotiradan foydalanish", column(vec![spinner(32.0, 3.0, ACCENT2)]).center());
        }
        let slices = p.slices.borrow().clone();
        let off = p.off.get();
        let sel_total: u64 = slices.iter().filter(|s| !off.contains(&s.0)).map(|s| s.1).sum();
        let total: u64 = slices.iter().map(|s| s.1).sum();
        let mut kids: Vec<Element> = Vec::new();
        // Ilova hajmi paneli: rangli chiziq.
        let mut bar: Vec<Element> = Vec::new();
        for (i, s) in slices.iter().enumerate() {
            bar.push(column(vec![]).bg(COLORS[i % COLORS.len()]).weight((s.1 as f32 / total.max(1) as f32).max(0.02)).height(14.0));
        }
        kids.push(
            Self::pglass(
                column(vec![
                    tx(if total == 0 { "Kesh bo'sh".to_string() } else { format_bytes(total as i64) }, 28.0, Color::WHITE).black(),
                    tx("Ilova keshi", 13.0, Color::rgba(255, 255, 255, 150)),
                    gap_box(0.0, 14.0),
                    if bar.is_empty() { gap_box(0.0, 14.0) } else { row(bar).radius(7.0).height(14.0).bg(Color::rgba(255, 255, 255, 20)) },
                ])
                .padding(18.0),
            )
            .radius(26.0)
            .margin_each(0.0, 0.0, 0.0, 12.0),
        );
        for (i, s) in slices.iter().enumerate() {
            let on = !off.contains(&s.0);
            let (pc, name) = (p.clone(), s.0.clone());
            kids.push(
                Self::pglass(
                    row(vec![
                        column(vec![]).width(14.0).height(14.0).radius(7.0).bg(COLORS[i % COLORS.len()]),
                        tx(s.0.clone(), 15.0, Color::WHITE).weight(1.0),
                        tx(format_bytes(s.1 as i64), 14.0, Color::rgba(255, 255, 255, 190)).bold(),
                        column(vec![if on { tx("✓", 14.0, Color::WHITE).bold() } else { gap_box(0.0, 0.0) }])
                            .center()
                            .width(24.0)
                            .height(24.0)
                            .radius(12.0)
                            .bg(if on { ACCENT } else { Color::rgba(255, 255, 255, 25) }),
                    ])
                    .gap(12.0)
                    .align(Align::Center)
                    .padding_xy(16.0, 14.0)
                    .on_click(move || {
                        let mut o = pc.off.get();
                        if let Some(i) = o.iter().position(|x| *x == name) {
                            o.remove(i);
                        } else {
                            o.push(name.clone());
                        }
                        pc.off.set(o);
                    }),
                )
                .radius(20.0)
                .margin_each(0.0, 0.0, 0.0, 8.0),
            );
        }
        let pc = p.clone();
        let can = sel_total > 0;
        kids.push(
            column(vec![tx(if can { format!("Keshni tozalash · {}", format_bytes(sel_total as i64)) } else { "Tozalash uchun toifa tanlang".into() }, 15.0, Color::WHITE).bold()])
                .center()
                .padding_xy(0.0, 16.0)
                .radius(28.0)
                .gradient(if can { ACCENT } else { ACCENT.with_opacity(0.4) }, if can { ACCENT2 } else { ACCENT2.with_opacity(0.4) })
                .margin_each(0.0, 8.0, 0.0, 0.0)
                .on_click(move || {
                    if can {
                        pc.confirm.set(true);
                    }
                }),
        );
        kids.push(tx("Barcha videolar, rasmlar, stikerlar va emojilar serverda qoladi — kerak bo'lsa ularni qayta yuklab olishingiz mumkin.", 12.0, Color::rgba(255, 255, 255, 120)).padding_xy(6.0, 12.0));
        kids.push(gap_box(0.0, 28.0));
        let mut layers = vec![self.scaffold("Xotiradan foydalanish", scroll_column(&p.scroll, kids).padding_each(16.0, 4.0, 16.0, 0.0))];
        if p.confirm.get() {
            let (pc, pw) = (p.clone(), p.clone());
            let slices2 = slices.clone();
            let off2 = off.clone();
            let panel = column(vec![
                tx("Keshni tozalash", 18.0, Color::WHITE).bold(),
                gap_box(0.0, 10.0),
                tx(format!("Tanlangan fayllar ({}) telefondan o'chiriladi.", format_bytes(sel_total as i64)), 14.0, Color::rgba(255, 255, 255, 200)),
                gap_box(0.0, 18.0),
                row(vec![
                    spacer(),
                    self.chip("Bekor qilish", false, move || pc.confirm.set(false)),
                    self.chip("Tozalash", true, move || {
                        pw.confirm.set(false);
                        let mut freed = 0u64;
                        for s in slices2.iter().filter(|s| !off2.contains(&s.0)) {
                            freed += s.1;
                            for d in &s.2 {
                                wipe(d);
                            }
                        }
                        pw.snack.set(format!("Kesh tozalandi · {}", format_bytes(freed as i64)));
                        let p3 = pw.clone();
                        spawn_bg(measure, move |list| *p3.slices.borrow_mut() = list);
                        let p4 = pw.clone();
                        spawn_bg(|| std::thread::sleep(std::time::Duration::from_millis(400)), move |_| p4.loaded.set(true));
                    }),
                ])
                .gap(8.0),
            ])
            .lens(Color::rgba(18, 18, 24, 120), GLASS_ZOOM)
            .radius(28.0)
            .padding(22.0)
            .margin_each(24.0, 0.0, 24.0, 0.0);
            let pc = p.clone();
            layers.push(column(vec![panel]).center().bg(Color::rgba(0, 0, 0, 120)).on_click(move || pc.confirm.set(false)));
        }
        let sn = p.snack.get();
        if !sn.is_empty() {
            let pc = p.clone();
            layers.push(column(vec![spacer(), tx(sn, 14.0, Color::WHITE).bold().bg(Color::rgba(30, 30, 36, 235)).radius(20.0).padding_xy(18.0, 10.0).margin_each(0.0, 0.0, 0.0, 40.0).on_click(move || pc.snack.set(String::new()))]).align(Align::Center));
        }
        stack(layers)
    }
}
