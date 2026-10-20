use super::*;
use std::cell::Cell;
use uzcoders_core::State;

fn font() -> Rc<Font> {
    Rc::new(Font::parse(include_bytes!("../../../assets/fonts/DejaVuSans.ttf")).unwrap())
}

fn tap(ui: &mut Ui, x: f32, y: f32) {
    ui.pointer(PointerEvent::new(PointerKind::Down, x, y));
    ui.pointer(PointerEvent::new(PointerKind::Up, x, y));
}

const SZ: Size = Size { w: 200.0, h: 300.0 };

#[test]
fn bosish_hisoblagichni_oshiradi() {
    let count = State::new(0i32);
    let c2 = count.clone();
    let mut ui = Ui::new(font(), move || {
        let c = c2.clone();
        column(vec![
            text(format!("{}", c2.get())),
            button("+", move || c.update(|v| *v += 1)),
        ])
        .padding(10.0)
    });
    ui.frame(SZ);
    let r = ui.hits[0].rect;
    tap(&mut ui, r.x + r.w / 2.0, r.y + r.h / 2.0);
    assert_eq!(count.get(), 1);
    assert!(ui.needs_frame());
    ui.frame(SZ);
    assert!(!ui.needs_frame());
}

#[test]
fn tashqarida_qoyvorish_bosmaydi() {
    let n = Rc::new(Cell::new(0));
    let n2 = n.clone();
    let mut ui = Ui::new(font(), move || {
        let n = n2.clone();
        column(vec![button("x", move || n.set(n.get() + 1))])
    });
    ui.frame(SZ);
    let r = ui.hits[0].rect;
    ui.pointer(PointerEvent::new(PointerKind::Down, r.x + 2.0, r.y + 2.0));
    ui.pointer(PointerEvent::new(PointerKind::Up, 190.0, 290.0));
    assert_eq!(n.get(), 0);
}

#[test]
fn zichlik_olchamlarni_oshiradi() {
    let mut ui = Ui::new(font(), || {
        column(vec![button("OK", || {}).width(100.0).height(40.0)])
    });
    ui.frame(SZ);
    let r1 = ui.hits[0].rect;
    ui.set_scale(2.0);
    assert!(ui.needs_frame());
    ui.frame(Size::new(400.0, 400.0));
    let r2 = ui.hits[0].rect;
    assert_eq!((r1.w, r1.h), (100.0, 40.0));
    assert_eq!((r2.w, r2.h), (200.0, 80.0));
}

fn list_ui(sc: &ScrollState, hits: &Rc<Cell<i32>>) -> Ui {
    let sc = sc.clone();
    let hits = hits.clone();
    Ui::new(font(), move || {
        let items = (0..20)
            .map(|i| {
                let h = hits.clone();
                column(vec![text(format!("Element {i}"))])
                    .height(50.0)
                    .on_click(move || h.set(i))
            })
            .collect();
        column(vec![scroll_column(&sc, items).grow(1.0)])
    })
}

#[test]
fn scroll_surish_va_chegara() {
    let sc = ScrollState::new();
    let clicked = Rc::new(Cell::new(-1));
    let mut ui = list_ui(&sc, &clicked);
    ui.frame(SZ);
    assert_eq!(sc.max_offset(), 20.0 * 50.0 - 300.0);
    // Yuqoriga 120 px suramiz: ofset oshadi, bosish ro'yxatdan o'tmaydi.
    ui.set_time(100);
    ui.pointer(PointerEvent::new(PointerKind::Down, 100.0, 250.0));
    ui.set_time(116);
    ui.pointer(PointerEvent::new(PointerKind::Move, 100.0, 200.0));
    ui.set_time(132);
    ui.pointer(PointerEvent::new(PointerKind::Move, 100.0, 130.0));
    ui.set_time(148);
    ui.pointer(PointerEvent::new(PointerKind::Up, 100.0, 130.0));
    assert!((sc.offset() - 120.0).abs() < 0.01, "{}", sc.offset());
    assert_eq!(clicked.get(), -1);
    // Tezlik bor: inersiya davom etadi.
    assert!(sc.velocity() > 0.0);
    assert!(ui.needs_frame());
    let before = sc.offset();
    ui.set_time(164);
    ui.frame(SZ);
    assert!(sc.offset() > before);
    // Juda uzoq inersiya chegarada to'xtaydi.
    for k in 0..400 {
        ui.set_time(180 + k * 16);
        ui.frame(SZ);
    }
    assert!(sc.offset() <= sc.max_offset() + 0.01);
    assert_eq!(sc.velocity(), 0.0);
}

