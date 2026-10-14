//! Exports the Gate 1 motor starter to PDF.
//!
//! ```text
//! cargo run -p novo-pdf --example gate1 -- [path/to/gate1.pdf]
//! ```

use novo_core::Document;
use novo_pdf::{PdfOptions, export_pdf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "gate1.pdf".to_string());
    let mut doc = Document::default();
    novo_tests::motor_starter(&mut doc)?;
    let bytes = export_pdf(doc.project(), &PdfOptions::default())?;
    std::fs::write(&path, &bytes)?;
    println!(
        "wrote {path}: {} pages, {} bytes",
        doc.project().pages().count(),
        bytes.len()
    );
    Ok(())
}
