//! Profil sahifasi (Kotlin `ProfileScreen`): hisob kartasi, statistika, bo'limlar va chiqish — hammasi lupali shisha, orqasida rangli fon.

use super::*;
use crate::data::{delete_account, fetch_profile, format_bytes, format_count, format_hours, Profile};

const P_MOVIE: &str = "M18,4l2,4h-3l-2,-4h-2l2,4h-3l-2,-4H8l2,4H7L5,4H4c-1.1,0 -1.99,0.9 -1.99,2L2,18c0,1.1 0.9,2 2,2h16c1.1,0 2,-0.9 2,-2V4h-4z";
const P_PLAY: &str = "M10,16.5l6,-4.5 -6,-4.5v9zM12,2C6.48,2 2,6.48 2,12s4.48,10 10,10 10,-4.48 10,-10S17.52,2 12,2zM12,20c-4.41,0 -8,-3.59 -8,-8s3.59,-8 8,-8 8,3.59 8,8 -3.59,8 -8,8z";
const P_GRID: &str = "M3,13h8L11,3L3,3v10zM3,21h8v-6L3,15v6zM13,21h8L21,11h-8v10zM13,3v6h8L21,3h-8z";
const P_BOOKMARK: &str = "M17,3H7c-1.1,0 -1.99,0.9 -1.99,2L5,21l7,-3 7,3V5c0,-1.1 -0.9,-2 -2,-2z";
const P_COMMENT: &str = "M21.99,4c0,-1.1 -0.89,-2 -1.99,-2H4c-1.1,0 -2,0.9 -2,2v12c0,1.1 0.9,2 2,2h14l4,4 -0.01,-18z";
const P_CLOCK: &str = "M11.99,2C6.47,2 2,6.48 2,12s4.47,10 9.99,10C17.52,22 22,17.52 22,12S17.52,2 11.99,2zM12,20c-4.42,0 -8,-3.58 -8,-8s3.58,-8 8,-8 8,3.58 8,8 -3.58,8 -8,8zM12.5,7H11v6l5.25,3.15 0.75,-1.23 -4.5,-2.67z";
const P_SD: &str = "M18,2h-8L4.02,8 4,20c0,1.1 0.9,2 2,2h12c1.1,0 2,-0.9 2,-2V4c0,-1.1 -0.9,-2 -2,-2zM12,8h-2V4h2v4zM15,8h-2V4h2v4zM18,8h-2V4h2v4z";
const P_CLOUD: &str = "M19.35,10.04C18.67,6.59 15.64,4 12,4 9.11,4 6.6,5.64 5.35,8.04 2.34,8.36 0,10.91 0,14c0,3.31 2.69,6 6,6h13c2.76,0 5,-2.24 5,-5 0,-2.64 -2.05,-4.78 -4.65,-4.96zM17,13l-5,5 -5,-5h3V9h4v4h3z";
const P_GEAR: &str = "M19.14,12.94c0.04,-0.3 0.06,-0.61 0.06,-0.94 0,-0.32 -0.02,-0.64 -0.07,-0.94l2.03,-1.58c0.18,-0.14 0.23,-0.41 0.12,-0.61l-1.92,-3.32c-0.12,-0.22 -0.37,-0.29 -0.59,-0.22l-2.39,0.96c-0.5,-0.38 -1.03,-0.7 -1.62,-0.94L14.4,2.81c-0.04,-0.24 -0.24,-0.41 -0.48,-0.41h-3.84c-0.24,0 -0.43,0.17 -0.47,0.41L9.25,5.35C8.66,5.59 8.12,5.92 7.63,6.29L5.24,5.33c-0.22,-0.08 -0.47,0 -0.59,0.22L2.74,8.87C2.62,9.08 2.66,9.34 2.86,9.48l2.03,1.58C4.84,11.36 4.8,11.69 4.8,12s0.02,0.64 0.07,0.94l-2.03,1.58c-0.18,0.14 -0.23,0.41 -0.12,0.61l1.92,3.32c0.12,0.22 0.37,0.29 0.59,0.22l2.39,-0.96c0.5,0.38 1.03,0.7 1.62,0.94l0.36,2.54c0.05,0.24 0.24,0.41 0.48,0.41h3.84c0.24,0 0.44,-0.17 0.47,-0.41l0.36,-2.54c0.59,-0.24 1.13,-0.56 1.62,-0.94l2.39,0.96c0.22,0.08 0.47,0 0.59,-0.22l1.92,-3.32c0.12,-0.22 0.07,-0.47 -0.12,-0.61l-2.01,-1.58zM12,15.6c-1.98,0 -3.6,-1.62 -3.6,-3.6s1.62,-3.6 3.6,-3.6 3.6,1.62 3.6,3.6 -1.62,3.6 -3.6,3.6z";
const P_PHONE: &str = "M17,1.01L7,1c-1.1,0 -2,0.9 -2,2v18c0,1.1 0.9,2 2,2h10c1.1,0 2,-0.9 2,-2V3c0,-1.1 -0.9,-1.99 -2,-1.99zM17,19H7V5h10v14z";
const P_HEADSET: &str = "M12,1c-4.97,0 -9,4.03 -9,9v7c0,1.66 1.34,3 3,3h3v-8H5v-2c0,-3.87 3.13,-7 7,-7s7,3.13 7,7v2h-4v8h3c1.66,0 3,-1.34 3,-3v-7c0,-4.97 -4.03,-9 -9,-9z";
const P_INFO: &str = "M12,2C6.48,2 2,6.48 2,12s4.48,10 10,10 10,-4.48 10,-10S17.52,2 12,2zM13,17h-2v-6h2v6zM13,9h-2L11,7h2v2z";
const P_TRASH: &str = "M6,19c0,1.1 0.9,2 2,2h8c1.1,0 2,-0.9 2,-2V7H6v12zM19,4h-3.5l-1,-1h-5l-1,1H5v2h14V4z";
const P_EXIT: &str = "M10.09,15.59L11.5,17l5,-5 -5,-5 -1.41,1.41L12.67,11H3v2h9.67l-2.58,2.59zM19,3H5c-1.11,0 -2,0.9 -2,2v4h2V5h14v14H5v-4H3v4c0,1.1 0.89,2 2,2h14c1.1,0 2,-0.9 2,-2V5c0,-1.1 -0.9,-2 -2,-2z";
const P_CHEVRON: &str = "M10,6L8.59,7.41 13.17,12l-4.58,4.59L10,18l6,-6z";
const P_COPY: &str = "M16,1H4c-1.1,0 -2,0.9 -2,2v14h2V3h12V1zM19,5H8c-1.1,0 -2,0.9 -2,2v14c0,1.1 0.9,2 2,2h11c1.1,0 2,-0.9 2,-2V7c0,-1.1 -0.9,-2 -2,-2zM19,21H8V7h11v14z";
const P_EDIT: &str = "M3,17.25V21h3.75L17.81,9.94l-3.75,-3.75L3,17.25zM20.71,7.04c0.39,-0.39 0.39,-1.02 0,-1.41l-2.34,-2.34c-0.39,-0.39 -1.02,-0.39 -1.41,0l-1.83,1.83 3.75,3.75 1.83,-1.83z";
const P_LOCK: &str = "M18,8h-1L17,6c0,-2.76 -2.24,-5 -5,-5S7,3.24 7,6v2H6c-1.1,0 -2,0.9 -2,2v10c0,1.1 0.9,2 2,2h12c1.1,0 2,-0.9 2,-2V10c0,-1.1 -0.9,-2 -2,-2zM12,17c-1.1,0 -2,-0.9 -2,-2s0.9,-2 2,-2 2,0.9 2,2 -0.9,2 -2,2zM15.1,8H8.9V6c0,-1.71 1.39,-3.1 3.1,-3.1 1.71,0 3.1,1.39 3.1,3.1v2z";
const P_CAMERA: &str = "M12,12m-3.2,0a3.2,3.2 0 1,0 6.4,0a3.2,3.2 0 1,0 -6.4,0M9,2L7.17,4H4c-1.1,0 -2,0.9 -2,2v12c0,1.1 0.9,2 2,2h16c1.1,0 2,-0.9 2,-2V6c0,-1.1 -0.9,-2 -2,-2h-3.17L15,2H9zM12,17c-2.76,0 -5,-2.24 -5,-5s2.24,-5 5,-5 5,2.24 5,5 -2.24,5 -5,5z";
const P_CROWN: &str = "M5,16L3,5l5.5,5L12,4l3.5,6L21,5l-2,11H5zM19,19c0,0.6 -0.4,1 -1,1H6c-0.6,0 -1,-0.4 -1,-1v-1h14v1z";

