//! Ilova holati va mantiq: kirish, ro'yxatlar, pleyer, yuklash, like/obuna/fikr.

use crate::feed::Feed;
use crate::ident::{self, Me};
use crate::json::esc;
use crate::media::{self, Status, Up};
use crate::model::*;
use crate::net;
use std::cell::{Cell, RefCell};
use std::time::Instant;
use uzcoders::prelude::*;

#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
    Home,
    Reels,
    Subs,
    Me,
}

pub struct App {
    pub me: State<Option<Me>>,
    pub tab: State<Tab>,
    pub watch: State<Option<Video>>,
    pub create: State<bool>,
    pub search_on: State<bool>,
    pub hot: State<bool>,
    pub f_new: Rc<Feed>,
    pub f_hot: Rc<Feed>,
    pub f_subs: Rc<Feed>,
    pub f_reels: Rc<Feed>,
    pub f_mine: Rc<Feed>,
    pub f_search: Rc<Feed>,
    pub reel_i: State<usize>,
    pub chan: State<Option<Channel>>,
    pub mine: State<Option<Channel>>,
    pub notes: State<Vec<Note>>,
    pub note_text: State<String>,
    pub name_text: State<String>,
    pub busy: State<bool>,
    pub msg: State<String>,
    pub search_text: State<String>,
    pub pos: State<f32>,
    pub st: RefCell<Status>,
    pub full: State<bool>,
    pub confirm_del: State<bool>,
    pub pending: State<Option<String>>,
    pub up_kind: State<String>,
    pub title_text: State<String>,
    pub up: State<Up>,
    pub sc_home: ScrollState,
    pub sc_subs: ScrollState,
    pub sc_me: ScrollState,
    pub sc_watch: ScrollState,
    pub sc_search: ScrollState,
    active: RefCell<String>,
    st_at: Cell<Option<Instant>>,
    up_at: Cell<Option<Instant>>,
    drag_at: Cell<Option<Instant>>,
    mine_asked: Cell<bool>,
}

impl App {
    pub fn new() -> Rc<App> {
        Rc::new(App {
            me: State::new(ident::load()),
            tab: State::new(Tab::Home),
            watch: State::new(None),
            create: State::new(false),
            search_on: State::new(false),
            hot: State::new(false),
            f_new: Feed::new(),
            f_hot: Feed::new(),
            f_subs: Feed::new(),
            f_reels: Feed::new(),
            f_mine: Feed::new(),
            f_search: Feed::new(),
            reel_i: State::new(0),
            chan: State::new(None),
            mine: State::new(None),
            notes: State::new(Vec::new()),
            note_text: State::new(String::new()),
            name_text: State::new(String::new()),
            busy: State::new(false),
            msg: State::new(String::new()),
            search_text: State::new(String::new()),
            pos: State::new(0.0),
            st: RefCell::new(Status::default()),
            full: State::new(false),
            confirm_del: State::new(false),
            pending: State::new(None),
            up_kind: State::new("video".into()),
            title_text: State::new(String::new()),
            up: State::new(Up::default()),
            sc_home: ScrollState::new(),
            sc_subs: ScrollState::new(),
            sc_me: ScrollState::new(),
            sc_watch: ScrollState::new(),
            sc_search: ScrollState::new(),
            active: RefCell::new(String::new()),
            st_at: Cell::new(None),
            up_at: Cell::new(None),
            drag_at: Cell::new(None),
            mine_asked: Cell::new(false),
        })
    }

    pub fn uid(&self) -> String {
        self.me.with(|m| m.as_ref().map(|m| m.id.clone()).unwrap_or_default())
    }

    /// So'rov qismi: `extra` + `me=<id>`.
    pub fn q(&self, extra: &str) -> String {
        format!("{extra}&me={}", self.uid())
    }

    pub fn q_new(&self) -> String {
        self.q("sort=new")
    }
    pub fn q_hot(&self) -> String {
        self.q("sort=views")
    }
    pub fn q_subs(&self) -> String {
        format!("sort=new&followed_by={}&me={}", self.uid(), self.uid())
    }
    pub fn q_reels(&self) -> String {
        self.q("kind=reel&sort=new")
    }
    pub fn q_mine(&self) -> String {
        format!("sort=new&author={}&me={}", self.uid(), self.uid())
    }

