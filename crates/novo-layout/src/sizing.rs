//! Enclosure sizing helper: from the devices, the free space and duct
//! allowance, and the heat load, recommend a mounting plate and enclosure
//! and say whether natural cooling is enough.
//!
//! Pure functions on plain numbers, millimetres and watts. The list of
//! enclosure sizes is the user's own ([`EnclosureCatalog`]): typed in or
//! imported from a CSV file, nothing is bundled from makers.
//!
//! **Planning values only.** The area rule is a quick estimate (the real
//! plate fit is the `place all` planner), and the heat balance follows the
//! effective-surface method of IEC 60890 with the usual sheet-steel constant.
//! The constants are named below so they can be checked against the
//! enclosure maker's handbook; the enclosure maker's own sizing wins.

use std::fmt;

use crate::DEFAULT_CLEARANCE_MM;

/// Heat transfer coefficient through the enclosure wall, W/(m2 K), per material.
/// Sheet steel about 5.5 is the figure quoted in maker handbooks; the others
/// are typical planning values. Verify against the maker before relying on them.
pub const K_STEEL: f64 = 5.5;
pub const K_STAINLESS: f64 = 4.5;
pub const K_ALUMINIUM: f64 = 12.0;
pub const K_PLASTIC: f64 = 3.5;

/// Air-flow constant for a fan, m3/h per (W/K), at sea level.
pub const FAN_FACTOR: f64 = 3.1;

/// Wall material of an enclosure; sets the heat transfer coefficient.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Material {
    #[default]
    Steel,
    Stainless,
    Aluminium,
    Plastic,
}

impl Material {
    pub fn k(self) -> f64 {
        match self {
            Material::Steel => K_STEEL,
            Material::Stainless => K_STAINLESS,
            Material::Aluminium => K_ALUMINIUM,
            Material::Plastic => K_PLASTIC,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Material::Steel => "steel",
            Material::Stainless => "stainless",
            Material::Aluminium => "aluminium",
            Material::Plastic => "plastic",
        }
    }

    fn parse(text: &str) -> Option<Material> {
        match text.trim().to_lowercase().as_str() {
            "" | "steel" | "staal" | "sheet steel" => Some(Material::Steel),
            "stainless" | "stainless steel" | "rvs" => Some(Material::Stainless),
            "aluminium" | "aluminum" => Some(Material::Aluminium),
            "plastic" | "polyester" | "kunststof" => Some(Material::Plastic),
            _ => None,
        }
    }
}

/// One enclosure from the user's list. `width`, `height` and `depth` are the
/// outside dimensions (they set the cooling surface); the plate is the
/// mounting plate that fits inside, `margin` the unusable border of it.
#[derive(Debug, Clone, PartialEq)]
pub struct EnclosureSize {
    pub name: String,
    pub width: f64,
    pub height: f64,
    pub depth: f64,
    pub plate_width: f64,
    pub plate_height: f64,
    pub margin: f64,
    pub material: Material,
}

impl EnclosureSize {
    pub fn plate_usable(&self) -> (f64, f64) {
        (
            (self.plate_width - 2.0 * self.margin).max(0.0),
            (self.plate_height - 2.0 * self.margin).max(0.0),
        )
    }

    /// Usable plate area in mm2.
    pub fn plate_area(&self) -> f64 {
        let (w, h) = self.plate_usable();
        w * h
    }
}

/// A problem found while reading a CSV list, with the 1-based line number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

/// The user's list of enclosure sizes. Starts empty.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EnclosureCatalog {
    pub items: Vec<EnclosureSize>,
}

const CSV_HEADER: &str =
    "name;width_mm;height_mm;depth_mm;plate_width_mm;plate_height_mm;margin_mm;material";

