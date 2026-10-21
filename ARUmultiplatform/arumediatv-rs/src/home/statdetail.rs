//! Statistika tafsiloti (Kotlin `StatDetailScreen`): qismlar, bo'limlar, sevimlilar, baholar va izohlar ro'yxati, 40 tadan sahifalab.

use super::*;
use crate::api;
use crate::data::{fmt_date, parse_seasons, Season};
use serde_json::Value;

pub struct StatPage {
    user: i64,
    key: &'static str,
    title: String,
    items: State<Rc<Vec<Value>>>,
    seasons: State<Rc<Vec<Season>>>,
    page: Cell<i64>,
    more: Cell<bool>,
    loading: State<bool>,
    error: State<String>,
    scroll: ScrollState,
}

fn num(v: &Value) -> i64 {
    v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)).unwrap_or(0)
}

impl Home {
    /// `key`: episodes | seasons | favorites | rated | comments. `owner` — boshqa foydalanuvchi ismi (o'zingiznikida bo'sh).
    pub(super) fn open_stat(self: &Rc<Self>, user: i64, key: &'static str, label: &str, owner: &str) {
        if user <= 0 {
            return;
        }
        let title = if owner.is_empty() { label.to_string() } else { format!("{label} · {owner}") };
        let p = Rc::new(StatPage {
            user,
            key,
            title,
            items: State::new(Rc::new(Vec::new())),
            seasons: State::new(Rc::new(Vec::new())),
            page: Cell::new(0),
            more: Cell::new(true),
            loading: State::new(false),
            error: State::new(String::new()),
            scroll: ScrollState::new(),
        });
        self.stat_more(&p);
        self.push(pages::Page::Stat(p));
    }

    fn stat_more(self: &Rc<Self>, p: &Rc<StatPage>) {
        if p.loading.get() || !p.more.get() {
            return;
        }
        p.loading.set(true);
        p.error.set(String::new());
        let (p2, me, session, n) = (p.clone(), self.clone(), self.session.clone(), p.page.get());
        let path = format!("/api/user/{}/stats/{}", p.user, p.key);
        spawn_bg(move || api::request("GET", &path, &format!("?page={n}"), None, Some(&session)), move |r| {
            p2.loading.set(false);
            match r {
                Ok((200, body)) => {
                    let j: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
                    let rows: Vec<Value> = j["items"].as_array().cloned().unwrap_or_default();
                    p2.more.set(j["has_more"].as_bool().unwrap_or(false) && !rows.is_empty());
                    p2.page.set(p2.page.get() + 1);
                    if matches!(p2.key, "seasons" | "favorites" | "rated") {
                        let add = parse_seasons(&Value::Array(rows).to_string());
                        let mut all = (*p2.seasons.get()).clone();
                        all.extend(add);
                        p2.seasons.set(Rc::new(all));
                    } else {
                        let mut all = (*p2.items.get()).clone();
                        all.extend(rows);
                        p2.items.set(Rc::new(all));
                    }
                    me.next_posters();
                }
                Ok((403, _)) => {
                    p2.more.set(false);
                    p2.error.set("Bu statistika yashirilgan".into());
                }
                Ok(_) => p2.error.set("Ro'yxat kelmadi".into()),
                Err(_) => p2.error.set("Internet yo'q".into()),
            }
        });
    }