/// Dialog turlari (`dlg`): 0 — yo'q, 1 — chiqish, 2/3 — o'chirish (1- va 2-tasdiq), 4 — ilova haqida, 5 — xabar.
impl Home {
    pub(super) fn load_profile(self: &Rc<Self>) {
        let (me, session) = (self.clone(), self.session.clone());
        spawn_bg(move || fetch_profile(&session), move |r| {
            let dir = format!("{}/posters", uzcoders::android::files_dir());
            let size: u64 = std::fs::read_dir(dir).map(|d| d.flatten().filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum()).unwrap_or(0);
            me.cache_bytes.set(size as i64);
            if let Ok(p) = r {
                if !p.photo_url.is_empty() && !me.posters.borrow().contains_key(&p.photo_url) {
                    me.queue.borrow_mut().insert(0, p.photo_url.clone());
                    me.next_posters();
                }
                crate::accounts::upsert(&me.session, &p);
                me.consent.set(p.consent);
                crate::gate::set_consent(p.consent);
                crate::gate::start(me.session.clone(), p.id);
                me.profile.set(Rc::new(p));
            }
        });
    }

    /// Dumaloq avatar: poster (diskdagi shifrlangan nusxa) butun rasm kvadratga keltirilib dumaloqlanadi, bir marta.
    /// Rasm hali yuklanmagan bo'lsa — yuklash qayta so'raladi (10 soniyada bir, 6 martagacha).
    fn avatar_img(self: &Rc<Self>, url: &str) -> Option<Rc<Image>> {
        if let Some(a) = self.avatar.borrow().as_ref() {
            return Some(a.clone());
        }
        let img = self.posters.borrow().get(url).cloned();
        let Some(img) = img else {
            if !url.is_empty() && !self.loading_posters.get() && self.avatar_tries.get() < 6 && self.avatar_try.get().map_or(true, |t| t.elapsed().as_secs() >= 10) {
                self.avatar_try.set(Some(std::time::Instant::now()));
                self.avatar_tries.set(self.avatar_tries.get() + 1);
                self.queue.borrow_mut().insert(0, url.to_string());
                self.next_posters();
            }
            return None;
        };
        // Poster 480x720 ga cho'zilgan (kvadrat rasm torayib ketgan) — butun rasm kvadratga qaytariladi, kesilmaydi.
        let a = Rc::new(squash(&img, 256).round_corners(128.0));
        *self.avatar.borrow_mut() = Some(a.clone());
        Some(a)
    }