impl EnclosureCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds or replaces (same name, ignoring case) an enclosure.
    pub fn upsert(&mut self, item: EnclosureSize) {
        match self
            .items
            .iter_mut()
            .find(|i| i.name.eq_ignore_ascii_case(&item.name))
        {
            Some(slot) => *slot = item,
            None => self.items.push(item),
        }
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|i| !i.name.eq_ignore_ascii_case(name));
        self.items.len() != before
    }

    /// Reads a list from CSV text. Columns, in order: name, width, height,
    /// depth, plate width, plate height, margin (optional, default 0),
    /// material (optional, default steel); millimetres. Separator is `;`,
    /// `,` or tab (what a spreadsheet export produces), decimal comma
    /// accepted with `;`. A header row and blank lines are skipped. Good rows
    /// are returned together with the errors of bad ones.
    pub fn from_csv(text: &str) -> (EnclosureCatalog, Vec<CatalogError>) {
        let mut catalog = EnclosureCatalog::new();
        let mut errors = Vec::new();
        for (index, raw) in text.lines().enumerate() {
            let line = index + 1;
            let raw = raw.trim().trim_start_matches('\u{feff}');
            if raw.is_empty() {
                continue;
            }
            let separator = if raw.contains(';') {
                ';'
            } else if raw.contains('\t') {
                '\t'
            } else {
                ','
            };
            let cells: Vec<&str> = raw.split(separator).map(str::trim).collect();
            // Header row.
            if line == 1 && cells[0].eq_ignore_ascii_case("name") {
                continue;
            }
            if cells.len() < 6 {
                errors.push(CatalogError {
                    line,
                    message: "needs at least name, width, height, depth, plate width, plate height"
                        .into(),
                });
                continue;
            }
            let mut numbers = [0.0; 6];
            let mut bad = None;
            for (slot, cell) in numbers.iter_mut().zip(&cells[1..6]) {
                match parse_mm(cell, separator) {
                    Some(v) if v > 0.0 => *slot = v,
                    _ => bad = Some(*cell),
                }
            }
            if let Some(cell) = bad {
                errors.push(CatalogError {
                    line,
                    message: format!("'{cell}' is not a positive size in mm"),
                });
                continue;
            }
            let margin = match cells.get(6).filter(|c| !c.is_empty()) {
                None => 0.0,
                Some(cell) => match parse_mm(cell, separator) {
                    Some(v) if v >= 0.0 => v,
                    _ => {
                        errors.push(CatalogError {
                            line,
                            message: format!("'{cell}' is not a margin in mm"),
                        });
                        continue;
                    }
                },
            };
            let material = match Material::parse(cells.get(7).copied().unwrap_or("")) {
                Some(m) => m,
                None => {
                    errors.push(CatalogError {
                        line,
                        message: format!("unknown material '{}'", cells[7]),
                    });
                    continue;
                }
            };
            if cells[0].is_empty() {
                errors.push(CatalogError {
                    line,
                    message: "name is empty".into(),
                });
                continue;
            }
            catalog.upsert(EnclosureSize {
                name: cells[0].to_string(),
                width: numbers[0],
                height: numbers[1],
                depth: numbers[2],
                plate_width: numbers[3],
                plate_height: numbers[4],
                margin,
                material,
            });
        }
        (catalog, errors)
    }

    /// The list as CSV text that [`EnclosureCatalog::from_csv`] reads back.
    pub fn to_csv(&self) -> String {
        let mut out = String::from(CSV_HEADER);
        out.push('\n');
        for i in &self.items {
            out.push_str(&format!(
                "{};{};{};{};{};{};{};{}\n",
                i.name.replace(';', " "),
                i.width,
                i.height,
                i.depth,
                i.plate_width,
                i.plate_height,
                i.margin,
                i.material.name()
            ));
        }
        out
    }
}

fn parse_mm(cell: &str, separator: char) -> Option<f64> {
    let cell = cell.trim();
    let cell = if separator == ';' {
        cell.replace(',', ".")
    } else {
        cell.to_string()
    };
    cell.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// A device to fit. `depth` zero means unknown (not checked). `power_loss`
/// is the part's loss at full load in watts; `load_factor` scales it
/// (1.0 when the part is loaded to its rating).
#[derive(Debug, Clone, PartialEq)]
pub struct SizingDevice {
    pub width: f64,
    pub height: f64,
    pub depth: f64,
    pub power_loss: f64,
    pub load_factor: f64,
    /// Extra gap this device needs on every side, if more than the flat gap.
    pub clearance: Option<f64>,
    /// True when the footprint is a default, not the part's real size.
    pub guessed: bool,
}

/// What the user asks for besides the devices.
#[derive(Debug, Clone, PartialEq)]
pub struct SizingRequest {
    /// Flat edge-to-edge gap, mm.
    pub gap: f64,
    /// Share of the plate kept free for later additions, 0.2 is 20 percent.
    pub free_space: f64,
    /// Share of the plate taken by wire ducts, 0.15 is 15 percent.
    pub duct_allowance: f64,
    /// Space between the device front and the door that is not usable, mm.
    pub depth_reserve: f64,
    pub installation: Installation,
    /// Highest ambient temperature outside the enclosure, degrees C.
    pub ambient_max: f64,
    /// Highest allowed temperature inside, degrees C.
    pub inside_max: f64,
}

impl Default for SizingRequest {
    fn default() -> Self {
        SizingRequest {
            gap: DEFAULT_CLEARANCE_MM,
            free_space: 0.2,
            duct_allowance: 0.2,
            depth_reserve: 20.0,
            installation: Installation::FreeStanding,
            ambient_max: 35.0,
            inside_max: 40.0,
        }
    }
}

/// Which faces of the enclosure touch a wall or neighbour or are covered.
/// Covered faces radiate less, see [`effective_surface`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Installation {
    pub rear_covered: bool,
    pub left_covered: bool,
    pub right_covered: bool,
    pub top_covered: bool,
    /// Standing on the floor: the bottom gives off no heat (IEC 60890).
    pub on_floor: bool,
}

