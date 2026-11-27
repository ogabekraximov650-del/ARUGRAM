//! "Yaratish" oynasi: 6 ta katta kvadrat tugma.

use crate::icons::*;
use crate::kit::*;
use crate::theme::*;
use crate::App;
use std::rc::Rc;
use uzcoders::prelude::*;

#[derive(Clone, Copy)]
enum Look {
    White,
    Signal,
    Dark,
}

fn tile(path: &'static str, title: &str, sub: &str, look: Look) -> Element {
    let (bg, fg, sub_c, ib, ifg) = match look {
        Look::White => (WHITE, BG, Color::hex(0x444444), BG, WHITE),
        Look::Signal => (SIG, BG, Color::hex(0x3A1A05), BG, WHITE),
        Look::Dark => (S2, WHITE, MUTE, WHITE, BG),
    };
    column(vec![
        column(vec![icon(path, 24.0).color(ifg)]).center().width(46.0).height(46.0).radius(14.0).bg(ib),
        spacer(),
        disp(title, 25.0, fg),
        t(sub, 12.0, sub_c).margin_each(0.0, 5.0, 0.0, 0.0),
    ])
    .padding(14.0)
    .height(150.0)
    .radius(24.0)
    .bg(bg)
}

impl App {
    pub fn create_view(self: &Rc<Self>, bottom: f32) -> Element {
        let (a, c) = (self.clone(), self.clone());
        let hint = |a: &Rc<App>, label: &'static str| {
            let a = a.clone();
            column(vec![t(label, 13.0, MUTE)]).center().padding_xy(0.0, 6.0).on_click(move || a.create.set(false))
        };
        let tiles = vec![
            tile(VIDEO, "Video", "Uzun video yuklash", Look::White),
            tile(SHORTS, "Oqim", "60 soniyagacha tik video", Look::Dark),
            tile(STORY, "Hikoya", "24 soat ko'rinadi", Look::Dark),
            tile(LIVE, "Jonli", "Hozir efirga chiqish", Look::Signal),
            tile(PHOTO, "Rasm", "Bir yoki bir nechta rasm", Look::Dark),
            tile(POST, "Post", "Matn va fikr ulashish", Look::Dark),
        ];
        let sheet = column(vec![
            row(vec![spacer(), column(vec![]).width(40.0).height(4.0).radius(2.0).bg(Color::hex(0x444444)), spacer()]),
            row(vec![disp("Yaratish", 40.0, WHITE), spacer(), column(vec![icon(CLOSE, 22.0).color(WHITE)]).center().width(44.0).height(44.0).radius(14.0).bg(S2).on_click(move || c.create.set(false))])
                .align(Align::Center)
                .margin_each(0.0, 14.0, 0.0, 14.0),
            grid(2, 8.0, tiles),
            hint(&a, "Yopish"),
        ])
        .padding_each(14.0, 10.0, 14.0, 12.0)
        .bg(Color::hex(0x0B0B0B))
        .border(1.0, LINE)
        .radius(28.0)
        .margin_each(8.0, 0.0, 8.0, 8.0 + bottom);
        stack(vec![column(vec![]).bg(Color::rgba(0, 0, 0, 170)).on_click(move || a.create.set(false)), column(vec![spacer(), sheet])])
    }
}
