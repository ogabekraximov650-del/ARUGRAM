//! Kirish oynalari (Kotlin `AddAccountScreen` + `BotLoginScreen` bilan bir xil): avval usulni tanlash (Telegram / Google),
//! Telegram — bot orqali (worker bir martalik token beradi, foydalanuvchi botda «Start» bosadi, ilova holatni so'raydi).
//! UZ: telefon raqami, kod, QR va MTProto YO'Q — Telegram'ga faqat worker orqali. EN: no phone/code/QR, the Telegram bot flow runs through the worker.

use crate::backend::{Backend, Poll};
use crate::theme::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uzcoders::prelude::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Stage {
    /// Kirish usulini tanlash.
    Choose,
    /// Worker'dan token olinmoqda.
    Preparing,
    /// Telegram botida «Start» kutilmoqda.
    Waiting,
    Expired,
    Error,
}

const TG_PATH: &str = "M9.04 15.6l-.38 4.1c.55 0 .79-.24 1.08-.52l2.6-2.48 5.38 3.94c.99.55 1.69.26 1.96-.91l3.55-16.6c.32-1.46-.53-2.03-1.5-1.67L1.2 10.6c-1.42.55-1.4 1.35-.24 1.7l5.35 1.67 12.42-7.83c.58-.38 1.12-.17.68.21z";
// Google logotipi: asl 48x48 `viewBox`; `icon()` 24x24 kutadi, shu sabab raqamlar 0,5 ga ko'paytiriladi.
const G_RED: &str = "M24 9.5c3.54 0 6.71 1.22 9.21 3.6l6.85-6.85C35.9 2.38 30.47 0 24 0 14.62 0 6.51 5.38 2.56 13.22l7.98 6.19C12.43 13.72 17.74 9.5 24 9.5z";
const G_BLUE: &str = "M46.98 24.55c0-1.57-.15-3.09-.38-4.55H24v9.02h12.94c-.58 2.96-2.26 5.48-4.78 7.18l7.73 6c4.51-4.18 7.09-10.36 7.09-17.65z";
const G_YELLOW: &str = "M10.53 28.59c-.48-1.45-.76-2.99-.76-4.59s.27-3.14.76-4.59l-7.98-6.19C.92 16.46 0 20.12 0 24c0 3.88.92 7.54 2.56 10.78l7.97-6.19z";
const G_GREEN: &str = "M24 48c6.48 0 11.93-2.13 15.89-5.81l-7.73-6c-2.15 1.45-4.92 2.3-8.16 2.3-6.26 0-11.57-4.22-13.47-9.91l-7.98 6.19C6.51 42.62 14.62 48 24 48z";

/// SVG yo'lidagi hamma sonlarni `k` marta kichraytiradi (faqat M/L/H/V/C/S/Q/T/Z va ularning kichik harfli variantlari — yoy yo'q).
fn scale_path(d: &str, k: f32) -> String {
    let mut out = String::new();
    let mut num = String::new();
    let flush = |num: &mut String, out: &mut String| {
        if !num.is_empty() {
            if let Ok(v) = num.parse::<f32>() {
                out.push_str(&format!("{:.3}", v * k));
            } else {
                out.push_str(num);
            }
            num.clear();
        }
    };
    let mut prev_e = false;
    for ch in d.chars() {
        let part = ch.is_ascii_digit() || ch == '.' || (ch == '-' && (num.is_empty() || prev_e)) || ((ch == 'e' || ch == 'E') && !num.is_empty());
        if ch == '-' && !num.is_empty() && !prev_e {
            flush(&mut num, &mut out);
            num.push(ch);
        } else if part {
            if ch == '.' && num.contains('.') {
                flush(&mut num, &mut out);
            }
            num.push(ch);
        } else {
            flush(&mut num, &mut out);
            out.push(ch);
        }
        prev_e = ch == 'e' || ch == 'E';
    }
    flush(&mut num, &mut out);
    out
}

