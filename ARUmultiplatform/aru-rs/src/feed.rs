//! Video ro'yxati: sahifalab yuklanadi; oxirgi muvaffaqiyatli ro'yxat diskda saqlanadi (keyingi ochilishda darrov ko'rinadi).

use crate::json::J;
use crate::model::Video;
use crate::net;
use std::cell::{Cell, RefCell};
use std::time::Instant;
use uzcoders::prelude::*;

pub const PAGE: usize = 20;
/// Shuncha soniyada javob kelmasa — "javob kelmadi" ko'rsatiladi va qayta urinish mumkin.
pub const STALL_SECS: u64 = 20;

pub struct Feed {
    pub items: State<Vec<Video>>,
    pub loading: State<bool>,
    pub err: State<String>,
    pub more: State<bool>,
    /// Bir marta yuklash boshlanganmi (ekran birinchi ko'ringanda).
    pub asked: Cell<bool>,
    started: Cell<Option<Instant>>,
    gen: Cell<u32>,
    key: RefCell<String>,
}

fn cache_path(key: &str) -> String {
    format!("{}/aru_feed_{key}.json", uzcoders::android::files_dir())
}

fn parse_list(j: &J) -> Vec<Video> {
    j.get("videos").arr().iter().map(Video::from).collect()
}

impl Feed {
    pub fn new() -> Rc<Feed> {
        Rc::new(Feed {
            items: State::new(Vec::new()),
            loading: State::new(false),
            err: State::new(String::new()),
            more: State::new(false),
            asked: Cell::new(false),
            started: Cell::new(None),
            gen: Cell::new(0),
            key: RefCell::new(String::new()),
        })
    }

    /// Javob juda kech kelyaptimi.
    pub fn stalled(&self) -> bool {
        self.loading.get() && self.started.get().is_some_and(|t| t.elapsed().as_secs() >= STALL_SECS)
    }

    /// `q` — so'rov qismi (`sort=new&me=...`). `append` — keyingi sahifa. `force` — qotib qolgan so'rov o'rniga yangisi.
    pub fn fetch(self: &Rc<Self>, q: String, append: bool, force: bool) {
        if self.loading.get() && !force {
            return;
        }
        self.asked.set(true);
        self.loading.set(true);
        self.started.set(Some(Instant::now()));
        let gen = self.gen.get().wrapping_add(1);
        self.gen.set(gen);
        let off = if append { self.items.with(|v| v.len()) } else { 0 };
        let me = self.clone();
        spawn_bg(
            move || net::call_text("GET", &format!("/api/videos?{q}&limit={PAGE}&offset={off}"), ""),
            move |r| {
                if me.gen.get() != gen {
                    return; // eskirgan javob
                }
                me.loading.set(false);
                match r {
                    Ok((j, text)) => {
                        let list = parse_list(&j);
                        me.more.set(list.len() >= PAGE);
                        me.err.set(String::new());
                        if !append {
                            let key = me.key.borrow().clone();
                            if !key.is_empty() {
                                let _ = std::fs::write(cache_path(&key), text);
                            }
                        }
                        me.items.update(|v| {
                            if !append {
                                v.clear();
                            }
                            v.extend(list);
                        });
                    }
                    Err(e) => me.err.set(e),
                }
            },
        );
    }

    /// Birinchi ko'rinishda bir marta: avval diskdagi ro'yxat (darrov), keyin serverdan yangilanadi.
    pub fn ensure(self: &Rc<Self>, key: &str, q: String) {
        if self.asked.get() {
            return;
        }
        if !key.is_empty() {
            *self.key.borrow_mut() = key.to_string();
            if self.items.with(|v| v.is_empty()) {
                if let Some(j) = std::fs::read_to_string(cache_path(key)).ok().and_then(|t| J::parse(&t)) {
                    let list = parse_list(&j);
                    if !list.is_empty() {
                        self.items.set(list);
                    }
                }
            }
        }
        self.fetch(q, false, false);
    }

    pub fn reset(&self) {
        self.asked.set(false);
    }
}
