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
    // UZ: Yuqoriga 120 px suramiz: ofset oshadi, bosish sifatida ro'yxatdan o'tmaydi.
    // EN: Drag up by 120 px: the offset grows and it is not registered as a tap.
    // RU: Протягиваем вверх на 120 px: смещение растёт, касание не считается нажатием.
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
    // UZ: Tezlik bor: inersiya davom etadi.
    // EN: There is velocity: the fling continues.
    // RU: Скорость есть: инерция продолжается.
    assert!(sc.velocity() > 0.0);
    assert!(ui.needs_frame());
    let before = sc.offset();
    ui.set_time(164);
    ui.frame(SZ);
    assert!(sc.offset() > before);
    // UZ: Juda uzoq inersiya chegarada to'xtashi kerak.
    // EN: A very long fling must stop at the bound.
    // RU: Очень долгая инерция должна остановиться на границе.
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
    // UZ: y=10 da endi 120+10=130 -> 2-element (100..150).
    // EN: At y=10 it is now 120+10=130 -> the 2nd item (100..150).
    // RU: При y=10 теперь 120+10=130 -> 2-й элемент (100..150).
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
    // UZ: y=150 scroll oynasidan pastda (oyna 0..100): kontent elementi bo'lsa ham bosilmasligi kerak.
    // EN: y=150 is below the scroll window (window 0..100): even if a content item exists there, it must not be tapped.
    // RU: y=150 ниже окна прокрутки (окно 0..100): даже если там есть элемент содержимого, он не должен нажиматься.
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
    // UZ: Boshiga bosib, kursorni qo'yamiz va belgi qo'shamiz.
    // EN: Tap at the start, place the cursor and insert a character.
    // RU: Нажимаем в начало, ставим курсор и добавляем символ.
    ui.frame(SZ);
    let r = ui.fields[0].rect;
    tap(&mut ui, r.x + 14.0, r.y + r.h / 2.0);
    ui.frame(SZ);
    ui.key_char('X');
    assert_eq!(val.get(), "Xsalo");
    // UZ: Orqaga: avval fokus olinadi.
    // EN: Back: focus is cleared first.
    // RU: Назад: сначала снимается фокус.
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
    // UZ: so'nggi qator ham teng ustunda
    // EN: the last row also uses equal columns.
    // RU: последняя строка тоже использует равные колонки.
    assert!((ui.hits[2].rect.w - 100.0).abs() < 0.5);
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
    // UZ: ko'rinish balandligi ma'lum bo'ldi
    // EN: the viewport height is now known.
    // RU: высота видимой области теперь известна.
    ui.frame(SZ);
    built.set(0);
    ui.frame(SZ);
    assert!(built.get() < 20, "{}", built.get());
    assert_eq!(sc.max_offset(), 10_000.0 * 50.0 - 300.0);
    sc.set_offset(250_000.0);
    built.set(0);
    ui.frame(SZ);
    assert!(built.get() < 20);
    // UZ: Chiziq o'rtasida ham elementlar bor (bo'shliq yo'q).
    // EN: There are items in the middle of the line too (no gaps).
    // RU: Элементы есть и в середине линии (пустот нет).
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

#[test]
fn spinner_uzluksiz_chizadi() {
    let mut ui = Ui::new(font(), || column(vec![spinner(20.0, 2.0, Color::WHITE)]));
    let cmds = ui.frame(SZ);
    assert!(cmds.iter().any(|c| matches!(c, DrawCmd::Arc { .. })));
    // UZ: animatsiya: har kadr qayta chiziladi
    // EN: animation: redrawn every frame.
    // RU: анимация: перерисовывается каждый кадр.
    assert!(ui.needs_frame());
    assert_eq!(ui.next_wakeup_ms(), Some(0));
}

#[test]
fn backdrop_buyrugi() {
    let mut ui = Ui::new(font(), || {
        column(vec![column(vec![text("x")])
            .backdrop(Color::rgba(255, 255, 255, 30), 16.0)
            .radius(12.0)])
    });
    let cmds = ui.frame(SZ);
    assert!(cmds
        .iter()
        .any(|c| matches!(c, DrawCmd::Backdrop { blur, .. } if *blur == 16.0)));
}

#[test]
fn linza_buyrugi() {
    let mut ui = Ui::new(font(), || {
        column(vec![column(vec![text("x")])
            .lens(Color::rgba(255, 255, 255, 12), 1.2)
            .radius(12.0)])
    });
    let cmds = ui.frame(SZ);
    assert!(cmds.iter().any(
        |c| matches!(c, DrawCmd::Backdrop { zoom, blur, .. } if *zoom == 1.2 && *blur == 0.0)
    ));
}

