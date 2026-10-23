//! Obuna va balans (Kotlin `BillingScreen`): balans/obuna paneli, "To'ldirish | Obuna | Tarix" — hammasi shisha.

use super::*;
use crate::api;
use crate::data::{format_count, fmt_date};
use serde_json::{json, Value};

const B_PREMIUM: &str = "M5,16L3,5l5.5,5L12,4l3.5,6L21,5l-2,11H5zM19,19c0,0.6 -0.4,1 -1,1H6c-0.6,0 -1,-0.4 -1,-1v-1h14v1z";
const B_ADD: &str = "M19,3H5C3.89,3 3,3.9 3,5v14c0,1.1 0.89,2 2,2h14c1.1,0 2,-0.9 2,-2V5C21,3.9 20.1,3 19,3zM18,13h-5v5h-2v-5H6v-2h5V6h2v5h5V13z";
const GREEN: Color = Color::rgb(0x4A, 0xDE, 0x80);

#[derive(Clone, Default)]
struct Plan {
    days: i64,
    price: i64,
}
#[derive(Clone, Default)]
struct PayLink {
    order_id: String,
    amount: i64,
    url: String,
    expires_at: i64,
}
#[derive(Clone, Default)]
struct Entry {
    kind: String,
    amount: i64,
    days: i64,
    created_at: i64,
}
#[derive(Clone, Default)]
pub struct Billing {
    balance: i64,
    until: i64,
    plans: Vec<Plan>,
    links: Vec<PayLink>,
    history: Vec<Entry>,
    loaded: bool,
    error: String,
}

pub struct BillingPage {
    data: State<Rc<Billing>>,
    tab: State<usize>,
    amount: State<String>,
    now: State<i64>,
    busy: State<bool>,
    confirm: State<i64>,
    msg: State<String>,
    scrolls: [ScrollState; 3],
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn num(v: &Value) -> i64 {
    v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)).or_else(|| v.as_str().and_then(|s| s.parse().ok())).unwrap_or(0)
}

/// `15 000 so'm`
pub fn format_sum(a: i64) -> String {
    let n = a.abs().to_string();
    let mut b = String::new();
    for (i, c) in n.chars().enumerate() {
        if i > 0 && (n.len() - i) % 3 == 0 {
            b.push(' ');
        }
        b.push(c);
    }
    format!("{}{b} so'm", if a < 0 { "-" } else { "" })
}

fn plan_label(days: i64) -> String {
    if days >= 365 {
        "12 oylik".into()
    } else if days > 0 && days % 30 == 0 {
        format!("{} oylik", days / 30)
    } else {
        format!("{days} kunlik")
    }
}

/// `2 kun 05:12:33` yoki `05:12:33`.
pub fn format_left(ms: i64) -> String {
    if ms <= 0 {
        return "tugadi".into();
    }
    let s = ms / 1000;
    let d = s / 86400;
    let clock = format!("{:02}:{:02}:{:02}", (s / 3600) % 24, (s / 60) % 60, s % 60);
    if d > 0 { format!("{d} kun {clock}") } else { clock }
}

fn discount_of(plan: &Plan, all: &[Plan]) -> i64 {
    let mut base = match all.first() {
        Some(b) => b.clone(),
        None => return 0,
    };
    if plan.days <= 0 {
        return 0;
    }
    for p in all {
        if p.days > 0 && p.days < base.days {
            base = p.clone();
        }
    }
    if base.days <= 0 || base.price <= 0 || plan.days <= base.days {
        return 0;
    }
    (((1.0 - (plan.price as f64 / plan.days as f64) / (base.price as f64 / base.days as f64)) * 100.0).round() as i64).max(0)
}

fn fetch_billing(session: &str) -> Result<Billing, String> {
    let j = api::get_json("/api/billing", "", Some(session))?;
    let arr = |k: &str| j[k].as_array().cloned().unwrap_or_default();
    Ok(Billing {
        balance: num(&j["balance"]),
        until: num(&j["subscription_until"]),
        plans: arr("plans").iter().map(|o| Plan { days: num(&o["days"]), price: num(&o["price"]) }).filter(|p| p.days > 0).collect(),
        links: arr("links")
            .iter()
            .map(|o| PayLink { order_id: o["order_id"].as_str().unwrap_or("").into(), amount: num(&o["amount"]), url: o["pay_url"].as_str().unwrap_or("").into(), expires_at: num(&o["expires_at"]) })
            .filter(|l| !l.url.is_empty())
            .collect(),
        history: arr("history").iter().map(|o| Entry { kind: o["kind"].as_str().unwrap_or("").into(), amount: num(&o["amount"]), days: num(&o["days"]), created_at: num(&o["created_at"]) }).collect(),
        loaded: true,
        error: String::new(),
    })
}

