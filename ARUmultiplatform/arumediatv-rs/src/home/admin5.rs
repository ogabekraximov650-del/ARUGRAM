//! Admin: ma'lumotlar bazasi (Kotlin `AdminDbScreen`, FAQAT O'QISH): jurnallar ro'yxati -> jadval (ustunlar, yozuvlar, qidiruv, tartiblash).

use super::*;
use crate::api;
use serde_json::Value;

#[derive(Clone)]
pub enum DbContent {
    Tables(Rc<DbTables>),
    Rows(Rc<DbRows>),
}

pub struct DbTables {
    items: State<Rc<Vec<(String, String)>>>,
    filter: State<String>,
    loaded: State<bool>,
    error: State<String>,
    scroll: ScrollState,
}

#[derive(Clone)]
struct Col {
    name: String,
    ty: String,
    pk: bool,
}

pub struct DbRows {
    table: String,
    cols: State<Rc<Vec<Col>>>,
    rows: State<Rc<Vec<Vec<Option<String>>>>>,
    has_more: State<bool>,
    order: State<String>,
    desc: State<bool>,
    fcol: State<String>,
    input: State<String>,
    applied: State<String>,
    zoom: State<f32>,
    loading: State<bool>,
    more_busy: std::cell::Cell<bool>,
    error: State<String>,
    detail: State<Option<usize>>,
    menu: State<bool>,
    hscroll: ScrollState,
    vscroll: ScrollState,
    dscroll: ScrollState,
    mscroll: ScrollState,
}

const STEP: usize = 100;

fn col_base(c: &Col) -> f32 {
    let t = c.ty.to_uppercase();
    if t.contains("INT") {
        96.0
    } else if t.contains("REAL") || t.contains("NUM") || t.contains("FLOA") {
        110.0
    } else {
        180.0
    }
}

fn input_style(st: &State<String>, hint: &str) -> Element {
    field(st).color(Color::WHITE).size(14.0).padding_xy(12.0, 10.0).flat_glass(Color::rgba(255, 255, 255, 22)).hint(hint).hint_color(Color::rgba(255, 255, 255, 150)).accent(ACCENT2).border(1.0, Color::TRANSPARENT).radius(12.0)
}

impl Home {
    pub(super) fn open_admin_db(self: &Rc<Self>) {
        uzcoders::prelude::breadcrumb("open_admin_db");
        let p = Rc::new(DbTables { items: State::new(Rc::new(Vec::new())), filter: State::new(String::new()), loaded: State::new(false), error: State::new(String::new()), scroll: ScrollState::new() });
        let (p2, session) = (p.clone(), self.session.clone());
        spawn_bg(
            move || -> Result<Value, String> {
                let (code, body) = api::request("GET", "/api/admin/db/tables", "", None, Some(&session))?;
                if code >= 400 {
                    return Err(if code == 403 { "Bu bo'lim faqat admin uchun".into() } else { format!("Jurnallar olinmadi ({code})") });
                }
                serde_json::from_str::<Value>(&body).map_err(|_| "Javob noto'g'ri".to_string())
            },
            move |r| {
                match r {
                    Ok(j) => p2.items.set(Rc::new(j["items"].as_array().map(|a| a.iter().map(|o| (o["name"].as_str().unwrap_or("").to_string(), o["kind"].as_str().unwrap_or("").to_string())).collect()).unwrap_or_default())),
                    Err(e) => p2.error.set(e),
                }
                p2.loaded.set(true);
            },
        );
        self.push(pages::Page::AdminDb(DbContent::Tables(p)));
    }

    fn open_db_rows(self: &Rc<Self>, table: String) {
        let p = Rc::new(DbRows {
            table,
            cols: State::new(Rc::new(Vec::new())),
            rows: State::new(Rc::new(Vec::new())),
            has_more: State::new(false),
            order: State::new(String::new()),
            desc: State::new(true),
            fcol: State::new(String::new()),
            input: State::new(String::new()),
            applied: State::new(String::new()),
            zoom: State::new(1.0),
            loading: State::new(true),
            more_busy: std::cell::Cell::new(false),
            error: State::new(String::new()),
            detail: State::new(None),
            menu: State::new(false),
            hscroll: ScrollState::new(),
            vscroll: ScrollState::new(),
            dscroll: ScrollState::new(),
            mscroll: ScrollState::new(),
        });
        self.db_load(&p, true);
        self.push(pages::Page::AdminDb(DbContent::Rows(p)));
    }

