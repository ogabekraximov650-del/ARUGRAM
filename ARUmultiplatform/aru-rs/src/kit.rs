//! Qayta ishlatiladigan kichik vidjetlar.

use crate::art::{art, Art};
use crate::theme::*;
use std::rc::Rc;
use uzcoders::prelude::*;

#[derive(Clone, Copy)]
pub enum Tone {
    Light,
    Dark,
    Signal,
}

/// ARU belgisi (kichik PNG, bir marta ochiladi).
pub fn logo(size: f32) -> Element {
    thread_local! {
        static L: Rc<Image> = Rc::new(decode_png(include_bytes!("../assets/logo.png")).expect("logo"));
    }
    L.with(|l| image(l, size, size))
}

/// Kvadrat "plita" belgi: harf plita ichidan o'yilgandek. Burchak radiusi — logotipdagi 22%.
pub fn avatar(letter: &str, size: f32, tone: Tone) -> Element {
    let (bg, fg) = match tone {
        Tone::Light => (WHITE, BG),
        Tone::Dark => (S3, WHITE),
        Tone::Signal => (SIG, BG),
    };
    column(vec![disp(letter, size * 0.58, fg)]).center().width(size).height(size).radius(size * 0.223).bg(bg)
}

/// Ismdan birinchi harf.
pub fn initial(s: &str) -> String {
    s.chars().next().map(|c| c.to_string()).unwrap_or_default()
}

/// Shaffof fonli belgi tugmasi (42x42).
pub fn icon_btn(path: &'static str) -> Element {
    column(vec![icon(path, 24.0).color(WHITE)]).center().width(42.0).height(42.0)
}

/// Kvadrat amal tugmasi: tepada belgi, pastda yozuv.
pub fn key(path: &'static str, label: &str, on: bool) -> Element {
    let (bg, fg) = if on { (SIG, BG) } else { (S2, WHITE) };
    column(vec![icon(path, 20.0).color(fg), gap_box(0.0, 3.0), b(label, 11.0, fg)]).center().height(58.0).radius(16.0).bg(bg).weight(1.0)
}

/// Keng amal tugmasi (rasm ustida): belgi va yozuv yonma-yon.
pub fn wide_key(path: &'static str, label: &str, on: bool) -> Element {
    let (bg, fg) = if on { (SIG, BG) } else { (Color::rgba(255, 255, 255, 36), WHITE) };
    row(vec![icon(path, 22.0).color(fg), gap_box(6.0, 0.0), b(label, 13.0, fg)]).center().height(54.0).radius(17.0).bg(bg).weight(1.0)
}

/// Tanlov tugmasi (chip).
pub fn pill(label: &str, on: bool) -> Element {
    column(vec![b(label, 13.5, if on { BG } else { WHITE })]).center().height(36.0).padding_xy(14.0, 0.0).radius(12.0).bg(if on { WHITE } else { S2 })
}

/// Muqovali karta: rasm + ustiga qatlamlar, `r` burchak radiusi.
pub fn pic(a: Art, r: f32, layers: Vec<Element>) -> Element {
    let mut v = vec![image_fill(&art(a))];
    v.extend(layers);
    stack(v).radius(r).round_clip(r).bg(S2)
}

/// O'yilgan burchak: pastki-chap burchakda sahifa foni rangidagi plita; ichiga matn qo'yiladi.
/// Chap va pastki chekkalari tekis (karta burchagi bilan mos), faqat yuqori-o'ng burchagi yumaloq.
pub fn notch(w: f32, h: f32, r: f32, inner: Element) -> Element {
    stack(vec![
        column(vec![]).bg(BG).radius(r),
        column(vec![]).bg(BG).width(r).height(h).abs().margin_each(0.0, 0.0, 0.0, 0.0),
        column(vec![]).bg(BG).width(w).height(r).abs().margin_each(0.0, h - r, 0.0, 0.0),
        inner,
    ])
    .width(w)
    .height(h)
}

/// Elementni karta pastki-chap burchagiga qo'yadi.
pub fn at_bottom_left(e: Element) -> Element {
    column(vec![spacer(), row(vec![e, spacer()])])
}

/// Davomiylik yozuvi (o'yilgan burchakda).
pub fn dur_tab(dur: &str) -> Element {
    at_bottom_left(notch(64.0, 28.0, 14.0, column(vec![b(dur, 12.5, WHITE)]).center()))
}

/// Vaqt chizg'ichi: tirqishlar; bosilsa/surilsa `pos` (0..1) o'zgaradi.
pub fn ruler(pos: &State<f32>, ticks: usize, tall: f32, w: f32) -> Element {
    let p = pos.get();
    let kids: Vec<Element> = (0..ticks)
        .map(|i| {
            let on = (i as f32 + 0.5) / ticks as f32 <= p;
            let h = if i % 4 == 0 { tall } else { tall * 0.6 };
            column(vec![]).width(2.0).height(h).bg(if on { WHITE } else { Color::rgba(255, 255, 255, 80) })
        })
        .collect();
    let s = pos.clone();
    row(kids).align(Align::End).justify(Justify::SpaceBetween).width(w).height(tall + 10.0).on_drag(move |x, _| s.set(x.clamp(0.0, 1.0)))
}

/// "mm:ss" ko'rinishi.
pub fn clock(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// "24:01" -> soniyalar.
pub fn dur_secs(d: &str) -> f32 {
    let mut it = d.split(':');
    let m: f32 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0.0);
    let s: f32 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0.0);
    m * 60.0 + s
}
