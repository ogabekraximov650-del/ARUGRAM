//! Testlar: tarmoq o'rniga sinov javoblari (faqat testda), ekranlar chiziladi va asosiy amallar tekshiriladi.

use super::*;
use crate::ident::Me;
use crate::net::MOCK;
use std::sync::{Mutex, MutexGuard};

/// Testlar tarmoq sinov javobi va vazifa navbatini bo'lishadi: ketma-ket bajariladi.
static LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn set_mock(m: fn(&str, &str, &str) -> (u16, String)) {
    *MOCK.lock().unwrap_or_else(|e| e.into_inner()) = Some(m);
}

const VIDEOS: &str = r#"{"videos":[
 {"id":"a1","title":"Qirq kun ichida Markaziy Osiyoni bosib o'tdik","kind":"video","uid":"u1u1u1u1","channel":"Sayohat Kundaligi","size":2000000,"duration":1441,"views":12400,"created":1,"has_thumb":false,"likes":5,"notes":2,"liked":false},
 {"id":"a2","title":"Tog'da tong","kind":"reel","uid":"u2u2u2u2","channel":"Oshxona","size":900000,"duration":45,"views":300,"created":2,"has_thumb":false,"likes":0,"notes":0,"liked":false},
 {"id":"a3","title":"Uyda tez ovqat","kind":"video","uid":"me123456","channel":"Sinov kanal","size":900000,"duration":900,"views":2000000,"created":3,"has_thumb":false,"likes":9,"notes":1,"liked":true}
]}"#;

