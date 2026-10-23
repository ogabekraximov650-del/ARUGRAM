//! UZ: Telegram rasmlari va animatsiyalari keshi: statik rasm (`tg_img`), `.tgs` animatsiya varaqlari (`tg_sheet`, `tg_anim_el`) va chatdagi stiker/GIF havolalari (`tg_ref_el`).
//! Hammasi fonda tayyorlanadi, tayyor bo'lgach oyna qayta chiziladi; xotira chegaralangan (eng eski ishlatilgani chiqariladi).
//! EN: A cache of Telegram images and animations: static images (`tg_img`), `.tgs` animation sheets (`tg_sheet`, `tg_anim_el`) and chat sticker/GIF refs (`tg_ref_el`).
//! Everything is prepared in the background and the window is redrawn when ready; memory is bounded (the least recently used is evicted).
//! RU: Кэш изображений и анимаций Telegram: статичные картинки (`tg_img`), листы `.tgs`-анимаций (`tg_sheet`, `tg_anim_el`) и ссылки на стикеры/GIF в чате (`tg_ref_el`).
//! Всё готовится в фоне, окно перерисовывается по готовности; память ограничена (вытесняется наименее используемое).

use super::*;
use crate::tgmedia::{self, TgDoc, TgSet};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub(super) enum TImg {
    Loading,
    Ready(Rc<Image>),
    Failed(Instant),
}

/// Tayyor animatsiya varag'i (UI oqimida): rasm, ustunlar, kadrlar soni, tezlik.
#[derive(Clone)]
pub(super) struct ASheet {
    img: Rc<Image>,
    cols: u32,
    count: u32,
    fps: f32,
}

enum AState {
    Loading,
    Ready(ASheet),
    Bad,
}

struct AEntry {
    st: AState,
    seen: Arc<AtomicU64>,
    last_use: Instant,
}

/// Xotirada ushlanadigan animatsiya varaqlarining umumiy chegarasi (bayt).
const ANIM_BUDGET: usize = 40 << 20;

fn epoch_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

thread_local! {
    /// Chatda animatsiya o'ynaydigan stikerli xabarlar (eng oxirgi bir nechtasi: ular ekranda ko'rinadi).
    static CHAT_ANIM: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
    static ANIMS: RefCell<HashMap<String, AEntry>> = RefCell::new(HashMap::new());
    static IMGS: RefCell<HashMap<String, TImg>> = RefCell::new(HashMap::new());
    static RECENT: RefCell<Vec<String>> = RefCell::new(Vec::new());
}

/// Chatdagi stikerli xabarlardan eng oxirgi `n` tasini animatsiya uchun belgilaydi.
pub(super) fn chat_anim_mark(ids: impl Iterator<Item = String>, n: usize) {
    CHAT_ANIM.with(|c| *c.borrow_mut() = ids.take(n).collect());
}

pub(super) fn chat_anim_ok(id: &str) -> bool {
    CHAT_ANIM.with(|c| c.borrow().contains(id))
}

/// Hujjat rasmini yuklaydi; havola (`file_reference`) eskirsa ro'yxatni yangilab, qayta uriniladi.
pub(super) fn load_doc(set: Option<TgSet>, gifs: bool, d: TgDoc, px: u32) -> Result<Image, String> {
    match tgmedia::image(&d, px) {
        Err(e) if e.contains("FILE_REFERENCE") => {
            let fresh = if gifs { tgmedia::gifs(true) } else { set.map(|s| tgmedia::set_docs(&s, true)).unwrap_or_default() };
            match fresh.into_iter().find(|x| x.id == d.id) {
                Some(nd) => tgmedia::image(&nd, px),
                None => Err(e),
            }
        }
        r => r,
    }
}

impl Home {
    /// Telegram rasmi: tayyor bo'lsa qaytaradi, bo'lmasa fonda yuklaydi (tayyor bo'lgach oyna qayta chiziladi).
    pub(super) fn tg_img(self: &Rc<Self>, key: &str, load: impl FnOnce() -> Result<Image, String> + Send + 'static) -> Option<Rc<Image>> {
        let _ = self.gate_rev.get();
        let st = IMGS.with(|m| m.borrow().get(key).cloned());
        match st {
            Some(TImg::Ready(i)) => return Some(i),
            Some(TImg::Loading) => return None,
            Some(TImg::Failed(t)) if t.elapsed().as_secs() < 20 => return None,
            _ => {}
        }
        IMGS.with(|m| m.borrow_mut().insert(key.to_string(), TImg::Loading));
        let (me, k) = (self.clone(), key.to_string());
        spawn_bg(load, move |r| {
            let st = match r {
                Ok(img) => TImg::Ready(Rc::new(img)),
                Err(_) => TImg::Failed(Instant::now()),
            };
            IMGS.with(|m| m.borrow_mut().insert(k, st));
            me.gate_rev.set(me.gate_rev.get() + 1);
        });
        None
    }

