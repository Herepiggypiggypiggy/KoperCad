//! Fonts: Inter for interface text, Phosphor for icons.
//!
//! Inter (SIL Open Font License, see `assets/fonts/Inter-LICENSE.txt`) is
//! bundled in the binary, so text looks the same on every machine and needs
//! nothing installed. The bundled files are Inter 4.1 subset with
//! `pyftsubset --unicodes="U+0000-DFFF,U+F900-FFFF" --layout-features='*'`,
//! which drops Inter's private use glyphs so Phosphor's icons show instead. egui's own fonts stay behind it as fallbacks for symbols
//! and emoji Inter lacks.

use egui::{FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

const INTER_REGULAR: &[u8] = include_bytes!("../assets/fonts/Inter-Regular.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/Inter-SemiBold.ttf");

/// Font family for headings and other emphasised text.
pub fn strong_family() -> FontFamily {
    FontFamily::Name("novo-strong".into())
}

/// egui's default fonts with Inter in front and Phosphor icons mixed in.
pub fn definitions() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "inter".into(),
        Arc::new(FontData::from_static(INTER_REGULAR)),
    );
    fonts.font_data.insert(
        "inter-semibold".into(),
        Arc::new(FontData::from_static(INTER_SEMIBOLD)),
    );

    let fallbacks = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "inter".into());
    let mut strong = vec!["inter-semibold".to_owned()];
    strong.extend(fallbacks);
    fonts.families.insert(strong_family(), strong);

    // Icons resolve in ordinary text, so `format!("{} Save", icons::FLOPPY_DISK)`
    // just works. The bundled Inter is subset to leave out the private use
    // area, where it would otherwise shadow Phosphor's icons.
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    if let Some(strong) = fonts.families.get_mut(&strong_family()) {
        strong.push("phosphor".into());
    }
    fonts
}
