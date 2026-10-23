//! Pleyer sahifasi (Kotlin `SeasonScreen` pleyer qismi): video, boshqaruv paneli (shisha), to'liq ekran, sifat, intro, keyingi qism,
//! tomosha tarixi va davom ettirish. Dekoder — `crate::player` (NDK), chizish — GL tashqi tekstura (`.video()`).

use super::*;
use crate::data::{Episode, QUALITIES};
use crate::downloads;
use super::pages::{self, SeasonPage};
use crate::player::{Player, Source, State as PState, Status};
use std::cell::{Cell, RefCell};
use std::time::Instant;

const V_PLAY: &str = "M8,5v14l11,-7z";
const V_PAUSE: &str = "M6,19h4V5H6v14zM14,5v14h4V5h-4z";
const V_PREV: &str = "M6,6h2v12H6zM9.5,12l8.5,6V6z";
const V_NEXT: &str = "M6,18l8.5,-6L6,6v12zM16,6v12h2V6h-2z";
const V_FULL: &str = "M7,14H5v5h5v-2H7v-3zM5,10h2V7h3V5H5v5zM17,17h-3v2h5v-5h-2v3zM14,5v2h3v3h2V5h-5z";
const V_FULL_EXIT: &str = "M5,16h3v3h2v-5H5v2zM8,8H5v2h5V5H8v3zM14,19h2v-3h3v-2h-5v5zM16,8V5h-2v5h5V8h-3z";
const V_GEAR: &str = "M19.14,12.94c0.04,-0.3 0.06,-0.61 0.06,-0.94 0,-0.32 -0.02,-0.64 -0.07,-0.94l2.03,-1.58c0.18,-0.14 0.23,-0.41 0.12,-0.61l-1.92,-3.32c-0.12,-0.22 -0.37,-0.29 -0.59,-0.22l-2.39,0.96c-0.5,-0.38 -1.03,-0.7 -1.62,-0.94L14.4,2.81c-0.04,-0.24 -0.24,-0.41 -0.48,-0.41h-3.84c-0.24,0 -0.43,0.17 -0.47,0.41L9.25,5.35C8.66,5.59 8.12,5.92 7.63,6.29L5.24,5.33c-0.22,-0.08 -0.47,0 -0.59,0.22L2.74,8.87C2.62,9.08 2.66,9.34 2.86,9.48l2.03,1.58C4.84,11.36 4.8,11.69 4.8,12s0.02,0.64 0.07,0.94l-2.03,1.58c-0.18,0.14 -0.23,0.41 -0.12,0.61l1.92,3.32c0.12,0.22 0.37,0.29 0.59,0.22l2.39,-0.96c0.5,0.38 1.03,0.7 1.62,0.94l0.36,2.54c0.05,0.24 0.24,0.41 0.48,0.41h3.84c0.24,0 0.44,-0.17 0.47,-0.41l0.36,-2.54c0.59,-0.24 1.13,-0.56 1.62,-0.94l2.39,0.96c0.22,0.08 0.47,0 0.59,-0.22l1.92,-3.32c0.12,-0.22 0.07,-0.47 -0.12,-0.61l-2.01,-1.58zM12,15.6c-1.98,0 -3.6,-1.62 -3.6,-3.6s1.62,-3.6 3.6,-3.6 3.6,1.62 3.6,3.6 -1.62,3.6 -3.6,3.6z";
const V_LIST: &str = "M3,10h11v2H3V10zM3,6h11v2H3V6zM3,14h7v2H3V14zM16,6v8.18C15.69,14.07 15.35,14 15,14c-1.66,0 -3,1.34 -3,3s1.34,3 3,3 3,-1.34 3,-3V8h3V6H16z";
const V_UNLOCK: &str = "M12,17c1.1,0 2,-0.9 2,-2s-0.9,-2 -2,-2 -2,0.9 -2,2 0.9,2 2,2zM18,8h-1V6c0,-2.76 -2.24,-5 -5,-5S7,3.24 7,6h1.9c0,-1.71 1.39,-3.1 3.1,-3.1 1.71,0 3.1,1.39 3.1,3.1v2H6c-1.1,0 -2,0.9 -2,2v10c0,1.1 0.9,2 2,2h12c1.1,0 2,-0.9 2,-2V10c0,-1.1 -0.9,-2 -2,-2zM18,20H6V10h12v10z";
const V_BACK: &str = "M20,11H7.83l5.59,-5.59L12,4l-8,8 8,8 1.41,-1.41L7.83,13H20v-2z";
const V_LOCK: &str = "M18,8h-1L17,6c0,-2.76 -2.24,-5 -5,-5S7,3.24 7,6v2H6c-1.1,0 -2,0.9 -2,2v10c0,1.1 0.9,2 2,2h12c1.1,0 2,-0.9 2,-2V10c0,-1.1 -0.9,-2 -2,-2zM12,17c-1.1,0 -2,-0.9 -2,-2s0.9,-2 2,-2 2,0.9 2,2 -0.9,2 -2,2zM15.1,8H8.9V6c0,-1.71 1.39,-3.1 3.1,-3.1 1.71,0 3.1,1.39 3.1,3.1v2z";

