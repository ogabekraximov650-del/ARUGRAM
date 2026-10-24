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
    Failed(Instant, u8),
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
    static IMG_SEEN: RefCell<HashMap<String, Arc<AtomicU64>>> = RefCell::new(HashMap::new());
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

fn sheet_path(key: &str) -> String {
    let d = format!("{}/tgsheets", crate::data::cache_dir());
    let _ = std::fs::create_dir_all(&d);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(key, &mut h);
    format!("{d}/{:016x}.bin", std::hash::Hasher::finish(&h))
}

/// Diskdagi tayyor varaq (shifrlangan): `AS1 | ustunlar | kadrlar | fps | PNG`.
fn sheet_disk_read(key: &str) -> Option<crate::tganim::AnimSheet> {
    let b = crate::secure::read(&sheet_path(key), "tgs")?;
    if b.len() < 15 || &b[..3] != b"AS1" {
        return None;
    }
    let u = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    let (cols, count, fps) = (u(3), u(7), f32::from_le_bytes([b[11], b[12], b[13], b[14]]));
    let img = uzcoders::prelude::decode_image(&b[15..])?;
    (cols > 0 && count > 0 && fps > 0.0).then_some(crate::tganim::AnimSheet { rgba: img.rgba, w: img.w, h: img.h, cols, count, fps })
}

fn sheet_disk_write(key: &str, s: &crate::tganim::AnimSheet) {
    let png = uzcoders::prelude::Image { w: s.w, h: s.h, rgba: s.rgba.clone() }.to_png();
    let mut b = Vec::with_capacity(png.len() + 15);
    b.extend_from_slice(b"AS1");
    b.extend_from_slice(&s.cols.to_le_bytes());
    b.extend_from_slice(&s.count.to_le_bytes());
    b.extend_from_slice(&s.fps.to_le_bytes());
    b.extend_from_slice(&png);
    let _ = crate::secure::write(&sheet_path(key), "tgs", &b);
}