fn mock(method: &str, path: &str, _body: &str) -> (u16, String) {
    let _ = method;
    if path.starts_with("/api/videos") {
        (200, VIDEOS.to_string())
    } else if path.starts_with("/api/user") {
        (200, r#"{"id":"me123456","name":"Sinov kanal"}"#.into())
    } else if path.starts_with("/api/channel") {
        (200, r#"{"id":"me123456","name":"Sinov kanal","videos":1,"views":2000000,"followers":12,"following":false}"#.into())
    } else if path.contains("/notes") {
        (200, r#"{"notes":[{"id":1,"at":72,"body":"Bu joy qayerda?","uid":"x","name":"malika","created":1}]}"#.into())
    } else if path.starts_with("/api/video/") {
        (200, r#"{"id":"a1","title":"Qirq kun","kind":"video","uid":"u1u1u1u1","channel":"Sayohat Kundaligi","size":2000000,"duration":1441,"views":12400,"created":1,"has_thumb":false,"likes":5,"notes":1,"liked":false,"followers":1240,"following":false,"done":true}"#.into())
    } else {
        (404, r#"{"error":"topilmadi"}"#.into())
    }
}

fn empty_mock(_m: &str, p: &str, _b: &str) -> (u16, String) {
    if p.starts_with("/api/videos") { (200, r#"{"videos":[]}"#.into()) } else { (200, "{}".into()) }
}

fn offline(_m: &str, _p: &str, _b: &str) -> (u16, String) {
    (0, String::new())
}

fn pump() {
    for _ in 0..400 {
        uzcoders::core::drain_tasks();
        if uzcoders::core::pending_tasks() == 0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

fn setup(m: fn(&str, &str, &str) -> (u16, String)) -> (Ui, Rc<App>) {
    set_mock(m);
    let (ui, app) = build(2.0);
    app.me.set(Some(Me { id: "me123456".into(), name: "Sinov kanal".into() }));
    (ui, app)
}

fn texts(ui: &mut Ui) -> Vec<String> {
    ui.frame(Size::new(780.0, 1688.0))
        .iter()
        .filter_map(|c| match c {
            DrawCmd::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn has(ui: &mut Ui, what: &str) -> bool {
    texts(ui).iter().any(|t| t.contains(what))
}

/// `ARU_SNAPSHOT_DIR` berilsa kadrni PNG qilib saqlaydi (ko'z bilan tekshirish uchun).
fn snap(ui: &mut Ui, name: &str) {
    let cmds = ui.frame(Size::new(780.0, 1688.0));
    if let Ok(dir) = std::env::var("ARU_SNAPSHOT_DIR") {
        let mut c = ui.canvas(780, 1688);
        c.clear(Color::BLACK);
        c.execute(&cmds);
        std::fs::write(format!("{dir}/{name}.png"), c.to_png()).unwrap();
    }
}

#[test]
fn kirish_va_kanal_yaratish() {
    let _g = lock();
    set_mock(mock);
    let (mut ui, app) = build(2.0);
    app.me.set(None);
    assert!(has(&mut ui, "XUSH KELIBSIZ"));
    snap(&mut ui, "0_kirish");
    app.name_text.set("A".into());
    app.register();
    assert!(app.me.with(|m| m.is_none()), "qisqa nom qabul qilinmasligi kerak");
    app.name_text.set("Sinov kanal".into());
    app.register();
    pump();
    assert!(app.me.with(|m| m.is_some()));
}

#[test]
fn bosh_sahifa_haqiqiy_royxat() {
    let _g = lock();
    let (mut ui, app) = setup(mock);
    let _ = texts(&mut ui);
    pump();
    assert!(has(&mut ui, "Qirq kun ichida"));
    assert!(has(&mut ui, "Uyda tez ovqat"));
    assert!(has(&mut ui, "24:01"));
    assert!(has(&mut ui, "12,4 ming"));
    snap(&mut ui, "1_bosh");
    app.hot.set(true);
    let _ = texts(&mut ui);
    pump();
    assert!(has(&mut ui, "Uyda tez ovqat"));
}

#[test]
fn bosh_sahifa_bosh_va_xatoli_holat() {
    let _g = lock();
    let (mut ui, _app) = setup(empty_mock);
    let _ = texts(&mut ui);
    pump();
    assert!(has(&mut ui, "Hozircha video yo'q"));
    snap(&mut ui, "1b_bosh_royxat_yoq");
    let (mut ui, _app) = setup(offline);
    let _ = texts(&mut ui);
    pump();
    assert!(has(&mut ui, "Internet yo'q"));
    assert!(has(&mut ui, "Qayta urinish"));
}

#[test]
fn video_ochiladi_like_va_fikr() {
    let _g = lock();
    let (mut ui, app) = setup(mock);
    let _ = texts(&mut ui);
    pump();
    let v = app.f_new.items.with(|l| l[0].clone());
    app.open_video(v.clone());
    pump();
    assert!(has(&mut ui, "VAQT BO'YICHA"));
    assert!(has(&mut ui, "Bu joy qayerda?"));
    assert!(has(&mut ui, "1,2 ming kuzatuvchi"), "{:?}", texts(&mut ui));
    snap(&mut ui, "2_korish");
    // like: darrov o'zgaradi
    let cur = app.watch.get().unwrap();
    app.toggle_like(&cur);
    let after = app.watch.get().unwrap();
    assert!(after.liked && after.likes == cur.likes + 1);
    assert!(app.f_new.items.with(|l| l[0].liked));
    // o'z videosi emas -> "O'chirish" yo'q, o'ziniki -> bor
    assert!(!has(&mut ui, "O'chirish"));
    let own = app.f_new.items.with(|l| l[2].clone());
    app.open_video(own);
    pump();
    assert!(has(&mut ui, "O'chirish"));
    app.close_video();
    assert!(app.watch.get().is_none());
}

#[test]
fn boshqa_bolimlar_chiziladi() {
    let _g = lock();
    let (mut ui, app) = setup(mock);
    let _ = texts(&mut ui);
    pump();
    app.tab.set(Tab::Reels);
    let _ = texts(&mut ui);
    pump();
    assert!(has(&mut ui, "TOG'DA TONG"), "{:?}", texts(&mut ui));
    snap(&mut ui, "3_oqim");
    app.tab.set(Tab::Subs);
    let _ = texts(&mut ui);
    pump();
    assert!(has(&mut ui, "OBUNALAR"));
    snap(&mut ui, "4_obunalar");
    app.tab.set(Tab::Me);
    let _ = texts(&mut ui);
    pump();
    assert!(has(&mut ui, "SINOV KANAL") || has(&mut ui, "SINOV"));
    assert!(has(&mut ui, "VIDEOLARIM"));
    snap(&mut ui, "5_profil");
    app.tab.set(Tab::Home);
    app.create.set(true);
    assert!(has(&mut ui, "YARATISH"));
    snap(&mut ui, "6_yaratish");
    app.create.set(false);
    app.pending.set(Some("/tmp/x.mp4".into()));
    app.title_text.set("Mening videom".into());
    assert!(has(&mut ui, "VIDEO SARLAVHASI"));
    snap(&mut ui, "7_sarlavha");
    app.pending.set(None);
    app.search_on.set(true);
    assert!(has(&mut ui, "Qidiruv"));
    snap(&mut ui, "8_qidiruv");
}

#[test]
fn orqaga_tugmasi_tartibi() {
    let _g = lock();
    let (_ui, app) = setup(mock);
    app.tab.set(Tab::Me);
    app.search_on.set(true);
    app.create.set(true);
    app.pending.set(Some("x".into()));
    assert!(app.back() && app.pending.get().is_none());
    assert!(app.back() && !app.create.get());
    assert!(app.back() && !app.search_on.get());
    app.watch.set(Some(model::Video { id: "a".into(), ..Default::default() }));
    assert!(app.back() && app.watch.get().is_none());
    assert!(app.back() && app.tab.get() == Tab::Home);
    assert!(!app.back());
}

#[test]
fn video_bosilsa_ochiladi() {
    let _g = lock();
    let (mut ui, app) = setup(mock);
    let _ = texts(&mut ui);
    pump();
    let cmds = ui.frame(Size::new(780.0, 1688.0));
    let (x, y) = cmds
        .iter()
        .find_map(|c| match c {
            DrawCmd::Text { x, baseline, text, size, .. } if text.starts_with("Qirq kun") => Some((*x + 4.0, *baseline - *size * 0.3)),
            _ => None,
        })
        .expect("sarlavha topilmadi");
    ui.pointer(PointerEvent::new(PointerKind::Down, x, y));
    ui.pointer(PointerEvent::new(PointerKind::Up, x, y));
    assert_eq!(app.watch.get().map(|v| v.id), Some("a1".to_string()));
}
