//! Video ro'yxati: sahifalab yuklanadi, xato va "yana bor" holatlari bilan.

use crate::model::Video;
use crate::net;
use std::cell::Cell;
use uzcoders::prelude::*;

pub const PAGE: usize = 20;

pub struct Feed {
    pub items: State<Vec<Video>>,
    pub loading: State<bool>,
    pub err: State<String>,
    pub more: State<bool>,
    /// Bir marta yuklash boshlanganmi (ekran birinchi ko'ringanda).
    pub asked: Cell<bool>,
}

impl Feed {
    pub fn new() -> Rc<Feed> {
        Rc::new(Feed { items: State::new(Vec::new()), loading: State::new(false), err: State::new(String::new()), more: State::new(false), asked: Cell::new(false) })
    }

    /// `q` — so'rov qismi (`sort=new&me=...`). `append` — keyingi sahifa.
    pub fn fetch(self: &Rc<Self>, q: String, append: bool) {
        if self.loading.get() {
            return;
        }
        self.asked.set(true);
        self.loading.set(true);
        let off = if append { self.items.with(|v| v.len()) } else { 0 };
        let me = self.clone();
        spawn_bg(
            move || net::call("GET", &format!("/api/videos?{q}&limit={PAGE}&offset={off}"), ""),
            move |r| {
                me.loading.set(false);
                match r {
                    Ok(j) => {
                        let list: Vec<Video> = j.get("videos").arr().iter().map(Video::from).collect();
                        me.more.set(list.len() >= PAGE);
                        me.err.set(String::new());
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

    /// Birinchi ko'rinishda bir marta yuklaydi.
    pub fn ensure(self: &Rc<Self>, q: String) {
        if !self.asked.get() {
            self.fetch(q, false);
        }
    }

    pub fn reset(&self) {
        self.asked.set(false);
    }
}
