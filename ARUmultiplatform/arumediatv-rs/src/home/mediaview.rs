//! Chatdagi rasm/video ko'ruvchisi (Kotlin `MediaViewer`): video ilova ichida (ExoPlayer, play/pauza va surish chizig'i),
//! rasm — to'liq ekranda. Avval video tashqi ko'ruvchida ochilishga urinardi va ochilmasdi.

use super::*;
use crate::downloads;
use crate::player::{Player, State as PState, Status};
use std::cell::{Cell, RefCell};

const M_PLAY: &str = "M8,5v14l11,-7z";
const M_PAUSE: &str = "M6,19h4V5H6v14zM14,5v14h4V5h-4z";
const M_BACK: &str = "M20,11H7.83l5.59,-5.59L12,4l-8,8 8,8 1.41,-1.41L7.83,13H20v-2z";

pub struct MediaPage {
    url: String,
    kind: &'static str,
    player: RefCell<Option<Player>>,
    loading: State<bool>,
    err: State<String>,
    status: State<Option<Status>>,
    held: Cell<bool>,
    closed: Cell<bool>,
    ticking: Cell<bool>,
    seeking: State<Option<f32>>,
    /// Boshqaruv paneli ko'rinadimi (ijro paytida bir necha soniyadan keyin yashiriladi).
    ctl: State<bool>,
    ctl_at: Cell<std::time::Instant>,
}

fn clock(ms: i64) -> String {
    let s = (ms.max(0) / 1000) as i64;
    let (h, m, sec) = (s / 3600, s % 3600 / 60, s % 60);
    if h > 0 { format!("{h}:{m:02}:{sec:02}") } else { format!("{m}:{sec:02}") }
}

impl Home {
    /// Chatdagi rasm yoki videoni ilova ichida ochadi. `kind`: "video" | "round" | "image".
    pub(super) fn open_media(self: &Rc<Self>, url: &str, kind: &str) {
        uzcoders::prelude::breadcrumb("open_media");
        let is_video = kind == "video" || kind == "round";
        let p = Rc::new(MediaPage {
            url: url.to_string(),
            kind: if is_video { "video" } else { "image" },
            player: RefCell::new(None),
            loading: State::new(is_video),
            err: State::new(String::new()),
            status: State::new(None),
            held: Cell::new(false),
            closed: Cell::new(false),
            ticking: Cell::new(false),
            seeking: State::new(None),
            ctl: State::new(true),
            ctl_at: Cell::new(std::time::Instant::now()),
        });
        self.push(pages::Page::Media(p.clone()));
        if !is_video {
            let have = self.posters.borrow().contains_key(url);
            if !have {
                self.queue.borrow_mut().push(url.to_string());
                self.next_posters();
            }
            return;
        }
        let (me, p2, session, u) = (self.clone(), p.clone(), self.session.clone(), url.to_string());
        spawn_bg(move || downloads::resolve(&session, &u), move |r| {
            if p2.closed.get() {
                if let Ok((_, true)) = r {
                    downloads::release_hold();
                }
                return;
            }
            p2.loading.set(false);
            match r {
                Ok((src, held)) => {
                    p2.held.set(held);
                    *p2.player.borrow_mut() = Some(Player::open(src, 0, true));
                    crate::gate::BUSY.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    uzcoders::android::keep_screen_on(true);
                    me.media_tick(&p2);
                }
                Err(e) => p2.err.set(e),
            }
        });
    }

    /// Sahifa yopilganda: pleyer to'xtatiladi.
    pub(super) fn media_close(&self, p: &Rc<MediaPage>) {
        if p.closed.replace(true) {
            return;
        }
        if let Some(pl) = p.player.borrow_mut().take() {
            pl.close();
            crate::gate::BUSY.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        }
        if p.held.replace(false) {
            downloads::release_hold();
        }
        uzcoders::android::keep_screen_on(false);
    }

    /// Har ~500 ms holat yangilanadi (chiziq, vaqt); ijro paytida panel 3.5 soniyadan keyin yashiriladi.
    fn media_tick(self: &Rc<Self>, p: &Rc<MediaPage>) {
        if p.ticking.replace(true) {
            return;
        }
        fn step(me: Rc<Home>, p: Rc<MediaPage>) {
            let (m2, p2) = (me.clone(), p.clone());
            spawn_bg(|| std::thread::sleep(std::time::Duration::from_millis(500)), move |_| {
                let open = !p2.closed.get() && m2.pages.get().iter().any(|pg| matches!(pg, pages::Page::Media(q) if Rc::ptr_eq(q, &p2)));
                let st = if open { p2.player.borrow().as_ref().map(|pl| pl.status()) } else { None };
                let Some(st) = st else {
                    p2.ticking.set(false);
                    return;
                };
                if p2.ctl.get() && st.playing && p2.ctl_at.get().elapsed().as_millis() > 3500 {
                    p2.ctl.set(false);
                }
                p2.status.set(Some(st));
                step(m2, p2);
            });
        }
        step(self.clone(), p.clone());
    }

    fn media_touch(&self, p: &Rc<MediaPage>) {
        p.ctl.set(true);
        p.ctl_at.set(std::time::Instant::now());
    }

    fn media_glass_btn(icon_path: &'static str, size: f32, boxsz: f32, f: impl Fn() + 'static) -> Element {
        column(vec![icon(icon_path, size).color(Color::WHITE)]).center().width(boxsz).height(boxsz).radius(boxsz / 2.0).lens(Color::rgba(255, 255, 255, 8), GLASS_ZOOM).radius(boxsz / 2.0).on_click(f)
    }

