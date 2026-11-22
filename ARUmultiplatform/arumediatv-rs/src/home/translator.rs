//! Tarjimon sahifasi (Kotlin `TranslatorScreen`): o'rtada dumaloq logotip, nom, tavsif, to'rtta raqam va animelari.

use super::*;
use crate::data::{fetch_translator_page, Translator, TranslatorPage};

pub struct TrPage {
    pub tr: Translator,
    pub data: State<Option<Rc<TranslatorPage>>>,
    pub failed: State<bool>,
    pub scroll: ScrollState,
}

impl Home {
    pub(super) fn open_translator(self: &Rc<Self>, t: Translator) {
        uzcoders::prelude::breadcrumb("open_translator");
        let p = Rc::new(TrPage { tr: t.clone(), data: State::new(None), failed: State::new(false), scroll: ScrollState::new() });
        let p2 = p.clone();
        spawn_bg(move || fetch_translator_page(t.id), move |r| match r {
            Ok(d) => p2.data.set(Some(Rc::new(d))),
            Err(_) => p2.failed.set(true),
        });
        self.push(pages::Page::Translator(p));
    }

    fn stat_tile(label: &str, value: String) -> Element {
        column(vec![
            column(vec![tx(label, 10.5, Color::rgba(255, 255, 255, 140)).line_height(12.0)]).center().height(26.0),
            tx(value, 17.0, Color::WHITE).black().padding_each(0.0, 4.0, 0.0, 0.0),
        ])
        .align(Align::Center)
        .bg(CARD)
        .border(1.0, BORDER)
        .radius(14.0)
        .padding_xy(4.0, 10.0)
        .weight(1.0)
    }

    /// Baho katagi: yulduzcha belgisi + raqam (shriftda "★" belgisi yo'q).
    fn stat_tile_star(label: &str, value: Option<String>) -> Element {
        let v = match value {
            Some(v) => row(vec![icon(STAR, 15.0).color(GOLD), tx(v, 17.0, Color::WHITE).black()]).gap(3.0).align(Align::Center),
            None => row(vec![tx("\u{2014}", 17.0, Color::WHITE).black()]),
        };
        column(vec![column(vec![tx(label, 10.5, Color::rgba(255, 255, 255, 140)).line_height(12.0)]).center().height(26.0), v.padding_each(0.0, 4.0, 0.0, 0.0)])
            .align(Align::Center)
            .bg(CARD)
            .border(1.0, BORDER)
            .radius(14.0)
            .padding_xy(4.0, 10.0)
            .weight(1.0)
    }

    pub(super) fn translator_page(self: &Rc<Self>, p: &Rc<TrPage>) -> Element {
        let ins = safe_insets();
        let (w, _) = window_dp();
        let card_w = ((w - 32.0 - 14.0) / 2.0).max(80.0);
        let card_h = card_w / 0.65;
        let data = p.data.get();
        let t = data.as_ref().map(|d| d.translator.clone()).unwrap_or_else(|| p.tr.clone());
        let ready = data.is_some();
        let (anime, episodes, views, rating) = data.as_ref().map(|d| (d.anime, d.episodes, d.views, d.rating)).unwrap_or((0, 0, 0, 0.0));
        let dash = || "\u{2014}".to_string();
        let mut kids = vec![
            gap_box(0.0, 56.0 - 8.0 + ins.top),
            row(vec![spacer(), self.tr_logo(&t, 96.0), spacer()]),
            gap_box(0.0, 12.0),
            row(vec![spacer(), tx(t.name.clone(), 21.0, Color::WHITE).black(), spacer()]),
        ];
        if !t.bio.is_empty() {
            kids.push(gap_box(0.0, 8.0));
            thread_local! {
                static REG: Font = Font::parse(include_bytes!("../../assets/fonts/Roboto-Regular.ttf")).expect("Roboto Regular");
            }
            let one_line = REG.with(|f| f.text_width(&t.bio, 13.0)) <= w - 56.0;
            let bio = text(t.bio.clone()).size(13.0).color(TEXT_DIM).line_height(18.0);
            // Kotlin: tavsif o'rtada. Bir qatorga sig'sa o'rtaga qo'yiladi, sig'masa kenglik ichida qatorlarga bo'linadi.
            kids.push(if one_line { row(vec![spacer(), bio, spacer()]) } else { row(vec![spacer(), column(vec![bio]).width(w - 56.0), spacer()]) });
        }
        kids.push(gap_box(0.0, 16.0));
        kids.push(
            row(vec![
                Self::stat_tile("Jami anime", if ready { anime.to_string() } else { dash() }),
                Self::stat_tile("Jami qismlar", if ready { episodes.to_string() } else { dash() }),
                Self::stat_tile("Ko'rishlar", if ready { crate::data::format_count(views) } else { dash() }),
                Self::stat_tile_star("O'rtacha baho", if ready && rating > 0.0 { Some(format!("{rating:.1}")) } else { None }),
            ])
            .gap(8.0),
        );
        kids.push(gap_box(0.0, 12.0));
        kids.push(tx("Animelar", 15.0, Color::WHITE).bold());
        kids.push(gap_box(0.0, 14.0));
        match &data {
            None if !p.failed.get() => kids.push(row(vec![spacer(), spinner(32.0, 3.0, ACCENT2), spacer()]).padding_each(0.0, 40.0, 0.0, 0.0)),
            None => kids.push(Self::empty(MENU, "Ma'lumot yuklanmadi")),
            Some(d) if d.seasons.is_empty() => kids.push(Self::empty(MENU, "Bu tarjimonning animelari hali yo'q")),
            Some(d) => {
                for pair in d.seasons.chunks(2) {
                    let a = self.card(&pair[0], card_h);
                    let b = match pair.get(1) {
                        Some(s) => self.card(s, card_h),
                        None => spacer().weight(1.0),
                    };
                    kids.push(row(vec![a, b]).gap(14.0).padding_each(0.0, 0.0, 0.0, 14.0));
                }
            }
        }
        let me = self.clone();
        let back = column(vec![icon(A_BACK_ICON, 24.0).color(Color::WHITE)]).center().width(44.0).height(44.0).radius(22.0).on_click(move || {
            me.pop();
        });
        stack(vec![
            column(vec![]).bg(BG),
            scroll_column(&p.scroll, kids).padding_each(16.0, 0.0, 16.0, 90.0 + ins.bottom),
            column(vec![row(vec![back, spacer()]), spacer()]).padding_each(8.0, 8.0 + ins.top, 8.0, 0.0),
        ])
    }
}

const A_BACK_ICON: &str = "M20,11H7.83l5.59,-5.59L12,4l-8,8 8,8 1.41,-1.41L7.83,13H20v-2z";
