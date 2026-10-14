//! Renders the brand SVGs in `assets/branding` to PNG and ICO.
//!
//! Usage, from the repository root:
//!
//! ```text
//! cargo run --manifest-path tools/brand-render/Cargo.toml -- assets/branding/icon.svg 16,24,32,48,64,256
//! ```
//!
//! Writes `<name>-<size>.png` next to the SVG for every size and `<name>.ico`
//! holding all of them.

use std::path::{Path, PathBuf};

fn render(tree: &resvg::usvg::Tree, size: u32) -> resvg::tiny_skia::Pixmap {
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size).expect("non-zero size");
    let scale = size as f32 / tree.size().width().max(tree.size().height());
    resvg::render(
        tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let svg = PathBuf::from(args.next().ok_or("missing SVG path")?);
    let sizes: Vec<u32> = args
        .next()
        .unwrap_or_else(|| "16,24,32,48,64,256".into())
        .split(',')
        .map(str::parse)
        .collect::<Result<_, _>>()?;

    let data = std::fs::read(&svg)?;
    let tree = resvg::usvg::Tree::from_data(&data, &resvg::usvg::Options::default())?;
    let stem = svg.file_stem().ok_or("no file name")?.to_string_lossy();
    let dir = svg.parent().unwrap_or(Path::new("."));

    let mut icon = ico::IconDir::new(ico::ResourceType::Icon);
    for &size in &sizes {
        let pixmap = render(&tree, size);
        let png = dir.join(format!("{stem}-{size}.png"));
        pixmap.save_png(&png)?;
        // The ICO wants straight alpha; tiny-skia stores premultiplied.
        let rgba: Vec<u8> = pixmap
            .pixels()
            .iter()
            .flat_map(|p| {
                let c = p.demultiply();
                [c.red(), c.green(), c.blue(), c.alpha()]
            })
            .collect();
        let image = ico::IconImage::from_rgba_data(size, size, rgba);
        icon.add_entry(ico::IconDirEntry::encode(&image)?);
        println!("wrote {}", png.display());
    }
    let ico_path = dir.join(format!("{stem}.ico"));
    icon.write(std::fs::File::create(&ico_path)?)?;
    println!("wrote {}", ico_path.display());
    Ok(())
}
