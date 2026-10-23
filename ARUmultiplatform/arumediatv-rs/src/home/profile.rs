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
const P_SMILE: &str = "M11.99,2C6.47,2 2,6.48 2,12s4.47,10 9.99,10C17.52,22 22,17.52 22,12S17.52,2 11.99,2zM12,20c-4.42,0 -8,-3.58 -8,-8s3.58,-8 8,-8 8,3.58 8,8 -3.58,8 -8,8zM15.5,11c0.83,0 1.5,-0.67 1.5,-1.5S16.33,8 15.5,8 14,8.67 14,9.5s0.67,1.5 1.5,1.5zM8.5,11c0.83,0 1.5,-0.67 1.5,-1.5S9.33,8 8.5,8 7,8.67 7,9.5 7.67,11 8.5,11zM12,17.5c2.33,0 4.31,-1.46 5.11,-3.5H6.89c0.8,2.04 2.78,3.5 5.11,3.5z";
const P_HEADSET: &str = "M12,1c-4.97,0 -9,4.03 -9,9v7c0,1.66 1.34,3 3,3h3v-8H5v-2c0,-3.87 3.13,-7 7,-7s7,3.13 7,7v2h-4v8h3c1.66,0 3,-1.34 3,-3v-7c0,-4.97 -4.03,-9 -9,-9z";
const P_INFO: &str = "M12,2C6.48,2 2,6.48 2,12s4.48,10 10,10 10,-4.48 10,-10S17.52,2 12,2zM13,17h-2v-6h2v6zM13,9h-2L11,7h2v2z";
const P_TRASH: &str = "M6,19c0,1.1 0.9,2 2,2h8c1.1,0 2,-0.9 2,-2V7H6v12zM19,4h-3.5l-1,-1h-5l-1,1H5v2h14V4z";
const P_EXIT: &str = "M10.09,15.59L11.5,17l5,-5 -5,-5 -1.41,1.41L12.67,11H3v2h9.67l-2.58,2.59zM19,3H5c-1.11,0 -2,0.9 -2,2v4h2V5h14v14H5v-4H3v4c0,1.1 0.89,2 2,2h14c1.1,0 2,-0.9 2,-2V5c0,-1.1 -0.9,-2 -2,-2z";
const P_CHEVRON: &str = "M10,6L8.59,7.41 13.17,12l-4.58,4.59L10,18l6,-6z";
const P_COPY: &str = "M16,1H4c-1.1,0 -2,0.9 -2,2v14h2V3h12V1zM19,5H8c-1.1,0 -2,0.9 -2,2v14c0,1.1 0.9,2 2,2h11c1.1,0 2,-0.9 2,-2V7c0,-1.1 -0.9,-2 -2,-2zM19,21H8V7h11v14z";
const P_EDIT: &str = "M3,17.25V21h3.75L17.81,9.94l-3.75,-3.75L3,17.25zM20.71,7.04c0.39,-0.39 0.39,-1.02 0,-1.41l-2.34,-2.34c-0.39,-0.39 -1.02,-0.39 -1.41,0l-1.83,1.83 3.75,3.75 1.83,-1.83z";
const P_LOCK: &str = "M18,8h-1L17,6c0,-2.76 -2.24,-5 -5,-5S7,3.24 7,6v2H6c-1.1,0 -2,0.9 -2,2v10c0,1.1 0.9,2 2,2h12c1.1,0 2,-0.9 2,-2V10c0,-1.1 -0.9,-2 -2,-2zM12,17c-1.1,0 -2,-0.9 -2,-2s0.9,-2 2,-2 2,0.9 2,2 -0.9,2 -2,2zM15.1,8H8.9V6c0,-1.71 1.39,-3.1 3.1,-3.1 1.71,0 3.1,1.39 3.1,3.1v2z";
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
        let a = Rc::new(Image { w: img.w, h: img.h, rgba: img.rgba.clone() }.resized(256, 256).round_corners(128.0));
        *self.avatar.borrow_mut() = Some(a.clone());
        Some(a)
    }

    pub(super) fn pglass(e: Element) -> Element {
        e.lens(Color::rgba(255, 255, 255, 8), GLASS_ZOOM)
    }

    fn stat(icon_path: &'static str, label: &str, value: String) -> Element {
        Self::pglass(
            column(vec![
                row(vec![icon(icon_path, 16.0).color(ACCENT2), tx(label, 12.0, Color::WHITE)]).gap(6.0).align(Align::Center),
                gap_box(0.0, 8.0),
                tx(value, 17.0, Color::WHITE).black(),
            ])
            .padding_xy(14.0, 14.0),
        )
        .radius(20.0)
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

    fn dialog(self: &Rc<Self>) -> Element {
        let kind = self.dlg.get();
        let me = self.clone();
        let close = move || me.dlg.set(0);
        let (title, text, buttons): (String, String, Vec<Element>) = match kind {
            1 => {
                let (c, me) = (close.clone(), self.clone());
                ("Chiqish".into(), "Hisobdan chiqasizmi?".into(), vec![self.chip("Bekor qilish", false, c), self.chip("Chiqish", true, move || me.do_logout())])
            }
            2 => {
                let (c, me) = (close.clone(), self.clone());
                (
                    "Accountni o'chirish".into(),
                    "Hisobingiz, tarix, sevimlilar, baholar va izohlaringiz o'chiriladi. Bu amalni qaytarib bo'lmaydi.".into(),
                    vec![self.chip("Bekor qilish", false, c), self.chip("Davom etish", true, move || me.dlg.set(3))],
                )
            }
            3 => {
                let (c, me) = (close.clone(), self.clone());
                (
                    "Ishonchingiz komilmi?".into(),
                    "Account butunlay o'chiriladi.".into(),
                    vec![
                        self.chip("Yo'q", false, c),
                        self.chip("Ha, o'chirilsin", true, move || {
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
                vec![self.chip("Yopish", true, close.clone())],
            ),
            6 => (self.dlg_msg.get(), String::new(), vec![]),
            _ => ("Xabar".into(), self.dlg_msg.get(), vec![self.chip("Yopish", true, close.clone())]),
        };
        let mut kids = vec![tx(title, 18.0, Color::WHITE).bold()];
        if !text.is_empty() {
            kids.push(gap_box(0.0, 10.0));
            kids.push(tx(text, 14.0, Color::WHITE));
        }
        if kind == 6 {
            kids.push(gap_box(0.0, 14.0));
            kids.push(spinner(28.0, 3.0, ACCENT2));
        }
        if !buttons.is_empty() {
            kids.push(gap_box(0.0, 18.0));
            kids.push(row(std::iter::once(spacer()).chain(buttons).collect()).gap(8.0));
        }
        let scrim_close = close.clone();
        let panel = column(kids).lens(Color::rgba(255, 255, 255, 14), GLASS_ZOOM).radius(28.0).padding(22.0).margin_each(24.0, 0.0, 24.0, 0.0);
        let scrim = column(vec![panel]).center().bg(Color::rgba(0, 0, 0, 45));
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

        let avatar = {
            // Aniq o'lcham: o'lchamsiz stack/ustun kichrayib ketardi; rasm yo'q bo'lsa — ko'rinadigan harfli doira.
            let inner = match self.avatar_img(&p.photo_url) {
                Some(a) => stack(vec![image_fill(&a)]).width(96.0).height(96.0),
                None => {
                    let letter: String = p.name.trim().chars().take(1).flat_map(|c| c.to_uppercase()).collect();
                    column(vec![tx(letter, 38.0, Color::WHITE).bold()]).center().width(96.0).height(96.0).radius(48.0).bg(ACCENT.with_opacity(0.85))
                }
            };
            let me_a = self.clone();
            column(vec![inner]).width(96.0).height(96.0).on_click(move || me_a.change_avatar())
        };
        let id_pill = row(vec![tx(format!("ID: {}", p.id), 14.0, Color::WHITE).bold(), icon(P_COPY, 16.0).color(Color::WHITE)])
            .gap(9.0)
            .align(Align::Center)
            .glass(Color::rgba(255, 255, 255, 24))
            .border(1.0, Color::rgba(255, 255, 255, 50))
            .radius(22.0)
            .padding_xy(14.0, 7.0);
        let hero = Self::pglass(
            row(vec![
                avatar,
                column(vec![
                    tx(p.name.clone(), 21.0, Color::WHITE).bold(),
                    if p.username.is_empty() { gap_box(0.0, 0.0) } else { tx(format!("@{}", p.username), 15.0, Color::WHITE) },
                    gap_box(0.0, 8.0),
                    row(vec![id_pill]),
                    gap_box(0.0, 6.0),
                    tx(format!("Balans: {}", p.balance), 14.5, Color::WHITE).bold(),
                ])
                .weight(1.0),
            ])
            .gap(20.0)
            .align(Align::Center)
            .padding(18.0),
        )
        .radius(28.0);

        let me_e = self.clone();
        let edit_btn = column(vec![icon(P_EDIT, 17.0).color(Color::WHITE)]).center().width(36.0).height(36.0).radius(18.0).bg(Color::rgba(255, 255, 255, 26)).on_click(move || me_e.open_edit());
        let hero = stack(vec![hero, column(vec![row(vec![spacer(), edit_btn])]).padding(14.0)]);
        let me_b = self.clone();
        let billing = column(vec![row(vec![
            icon(P_CROWN, 22.0).color(Color::WHITE),
            tx("Obuna olish va Balans to'ldirish", 14.0, Color::WHITE).bold().weight(1.0),
            icon(P_CHEVRON, 20.0).color(Color::WHITE),
        ])
        .gap(10.0)
        .align(Align::Center)
        .padding_xy(16.0, 16.0)])
        .lens(ACCENT.with_opacity(0.45), GLASS_ZOOM)
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
            self.tile(P_SMILE, "Emoji, GIF va stikerlar", { let me = self.clone(); move || me.open_packs() }),
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
            row(vec![Self::pglass(column(vec![tx("Profil", 24.0, Color::WHITE).black()]).padding_xy(22.0, 8.0)).radius(26.0), spacer()]).padding_each(0.0, 0.0, 0.0, 12.0),
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
        let body = scroll_column(&self.prof_scroll, kids).padding_each(16.0, 8.0 + ins.top, 16.0, 90.0 + ins.bottom);
        let mut layers = vec![backdrop, body];
        if self.dlg.get() != 0 {
            layers.push(self.dialog());
        }
        stack(layers)
    }
}

const P_BELL: &str = super::BELL;
