//! Admin: kodlash navbati va jonli holat (Kotlin `AdminEncodeScreen`). Har 4 soniyada `/api/encode/live`, navbat — kamroq.

use super::*;
use crate::api;
use serde_json::Value;

pub struct EncodePage {
    live: State<Option<Rc<Value>>>,
    queue: State<Option<Rc<Value>>>,
    error: State<String>,
    ticks: std::cell::Cell<u32>,
    last_job: std::cell::RefCell<String>,
    scroll: ScrollState,
}

fn f(v: &Value, k: &str) -> f64 {
    v[k].as_f64().or_else(|| v[k].as_str().and_then(|s| s.parse().ok())).unwrap_or(f64::NAN)
}

fn n(v: f64, digits: usize, suffix: &str) -> String {
    if v.is_nan() { "—".into() } else { format!("{v:.digits$}{suffix}") }
}

fn clock(sec: f64) -> String {
    if sec.is_nan() || sec < 0.0 {
        return "—".into();
    }
    let s = sec as i64;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60) } else { format!("{:02}:{:02}", s / 60, s % 60) }
}

fn arr(v: &Value) -> Vec<Value> {
    v.as_array().cloned().unwrap_or_default()
}

impl Home {
    pub(super) fn open_admin_encode(self: &Rc<Self>) {
        uzcoders::prelude::breadcrumb("open_admin_encode");
        let p = Rc::new(EncodePage { live: State::new(None), queue: State::new(None), error: State::new(String::new()), ticks: std::cell::Cell::new(0), last_job: std::cell::RefCell::new(String::new()), scroll: ScrollState::new() });
        self.push(pages::Page::AdminEncode(p.clone()));
        self.encode_poll(&p);
    }

    fn encode_poll(self: &Rc<Self>, p: &Rc<EncodePage>) {
        let (me, p2, session) = (self.clone(), p.clone(), self.session.clone());
        let want_queue = p.queue.get().is_none() || p.ticks.get() % 15 == 0;
        let last_job = p.last_job.borrow().clone();
        spawn_bg(
            move || {
                let live = api::get_json("/api/encode/live", "", Some(&session));
                let job = live.as_ref().ok().map(|l| format!("{}:{}:{}", l["status"]["anime_id"], l["status"]["season_id"], l["status"]["epizod_id"])).unwrap_or_default();
                let queue = if want_queue || job != last_job { Some(api::get_json("/api/encode/admin", "", Some(&session))) } else { None };
                (live, queue, job)
            },
            move |(live, queue, job)| {
                match live {
                    Ok(l) => {
                        p2.live.set(Some(Rc::new(l)));
                        p2.error.set(String::new());
                    }
                    Err(e) => {
                        if p2.queue.get().is_none() {
                            p2.error.set(e);
                        }
                    }
                }
                if let Some(Ok(q)) = queue {
                    p2.queue.set(Some(Rc::new(q)));
                    *p2.last_job.borrow_mut() = job;
                }
                p2.ticks.set(p2.ticks.get() + 1);
                if !me.pages.get().iter().any(|pg| matches!(pg, pages::Page::AdminEncode(q) if Rc::ptr_eq(q, &p2))) {
                    return;
                }
                let (me2, p3) = (me.clone(), p2.clone());
                spawn_bg(|| std::thread::sleep(std::time::Duration::from_secs(4)), move |_| {
                    if me2.pages.get().iter().any(|pg| matches!(pg, pages::Page::AdminEncode(q) if Rc::ptr_eq(q, &p3))) {
                        me2.encode_poll(&p3);
                    }
                });
            },
        );
    }

    fn enc_card(title: &str, trailing: &str, body: Vec<Element>) -> Element {
        let mut head = vec![tx(title, 14.0, Color::WHITE).bold().weight(1.0)];
        if !trailing.is_empty() {
            head.push(tx(trailing, 11.0, Color::WHITE));
        }
        let mut col = vec![row(head).align(Align::Center)];
        col.push(gap_box(0.0, 10.0));
        col.extend(body);
        Self::pglass(column(col).padding(14.0)).radius(22.0).margin_each(0.0, 0.0, 0.0, 12.0)
    }