    /// Matnni `max_w` (dp) ichiga sig'maganda oxiriga "…" qo'yib qisqartiradi (Kotlin `maxLines = 1, overflow = Ellipsis`).
    pub(super) fn ellipsize(s: &str, size: f32, max_w: f32) -> String {
        thread_local! {
            static BOLD: Font = Font::parse(include_bytes!("../../assets/fonts/Roboto-Bold.ttf")).expect("Roboto Bold");
        }
        BOLD.with(|f| {
            if f.text_width(s, size) <= max_w {
                return s.to_string();
            }
            let mut t: String = s.chars().collect();
            while !t.is_empty() && f.text_width(&format!("{t}\u{2026}"), size) > max_w {
                t.pop();
            }
            format!("{}\u{2026}", t.trim_end())
        })
    }

    pub(super) fn pglass(e: Element) -> Element {
        e.flat_lens(Color::rgba(255, 255, 255, 8), GLASS_ZOOM)
    }

    /// Kotlin `StatBox`: karta fon, 18 dp burchak, ichki joy 14 dp; matnlar 24 sp qator balandligiga ega (Material `bodyLarge`).
    fn stat(icon_path: &'static str, label: &str, value: String) -> Element {
        column(vec![
            row(vec![icon(icon_path, 16.0).color(ACCENT), gap_box(6.0, 0.0), tx(label, 12.0, Color::rgba(255, 255, 255, 140)).line_height(24.0)]).align(Align::Center),
            tx(value, 17.0, Color::WHITE).black().line_height(24.0).margin_each(0.0, 8.0, 0.0, 0.0),
        ])
        .padding_xy(14.0, 14.0)
        .bg(CARD)
        .border(1.0, BORDER)
        .radius(18.0)
        .weight(1.0)
    }

    fn tile(&self, icon_path: &'static str, title: &str, f: impl Fn() + 'static) -> Element {
        row(vec![
            column(vec![icon(icon_path, 22.0).color(ACCENT2)]).center().width(42.0).height(42.0).radius(14.0).bg(ACCENT.with_opacity(0.2)),
            tx(title, 16.0, Color::WHITE).weight(1.0),
            icon(P_CHEVRON, 24.0).color(Color::WHITE),
        ])
        .gap(16.0)
        .align(Align::Center)
        .padding_xy(16.0, 12.0)
        .on_click(f)
    }

    fn danger(&self, icon_path: &'static str, label: &str, f: impl Fn() + 'static) -> Element {
        Self::pglass(
            row(vec![
                column(vec![icon(icon_path, 22.0).color(ACCENT2)]).center().width(40.0).height(40.0).radius(13.0).bg(ACCENT.with_opacity(0.2)),
                tx(label, 16.0, ACCENT2).bold(),
            ])
            .gap(14.0)
            .align(Align::Center)
            .padding_xy(20.0, 14.0),
        )
        .radius(24.0)
        .on_click(f)
    }

