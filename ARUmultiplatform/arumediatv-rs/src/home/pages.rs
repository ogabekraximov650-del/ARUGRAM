//! Ichki sahifalar (Kotlin `SeasonScreen`, `SettingsScreen`, `SessionsScreen`): sahifalar steki Home ustida ochiladi.

use super::*;
use serde_json::Value;
use crate::data::{
    fetch_devices, fetch_episodes, fetch_info, fetch_sections, fmt_moment, fmt_ymd, format_count, format_hours, revoke_device, Device, Episode, SeasonInfo,
};

const A_BACK: &str = "M20,11H7.83l5.59,-5.59L12,4l-8,8 8,8 1.41,-1.41L7.83,13H20v-2z";
const A_EYE: &str = "M12,4.5C7,4.5 2.73,7.61 1,12c1.73,4.39 6,7.5 11,7.5s9.27,-3.11 11,-7.5c-1.73,-4.39 -6,-7.5 -11,-7.5zM12,17c-2.76,0 -5,-2.24 -5,-5s2.24,-5 5,-5 5,2.24 5,5 -2.24,5 -5,5zM12,9c-1.66,0 -3,1.34 -3,3s1.34,3 3,3 3,-1.34 3,-3 -1.34,-3 -3,-3z";
const A_CLOCK: &str = "M11.99,2C6.47,2 2,6.48 2,12s4.47,10 9.99,10C17.52,22 22,17.52 22,12S17.52,2 11.99,2zM12,20c-4.42,0 -8,-3.58 -8,-8s3.58,-8 8,-8 8,3.58 8,8 -3.58,8 -8,8zM12.5,7H11v6l5.25,3.15 0.75,-1.23 -4.5,-2.67z";
const A_HEART: &str = "M12,21.35l-1.45,-1.32C5.4,15.36 2,12.28 2,8.5 2,5.42 4.42,3 7.5,3c1.74,0 3.41,0.81 4.5,2.09C13.09,3.81 14.76,3 16.5,3 19.58,3 22,5.42 22,8.5c0,3.78 -3.4,6.86 -8.55,11.54L12,21.35z";
const A_HEART_O: &str = "M16.5,3c-1.74,0 -3.41,0.81 -4.5,2.09C10.91,3.81 9.24,3 7.5,3 4.42,3 2,5.42 2,8.5c0,3.78 3.4,6.86 8.55,11.54L12,21.35l1.45,-1.32C18.6,15.36 22,12.28 22,8.5 22,5.42 19.58,3 16.5,3zM12.1,18.55l-0.1,0.1 -0.1,-0.1C7.14,14.24 4,11.39 4,8.5 4,6.5 5.5,5 7.5,5c1.54,0 3.04,0.99 3.57,2.36h1.87C13.46,5.99 14.96,5 16.5,5c2,0 3.5,1.5 3.5,3.5 0,2.89 -3.14,5.74 -7.9,10.05z";
const A_STAR_O: &str = "M22,9.24l-7.19,-0.62L12,2 9.19,8.63 2,9.24l5.46,4.73L5.82,21 12,17.27 18.18,21l-1.63,-7.03L22,9.24zM12,15.4l-3.76,2.27 1,-4.28 -3.32,-2.88 4.38,-0.38L12,6.1l1.71,4.04 4.38,0.38 -3.32,2.88 1,4.28L12,15.4z";
const A_MOVIE: &str = "M18,4l2,4h-3l-2,-4h-2l2,4h-3l-2,-4H8l2,4H7L5,4H4c-1.1,0 -1.99,0.9 -1.99,2L2,18c0,1.1 0.9,2 2,2h16c1.1,0 2,-0.9 2,-2V4h-4z";
const A_EVENT: &str = "M16.53,11.06L15.47,10l-4.88,4.88 -2.12,-2.12 -1.06,1.06L10.59,17l5.94,-5.94zM19,3h-1V1h-2v2H8V1H6v2H5c-1.11,0 -1.99,0.9 -1.99,2L3,19c0,1.1 0.89,2 2,2h14c1.1,0 2,-0.9 2,-2V5c0,-1.1 -0.9,-2 -2,-2zM19,19H5V8h14v11z";
const A_PLAY: &str = "M8,5v14l11,-7z";
const A_LOGOUT: &str = "M10.09,15.59L11.5,17l5,-5 -5,-5 -1.41,1.41L12.67,11H3v2h9.67l-2.58,2.59zM19,3H5c-1.11,0 -2,0.9 -2,2v4h2V5h14v14H5v-4H3v4c0,1.1 0.89,2 2,2h14c1.1,0 2,-0.9 2,-2V5c0,-1.1 -0.9,-2 -2,-2z";
const A_PHONE: &str = "M17,1.01L7,1c-1.1,0 -2,0.9 -2,2v18c0,1.1 0.9,2 2,2h10c1.1,0 2,-0.9 2,-2V3c0,-1.1 -0.9,-1.99 -2,-1.99zM17,19H7V5h10v14z";
const A_CHECK: &str = "M9,16.17L4.83,12l-1.42,1.41L9,19 21,7l-1.41,-1.41z";
const A_DOWNLOAD: &str = "M19,9h-4V3H9v6H5l7,7 7,-7zM5,18v2h14v-2H5z";
const OK_GREEN: Color = Color::rgb(0x4A, 0xDE, 0x80);

