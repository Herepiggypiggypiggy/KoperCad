//! The template data model.

use novo_core::{PageFormat, PaperSize};
use serde::{Deserialize, Serialize};

use super::table::TableSpec;
use crate::table::Align;

/// File format version written into every template file.
pub const FORMAT_VERSION: u32 = 1;

/// What a template is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateKind {
    /// Tables from report data (parts list, wire list).
    #[default]
    Report,
    /// A fixed form with fields (cover sheet, test record).
    Form,
    /// Only frame and title block: the sheet a schematic page is drawn on.
    Sheet,
}

/// The sheet frame: a line inside the page edge, with the column numbers of
/// the cross-reference grid in the margins (the page's
/// [`PageFormat::columns`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    /// Distance of the frame line from the sheet edge, mm.
    pub margin_mm: f64,
    pub column_numbers: bool,
}

impl Default for Frame {
    /// The canvas frame: 10 mm, with column numbers.
    fn default() -> Self {
        Frame {
            margin_mm: 10.0,
            column_numbers: true,
        }
    }
}

/// One row of the title block: a caption and a value that may use fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TitleRow {
    pub caption: String,
    /// Translation key of the caption (DOC-2); unused by the engine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// Text with `{field}` placeholders.
    pub value: String,
}

impl TitleRow {
    pub fn new(caption: &str, value: &str) -> Self {
        TitleRow {
            caption: caption.to_owned(),
            key: None,
            value: value.to_owned(),
        }
    }
}

/// The title block in the bottom-right corner of the frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TitleBlock {
    pub width_mm: f64,
    pub row_mm: f64,
    /// Width of the caption column, mm.
    pub caption_mm: f64,
    pub rows: Vec<TitleRow>,
}

impl TitleBlock {
    pub fn height_mm(&self) -> f64 {
        self.row_mm * self.rows.len() as f64
    }
}

impl Default for TitleBlock {
    /// The canvas title block: 140 mm wide, six rows of 6 mm.
    fn default() -> Self {
        TitleBlock {
            width_mm: 140.0,
            row_mm: 6.0,
            caption_mm: 24.0,
            rows: vec![
                TitleRow::new("Project", "{project.name}"),
                TitleRow::new("Page", "{page.name}  {page.title}"),
                TitleRow::new("Sheet", "{page} of {pages}"),
                TitleRow::new("Structure", "{page.structure}"),
                TitleRow::new("Revision", "{revision}  {revision_date}"),
                TitleRow::new("Drawn / checked", "{drawn_by} / {checked_by}"),
            ],
        }
    }
}

/// A table placed on the sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableElement {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub spec: TableSpec,
}

/// Something placed on the sheet, in mm from the top-left corner (y down).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Element {
    Text {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        /// Text with `{field}` placeholders.
        text: String,
        text_mm: f64,
        #[serde(default)]
        bold: bool,
        #[serde(default)]
        align: Align,
        /// Translation key (DOC-2); unused by the engine.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<String>,
        /// Printed on every page of the document, not only the first.
        #[serde(default)]
        every_page: bool,
    },
    Line {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        #[serde(default)]
        heavy: bool,
        #[serde(default)]
        every_page: bool,
    },
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        #[serde(default)]
        heavy: bool,
        #[serde(default)]
        every_page: bool,
    },
    Table(TableElement),
}

impl Element {
    /// A text element with the common defaults.
    pub fn text(x: f64, y: f64, width: f64, height: f64, text: &str, text_mm: f64) -> Element {
        Element::Text {
            x,
            y,
            width,
            height,
            text: text.to_owned(),
            text_mm,
            bold: false,
            align: Align::Left,
            key: None,
            every_page: false,
        }
    }

    /// The element's bounding box `(x, y, w, h)`.
    pub fn bounds(&self) -> (f64, f64, f64, f64) {
        match self {
            Element::Text {
                x,
                y,
                width,
                height,
                ..
            }
            | Element::Rect {
                x,
                y,
                width,
                height,
                ..
            } => (*x, *y, *width, *height),
            Element::Line { x1, y1, x2, y2, .. } => {
                (x1.min(*x2), y1.min(*y2), (x2 - x1).abs(), (y2 - y1).abs())
            }
            Element::Table(t) => (t.x, t.y, t.width, t.height),
        }
    }
}

/// A report or form template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Template {
    /// File format version ([`FORMAT_VERSION`]).
    #[serde(default = "version")]
    pub version: u32,
    /// Unique name; the file name derives from it.
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub kind: TemplateKind,
    pub format: PageFormat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<Frame>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_block: Option<TitleBlock>,
    #[serde(default)]
    pub elements: Vec<Element>,
}

fn version() -> u32 {
    FORMAT_VERSION
}

impl Template {
    /// An empty template on `format`, no frame, no title block.
    pub fn new(name: &str, kind: TemplateKind, format: PageFormat) -> Self {
        Template {
            version: FORMAT_VERSION,
            name: name.to_owned(),
            description: String::new(),
            kind,
            format,
            frame: None,
            title_block: None,
            elements: Vec::new(),
        }
    }

    /// A4 landscape.
    pub fn a4_landscape() -> PageFormat {
        PageFormat {
            size: PaperSize::A4,
            landscape: true,
            ..PageFormat::default()
        }
    }

    /// Sheet size in mm.
    pub fn size_mm(&self) -> (f64, f64) {
        self.format.size_mm()
    }

    /// The area inside the frame, `(x, y, w, h)`; the whole sheet without a
    /// frame.
    pub fn inner_rect(&self) -> (f64, f64, f64, f64) {
        let (w, h) = self.size_mm();
        let m = self.frame.as_ref().map_or(0.0, |f| f.margin_mm);
        (m, m, (w - 2.0 * m).max(0.0), (h - 2.0 * m).max(0.0))
    }

    /// The title block's rectangle `(x, y, w, h)`, if there is one.
    pub fn title_block_rect(&self) -> Option<(f64, f64, f64, f64)> {
        let block = self.title_block.as_ref()?;
        let (x, y, w, h) = self.inner_rect();
        let (bw, bh) = (block.width_mm, block.height_mm());
        Some((x + w - bw, y + h - bh, bw, bh))
    }
}