#[test]
fn scroll_ichida_bosish_ofsetni_hisobga_oladi() {
    let sc = ScrollState::new();
    let clicked = Rc::new(Cell::new(-1));
    let mut ui = list_ui(&sc, &clicked);
    ui.frame(SZ);
    sc.set_offset(120.0);
    ui.frame(SZ);
    // y=10 da endi 120+10=130 -> 2-element (100..150).
    tap(&mut ui, 50.0, 10.0);
    assert_eq!(clicked.get(), 2);
}

#[test]
fn scroll_klipdan_tashqari_bosilmaydi() {
    let sc = ScrollState::new();
    let clicked = Rc::new(Cell::new(-1));
    let sc2 = sc.clone();
    let c2 = clicked.clone();
    let mut ui = Ui::new(font(), move || {
        let c = c2.clone();
        column(vec![
            scroll_column(
                &sc2,
                (0..10)
                    .map(|i| {
                        let c = c.clone();
                        column(vec![]).height(50.0).on_click(move || c.set(i))
                    })
                    .collect(),
            )
            .height(100.0),
            column(vec![]).height(200.0),
        ])
    });
    ui.frame(SZ);
    sc.set_offset(300.0);
    ui.frame(SZ);
    // y=150 scroll oynasidan pastda (oyna 0..100): kontent elementi bo'lsa ham bosilmasligi kerak.
    tap(&mut ui, 50.0, 150.0);
    assert_eq!(clicked.get(), -1);
    tap(&mut ui, 50.0, 10.0);
    assert_eq!(clicked.get(), 6);
}

#[test]
fn matn_kiritish_va_kursor() {
    let val = State::new(String::new());
    let v2 = val.clone();
    let mut ui = Ui::new(font(), move || {
        column(vec![field(&v2).hint("Ism")]).padding(10.0)
    });
    ui.frame(SZ);
    assert!(!ui.wants_keyboard());
    assert!(!ui.key_char('a'));
    let r = ui.fields[0].rect;
    tap(&mut ui, r.x + 5.0, r.y + r.h / 2.0);
    assert!(ui.wants_keyboard());
    ui.frame(SZ);
    for c in "salom".chars() {
        assert!(ui.key_char(c));
    }
    assert_eq!(val.get(), "salom");
    assert!(ui.key_backspace());
    assert_eq!(val.get(), "salo");
    // Boshiga bosib, kursorni qo'yamiz va belgi qo'shamiz.
    ui.frame(SZ);
    let r = ui.fields[0].rect;
    tap(&mut ui, r.x + 14.0, r.y + r.h / 2.0);
    ui.frame(SZ);
    ui.key_char('X');
    assert_eq!(val.get(), "Xsalo");
    // Orqaga: avval fokus olinadi.
    assert!(ui.key_back());
    assert!(!ui.wants_keyboard());
    assert!(!ui.key_back());
}

#[test]
fn filtr_chegara_format() {
    let val = State::new(String::new());
    let v2 = val.clone();
    fn digits(c: char) -> bool {
        c.is_ascii_digit()
    }
    fn fmt(s: &str) -> String {
        let d: Vec<char> = s.chars().filter(|c| c.is_ascii_digit()).collect();
        let mut out = String::new();
        for (i, c) in d.iter().enumerate() {
            if i == 2 || i == 5 || i == 7 {
                out.push(' ');
            }
            out.push(*c);
        }
        out
    }
    let mut ui = Ui::new(font(), move || {
        column(vec![field(&v2).filter(digits).max_len(9).format(fmt)]).padding(10.0)
    });
    ui.frame(SZ);
    let r = ui.fields[0].rect;
    tap(&mut ui, r.x + 5.0, r.y + 5.0);
    ui.frame(SZ);
    for c in "9a01b234567899".chars() {
        ui.key_char(c);
        ui.frame(SZ);
    }
    assert_eq!(val.get(), "90 123 45 67");
    assert!(ui.key_backspace());
    assert!(ui.key_backspace());
    assert_eq!(val.get(), "90 123 45");
    assert!(ui.key_backspace());
    assert_eq!(val.get(), "90 123 4");
}

#[test]
fn parol_nuqtalar() {
    let val = State::new("abc".to_string());
    let v2 = val.clone();
    let mut ui = Ui::new(font(), move || column(vec![field(&v2).password()]));
    let cmds = ui.frame(SZ);
    let shown = cmds.iter().find_map(|c| match c {
        DrawCmd::Text { text, .. } => Some(text.clone()),
        _ => None,
    });
    assert_eq!(shown.unwrap(), "\u{2022}\u{2022}\u{2022}");
}

#[test]
fn orqaga_tugmasi_navigatsiya() {
    #[derive(Clone, PartialEq, Debug)]
    enum R {
        Home,
        Detail,
    }
    let nav = Navigator::new(R::Home);
    let n2 = nav.clone();
    let mut ui = Ui::new(font(), move || match n2.current() {
        R::Home => column(vec![text("Home")]),
        R::Detail => column(vec![text("Detail")]),
    });
    let n3 = nav.clone();
    ui.set_back_handler(move || n3.pop());
    ui.frame(SZ);
    assert!(!ui.key_back());
    nav.push(R::Detail);
    assert!(ui.needs_frame());
    assert!(ui.key_back());
    assert_eq!(nav.current(), R::Home);
}

