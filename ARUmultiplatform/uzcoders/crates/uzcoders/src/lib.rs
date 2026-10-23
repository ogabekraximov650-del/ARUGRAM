//! UZ: # uzcoders
//! Rust tilida noldan yozilgan, tashqi bog'liqliksiz mobil UI kutubxonasi.
//! Hammasini `use uzcoders::prelude::*;` bilan oling.
//! EN: # uzcoders
//! A mobile UI library written from scratch in Rust, with no external dependencies.
//! Import everything with `use uzcoders::prelude::*;`.
//! RU: # uzcoders
//! Мобильная UI-библиотека, написанная с нуля на Rust, без внешних зависимостей.
//! Подключайте всё через `use uzcoders::prelude::*;`.

pub use uzcoders_android as android;
pub use uzcoders_core as core;
pub use uzcoders_gpu as gpu;
pub use uzcoders_layout as layout;
pub use uzcoders_render as render;
pub use uzcoders_text as text;
pub use uzcoders_ui as ui;

pub mod prelude {
    pub use std::rc::Rc;
    pub use uzcoders_core::*;
    pub use uzcoders_layout::{Align, Justify};
    pub use uzcoders_render::{decode_image, decode_jpeg, decode_png, Canvas, DrawCmd, Image};
    pub use uzcoders_text::Font;
    pub use uzcoders_ui::{
        button, column, field, gap_box, grid, icon, image, image_fill, lazy_column, lazy_column_var, row,
        scroll_column, spacer, sprite, spinner, stack, text, Element, Hold, KeyboardKind, Navigator,
        ScrollState,
        Ui,
    };
}
