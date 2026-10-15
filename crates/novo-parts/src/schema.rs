//! Catalog schema version 2: the structured part data that sizing, routing,
//! ordering and the 3D view read (design: `docs/PARTS-LIBRARY-DESIGN.md`
//! section 5).
//!
//! Every field is optional and defaults to "unknown". Unknown stays unknown:
//! nothing here fills in a guess, so a report can list exactly what is missing.

use serde::{Deserialize, Serialize};

/// Classification codes only. The texts come from the ETIM model file.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Classification {
    /// ETIM class, `EC000897`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etim: Option<String>,
    /// ECLASS code, `27-14-11-01`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eclass: Option<String>,
}

impl Classification {
    pub fn is_empty(&self) -> bool {
        self.etim.is_none() && self.eclass.is_none()
    }
}

/// How a conductor is connected at a terminal of a part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionKind {
    Screw,
    PushIn,
    Spring,
    Screwless,
    Lug,
    Pin,
    Faston,
    SolderPin,
    #[default]
    #[serde(other)]
    Unknown,
}

/// Side of the part a terminal faces when it is mounted in its normal
/// position (rail up, plate behind).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
    Front,
    #[default]
    #[serde(other)]
    Unknown,
}

/// One connection point of a part.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PartTerminal {
    /// Marking on the part: `1`, `A1`, `13`.
    pub name: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub kind: ConnectionKind,
    /// Smallest and largest conductor cross-section in mm².
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wire_mm2: Option<(f64, f64)>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub side: Side,
    /// Position of the entry point in the part's own frame in mm (origin at
    /// the back-left-bottom corner of the outline). Unknown until the part's
    /// geometry has been read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position_mm: Option<[f64; 3]>,
}

/// Where the part stands in its maker's range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lifecycle {
    Active,
    PhasingOut,
    Discontinued,
    /// The maker's data says nothing.
    #[default]
    #[serde(other)]
    Unknown,
}

/// A catalog price. The price is the maker's list price as imported, not an
/// offer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Price {
    pub amount: f64,
    /// ISO 4217 code, `EUR`.
    pub currency: String,
    /// Quantity the price is for (price per 100 pieces is `100`).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub per: u32,
    /// Date of the price list, `2026-10-15`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub date: String,
}

impl Price {
    /// Price of a single piece.
    pub fn each(&self) -> f64 {
        self.amount / f64::from(self.per.max(1))
    }
}

/// Another part, by maker and order number.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PartRef {
    pub manufacturer: String,
    pub order_number: String,
}

/// What kind of local file an [`Asset`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Step,
    Dxf2d,
    Mesh,
    Datasheet,
    Image,
    #[default]
    #[serde(other)]
    Other,
}

/// A file kept in the user's local library (never shipped, never shared).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Asset {
    pub kind: AssetKind,
    /// Path relative to the library root, `Sources/wago/2002-1201.stp`.
    pub path: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sha256: String,
}

/// Where a part came from. Anything not `authored` is excluded from export
/// bundles by the caller.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Provenance {
    /// `authored`, `import:bmecat`, `import:csv`, `import:step`, `import:edz`.
    pub source: String,
    /// File the data was read from.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub file: String,
    /// Import date, `2026-10-15`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub imported: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub licence_note: String,
}

impl Provenance {
    /// Whether the part may go into a shared bundle (we wrote it ourselves).
    pub fn redistributable(&self) -> bool {
        self.source == "authored"
    }
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

fn one() -> u32 {
    1
}

fn is_one(value: &u32) -> bool {
    *value == 1
}

/// Where a numeric value in a part's `data` came from. A key without an
/// entry in `CatalogPart::sources` has an unknown source, and calculations
/// show it as such.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataSource {
    /// Read from the maker's datasheet or catalog file.
    MakerDatasheet,
    /// Typed in by a person.
    Typed,
    /// Estimated by KoperCAD or the user; not from the maker.
    Estimated,
    #[default]
    #[serde(other)]
    Unknown,
}

/// One point of a protective device's let-through table at a prospective
/// short-circuit current.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LetThroughPoint {
    /// Prospective short-circuit current in kA.
    pub ik_ka: f64,
    /// Let-through energy in A²s.
    pub i2t_a2s: f64,
    /// Peak let-through current in kA.
    pub ip_ka: f64,
}