    // ───────────── kirish (kanal nomi) ─────────────

    pub fn register(self: &Rc<Self>) {
        let name = self.name_text.get().trim().to_string();
        if name.chars().count() < 2 {
            self.msg.set("Kanal nomi kamida 2 ta belgi bo'lsin".into());
            return;
        }
        if self.busy.get() {
            return;
        }
        self.busy.set(true);
        self.msg.set(String::new());
        let me = Me { id: ident::new_id(), name: name.chars().take(40).collect() };
        let (id, nm) = (me.id.clone(), me.name.clone());
        let a = self.clone();
        spawn_bg(
            move || net::call("POST", "/api/user", &format!("{{\"id\":{},\"name\":{}}}", esc(&id), esc(&nm))),
            move |r| {
                a.busy.set(false);
                match r {
                    Ok(_) => {
                        ident::save(&me);
                        a.me.set(Some(me));
                        a.refresh_all();
                    }
                    Err(e) => a.msg.set(e),
                }
            },
        );
    }

    /// Hamma ro'yxatlar qaytadan yuklanadi (yuklashdan, kirishdan keyin).
    pub fn refresh_all(self: &Rc<Self>) {
        for f in [&self.f_new, &self.f_hot, &self.f_subs, &self.f_reels, &self.f_mine, &self.f_search] {
            f.reset();
        }
        self.mine_asked.set(false);
        self.f_new.items.set(Vec::new());
        self.f_hot.items.set(Vec::new());
        self.f_subs.items.set(Vec::new());
        self.f_reels.items.set(Vec::new());
        self.f_mine.items.set(Vec::new());
    }

    pub fn load_mine(self: &Rc<Self>) {
        if self.mine_asked.get() || self.me.with(|m| m.is_none()) {
            return;
        }
        self.mine_asked.set(true);
        let a = self.clone();
        let (id, uid) = (self.uid(), self.uid());
        spawn_bg(move || net::call("GET", &format!("/api/channel/{id}?me={uid}"), ""), move |r| {
            if let Ok(j) = r {
                a.mine.set(Some(Channel::from(&j)));
            } else {
                a.mine_asked.set(false);
            }
        });
    }

    // ───────────── video ochish / yopish ─────────────

    pub fn open_video(self: &Rc<Self>, v: Video) {
        self.pos.set(0.0);
        self.chan.set(None);
        self.notes.set(Vec::new());
        self.note_text.set(String::new());
        self.confirm_del.set(false);
        self.sc_watch.scroll_to_top();
        let (id, uid) = (v.id.clone(), self.uid());
        self.watch.set(Some(v));
        let a = self.clone();
        let id1 = id.clone();
        spawn_bg(move || net::call("GET", &format!("/api/video/{id1}?me={uid}"), ""), move |r| {
            if let Ok(j) = r {
                let vv = Video::from(&j);
                a.chan.set(Some(Channel { id: vv.uid.clone(), name: vv.channel.clone(), videos: 0, views: 0, followers: j.n("followers"), following: j.b("following") }));
                a.patch(&vv.id.clone(), |x| {
                    x.likes = vv.likes;
                    x.liked = vv.liked;
                    x.notes = vv.notes;
                    x.views = vv.views;
                });
            }
        });
        self.load_notes(&id);
    }

    pub fn load_notes(self: &Rc<Self>, id: &str) {
        let (a, id1) = (self.clone(), id.to_string());
        spawn_bg(move || net::call("GET", &format!("/api/video/{id1}/notes"), ""), move |r| {
            if let Ok(j) = r {
                a.notes.set(j.get("notes").arr().iter().map(Note::from).collect());
            }
        });
    }

    pub fn close_video(&self) {
        if self.full.get() {
            self.full.set(false);
            uzcoders::android::set_fullscreen(false);
        }
        self.watch.set(None);
    }

    pub fn set_full(&self, on: bool) {
        self.full.set(on);
        uzcoders::android::set_fullscreen(on);
    }