    fn enc_stats(items: Vec<(&str, String)>) -> Element {
        // Ikki ustunli to'r.
        let mut rows: Vec<Element> = Vec::new();
        for ch in items.chunks(2) {
            let cells: Vec<Element> = ch
                .iter()
                .map(|(k, v)| column(vec![tx(*k, 10.0, Color::WHITE), tx(v.clone(), 12.5, Color::WHITE).bold()]).bg(Color::rgba(255, 255, 255, 18)).radius(10.0).padding_xy(10.0, 6.0).weight(1.0))
                .collect();
            let mut r = cells;
            if r.len() == 1 {
                r.push(column(vec![]).weight(1.0));
            }
            rows.push(row(r).gap(8.0).margin_each(0.0, 0.0, 0.0, 8.0));
        }
        column(rows)
    }

    fn enc_bar(pct: f64) -> Element {
        let frac = (pct / 100.0).clamp(0.0, 1.0) as f32;
        row(vec![
            stack(vec![column(vec![]).height(8.0).radius(4.0).bg(Color::rgba(255, 255, 255, 30)), row(vec![column(vec![]).height(8.0).radius(4.0).bg(ACCENT2).weight(frac.max(0.001)), column(vec![]).weight((1.0 - frac).max(0.001))])]).weight(1.0),
            tx(n(pct, 1, "%"), 12.0, Color::WHITE).bold(),
        ])
        .gap(8.0)
        .align(Align::Center)
    }

    fn enc_queue_row(j: &Value, error: bool) -> Element {
        let name = j["nomi"].as_str().filter(|s| !s.is_empty()).or_else(|| j["anime_name"].as_str()).unwrap_or("").to_string();
        let mut col = vec![tx(name, 13.0, Color::WHITE).bold(), tx(format!("{}-bo'lim · {}-qism", j["bolim_id"], j["epizod_number"]), 12.0, Color::WHITE)];
        if error {
            if let Some(t) = j["text"].as_str().filter(|s| !s.is_empty()) {
                col.push(tx(t, 11.0, ERROR));
            }
        }
        column(col).padding_xy(0.0, 5.0)
    }