/// Bo'lim sahifasi holati (har ochilganda yangidan quriladi).
pub struct SeasonPage {
    pub season: Season,
    pub(super) tab: State<usize>,
    pub(super) info: State<Rc<SeasonInfo>>,
    pub(super) eps: State<Rc<Vec<Episode>>>,
    pub(super) eps_loading: State<bool>,
    pub(super) eps_err: State<String>,
    /// Play bosilganda qismlar hali yuklanmagan edi: yuklangach o'zi ijro boshlaydi.
    pub(super) want_play: State<bool>,
    pub(super) sections: State<Rc<Vec<Season>>>,
    pub(super) cm: super::comments::CommentsState,
    pub(super) rating_open: State<bool>,
    pub(super) exp: State<i64>,
    pub(super) chunk: State<usize>,
    pub(super) newest: State<bool>,
    pub(super) chips_scroll: ScrollState,
    pub(super) scroll: [ScrollState; 4],
    pub(super) pl: super::player_ui::PlayerState,
}

#[derive(Clone)]
pub enum Page {
    Season(Rc<SeasonPage>),
    Settings,
    Sessions,
    Billing(Rc<super::billing::BillingPage>),
    Edit(Rc<super::edit::EditPage>),
    Storage(Rc<super::storage::StoragePage>),
    Support(Rc<super::support::SupportPage>),
    Media(Rc<super::mediaview::MediaPage>),
    DlAnime(Rc<super::dl::DlAnimePage>),
    Packs(Rc<super::packs::PacksPage>),
    AdminChannels(Rc<super::admin2::ChannelsPage>),
    AdminPacks(Rc<super::admin2::PacksReviewPage>),
    AdminApp(Rc<super::admin2::AppPage>),
    Public(Rc<super::pubprofile::PubPage>),
    Stat(Rc<super::statdetail::StatPage>),
    AdminHub,
    AdminUsers(Rc<super::admin::UsersPage>),
    AdminThreads(Rc<super::admin::ListPage>),
    AdminReports(Rc<super::admin::ListPage>),
    AdminStats(Rc<super::admin::StatsPage>),
    AdminEncode(Rc<super::admin3::EncodePage>),
    AdminContent(super::admin4::Content),
}