    /// Chatdagi stiker/GIF havolasi (`tgs.…` / `tgg.…`) -> rasm. Tayyor bo'lguncha bo'sh joy. `anim` — `.tgs` bo'lsa animatsiya ham tayyorlansin va o'ynasin.
    pub(super) fn tg_ref_el(self: &Rc<Self>, r: &str, size: f32, anim: bool) -> Option<Element> {
        let sticker = r.starts_with("tgs.");
        if !sticker && !r.starts_with("tgg.") {
            return None;
        }
        let rr = r.to_string();
        let img = self.tg_img(&format!("ref_{r}"), move || {
            let d = if sticker { tgmedia::resolve_sticker(&rr) } else { tgmedia::resolve_gif(&rr) }.ok_or_else(|| "havola hal bo'lmadi".to_string())?;
            tgmedia::image(&d, 512)
        });
        let still = match img {
            Some(i) => {
                let k = (size / i.w as f32).min(size / i.h as f32);
                image(&i, i.w as f32 * k, i.h as f32 * k).radius(if sticker { 0.0 } else { 14.0 })
            }
            None => return Some(column(vec![spinner(22.0, 2.0, Color::WHITE)]).center().width(size).height(size)),
        };
        if !anim {
            return Some(still);
        }
        let rr = r.to_string();
        let px = (size * density()).clamp(96.0, 224.0) as u32;
        let sheet = self.tg_sheet(&format!("ar{r}"), move |wanted| {
            let d = if sticker { tgmedia::resolve_sticker(&rr) } else { tgmedia::resolve_gif(&rr) }.ok_or_else(|| "havola hal bo'lmadi".to_string())?;
            let gif = !sticker;
            if !gif && d.kind != "tgs" {
                return Err("animatsiyasiz".into());
            }
            let max = if gif { 6 << 20 } else { 1 << 20 };
            let bytes = match tgmedia::file_bytes(&d, max) {
                Err(e) if e.contains("FILE_REFERENCE") && sticker => {
                    let nd = tgmedia::resolve_sticker_fresh(&rr).ok_or(e)?;
                    tgmedia::file_bytes(&nd, max)?
                }
                r => r?,
            };
            if !wanted() {
                return Err("kerak emas".into());
            }
            if gif { crate::tganim::gif_sheet(&bytes, px, 36, 15.0) } else { crate::tganim::build_safe(&bytes, px, 48, 24.0) }
        });
        Some(match sheet {
            Some(s) => {
                // UZ: GIF kadrlari asl nisbatda; stiker kvadrat. EN: GIF frames keep their aspect; a sticker is square. RU: Кадры GIF в исходной пропорции; стикер квадратный.
                let (cw, ch) = (s.img.w / s.cols, s.img.h / s.count.div_ceil(s.cols).max(1));
                let k = (size / cw.max(1) as f32).min(size / ch.max(1) as f32);
                sprite(&s.img, s.cols, s.count, s.fps, true, cw as f32 * k, ch as f32 * k).radius(if sticker { 0.0 } else { 14.0 })
            }
            None => still,
        })
    }