/// Pleyer holati (har bir bo'lim sahifasida bitta).
pub struct PlayerState {
    pub(super) player: RefCell<Option<Player>>,
    pub(super) status: State<Status>,
    /// (qism id, sifat, video manzili)
    pub(super) cur: State<Option<(i64, String, String)>>,
    pub(super) fullscreen: State<bool>,
    pub(super) ctl: State<bool>,
    ctl_at: Cell<Instant>,
    pub(super) settings: State<bool>,
    pub(super) auto_next: State<bool>,
    pub(super) auto_skip: State<bool>,
    pub(super) seeking: State<Option<f32>>,
    pub(super) perr: State<String>,
    pub(super) loading: State<bool>,
    pub(super) locked: State<bool>,
    /// To'liq ekran: qismlar ro'yxati oynasi.
    pub(super) ep_list: State<bool>,
    ep_scroll: ScrollState,
    set_scroll: ScrollState,
    /// Ikki marta bosib sek: oxirgi bosish (chap=true, vaqt), jamlangan soniya (chap<0 / o'ng>0), kutilayotgan sek avlodi.
    last_tap: Cell<Option<(bool, Instant)>>,
    pub(super) seek_acc: State<i32>,
    seek_gen: Cell<u32>,
    sleep_at: Cell<Option<Instant>>,
    held: Cell<bool>,
    watched_ms: Cell<i64>,
    new_view: Cell<bool>,
    last_save: Cell<Option<Instant>>,
    ticking: Cell<bool>,
    skipped: RefCell<Vec<i64>>,
    pub(super) closed: Cell<bool>,
    pub(super) sub_until: State<i64>,
    /// Ijro tezligi (foiz).
    pub(super) speed: State<u32>,
}

impl PlayerState {
    pub(super) fn new() -> PlayerState {
        PlayerState {
            player: RefCell::new(None),
            status: State::new(Status { state: PState::Opening, pos_ms: 0, dur_ms: 0, playing: false, buffering: false, error: String::new(), width: 0, height: 0, speed: 1.0, buffered_ms: 0, dbg: String::new() }),
            cur: State::new(None),
            fullscreen: State::new(false),
            ctl: State::new(true),
            ctl_at: Cell::new(Instant::now()),
            settings: State::new(false),
            auto_next: State::new(true),
            auto_skip: State::new(false),
            seeking: State::new(None),
            perr: State::new(String::new()),
            loading: State::new(false),
            locked: State::new(false),
            ep_list: State::new(false),
            ep_scroll: ScrollState::new(),
            set_scroll: ScrollState::new(),
            last_tap: Cell::new(None),
            seek_acc: State::new(0),
            seek_gen: Cell::new(0),
            sleep_at: Cell::new(None),
            held: Cell::new(false),
            watched_ms: Cell::new(0),
            new_view: Cell::new(true),
            last_save: Cell::new(None),
            ticking: Cell::new(false),
            skipped: RefCell::new(Vec::new()),
            closed: Cell::new(false),
            sub_until: State::new(-1),
            speed: State::new(100),
        }
    }
}