    /// Profil rasmini almashtirish: tanlangan rasm 720 px gacha kichraytiriladi va Telegram orqali yuklanadi.
    pub(super) fn change_avatar(self: &Rc<Self>) {
        let me = self.clone();
        self.pick("image/*", move |path| {
            let uid = me.profile.get().id;
            let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
            let name = format!("avatar_{uid}_{ts}.jpg");
            let (m2, session, n2) = (me.clone(), me.session.clone(), name.clone());
            me.dlg_msg.set("Rasm yuklanmoqda...".into());
            me.dlg.set(6);
            spawn_bg(
                move || -> Result<(), String> {
                    let bytes = std::fs::read(&path).map_err(|_| "Rasmni o'qib bo'lmadi".to_string())?;
                    let img = crate::webp::decode_any(&bytes).ok_or("Rasm noto'g'ri yoki qo'llab-quvvatlanmaydi (JPEG/PNG)".to_string())?.resized(720, 720);
                    let out = format!("{}/{n2}", std::path::Path::new(&path).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default());
                    std::fs::write(&out, img.to_png()).map_err(|_| "Rasm tayyorlanmadi".to_string())?;
                    let r = crate::backend::upload_file(&out, &n2, "image/jpeg", &session, &|_, _| {});
                    let _ = std::fs::remove_file(&out);
                    let _ = std::fs::remove_file(&path);
                    r?;
                    let (code, body) = crate::api::request("POST", "/api/auth/avatar", "", Some(&serde_json::json!({"file": n2})), Some(&session)).map_err(|_| "Tarmoq xatosi".to_string())?;
                    if code >= 400 { Err(format!("Rasm saqlanmadi ({code}): {body}")) } else { Ok(()) }
                },
                move |r| match r {
                    Ok(()) => {
                        m2.dlg.set(0);
                        *m2.avatar.borrow_mut() = None;
                        m2.load_profile();
                    }
                    Err(e) => {
                        m2.dlg_msg.set(e);
                        m2.dlg.set(5);
                    }
                },
            );
        });
    }

    fn do_logout(&self) {
        self.dlg.set(0);
        (self.on_logout)();
    }

    /// Material `TextButton`: matnli tugma (14 sp, o'rtacha qalinlik), 12x8 ichki joy.
    pub(super) fn text_btn(label: &str, color: Color, f: impl Fn() + 'static) -> Element {
        column(vec![tx(label, 14.0, color).bold()]).center().padding_xy(12.0, 10.0).radius(20.0).on_click(f)
    }

    fn dialog(self: &Rc<Self>) -> Element {
        let kind = self.dlg.get();
        let me = self.clone();
        let close = move || me.dlg.set(0);
        let (title, text, buttons): (String, String, Vec<Element>) = match kind {
            1 => {
                let (c, me) = (close.clone(), self.clone());
                ("Chiqish".into(), "Hisobdan chiqasizmi?".into(), vec![Self::text_btn("Bekor qilish", ACCENT, c), Self::text_btn("Chiqish", Color::hex(0xF87171), move || me.do_logout())])
            }
            2 => {
                let (c, me) = (close.clone(), self.clone());
                (
                    "Accountni o'chirish".into(),
                    "Hisobingiz, tarix, sevimlilar, baholar va izohlaringiz o'chiriladi. Bu amalni qaytarib bo'lmaydi.".into(),
                    vec![Self::text_btn("Bekor qilish", ACCENT, c), Self::text_btn("Davom etish", Color::hex(0xF87171), move || me.dlg.set(3))],
                )
            }
            3 => {
                let (c, me) = (close.clone(), self.clone());
                (
                    "Ishonchingiz komilmi?".into(),
                    "Account butunlay o'chiriladi.".into(),
                    vec![
                        Self::text_btn("Yo'q", ACCENT, c),
                        Self::text_btn("Ha, o'chirilsin", Color::hex(0xF87171), move || {
                            me.dlg_msg.set("Ma'lumotlar tozalanmoqda...".into());
                            me.dlg.set(6);
                            let (m2, session) = (me.clone(), me.session.clone());
                            spawn_bg(move || delete_account(&session), move |r| match r {
                                Ok(()) => m2.do_logout(),
                                Err(e) => {
                                    m2.dlg_msg.set(e);
                                    m2.dlg.set(5);
                                }
                            });
                        }),
                    ],
                )
            }
            4 => (
                "ARUMEDIATV".into(),
                format!("Versiya: {}\nRust + uzcoders (OpenGL ES)", crate::api::APP_VERSION),
                vec![Self::text_btn("Yopish", ACCENT, close.clone())],
            ),
            6 => (self.dlg_msg.get(), String::new(), vec![]),
            _ => ("Xabar".into(), self.dlg_msg.get(), vec![Self::text_btn("Yopish", ACCENT, close.clone())]),
        };
        // Material 3 `AlertDialog` (Kotlin): `SURFACE` fon, 28 dp burchak, nom 24 sp, matn 14 sp `TEXT_DIM`, matnli tugmalar.
        let mut kids = vec![tx(title, 24.0, Color::WHITE)];
        if !text.is_empty() {
            kids.push(gap_box(0.0, 16.0));
            kids.push(tx(text, 14.0, TEXT_DIM));
        }
        if kind == 6 {
            kids.push(gap_box(0.0, 14.0));
            kids.push(spinner(28.0, 3.0, ACCENT2));
        }
        if !buttons.is_empty() {
            kids.push(gap_box(0.0, 24.0));
            kids.push(row(std::iter::once(spacer()).chain(buttons).collect()).gap(8.0));
        }
        let scrim_close = close.clone();
        let panel = column(kids).bg(SURFACE).radius(28.0).padding(24.0).margin_each(24.0, 0.0, 24.0, 0.0);
        let scrim = column(vec![panel]).center().bg(Color::rgba(0, 0, 0, 82));
        if kind == 6 { scrim } else { scrim.on_click(scrim_close) }
    }

