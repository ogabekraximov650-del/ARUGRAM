use arugramrust::{login::Page, make_ui_parts, tg::set_test_time};
use uzcoders::prelude::*;
fn main() {
    let dir = std::env::args().nth(1).unwrap();
    let (mut ui, _intro, login) = make_ui_parts(2.0625);
    let sz = Size::new(720.0, 1600.0);
    let uz = arugramrust::countries::COUNTRIES.iter().position(|c| c.iso == "UZ").unwrap();
    let mut snap = |ui: &mut Ui, name: &str, t: f32| {
        set_test_time(Some(t));
        ui.set_time(t as u64);
        let cmds = ui.frame(sz);
        let mut c = ui.canvas(720, 1600);
        c.clear(Color::hex(0x000000));
        c.execute(&cmds);
        std::fs::write(format!("{dir}/{name}.png"), c.to_png()).unwrap();
    };
    set_test_time(Some(0.0));
    ui.set_insets(EdgeInsets::new(0.0, 59.0, 0.0, 100.0));
    login.show();
    snap(&mut ui, "a0", 100.0);
    snap(&mut ui, "a1", 900.0);
    login.debug_picker(true);
    snap(&mut ui, "pick", 1500.0);
    login.debug_picker(false);
    login.debug_digits(uz, "");
    snap(&mut ui, "ph0", 1900.0);
    snap(&mut ui, "ph", 3000.0);
    login.debug_digits(uz, "9012");
    snap(&mut ui, "a2", 2000.0);
    snap(&mut ui, "a3", 3000.0);
    login.debug_icons_sync();
    login.debug_goto(Page::Code);
    snap(&mut ui, "b0", 3200.0);
    snap(&mut ui, "b1", 4600.0);
    login.debug_code("12", 4650.0);
    snap(&mut ui, "b2", 4700.0);
    snap(&mut ui, "b3", 5200.0);

    login.debug_goto(Page::Pass);
    snap(&mut ui, "c1", 6500.0);
    login.debug_goto(Page::Done);
    snap(&mut ui, "d1", 8500.0);
}
