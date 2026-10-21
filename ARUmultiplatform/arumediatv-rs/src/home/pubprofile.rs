//! Boshqa foydalanuvchining ommaviy profili (Kotlin `PublicProfileScreen`): yashirilgan statistika ko'rinmaydi.

use super::*;
use crate::api;
use crate::data::{fmt_ymd, format_bytes, format_hours};
use serde_json::Value;

pub struct PubPage {
    id: i64,
    data: State<Option<Rc<Value>>>,
    error: State<String>,
}

fn num(v: &Value) -> i64 {
    v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)).unwrap_or(0)
}

impl Home {
    pub(super) fn open_public_profile(self: &Rc<Self>, id: i64) {
        if id <= 0 {
            return;
        }
        let p = Rc::new(PubPage { id, data: State::new(None), error: State::new(String::new()) });
        let (p2, session) = (p.clone(), self.session.clone());
        spawn_bg(move || api::get_json(&format!("/api/user/{id}"), "", Some(&session)), move |r| match r {
            Ok(j) => p2.data.set(Some(Rc::new(j))),
            Err(e) => p2.error.set(e),
        });
        self.push(pages::Page::Public(p));
    }

    pub(super) fn public_page(self: &Rc<Self>, p: &Rc<PubPage>) -> Element {
        let Some(d) = p.data.get() else {
            return self.scaffold("Profil", if p.error.get().is_empty() { column(vec![spinner(32.0, 3.0, ACCENT2)]).center() } else { column(vec![tx(p.error.get(), 14.0, ERROR)]).center() });
        };
        let first = d["first_name"].as_str().unwrap_or("").trim();
        let last = d["last_name"].as_str().unwrap_or("").trim();
        let username = d["username"].as_str().unwrap_or("");
        let name = match format!("{first} {last}").trim() {
            n if !n.is_empty() => n.to_string(),
            _ if !username.is_empty() => format!("@{username}"),
            _ => format!("Foydalanuvchi {}", p.id),
        };
        let letter: String = name.trim_start_matches('@').chars().take(1).flat_map(|c| c.to_uppercase()).collect();
        let mut badges = vec![tx(format!("ID {}", p.id), 12.0, Color::rgba(255, 255, 255, 170)).bg(Color::rgba(255, 255, 255, 20)).radius(10.0).padding_xy(10.0, 4.0)];
        if d["premium"].as_bool().unwrap_or(false) {
            badges.push(tx("Premium", 11.0, GOLD).bold().bg(GOLD.with_opacity(0.18)).radius(10.0).padding_xy(10.0, 4.0));
        }
        let header = Self::pglass(
            row(vec![
                column(vec![tx(letter, 30.0, ACCENT2).bold()]).center().width(78.0).height(78.0).radius(39.0).bg(ACCENT.with_opacity(0.22)),
                column(vec![
                    tx(name, 20.0, Color::WHITE).bold(),
                    if username.is_empty() { gap_box(0.0, 0.0) } else { tx(format!("@{username}"), 14.0, Color::rgba(255, 255, 255, 160)) },
                    row(badges).gap(8.0).padding_each(0.0, 8.0, 0.0, 0.0),
                ])
                .weight(1.0),
            ])
            .gap(16.0)
            .align(Align::Center)
            .padding(18.0),
        )
        .radius(26.0);
        let mut kids: Vec<Element> = vec![header.margin_each(0.0, 0.0, 0.0, 14.0)];
        if d["admin_view"].as_bool().unwrap_or(false) {
            let line = |l: &str, v: String| row(vec![tx(format!("{l}:"), 13.0, Color::rgba(255, 255, 255, 150)).width(130.0), tx(v, 13.0, Color::WHITE).bold()]).padding_each(0.0, 3.0, 0.0, 3.0);
            let until = num(&d["subscription_until"]);
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|x| x.as_millis() as i64).unwrap_or(0);
            kids.push(
                Self::pglass(
                    column(vec![
                        tx("ADMIN KO'RINISHI", 11.0, ACCENT2).bold(),
                        gap_box(0.0, 8.0),
                        line("Telegram ID", num(&d["telegram_id"]).to_string()),
                        line("Balans", super::billing::format_sum(num(&d["balance"]))),
                        line("Trafik", format_bytes(num(&d["traffic"]))),
                        line("Obuna", if until > now { format!("{} gacha", fmt_ymd(until)) } else { "yo'q".into() }),
                        line("Ro'yxatdan o'tgan", if num(&d["created_at"]) > 0 { fmt_ymd(num(&d["created_at"])) } else { "—".into() }),
                    ])
                    .padding(16.0),
                )
                .radius(22.0)
                .margin_each(0.0, 0.0, 0.0, 14.0),
            );
        }
        let rows: [(&str, &str, String); 7] = [
            ("Anime", "animes", num(&d["animes"]).to_string()),
            ("Qismlar", "episodes", num(&d["episodes"]).to_string()),
            ("Bo'limlar", "seasons", num(&d["seasons"]).to_string()),
            ("Tomosha vaqti", "watch_ms", format!("{} soat", format_hours(num(&d["watch_ms"])))),
            ("Sevimlilar", "favorites", num(&d["favorites"]).to_string()),
            ("Baholangan", "rated", num(&d["rated"]).to_string()),
            ("Izohlar", "comments", num(&d["comments"]).to_string()),
        ];
        for pair in rows.chunks(2) {
            let mut r: Vec<Element> = pair
                .iter()
                .map(|(label, key, value)| {
                    let visible = d.get(*key).is_some();
                    Self::pglass(column(vec![tx(if visible { value.clone() } else { "Yashirilgan".to_string() }, if visible { 20.0 } else { 14.0 }, if visible { ACCENT2 } else { Color::rgba(255, 255, 255, 110) }).black(), tx(*label, 12.0, Color::rgba(255, 255, 255, 150))]).padding_xy(16.0, 14.0)).radius(22.0).weight(1.0)
                })
                .collect();
            if r.len() == 1 {
                r.push(spacer().weight(1.0));
            }
            kids.push(row(r).gap(10.0).padding_each(0.0, 0.0, 0.0, 10.0));
        }
        kids.push(gap_box(0.0, 32.0));
        self.scaffold("Profil", scroll_column(&self.settings_scroll, kids).padding_each(16.0, 4.0, 16.0, 0.0))
    }
}