impl Home {
    /// Qismlar ro'yxati: xato bo'lsa 2 marta qayta uriniladi, so'ng sababi ko'rsatiladi (bosilsa qayta yuklanadi).
    pub(super) fn load_eps(self: &Rc<Self>, p: &Rc<SeasonPage>) {
        let (a, s) = (p.season.anime_id, p.season.season_id);
        p.eps_loading.set(true);
        p.eps_err.set(String::new());
        let (p2, me) = (p.clone(), self.clone());
        spawn_bg(
            move || {
                let mut last = Err(String::new());
                for i in 0..3 {
                    last = fetch_episodes(a, s);
                    if last.is_ok() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(600 * (i + 1)));
                }
                last
            },
            move |r| {
                let ok = r.is_ok();
                match r {
                    Ok(e) => p2.eps.set(Rc::new(e)),
                    Err(e) => p2.eps_err.set(e),
                }
                p2.eps_loading.set(false);
                if p2.want_play.get() {
                    p2.want_play.set(false);
                    p2.pl.loading.set(false);
                    if ok {
                        me.pl_start_default(&p2);
                    } else {
                        p2.pl.perr.set(format!("Qismlar yuklanmadi: {}", p2.eps_err.get()));
                    }
                }
            },
        );
    }

    pub(super) fn open_season(self: &Rc<Self>, season: Season) {
        let p = Rc::new(SeasonPage {
            season: season.clone(),
            tab: State::new(0),
            info: State::new(Rc::new(SeasonInfo::default())),
            eps: State::new(Rc::new(Vec::new())),
            eps_loading: State::new(true),
            eps_err: State::new(String::new()),
            want_play: State::new(false),
            sections: State::new(Rc::new(Vec::new())),
            cm: super::comments::CommentsState::new(),
            rating_open: State::new(false),
            exp: State::new(0),
            chunk: State::new(usize::MAX),
            newest: State::new(true),
            chips_scroll: ScrollState::new(),
            scroll: [ScrollState::new(), ScrollState::new(), ScrollState::new(), ScrollState::new()],
            pl: super::player_ui::PlayerState::new(),
        });
        self.pl_open_gate(&p);
        let (a, s) = (season.anime_id, season.season_id);
        let session = self.session.clone();
        let (p1, s1) = (p.clone(), session.clone());
        spawn_bg(move || fetch_info(a, s, &s1), move |r| {
            if let Ok(i) = r {
                p1.info.set(Rc::new(i));
            }
        });
        self.load_eps(&p);
        let (p3, me) = (p.clone(), self.clone());
        spawn_bg(move || fetch_sections(a), move |r| {
            if let Ok(list) = r {
                {
                    let have = me.posters.borrow();
                    let mut q = me.queue.borrow_mut();
                    for u in list.iter().map(|x| &x.photo_url) {
                        if !u.is_empty() && !have.contains_key(u) && !q.contains(u) {
                            q.push(u.clone());
                        }
                    }
                }
                p3.sections.set(Rc::new(list));
                me.next_posters();
            }
        });
        self.cm_load(&p);
        self.push(Page::Season(p));
    }

    pub(super) fn push(&self, page: Page) {
        let mut v = self.pages.get();
        v.push(page);
        self.pages.set(v);
    }

    pub(super) fn pop(&self) -> bool {
        let mut v = self.pages.get();
        let popped = v.pop();
        if let Some(Page::Season(sp)) = &popped {
            self.pl_close(sp);
        }
        if let Some(Page::Media(mp)) = &popped {
            self.media_close(mp);
        }
        let had = popped.is_some();
        if had {
            self.pages.set(v);
            self.dl_unwatch_all();
        }
        had
    }

    pub(super) fn open_settings(self: &Rc<Self>) {
        let p = self.profile.get();
        self.hidden.set(p.hidden.clone());
        self.consent.set(p.consent);
        self.push(Page::Settings);
    }

    pub(super) fn open_sessions(self: &Rc<Self>) {
        self.devices.set(None);
        let (me, session) = (self.clone(), self.session.clone());
        spawn_bg(move || fetch_devices(&session), move |r| me.devices.set(Some(r.map(Rc::new).map_err(|_| ()))));
        self.push(Page::Sessions);
    }

    // ── Umumiy bo'laklar ──

    fn card_box(e: Element) -> Element {
        Self::pglass(e).radius(18.0)
    }

    /// Profil orqali ochiladigan sahifa: rangli orqa fon, shisha sarlavha (orqaga + nom) va tana.
    pub(super) fn scaffold(self: &Rc<Self>, title: &str, body: Element) -> Element {
        let ins = safe_insets();
        let me = self.clone();
        let back = column(vec![icon(A_BACK, 24.0).color(Color::WHITE)]).center().width(44.0).height(44.0).radius(22.0).bg(Color::rgba(255, 255, 255, 22)).on_click(move || {
            me.pop();
        });
        let head = Self::pglass(row(vec![back, tx(title, 22.0, Color::WHITE).bold().weight(1.0)]).gap(12.0).align(Align::Center).padding_each(10.0, 8.0, 18.0, 8.0)).radius(32.0);
        stack(vec![Self::glass_backdrop(), column(vec![head.margin_each(16.0, 12.0 + ins.top, 16.0, 8.0), body.grow(1.0)]).padding_each(0.0, 0.0, 0.0, ins.bottom)])
    }

    fn tile_btn(icon_path: &'static str, tint: Color, label: String, f: impl Fn() + 'static) -> Element {
        Self::card_box(row(vec![icon(icon_path, 22.0).color(tint), tx(label, 13.0, Color::WHITE).bold().weight(1.0)]).gap(8.0).align(Align::Center).padding_xy(14.0, 14.0)).weight(1.0).on_click(f)
    }

    fn stat_line(icon_path: &'static str, label: &str, value: String) -> Element {
        row(vec![
            icon(icon_path, 16.0).color(Color::WHITE),
            tx(format!("{label}:"), 13.0, TEXT_DIM),
            row(vec![spacer(), tx(value, 13.5, Color::WHITE).bold()]).weight(1.0),
        ])
        .gap(9.0)
        .align(Align::Center)
        .padding_each(0.0, 5.0, 0.0, 5.0)
    }

    fn info_row(label: &str, value: String) -> Element {
        if value.is_empty() {
            return gap_box(0.0, 0.0);
        }
        row(vec![tx(label, 13.0, TEXT_DIM).width(90.0), tx(value, 13.0, Color::WHITE).weight(1.0)]).padding_each(0.0, 0.0, 0.0, 8.0)
    }

    pub(super) fn switch(on: bool, f: impl Fn() + 'static) -> Element {
        let thumb = column(vec![]).width(20.0).height(20.0).radius(10.0).bg(Color::WHITE);
        let inner = if on { row(vec![spacer(), thumb]) } else { row(vec![thumb, spacer()]) };
        inner.width(46.0).height(26.0).padding_xy(3.0, 3.0).radius(13.0).bg(if on { ACCENT } else { Color::rgba(255, 255, 255, 40) }).on_click(f)
    }

    pub(super) fn page_view(self: &Rc<Self>, page: Page) -> Element {
        match page {
            Page::Season(p) => self.season_page(&p),
            Page::Settings => self.settings_page(),
            Page::Sessions => self.sessions_page(),
            Page::Billing(p) => self.billing_page(&p),
            Page::Edit(p) => self.edit_page(&p),
            Page::Storage(p) => self.storage_page(&p),
            Page::Support(p) => self.support_page(&p),
            Page::Media(p) => self.media_page(&p),
            Page::DlAnime(p) => self.dl_anime_page(&p),
            Page::Packs(p) => self.packs_page(&p),
            Page::AdminChannels(p) => self.admin_channels_page(&p),
            Page::AdminPacks(p) => self.admin_packs_page(&p),
            Page::AdminApp(p) => self.admin_app_page(&p),
            Page::Public(p) => self.public_page(&p),
            Page::Stat(p) => self.stat_page(&p),
            Page::AdminHub => self.admin_hub_page(),
            Page::AdminUsers(p) => self.admin_users_page(&p),
            Page::AdminThreads(p) => self.admin_threads_page(&p),
            Page::AdminReports(p) => self.admin_reports_page(&p),
            Page::AdminStats(p) => self.admin_stats_page(&p),
            Page::AdminEncode(p) => self.admin_encode_page(&p),
            Page::AdminContent(c) => self.admin_content_page(&c),
        }
    }

    // ── Bo'lim sahifasi ──

    fn season_page(self: &Rc<Self>, p: &Rc<SeasonPage>) -> Element {
        let ins = safe_insets();
        let (w, _) = window_dp();
        let s = &p.season;
        let info = p.info.get();
        let tab = p.tab.get();
        if let Some(c) = self.consent_screen(p) {
            return c;
        }
        // Tepada muqova (Kotlin'da pleyer turgan joy): pleyer tayyor bo'lguncha poster kadri.
        let cover_h = (w - 32.0) * 9.0 / 16.0;
        let cover_img = match self.frame(&s.photo_url) {
            Some(f) => image_fill(&f),
            None => {
                let (a, b) = name_colors(&s.name);
                column(vec![]).gradient(a, b).radius(18.0)
            }
        };
        let me = self.clone();
        let back = column(vec![icon(A_BACK, 24.0).color(Color::WHITE)]).center().width(44.0).height(44.0).radius(22.0).bg(Color::rgba(0, 0, 0, 120)).on_click(move || {
            me.pop();
        });
        let bolim = if s.section_no > 0 { s.section_no } else { 1 };
        let cover = stack(vec![
            cover_img,
            column(vec![spacer(), column(vec![tx(s.name.clone(), 17.0, Color::WHITE).bold(), tx(format!("{bolim}-bo'lim · {} qism", format_count(s.episodes)), 12.5, Color::WHITE)]).padding_each(14.0, 26.0, 14.0, 12.0).gradient(Color::rgba(0, 0, 0, 0), Color::rgba(0, 0, 0, 200)).radius(18.0)]),
            column(vec![row(vec![back, spacer()])]).padding(8.0),
        ]);
        if p.pl.fullscreen.get() {
            let (fw, fh) = window_dp();
            let v = self.player_view(p, column(vec![]), fw, fh, true);
            let mut layers = vec![column(vec![v]).bg(Color::BLACK)];
            if p.pl.settings.get() {
                layers.push(self.player_settings(p));
            }
            return stack(layers);
        }
        let cover = self.player_view(p, cover, w - 32.0, cover_h, false).margin_each(16.0, 8.0 + ins.top, 16.0, 0.0);

        let titles = ["Ma'lumot", "Qismlar", "Bo'limlar", "Izohlar"];
        let tabs: Vec<Element> = titles
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let st = p.tab.clone();
                let on = i == tab;
                let e = column(vec![tx(*t, 13.0, if on { Color::WHITE } else { Color::rgba(255, 255, 255, 140) }).bold()]).center().weight(1.0).padding_xy(4.0, 8.0).radius(11.0).on_click(move || st.set(i));
                if on { e.bg(ACCENT) } else { e }
            })
            .collect();
        let tab_bar = Self::card_box(row(tabs).gap(2.0).padding_xy(3.0, 3.0)).radius(14.0).margin_each(16.0, 10.0, 16.0, 6.0);

        let body = match tab {
            0 => self.season_info_tab(p, &info),
            1 => self.season_eps_tab(p),
            2 => self.season_sections_tab(p),
            _ => self.season_comments_tab(p),
        };
        let pc = p.clone();
        let body = body.on_swipe(move |d| pc.tab.set((pc.tab.get() as i32 + d).clamp(0, 3) as usize));
        let mut layers = vec![Self::glass_backdrop(), column(vec![cover, tab_bar, body.grow(1.0)]).padding_each(0.0, 0.0, 0.0, ins.bottom)];
        if p.rating_open.get() {
            layers.push(self.rating_dialog(p, &info));
        }
        if p.pl.settings.get() {
            layers.push(self.player_settings(p));
        }
        if let Some(o) = self.cm_overlay(p) {
            layers.push(o);
        }
        stack(layers)
    }

    fn season_info_tab(self: &Rc<Self>, p: &Rc<SeasonPage>, info: &Rc<SeasonInfo>) -> Element {
        let s = &p.season;
        let pick = |k: &str, fb: String| {
            let v = info.str(k);
            if v.is_empty() { fb } else { v }
        };
        let rc = if info.season.is_null() { s.rating_count } else { info.num("rating_count") };
        let (pr, pi) = (p.clone(), info.clone());
        let rate = Self::tile_btn(
            if info.my_stars > 0 { STAR } else { A_STAR_O },
            GOLD,
            if info.my_stars > 0 { format!("Bahoyingiz: {}", info.my_stars) } else { "Baholash".into() },
            move || {
                let _ = &pi;
                pr.rating_open.set(true);
            },
        );
        let (pf, me) = (p.clone(), self.clone());
        let fav = Self::tile_btn(if info.is_fav { A_HEART } else { A_HEART_O }, ACCENT2, if info.is_fav { "Sevimlilarda".into() } else { "Sevimlilarga qo'shish".into() }, move || {
            let cur = pf.info.get();
            let mut next = (*cur).clone();
            next.is_fav = !cur.is_fav;
            let cnt = next.num("fav_count");
            next.season["fav_count"] = Value::from((cnt + if next.is_fav { 1 } else { -1 }).max(0));
            crate::sync::put_favorite(pf.season.anime_id, pf.season.season_id, next.is_fav);
            pf.info.set(Rc::new(next));
            me.load_library();
        });
        let rating_text = if rc > 0 { format!("{:05.2}  ({} ta baho)", if info.rating > 0.0 { info.rating } else { s.rating_avg }, format_count(rc)) } else { "hali baholanmagan".into() };
        let bolim = {
            let b = info.num("bolim_id");
            if b > 0 { b } else if s.section_no > 0 { s.section_no } else { 1 }
        };
        let created = info.num("created_at");
        let mut stats = vec![
            Self::stat_line(A_EYE, "Ko'rishlar", format_count(if info.season.is_null() { s.views } else { info.num("views_total") })),
            Self::stat_line(A_CLOCK, "Tomosha vaqti", format!("{} soat", format_hours(info.num("watch_ms_total")))),
            Self::stat_line(A_HEART_O, "Sevimlilarga qo'shilgan", format_count(info.num("fav_count"))),
            Self::stat_line(STAR, "Reyting", rating_text),
            Self::stat_line(A_MOVIE, "Bo'lim", format!("{bolim}-bo'lim · {} qism", format_count(if info.season.is_null() { s.episodes } else { info.num("epizod_count") }))),
        ];
        if created > 0 {
            stats.push(Self::stat_line(A_EVENT, "Qo'shilgan sana", fmt_moment(created)));
        }
        let mut det = vec![
            Self::info_row("Turi", pick("turi", s.kind.clone())),
            Self::info_row("Yili", pick("yili", s.year.clone())),
            Self::info_row("Janri", pick("janri", s.genres.join(", "))),
            Self::info_row("Studio", info.str("studio")),
            Self::info_row("Tarjimon", info.str("tarjimon")),
            Self::info_row("Holati", pick("holati", s.status.clone())),
        ];
        let tavsif = info.str("tavsif");
        if !tavsif.is_empty() {
            det.push(gap_box(0.0, 6.0));
            det.push(tx("Tavsif", 12.0, TEXT_DIM).bold());
            det.push(gap_box(0.0, 6.0));
            det.push(tx(tavsif, 14.0, Color::WHITE));
        }
        scroll_column(
            &p.scroll[0],
            vec![
                row(vec![rate, fav]).gap(12.0),
                gap_box(0.0, 12.0),
                Self::card_box(column(stats).padding(16.0)),
                gap_box(0.0, 12.0),
                Self::card_box(column(det).padding(16.0)),
                gap_box(0.0, 24.0),
            ],
        )
        .padding_each(16.0, 4.0, 16.0, 0.0)
    }

    fn rating_dialog(self: &Rc<Self>, p: &Rc<SeasonPage>, info: &Rc<SeasonInfo>) -> Element {
        let mk = |n: i64| {
            let (pp, on) = (p.clone(), n == info.my_stars);
            let e = column(vec![tx(n.to_string(), 16.0, Color::WHITE).bold()]).center().width(46.0).height(46.0).radius(23.0).on_click(move || {
                pp.rating_open.set(false);
                let cur = pp.info.get();
                let mut next = (*cur).clone();
                let old_count = cur.num("rating_count");
                let old_sum = if cur.season.get("rating_sum").is_some() { cur.num("rating_sum") } else { (cur.rating * old_count as f64).round() as i64 };
                let count = old_count + if cur.my_stars > 0 { 0 } else { 1 };
                let sum = old_sum + n - cur.my_stars;
                next.rating = if count <= 0 { 0.0 } else { (sum as f64 / count as f64 * 100.0).round() / 100.0 };
                next.my_stars = n;
                next.season["rating_count"] = Value::from(count);
                next.season["rating_sum"] = Value::from(sum);
                crate::sync::put_rating(pp.season.anime_id, pp.season.season_id, n);
                pp.info.set(Rc::new(next));
            });
            if on { e.bg(ACCENT) } else { e.bg(Color::rgba(255, 255, 255, 22)) }
        };
        let pc = p.clone();
        let panel = column(vec![
            tx("Baholang (1–10)", 18.0, Color::WHITE).bold(),
            gap_box(0.0, 16.0),
            row((1..=5).map(mk).collect()).gap(8.0),
            gap_box(0.0, 8.0),
            row((6..=10).map(mk).collect()).gap(8.0),
            gap_box(0.0, 14.0),
            row(vec![spacer(), self.chip("Bekor qilish", false, move || pc.rating_open.set(false))]),
        ])
        .lens(Color::rgba(18, 18, 24, 120), GLASS_ZOOM)
        .radius(28.0)
        .padding(22.0)
        .margin_each(24.0, 0.0, 24.0, 0.0);
        let pc = p.clone();
        column(vec![panel]).center().bg(Color::rgba(0, 0, 0, 140)).on_click(move || pc.rating_open.set(false))
    }

    fn season_eps_tab(self: &Rc<Self>, p: &Rc<SeasonPage>) -> Element {
        let eps = p.eps.get();
        if eps.is_empty() {
            return if p.eps_loading.get() {
                column(vec![spinner(32.0, 3.0, ACCENT2)]).center()
            } else if !p.eps_err.get().is_empty() {
                let (me, pc) = (self.clone(), p.clone());
                column(vec![tx(format!("Qismlar yuklanmadi: {}", p.eps_err.get()), 13.5, Color::WHITE), gap_box(0.0, 12.0), self.chip("Qayta urinish", true, move || me.load_eps(&pc))]).center().padding(24.0)
            } else {
                Self::empty(A_PLAY, "Qismlar topilmadi")
            };
        }
        const RANGE: usize = 50;
        let chunks: Vec<&[Episode]> = eps.chunks(RANGE).collect();
        let sel = if p.chunk.get() == usize::MAX { chunks.len().saturating_sub(1) } else { p.chunk.get().min(chunks.len().saturating_sub(1)) };
        let mut shown: Vec<Episode> = chunks.get(sel).map(|c| c.to_vec()).unwrap_or_default();
        if p.newest.get() {
            shown.reverse();
        }
        // Oraliq chiplari va tartib tugmasi (gorizontal suriladi).
        let mut chips: Vec<Element> = Vec::new();
        if chunks.len() > 1 {
            for (i, c) in chunks.iter().enumerate() {
                let pc = p.chunk.clone();
                chips.push(self.chip(&format!("{}–{}", c[0].number, c[c.len() - 1].number), i == sel, move || pc.set(i)));
            }
        }
        let pn = p.newest.clone();
        chips.push(self.chip(if p.newest.get() { "Yangilari tepada" } else { "Eskilari tepada" }, false, move || pn.set(!pn.get())));
        let mut kids: Vec<Element> = vec![row(chips).gap(8.0).scroll(&p.chips_scroll).height(40.0).margin_each(0.0, 0.0, 0.0, 8.0)];
        let live = self.dl.get();
        let exp = p.exp.get();
        for e in shown.iter() {
            let open = exp == e.id;
            // Yuklash holati (barcha sifatlar bo'yicha).
            let (mut done, mut active, mut pct) = (false, false, 0);
            for q in &e.qualities {
                if let Some(st) = live.get(&e.url(q)) {
                    done |= st.complete();
                    if (st.downloading || st.queued) && !st.complete() {
                        active = true;
                        pct = pct.max(st.percent());
                    }
                }
            }
            let (pe, me, eid) = (p.exp.clone(), self.clone(), e.id);
            let urls: Vec<String> = e.urls.iter().map(|(_, u)| u.clone()).collect();
            let right: Element = if done {
                icon(A_CHECK, 24.0).color(OK_GREEN)
            } else if active {
                tx(format!("{pct}"), 12.0, ACCENT2).black()
            } else {
                icon(A_DOWNLOAD, 24.0).color(Color::WHITE)
            };
            let sub = if e.name.trim().is_empty() { format!("{} ko'rish", format_count(e.views)) } else { format!("{} · {} ko'rish", e.name.trim(), format_count(e.views)) };
            let mut col = vec![row(vec![
                column(vec![tx(e.number.to_string(), 16.0, Color::WHITE).bold()]).center().width(48.0).height(48.0).radius(13.0).bg(Color::rgba(255, 255, 255, 18)),
                column(vec![tx(format!("{}-qism", e.number), 15.5, Color::WHITE).bold(), tx(sub, 12.0, TEXT_DIM)]).weight(1.0),
                column(vec![right]).center().width(48.0).height(48.0).on_click(move || {
                    if pe.get() == eid {
                        pe.set(0);
                    } else {
                        pe.set(eid);
                        me.dl_watch(&urls);
                    }
                }),
            ])
            .gap(12.0)
            .align(Align::Center)
            .padding_xy(12.0, 12.0)
            .on_click({
                let (me, pp, ee) = (self.clone(), p.clone(), e.clone());
                move || me.play_episode(&pp, &ee, None, None, true)
            })];
            if open {
                let mut rows: Vec<Element> = Vec::new();
                for q in &e.qualities {
                    rows.push(self.quality_row(e.url(q), e.quality_title(q), e.size(q)));
                }
                col.push(column(rows).padding_each(12.0, 0.0, 12.0, 10.0));
            }
            kids.push(Self::card_box(column(col)).margin_each(0.0, 0.0, 0.0, 8.0));
        }
        kids.push(gap_box(0.0, 16.0));
        scroll_column(&p.scroll[1], kids).padding_each(16.0, 4.0, 16.0, 0.0)
    }

    fn season_sections_tab(self: &Rc<Self>, p: &Rc<SeasonPage>) -> Element {
        let list = p.sections.get();
        if list.is_empty() {
            return Self::empty(A_PLAY, "Bo'limlar topilmadi");
        }
        let me = self.clone();
        let cur = p.season.season_id;
        lazy_column(&p.scroll[2], list.len(), 76.0, move |i| {
            let s = &list[i];
            let is_cur = s.season_id == cur;
            let thumb = match me.posters.borrow().get(&s.photo_url) {
                Some(img) => image_fill(img),
                None => {
                    let (a, b) = name_colors(&s.name);
                    column(vec![]).gradient(a, b).radius(10.0)
                }
            };
            let (m2, s2) = (me.clone(), s.clone());
            let mut kids = vec![
                stack(vec![thumb]).width(54.0).height(54.0),
                column(vec![
                    tx(if s.name.is_empty() { format!("{}-bo'lim", s.section_no) } else { s.name.clone() }, 14.0, if is_cur { ACCENT2 } else { Color::WHITE }).bold(),
                    tx(format!("{}-bo'lim · {} qism", s.section_no, s.episodes), 12.0, TEXT_DIM),
                ])
                .weight(1.0),
            ];
            if is_cur {
                kids.push(icon(A_PLAY, 24.0).color(ACCENT2));
            }
            let mut e = row(kids).gap(12.0).align(Align::Center).padding(10.0).glass(if is_cur { ACCENT.with_opacity(0.3) } else { Color::rgba(255, 255, 255, 20) }).border(1.0, if is_cur { ACCENT.with_opacity(0.6) } else { Color::rgba(255, 255, 255, 40) }).radius(16.0).height(66.0).margin_each(0.0, 0.0, 0.0, 10.0);
            if !is_cur {
                e = e.on_click(move || {
                    m2.pop();
                    m2.open_season(s2.clone());
                });
            }
            e
        })
        .padding_each(16.0, 4.0, 16.0, 24.0)
    }

    // ── Sozlamalar ──

    fn settings_page(self: &Rc<Self>) -> Element {
        let rows: [(&str, &str, &str); 7] = [
            ("anime", "Anime", "Nechta anime ko'rganingiz"),
            ("episodes", "Qismlar", "Nechta qism ko'rganingiz va ularning ro'yxati"),
            ("seasons", "Bo'limlar", "Ko'rgan bo'limlaringiz ro'yxati"),
            ("favorites", "Sevimlilar", "Sevimlilarga saqlaganlaringiz"),
            ("rated", "Baholangan", "Baho bergan bo'limlaringiz va bergan bahoyingiz"),
            ("comments", "Izohlar", "Yozgan izohlaringiz, javoblari va layklari"),
            ("watch", "Tomosha vaqti", "Jami necha soat tomosha qilganingiz"),
        ];
        let hidden = self.hidden.get();
        let label = |t: &str| tx(t, 12.0, ACCENT2).bold().padding_each(4.0, 8.0, 0.0, 0.0);
        let hint = |t: &str| tx(t, 12.0, TEXT_FAINT).padding_xy(4.0, 6.0);
        let mut kids = vec![
            label("MAXFIYLIK"),
            hint("Statistikangiz odatda OCHIQ turadi. Yashirmoqchi bo'lganingizning tugmasini yoqing — yoqilgani boshqalarga umuman ko'rinmaydi."),
        ];
        for (key, title, sub) in rows {
            let on = hidden.iter().any(|h| h == key);
            let me = self.clone();
            kids.push(
                Self::pglass(
                    row(vec![column(vec![tx(title, 15.0, Color::WHITE).bold(), tx(sub, 12.0, TEXT_DIM)]).weight(1.0), Self::switch(on, move || {
                        let mut h = me.hidden.get();
                        if let Some(i) = h.iter().position(|x| x == key) {
                            h.remove(i);
                        } else {
                            h.push(key.to_string());
                        }
                        crate::sync::put_settings(&h, me.consent.get());
                        me.hidden.set(h);
                    })])
                    .gap(8.0)
                    .align(Align::Center)
                    .padding_each(16.0, 10.0, 12.0, 10.0),
                )
                .radius(16.0)
                .margin_each(0.0, 5.0, 0.0, 5.0),
            );
        }
        kids.push(hint("Profil rasmi, ismingiz, username va ID raqamingiz har doim ko'rinadi. Balansingiz va sarflagan trafigingiz esa HECH QACHON boshqalarga ko'rinmaydi."));
        kids.push(label("BEPUL KO'RISH"));
        let me = self.clone();
        kids.push(
            Self::pglass(
                row(vec![
                    column(vec![
                        tx("Kanallarga avtomatik obuna", 15.0, Color::WHITE).bold(),
                        tx("Bepul bo'limlar uchun ilova Telegram hisobingiz bilan majburiy kanallarga qo'shiladi. O'chirsangiz, bepul bo'lim ochilganda ruxsat yana so'raladi.", 12.0, TEXT_DIM),
                    ])
                    .weight(1.0),
                    Self::switch(self.consent.get(), move || {
                        let on = !me.consent.get();
                        crate::sync::put_settings(&me.hidden.get(), on);
                        crate::gate::set_consent(on);
                        me.consent.set(on);
                    }),
                ])
                .gap(8.0)
                .align(Align::Center)
                .padding_each(16.0, 10.0, 12.0, 10.0),
            )
            .radius(16.0)
            .margin_each(0.0, 6.0, 0.0, 5.0),
        );
        kids.push(gap_box(0.0, 32.0));
        self.scaffold("Sozlamalar", scroll_column(&self.settings_scroll, kids).padding_each(16.0, 0.0, 16.0, 0.0))
    }

    // ── Qurilmalar ──

    fn sessions_page(self: &Rc<Self>) -> Element {
        let body = match self.devices.get() {
            None => column(vec![spinner(32.0, 3.0, ACCENT2)]).center(),
            Some(Err(())) => column(vec![tx("Ro'yxatni olib bo'lmadi", 14.0, TEXT_FAINT)]).center(),
            Some(Ok(list)) if list.is_empty() => column(vec![tx("Qurilmalar yo'q", 14.0, TEXT_FAINT)]).center(),
            Some(Ok(list)) => {
                let mut kids = vec![tx(format!("{} / 4 qurilma. Chegara to'lganda yangi qurilma kirsa, eng oldin onlayn bo'lgani avtomatik chiqariladi.", list.len()), 13.0, Color::WHITE).padding_each(4.0, 0.0, 0.0, 14.0)];
                for d in list.iter() {
                    kids.push(self.device_card(d, &list));
                }
                kids.push(gap_box(0.0, 40.0));
                scroll_column(&self.sessions_scroll, kids).padding_each(16.0, 8.0, 16.0, 0.0)
            }
        };
        self.scaffold("Qurilmalar", body)
    }

    fn device_card(self: &Rc<Self>, d: &Device, all: &Rc<Vec<Device>>) -> Element {
        let when = |ms: i64| if ms <= 0 { "—".to_string() } else { crate::data::fmt_date(ms) };
        let mut head = vec![
            icon(A_PHONE, 20.0).color(if d.current { OK_GREEN } else { Color::rgba(255, 255, 255, 180) }),
            tx(if d.device.is_empty() { "Noma'lum qurilma".to_string() } else { d.device.clone() }, 15.0, Color::WHITE).bold().weight(1.0),
        ];
        if d.current {
            head.push(tx("Shu qurilma", 11.0, OK_GREEN).bold().bg(OK_GREEN.with_opacity(0.16)).radius(8.0).padding_xy(9.0, 3.0));
        } else {
            let (me, id, all) = (self.clone(), d.id, all.clone());
            head.push(
                column(vec![icon(A_LOGOUT, 18.0).color(Color::WHITE)]).center().width(40.0).height(40.0).on_click(move || {
                    let (m2, session, all) = (me.clone(), me.session.clone(), all.clone());
                    spawn_bg(move || revoke_device(&session, id), move |ok| {
                        if ok {
                            let v: Vec<Device> = all.iter().filter(|x| x.id != id).cloned().collect();
                            m2.devices.set(Some(Ok(Rc::new(v))));
                        }
                    });
                }),
            );
        }
        let line = |k: &str, v: String| row(vec![tx(k, 13.0, Color::WHITE).width(110.0), tx(if v.is_empty() { "—".into() } else { v }, 13.0, Color::WHITE)]).padding_each(0.0, 4.0, 0.0, 0.0);
        Self::pglass(
            column(vec![
                row(head).gap(10.0).align(Align::Center),
                gap_box(0.0, 10.0),
                line("Tizim", d.platform.clone()),
                line("Ilova", if d.app_version.is_empty() { String::new() } else { format!("v{}", d.app_version) }),
                line("Kirgan", when(d.created_at)),
                line("Oxirgi faollik", when(d.last_seen_at)),
            ])
            .padding(16.0),
        )
        .margin_each(0.0, 0.0, 0.0, 12.0)
    }
}