fn google_paths() -> &'static [&'static str; 4] {
    static P: std::sync::OnceLock<[&'static str; 4]> = std::sync::OnceLock::new();
    P.get_or_init(|| {
        let leak = |s: &str| -> &'static str { Box::leak(scale_path(s, 0.5).into_boxed_str()) };
        [leak(G_RED), leak(G_BLUE), leak(G_YELLOW), leak(G_GREEN)]
    })
}

fn tx(s: impl Into<String>, size: f32, color: Color) -> Element {
    text(s).size(size).color(color).line_height(size * 1.3)
}

/// Kirish oynasi holati.
pub struct Login {
    stage: State<Stage>,
    message: State<String>,
    left: State<i64>,
    link: RefCell<String>,
    token: RefCell<String>,
    /// Har urinish yangi raqam oladi: eski so'rovlar natijasi e'tiborga olinmaydi (Orqaga / qayta urinish).
    gen: Cell<u32>,
    until: Cell<Option<Instant>>,
    backend: Arc<dyn Backend>,
    on_done: Rc<dyn Fn(String)>,
    /// «Qaysi Telegram orqali kirasiz?» oynasi ochiqmi.
    picker: State<bool>,
    /// Hozirgi urinish Google orqalimi (aks holda Telegram bot).
    google: Cell<bool>,
    /// O'rnatilgan Telegram ilovalari: (paket, nom, ikon).
    clients: RefCell<Vec<(String, String, Option<Rc<Image>>)>>,
}

impl Login {
    pub fn new(backend: Arc<dyn Backend>, on_done: Rc<dyn Fn(String)>) -> Rc<Login> {
        Rc::new(Login {
            stage: State::new(Stage::Choose),
            message: State::new(String::new()),
            left: State::new(0),
            link: RefCell::new(String::new()),
            token: RefCell::new(String::new()),
            gen: Cell::new(0),
            until: Cell::new(None),
            backend,
            on_done,
            picker: State::new(false),
            google: Cell::new(false),
            clients: RefCell::new(Vec::new()),
        })
    }

    /// Telefondagi Telegram klientlarini o'qiydi (Java `tgClients`).
    fn load_clients(&self) {
        let raw = uzcoders::android::activity_call_ret("tgClients", "");
        let list = raw
            .lines()
            .filter_map(|l| {
                let mut p = l.split('\t');
                let (pkg, label, icon) = (p.next()?.to_string(), p.next()?.to_string(), p.next().unwrap_or(""));
                if pkg.is_empty() {
                    return None;
                }
                let img = if icon.is_empty() { None } else { std::fs::read(icon).ok().and_then(|b| crate::webp::decode_any(&b)).map(Rc::new) };
                Some((pkg, label, img))
            })
            .collect();
        *self.clients.borrow_mut() = list;
    }

    /// Tanlangan Telegram ilovasida botni ochadi (`None` — tizim o'zi tanlaydi).
    fn open_with(&self, pkg: Option<&str>) {
        self.picker.set(false);
        let link = self.link.borrow().clone();
        uzcoders::android::activity_call("tgOpen", &format!("{}|{}", pkg.unwrap_or(""), link));
    }

    /// Kotlin `TgClientPicker` (Material `AlertDialog`).
    fn picker_view(self: &Rc<Self>) -> Element {
        let clients = self.clients.borrow().clone();
        let mut kids = vec![tx("Qaysi Telegram orqali kirasiz?", 17.0, Color::WHITE), gap_box(0.0, 16.0)];
        if clients.is_empty() {
            kids.push(tx("Telegram ilovasi topilmadi. Havola brauzerda ochiladi.", 13.0, TEXT_DIM));
            kids.push(gap_box(0.0, 12.0));
            let me = self.clone();
            kids.push(column(vec![tx("Brauzerda ochish", 14.0, ACCENT).bold()]).center().padding_xy(12.0, 10.0).on_click(move || {
                me.picker.set(false);
                uzcoders::android::open_url(&me.link.borrow());
            }));
        }
        for (pkg, label, icon) in clients {
            let me = self.clone();
            let logo = match icon {
                Some(img) => image(&img, 36.0, 36.0),
                None => column(vec![]).width(36.0).height(36.0).radius(18.0).bg(Color::hex(0x229ED9)),
            };
            kids.push(row(vec![logo, gap_box(12.0, 0.0), tx(label, 15.0, Color::WHITE)]).align(Align::Center).padding_xy(0.0, 10.0).on_click(move || me.open_with(Some(&pkg))));
        }
        let me = self.clone();
        kids.push(gap_box(0.0, 12.0));
        kids.push(row(vec![spacer(), column(vec![tx("Bekor qilish", 14.0, TEXT_DIM).bold()]).center().padding_xy(12.0, 10.0).on_click(move || me.picker.set(false))]));
        let panel = column(kids).bg(SURFACE).radius(28.0).padding(24.0).margin_each(24.0, 0.0, 24.0, 0.0);
        let me = self.clone();
        column(vec![panel]).center().bg(Color::rgba(0, 0, 0, 82)).on_click(move || me.picker.set(false))
    }

    /// "Orqaga": bot oynasidan usul tanlashga qaytadi; tanlash oynasida — ilovadan chiqish tizimga qoladi.
    pub fn back(&self) -> bool {
        if self.stage.get() == Stage::Choose {
            return false;
        }
        self.restart();
        true
    }

    /// Boshidan (usul tanlash).
    pub fn restart(&self) {
        self.google.set(false);
        self.gen.set(self.gen.get().wrapping_add(1));
        self.stage.set(Stage::Choose);
        self.message.set(String::new());
        self.until.set(None);
    }

    /// Google orqali: worker havola beradi, u brauzerda ochiladi (Google akkaunt tanlanadi), ilova holatni so'rab turadi.
    fn start_google(self: &Rc<Self>) {
        self.google.set(true);
        let g = self.gen.get().wrapping_add(1);
        self.gen.set(g);
        self.stage.set(Stage::Preparing);
        self.message.set(String::new());
        let (b, me) = (self.backend.clone(), self.clone());
        spawn_bg(move || b.google_start(), move |r| {
            if me.gen.get() != g {
                return;
            }
            match r {
                Ok(l) => {
                    *me.link.borrow_mut() = l.link.clone();
                    *me.token.borrow_mut() = l.token.clone();
                    me.until.set(Some(Instant::now() + Duration::from_secs(l.expires.max(30))));
                    me.left.set(l.expires as i64);
                    me.stage.set(Stage::Waiting);
                    uzcoders::android::open_url(&l.link);
                    me.poll(g);
                }
                Err(e) => {
                    me.message.set(e);
                    me.stage.set(Stage::Error);
                }
            }
        });
    }

    fn start_bot(self: &Rc<Self>) {
        self.google.set(false);
        let g = self.gen.get().wrapping_add(1);
        self.gen.set(g);
        self.stage.set(Stage::Preparing);
        self.message.set(String::new());
        let (b, me) = (self.backend.clone(), self.clone());
        spawn_bg(move || b.start(), move |r| {
            if me.gen.get() != g {
                return;
            }
            match r {
                Ok(l) => {
                    *me.link.borrow_mut() = l.link.clone();
                    *me.token.borrow_mut() = l.token.clone();
                    me.until.set(Some(Instant::now() + Duration::from_secs(l.expires.max(30))));
                    me.left.set(l.expires as i64);
                    me.stage.set(Stage::Waiting);
                    // Kotlin: avval «Qaysi Telegram orqali kirasiz?» oynasi (o'rnatilgan klientlar), keyin botda «Start».
                    me.load_clients();
                    me.picker.set(true);
                    me.poll(g);
                }
                Err(e) => {
                    me.message.set(e);
                    me.stage.set(Stage::Error);
                }
            }
        });
    }

    /// Holatni har ~0,8 soniyada so'raydi (Kotlin bilan bir xil).
    fn poll(self: &Rc<Self>, g: u32) {
        let (b, me, token) = (self.backend.clone(), self.clone(), self.token.borrow().clone());
        spawn_bg(move || {
            std::thread::sleep(Duration::from_millis(800));
            b.poll(&token)
        }, move |p| {
            if me.gen.get() != g || me.stage.get() != Stage::Waiting {
                return;
            }
            match p {
                Poll::Ok(session) => {
                    crate::api::save_session(&session);
                    (me.on_done)(session);
                }
                Poll::Expired => me.stage.set(Stage::Expired),
                Poll::Banned(m) => {
                    me.message.set(m);
                    me.stage.set(Stage::Error);
                }
                Poll::Error(e) => {
                    me.message.set(e);
                    me.stage.set(Stage::Error);
                }
                Poll::Pending => {
                    let left = me.until.get().map_or(0, |t| t.saturating_duration_since(Instant::now()).as_secs() as i64 + 1);
                    me.left.set(left);
                    if left <= 0 {
                        me.stage.set(Stage::Expired);
                    } else {
                        me.poll(g);
                    }
                }
            }
        });
    }

    pub fn view(self: &Rc<Self>) -> Element {
        let content = match self.stage.get() {
            Stage::Choose => self.choose_view(),
            _ => self.bot_view(),
        };
        let mut layers = vec![column(vec![]).bg(BG), content];
        if self.stage.get() == Stage::Waiting && self.picker.get() {
            layers.push(self.picker_view());
        }
        stack(layers)
    }

    /// Kapsula tugma (Kotlin `Choice`): 58 dp balandlik, to'liq yumaloq, chapda dumaloq logotip, matn o'rtada.
    fn choice(label: &str, logo: Element, f: impl Fn() + 'static) -> Element {
        let (w, _) = window_dp();
        stack(vec![
            row(vec![gap_box(7.0, 0.0), logo]).align(Align::Center),
            column(vec![tx(label, 16.0, Color::WHITE).bold()]).center(),
        ])
        .width((w - 48.0).max(200.0))
        .height(58.0)
        .bg(ACCENT)
        .radius(29.0)
        .on_click(f)
    }

    fn logo_circle(bg: Color, inner: Element) -> Element {
        column(vec![inner]).center().width(44.0).height(44.0).radius(22.0).bg(bg)
    }

    fn choose_view(self: &Rc<Self>) -> Element {
        let me = self.clone();
        let tg = Self::logo_circle(Color::hex(0x229ED9), icon(TG_PATH, 26.0).color(Color::WHITE));
        let g = google_paths();
        let google = Self::logo_circle(
            Color::WHITE,
            stack(vec![
                icon(g[0], 26.0).color(Color::hex(0xEA4335)),
                icon(g[1], 26.0).color(Color::hex(0x4285F4)),
                icon(g[2], 26.0).color(Color::hex(0xFBBC05)),
                icon(g[3], 26.0).color(Color::hex(0x34A853)),
            ])
            .width(26.0)
            .height(26.0),
        );
        let m2 = self.clone();
        let mut kids = vec![
            tx("Accountga kirish", 22.0, Color::WHITE).bold(),
            gap_box(0.0, 8.0),
            tx("Kirish usulini tanlang", 14.0, Color::rgba(255, 255, 255, 140)),
            gap_box(0.0, 28.0),
            Self::choice("Telegram orqali", tg, move || me.start_bot()),
            gap_box(0.0, 12.0),
            Self::choice("Google orqali", google, move || m2.start_google()),
        ];
        let msg = self.message.get();
        if !msg.is_empty() {
            kids.push(gap_box(0.0, 16.0));
            kids.push(tx(msg, 13.0, Color::hex(0xF87171)));
        }
        column(kids).center().padding(24.0)
    }

    fn retry_btn(self: &Rc<Self>) -> Element {
        let me = self.clone();
        column(vec![tx("Qayta urinish", 15.0, Color::WHITE).bold()]).center().bg(ACCENT).radius(14.0).padding_xy(24.0, 12.0).on_click(move || if me.google.get() { me.start_google() } else { me.start_bot() })
    }

    fn bot_view(self: &Rc<Self>) -> Element {
        let google = self.google.get();
        let mut kids = vec![tx(if google { "Google orqali kirish" } else { "Telegram orqali kirish" }, 22.0, Color::WHITE).bold(), gap_box(0.0, 10.0)];
        match self.stage.get() {
            Stage::Preparing | Stage::Choose => kids.push(spinner(32.0, 2.0, ACCENT)),
            Stage::Waiting => {
                let me = self.clone();
                let (w, _) = window_dp();
                let cw = (w - 48.0).max(200.0);
                if google {
                    kids.push(tx("Brauzerda Google accountingizni tanlang. Tasdiqlangach ilova o'zi kiradi.", 14.0, TEXT_DIM).width(cw));
                    kids.push(gap_box(0.0, 20.0));
                    kids.push(column(vec![tx("Brauzerda qayta ochish", 15.0, Color::WHITE).bold()]).center().width(cw).bg(ACCENT).radius(16.0).padding_xy(0.0, 15.0).on_click(move || {
                        let u = me.link.borrow().clone();
                        uzcoders::android::open_url(&u);
                    }));
                    kids.push(gap_box(0.0, 10.0));
                    kids.push(tx(format!("Havola muddati: {} s", self.left.get().max(0)), 12.0, TEXT_FAINT));
                } else {
                kids.push(tx("Telegram ilovasini tanlang, botda «Start» tugmasini bosib hisobingizni tasdiqlang. Tasdiqlangach ilova o'zi kiradi.", 14.0, TEXT_DIM).width(cw));
                kids.push(gap_box(0.0, 20.0));
                kids.push(
                    column(vec![tx("Telegram ilovasini tanlash", 15.0, Color::WHITE).bold()])
                        .center()
                        .width(cw)
                        .bg(ACCENT)
                        .radius(16.0)
                        .padding_xy(0.0, 15.0)
                        .on_click(move || {
                            me.load_clients();
                            me.picker.set(true);
                        }),
                );
                kids.push(gap_box(0.0, 10.0));
                kids.push(tx(format!("Havola muddati: {} s", self.left.get().max(0)), 12.0, TEXT_FAINT));
                }
            }
            Stage::Expired => {
                kids.push(tx("Havola muddati tugadi", 14.0, TEXT_DIM));
                kids.push(gap_box(0.0, 14.0));
                kids.push(self.retry_btn());
            }
            Stage::Error => {
                kids.push(tx(self.message.get(), 14.0, Color::hex(0xF87171)));
                kids.push(gap_box(0.0, 14.0));
                kids.push(self.retry_btn());
            }
        }
        let me = self.clone();
        kids.push(gap_box(0.0, 24.0));
        kids.push(tx("Orqaga", 14.0, TEXT_DIM).padding_xy(10.0, 10.0).on_click(move || {
            me.restart();
        }));
        column(kids).center().padding(24.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yol_kichraytiriladi() {
        assert_eq!(scale_path("M24 9.5c3.54-1", 0.5), "M12.000 4.750c1.770-0.500");
    }
}
