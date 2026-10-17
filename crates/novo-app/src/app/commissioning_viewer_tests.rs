//! Looking at the photos of a punch item.

use std::io::Cursor;

use eframe::egui;
use novo_commissioning::{PunchItem, PunchState, Severity};

use super::commissioning_viewer::decode;
use super::gate1_tests::Drive;
use crate::actions::Action;

fn picture(width: u32, height: u32, format: image::ImageFormat) -> Vec<u8> {
    let img = image::RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
    });
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, format).unwrap();
    out.into_inner()
}

#[test]
fn png_and_jpeg_decode_big_ones_shrink_and_other_bytes_do_not() {
    for format in [image::ImageFormat::Png, image::ImageFormat::Jpeg] {
        let small = decode(&picture(40, 30, format)).expect("a small picture");
        assert_eq!(small.size, [40, 30]);
    }
    let big = decode(&picture(3200, 1600, image::ImageFormat::Png)).unwrap();
    assert_eq!(big.size, [1600, 800], "longest side scaled to 1600");
    assert!(decode(b"not a picture").is_none());
}

#[test]
fn a_thumbnail_opens_the_picture_in_a_window() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    assert!(d.app.save_to(&dir.path().join("proj.novo")));
    d.app.run_command(&ctx, "view.commissioning", None);
    d.frame();
    let file = dir.path().join("snap.png");
    std::fs::write(&file, picture(64, 48, image::ImageFormat::Png)).unwrap();
    let broken = dir.path().join("broken.jpg");
    std::fs::write(&broken, b"not really a jpeg").unwrap();
    let mode = d.app.wb.commissioning.as_mut().unwrap();
    mode.tab = super::commissioning::Tab::Punch;
    let id = mode.store.add_punch(PunchItem {
        id: 0,
        title: "loose ferrule".into(),
        note: String::new(),
        severity: Severity::Minor,
        state: PunchState::Open,
        target: None,
        check: None,
        raised_by: "tv".into(),
        raised_at: "2026-10-17T09:00:00Z".into(),
        closed_by: None,
        closed_at: None,
        photos: Vec::new(),
        signature: None,
    });
    assert_eq!(d.app.attach_photo_files(id, &[file, broken]), 2);
    d.frame();
    d.frame();
    // only the real picture gets a thumbnail; the broken one keeps its name
    let thumb = d
        .app
        .learn_anchor("commissioning.photo_view")
        .expect("a thumbnail");
    assert!(thumb.width() > 0.0);
    assert!(
        d.app
            .wb
            .commissioning
            .as_ref()
            .unwrap()
            .viewer
            .open
            .is_none()
    );
    d.frame_with(vec![egui::Event::PointerMoved(thumb.center())]);
    for pressed in [true, false] {
        d.frame_with(vec![egui::Event::PointerButton {
            pos: thumb.center(),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }]);
    }
    d.frame();
    let open = d.app.wb.commissioning.as_ref().unwrap().viewer.open.clone();
    assert!(
        open.is_some_and(|(name, _)| name.ends_with(".png")),
        "the window opens"
    );
}