    /// `reset` — boshidan (tartib/qidiruv o'zgarganda), aks holda keyingi 100 ta yozuv qo'shiladi.
    fn db_load(self: &Rc<Self>, p: &Rc<DbRows>, reset: bool) {
        if !reset && (p.more_busy.get() || p.loading.get() || !p.has_more.get()) {
            return;
        }
        let offset = if reset { 0 } else { p.rows.get().len() };
        if reset {
            p.loading.set(true);
            p.error.set(String::new());
        } else {
            p.more_busy.set(true);
        }
        let applied = p.applied.get();
        let q = format!(
            "?table={}&offset={offset}&limit={STEP}&order={}&dir={}&col={}&q={}",
            crate::backend::urlenc(&p.table),
            crate::backend::urlenc(&p.order.get()),
            if p.desc.get() { "desc" } else { "asc" },
            crate::backend::urlenc(&if applied.is_empty() { String::new() } else { p.fcol.get() }),
            crate::backend::urlenc(&applied)
        );
        let (p2, session) = (p.clone(), self.session.clone());
        spawn_bg(
            move || -> Result<Value, String> {
                let (code, body) = api::request("GET", "/api/admin/db/rows", &q, None, Some(&session))?;
                let j: Value = serde_json::from_str(&body).map_err(|_| "Javob noto'g'ri".to_string())?;
                if code >= 400 {
                    return Err(j["error"].as_str().map(String::from).unwrap_or_else(|| format!("Yozuvlar olinmadi ({code})")));
                }
                Ok(j)
            },
            move |r| {
                p2.more_busy.set(false);
                match r {
                    Ok(j) => {
                        let cols: Vec<Col> = j["columns"].as_array().map(|a| a.iter().map(|o| Col { name: o["name"].as_str().unwrap_or("").into(), ty: o["type"].as_str().unwrap_or("").into(), pk: o["pk"].as_bool().unwrap_or(false) }).collect()).unwrap_or_default();
                        let n = cols.len();
                        let new: Vec<Vec<Option<String>>> = j["rows"]
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .map(|r| {
                                        (0..n)
                                            .map(|c| match r.get(c) {
                                                None | Some(Value::Null) => None,
                                                Some(Value::String(s)) => Some(s.clone()),
                                                Some(v) => Some(v.to_string()),
                                            })
                                            .collect()
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        if reset {
                            p2.rows.set(Rc::new(new));
                            p2.vscroll.scroll_to_top();
                        } else {
                            let mut all = (*p2.rows.get()).clone();
                            all.extend(new);
                            p2.rows.set(Rc::new(all));
                        }
                        if p2.fcol.get().is_empty() {
                            p2.fcol.set(cols.first().map(|c| c.name.clone()).unwrap_or_default());
                        }
                        p2.cols.set(Rc::new(cols));
                        p2.has_more.set(j["has_more"].as_bool().unwrap_or(false));
                    }
                    Err(e) => {
                        if reset {
                            p2.rows.set(Rc::new(Vec::new()));
                        }
                        p2.error.set(e);
                    }
                }
                p2.loading.set(false);
            },
        );
    }

    pub(super) fn admin_db_page(self: &Rc<Self>, c: &DbContent) -> Element {
        match c {
            DbContent::Tables(p) => self.db_tables_page(p),
            DbContent::Rows(p) => self.db_rows_page(p),
        }
    }

    fn db_tables_page(self: &Rc<Self>, p: &Rc<DbTables>) -> Element {
        let items = p.items.get();
        let f = p.filter.get().trim().to_lowercase();
        let shown: Vec<(String, String)> = items.iter().filter(|(n, _)| f.is_empty() || n.to_lowercase().contains(&f)).cloned().collect();
        let list: Element = if !p.loaded.get() {
            column(vec![spinner(32.0, 3.0, ACCENT2)]).center().grow(1.0)
        } else if !p.error.get().is_empty() {
            column(vec![tx(p.error.get(), 14.0, Color::rgba(255, 255, 255, 179))]).center().grow(1.0)
        } else if shown.is_empty() {
            column(vec![tx("Jurnal topilmadi", 14.0, Color::rgba(255, 255, 255, 115))]).center().grow(1.0)
        } else {
            let me = self.clone();
            let n = shown.len();
            let shown = Rc::new(shown);
            let sh = shown.clone();
            column(vec![
                tx(format!("{n} ta jurnal \u{b7} faqat o'qish"), 12.0, TEXT_FAINT).margin_each(0.0, 0.0, 0.0, 6.0),
                lazy_column(&p.scroll, n, 56.0, move |i| {
                    let (name, kind) = sh[i].clone();
                    let mut kids = vec![tx(name.clone(), 14.5, Color::WHITE).bold().weight(1.0)];
                    if kind == "view" {
                        kids.push(tx("ko'rinish", 11.0, ACCENT));
                    }
                    let me2 = me.clone();
                    row(kids).align(Align::Center).bg(CARD).border(1.0, BORDER).radius(14.0).padding_xy(14.0, 14.0).margin_each(0.0, 0.0, 0.0, 8.0).on_click(move || me2.open_db_rows(name.clone()))
                })
                .grow(1.0),
            ])
            .grow(1.0)
        };
        let body = column(vec![input_style(&p.filter, "Jurnal nomini qidiring").margin_each(0.0, 0.0, 0.0, 10.0), list]).padding_each(16.0, 4.0, 16.0, 0.0);
        self.scaffold("Ma'lumotlar bazasi", body)
    }

    fn db_rows_page(self: &Rc<Self>, p: &Rc<DbRows>) -> Element {
        let ins = safe_insets();
        let (sw, sh) = window_dp();
        let cols = p.cols.get();
        let rows = p.rows.get();
        let zoom = p.zoom.get();
        let mut top: Vec<Element> = Vec::new();
        if !cols.is_empty() {
            let (pm, pi, pa, me) = (p.menu.clone(), p.input.clone(), p.applied.clone(), self.clone());
            let pc = p.clone();
            let colbtn = tx(format!("{} \u{25be}", if p.fcol.get().is_empty() { "ustun".to_string() } else { p.fcol.get() }), 12.0, Color::WHITE).bg(CARD_ALT).radius(10.0).padding_xy(10.0, 12.0).on_click(move || pm.set(true));
            let search = tx("Qidirish", 13.0, ACCENT).bold().padding_xy(10.0, 12.0).on_click(move || {
                pa.set(pi.get().trim().to_string());
                me.db_load(&pc, true);
            });
            top.push(row(vec![colbtn, input_style(&p.input, "qiymat bo'yicha qidirish").weight(1.0), search]).gap(8.0).align(Align::Center).margin_each(0.0, 0.0, 0.0, 4.0));
            let (pz1, pz2, pz3) = (p.zoom.clone(), p.zoom.clone(), p.zoom.clone());
            let zb = |t: &str, f: Box<dyn Fn()>| tx(t, 13.0, ACCENT).bold().padding_xy(10.0, 6.0).on_click(f);
            top.push(
                row(vec![
                    tx(format!("{}{} yozuv \u{b7} {} ustun \u{b7} {}%", rows.len(), if p.has_more.get() { "+" } else { "" }, cols.len(), (zoom * 100.0) as i32), 12.0, TEXT_FAINT).weight(1.0),
                    zb("\u{2212}", Box::new(move || pz1.set((pz1.get() - 0.2).max(0.4)))),
                    zb("+", Box::new(move || pz2.set((pz2.get() + 0.2).min(3.0)))),
                    zb("100%", Box::new(move || pz3.set(1.0))),
                ])
                .align(Align::Center),
            );
        }
        let table: Element = if p.loading.get() && rows.is_empty() {
            column(vec![spinner(32.0, 3.0, ACCENT2)]).center().grow(1.0)
        } else if !p.error.get().is_empty() && rows.is_empty() {
            column(vec![tx(p.error.get(), 14.0, Color::rgba(255, 255, 255, 179))]).center().grow(1.0)
        } else if rows.is_empty() {
            column(vec![tx(if p.applied.get().is_empty() { "Jurnal bo'sh" } else { "Hech narsa topilmadi" }, 14.0, Color::rgba(255, 255, 255, 115))]).center().grow(1.0)
        } else {
            let idx_w = 56.0 * zoom;
            let widths: Vec<f32> = cols.iter().map(|c| col_base(c) * zoom).collect();
            let total_w = idx_w + widths.iter().sum::<f32>();
            let fs = 12.0 * zoom;
            let pad = 8.0 * zoom;
            let row_h = fs * 1.3 + pad * 2.0;
            let cell = |t: String, w: f32, color: Color| {
                let t: String = t.chars().take(((w / (fs * 0.5)) as usize).max(4)).map(|c| if c == '\n' || c == '\r' { ' ' } else { c }).collect();
                column(vec![tx(Self::ellipsize(&t, fs, w - pad * 2.0), fs, color)]).width(w).height(row_h).padding_xy(pad, pad).border(0.5, BORDER)
            };
            // Sarlavha: ustun nomi (bosilsa shu bo'yicha tartiblanadi, yana bossa yo'nalish almashadi).
            let mut hcells: Vec<Element> = vec![cell("#".into(), idx_w, TEXT_FAINT)];
            for (i, c) in cols.iter().enumerate() {
                let on = p.order.get() == c.name;
                let label = format!("{}{}{}", if c.pk { "\u{1f511} " } else { "" }, c.name, if on { if p.desc.get() { " \u{2193}" } else { " \u{2191}" } } else { "" });
                let (pp, me, cn) = (p.clone(), self.clone(), c.name.clone());
                hcells.push(cell(label, widths[i], if on { ACCENT } else { Color::WHITE }).bg(CARD_ALT).on_click(move || {
                    if pp.order.get() == cn {
                        pp.desc.set(!pp.desc.get());
                    } else {
                        pp.order.set(cn.clone());
                        pp.desc.set(true);
                    }
                    me.db_load(&pp, true);
                }));
            }
            let header = row(hcells).width(total_w).height(row_h);
            let table_h = (sh - (12.0 + ins.top + 74.0 + 8.0) - 100.0 - row_h - ins.bottom - 8.0).max(120.0);
            let n = rows.len();
            let (me, pp, rs, cs) = (self.clone(), p.clone(), rows.clone(), cols.clone());
            let body = lazy_column(&p.vscroll, n, row_h, move |i| {
                if i + 3 >= n && pp.has_more.get() {
                    me.db_load(&pp, false);
                }
                let r = &rs[i];
                let mut cells: Vec<Element> = vec![cell((i + 1).to_string(), idx_w, TEXT_FAINT)];
                for (c, col) in cs.iter().enumerate() {
                    let v = r.get(c).cloned().flatten();
                    cells.push(cell(v.clone().unwrap_or_else(|| "NULL".into()), col_base(col) * zoom, if v.is_none() { TEXT_FAINT } else { Color::WHITE }));
                }
                let pd = pp.clone();
                row(cells).width(total_w).bg(if i % 2 == 0 { BG } else { CARD }).on_click(move || pd.detail.set(Some(i)))
            })
            .width(total_w)
            .height(table_h);
            row(vec![column(vec![header, body]).width(total_w)]).scroll(&p.hscroll).height(table_h + row_h).margin_each(0.0, 4.0, 0.0, 0.0)
        };
        let _ = sw;
        let mut layers = vec![self.scaffold(&p.table, column(top.into_iter().chain(std::iter::once(table)).collect()).padding_each(16.0, 4.0, 16.0, 0.0))];
        // Ustun tanlash.
        if p.menu.get() {
            let pc = p.clone();
            let mut kids: Vec<Element> = vec![tx("Qidiriladigan ustun", 16.0, Color::WHITE).bold().margin_each(0.0, 0.0, 0.0, 8.0)];
            for c in cols.iter() {
                let (pp, name) = (p.clone(), c.name.clone());
                kids.push(tx(c.name.clone(), 14.0, if p.fcol.get() == c.name { ACCENT } else { Color::WHITE }).padding_xy(4.0, 10.0).on_click(move || {
                    pp.fcol.set(name.clone());
                    pp.menu.set(false);
                }));
            }
            let panel = scroll_column(&p.mscroll, kids).bg(CARD).border(1.0, BORDER).radius(20.0).padding(18.0).margin_each(24.0, 80.0, 24.0, 80.0);
            layers.push(column(vec![panel]).center().bg(Color::rgba(0, 0, 0, 120)).on_click(move || pc.menu.set(false)));
        }
        // Yozuv tafsiloti.
        if let Some(i) = p.detail.get() {
            if let Some(r) = rows.get(i) {
                let pc = p.clone();
                let mut kids: Vec<Element> = vec![tx(p.table.clone(), 16.0, Color::WHITE).bold()];
                for (c, col) in cols.iter().enumerate() {
                    let v = r.get(c).cloned().flatten();
                    kids.push(tx(format!("{}{}", col.name, if col.ty.is_empty() { String::new() } else { format!(" \u{b7} {}", col.ty.to_lowercase()) }), 11.0, TEXT_FAINT).margin_each(0.0, 8.0, 0.0, 0.0));
                    kids.push(tx(v.clone().unwrap_or_else(|| "NULL".into()), 13.0, if v.is_none() { TEXT_FAINT } else { Color::WHITE }));
                }
                let pc2 = p.clone();
                let panel = column(vec![scroll_column(&p.dscroll, kids).weight(1.0), row(vec![spacer(), tx("Yopish", 14.0, ACCENT).bold().padding_xy(10.0, 10.0).on_click(move || pc2.detail.set(None))])])
                    .bg(CARD)
                    .border(1.0, BORDER)
                    .radius(20.0)
                    .padding(18.0)
                    .margin_each(20.0, 80.0, 20.0, 80.0);
                layers.push(column(vec![panel]).bg(Color::rgba(0, 0, 0, 120)).on_click(move || pc.detail.set(None)));
            }
        }
        stack(layers)
    }
}