    /// Profil orqali ochiladigan sahifalar uchun rangli orqa fon (shisha shu ustidan sindirib ko'rsatadi).
    pub(super) fn glass_backdrop() -> Element {
        crate::theme::glass_backdrop()
    }

    pub(super) fn profile_view(self: &Rc<Self>) -> Element {
        let ins = safe_insets();
        let p = self.profile.get();
        let backdrop = Self::glass_backdrop();

        let active = p.sub_until > crate::home::now_ms_ui() as i64;
        let avatar = {
            // Aniq o'lcham: o'lchamsiz stack/ustun kichrayib ketardi; rasm yo'q bo'lsa — ko'rinadigan harfli doira.
            let inner = match self.avatar_img(&p.photo_url) {
                Some(a) => stack(vec![image_fill(&a)]).width(96.0).height(96.0),
                None => {
                    let letter: String = p.name.trim().chars().take(1).flat_map(|c| c.to_uppercase()).collect();
                    column(vec![tx(letter, 38.0, Color::WHITE).bold()]).center().width(96.0).height(96.0).radius(48.0).bg(ACCENT.with_opacity(0.85))
                }
            };
            let mut layers = vec![inner];
            if active {
                // Kotlin: tepada "PREMIUM" nishoni (Gold -> Accent2 gradient, AccentTint matn).
                layers.push(column(vec![
                    row(vec![spacer(), row(vec![icon(WORKSPACE_PREMIUM, 11.0).color(ACCENT_TINT), gap_box(2.0, 0.0), tx("PREMIUM", 8.0, ACCENT_TINT).black().letter_spacing(0.2).line_height(24.0)]).align(Align::Center).gradient(GOLD, ACCENT2).radius(20.0).padding_xy(6.0, 2.5), spacer()]).padding_each(0.0, 3.0, 0.0, 0.0),
                ]));
            }
            // Kamera belgisi (pastki o'ng): Card halqa ichida ACCENT doira.
            layers.push(column(vec![spacer(), row(vec![spacer(), column(vec![icon(P_CAMERA, 13.0).color(Color::WHITE)]).center().padding(5.0).radius(14.0).bg(ACCENT).border(2.0, CARD)])]));
            let me_a = self.clone();
            stack(layers).width(96.0).height(96.0).on_click(move || me_a.change_avatar())
        };
        let id_pill = row(vec![tx(format!("ID: {}", p.id), 14.5, Color::rgba(255, 255, 255, 199)).bold().line_height(24.0), icon(P_COPY, 16.0).color(Color::rgba(255, 255, 255, 153))])
            .gap(9.0)
            .align(Align::Center)
            .bg(Color::rgba(255, 255, 255, 20))
            .border(1.0, Color::rgba(255, 255, 255, 51))
            .radius(22.0)
            .padding_xy(14.0, 7.0);
        let (sw, _) = window_dp();
        // Matn ustuni kengligi: ekran - 32 (chekka) - 36 (ichki) - 96 (avatar) - 30 (oraliq) - 34 (o'ngdan joy).
        let name_w = sw - 32.0 - 36.0 - 96.0 - 30.0 - 34.0;
        let hero = column(vec![row(vec![
            avatar,
            gap_box(30.0, 0.0),
            column(vec![
                tx(Self::ellipsize(&p.name, 21.0, name_w), 21.0, Color::WHITE).bold().line_height(24.0),
                if p.username.is_empty() { gap_box(0.0, 0.0) } else { tx(Self::ellipsize(&format!("@{}", p.username), 15.0, name_w), 15.0, TEXT_DIM).line_height(24.0).margin_each(0.0, 4.0, 0.0, 0.0) },
                gap_box(0.0, 8.0),
                row(vec![id_pill]),
                gap_box(0.0, 6.0),
                tx(format!("Balans: {}", p.balance), 14.5, Color::rgba(255, 255, 255, 158)).bold().line_height(24.0),
            ])
            .weight(1.0),
        ])
        .align(Align::Center)
        .padding(18.0)])
        .bg(CARD)
        .border(1.0, BORDER)
        .radius(20.0);

        let me_e = self.clone();
        let edit_btn = column(vec![icon(P_EDIT, 17.0).color(Color::rgba(255, 255, 255, 217))]).center().padding(8.0).radius(21.0).bg(Color::rgba(255, 255, 255, 26)).border(1.0, Color::rgba(255, 255, 255, 46)).on_click(move || me_e.open_edit());
        let hero = stack(vec![hero, column(vec![row(vec![spacer(), edit_btn])]).padding(18.0)]);
        let me_b = self.clone();
        let billing = column(vec![row(vec![
            icon(P_CROWN, 22.0).color(Color::WHITE),
            tx("Obuna olish va Balans to'ldirish", 14.0, Color::WHITE).bold().weight(1.0),
            icon(P_CHEVRON, 20.0).color(Color::WHITE),
        ])
        .gap(10.0)
        .align(Align::Center)
        .padding_xy(16.0, 16.0)])
        .flat_lens(ACCENT.with_opacity(0.45), GLASS_ZOOM)
        .radius(24.0)
        .on_click(move || me_b.open_billing());

        let uid = p.id;
        let st = |ic: &'static str, label: &'static str, value: String, key: &'static str| {
            let me = self.clone();
            Self::stat(ic, label, value).on_click(move || me.open_stat(uid, key, label, ""))
        };
        let stats = column(vec![
            row(vec![Self::stat(P_MOVIE, "Anime", format_count(p.animes)), st(P_PLAY, "Qismlar", format_count(p.episodes), "episodes")]).gap(12.0),
            gap_box(0.0, 12.0),
            row(vec![st(P_GRID, "Bo'limlar", format_count(p.seasons), "seasons"), st(P_BOOKMARK, "Sevimlilar", format_count(p.favorites), "favorites")]).gap(12.0),
            gap_box(0.0, 12.0),
            row(vec![st(STAR, "Baholangan", format_count(p.rated), "rated"), st(P_COMMENT, "Izohlar", format_count(p.comments), "comments")]).gap(12.0),
            gap_box(0.0, 12.0),
            row(vec![Self::stat(P_CLOCK, "Tomosha vaqti", format!("{} soat", format_hours(p.watch_ms))), { let me = self.clone(); Self::stat(P_SD, "Xotira", format_bytes(self.cache_bytes.get())).on_click(move || me.open_storage()) }]).gap(12.0),
        ]);