#[test]
fn grid_teng_kenglik() {
    let mut ui = Ui::new(font(), || {
        grid(
            2,
            10.0,
            (0..3)
                .map(|_| column(vec![text("x")]).on_click(|| {}))
                .collect(),
        )
    });
    ui.frame(Size::new(210.0, 300.0));
    assert_eq!(ui.hits.len(), 3);
    assert!((ui.hits[0].rect.w - 100.0).abs() < 0.5);
    assert!((ui.hits[1].rect.w - 100.0).abs() < 0.5);
    assert!((ui.hits[2].rect.w - 100.0).abs() < 0.5); // so'nggi qator ham teng ustunda
    assert!(ui.hits[2].rect.y > ui.hits[0].rect.y);
}

#[test]
fn chizish_buyruqlari() {
    let mut ui = Ui::new(font(), || {
        column(vec![text("Salom").size(20.0), button("OK", || {})])
    });
    let cmds = ui.frame(SZ);
    assert!(cmds.iter().any(|c| matches!(c, DrawCmd::RoundRect { .. })));
    assert_eq!(
        cmds.iter()
            .filter(|c| matches!(c, DrawCmd::Text { .. }))
            .count(),
        2
    );
}

#[test]
fn virtual_royxat_faqat_korinadiganni_quradi() {
    let sc = ScrollState::new();
    let built = Rc::new(Cell::new(0usize));
    let (s2, b2) = (sc.clone(), built.clone());
    let mut ui = Ui::new(font(), move || {
        let b = b2.clone();
        column(vec![lazy_column(&s2, 10_000, 50.0, move |i| {
            b.set(b.get() + 1);
            column(vec![text(format!("{i}"))])
        })
        .grow(1.0)])
    });
    ui.frame(SZ);
    ui.frame(SZ); // ko'rinish balandligi ma'lum bo'ldi
    built.set(0);
    ui.frame(SZ);
    assert!(built.get() < 20, "{}", built.get());
    assert_eq!(sc.max_offset(), 10_000.0 * 50.0 - 300.0);
    sc.set_offset(250_000.0);
    built.set(0);
    ui.frame(SZ);
    assert!(built.get() < 20);
    // Chiziq o'rtasida ham elementlar bor (bo'shliq yo'q).
    let cmds = ui.frame(SZ);
    let n = cmds
        .iter()
        .filter(|c| matches!(c, DrawCmd::Text { .. }))
        .count();
    assert!(n >= 6, "{n}");
}

#[test]
fn ime_tugallanmagan_va_ochirish() {
    let val = State::new(String::new());
    let v2 = val.clone();
    let mut ui = Ui::new(font(), move || column(vec![field(&v2)]).padding(10.0));
    ui.frame(SZ);
    let r = ui.fields[0].rect;
    tap(&mut ui, r.x + 5.0, r.y + 5.0);
    ui.frame(SZ);
    assert!(ui.ime_commit("sa"));
    ui.frame(SZ);
    ui.ime_composing("l");
    ui.frame(SZ);
    ui.ime_composing("lom");
    assert_eq!(val.get(), "salom");
    ui.ime_finish();
    ui.ime_commit("!");
    assert_eq!(val.get(), "salom!");
    ui.ime_delete(2, 0);
    assert_eq!(val.get(), "salo");
    ui.clear_focus();
    assert!(!ui.wants_keyboard());
}

#[test]
fn klaviatura_turi() {
    let val = State::new(String::new());
    let v2 = val.clone();
    let mut ui = Ui::new(font(), move || {
        column(vec![field(&v2).keyboard(KeyboardKind::Number)]).padding(10.0)
    });
    ui.frame(SZ);
    let r = ui.fields[0].rect;
    tap(&mut ui, r.x + 5.0, r.y + 5.0);
    assert_eq!(ui.keyboard_kind(), KeyboardKind::Number);
    assert_eq!(KeyboardKind::Number.android_input_type(), 2);
}

#[test]
fn shisha_va_dog_buyruqlari() {
    let mut ui = Ui::new(font(), || {
        column(vec![column(vec![text("x")])
            .glass(Color::rgba(255, 255, 255, 20))
            .radius(12.0)])
        .glows(vec![(0.5, 0.2, 100.0, Color::rgba(255, 100, 0, 80))])
    });
    let cmds = ui.frame(SZ);
    assert!(cmds.iter().any(|c| matches!(c, DrawCmd::Glow { .. })));
    assert!(cmds.iter().any(|c| matches!(c, DrawCmd::Glass { .. })));
}
