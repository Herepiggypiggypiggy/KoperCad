//! Print setup: which paper a sheet goes on, at what scale, inside which
//! margins, and the page numbers added to the paper. Pure geometry, so the
//! preview window and the PDF writer place a sheet the same way.
//!
//! Without a setup a sheet prints on a page of its own size at 100 %, as it
//! always did. With one, every sheet is placed on the chosen paper:
//!
//! * [`PrintScale::Fit`] shrinks or enlarges the sheet to the printable box
//!   (the paper minus the margins); [`PrintScale::Percent`] keeps a fixed
//!   scale, 100 % being true size. A sheet that does not fit at a fixed scale
//!   is cut off at the margins and reported in [`Placement::cut_off`], never
//!   silently.
//! * The sheet is centred in the box, or sits in its top left corner.
//! * Page numbers (`Page 3 of 12`) go in a strip under the printable box that
//!   is kept free for them, on the paper, outside the sheet's own title block.

/// Paper a sheet can be printed on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PrintPaper {
    /// Each sheet on a page of its own format (nothing is rescaled unless
    /// the margins ask for it).
    #[default]
    AsSheet,
    A4,
    A3,
    A2,
    A1,
    A0,
    Letter,
    Legal,
    Tabloid,
}

impl PrintPaper {
    /// The papers a person chooses between, in the order they are offered.
    pub const ALL: [PrintPaper; 9] = [
        PrintPaper::AsSheet,
        PrintPaper::A4,
        PrintPaper::A3,
        PrintPaper::A2,
        PrintPaper::A1,
        PrintPaper::A0,
        PrintPaper::Letter,
        PrintPaper::Legal,
        PrintPaper::Tabloid,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PrintPaper::AsSheet => "As the sheet",
            PrintPaper::A4 => "A4",
            PrintPaper::A3 => "A3",
            PrintPaper::A2 => "A2",
            PrintPaper::A1 => "A1",
            PrintPaper::A0 => "A0",
            PrintPaper::Letter => "Letter",
            PrintPaper::Legal => "Legal",
            PrintPaper::Tabloid => "Tabloid (11 x 17 in)",
        }
    }

    /// A short, stable key for settings files.
    pub fn key(self) -> &'static str {
        match self {
            PrintPaper::AsSheet => "sheet",
            PrintPaper::A4 => "a4",
            PrintPaper::A3 => "a3",
            PrintPaper::A2 => "a2",
            PrintPaper::A1 => "a1",
            PrintPaper::A0 => "a0",
            PrintPaper::Letter => "letter",
            PrintPaper::Legal => "legal",
            PrintPaper::Tabloid => "tabloid",
        }
    }

    pub fn from_key(key: &str) -> Option<PrintPaper> {
        Self::ALL.into_iter().find(|p| p.key() == key)
    }

    /// Width and height in millimetres, portrait. `None` for
    /// [`PrintPaper::AsSheet`], which takes the sheet's own size.
    pub fn portrait_mm(self) -> Option<(f32, f32)> {
        Some(match self {
            PrintPaper::AsSheet => return None,
            PrintPaper::A4 => (210.0, 297.0),
            PrintPaper::A3 => (297.0, 420.0),
            PrintPaper::A2 => (420.0, 594.0),
            PrintPaper::A1 => (594.0, 841.0),
            PrintPaper::A0 => (841.0, 1189.0),
            PrintPaper::Letter => (215.9, 279.4),
            PrintPaper::Legal => (215.9, 355.6),
            PrintPaper::Tabloid => (279.4, 431.8),
        })
    }
}

/// Which way the paper is turned.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Orientation {
    /// Each sheet decides: a wide sheet goes on landscape paper.
    #[default]
    Auto,
    Landscape,
    Portrait,
}

impl Orientation {
    pub const ALL: [Orientation; 3] = [
        Orientation::Auto,
        Orientation::Landscape,
        Orientation::Portrait,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Orientation::Auto => "Automatic",
            Orientation::Landscape => "Landscape",
            Orientation::Portrait => "Portrait",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Orientation::Auto => "auto",
            Orientation::Landscape => "landscape",
            Orientation::Portrait => "portrait",
        }
    }

    pub fn from_key(key: &str) -> Option<Orientation> {
        Self::ALL.into_iter().find(|o| o.key() == key)
    }
}

/// How big the sheet is drawn on the paper.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum PrintScale {
    /// As large as fits inside the margins.
    #[default]
    Fit,
    /// A fixed scale in percent; 100 prints true size.
    Percent(f32),
}

/// The smallest and largest fixed scale offered.
pub const MIN_PERCENT: f32 = 10.0;
pub const MAX_PERCENT: f32 = 400.0;

/// Height of the strip under the printable box that holds the page number,
/// at least, in millimetres.
pub const FOOTER_STRIP_MM: f32 = 9.0;

/// Text height of the page number, in millimetres.
pub const FOOTER_TEXT_MM: f32 = 3.0;