        let traffic = Self::pglass(
            column(vec![
                row(vec![
                    icon(P_CLOUD, 15.0).color(ACCENT2),
                    tx("Trafik", 12.0, Color::WHITE).weight(1.0),
                    tx(format_bytes(p.traffic), 17.0, Color::WHITE).black(),
                ])
                .gap(6.0)
                .align(Align::Center),
                gap_box(0.0, 12.0),
                column(vec![]).height(7.0).radius(4.0).bg(if p.traffic > 0 { ACCENT } else { Color::rgba(255, 255, 255, 26) }),
                gap_box(0.0, 10.0),
                tx(if p.traffic > 0 { "Oldingi hisob".to_string() } else { "Hozircha trafik sarflanmagan".to_string() }, 12.5, Color::WHITE),
            ])
            .padding_xy(16.0, 14.0),
        )
        .radius(22.0);

        let soon = |me: &Rc<Self>, msg: &'static str| {
            let me = me.clone();
            move || {
                me.dlg_msg.set(msg.into());
                me.dlg.set(5);
            }
        };
        let divider = || column(vec![]).height(1.0).bg(Color::rgba(255, 255, 255, 30));
        let me = self.clone();
        let about = move || me.dlg.set(4);
        let group = Self::pglass(column(vec![
            self.tile(P_BELL, "Bildirishnoma", soon(self, "«Bildirishnoma» bo'limi tez orada qo'shiladi")),
            divider(),
            self.tile(P_GEAR, "Sozlamalar", { let me = self.clone(); move || me.open_settings() }),
            divider(),
            self.tile(P_PHONE, "Qurilmalar", { let me = self.clone(); move || me.open_sessions() }),
            divider(),
            self.tile(P_INFO, "Xatolar jurnali", { let me = self.clone(); move || me.open_errlog() }),
            divider(),
            divider(),
            self.tile(P_HEADSET, "Admin bilan bog'lanish", { let me = self.clone(); move || me.open_support(None) }),
            divider(),
            self.tile(P_INFO, "Ilova haqida", about),
        ]))
        .radius(24.0);

        let admin_tile = if p.is_admin {
            let me = self.clone();
            Some(Self::pglass(self.tile(P_LOCK, "Admin paneli", move || me.open_admin_hub())).radius(24.0))
        } else {
            None
        };
        let (m1, m2) = (self.clone(), self.clone());
        let kids = vec![
            gap_box(0.0, 8.0),
            self.acc_capsule(false),
            // Kotlin `PageHeader("Profil")`: ARU + MEDIATV va kichik "Profil" yozuvi.
            column(vec![
                row(vec![tx("ARU", 24.0, Color::WHITE).black(), tx("MEDIATV", 24.0, ACCENT2).black()]),
                gap_box(0.0, 2.0),
                tx("Profil", 13.0, Color::rgba(255, 255, 255, 115)),
            ])
            .padding_each(4.0, 16.0, 4.0, 4.0),
            gap_box(0.0, 8.0),
            hero,
            gap_box(0.0, 16.0),
            billing,
            gap_box(0.0, 12.0),
            stats,
            gap_box(0.0, 12.0),
            traffic,
            gap_box(0.0, 16.0),
            group,
            gap_box(0.0, 16.0),
            admin_tile.unwrap_or_else(|| gap_box(0.0, 0.0)),
            gap_box(0.0, if p.is_admin { 16.0 } else { 0.0 }),
            self.danger(P_TRASH, "Accountni o'chirish", move || m1.dlg.set(2)),
            gap_box(0.0, 16.0),
            self.danger(P_EXIT, "Accountdan chiqish", move || m2.dlg.set(1)),
        ];
        let body = scroll_column(&self.prof_scroll, kids).padding_each(16.0, ins.top, 16.0, 90.0 + ins.bottom);
        let mut layers = vec![backdrop, body];
        if ACC_OPEN.with(|s| s.get()) {
            layers.push(self.acc_panel(&p, ins.top));
        }
        if let Some(id) = ACC_REMOVE.with(|s| s.get()) {
            layers.push(self.acc_remove_dialog(id));
        }
        if self.dlg.get() != 0 {
            layers.push(self.dialog());
        }
        stack(layers)
    }

    /// Profil tepasidagi "Accountlarim ⌄" kapsulasi (Kotlin `CapsuleHeader`): 44 dp, oq 28% chegara, 22 dp burchak.
    fn acc_capsule(self: &Rc<Self>, open: bool) -> Element {
        const DOWN: &str = "M16.59,8.59L12,13.17 7.41,8.59 6,10l6,6 6,-6z";
        const UP: &str = "M12,8l-6,6 1.41,1.41L12,10.83l4.59,4.58L18,14z";
        let inner = row(vec![tx("Accountlarim", 16.0, Color::WHITE).line_height(24.0), gap_box(10.0, 0.0), icon(if open { UP } else { DOWN }, 22.0).color(Color::WHITE)]).align(Align::Center).height(44.0).padding_xy(18.0, 0.0);
        if open {
            inner.on_click(|| ACC_OPEN.with(|s| s.set(false)))
        } else {
            row(vec![inner.border(1.0, Color::rgba(255, 255, 255, 71)).radius(22.0).on_click(|| ACC_OPEN.with(|s| s.set(true))), spacer()])
        }
    }

    /// Ochilgan oyna: hisoblar ro'yxati, "Account qo'shish", soni (Kotlin `AccountsCapsule`).
    fn acc_panel(self: &Rc<Self>, p: &Profile, top: f32) -> Element {
        const CHECK: &str = "M9,16.17L4.83,12l-1.42,1.41L9,19 21,7l-1.41,-1.41z";
        let accs = crate::accounts::list();
        let now = crate::home::now_ms_ui() as i64;
        let mut kids: Vec<Element> = vec![self.acc_capsule(true)];
        for a in accs.iter() {
            let active = a.id == p.id;
            let letter: String = a.name.trim().chars().take(1).flat_map(|c| c.to_uppercase()).collect();
            let avatar = column(vec![tx(letter, 16.0, Color::WHITE).bold()]).center().width(40.0).height(40.0).radius(20.0).bg(ACCENT.with_opacity(0.85));
            let own = if a.username.is_empty() { format!("ID {}", a.id) } else { format!("@{}", a.username) };
            let sub = if a.sub_until > now { " \u{b7} Obuna" } else { "" };
            let title = if a.ident.is_empty() { format!("ID {}", a.id) } else { a.ident.clone() };
            let right: Element = if active {
                icon(CHECK, 20.0).color(ACCENT2)
            } else {
                let id = a.id;
                tx("\u{2715}", 16.0, Color::rgba(255, 255, 255, 115)).padding(8.0).on_click(move || ACC_REMOVE.with(|s| s.set(Some(id))))
            };
            let mut r = row(vec![avatar, gap_box(12.0, 0.0), column(vec![tx(title, 15.0, Color::WHITE).bold().line_height(20.0), tx(format!("{own}{sub}"), 12.0, Color::rgba(255, 255, 255, 128))]).weight(1.0), right])
                .align(Align::Center)
                .padding_xy(8.0, 8.0)
                .radius(16.0)
                .margin_each(0.0, 0.0, 0.0, 0.0);
            if active {
                r = r.bg(ACCENT.with_opacity(0.12));
            } else {
                let (me, from, to) = (self.clone(), p.id, a.id);
                r = r.on_click(move || {
                    ACC_OPEN.with(|s| s.set(false));
                    let _ = &me;
                    crate::accounts::switch_to(from, to);
                });
            }
            kids.push(r);
        }
        let (me, from) = (self.clone(), p.id);
        kids.push(
            row(vec![
                column(vec![tx("\u{ff0b}", 18.0, Color::WHITE)]).center().width(38.0).height(38.0).radius(19.0).bg(Color::rgba(255, 255, 255, 26)),
                gap_box(12.0, 0.0),
                tx("Account qo'shish", 15.0, Color::WHITE).bold(),
            ])
            .align(Align::Center)
            .padding_xy(8.0, 12.0)
            .radius(16.0)
            .on_click(move || {
                ACC_OPEN.with(|s| s.set(false));
                if !crate::accounts::can_add() {
                    me.dlg_msg.set(if crate::accounts::limit() < 5 { "Hozir 3 tagacha account ulash mumkin. Ulangan accountlardan birortasida obuna bo'lsa 5 tagacha ulash mumkin.".into() } else { "Eng ko'pi 5 ta account ulash mumkin.".into() });
                    me.dlg.set(5);
                } else {
                    crate::accounts::begin_add(from);
                }
            }),
        );
        let lim = crate::accounts::limit();
        kids.push(tx(format!("{} / {lim}{}", accs.len(), if lim < 5 { " \u{b7} birorta accountda obuna bo'lsa 5 tagacha" } else { "" }), 12.0, Color::rgba(255, 255, 255, 115)).padding_xy(10.0, 4.0));
        let panel = column(kids).width(290.0).padding_each(0.0, 0.0, 0.0, 8.0).bg(SURFACE).border(1.0, Color::rgba(255, 255, 255, 71)).radius(24.0).padding_each(6.0, 0.0, 6.0, 8.0);
        let scrim = column(vec![]).on_click(|| ACC_OPEN.with(|s| s.set(false)));
        let off = self.prof_scroll.offset() / density();
        stack(vec![scrim, column(vec![row(vec![panel, spacer()]).margin_each(16.0, (top + 8.0 - off).max(0.0) + 8.0, 0.0, 0.0), spacer()])])
    }

    fn acc_remove_dialog(self: &Rc<Self>, id: i64) -> Element {
        let name = crate::accounts::list().into_iter().find(|a| a.id == id).map(|a| if a.name.is_empty() { a.id.to_string() } else { a.name }).unwrap_or_default();
        let close = || ACC_REMOVE.with(|s| s.set(None));
        let panel = column(vec![
            tx("Accountni olib tashlash", 18.0, Color::WHITE).bold(),
            tx(format!("\u{ab}{name}\u{bb} shu telefondan chiqariladi. Accountning o'zi o'chmaydi \u{2014} keyin yana kirish mumkin."), 14.0, Color::rgba(255, 255, 255, 179)).padding_each(0.0, 12.0, 0.0, 16.0),
            row(vec![
                spacer(),
                Self::text_btn("Bekor qilish", ACCENT, close),
                Self::text_btn("Olib tashlash", Color::hex(0xF87171), move || {
                    ACC_REMOVE.with(|s| s.set(None));
                    crate::accounts::remove(id);
                }),
            ]),
        ])
        .bg(SURFACE)
        .radius(28.0)
        .padding(24.0)
        .margin_each(24.0, 0.0, 24.0, 0.0);
        column(vec![panel]).center().bg(Color::rgba(0, 0, 0, 140)).on_click(close)
    }
}

