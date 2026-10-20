//! # uzcoders
//! Rust tilida noldan yozilgan, tashqi bog'liqliksiz mobil UI kutubxonasi.
//! Hammasini `use uzcoders::prelude::*;` bilan oling.

pub use uzcoders_android as android;
pub use uzcoders_core as core;
pub use uzcoders_layout as layout;
pub use uzcoders_render as render;
pub use uzcoders_text as text;
pub use uzcoders_ui as ui;

pub mod prelude {
    pub use std::rc::Rc;
    pub use uzcoders_core::*;
    pub use uzcoders_layout::{Align, Justify};
    pub use uzcoders_render::{Canvas, DrawCmd};
    pub use uzcoders_text::Font;
    pub use uzcoders_ui::{
        button, column, field, gap_box, grid, icon, row, scroll_column, spacer, text, Element,
        Navigator, ScrollState, Ui,
    };
}
