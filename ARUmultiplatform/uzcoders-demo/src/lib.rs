//! uzcoders bilan yozilgan kichik ilova: hisoblagich. Kutubxonani telefonda sinash uchun.

use uzcoders::prelude::*;

static FONT: &[u8] = include_bytes!("../../uzcoders/assets/fonts/DejaVuSans.ttf");

/// Ilovaning butun UI'si shu yerda. `scale` — ekran zichligi (dp -> piksel).
pub fn make_ui(scale: f32) -> Ui {
    let font = Rc::new(Font::parse(FONT).expect("shrift"));
    let count = State::new(0i32);
    let mut ui = Ui::new(font, move || {
        let (inc, dec, reset) = (count.clone(), count.clone(), count.clone());
        column(vec![
            text("uzcoders").size(28.0).color(Color::WHITE),
            text("Rust'da noldan yozilgan UI").size(14.0).color(Color::hex(0x8DA2B5)),
            spacer(),
            text(format!("{}", count.get())).size(96.0).color(Color::WHITE),
            spacer(),
            row(vec![
                button("−", move || dec.update(|v| *v -= 1)).grow(1.0),
                button("+", move || inc.update(|v| *v += 1)).grow(1.0),
            ])
            .gap(12.0)
            .width(300.0),
            button("Nolga qaytarish", move || reset.set(0)).bg(Color::hex(0x3B4B5C)).margin(12.0),
            // Pastdagi tizim navigatsiya paneli ostida qolmaslik uchun bo'sh joy.
            column(vec![]).height(48.0),
        ])
        .padding(24.0)
        .gap(8.0)
        .align(Align::Center)
        .bg(Color::hex(0x17212B))
    });
    ui.set_scale(scale);
    ui
}

#[cfg(target_os = "android")]
uzcoders::android::android_app!(make_ui);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tugmalar_ishlaydi() {
        let mut ui = make_ui(1.0);
        let cmds = ui.frame(Size::new(360.0, 640.0));
        assert!(cmds.len() > 6);
        let mut canvas = Canvas::new(360, 640, ui.font());
        canvas.clear(Color::BLACK);
        canvas.execute(&cmds);
        // Fon rangi (0x17212B) ekran burchagida ko'rinishi kerak.
        assert_eq!(canvas.pixel(2, 2), Color::hex(0x17212B));
    }
}