#[allow(non_upper_case_globals)]
impl Installation {
    /// Floor standing, free on all other sides.
    pub const FreeStanding: Installation = Installation {
        rear_covered: false,
        left_covered: false,
        right_covered: false,
        top_covered: false,
        on_floor: true,
    };
    /// Wall mounted, bottom open to the air.
    pub const Wall: Installation = Installation {
        rear_covered: true,
        on_floor: false,
        ..Installation::FreeStanding
    };
    /// Wall mounted with a neighbour on both sides.
    pub const WallInRow: Installation = Installation {
        rear_covered: true,
        left_covered: true,
        right_covered: true,
        top_covered: false,
        on_floor: false,
    };
}

/// Effective cooling surface in m2, the IEC 60890 sum of face area times
/// the surface factor b: top 1.4 (0.7 covered), sides, front and rear 0.9
/// (0.5 covered), bottom 0.5 (0 when standing on the floor, so a free standing box is 1.8 H (W + D) + 1.4 W D). Dimensions are the outside size in mm.
pub fn effective_surface(width: f64, height: f64, depth: f64, install: Installation) -> f64 {
    let (w, h, d) = (width / 1000.0, height / 1000.0, depth / 1000.0);
    let face =
        |area: f64, covered: bool, free: f64, cov: f64| area * if covered { cov } else { free };
    let top = face(w * d, install.top_covered, 1.4, 0.7);
    let bottom = if install.on_floor { 0.0 } else { w * d * 0.5 };
    let front = w * h * 0.9;
    let rear = face(w * h, install.rear_covered, 0.9, 0.5);
    let left = face(d * h, install.left_covered, 0.9, 0.5);
    let right = face(d * h, install.right_covered, 0.9, 0.5);
    top + bottom + front + rear + left + right
}

/// Heat balance of one enclosure with a given loss.
#[derive(Debug, Clone, PartialEq)]
pub struct Thermal {
    /// Total power loss of the devices, W.
    pub loss: f64,
    pub surface: f64,
    /// Heat the walls give off at the allowed temperature difference, W.
    pub dissipation: f64,
    /// Inside temperature rise over the ambient at this loss, K.
    pub rise: f64,
    /// Inside temperature at the highest ambient, degrees C.
    pub inside_temperature: f64,
    /// True when `inside_temperature` stays within the allowed maximum.
    pub natural_ok: bool,
    /// Cooling power to add, W; 0 when natural cooling is enough.
    pub cooling_needed: f64,
    /// Fan air flow that would carry `cooling_needed`, m3/h; `None` when no
    /// cooling is needed or the allowed difference is zero or negative.
    pub fan_airflow: Option<f64>,
}

/// Total loss of the devices, W.
pub fn total_loss(devices: &[SizingDevice]) -> f64 {
    devices
        .iter()
        .map(|d| d.power_loss.max(0.0) * d.load_factor.clamp(0.0, 1.0))
        .sum()
}

/// Heat balance of `enclosure` carrying `loss` watts.
pub fn thermal(enclosure: &EnclosureSize, loss: f64, req: &SizingRequest) -> Thermal {
    let surface = effective_surface(
        enclosure.width,
        enclosure.height,
        enclosure.depth,
        req.installation,
    );
    let k = enclosure.material.k();
    let allowed = req.inside_max - req.ambient_max;
    let dissipation = k * surface * allowed.max(0.0);
    let rise = if k * surface > 0.0 {
        loss / (k * surface)
    } else {
        f64::INFINITY
    };
    let inside_temperature = req.ambient_max + rise;
    let cooling_needed = (loss - dissipation).max(0.0);
    let fan_airflow =
        (cooling_needed > 0.0 && allowed > 0.0).then(|| FAN_FACTOR * cooling_needed / allowed);
    Thermal {
        loss,
        surface,
        dissipation,
        rise,
        inside_temperature,
        natural_ok: cooling_needed <= 0.0,
        cooling_needed,
        fan_airflow,
    }
}

/// Why an enclosure was left out or what to look at.
#[derive(Debug, Clone, PartialEq)]
pub enum Note {
    /// The list of enclosures is empty.
    NoCatalog,
    /// Some device footprints are guessed, so the area is too.
    GuessedSizes(usize),
    /// No device lists a power loss, so the heat result is zero by omission.
    NoLossData,
    /// No enclosure in the list holds the devices.
    NothingFits,
    /// The recommended enclosure needs cooling; a larger one that cools
    /// naturally exists (name).
    LargerCoolsNaturally(String),
    /// No listed enclosure cools naturally.
    NoneCoolsNaturally,
    /// A device is larger than the plate of every listed enclosure (device
    /// index in the input).
    DeviceTooBig(usize),
    /// A device is deeper than the enclosure leaves room for (device index).
    TooDeep(usize),
}

