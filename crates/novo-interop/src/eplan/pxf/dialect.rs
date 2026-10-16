//! Everything the PXF writer assumes about one EPLAN release, in one place.
//! Moving to a newer release means adding a [`Dialect`] here and checking
//! the rows below against a file that release wrote; nothing else in the
//! writer names a version.
//!
//! Status of every row: taken from the findings about projects exported by
//! EPLAN Electric P8 2024 (PxfVersion 1.23) and never confirmed by opening a
//! KoperCAD file in EPLAN. What EPLAN 2027 changes is **unknown**; no 2027
//! dialect exists, so nothing is written for it.

/// The values that differ between EPLAN releases.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dialect {
    /// Release name for messages.
    pub release: &'static str,
    /// `Version` on the root element.
    pub version: &'static str,
    /// `PxfVersion` on the root element.
    pub pxf_version: &'static str,
    /// `SchemaVersion` on the root element.
    pub schema_version: &'static str,
    /// Symbol library names and their numeric ids (`ESymLib` rows).
    pub symbol_libraries: &'static [(&'static str, u32)],
    /// Sheet size in millimetres (width, height) the position origin refers
    /// to: bottom left, y up. Other sheet sizes are converted with their own
    /// height and reported as unchecked.
    pub sheet_mm: (f64, f64),
    /// Language written into multi-language texts when none is given.
    pub default_lang: &'static str,
}

/// EPLAN Electric P8 2024, the release the findings were taken from.
pub const EPLAN_2024: Dialect = Dialect {
    release: "EPLAN 2024",
    version: "2024.0",
    pxf_version: "1.23",
    schema_version: "1.0",
    symbol_libraries: &[
        ("SPECIAL", 0),
        ("IEC_symbol", 1),
        ("IEC_single_symbol", 2),
        ("GRAPHICS", 3),
    ],
    sheet_mm: (420.0, 297.0),
    default_lang: "en_US",
};

impl Default for Dialect {
    fn default() -> Self {
        EPLAN_2024
    }
}