/// POST + xato matni (`{"error": ...}`).
fn post(session: &str, path: &str, body: Value) -> Result<Value, String> {
    let (code, text) = api::request("POST", path, "", Some(&body), Some(session)).map_err(|_| "Internet yo'q — qaytadan urinib ko'ring".to_string())?;
    let j: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if code >= 400 {
        return Err(j["error"].as_str().map(String::from).unwrap_or_else(|| format!("Xato ({code})")));
    }
    Ok(j)
}

impl Home {
    pub(super) fn open_billing(self: &Rc<Self>) {
        let p = Rc::new(BillingPage {
            data: State::new(Rc::new(Billing::default())),
            tab: State::new(0),
            amount: State::new(String::new()),
            now: State::new(now_ms()),
            busy: State::new(false),
            confirm: State::new(0),
            msg: State::new(String::new()),
            scrolls: [ScrollState::new(), ScrollState::new(), ScrollState::new()],
        });
        self.bill_reload(&p);
        self.bill_tick(&p);
        self.push(pages::Page::Billing(p));
    }

    fn bill_open(&self, p: &Rc<BillingPage>) -> bool {
        matches!(self.pages.get().last(), Some(pages::Page::Billing(q)) if Rc::ptr_eq(q, p))
    }

    fn bill_reload(self: &Rc<Self>, p: &Rc<BillingPage>) {
        let (p2, session) = (p.clone(), self.session.clone());
        spawn_bg(move || fetch_billing(&session), move |r| match r {
            Ok(b) => {
                if p2.tab.get() == 0 && b.balance > 0 && !p2.data.get().loaded {
                    p2.tab.set(1);
                }
                p2.data.set(Rc::new(b));
            }
            Err(e) => {
                let mut b = (*p2.data.get()).clone();
                b.error = e;
                p2.data.set(Rc::new(b));
            }
        });
    }

    /// Har soniyada qolgan vaqt yangilanadi; to'lov havolalari fonda tekshiriladi (avval tez, keyin sekin).
    fn bill_tick(self: &Rc<Self>, p: &Rc<BillingPage>) {
        let (me, p2) = (self.clone(), p.clone());
        let n = Rc::new(std::cell::Cell::new(0u32));
        fn step(me: Rc<Home>, p: Rc<BillingPage>, n: Rc<std::cell::Cell<u32>>) {
            let (me2, p2, n2) = (me.clone(), p.clone(), n.clone());
            spawn_bg(|| std::thread::sleep(std::time::Duration::from_secs(1)), move |_| {
                if !me2.bill_open(&p2) {
                    return;
                }
                p2.now.set(now_ms());
                n2.set(n2.get() + 1);
                let every = if n2.get() < 120 { 5 } else if n2.get() < 600 { 15 } else { 60 };
                if n2.get() % every == 0 {
                    let d = p2.data.get();
                    if let Some(l) = d.links.iter().find(|l| l.expires_at > now_ms()) {
                        let (oid, session, me3, p3) = (l.order_id.clone(), me2.session.clone(), me2.clone(), p2.clone());
                        spawn_bg(move || post(&session, "/api/billing/check", json!({"order_id": oid})), move |r| {
                            if matches!(r, Ok(j) if j["status"] == "paid") {
                                me3.bill_reload(&p3);
                                p3.tab.set(2);
                            }
                        });
                    }
                }
                step(me2, p2, n2);
            });
        }
        step(me, p2, n);
    }