/// The result of [`recommend`].
#[derive(Debug, Clone, PartialEq)]
pub struct Recommendation {
    /// Plate area the devices need with gaps, ducts and free space, mm2.
    pub required_plate_area: f64,
    /// Device area alone, mm2.
    pub device_area: f64,
    /// Total loss, W.
    pub loss: f64,
    /// Smallest listed enclosure that fits, by plate area.
    pub enclosure: Option<EnclosureSize>,
    /// Plate fill with the recommended enclosure, devices with gaps over
    /// the usable plate, 0..1.
    pub fill: Option<f64>,
    /// Heat balance of the recommended enclosure.
    pub thermal: Option<Thermal>,
    /// Smallest enclosure that fits and also cools naturally.
    pub natural: Option<EnclosureSize>,
    pub notes: Vec<Note>,
}

fn gapped(dev: &SizingDevice, req: &SizingRequest) -> f64 {
    let gap = dev.clearance.unwrap_or(req.gap).max(req.gap);
    (dev.width + gap) * (dev.height + gap)
}

/// Recommends a plate and enclosure. The smallest enclosure (by usable
/// plate area, then by volume) holding the devices is chosen; the thermal
/// result is for that enclosure, and `natural` names the smallest fitting
/// enclosure that needs no cooling, when the first one does.
pub fn recommend(
    devices: &[SizingDevice],
    catalog: &EnclosureCatalog,
    req: &SizingRequest,
) -> Recommendation {
    let device_area: f64 = devices.iter().map(|d| d.width * d.height).sum();
    let gapped_area: f64 = devices.iter().map(|d| gapped(d, req)).sum();
    let required = gapped_area
        / (1.0 - req.duct_allowance.clamp(0.0, 0.9))
        / (1.0 - req.free_space.clamp(0.0, 0.9));
    let loss = total_loss(devices);

    let mut notes = Vec::new();
    let guessed = devices.iter().filter(|d| d.guessed).count();
    if guessed > 0 {
        notes.push(Note::GuessedSizes(guessed));
    }
    if !devices.is_empty() && loss == 0.0 {
        notes.push(Note::NoLossData);
    }
    if catalog.items.is_empty() {
        notes.push(Note::NoCatalog);
    }

    let mut sorted: Vec<&EnclosureSize> = catalog.items.iter().collect();
    sorted.sort_by(|a, b| {
        a.plate_area()
            .total_cmp(&b.plate_area())
            .then((a.width * a.height * a.depth).total_cmp(&(b.width * b.height * b.depth)))
    });

    let holds = |e: &EnclosureSize| -> bool {
        let (pw, ph) = e.plate_usable();
        e.plate_area() >= required
            && devices.iter().all(|d| {
                let fits_plate =
                    (d.width <= pw && d.height <= ph) || (d.height <= pw && d.width <= ph);
                let fits_depth = d.depth <= 0.0 || d.depth + req.depth_reserve <= e.depth;
                fits_plate && fits_depth
            })
    };

    let chosen = sorted.iter().copied().find(|e| holds(e));
    let natural = chosen.and_then(|first| {
        sorted
            .iter()
            .copied()
            .filter(|e| holds(e))
            .find(|e| thermal(e, loss, req).natural_ok)
            .filter(|e| e.name != first.name)
    });

    if chosen.is_none() && !catalog.items.is_empty() {
        notes.push(Note::NothingFits);
        let max_plate = sorted
            .iter()
            .map(|e| e.plate_usable())
            .fold((0.0f64, 0.0f64), |a, b| (a.0.max(b.0), a.1.max(b.1)));
        let max_depth = sorted.iter().map(|e| e.depth).fold(0.0f64, f64::max);
        for (i, d) in devices.iter().enumerate() {
            let long = d.width.max(d.height);
            let short = d.width.min(d.height);
            if long > max_plate.0.max(max_plate.1) || short > max_plate.0.min(max_plate.1) {
                notes.push(Note::DeviceTooBig(i));
            } else if d.depth > 0.0 && d.depth + req.depth_reserve > max_depth {
                notes.push(Note::TooDeep(i));
            }
        }
    }

    let thermal_result = chosen.map(|e| thermal(e, loss, req));
    if let Some(t) = &thermal_result
        && !t.natural_ok
    {
        match &natural {
            Some(n) => notes.push(Note::LargerCoolsNaturally(n.name.clone())),
            None => notes.push(Note::NoneCoolsNaturally),
        }
    }

    Recommendation {
        required_plate_area: required,
        device_area,
        loss,
        fill: chosen.map(|e| gapped_area / e.plate_area()),
        enclosure: chosen.cloned(),
        thermal: thermal_result,
        natural: natural.cloned(),
        notes,
    }
}
