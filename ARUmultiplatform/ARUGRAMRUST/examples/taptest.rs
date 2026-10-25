use arugramrust::{make_ui_parts, tg::set_test_time};
use uzcoders::prelude::*;
fn main() {
    let (mut ui, intro, login) = make_ui_parts(2.0);
    let sz = Size::new(720.0, 1500.0);
    set_test_time(Some(0.0));
    ui.set_time(0);
    ui.frame(sz);
    let tap = |ui: &mut Ui, x: f32, y: f32| {
        ui.pointer(PointerEvent::new(PointerKind::Down, x, y));
        ui.pointer(PointerEvent::new(PointerKind::Up, x, y));
    };
    println!("night before: {}", intro.is_night());
    tap(&mut ui, (360.0 - 38.0) * 2.0, 66.0 * 2.0);
    set_test_time(Some(100.0));
    ui.set_time(100);
    ui.frame(sz);
    println!("night after tap on theme: {}", intro.is_night());
    tap(&mut ui, 360.0, 656.0 * 2.0);
    ui.frame(sz);
    println!("login started after tap on start: {}", login.started());
}