    pub(super) fn billing_page(self: &Rc<Self>, p: &Rc<BillingPage>) -> Element {
        let d = p.data.get();
        let now = p.now.get();
        let active = d.until > now;
        let head = Self::pglass(
            row(vec![
                column(vec![tx("Balans", 12.0, Color::WHITE), tx(format_sum(d.balance), 19.0, Color::WHITE).black()]),
                spacer(),
                column(vec![
                    tx("Obuna", 12.0, Color::WHITE),
                    tx(if active { format_left(d.until - now) } else { "Yo'q".to_string() }, 14.0, if active { GREEN } else { Color::rgba(255, 255, 255, 180) }).bold(),
                ])
                .align(Align::End),
            ])
            .padding_xy(16.0, 14.0),
        )
        .radius(24.0);
        let tab = p.tab.get();
        let tabs: Vec<Element> = ["To'ldirish", "Obuna", "Tarix"]
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let t = p.tab.clone();
                let e = column(vec![tx(*name, 12.5, if i == tab { Color::WHITE } else { Color::rgba(255, 255, 255, 170) }).bold()]).center().weight(1.0).padding_xy(4.0, 11.0).radius(20.0).on_click(move || t.set(i));
                if i == tab { e.gradient(ACCENT, ACCENT2) } else { e }
            })
            .collect();
        let tab_bar = Self::pglass(row(tabs).gap(4.0).padding_xy(6.0, 6.0)).radius(28.0);
        let body = match tab {
            0 => self.bill_topup(p, &d, now),
            1 => self.bill_subscribe(p, &d, now),
            _ => self.bill_history(p, &d),
        };
        let pc = p.clone();
        let body = body.on_swipe(move |dir| pc.tab.set((pc.tab.get() as i32 + dir).clamp(0, 2) as usize));
        let mut content = vec![head.margin_each(0.0, 0.0, 0.0, 10.0), tab_bar.margin_each(0.0, 0.0, 0.0, 10.0), body.grow(1.0)];
        if !p.msg.get().is_empty() {
            let pm = p.clone();
            content.insert(2, tx(p.msg.get(), 13.0, Color::WHITE).bg(Color::rgba(0, 0, 0, 120)).radius(14.0).padding_xy(14.0, 8.0).margin_each(0.0, 0.0, 0.0, 8.0).on_click(move || pm.msg.set(String::new())));
        }
        let page = self.scaffold("Obuna va balans", column(content).padding_each(16.0, 4.0, 16.0, 0.0));
        let mut layers = vec![page];
        if p.confirm.get() > 0 {
            layers.push(self.bill_confirm(p, &d));
        }
        stack(layers)
    }

    fn placeholder(loading: bool, text: &str) -> Element {
        column(vec![if loading { spinner(28.0, 3.0, ACCENT2) } else { gap_box(0.0, 0.0) }, gap_box(0.0, 14.0), tx(text, 13.0, Color::WHITE)]).center()
    }

    fn bill_topup(self: &Rc<Self>, p: &Rc<BillingPage>, d: &Rc<Billing>, now: i64) -> Element {
        let amount_field = field(&p.amount)
            .color(Color::WHITE)
            .size(17.0)
            .hint("10000")
            .hint_color(Color::rgba(255, 255, 255, 70))
            .accent(ACCENT2)
            .keyboard(KeyboardKind::Number)
            .padding_xy(0.0, 10.0)
            .no_border()
            .weight(1.0);
        let (pp, me) = (p.clone(), self.clone());
        let go = column(vec![if p.busy.get() { spinner(20.0, 2.0, Color::WHITE) } else { tx("To'lashga o'tish", 14.0, Color::WHITE).bold() }])
            .center()
            .padding_xy(0.0, 14.0)
            .radius(26.0)
            .gradient(ACCENT, ACCENT2)
            .on_click(move || {
                if pp.busy.get() {
                    return;
                }
                let a: i64 = pp.amount.get().chars().filter(|c| c.is_ascii_digit()).take(9).collect::<String>().parse().unwrap_or(0);
                if a <= 0 {
                    pp.msg.set("Summani yozing".into());
                    return;
                }
                pp.busy.set(true);
                let (p2, me2, session) = (pp.clone(), me.clone(), me.session.clone());
                spawn_bg(move || post(&session, "/api/billing/create", json!({"amount": a})), move |r| {
                    p2.busy.set(false);
                    match r {
                        Ok(j) => {
                            p2.amount.set(String::new());
                            match j["pay_url"].as_str().filter(|u| !u.is_empty()) {
                                Some(u) => uzcoders::android::open_url(u),
                                None => p2.msg.set("Havola tayyor — pastdagi \"To'lov qilish\" tugmasini bosing".into()),
                            }
                            me2.bill_reload(&p2);
                        }
                        Err(e) => p2.msg.set(e),
                    }
                });
            });
        let mut kids = vec![
            Self::pglass(
                column(vec![
                    tx("Summani yozing", 12.5, Color::WHITE),
                    gap_box(0.0, 8.0),
                    row(vec![amount_field, tx("so'm", 14.0, Color::WHITE)]).gap(8.0).align(Align::Center).bg(Color::rgba(255, 255, 255, 18)).radius(14.0).padding_xy(14.0, 2.0),
                    gap_box(0.0, 12.0),
                    go,
                ])
                .padding_xy(16.0, 14.0),
            )
            .radius(24.0),
        ];
        let links: Vec<PayLink> = d.links.iter().filter(|l| l.expires_at > now).cloned().collect();
        if links.is_empty() {
            kids.push(tx("Summani yozing — to'lov sahifasi o'zi ochiladi.\nPul hisobingizga avtomatik tushadi, havola esa 1 soat faol turadi.", 12.0, Color::WHITE).padding_each(4.0, 8.0, 4.0, 0.0));
        }
        for l in links {
            let left = ((l.expires_at - now) / 1000).max(0);
            let url = l.url.clone();
            let (oid, session, me, pc) = (l.order_id.clone(), self.session.clone(), self.clone(), p.clone());
            kids.push(
                Self::pglass(
                    column(vec![
                        row(vec![tx(format_sum(l.amount), 16.0, Color::WHITE).black(), spacer(), tx(format!("Qolgan vaqt {:02}:{:02} daqiqa", left / 60, left % 60), 11.5, Color::WHITE)]),
                        gap_box(0.0, 12.0),
                        row(vec![
                            column(vec![tx("To'lov qilish", 13.0, Color::WHITE).bold()]).center().weight(1.0).padding_xy(0.0, 12.0).radius(22.0).gradient(ACCENT, ACCENT2).on_click(move || uzcoders::android::open_url(&url)),
                            column(vec![tx("Tekshirish", 13.0, Color::WHITE).bold()]).center().weight(1.0).padding_xy(0.0, 12.0).radius(22.0).bg(Color::rgba(255, 255, 255, 20)).border(1.0, Color::WHITE).on_click(move || {
                                let (oid, session, me, pc) = (oid.clone(), session.clone(), me.clone(), pc.clone());
                                spawn_bg(move || post(&session, "/api/billing/check", json!({"order_id": oid})), move |r| match r {
                                    Ok(j) if j["status"] == "paid" => {
                                        pc.msg.set("To'lov qabul qilindi — balans yangilandi".into());
                                        me.bill_reload(&pc);
                                    }
                                    Ok(_) => pc.msg.set("Hozircha to'lov ko'rinmadi".into()),
                                    Err(e) => pc.msg.set(e),
                                });
                            }),
                        ])
                        .gap(10.0),
                    ])
                    .padding_xy(16.0, 12.0),
                )
                .radius(24.0)
                .margin_each(0.0, 10.0, 0.0, 0.0),
            );
        }
        kids.push(gap_box(0.0, 28.0));
        scroll_column(&p.scrolls[0], kids)
    }

    fn bill_subscribe(self: &Rc<Self>, p: &Rc<BillingPage>, d: &Rc<Billing>, now: i64) -> Element {
        if d.plans.is_empty() {
            return Self::placeholder(!d.loaded && d.error.is_empty(), if d.error.is_empty() { "Tariflar yuklanmoqda..." } else { &d.error });
        }
        let locked = d.until > now;
        let mut kids: Vec<Element> = Vec::new();
        if locked {
            kids.push(
                Self::pglass(
                    row(vec![
                        icon(B_PREMIUM, 22.0).color(ACCENT2),
                        column(vec![
                            tx("Obunangiz faol", 14.0, Color::WHITE).bold(),
                            row(vec![tx("Yana ", 12.0, Color::WHITE), tx(format_left(d.until - now), 12.5, GREEN).bold()]),
                            tx("Tugagach yangi tarif tanlay olasiz.", 11.5, Color::WHITE),
                        ]),
                    ])
                    .gap(10.0)
                    .align(Align::Center)
                    .padding_xy(14.0, 12.0),
                )
                .radius(22.0)
                .margin_each(0.0, 0.0, 0.0, 10.0),
            );
        }
        for plan in d.plans.iter() {
            let disc = discount_of(plan, &d.plans);
            let mut title = vec![tx(format!("+ {}", plan_label(plan.days)), 16.0, Color::WHITE).bold()];
            if disc >= 5 {
                title.push(tx(format!("{disc}% chegirma"), 10.5, GREEN).bold().bg(GREEN.with_opacity(0.18)).radius(8.0).padding_xy(7.0, 3.0));
            }
            let (pp, days, price, balance) = (p.clone(), plan.days, plan.price, d.balance);
            kids.push(
                Self::pglass(
                    row(vec![
                        column(vec![row(title).gap(8.0).align(Align::Center), tx(format_sum(plan.price), 13.0, Color::WHITE)]).weight(1.0),
                        column(vec![tx("Sotib olish", 13.0, Color::WHITE).bold()]).center().padding_xy(16.0, 10.0).radius(20.0).bg(if locked { ACCENT.with_opacity(0.35) } else { ACCENT }).on_click(move || {
                            if locked {
                                pp.msg.set("Sizda faol obuna bor — u tugagach yangisini olasiz".into());
                            } else if balance < price {
                                pp.msg.set("Balansda mablag' yetarli emas — avval to'ldiring".into());
                            } else {
                                pp.confirm.set(days);
                            }
                        }),
                    ])
                    .gap(10.0)
                    .align(Align::Center)
                    .padding_each(16.0, 12.0, 12.0, 12.0),
                )
                .radius(22.0)
                .margin_each(0.0, 0.0, 0.0, 10.0),
            );
        }
        kids.push(tx(if locked { "Obunangiz tugagach yangi tarif tanlay olasiz." } else { "Obuna balansdan yechiladi." }, 11.5, Color::WHITE).padding_each(4.0, 4.0, 4.0, 28.0));
        scroll_column(&p.scrolls[1], kids)
    }

    fn bill_history(self: &Rc<Self>, p: &Rc<BillingPage>, d: &Rc<Billing>) -> Element {
        if d.history.is_empty() {
            return Self::placeholder(!d.loaded && d.error.is_empty(), if d.error.is_empty() { "Hozircha yozuv yo'q" } else { &d.error });
        }
        let mut kids: Vec<Element> = Vec::new();
        for e in d.history.iter() {
            let up = e.kind == "topup";
            let c = if up { GREEN } else { ACCENT2 };
            kids.push(
                Self::pglass(
                    row(vec![
                        column(vec![icon(if up { B_ADD } else { B_PREMIUM }, 18.0).color(c)]).center().width(34.0).height(34.0).radius(10.0).bg(c.with_opacity(0.18)),
                        column(vec![tx(if up { "Balans to'ldirildi".to_string() } else { format!("{} obuna", plan_label(e.days)) }, 13.5, Color::WHITE).bold(), tx(fmt_date(e.created_at), 11.5, Color::WHITE)]).weight(1.0),
                        tx(format!("{}{}", if up { "+" } else { "−" }, format_sum(e.amount.abs())), 13.5, if up { GREEN } else { Color::rgba(255, 255, 255, 210) }).black(),
                    ])
                    .gap(12.0)
                    .align(Align::Center)
                    .padding_xy(14.0, 11.0),
                )
                .radius(20.0)
                .margin_each(0.0, 0.0, 0.0, 10.0),
            );
        }
        kids.push(gap_box(0.0, 28.0));
        scroll_column(&p.scrolls[2], kids)
    }

    fn bill_confirm(self: &Rc<Self>, p: &Rc<BillingPage>, d: &Rc<Billing>) -> Element {
        let days = p.confirm.get();
        let price = d.plans.iter().find(|x| x.days == days).map(|x| x.price).unwrap_or(0);
        let (pc, pb, me) = (p.clone(), p.clone(), self.clone());
        let panel = column(vec![
            tx("Obunani tasdiqlang", 18.0, Color::WHITE).bold(),
            gap_box(0.0, 10.0),
            tx(format!("{} obuna — {}.\nSumma balansingizdan yechiladi.", plan_label(days), format_sum(price)), 14.0, Color::WHITE),
            gap_box(0.0, 18.0),
            row(vec![
                spacer(),
                self.chip("Bekor qilish", false, move || pc.confirm.set(0)),
                self.chip("Sotib olish", true, move || {
                    pb.confirm.set(0);
                    let (p2, me2, session) = (pb.clone(), me.clone(), me.session.clone());
                    spawn_bg(move || post(&session, "/api/billing/subscribe", json!({"days": days})), move |r| match r {
                        Ok(_) => {
                            p2.msg.set("Obuna faollashtirildi".into());
                            me2.bill_reload(&p2);
                            me2.load_profile();
                        }
                        Err(e) => p2.msg.set(e),
                    });
                }),
            ])
            .gap(8.0),
        ])
        .lens(Color::rgba(255, 255, 255, 14), GLASS_ZOOM)
        .radius(28.0)
        .padding(22.0)
        .margin_each(24.0, 0.0, 24.0, 0.0);
        let pc = p.clone();
        column(vec![panel]).center().bg(Color::rgba(0, 0, 0, 45)).on_click(move || pc.confirm.set(0))
    }
}