impl Home {
    /// Telegram rasmi: tayyor bo'lsa qaytaradi, bo'lmasa fonda yuklaydi (tayyor bo'lgach oyna qayta chiziladi).
    pub(super) fn tg_img(self: &Rc<Self>, key: &str, load: impl FnOnce() -> Result<Image, String> + Send + 'static) -> Option<Rc<Image>> {
        let _ = self.gate_rev.get();
        let st = IMGS.with(|m| m.borrow().get(key).cloned());
        match st {
            Some(TImg::Ready(i)) => return Some(i),
            Some(TImg::Loading) => return None,
            // UZ: Xato bo'lsa 3, 6, 9 soniyadan keyin qayta uriniladi (jami 3 marta, Kotlindagidek).
            // EN: After a failure it retries after 3, 6, 9 seconds (3 times in total, like Kotlin).
            // RU: После ошибки повторяет через 3, 6, 9 секунд (всего 3 раза, как в Kotlin).
            Some(TImg::Failed(t, n)) if n >= 4 || t.elapsed().as_secs() < 3 * n as u64 => {
                if n < 4 {
                    let me = self.clone();
                    let wait = (3 * n as u64).saturating_sub(t.elapsed().as_secs()) + 1;
                    spawn_bg(move || std::thread::sleep(std::time::Duration::from_secs(wait)), move |_| me.gate_rev.set(me.gate_rev.get() + 1));
                }
                return None;
            }
            _ => {}
        }
        // UZ: Ko'rilgan vaqt har qurilishda yangilanadi; ekrandan chiqib ketgan katakning ishi boshlanmaydi/to'xtatiladi.
        // EN: The seen time is refreshed on every build; a job for a cell that left the screen does not start / is stopped.
        // RU: Время показа обновляется при каждой сборке; задача ушедшей с экрана ячейки не начинается / останавливается.
        let seen = IMG_SEEN.with(|m| m.borrow_mut().entry(key.to_string()).or_insert_with(|| Arc::new(AtomicU64::new(0))).clone());
        seen.store(epoch_ms(), Ordering::Relaxed);
        let tries = match st {
            Some(TImg::Failed(_, n)) => n,
            _ => 0,
        };
        IMGS.with(|m| m.borrow_mut().insert(key.to_string(), TImg::Loading));
        let (me, k) = (self.clone(), key.to_string());
        spawn_bg(
            move || {
                tgmedia::set_seen(Some(seen));
                let r = if tgmedia::cancelled() { Err("kerak emas".to_string()) } else { load() };
                tgmedia::set_seen(None);
                r
            },
            move |r| {
                match r {
                    Ok(img) => {
                        IMGS.with(|m| m.borrow_mut().insert(k, TImg::Ready(Rc::new(img))));
                    }
                    // Ekrandan chiqib ketgan: yozuv o'chiriladi, qaytib ko'ringanda qayta uriniladi.
                    Err(e) if e == "kerak emas" => {
                        IMGS.with(|m| m.borrow_mut().remove(&k));
                    }
                    Err(e) => {
                        uzcoders::prelude::report_error("rasm (Telegram)", &e);
                        IMGS.with(|m| m.borrow_mut().insert(k, TImg::Failed(Instant::now(), tries.saturating_add(1))));
                    }
                }
                me.gate_rev.set(me.gate_rev.get() + 1);
            },
        );
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
        let sheet = self.tg_sheet(&format!("ar{r}"), px, true, move |wanted| {
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
            if gif { crate::tganim::gif_sheet(&bytes, px, 36, 15.0) } else { crate::tganim::build_safe(&bytes, px, 36, 15.0, wanted) }
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
    pub(super) fn tg_sheet(self: &Rc<Self>, key: &str, px: u32, allow_build: bool, load: impl FnOnce(&dyn Fn() -> bool) -> Result<crate::tganim::AnimSheet, String> + Send + 'static) -> Option<ASheet> {
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
        // UZ: Bir vaqtda ko'pi bilan 3 ta varaq tayyorlanadi; ortiqchasi oqimlar pulini band qilmaydi, keyingi chizishda qayta so'raladi.
        // EN: At most 3 sheets are built at once; extra ones do not occupy the thread pool and are requested again on the next draw.
        // RU: Одновременно строится не более 3 листов; лишние не занимают пул потоков и запрашиваются снова при следующей отрисовке.
        if ANIMS.with(|m| m.borrow().values().filter(|e| matches!(e.st, AState::Loading)).count()) >= 3 {
            return None;
        }
        let disk_key = format!("{key}_{px}");
        // UZ: O'ynamayotgan katak yangi varaq yasamaydi (og'ir): faqat diskdagi tayyorini oladi.
        // EN: A cell that is not playing does not build a new sheet (heavy): it only picks up a ready one from disk.
        // RU: Не играющая ячейка не строит новый лист (это тяжело): берёт только готовый с диска.
        if !allow_build && !std::path::Path::new(&sheet_path(&disk_key)).exists() {
            return None;
        }
        let seen = Arc::new(AtomicU64::new(now));
        let seen2 = seen.clone();
        ANIMS.with(|m| m.borrow_mut().insert(key.to_string(), AEntry { st: AState::Loading, seen: seen.clone(), last_use: Instant::now() }));
        let (me, k) = (self.clone(), key.to_string());
        spawn_bg(
            move || {
                // UZ: Tayyor varaq diskda: qayta chizilmaydi/dekodlanmaydi, faqat bitta PNG o'qiladi.
                // EN: A ready sheet is on disk: it is not re-rendered/re-decoded, only one PNG is read.
                // RU: Готовый лист на диске: не рисуется/не декодируется заново, читается только один PNG.
                if let Some(s) = sheet_disk_read(&disk_key) {
                    return Some(Ok(s));
                }
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
                tgmedia::set_seen(Some(seen2));
                let (vm, rss) = uzcoders::prelude::memory_mb();
                uzcoders::prelude::breadcrumb(&format!("animatsiya varag'i boshlandi (xotira {vm}/{rss} MB)"));
                let r = load(&wanted);
                tgmedia::set_seen(None);
                BUSY.fetch_sub(1, Ordering::SeqCst);
                if let Ok(s) = &r {
                    sheet_disk_write(&disk_key, s);
                }
                let (vm, rss) = uzcoders::prelude::memory_mb();
                uzcoders::prelude::breadcrumb(match &r {
                    Ok(s) => format!("animatsiya varag'i tayyor {}x{} (xotira {vm}/{rss} MB)", s.w, s.h),
                    Err(e) => format!("animatsiya varag'i xato: {e}"),
                }.as_str());
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
                        Some(Err(err)) => {
                            if err != "kerak emas" && err != "animatsiyasiz" {
                                uzcoders::prelude::report_error("animatsiya", &err);
                            }
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
        let sheet = self.tg_sheet(&format!("a{key}"), px, playing, move |wanted| {
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
            if gif { crate::tganim::gif_sheet(&bytes, px, frames, fps) } else { crate::tganim::build_safe(&bytes, px, frames, fps, wanted) }
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
