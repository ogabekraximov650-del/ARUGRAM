use arugramrust::{make_ui_parts, tg::set_test_time};
use uzcoders::prelude::*;
fn main() {
    let dir = std::env::args().nth(1).unwrap();
    let (mut ui, intro, _login) = make_ui_parts(2.0);
    let sz = Size::new(720.0, 1500.0);
    let mut now = 0.0f32;
    let mut step = |ui: &mut Ui, now: &mut f32, name: Option<&str>| {
        *now += 16.0;
        set_test_time(Some(*now));
        ui.set_time(*now as u64);
        let cmds = ui.frame(sz);
        for _ in 0..400 {
            if uzcoders::prelude::drain_tasks() || uzcoders::prelude::pending_tasks() == 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        std::thread::sleep(std::time::Duration::from_millis(14));
        uzcoders::prelude::drain_tasks();
        if let Some(n) = name {
            let mut c = ui.canvas(720, 1500);
            c.clear(Color::hex(0x000000));
            c.execute(&cmds);
            std::fs::write(format!("{dir}/{n}.png"), c.to_png()).unwrap();
        }
    };
    for page in 0..6usize {
        intro.debug_set(page);
        for i in 0..140 {
            let name = format!("i{page}");
            step(&mut ui, &mut now, if i == 139 { Some(name.as_str()) } else { None });
        }
    }
}