fn mmss(ms: i64) -> String {
    let t = (ms / 1000).max(0);
    let (h, m, s) = (t / 3600, (t % 3600) / 60, t % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

impl Home {
    /// Ko'rish ruxsati: obuna faol yoki bo'lim bepul.
    pub(super) fn can_watch(&self, p: &Rc<SeasonPage>) -> bool {
        let until = p.pl.sub_until.get();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
        until > now || p.season.free == Some(true)
    }

    fn pl_save_progress(&self, p: &Rc<SeasonPage>) {
        if let (Some((_, _, url)), Some(pl)) = (p.pl.cur.get(), p.pl.player.borrow().as_ref()) {
            let st = pl.status();
            if st.dur_ms > 0 {
                downloads::progress_set(&url, st.pos_ms, st.dur_ms);
            }
        }
    }

    fn pl_flush_history(&self, p: &Rc<SeasonPage>) {
        let Some((ep, q, url)) = p.pl.cur.get() else { return };
        let st = match p.pl.player.borrow().as_ref() {
            Some(pl) => pl.status(),
            None => return,
        };
        if st.dur_ms <= 0 || st.pos_ms < 1000 {
            return;
        }
        crate::sync::put_history(p.season.anime_id, p.season.season_id, ep, &url, &q, st.pos_ms, st.dur_ms, p.pl.watched_ms.get().min(st.dur_ms), p.pl.new_view.get());
        p.pl.new_view.set(false);
    }

    /// Sahifa yopilganda: tarix yoziladi, pleyer to'xtatiladi.
    pub(super) fn pl_close(&self, p: &Rc<SeasonPage>) {
        if p.pl.closed.replace(true) {
            return;
        }
        self.pl_save_progress(p);
        self.pl_flush_history(p);
        if let Some(pl) = p.pl.player.borrow_mut().take() {
            pl.close();
            crate::gate::BUSY.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        }
        if p.pl.held.replace(false) {
            downloads::release_hold();
        }
        if p.pl.fullscreen.get() {
            p.pl.fullscreen.set(false);
            uzcoders::android::set_fullscreen(false);
        }
        uzcoders::android::keep_screen_on(false);
        std::thread::spawn(crate::sync::flush);
    }

    pub(super) fn pl_open_gate(self: &Rc<Self>, p: &Rc<SeasonPage>) {
        let (p2, session) = (p.clone(), self.session.clone());
        spawn_bg(move || crate::api::get_json("/api/billing", "", Some(&session)), move |r| {
            let until = r.ok().map(|j| j["subscription_until"].as_i64().unwrap_or(0)).unwrap_or(0);
            p2.pl.sub_until.set(until);
        });
        // Bepul bo'lim: majburiy kanallar ro'yxati (ruxsat ekrani uchun).
        if p.season.free == Some(true) && crate::gate::channels().is_none() {
            let (me, session) = (self.clone(), self.session.clone());
            spawn_bg(move || crate::gate::fetch(&session), move |_| me.gate_rev.set(me.gate_rev.get() + 1));
        }
    }

    /// Bepul bo'lim, obuna yo'q va ruxsat berilmagan bo'lsa — ruxsat ekrani (Kotlin `ConsentScreen`).
    pub(super) fn consent_screen(self: &Rc<Self>, p: &Rc<SeasonPage>) -> Option<Element> {
        let until = p.pl.sub_until.get();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
        let _ = self.gate_rev.get();
        if p.season.free != Some(true) || until < 0 || until > now || self.consent.get() {
            return None;
        }
        let chans = crate::gate::channels();
        if matches!(&chans, Some(c) if c.is_empty()) {
            return None;
        }
        let mut kids: Vec<Element> = vec![
            gap_box(0.0, 24.0),
            tx("Bepul ko'rish", 22.0, Color::WHITE).black(),
            tx("Bu bo'limni bepul ko'rish uchun quyidagi kanallarga obuna bo'lishingiz kerak. Ruxsat bersangiz, ilova Telegram hisobingiz bilan ularga o'zi qo'shiladi (yopiq kanalga so'rov yuboradi). Kanaldan istalgan vaqtda Telegram'da chiqib ketishingiz mumkin.", 13.5, Color::WHITE).line_height(21.0).padding_each(0.0, 10.0, 0.0, 16.0),
        ];
        match chans {
            None => kids.push(column(vec![spinner(28.0, 3.0, ACCENT2)]).center().padding(12.0)),
            Some(c) => {
                for ch in c {
                    kids.push(tx(format!("{} ({})", if ch.title.is_empty() { ch.url.clone() } else { ch.title.clone() }, if ch.kind == "public" { "ochiq" } else { "yopiq" }), 14.0, Color::WHITE).padding_each(0.0, 0.0, 0.0, 8.0));
                }
            }
        }
        kids.push(gap_box(0.0, 16.0));
        let me = self.clone();
        kids.push(
            column(vec![tx("Ruxsat berish va ko'rish", 15.0, Color::WHITE).black()]).center().padding_xy(0.0, 15.0).radius(28.0).gradient(ACCENT, ACCENT2).on_click(move || {
                crate::sync::put_settings(&me.hidden.get(), true);
                crate::gate::set_consent(true);
                me.consent.set(true);
            }),
        );
        let me2 = self.clone();
        kids.push(tx("Kanalsiz — obuna olish", 13.0, Color::WHITE).padding(14.0).on_click(move || me2.open_billing()));
        Some(self.scaffold("Bepul ko'rish", scroll_column(&p.scroll[0], kids).padding_each(24.0, 0.0, 24.0, 0.0)))
    }

    /// Saqlangan (yoki birinchi) qismni ijro etadi.
    pub(super) fn pl_start_default(self: &Rc<Self>, p: &Rc<SeasonPage>) {
        let eps = p.eps.get();
        let Some(first) = eps.first() else {
            return;
        };
        let last = p.pl.cur.get().map(|c| c.0);
        let ep = last.and_then(|id| eps.iter().find(|x| x.id == id)).unwrap_or(first);
        self.play_episode(p, &ep.clone(), None, None, true);
    }

    /// Qismni ijro etadi. `quality` — tanlangan sifat; `resume` — boshlang'ich joy (bo'lmasa saqlangan joy).
    pub(super) fn play_episode(self: &Rc<Self>, p: &Rc<SeasonPage>, ep: &Episode, quality: Option<String>, resume: Option<i64>, auto_play: bool) {
        if !self.can_watch(p) {
            p.pl.perr.set("Bu bo'limni ko'rish uchun obuna kerak".into());
            return;
        }
        let q = quality.filter(|q| ep.qualities.contains(q)).or_else(|| p.pl.cur.get().map(|c| c.1).filter(|q| ep.qualities.contains(q))).or_else(|| QUALITIES.iter().map(|s| s.to_string()).find(|q| ep.qualities.contains(q)));
        let Some(q) = q else {
            p.pl.perr.set("Bu qismda video yo'q".into());
            return;
        };
        let url = ep.url(&q);
        // Oldingi qism tarixi va pleyer yopiladi.
        self.pl_save_progress(p);
        self.pl_flush_history(p);
        if let Some(old) = p.pl.player.borrow_mut().take() {
            old.close();
            crate::gate::BUSY.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        }
        if p.pl.held.replace(false) {
            downloads::release_hold();
        }
        p.pl.watched_ms.set(0);
        p.pl.new_view.set(true);
        p.pl.skipped.borrow_mut().clear();
        p.pl.perr.set(String::new());
        p.pl.loading.set(true);
        p.pl.cur.set(Some((ep.id, q.clone(), url.clone())));
        p.pl.ctl.set(true);
        let start = resume.or_else(|| downloads::progress_get(&url).and_then(|(pos, dur)| if pos > 3000 && pos < dur - 5000 { Some(pos) } else { None })).unwrap_or(0);
        let (p2, me, session, url2) = (p.clone(), self.clone(), self.session.clone(), url.clone());
        spawn_bg(move || downloads::resolve(&session, &url2), move |r| {
            if p2.pl.closed.get() || p2.pl.cur.get().map(|c| c.2) != Some(url) {
                if let Ok((_, true)) = r {
                    downloads::release_hold();
                }
                return;
            }
            p2.pl.loading.set(false);
            match r {
                Ok((src, held)) => {
                    p2.pl.held.set(held);
                    let np = Player::open(src, start, auto_play);
                    if p2.pl.speed.get() != 100 {
                        np.set_speed(p2.pl.speed.get() as f32 / 100.0);
                    }
                    *p2.pl.player.borrow_mut() = Some(np);
                    crate::gate::BUSY.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    uzcoders::android::keep_screen_on(true);
                    me.pl_tick(&p2);
                }
                Err(e) => p2.pl.perr.set(e),
            }
        });
    }

    /// Har ~500 ms: holat yangilanadi, tarix/jarayon saqlanadi, qism tugasa keyingisi ochiladi.
    fn pl_tick(self: &Rc<Self>, p: &Rc<SeasonPage>) {
        if p.pl.ticking.replace(true) {
            return;
        }
        fn step(me: Rc<Home>, p: Rc<SeasonPage>) {
            let (m2, p2) = (me.clone(), p.clone());
            spawn_bg(|| std::thread::sleep(std::time::Duration::from_millis(500)), move |_| {
                if p2.pl.closed.get() || !m2.pages.get().iter().any(|pg| matches!(pg, pages::Page::Season(q) if Rc::ptr_eq(q, &p2))) {
                    p2.pl.ticking.set(false);
                    return;
                }
                let st = match p2.pl.player.borrow().as_ref() {
                    Some(pl) => pl.status(),
                    None => {
                        p2.pl.ticking.set(false);
                        return;
                    }
                };
                if st.playing && st.state == PState::Ready && !st.buffering {
                    p2.pl.watched_ms.set(p2.pl.watched_ms.get() + 500);
                }
                // Boshqaruv paneli: ijro paytida 3.5 soniyadan keyin yashiriladi.
                if p2.pl.ctl.get() && st.playing && p2.pl.ctl_at.get().elapsed().as_millis() > 3500 && !p2.pl.settings.get() {
                    p2.pl.ctl.set(false);
                }
                // Intro: avto o'tkazish.
                if p2.pl.auto_skip.get() {
                    if let Some(ep) = p2.eps.get().iter().find(|e| Some(e.id) == p2.pl.cur.get().map(|c| c.0)) {
                        if let Some((a, b)) = ep.intro.iter().find(|(a, b)| st.pos_ms >= *a && st.pos_ms < *b) {
                            if !p2.pl.skipped.borrow().contains(a) {
                                p2.pl.skipped.borrow_mut().push(*a);
                                if let Some(pl) = p2.pl.player.borrow().as_ref() {
                                    pl.seek_ms(*b);
                                }
                            }
                        }
                    }
                }
                // Uyqu taymeri.
                if let Some(t) = p2.pl.sleep_at.get() {
                    if Instant::now() >= t {
                        p2.pl.sleep_at.set(None);
                        if let Some(pl) = p2.pl.player.borrow().as_ref() {
                            pl.pause();
                        }
                        uzcoders::android::toast("Uyqu taymeri: pauza qilindi");
                    }
                }
                let due = p2.pl.last_save.get().map(|t| t.elapsed().as_secs() >= 5).unwrap_or(true);
                if due {
                    p2.pl.last_save.set(Some(Instant::now()));
                    m2.pl_save_progress(&p2);
                }
                p2.pl.status.set(st.clone());
                // Qism tugadi: keyingisi.
                if st.state == PState::Ended {
                    m2.pl_save_progress(&p2);
                    m2.pl_flush_history(&p2);
                    if p2.pl.auto_next.get() {
                        let cur = p2.pl.cur.get().map(|c| c.0);
                        let eps = p2.eps.get();
                        if let Some(i) = eps.iter().position(|e| Some(e.id) == cur) {
                            if let Some(n) = eps.get(i + 1) {
                                m2.play_episode(&p2, &n.clone(), None, Some(0), true);
                                step(m2, p2);
                                return;
                            }
                        }
                    }
                }
                step(m2, p2);
            });
        }
        step(self.clone(), p.clone());
    }

    fn pl_touch(&self, p: &Rc<SeasonPage>) {
        p.pl.ctl_at.set(Instant::now());
        p.pl.ctl.set(true);
    }

    fn pl_toggle_play(&self, p: &Rc<SeasonPage>) {
        if let Some(pl) = p.pl.player.borrow().as_ref() {
            if pl.status().playing { pl.pause() } else { pl.play() }
        }
        self.pl_touch(p);
    }

    fn pl_step(self: &Rc<Self>, p: &Rc<SeasonPage>, d: i64) {
        let cur = p.pl.cur.get().map(|c| c.0);
        let eps = p.eps.get();
        if let Some(i) = eps.iter().position(|e| Some(e.id) == cur) {
            let j = i as i64 + d;
            if j >= 0 {
                if let Some(e) = eps.get(j as usize) {
                    self.play_episode(p, &e.clone(), None, Some(0), true);
                }
            }
        }
    }

    pub(super) fn pl_set_fullscreen(&self, p: &Rc<SeasonPage>, on: bool) {
        p.pl.fullscreen.set(on);
        uzcoders::android::set_fullscreen(on);
    }

    fn pl_round(icon_path: &'static str, size: f32, boxsz: f32, f: Box<dyn Fn()>) -> Element {
        column(vec![icon(icon_path, size).color(Color::WHITE)]).center().width(boxsz).height(boxsz).radius(boxsz / 2.0).lens(Color::rgba(255, 255, 255, 8), GLASS_ZOOM).radius(boxsz / 2.0).on_click(f)
    }

    /// Video maydoni (cover o'rnida): video, yuklanish/xato holati va shisha boshqaruv paneli.
    pub(super) fn player_view(self: &Rc<Self>, p: &Rc<SeasonPage>, cover: Element, w: f32, h: f32, fullscreen: bool) -> Element {
        let st = p.pl.status.get();
        let cur = p.pl.cur.get();
        let started = cur.is_some() && p.pl.player.borrow().is_some();
        let mut layers: Vec<Element> = Vec::new();
        if started {
            layers.push(column(vec![]).bg(Color::BLACK).radius(if fullscreen { 0.0 } else { 18.0 }));
            // Video tomonlar nisbatida (letterbox).
            let (vw, vh) = if st.width > 0 && st.height > 0 {
                let ar = st.width as f32 / st.height as f32;
                if w / h > ar { (h * ar, h) } else { (w, w / ar) }
            } else {
                (w, h)
            };
            layers.push(column(vec![column(vec![]).video().width(vw).height(vh)]).center());
        } else {
            layers.push(cover);
        }
        // Bir bosish: boshqaruvni ko'rsatish/yashirish; chap/o'ng yarmida ikki marta bosish: -/+5 soniya (jamlanadi).
        let (m1, p1, m2, p2) = (self.clone(), p.clone(), self.clone(), p.clone());
        layers.push(row(vec![
            column(vec![]).weight(1.0).on_click(move || m1.pl_tap(&p1, true)),
            column(vec![]).weight(1.0).on_click(move || m2.pl_tap(&p2, false)),
        ]));
        let err = p.pl.perr.get();
        let busy = p.pl.loading.get() || (started && (st.state == PState::Opening || (st.buffering && st.playing) || false));
        if !err.is_empty() || st.state == PState::Error {
            let msg = if !err.is_empty() { err } else { st.error.clone() };
            let (me, pc) = (self.clone(), p.clone());
            layers.push(
                column(vec![
                    tx(msg, 13.5, Color::WHITE).bold(),
                    gap_box(0.0, 12.0),
                    column(vec![tx("Qayta urinish", 13.0, Color::WHITE).bold()]).center().padding_xy(18.0, 10.0).radius(20.0).gradient(ACCENT, ACCENT2).on_click(move || {
                        if let Some((id, q, _)) = pc.pl.cur.get() {
                            if let Some(e) = pc.eps.get().iter().find(|e| e.id == id) {
                                me.play_episode(&pc, &e.clone(), Some(q), None, true);
                            }
                        } else {
                            pc.pl.perr.set(String::new());
                            if pc.eps.get().is_empty() {
                                me.load_eps(&pc);
                            }
                        }
                    }),
                ])
                .center()
                .padding(20.0)
                .bg(Color::rgba(0, 0, 0, 150)),
            );
        } else if busy {
            layers.push(column(vec![spinner(72.0, 3.0, ACCENT2)]).center());
        }
        if started && err_is_empty(&p.pl.perr) {
            layers.push(self.player_controls(p, &st, fullscreen, w));
        } else if !started && err_is_empty(&p.pl.perr) {
            // Hali ijro boshlanmagan: katta play tugmasi.
            let (me, pc) = (self.clone(), p.clone());
            layers.push(
                column(vec![Self::pl_round(V_PLAY, 34.0, 64.0, Box::new(move || {
                    if !me.can_watch(&pc) {
                        pc.pl.perr.set("Bu bo'limni ko'rish uchun obuna kerak".into());
                    } else if !pc.eps.get().is_empty() {
                        me.pl_start_default(&pc);
                    } else {
                        // Qismlar hali yuklanmagan: yuklangach ijro o'zi boshlanadi (qayta bosish shart emas).
                        pc.want_play.set(true);
                        pc.pl.loading.set(true);
                        if !pc.eps_loading.get() {
                            me.load_eps(&pc);
                        }
                    }
                }))])
                .center(),
            );
        }
        if fullscreen && p.pl.ep_list.get() {
            layers.push(self.pl_episode_list(p));
        }
        stack(layers).height(h)
    }

    /// Shisha doira tugma (belgi).
    fn gbtn(icon_path: &'static str, size: f32, boxsz: f32, f: impl Fn() + 'static) -> Element {
        Self::pl_round(icon_path, size, boxsz, Box::new(f))
    }

    /// Shisha "tabletka" ichidagi matn (vaqt, sarlavha).
    fn gpill(e: Element) -> Element {
        e.lens(Color::rgba(255, 255, 255, 8), GLASS_ZOOM).radius(16.0).padding_xy(14.0, 8.0)
    }

    /// Ikki marta bosib sek: ko'rsatkich (shisha doira, o'q va soniya).
    fn pl_seek_badge(acc: i32) -> Element {
        let left = acc < 0;
        let secs = acc.abs();
        column(vec![icon(if left { V_PREV } else { V_NEXT }, 22.0).color(Color::WHITE), tx(format!("{secs}s"), 12.0, Color::WHITE).bold()])
            .center()
            .width(76.0)
            .height(76.0)
            .lens(Color::rgba(255, 255, 255, 8), GLASS_ZOOM)
            .radius(38.0)
    }

    pub(super) fn pl_tap(self: &Rc<Self>, p: &Rc<SeasonPage>, left: bool) {
        let now = Instant::now();
        let double = matches!(p.pl.last_tap.get(), Some((l, t)) if l == left && t.elapsed().as_millis() < 380);
        p.pl.last_tap.set(Some((left, now)));
        if p.pl.locked.get() {
            return;
        }
        if !double {
            if p.pl.ctl.get() && p.pl.player.borrow().as_ref().map(|x| x.status().playing).unwrap_or(false) {
                p.pl.ctl.set(false);
            } else {
                self.pl_touch(p);
            }
            return;
        }
        self.pl_touch(p);
        // Haqiqiy chegaralar ichida jamlanadi (Kotlin bilan bir xil: har bosish 5 soniya).
        let st = p.pl.player.borrow().as_ref().map(|x| x.status());
        let (pos, dur) = st.map(|s| (s.pos_ms, s.dur_ms)).unwrap_or((0, 0));
        let acc = p.pl.seek_acc.get();
        let same = (acc < 0) == left || acc == 0;
        let base = if same { acc } else { 0 };
        let eff = pos + base as i64 * 1000;
        let room = if left { (eff / 1000).max(0) } else if dur > 0 { ((dur - eff) / 1000).max(0) } else { 5 };
        let add = room.min(5) as i32;
        if add == 0 {
            return;
        }
        p.pl.seek_acc.set(base + if left { -add } else { add });
        let gen = p.pl.seek_gen.get() + 1;
        p.pl.seek_gen.set(gen);
        let (me, pc) = (self.clone(), p.clone());
        spawn_bg(|| std::thread::sleep(std::time::Duration::from_millis(700)), move |_| {
            if pc.pl.seek_gen.get() != gen {
                return;
            }
            let acc = pc.pl.seek_acc.get();
            pc.pl.seek_acc.set(0);
            if acc != 0 {
                if let Some(pl) = pc.pl.player.borrow().as_ref() {
                    let s = pl.status();
                    let target = (s.pos_ms + acc as i64 * 1000).clamp(0, if s.dur_ms > 0 { s.dur_ms } else { i64::MAX });
                    pl.seek_ms(target);
                }
            }
            me.pl_touch(&pc);
        });
    }

    /// Vaqt chizig'i (hammasi shisha): yuklangan qism xira, ko'rilgani urg'u gradienti, doira. Bosish/surish bilan o'tkaziladi.
    fn pl_bar(p: &Rc<SeasonPage>, frac: f32, buffered: f32, th: f32, thumb: f32) -> Element {
        let w = |x: f32| (x.clamp(0.0, 1.0) * 1000.0).max(0.001);
        let layer = |a: f32, b: Element| row(vec![b.weight(w(a)), column(vec![]).weight((1000.0 - w(a)).max(0.001))]);
        let mid = |e: Element| column(vec![e]).justify(Justify::Center);
        // Chiziq — xira sutdek shisha, ozgina lupa.
        let track = column(vec![]).height(th).radius(th / 2.0).lens(Color::rgba(255, 255, 255, 38), 1.25).radius(th / 2.0);
        let buf = layer(buffered, column(vec![]).height(th).radius(th / 2.0).bg(Color::rgba(255, 255, 255, 46)));
        let played = layer(frac, column(vec![]).height(th).radius(th / 2.0).lens(Color::rgba(255, 255, 255, 90), 1.35).radius(th / 2.0));
        // Dumaloq — shisha, kuchliroq lupa.
        let knob = row(vec![column(vec![]).weight(w(frac)).height(1.0), column(vec![]).width(thumb * 2.0).height(thumb * 2.0).radius(thumb).lens(Color::rgba(255, 255, 255, 60), 1.9).radius(thumb).border(1.5, Color::rgba(255, 255, 255, 170)), column(vec![]).weight((1000.0 - w(frac)).max(0.001)).height(1.0)]).align(Align::Center);
        let pd = p.clone();
        stack(vec![mid(track), mid(buf), mid(played), knob])
            .height(thumb * 2.0 + 14.0)
            .weight(1.0)
            .on_drag(move |fx, fin| {
                if fin {
                    pd.pl.seeking.set(None);
                    if let Some(pl) = pd.pl.player.borrow().as_ref() {
                        let d = pl.status().dur_ms;
                        pl.seek_ms((fx * d as f32) as i64);
                    }
                } else {
                    pd.pl.seeking.set(Some(fx));
                }
            })
    }

    /// Boshqaruv paneli (Kotlin `PlayerControls`): kichik rejim — markazda play/pauza, pastda chiziq + vaqt + sozlama + to'liq ekran;
    /// to'liq ekran — tepada orqaga/sarlavha/qulf/sozlama/qismlar/chiqish, pastda oldingi-pauza-keyingi va chiziq. Hammasi shisha.
    fn player_controls(self: &Rc<Self>, p: &Rc<SeasonPage>, st: &Status, fullscreen: bool, _w: f32) -> Element {
        let dur = st.dur_ms.max(1);
        let locked = p.pl.locked.get();
        let show_all = (p.pl.ctl.get() || !st.playing) && !locked;
        let frac = p.pl.seeking.get().unwrap_or((st.pos_ms as f32 / dur as f32).clamp(0.0, 1.0));
        let pos = if let Some(f) = p.pl.seeking.get() { (f * dur as f32) as i64 } else { st.pos_ms };
        let buffered = (st.buffered_ms as f32 / dur as f32).clamp(0.0, 1.0);
        let edge = if fullscreen { 36.0 } else { 0.0 };
        let mut layers: Vec<Element> = Vec::new();

        // Qulflangan: faqat qulf tugmasi (yuqori o'ng).
        if locked {
            let (pc, me) = (p.clone(), self.clone());
            layers.push(column(vec![row(vec![spacer(), Self::gbtn(V_LOCK, 20.0, 42.0, move || {
                pc.pl.locked.set(false);
                me.pl_touch(&pc);
            })])
            .padding_each(12.0 + edge, 8.0, 12.0 + edge, 0.0)]));
            return stack(layers);
        }

        // Boshqaruv yashirin: pastda ingichka doimiy chiziq.
        if !show_all {
            let w = |x: f32| (x.clamp(0.0, 1.0) * 1000.0).max(0.001);
            layers.push(column(vec![
                spacer(),
                stack(vec![
                    column(vec![]).height(3.0).bg(Color::rgba(255, 255, 255, 50)),
                    row(vec![column(vec![]).weight(w(frac)).height(3.0).gradient(ACCENT, ACCENT2), column(vec![]).weight((1000.0 - w(frac)).max(0.001))]),
                ])
                .height(3.0),
            ]));
        } else {
            // Qorayish (tepa va past) — yozuvlar o'qilishi uchun.
            layers.push(column(vec![
                column(vec![]).height(90.0).gradient(Color::rgba(0, 0, 0, 150), Color::rgba(0, 0, 0, 0)),
                spacer(),
                column(vec![]).height(if fullscreen { 130.0 } else { 80.0 }).gradient(Color::rgba(0, 0, 0, 0), Color::rgba(0, 0, 0, 200)),
            ]));
        }

        // Ikki marta bosib sek ko'rsatkichi.
        let acc = p.pl.seek_acc.get();
        if acc != 0 {
            layers.push(row(vec![
                if acc < 0 { column(vec![Self::pl_seek_badge(acc)]).center().weight(1.0) } else { column(vec![]).weight(1.0) },
                if acc > 0 { column(vec![Self::pl_seek_badge(acc)]).center().weight(1.0) } else { column(vec![]).weight(1.0) },
            ]));
        }

        // Intro o'tkazish.
        let intro = p.pl.cur.get().and_then(|c| p.eps.get().iter().find(|e| e.id == c.0).cloned()).and_then(|ep| ep.intro.iter().find(|(a, b)| st.pos_ms >= *a && st.pos_ms < *b).map(|(_, b)| *b));
        if let Some(b) = intro {
            let pc = p.clone();
            let btn = Self::gpill(tx("Introni o'tkazish >", 13.0, Color::WHITE).bold()).on_click(move || {
                if let Some(pl) = pc.pl.player.borrow().as_ref() {
                    pl.seek_ms(b);
                }
            });
            layers.push(column(vec![spacer(), row(vec![spacer(), btn]).padding_each(0.0, 0.0, 14.0 + edge, if show_all { if fullscreen { 120.0 } else { 46.0 } } else { 16.0 })]));
        }
        if !show_all {
            return stack(layers);
        }

        let (title, subtitle) = {
            let cur = p.pl.cur.get();
            let ep = cur.as_ref().and_then(|c| p.eps.get().iter().find(|e| e.id == c.0).cloned());
            let bolim = p.season.section_no.max(1);
            (p.season.name.clone(), ep.map(|e| format!("{bolim}-bo'lim · {}-qism", e.number)).unwrap_or_default())
        };
        let (me, pc) = (self.clone(), p.clone());
        let back = Self::gbtn(V_BACK, 22.0, 42.0, move || {
            if pc.pl.fullscreen.get() {
                me.pl_set_fullscreen(&pc, false);
            } else {
                me.pop();
            }
        });
        let (me, pc) = (self.clone(), p.clone());
        let full = Self::gbtn(if fullscreen { V_FULL_EXIT } else { V_FULL }, 20.0, if fullscreen { 42.0 } else { 36.0 }, move || {
            let on = !pc.pl.fullscreen.get();
            me.pl_set_fullscreen(&pc, on);
        });
        let pc = p.clone();
        let gear = Self::gbtn(V_GEAR, 20.0, if fullscreen { 42.0 } else { 36.0 }, move || pc.pl.settings.set(true));
        let time = |a: i64, b: i64, sep: &str| {
            let (txt, size) = (format!("{}{sep}{}", mmss(a), mmss(b)), if fullscreen { 13.0 } else { 11.5 });
            let w = txt.chars().count() as f32 * size * 0.62 + 28.0;
            column(vec![tx(txt, size, Color::WHITE).bold()]).center().width(w).height(if fullscreen { 38.0 } else { 34.0 }).lens(Color::rgba(255, 255, 255, 14), GLASS_ZOOM).radius(19.0)
        };

        if fullscreen {
            let (pc, me) = (p.clone(), self.clone());
            let lock = Self::gbtn(V_UNLOCK, 20.0, 42.0, move || {
                pc.pl.locked.set(true);
                me.pl_touch(&pc);
            });
            let pc = p.clone();
            let list = Self::gbtn(V_LIST, 20.0, 42.0, move || pc.pl.ep_list.set(true));
            let mut head = vec![Self::gpill(column(if subtitle.is_empty() { vec![tx(title, 14.0, Color::WHITE).bold()] } else { vec![tx(title, 14.0, Color::WHITE).bold(), tx(subtitle, 11.5, Color::WHITE)] })).weight(1.0)];
            head.splice(0..0, [back]);
            head.extend([lock, gear, list, full]);
            layers.push(column(vec![row(head).gap(8.0).align(Align::Center).padding_each(12.0 + edge, 6.0, 12.0 + edge, 0.0), spacer()]));
            let (m1, p1, m2, p2, m3, p3) = (self.clone(), p.clone(), self.clone(), p.clone(), self.clone(), p.clone());
            let ctl = row(vec![
                spacer(),
                Self::gbtn(V_PREV, 24.0, 44.0, move || m1.pl_step(&p1, -1)),
                Self::gbtn(if st.playing { V_PAUSE } else { V_PLAY }, 30.0, 54.0, move || m2.pl_toggle_play(&p2)),
                Self::gbtn(V_NEXT, 24.0, 44.0, move || m3.pl_step(&p3, 1)),
            ])
            .gap(10.0)
            .align(Align::Center);
            let bar_row = row(vec![Self::pl_bar(p, frac, buffered, 6.0, 8.0), time(pos, st.dur_ms, " / ")]).gap(10.0).align(Align::Center);
            layers.push(column(vec![spacer(), column(vec![ctl, gap_box(0.0, 6.0), bar_row]).padding_each(12.0 + edge, 0.0, 12.0 + edge, 6.0)]));
        } else {
            layers.push(column(vec![row(vec![back, spacer()]).padding_each(8.0, 8.0, 8.0, 0.0), spacer()]));
            let (me, pc) = (self.clone(), p.clone());
            layers.push(column(vec![column(vec![Self::gbtn(if st.playing { V_PAUSE } else { V_PLAY }, 36.0, 64.0, move || me.pl_toggle_play(&pc))]).center().grow(1.0)]).center());
            let bar_row = row(vec![Self::pl_bar(p, frac, buffered, 4.0, 6.0), time(pos, st.dur_ms, "/"), gear, full]).gap(6.0).align(Align::Center);
            layers.push(column(vec![spacer(), bar_row.padding_each(10.0, 0.0, 6.0, 4.0)]));
        }
        stack(layers)
    }

    /// To'liq ekran: qismlar ro'yxati (o'ngda shisha oyna).
    fn pl_episode_list(self: &Rc<Self>, p: &Rc<SeasonPage>) -> Element {
        let cur = p.pl.cur.get().map(|c| c.0);
        let eps = p.eps.get();
        let mut kids: Vec<Element> = vec![tx("Qismlar", 16.0, Color::WHITE).bold().margin_each(4.0, 0.0, 0.0, 10.0)];
        for e in eps.iter() {
            let (me, pc, ep) = (self.clone(), p.clone(), e.clone());
            let on = Some(e.id) == cur;
            let row_el = row(vec![tx(format!("{}-qism", e.number), 14.0, Color::WHITE).bold().weight(1.0), tx(if on { "ijroda" } else { "" }, 11.0, ACCENT2)]).align(Align::Center).padding_xy(14.0, 11.0);
            let card = if on { row_el.gradient(ACCENT.with_opacity(0.55), ACCENT2.with_opacity(0.55)) } else { row_el.lens(Color::rgba(255, 255, 255, 8), GLASS_ZOOM) };
            kids.push(card.radius(16.0).margin_each(0.0, 0.0, 0.0, 6.0).on_click(move || {
                pc.pl.ep_list.set(false);
                me.play_episode(&pc, &ep, None, Some(0), true);
            }));
        }
        let panel = scroll_column(&p.pl.ep_scroll, kids).lens(Color::rgba(255, 255, 255, 14), GLASS_ZOOM).radius(24.0).padding(12.0).width(260.0);
        let pc = p.clone();
        stack(vec![column(vec![]).bg(Color::rgba(0, 0, 0, 90)).on_click(move || pc.pl.ep_list.set(false)), row(vec![spacer(), panel.margin_each(0.0, 16.0, 16.0, 16.0)])])
    }

    /// Pleyer sozlamalari: sifat, avto-keyingi, intro avto o'tkazish, uyqu taymeri.
    pub(super) fn player_settings(self: &Rc<Self>, p: &Rc<SeasonPage>) -> Element {
        let cur = p.pl.cur.get();
        let ep = cur.as_ref().and_then(|c| p.eps.get().iter().find(|e| e.id == c.0).cloned());
        let mut kids: Vec<Element> = vec![tx("Sozlamalar", 18.0, Color::WHITE).bold(), gap_box(0.0, 12.0)];
        if let (Some(e), Some((_, q, _))) = (&ep, &cur) {
            kids.push(tx("Sifat", 13.0, Color::WHITE).bold());
            kids.push(gap_box(0.0, 8.0));
            let chips: Vec<Element> = e
                .qualities
                .iter()
                .map(|qq| {
                    let (me, pc, e2, q2) = (self.clone(), p.clone(), e.clone(), qq.clone());
                    let size = e.size(qq);
                    let label = format!("{}{}", e.quality_title(qq), if size > 0 { format!(" · {}", crate::data::format_bytes(size)) } else { String::new() });
                    self.chip(&label, qq == q, move || {
                        pc.pl.settings.set(false);
                        let pos = pc.pl.player.borrow().as_ref().map(|x| x.status().pos_ms).unwrap_or(0);
                        me.play_episode(&pc, &e2, Some(q2.clone()), Some(pos), true);
                    })
                })
                .collect();
            kids.push(row(chips).gap(8.0).scroll(&self.chips_scroll2).height(44.0));
            kids.push(gap_box(0.0, 12.0));
        }
        let toggle = |label: &str, st: &State<bool>| {
            let s2 = st.clone();
            row(vec![tx(label, 14.0, Color::WHITE).weight(1.0), Self::switch(st.get(), move || s2.set(!s2.get()))]).align(Align::Center).padding_xy(2.0, 8.0)
        };
        kids.push(tx("Tezlik", 13.0, Color::WHITE).bold());
        kids.push(gap_box(0.0, 8.0));
        let speeds: Vec<Element> = [(50u32, "0.5x"), (75, "0.75x"), (100, "1x"), (125, "1.25x"), (150, "1.5x"), (200, "2x")]
            .iter()
            .map(|(v, l)| {
                let pc = p.clone();
                let v = *v;
                self.chip(l, pc.pl.speed.get() == v, move || {
                    pc.pl.speed.set(v);
                    if let Some(pl) = pc.pl.player.borrow().as_ref() {
                        pl.set_speed(v as f32 / 100.0);
                    }
                })
            })
            .collect();
        kids.push(row(speeds).gap(8.0).scroll(&self.chips_scroll2).height(44.0));
        kids.push(gap_box(0.0, 12.0));
        kids.push(toggle("Avto keyingi qism", &p.pl.auto_next));
        kids.push(toggle("Introni avto o'tkazish", &p.pl.auto_skip));
        kids.push(tx("Uyqu taymeri", 13.0, Color::WHITE).bold().padding_each(0.0, 8.0, 0.0, 8.0));
        let timers: Vec<Element> = [(0, "O'chiq"), (15, "15 daq"), (30, "30 daq"), (60, "60 daq")]
            .iter()
            .map(|(m, l)| {
                let pc = p.clone();
                let m = *m;
                let on = m == 0 && pc.pl.sleep_at.get().is_none();
                self.chip(l, on, move || {
                    pc.pl.sleep_at.set(if m == 0 { None } else { Some(Instant::now() + std::time::Duration::from_secs(m as u64 * 60)) });
                    pc.pl.settings.set(false);
                })
            })
            .collect();
        kids.push(row(timers).gap(8.0).scroll(&self.chips_scroll2).height(44.0));
        let pc = p.clone();
        kids.push(row(vec![spacer(), self.chip("Yopish", true, move || pc.pl.settings.set(false))]).padding_each(0.0, 14.0, 0.0, 0.0));
        let panel = scroll_column(&p.pl.set_scroll, kids).lens(Color::rgba(255, 255, 255, 14), GLASS_ZOOM).radius(28.0).padding(20.0).width(380.0).margin_each(0.0, 12.0, 0.0, 12.0);
        let pc = p.clone();
        stack(vec![column(vec![]).bg(Color::rgba(0, 0, 0, 45)).on_click(move || pc.pl.settings.set(false)), row(vec![spacer(), panel, spacer()])])
    }
}

fn err_is_empty(s: &State<String>) -> bool {
    s.with(|v| v.is_empty())
}

#[allow(dead_code)]
fn _unused(_: Source) {}