thread_local! {
    /// "Accountlarim" oynasi ochiqmi va qaysi hisobni olib tashlash so'ralmoqda.
    static ACC_OPEN: State<bool> = State::new(false);
    static ACC_REMOVE: State<Option<i64>> = State::new(None);
}

const P_BELL: &str = super::BELL;

/// Rasmni majburan `n`×`n` kvadratga (bilinear) keltiradi: poster 480x720 ga cho'zilgan kvadrat rasm edi, shuning uchun nisbat tiklanadi.
fn squash(img: &Image, n: u32) -> Image {
    let (w, h) = (img.w as f32, img.h as f32);
    let mut out = vec![0u8; (n * n * 4) as usize];
    for y in 0..n {
        let sy = ((y as f32 + 0.5) * h / n as f32 - 0.5).clamp(0.0, h - 1.0);
        let (y0, fy) = (sy.floor() as u32, sy.fract());
        let y1 = (y0 + 1).min(img.h - 1);
        for x in 0..n {
            let sx = ((x as f32 + 0.5) * w / n as f32 - 0.5).clamp(0.0, w - 1.0);
            let (x0, fx) = (sx.floor() as u32, sx.fract());
            let x1 = (x0 + 1).min(img.w - 1);
            for c in 0..4usize {
                let g = |xx: u32, yy: u32| img.rgba[((yy * img.w + xx) * 4) as usize + c] as f32;
                let v = g(x0, y0) * (1.0 - fx) * (1.0 - fy) + g(x1, y0) * fx * (1.0 - fy) + g(x0, y1) * (1.0 - fx) * fy + g(x1, y1) * fx * fy;
                out[((y * n + x) * 4) as usize + c] = v as u8;
            }
        }
    }
    Image { w: n, h: n, rgba: out }
}