/// How the printed sheets are placed on the paper.
#[derive(Clone, Debug, PartialEq)]
pub struct PrintSetup {
    pub paper: PrintPaper,
    pub orientation: Orientation,
    pub scale: PrintScale,
    /// Paper left empty on every side, in millimetres. Most office printers
    /// cannot print closer than 5 mm to the edge.
    pub margin_mm: f32,
    /// Centre the sheet in the printable box instead of using its top left
    /// corner.
    pub centre: bool,
    /// Add `Page n of N`, the sheet's name and, when the scale is not 100 %,
    /// the scale in the strip under the sheet.
    pub page_numbers: bool,
}

impl Default for PrintSetup {
    fn default() -> Self {
        Self {
            paper: PrintPaper::A4,
            orientation: Orientation::Auto,
            scale: PrintScale::Fit,
            margin_mm: 8.0,
            centre: true,
            page_numbers: true,
        }
    }
}

/// Where one sheet lands on its paper. Lengths in millimetres, y down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// The paper.
    pub paper: (f32, f32),
    /// The sheet's top left corner on the paper.
    pub offset: (f32, f32),
    /// Paper millimetres per sheet millimetre.
    pub scale: f32,
    /// The printable box on the paper: `(x, y, width, height)`.
    pub printable: (f32, f32, f32, f32),
    /// The sheet does not fit the printable box at this scale and is cut off
    /// at its edge.
    pub cut_off: bool,
}

impl Placement {
    /// The identity: the sheet's own page, full size.
    pub fn identity(size: (f32, f32)) -> Self {
        Self {
            paper: size,
            offset: (0.0, 0.0),
            scale: 1.0,
            printable: (0.0, 0.0, size.0, size.1),
            cut_off: false,
        }
    }

    /// Paper position of a point of the sheet.
    pub fn to_paper(&self, p: (f32, f32)) -> (f32, f32) {
        (
            self.offset.0 + p.0 * self.scale,
            self.offset.1 + p.1 * self.scale,
        )
    }

    /// The scale as people say it: `71 %`, `100 %`.
    pub fn percent_text(&self) -> String {
        let p = self.scale * 100.0;
        if (p - p.round()).abs() < 0.05 {
            format!("{} %", p.round())
        } else {
            format!("{p:.1} %")
        }
    }

    /// Whether the sheet is drawn at its true size.
    pub fn is_true_size(&self) -> bool {
        (self.scale - 1.0).abs() < 1e-4
    }
}

/// Where a sheet of `size` (mm) lands under `setup`.
pub fn place(size: (f32, f32), setup: &PrintSetup) -> Placement {
    let (sw, sh) = (size.0.max(1.0), size.1.max(1.0));
    let paper = match setup.paper.portrait_mm() {
        None => (sw, sh),
        Some((w, h)) => {
            let landscape = match setup.orientation {
                Orientation::Landscape => true,
                Orientation::Portrait => false,
                Orientation::Auto => sw >= sh,
            };
            if landscape { (h, w) } else { (w, h) }
        }
    };
    // The margin cannot eat the paper.
    let margin = setup
        .margin_mm
        .clamp(0.0, (paper.0.min(paper.1) / 2.0 - 10.0).max(0.0));
    let bottom = if setup.page_numbers {
        margin.max(FOOTER_STRIP_MM)
    } else {
        margin
    };
    let printable = (
        margin,
        margin,
        (paper.0 - 2.0 * margin).max(1.0),
        (paper.1 - margin - bottom).max(1.0),
    );
    let scale = match setup.scale {
        PrintScale::Fit => (printable.2 / sw).min(printable.3 / sh),
        PrintScale::Percent(p) => p.clamp(MIN_PERCENT, MAX_PERCENT) / 100.0,
    };
    let (w, h) = (sw * scale, sh * scale);
    let (x, y) = if setup.centre {
        (
            printable.0 + (printable.2 - w) / 2.0,
            printable.1 + (printable.3 - h) / 2.0,
        )
    } else {
        (printable.0, printable.1)
    };
    // A sheet a hair over the box (rounding) is not "cut off".
    let cut_off = w > printable.2 + 0.05 || h > printable.3 + 0.05;
    // A cut-off sheet starts at the box corner, so what is lost is the right
    // and lower edge, the title block's neighbours, not an arbitrary middle.
    let (x, y) = (
        if w > printable.2 { printable.0 } else { x },
        if h > printable.3 { printable.1 } else { y },
    );
    Placement {
        paper,
        offset: (x, y),
        scale,
        printable,
        cut_off,
    }
}