    pub(super) fn admin_encode_page(self: &Rc<Self>, p: &Rc<EncodePage>) -> Element {
        let q = p.queue.get();
        let live = p.live.get();
        if q.is_none() && live.is_none() {
            return self.scaffold("Kodlash navbati", if p.error.get().is_empty() { column(vec![spinner(32.0, 3.0, ACCENT2)]).center() } else { column(vec![tx(p.error.get(), 14.0, ERROR)]).center() });
        }
        let st = live.as_ref().map(|l| l["status"].clone()).unwrap_or(Value::Null);
        let data = st["data"].clone();
        let (cur, xfer, src, sys) = (&data["cur"], &data["xfer"], &data["src"], &data["sys"]);
        let updated = st["updated_at"].as_i64().unwrap_or(0);
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
        let age = if updated > 0 { (now - updated) / 1000 } else { -1 };
        let mut kids: Vec<Element> = Vec::new();
        let (txt, col) = if (0..120).contains(&age) { (format!("• Jonli · {age}s"), Color::hex(0x4ADE80)) } else if age >= 0 { (format!("• Eski holat · {age}s"), GOLD) } else { ("• Holat yo'q".to_string(), GOLD) };
        kids.push(tx(txt, 12.0, col).margin_each(0.0, 0.0, 0.0, 8.0));

        if cur.is_object() {
            let pct = f(cur, "pct");
            let frames = src["frames"].as_i64().filter(|v| *v > 0).map(|v| format!(" / {v}")).unwrap_or_default();
            kids.push(Self::enc_card(
                &format!("Kodlanmoqda: {}", cur["q"].as_str().unwrap_or("")),
                "",
                vec![
                    Self::enc_bar(if pct.is_nan() { 0.0 } else { pct }),
                    tx(format!("Kodlandi: {} / {}", clock(f(cur, "out_s")), clock(f(cur, "dur_s"))), 13.0, Color::WHITE).bold().padding_each(0.0, 8.0, 0.0, 8.0),
                    Self::enc_stats(vec![
                        ("Kadr", format!("{}{frames}", cur["frame"].as_i64().unwrap_or(0))),
                        ("Kadr/s", n(f(cur, "fps"), 1, "")),
                        ("Tezlik", n(f(cur, "speed"), 2, "x")),
                        ("Bitreyt", n(f(cur, "bitrate_kbps"), 0, " kb/s")),
                        ("Hajm", n(f(cur, "size_mb"), 1, " MB")),
                        ("Taxminiy hajm", n(f(cur, "est_mb"), 0, " MB")),
                        ("O'tdi", clock(f(cur, "elapsed"))),
                        ("Qoldi", format!("~{}", clock(f(cur, "eta")))),
                        ("Kvantizator (q)", n(f(cur, "qp"), 1, "")),
                        ("Tashlangan / takror", format!("{} / {}", cur["drop"].as_i64().unwrap_or(0), cur["dup"].as_i64().unwrap_or(0))),
                    ]),
                ],
            ));
        } else if xfer.is_object() {
            let pct = f(xfer, "pct");
            kids.push(Self::enc_card(
                xfer["kind"].as_str().filter(|s| !s.is_empty()).unwrap_or("Yuklash"),
                "",
                vec![
                    Self::enc_bar(if pct.is_nan() { 0.0 } else { pct }),
                    gap_box(0.0, 8.0),
                    Self::enc_stats(vec![
                        ("Hajm", format!("{} / {}", n(f(xfer, "cur_mb"), 1, ""), n(f(xfer, "total_mb"), 1, " MB"))),
                        ("Tezlik", n(f(xfer, "mbps"), 2, " MB/s")),
                        ("O'tdi", clock(f(xfer, "elapsed"))),
                        ("Qoldi", format!("~{}", clock(f(xfer, "eta")))),
                    ]),
                ],
            ));
        } else {
            kids.push(Self::enc_card("Joriy bosqich", "", vec![tx(st["progress"].as_str().filter(|s| !s.is_empty()).unwrap_or("Hozir kodlanayotgan qism yo'q"), 13.0, Color::WHITE)]));
        }

        let ladder = arr(&data["ladder"]);
        if !ladder.is_empty() {
            let done = ladder.iter().filter(|r| r["state"] == "done").count();
            let rows: Vec<Element> = ladder
                .iter()
                .map(|r| {
                    let s = r["state"].as_str().unwrap_or("");
                    let mut parts: Vec<String> = vec![match s { "enc" => "kodlanmoqda", "upload" => "yuklanmoqda", "done" => "tayyor", _ => "kutmoqda" }.into()];
                    if !r["crf"].is_null() { parts.push(format!("CRF {}", r["crf"])); }
                    if !r["size_mb"].is_null() { parts.push(format!("{} MB", r["size_mb"])); }
                    if !r["kbps"].is_null() { parts.push(format!("{} kb/s", r["kbps"])); }
                    if !r["enc_s"].is_null() { parts.push(clock(f(r, "enc_s"))); }
                    row(vec![
                        tx(match s { "done" => "+", "enc" | "upload" => "~", _ => "-" }, 14.0, if s == "done" { Color::hex(0x4ADE80) } else { ACCENT2 }).width(22.0),
                        tx(r["q"].as_str().unwrap_or(""), 13.0, Color::WHITE).bold().width(52.0),
                        tx(parts.join(" · "), 12.0, Color::WHITE),
                    ])
                    .padding_xy(0.0, 3.0)
                })
                .collect();
            kids.push(Self::enc_card("Sifatlar", &format!("{done}/{} tayyor", ladder.len()), rows));
        }
        if src.is_object() {
            let mut items = vec![
                ("O'lcham", format!("{}{}p", src["w"].as_i64().filter(|w| *w > 0).map(|w| format!("{w}×")).unwrap_or_default(), src["h"])),
                ("Davomiyligi", clock(f(src, "dur_s"))),
                ("Hajm", n(f(src, "mb"), 1, " MB")),
                ("Bitreyt", n(f(src, "kbps"), 0, " kb/s")),
            ];
            if !src["fps"].is_null() { items.push(("Kadr/s", n(f(src, "fps"), 2, ""))); }
            if let Some(c) = src["codec"].as_str().filter(|s| !s.is_empty()) { items.push(("Kodek", c.to_string())); }
            kids.push(Self::enc_card("Asl video", "", vec![Self::enc_stats(items)]));
        }
        if sys.is_object() {
            kids.push(Self::enc_card(
                "Kompyuter (GitHub runner)",
                "",
                vec![Self::enc_stats(vec![
                    ("CPU", n(f(sys, "cpu"), 1, "%")),
                    ("Yadro", sys["cores"].to_string()),
                    ("Yuklama", n(f(sys, "load"), 2, "")),
                    ("RAM", if sys["ram_total_mb"].as_i64().unwrap_or(0) > 0 { format!("{} / {} MB", sys["ram_used_mb"], sys["ram_total_mb"]) } else { "-".into() }),
                    ("Bo'sh disk", n(f(sys, "disk_free_gb"), 1, " GB")),
                ])],
            ));
        }
        let lines: Vec<String> = arr(&st["lines"]).iter().filter_map(|l| l.as_str().map(String::from)).filter(|l| !l.trim().is_empty()).collect();
        let log = if lines.is_empty() { tx(live.as_ref().and_then(|l| l["status_error"].as_str()).filter(|s| !s.is_empty()).unwrap_or("Log yo'q"), 12.0, Color::WHITE) } else { tx(lines[lines.len().saturating_sub(60)..].join("\n"), 10.0, Color::WHITE).bg(Color::rgba(0, 0, 0, 115)).radius(10.0).padding(10.0) };
        kids.push(Self::enc_card("Encode log", "", vec![log]));
        if let Some(g) = live.as_ref().and_then(|l| if l["gh"].is_object() { Some(l["gh"].clone()) } else if l["github"].is_object() { Some(l["github"].clone()) } else { None }) {
            let url = g["url"].as_str().filter(|s| !s.is_empty()).or_else(|| g["html_url"].as_str()).unwrap_or("").to_string();
            let mut body: Vec<Element> = Vec::new();
            if !url.is_empty() {
                let u = url.clone();
                body.push(tx(url, 12.0, ACCENT2).on_click(move || uzcoders::android::open_url(&u)));
            }
            for s in arr(&g["steps"]) {
                body.push(tx(format!("{} — {}", s["name"].as_str().unwrap_or(""), s["status"].as_str().unwrap_or("")), 11.0, Color::WHITE).padding_each(0.0, 2.0, 0.0, 0.0));
            }
            kids.push(Self::enc_card("GitHub run", g["status"].as_str().unwrap_or(""), body));
        }
        if let Some(q) = q {
            let (running, queued, errors) = (arr(&q["running"]), arr(&q["queue"]), arr(&q["errors"]));
            let list = |v: &Vec<Value>, empty: &str, err: bool| -> Vec<Element> { if v.is_empty() { vec![tx(empty, 12.0, Color::WHITE)] } else { v.iter().map(|j| Self::enc_queue_row(j, err)).collect() } };
            kids.push(Self::enc_card(&format!("Hozir ({})", running.len()), "", list(&running, "Yo'q", false)));
            kids.push(Self::enc_card(&format!("Navbatda ({})", queued.len()), "", list(&queued, "Navbat bo'sh", false)));
            if !errors.is_empty() {
                kids.push(Self::enc_card(&format!("Xatolar ({})", errors.len()), "", list(&errors, "", true)));
            }
        }
        kids.push(gap_box(0.0, 32.0));
        self.scaffold("Kodlash navbati", scroll_column(&p.scroll, kids).padding_each(16.0, 4.0, 16.0, 0.0))
    }
}