    /// Animatsiya varag'i: tayyor bo'lsa qaytaradi; bo'lmasa fonda tayyorlaydi (bir vaqtda 2 ta; ekrandan ketgan katak uchun ish tashlab yuboriladi).
    /// `load(kerakmi)` — varaqni yasaydi; `kerakmi()` yolg'on bo'lsa, ish to'xtatiladi.
    pub(super) fn tg_sheet(self: &Rc<Self>, key: &str, load: impl FnOnce(&dyn Fn() -> bool) -> Result<crate::tganim::AnimSheet, String> + Send + 'static) -> Option<ASheet> {
        let _ = self.gate_rev.get();
        let now = epoch_ms();
        let state = ANIMS.with(|m| {
            let mut m = m.borrow_mut();
            let e = m.get_mut(key)?;
            e.seen.store(now, Ordering::Relaxed);
            e.last_use = Instant::now();
            Some(match &e.st {
                AState::Ready(s) => Some(s.clone()),
                _ => None,
            })
        });
        if let Some(r) = state {
            return r;
        }
        let seen = Arc::new(AtomicU64::new(now));
        ANIMS.with(|m| m.borrow_mut().insert(key.to_string(), AEntry { st: AState::Loading, seen: seen.clone(), last_use: Instant::now() }));
        let (me, k) = (self.clone(), key.to_string());
        spawn_bg(
            move || {
                static BUSY: AtomicUsize = AtomicUsize::new(0);
                let wanted = || epoch_ms().saturating_sub(seen.load(Ordering::Relaxed)) < 4000;
                loop {
                    if !wanted() {
                        return None;
                    }
                    let n = BUSY.load(Ordering::SeqCst);
                    if n < 2 && BUSY.compare_exchange(n, n + 1, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(80));
                }
                let r = load(&wanted);
                BUSY.fetch_sub(1, Ordering::SeqCst);
                Some(r)
            },
            move |r| {
                ANIMS.with(|m| {
                    let mut m = m.borrow_mut();
                    match r {
                        // Ekrandan ketgan: yozuv o'chiriladi, qaytib ko'ringanda qayta uriniladi.
                        None => {
                            m.remove(&k);
                        }
                        Some(Ok(sh)) => {
                            let img = Rc::new(Image { w: sh.w, h: sh.h, rgba: sh.rgba });
                            if let Some(e) = m.get_mut(&k) {
                                e.st = AState::Ready(ASheet { img, cols: sh.cols, count: sh.count, fps: sh.fps });
                            }
                        }
                        Some(Err(_)) => {
                            if let Some(e) = m.get_mut(&k) {
                                e.st = AState::Bad;
                            }
                        }
                    }
                    // Xotira chegarasi: eng eski ishlatilganlari tashlanadi.
                    let bytes = |e: &AEntry| match &e.st {
                        AState::Ready(s) => (s.img.w * s.img.h * 4) as usize,
                        _ => 0,
                    };
                    let mut total: usize = m.values().map(bytes).sum();
                    while total > ANIM_BUDGET {
                        let victim = m.iter().filter(|(_, e)| matches!(e.st, AState::Ready(_))).min_by_key(|(_, e)| e.last_use).map(|(k, _)| k.clone());
                        let Some(v) = victim else { break };
                        if let Some(e) = m.remove(&v) {
                            total -= bytes(&e);
                        }
                    }
                });
                me.gate_rev.set(me.gate_rev.get() + 1);
            },
        );
        None
    }

    /// `.tgs` stikerning animatsiyali ko'rinishi (tayyor bo'lsa), aks holda `still` (statik rasm). `playing` — hozir o'ynasinmi.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn tg_anim_el(self: &Rc<Self>, key: &str, d: &TgDoc, set: Option<TgSet>, px: u32, frames: u32, fps: f32, w: f32, h: f32, playing: bool, still: Element) -> Element {
        let gif = d.kind == "mp4";
        if d.kind != "tgs" && !gif {
            return still;
        }
        let max = if gif { 6 << 20 } else { 1 << 20 };
        let dd = d.clone();
        let sheet = self.tg_sheet(&format!("a{key}"), move |wanted| {
            let bytes = match tgmedia::file_bytes(&dd, max) {
                Err(e) if e.contains("FILE_REFERENCE") => {
                    let fresh = match &set {
                        Some(s) => tgmedia::set_docs(s, true),
                        None => tgmedia::gifs(true),
                    };
                    let nd = fresh.into_iter().find(|x| x.id == dd.id).ok_or(e)?;
                    tgmedia::file_bytes(&nd, max)?
                }
                r => r?,
            };
            if !wanted() {
                return Err("kerak emas".into());
            }
            if gif { crate::tganim::gif_sheet(&bytes, px, frames, fps) } else { crate::tganim::build_safe(&bytes, px, frames, fps) }
        });
        match sheet {
            Some(s) => {
                // Kadr nisbati saqlanadi (GIF kvadrat emas): berilgan quti ichiga sig'diriladi.
                let (cw, ch) = (s.img.w / s.cols, s.img.h / s.count.div_ceil(s.cols).max(1));
                let k = (w / cw.max(1) as f32).min(h / ch.max(1) as f32);
                sprite(&s.img, s.cols, s.count, s.fps, playing, cw as f32 * k, ch as f32 * k)
            }
            None => still,
        }
    }
}