    pub(super) fn stat_page(self: &Rc<Self>, p: &Rc<StatPage>) -> Element {
        let ins = safe_insets();
        let is_seasons = matches!(p.key, "seasons" | "favorites" | "rated");
        let empty = if is_seasons { p.seasons.get().is_empty() } else { p.items.get().is_empty() };
        let footer: Element = if p.loading.get() {
            column(vec![spinner(24.0, 2.5, ACCENT2)]).center().padding(14.0)
        } else if !p.error.get().is_empty() {
            let (me, pc) = (self.clone(), p.clone());
            tx(format!("{} — qayta urinish", p.error.get()), 13.0, ERROR).padding(14.0).on_click(move || me.stat_more(&pc))
        } else if p.more.get() {
            let (me, pc) = (self.clone(), p.clone());
            column(vec![tx("Yana yuklash", 14.0, Color::WHITE).bold()]).center().padding_xy(20.0, 12.0).radius(20.0).bg(Color::rgba(255, 255, 255, 22)).margin_each(16.0, 8.0, 16.0, 8.0).on_click(move || me.stat_more(&pc))
        } else {
            gap_box(0.0, 0.0)
        };
        if empty {
            let msg = if p.loading.get() { String::new() } else if p.error.get().is_empty() { "Hozircha bo'sh".to_string() } else { p.error.get() };
            return self.scaffold(&p.title, column(vec![if p.loading.get() { spinner(32.0, 3.0, ACCENT2) } else { tx(msg, 15.0, TEXT_DIM) }]).center());
        }
        let body = if is_seasons {
            let (w, _) = window_dp();
            let card_w = ((w - 32.0 - 12.0) / 2.0).max(80.0);
            column(vec![self.poster_grid(p.seasons.get(), &p.scroll, card_w * 1.5, 4.0, 8.0).grow(1.0), footer])
        } else {
            let mut kids: Vec<Element> = Vec::new();
            for o in p.items.get().iter() {
                kids.push(if p.key == "comments" { Self::stat_comment(o) } else { Self::stat_episode(o) });
            }
            kids.push(footer);
            kids.push(gap_box(0.0, 24.0 + ins.bottom));
            scroll_column(&p.scroll, kids).padding_each(16.0, 4.0, 16.0, 0.0)
        };
        self.scaffold(&p.title, body)
    }

    fn stat_episode(o: &Value) -> Element {
        let title = o["season_name"].as_str().filter(|s| !s.is_empty()).or(o["anime_name"].as_str()).unwrap_or("");
        let (pos, dur) = (num(&o["position_ms"]), num(&o["duration_ms"]));
        let pct = if dur > 0 { (pos * 100 / dur).clamp(0, 100) } else { 0 };
        let mut sub = format!("{}-qism · {}-bo'lim", num(&o["epizod_number"]), num(&o["bolim_id"]));
        if num(&o["view_count"]) > 1 {
            sub.push_str(&format!(" · {} marta", num(&o["view_count"])));
        }
        let mut kids = vec![
            tx(title, 15.0, Color::WHITE).bold(),
            tx(sub, 12.5, Color::WHITE),
            gap_box(0.0, 8.0),
            column(vec![]).height(5.0).radius(3.0).bg(Color::rgba(255, 255, 255, 40)).weight(1.0),
        ];
        let when = num(&o["updated_at"]);
        kids.push(gap_box(0.0, 6.0));
        kids.push(tx(format!("{pct}% · {}", if when > 0 { fmt_date(when) } else { String::new() }), 11.5, Color::WHITE));
        if num(&o["deleted_at"]) != 0 {
            kids.push(tx("Tarixdan tozalangan", 11.5, TEXT_DIM));
        }
        Self::pglass(column(kids).padding(14.0)).radius(18.0).margin_each(0.0, 0.0, 0.0, 12.0)
    }

    fn stat_comment(o: &Value) -> Element {
        let title = o["season_name"].as_str().filter(|s| !s.is_empty()).or(o["anime_name"].as_str()).unwrap_or("");
        let body = match (o["body"].as_str().unwrap_or(""), o["media_type"].as_str().unwrap_or("")) {
            ("", "sticker") => "[Stiker]".to_string(),
            ("", "gif") => "[GIF]".to_string(),
            (b, _) => b.to_string(),
        };
        let meta = format!("{} layk · {} javob · {}", num(&o["likes"]), num(&o["reply_count"]), fmt_date(num(&o["created_at"])));
        Self::pglass(column(vec![tx(title, 12.0, ACCENT2).bold(), gap_box(0.0, 4.0), tx(body, 14.0, Color::WHITE), gap_box(0.0, 6.0), tx(meta, 11.0, Color::WHITE)]).padding(14.0)).radius(18.0).margin_each(0.0, 0.0, 0.0, 12.0)
    }
}