#[test]
fn sprite_kadrlari_vaqt_bilan_almashadi() {
    // UZ: 2x2 varaq (4 kadr, har biri 4x4 piksel), 10 kadr/s. EN: A 2x2 sheet (4 frames of 4x4 px), 10 fps. RU: Лист 2x2 (4 кадра по 4x4 пикселя), 10 кадров/с.
    let sheet = Rc::new(Image {
        w: 8,
        h: 8,
        rgba: vec![255; 8 * 8 * 4],
    });
    let s2 = sheet.clone();
    let mut ui = Ui::new(font(), move || column(vec![sprite(&s2, 2, 4, 10.0, true, 40.0, 40.0)]));
    let src_at = |ui: &mut Ui, ms: u64| {
        ui.set_time(ms);
        ui.frame(SZ).iter().find_map(|c| match c {
            DrawCmd::ImageRegion { src, .. } => Some(*src),
            _ => None,
        })
    };
    assert_eq!(src_at(&mut ui, 0), Some((0, 0, 4, 4)));
    assert_eq!(src_at(&mut ui, 100), Some((4, 0, 4, 4)));
    assert_eq!(src_at(&mut ui, 250), Some((0, 4, 4, 4)));
    assert_eq!(src_at(&mut ui, 350), Some((4, 4, 4, 4)));
    assert_eq!(src_at(&mut ui, 400), Some((0, 0, 4, 4)));
    assert!(ui.needs_frame());
}

#[test]
fn sprite_toxtagan_birinchi_kadr_va_qayta_chizmaydi() {
    let sheet = Rc::new(Image {
        w: 8,
        h: 4,
        rgba: vec![255; 8 * 4 * 4],
    });
    let mut ui = Ui::new(font(), move || column(vec![sprite(&sheet, 2, 2, 10.0, false, 40.0, 40.0)]));
    ui.set_time(500);
    let cmds = ui.frame(SZ);
    assert!(cmds.iter().any(|c| matches!(c, DrawCmd::ImageRegion { src: (0, 0, 4, 4), .. })));
    assert!(!ui.needs_frame());
}

#[test]
fn uzoq_bosish_boshlanadi_va_bosishni_bekor_qiladi() {
    let log: Rc<std::cell::RefCell<Vec<String>>> = Rc::new(std::cell::RefCell::new(Vec::new()));
    let (l1, l2) = (log.clone(), log.clone());
    let mut ui = Ui::new(font(), move || {
        let (l1, l2) = (l1.clone(), l2.clone());
        column(vec![column(vec![text("x")])
            .width(100.0)
            .height(100.0)
            .on_click(move || l1.borrow_mut().push("click".into()))
            .on_hold(300, move |h| l2.borrow_mut().push(format!("{h:?}")))])
    });
    ui.set_time(0);
    ui.frame(SZ);
    ui.pointer(PointerEvent::new(PointerKind::Down, 20.0, 20.0));
    // UZ: Muddat tugamagan: hali boshlanmagan; uyg'onish muddati ko'rinadi. EN: Not elapsed yet: not started; the wake-up time is known. RU: Время не истекло: не началось; срок пробуждения известен.
    ui.set_time(200);
    assert!(log.borrow().is_empty());
    assert_eq!(ui.next_wakeup_ms(), Some(100));
    ui.set_time(320);
    assert_eq!(*log.borrow(), vec!["Start".to_string()]);
    ui.pointer(PointerEvent::new(PointerKind::Move, 20.0, 5.0));
    ui.pointer(PointerEvent::new(PointerKind::Up, 20.0, 5.0));
    let l = log.borrow().clone();
    assert_eq!(l.len(), 3, "{l:?}");
    assert!(l[1].starts_with("Move(") && l[2].starts_with("End("), "{l:?}");
    assert!(!l.iter().any(|x| x == "click"), "{l:?}");
}

#[test]
fn qisqa_bosish_va_erta_siljish_uzoq_bosishni_bekor_qiladi() {
    let log: Rc<std::cell::RefCell<Vec<String>>> = Rc::new(std::cell::RefCell::new(Vec::new()));
    let (l1, l2) = (log.clone(), log.clone());
    let mut ui = Ui::new(font(), move || {
        let (l1, l2) = (l1.clone(), l2.clone());
        column(vec![column(vec![text("x")])
            .width(100.0)
            .height(100.0)
            .on_click(move || l1.borrow_mut().push("click".into()))
            .on_hold(300, move |h| l2.borrow_mut().push(format!("{h:?}")))])
    });
    ui.set_time(0);
    ui.frame(SZ);
    // UZ: Qisqa bosish: oddiy click. EN: A short press: a normal click. RU: Короткое нажатие: обычный клик.
    tap(&mut ui, 20.0, 20.0);
    assert_eq!(*log.borrow(), vec!["click".to_string()]);
    log.borrow_mut().clear();
    // UZ: Muddatdan oldin siljish: uzoq bosish bekor. EN: Moving before the time: the hold is cancelled. RU: Сдвиг до срока: удержание отменено.
    ui.pointer(PointerEvent::new(PointerKind::Down, 20.0, 20.0));
    ui.pointer(PointerEvent::new(PointerKind::Move, 80.0, 90.0));
    ui.set_time(1000);
    ui.pointer(PointerEvent::new(PointerKind::Up, 80.0, 90.0));
    assert!(!log.borrow().iter().any(|x| x == "Start"), "{:?}", log.borrow());
}