    /// Videoning maydonlarini hamma joyda (ko'rilayotgan va ro'yxatlarda) yangilaydi.
    fn patch(&self, id: &str, f: impl Fn(&mut Video)) {
        if let Some(mut v) = self.watch.get() {
            if v.id == id {
                f(&mut v);
                self.watch.set(Some(v));
            }
        }
        for feed in [&self.f_new, &self.f_hot, &self.f_subs, &self.f_reels, &self.f_mine, &self.f_search] {
            feed.items.update(|list| {
                for v in list.iter_mut().filter(|v| v.id == id) {
                    f(v);
                }
            });
        }
    }

    // ───────────── like / obuna / fikr / o'chirish ─────────────

    pub fn toggle_like(self: &Rc<Self>, v: &Video) {
        let on = !v.liked;
        self.patch(&v.id, |x| {
            x.liked = on;
            x.likes = (x.likes + if on { 1 } else { -1 }).max(0);
        });
        let (id, uid) = (v.id.clone(), self.uid());
        spawn_bg(move || net::call("POST", &format!("/api/video/{id}/like"), &format!("{{\"uid\":{},\"on\":{on}}}", esc(&uid))), |_| {});
    }

    pub fn toggle_follow(self: &Rc<Self>, target: &str) {
        let Some(c) = self.chan.get() else { return };
        let on = !c.following;
        self.chan.set(Some(Channel { following: on, followers: (c.followers + if on { 1 } else { -1 }).max(0), ..c }));
        self.f_subs.reset();
        self.f_subs.items.set(Vec::new());
        let (t, uid) = (target.to_string(), self.uid());
        spawn_bg(move || net::call("POST", "/api/follow", &format!("{{\"uid\":{},\"target\":{},\"on\":{on}}}", esc(&uid), esc(&t))), |_| {});
    }

    pub fn add_note(self: &Rc<Self>) {
        let Some(v) = self.watch.get() else { return };
        let text = self.note_text.get().trim().to_string();
        if text.is_empty() {
            return;
        }
        self.note_text.set(String::new());
        let at = (self.st.borrow().pos_ms / 1000).max(0);
        let (a, uid, id) = (self.clone(), self.uid(), v.id.clone());
        let id2 = id.clone();
        spawn_bg(
            move || net::call("POST", &format!("/api/video/{id}/notes"), &format!("{{\"uid\":{},\"at\":{at},\"text\":{}}}", esc(&uid), esc(&text))),
            move |r| match r {
                Ok(_) => {
                    a.patch(&id2, |x| x.notes += 1);
                    a.load_notes(&id2);
                }
                Err(e) => a.msg.set(e),
            },
        );
    }

    pub fn delete_video(self: &Rc<Self>) {
        let Some(v) = self.watch.get() else { return };
        self.confirm_del.set(false);
        let (a, id, uid) = (self.clone(), v.id.clone(), self.uid());
        spawn_bg(move || net::call("DELETE", &format!("/api/video/{id}?me={uid}"), ""), move |r| match r {
            Ok(_) => {
                a.close_video();
                a.refresh_all();
            }
            Err(e) => a.msg.set(e),
        });
    }

    // ───────────── pleyer ─────────────

    /// Qaysi video o'ynashi kerak: ochiq video sahifasi, aks holda (Oqim bo'limi ochiq bo'lsa) joriy oqim.
    fn desired(&self) -> Option<(String, Video)> {
        if let Some(v) = self.watch.get() {
            return Some((format!("w:{}", v.id), v));
        }
        let covered = self.create.get() || self.search_on.get() || self.pending.get().is_some() || self.me.with(|m| m.is_none());
        if self.tab.get() == Tab::Reels && !covered {
            let i = self.reel_i.get();
            if let Some(v) = self.f_reels.items.with(|l| l.get(i).cloned()) {
                return Some((format!("r:{}", v.id), v));
            }
        }
        None
    }