/// Stable `data` keys for protection and cable engineering (read by the
/// calculation and coordination code). Values are SI units as the suffix
/// says; text keys are noted. Missing key means unknown.
pub mod eng {
    // Protective devices
    pub const RATED_CURRENT_A: &str = "rated_current_a";
    pub const ICU_KA: &str = "breaking_capacity_icu_ka";
    pub const ICS_KA: &str = "breaking_capacity_ics_ka";
    pub const BREAKING_TEST_VOLTAGE_V: &str = "breaking_test_voltage_v";
    /// Text: `B`, `C`, `D`, `K`, `Z` or a maker curve id.
    pub const TRIP_CURVE: &str = "trip_curve";
    pub const MAGNETIC_TRIP_LOW_X_IN: &str = "magnetic_trip_low_x_in";
    pub const MAGNETIC_TRIP_HIGH_X_IN: &str = "magnetic_trip_high_x_in";
    pub const IR_MIN_A: &str = "ir_min_a";
    pub const IR_MAX_A: &str = "ir_max_a";
    pub const ISD_MIN_A: &str = "isd_min_a";
    pub const ISD_MAX_A: &str = "isd_max_a";
    pub const II_MIN_A: &str = "ii_min_a";
    pub const II_MAX_A: &str = "ii_max_a";
    /// Text: `gG`, `aM`, `gR`.
    pub const FUSE_CLASS: &str = "fuse_class";
    pub const POLES: &str = "poles";
    pub const SCCR_KA: &str = "sccr_ka";
    /// Text: the max upstream fuse class of a tested SCCR combination.
    pub const SCCR_UPSTREAM_FUSE_CLASS: &str = "sccr_upstream_fuse_class";
    pub const POWER_LOSS_W: &str = "power_loss_w";
    // Contactors
    pub const AC3_CURRENT_A: &str = "ac3_current_a";
    pub const AC1_CURRENT_A: &str = "ac1_current_a";
    pub const MAX_SCPD_TYPE1_A: &str = "max_scpd_type1_a";
    pub const MAX_SCPD_TYPE2_A: &str = "max_scpd_type2_a";
    pub const COIL_VOLTAGE_V: &str = "coil_voltage_v";
    // Overload relays
    pub const SETTING_MIN_A: &str = "setting_min_a";
    pub const SETTING_MAX_A: &str = "setting_max_a";
    /// Text: `10A`, `10`, `20`, `30`.
    pub const TRIP_CLASS: &str = "trip_class";
    // Motors
    pub const RATED_POWER_KW: &str = "rated_power_kw";
    pub const EFFICIENCY: &str = "efficiency";
    pub const COS_PHI: &str = "cos_phi";
    /// Ia / In.
    pub const LOCKED_ROTOR_RATIO: &str = "locked_rotor_ratio";
    /// Text: `dol`, `star_delta`, `soft_starter`, `vfd`.
    pub const STARTING_METHOD: &str = "starting_method";
    // Cables and wires
    pub const SECTION_MM2: &str = "section_mm2";
    /// Text: `cu`, `al`.
    pub const CONDUCTOR_MATERIAL: &str = "conductor_material";
    /// Text: `pvc`, `xlpe`, `epr`, `halogen_free`.
    pub const INSULATION: &str = "insulation";
    pub const CORE_COUNT: &str = "core_count";
    pub const R20_OHM_PER_KM: &str = "r20_ohm_per_km";
    pub const X_OHM_PER_KM: &str = "x_ohm_per_km";
    pub const MAX_TEMP_C: &str = "max_temp_c";
    pub const OUTER_DIAMETER_MM: &str = "outer_diameter_mm";
    // Terminals, busbars, distribution blocks use RATED_CURRENT_A and SCCR_KA.
    // Transformers, UPS, power supplies
    pub const SN_KVA: &str = "sn_kva";
    pub const UK_PERCENT: &str = "uk_percent";
    pub const PK_W: &str = "pk_w";
    /// Text: `Dyn11`.
    pub const VECTOR_GROUP: &str = "vector_group";
}