    pub(super) fn media_page(self: &Rc<Self>, p: &Rc<MediaPage>) -> Element {
        let ins = safe_insets();
        let (w, h) = window_dp();
        let me_b = self.clone();
        let back = Self::media_glass_btn(M_BACK, 24.0, 52.0, move || {
            me_b.pop();
        });
        let top = row(vec![back, spacer()]).padding_each(12.0, 8.0 + ins.top, 12.0, 0.0);

        if p.kind == "image" {
            let body: Element = match self.posters.borrow().get(&p.url).cloned() {
                Some(img) => {
                    let k = (w / img.w as f32).min((h - ins.top - ins.bottom - 70.0) / img.h as f32);
                    column(vec![image(&img, img.w as f32 * k, img.h as f32 * k)]).center()
                }
                None => column(vec![spinner(32.0, 3.0, Color::WHITE)]).center(),
            };
            let _ = self.gate_rev.get();
            return stack(vec![column(vec![]).bg(Color::BLACK), body.grow(1.0), column(vec![top])]);
        }

        let st = p.status.get();
        let (vw, vh) = match &st {
            Some(s) if s.width > 0 && s.height > 0 => {
                let ar = s.width as f32 / s.height as f32;
                if w / h > ar { (h * ar, h) } else { (w, w / ar) }
            }
            _ => (w, w * 9.0 / 16.0),
        };
        let me_t = self.clone();
        let pt = p.clone();
        let video = column(vec![column(vec![]).video().width(vw).height(vh)]).center().on_click(move || {
            if pt.ctl.get() {
                pt.ctl.set(false);
            } else {
                me_t.media_touch(&pt);
            }
        });
        let mut layers: Vec<Element> = vec![column(vec![]).bg(Color::BLACK), video];
        let err = p.err.get();
        if p.loading.get() {
            layers.push(column(vec![spinner(48.0, 3.0, ACCENT2)]).center());
        } else if !err.is_empty() {
            layers.push(column(vec![tx(err, 14.0, Color::WHITE).bold()]).center().padding(24.0));
        } else if let Some(s) = st {
            if s.state == PState::Error {
                layers.push(column(vec![tx(if s.error.is_empty() { "Video ochilmadi".to_string() } else { s.error.clone() }, 14.0, Color::WHITE).bold()]).center().padding(24.0));
            } else if s.state == PState::Opening || (s.buffering && s.playing) {
                layers.push(column(vec![spinner(48.0, 3.0, ACCENT2)]).center());
            }
            if p.ctl.get() || !s.playing {
                let dur = s.dur_ms.max(1);
                let frac = p.seeking.get().unwrap_or((s.pos_ms as f32 / dur as f32).clamp(0.0, 1.0));
                let pos = p.seeking.get().map(|f| (f * dur as f32) as i64).unwrap_or(s.pos_ms);
                let buffered = (s.buffered_ms as f32 / dur as f32).clamp(0.0, 1.0);
                let wt = |x: f32| (x.clamp(0.0, 1.0) * 1000.0).max(0.001);
                let mid = |e: Element| column(vec![e]).justify(Justify::Center);
                let track = column(vec![]).height(6.0).radius(3.0).bg(Color::rgba(255, 255, 255, 50));
                let buf = row(vec![column(vec![]).weight(wt(buffered)).height(6.0).radius(3.0).bg(Color::rgba(255, 255, 255, 70)), column(vec![]).weight((1000.0 - wt(buffered)).max(0.001))]);
                let played = row(vec![column(vec![]).weight(wt(frac)).height(6.0).radius(3.0).gradient(ACCENT, ACCENT2), column(vec![]).weight((1000.0 - wt(frac)).max(0.001))]);
                let knob = row(vec![column(vec![]).weight(wt(frac)).height(1.0), column(vec![]).width(20.0).height(20.0).radius(10.0).bg(Color::WHITE), column(vec![]).weight((1000.0 - wt(frac)).max(0.001)).height(1.0)]).align(Align::Center);
                let pd = p.clone();
                let bar = stack(vec![mid(track), mid(buf), mid(played), knob]).height(30.0).weight(1.0).on_drag(move |fx, fin| {
                    if fin {
                        pd.seeking.set(None);
                        if let Some(pl) = pd.player.borrow().as_ref() {
                            pl.seek_ms((fx * pl.status().dur_ms as f32) as i64);
                        }
                    } else {
                        pd.seeking.set(Some(fx));
                    }
                });
                let bottom = Self::pglass(row(vec![
                    tx(clock(pos), 12.5, Color::WHITE).bold(),
                    bar,
                    tx(clock(s.dur_ms), 12.5, Color::WHITE).bold(),
                ])
                .gap(10.0)
                .align(Align::Center)
                .padding_xy(16.0, 8.0))
                .radius(28.0)
                .margin_each(14.0, 0.0, 14.0, 14.0 + ins.bottom);
                let (me_p, pp) = (self.clone(), p.clone());
                let play = Self::media_glass_btn(if s.playing { M_PAUSE } else { M_PLAY }, 34.0, 72.0, move || {
                    if let Some(pl) = pp.player.borrow().as_ref() {
                        if pl.status().playing { pl.pause() } else { pl.play() }
                    }
                    me_p.media_touch(&pp);
                });
                layers.push(column(vec![column(vec![top]), spacer(), row(vec![spacer(), play, spacer()]), spacer(), bottom]));
            }
        } else {
            layers.push(column(vec![top]));
        }
        if p.ctl.get() && p.status.get().is_none() {
            // Holat hali yo'q: faqat orqaga tugmasi.
        }
        stack(layers)
    }
}