    /// Har kadrda: kerakli video o'zgargan bo'lsa pleyerni almashtiradi; holatni so'raydi.
    pub fn sync_player(self: &Rc<Self>) {
        let want = self.desired();
        let key = want.as_ref().map(|(k, _)| k.clone()).unwrap_or_default();
        if *self.active.borrow() != key {
            *self.active.borrow_mut() = key;
            *self.st.borrow_mut() = Status::default();
            self.pos.set(0.0);
            match want {
                Some((_, v)) => {
                    media::open(&v.id, v.size, 0, true);
                    let id = v.id.clone();
                    spawn_bg(move || net::call("POST", &format!("/api/video/{id}/view"), "{}"), |_| {});
                }
                None => media::cmd("close"),
            }
        }
        if self.active.borrow().is_empty() {
            return;
        }
        let due = self.st_at.get().map_or(true, |t| t.elapsed().as_millis() >= 200);
        if due {
            self.st_at.set(Some(Instant::now()));
            let s = media::status();
            let dragging = self.drag_at.get().is_some_and(|t| t.elapsed().as_millis() < 700);
            if !dragging && s.dur_ms > 0 {
                self.pos.set((s.pos_ms as f32 / s.dur_ms as f32).clamp(0.0, 1.0));
            }
            *self.st.borrow_mut() = s;
        }
        let s = self.st.borrow();
        if s.playing || s.buffering || !s.ready {
            request_frame();
        }
    }

    pub fn play_pause(&self) {
        let playing = self.st.borrow().playing;
        media::cmd(if playing { "pause" } else { "play" });
        self.st_at.set(None);
    }

    /// Chizg'ichdan surilganda: `up` — barmoq ko'tarilganda aynan shu joyga o'tiladi.
    pub fn scrub(&self, x: f32, up: bool) {
        self.drag_at.set(Some(Instant::now()));
        if up {
            let dur = self.st.borrow().dur_ms;
            if dur > 0 {
                media::cmd(&format!("seek:{}", (x * dur as f32) as i64));
                self.st_at.set(None);
            }
        }
    }

    pub fn seek_secs(&self, secs: i64) {
        media::cmd(&format!("seek:{}", secs.max(0) * 1000));
        self.st_at.set(None);
    }

    pub fn reel_go(&self, d: i32) {
        let n = self.f_reels.items.with(|l| l.len());
        if n == 0 {
            return;
        }
        let i = self.reel_i.get() as i32 + d;
        self.reel_i.set(i.clamp(0, n as i32 - 1) as usize);
    }

    // ───────────── yuklash ─────────────

    pub fn pick(&self, kind: &str) {
        self.up_kind.set(kind.to_string());
        self.create.set(false);
        uzcoders::android::pick_file("video/*");
    }

    /// Tanlangan fayllarni oladi (har kadrda).
    pub fn take_picked(&self) {
        for p in uzcoders::android::take_picked() {
            let name = std::path::Path::new(&p).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let name = if name.starts_with("pick_") { String::new() } else { name };
            self.title_text.set(name);
            self.pending.set(Some(p));
        }
    }

    pub fn start_upload(&self) {
        let Some(path) = self.pending.get() else { return };
        let title = self.title_text.get().trim().to_string();
        if title.is_empty() {
            self.msg.set("Sarlavha yozing".into());
            return;
        }
        self.msg.set(String::new());
        self.pending.set(None);
        self.up.set(Up { state: "run".into(), sent: 0, total: 1, err: String::new() });
        media::upload(&self.uid(), &self.up_kind.get(), &title, &path);
    }

    /// Yuklash borishini so'raydi; tugagach ro'yxatlarni yangilaydi.
    pub fn poll_upload(self: &Rc<Self>) {
        if self.up.with(|u| u.state != "run") {
            return;
        }
        request_frame();
        if self.up_at.get().is_some_and(|t| t.elapsed().as_millis() < 400) {
            return;
        }
        self.up_at.set(Some(Instant::now()));
        let u = media::up_status();
        if u.state == "idle" {
            return;
        }
        if u != self.up.get() {
            self.up.set(u.clone());
        }
        if u.state == "done" {
            self.refresh_all();
        }
    }

    pub fn cancel_upload(&self) {
        media::upload_cancel();
        self.up.set(Up::default());
    }

    // ───────────── orqaga tugmasi ─────────────

    pub fn back(&self) -> bool {
        if self.me.with(|m| m.is_none()) {
            return false;
        }
        if self.pending.get().is_some() {
            self.pending.set(None);
        } else if self.create.get() {
            self.create.set(false);
        } else if self.search_on.get() {
            self.search_on.set(false);
        } else if self.watch.get().is_some() {
            self.close_video();
        } else if self.tab.get() != Tab::Home {
            self.tab.set(Tab::Home);
        } else {
            return false;
        }
        true
    }
}
