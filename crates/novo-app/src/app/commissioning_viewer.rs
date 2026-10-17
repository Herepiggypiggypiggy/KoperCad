//! Seeing the photos of a punch item: a row of thumbnails under the item and
//! a window with the picture large. Only PNG and JPEG are decoded (what a
//! phone or tablet camera makes); other files keep their name in the row.

use std::collections::HashMap;
use std::path::PathBuf;

use eframe::egui;
use novo_commissioning::{Photo, is_photo_name};
use novo_theme::Palette;

/// The longest side of a decoded picture; bigger ones are scaled down once.
const MAX_SIDE: u32 = 1600;
/// The thumbnail's edge in points.
const THUMB: f32 = 72.0;

/// Decoded pictures by file name, and the one open in the window.
#[derive(Default)]
pub(crate) struct PhotoViewer {
    /// The photos folder, set every frame by the panel.
    pub(super) dir: Option<PathBuf>,
    /// `None` is a file that could not be shown (missing or not PNG/JPEG).
    textures: HashMap<String, Option<egui::TextureHandle>>,
    /// The picture in the window: file name and when it was taken.
    pub(super) open: Option<(String, String)>,
}

/// A picture as an egui image, scaled so its longest side is at most
/// [`MAX_SIDE`]. `None` for anything the decoder cannot read.
pub(super) fn decode(bytes: &[u8]) -> Option<egui::ColorImage> {
    let img = image::load_from_memory(bytes).ok()?;
    let img = if img.width().max(img.height()) > MAX_SIDE {
        img.resize(MAX_SIDE, MAX_SIDE, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [rgba.width() as usize, rgba.height() as usize],
        rgba.as_raw(),
    ))
}

impl PhotoViewer {
    fn texture(&mut self, ctx: &egui::Context, name: &str) -> Option<egui::TextureHandle> {
        if !self.textures.contains_key(name) {
            let shown = is_photo_name(name)
                .then(|| self.dir.as_ref().map(|d| d.join(name)))
                .flatten()
                .and_then(|path| std::fs::read(path).ok())
                .and_then(|bytes| decode(&bytes))
                .map(|image| ctx.load_texture(name, image, egui::TextureOptions::LINEAR));
            self.textures.insert(name.to_owned(), shown);
        }
        self.textures.get(name).cloned().flatten()
    }
}

/// The thumbnails of an item's pictures. A touched one opens the window.
pub(super) fn strip(ui: &mut egui::Ui, viewer: &mut PhotoViewer, photos: &[Photo]) {
    let p = Palette::of(ui);
    ui.horizontal_wrapped(|ui| {
        for photo in photos {
            let Some(texture) = viewer.texture(ui.ctx(), &photo.name) else {
                // Not a picture the viewer reads, or the file is not here.
                ui.weak(&photo.name)
                    .on_hover_text("This picture cannot be shown here");
                continue;
            };
            let button = ui
                .add(
                    egui::Button::image(
                        egui::Image::new(&texture)
                            .fit_to_exact_size(egui::vec2(THUMB, THUMB))
                            .corner_radius(2.0),
                    )
                    .frame(true)
                    .stroke(egui::Stroke::new(1.0, p.border_subtle)),
                )
                .on_hover_text("Open the picture");
            super::cabinet_bar::note_ctx(
                ui.ctx(),
                "commissioning.photo_view",
                "Photo thumbnail",
                &button,
            );
            if button.clicked() {
                viewer.open = Some((photo.name.clone(), photo.at.clone()));
            }
        }
    });
}

/// The window with the picture large, while one is open.
pub(super) fn window(ctx: &egui::Context, viewer: &mut PhotoViewer) {
    let Some((name, at)) = viewer.open.clone() else {
        return;
    };
    let Some(texture) = viewer.texture(ctx, &name) else {
        viewer.open = None;
        return;
    };
    let screen = ctx.content_rect().size();
    let mut open = true;
    egui::Window::new("Photo")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .max_size(screen * 0.9)
        .show(ctx, |ui| {
            let room = screen * 0.8;
            let size = texture.size_vec2();
            let scale = (room.x / size.x).min(room.y / size.y).min(1.0);
            ui.add(egui::Image::new(&texture).fit_to_exact_size(size * scale));
            ui.weak(format!("{name} · {at}"));
        });
    if !open {
        viewer.open = None;
    }
}