#[test]
fn har_xil_balandlikli_virtual_royxat_faqat_korinadiganni_quradi() {
    let built: Rc<std::cell::RefCell<Vec<usize>>> = Rc::new(std::cell::RefCell::new(Vec::new()));
    let sc = ScrollState::new();
    let (b, s2) = (built.clone(), sc.clone());
    let heights: Vec<f32> = (0..200).map(|i| if i % 5 == 0 { 30.0 } else { 50.0 }).collect();
    let h2 = heights.clone();
    let mut ui = Ui::new(font(), move || {
        let b = b.clone();
        column(vec![lazy_column_var(&s2, &h2, move |i| {
            b.borrow_mut().push(i);
            column(vec![text(format!("{i}"))])
        })
        .height(200.0)])
    });
    // UZ: Birinchi kadrda ko'rinish balandligi hali noma'lum (katta zaxira); ikkinchisida aniq. EN: On the first frame the viewport is unknown (a big margin); on the second it is known. RU: На первом кадре высота окна неизвестна (большой запас); на втором известна.
    ui.frame(SZ);
    built.borrow_mut().clear();
    ui.frame(SZ);
    let n0 = built.borrow().len();
    assert!(n0 > 0 && n0 < 20, "{n0}");
    assert_eq!(built.borrow()[0], 0);
    // UZ: 4000 dp pastga: boshidagi elementlar qurilmaydi. EN: Scrolled 4000 dp down: the first items are not built. RU: Прокрутка на 4000 dp вниз: первые элементы не строятся.
    built.borrow_mut().clear();
    sc.set_offset(4000.0 * 1.0);
    ui.frame(SZ);
    let b = built.borrow().clone();
    assert!(!b.is_empty() && b[0] > 50 && b.len() < 20, "{b:?}");
}

#[test]
fn ota_qutisidan_chiqqan_element_joylashuv_xatosi_sifatida_yoziladi() {
    uzcoders_core::clear_errors();
    let mut ui = Ui::new(font(), || {
        column(vec![
            column(vec![column(vec![]).width(120.0).height(10.0).bg(Color::WHITE)]).width(40.0).height(40.0),
            column(vec![text("Qisqa")]).width(100.0).height(40.0),
        ])
    });
    ui.frame(SZ);
    ui.frame(SZ);
    let t = uzcoders_core::errors_text();
    assert!(t.contains("joylashuv: element 120x10"), "{t}");
    assert!(!t.contains("Qisqa"));
    assert_eq!(t.lines().count(), 1, "{t}");
    uzcoders_core::clear_errors();
    let mut ui = Ui::new(font(), || {
        column(vec![column(vec![column(vec![]).width(120.0).height(10.0).gradient(Color::WHITE, Color::BLACK)]).width(40.0).height(40.0)])
    });
    ui.frame(SZ);
    assert!(uzcoders_core::errors_text().is_empty(), "bezak xato bermasin");
    uzcoders_core::clear_errors();
}

#[test]
fn sprite_frame_aniq_kadrni_chizadi() {
    let sheet = Rc::new(Image { w: 4, h: 2, rgba: vec![0; 32] });
    let s2 = sheet.clone();
    let mut ui = Ui::new(font(), move || sprite_frame(&s2, 2, 4, 3, 20.0, 20.0));
    let cmds = ui.frame(SZ);
    let src = cmds.iter().find_map(|c| if let DrawCmd::ImageRegion { src, .. } = c { Some(*src) } else { None });
    assert_eq!(src, Some((2, 1, 2, 1)));
    assert!(!ui.needs_frame());
}

#[test]
fn maydonda_emoji_rasm_bilan_chiziladi_va_butun_ochadi() {
    let st = State::new("a😀".to_string());
    let img = Rc::new(Image { w: 4, h: 4, rgba: vec![255; 64] });
    let (s2, i2) = (st.clone(), img.clone());
    let mut ui = Ui::new(font(), move || {
        let i3 = i2.clone();
        field(&s2).inline_images(move |c| if c == "😀" { Some((i3.clone(), None)) } else { None })
    });
    let cmds = ui.frame(SZ);
    let imgs = cmds.iter().filter(|c| matches!(c, DrawCmd::Image { .. })).count();
    assert_eq!(imgs, 1);
    assert!(cmds.iter().any(|c| matches!(c, DrawCmd::Text { text, .. } if text == "a")));
    let r = ui.fields[0].rect;
    tap(&mut ui, r.x + r.w - 2.0, r.y + r.h / 2.0);
    assert!(ui.key_backspace());
    assert_eq!(st.get(), "a");
}