/// The text of the strip under a sheet: `(left, middle, right)`.
pub fn footer_text(
    name: &str,
    placement: &Placement,
    number: usize,
    total: usize,
) -> (String, String, String) {
    let middle = if placement.is_true_size() {
        String::new()
    } else {
        format!("Printed at {}", placement.percent_text())
    };
    (name.to_owned(), middle, format!("Page {number} of {total}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const A3L: (f32, f32) = (420.0, 297.0);

    fn setup(paper: PrintPaper) -> PrintSetup {
        PrintSetup {
            paper,
            ..PrintSetup::default()
        }
    }

    #[test]
    fn an_a3_sheet_fitted_to_a4_is_scaled_to_the_printable_width() {
        let p = place(A3L, &setup(PrintPaper::A4));
        assert_eq!(p.paper, (297.0, 210.0), "wide sheet, landscape paper");
        // The box is 281 x 193 mm (the foot keeps 9 mm for the page number),
        // so the height decides: 193 / 297.
        assert!((p.scale - 193.0 / 297.0).abs() < 1e-5, "{}", p.scale);
        assert!(!p.cut_off);
        // Centred sideways, touching the top margin.
        assert!((p.offset.0 - (8.0 + (281.0 - 420.0 * p.scale) / 2.0)).abs() < 1e-3);
        assert!((p.offset.1 - 8.0).abs() < 1e-3);
        assert_eq!(p.percent_text(), "65 %");
    }

    #[test]
    fn auto_orientation_follows_the_sheet() {
        let wide = place((420.0, 297.0), &setup(PrintPaper::A4));
        let tall = place((297.0, 420.0), &setup(PrintPaper::A4));
        assert_eq!(wide.paper, (297.0, 210.0));
        assert_eq!(tall.paper, (210.0, 297.0));
        let forced = place(
            (420.0, 297.0),
            &PrintSetup {
                orientation: Orientation::Portrait,
                ..setup(PrintPaper::A4)
            },
        );
        assert_eq!(forced.paper, (210.0, 297.0));
    }

    #[test]
    fn a_fixed_scale_that_does_not_fit_is_reported_and_starts_at_the_corner() {
        let p = place(
            A3L,
            &PrintSetup {
                scale: PrintScale::Percent(100.0),
                ..setup(PrintPaper::A4)
            },
        );
        assert!(p.cut_off);
        assert!(p.is_true_size());
        assert_eq!(p.offset, (8.0, 8.0));
    }

    #[test]
    fn a_fixed_scale_that_fits_is_centred_and_not_cut() {
        let p = place(
            (210.0, 148.0),
            &PrintSetup {
                scale: PrintScale::Percent(100.0),
                ..setup(PrintPaper::A3)
            },
        );
        assert!(!p.cut_off);
        // A5 landscape on A3 landscape (420 x 297): centred in the box.
        assert!((p.offset.0 - (8.0 + (404.0 - 210.0) / 2.0)).abs() < 1e-3);
    }

    #[test]
    fn as_the_sheet_with_no_margin_is_the_old_output() {
        let p = place(
            A3L,
            &PrintSetup {
                paper: PrintPaper::AsSheet,
                margin_mm: 0.0,
                page_numbers: false,
                scale: PrintScale::Percent(100.0),
                ..PrintSetup::default()
            },
        );
        assert_eq!(p, Placement::identity(A3L));
    }

    #[test]
    fn page_numbers_keep_a_strip_free_under_the_sheet() {
        let with = place(A3L, &setup(PrintPaper::A4));
        let without = place(
            A3L,
            &PrintSetup {
                page_numbers: false,
                ..setup(PrintPaper::A4)
            },
        );
        assert!(with.printable.3 < without.printable.3);
        // The sheet never reaches into the strip.
        let bottom = with.offset.1 + A3L.1 * with.scale;
        assert!(bottom <= with.paper.1 - FOOTER_STRIP_MM + 1e-3);
    }

    #[test]
    fn nonsense_margins_and_scales_stay_printable() {
        let p = place(
            A3L,
            &PrintSetup {
                margin_mm: 500.0,
                scale: PrintScale::Percent(0.0),
                ..setup(PrintPaper::A4)
            },
        );
        assert!(p.printable.2 >= 1.0 && p.printable.3 >= 1.0);
        assert!(p.scale >= MIN_PERCENT / 100.0);
    }

    #[test]
    fn keys_round_trip() {
        for p in PrintPaper::ALL {
            assert_eq!(PrintPaper::from_key(p.key()), Some(p));
        }
        for o in Orientation::ALL {
            assert_eq!(Orientation::from_key(o.key()), Some(o));
        }
        assert_eq!(PrintPaper::from_key("b5"), None);
    }

    #[test]
    fn the_footer_names_the_scale_only_when_it_is_not_true_size() {
        let p = place(A3L, &setup(PrintPaper::A4));
        let (left, middle, right) = footer_text("12", &p, 3, 9);
        assert_eq!((left.as_str(), right.as_str()), ("12", "Page 3 of 9"));
        assert_eq!(middle, "Printed at 65 %");
        let q = Placement::identity(A3L);
        assert_eq!(footer_text("12", &q, 1, 1).1, "");
    }
}
