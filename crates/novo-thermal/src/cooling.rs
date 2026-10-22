//! Cooling selection: from the heat of a cabinet, propose a fan, a filter
//! fan or a cooling unit that holds a target temperature inside.
//!
//! **Method.** The enclosure walls give off `k * A * (target - ambient)` watts
//! at the target (IEC 60890 effective surface `A`, wall coefficient `k`, the
//! same numbers as `novo_layout::sizing`). What the walls cannot carry is left
//! for the cooling product:
//!
//! ```text
//! needed = loss - k * A * (target - ambient)        (never below 0)
//! ```
//!
//! A target below the ambient makes the wall a heat *source*, so the need
//! grows; the formula covers it. A fan can only bring in air that is colder
//! than the target, so it needs `target - ambient >= MIN_FAN_DELTA_K`; it
//! carries `needed` with `FAN_FACTOR * needed / (target - ambient)` m3/h of
//! installed flow (the maker's free-blowing figure is derated for the filter
//! and the grille). A cooling unit holds the set point, so its rated power
//! has to cover `needed` plus a margin.
//!
//! **Data.** A product is `Maker` when it comes from a part in the project
//! (properties `airflow_m3h`, `cooling_capacity_w`) or from the person's own
//! list, and `Planning` when it is a generic size from the ladder below. A
//! pick from real data is always preferred; a planning pick says so.
//!
//! **Honesty.** The loss is the model's (rated loss times load factor, plus
//! any wire heat the caller adds), the wall balance is the standard's
//! estimate, and the derating factors are typical. The maker's selection
//! tool and datasheet curves win; this is the proposal to start from.

use std::fmt;

use novo_core::{Part, Project, Properties, Value};
use novo_layout::sizing::{FAN_FACTOR, Installation, Material, effective_surface};

/// Smallest `target - ambient` for which a fan is proposed, K. Below it the
/// flow needed grows without bound and the air hardly cools anything.
pub const MIN_FAN_DELTA_K: f64 = 3.0;
/// Share of a free-blowing fan flow that is left through a grille.
pub const FAN_DERATE: f64 = 0.9;
/// Share of a filter fan's free-blowing flow that is left with the filter
/// mat and outlet filter fitted (clean, planning value).
pub const FILTER_FAN_DERATE: f64 = 0.7;
/// Safety margin on a cooling unit, as a factor on the needed power.
pub const UNIT_MARGIN: f64 = 1.1;
/// Most identical fans proposed side by side.
pub const MAX_FANS: usize = 3;

/// Property keys a part carries for cooling.
pub mod keys {
    /// Free-blowing air flow, m3/h.
    pub const AIRFLOW_M3H: &str = "airflow_m3h";
    /// Rated cooling power, W (at the maker's rating point).
    pub const COOLING_CAPACITY_W: &str = "cooling_capacity_w";
    /// `fan`, `filter_fan` or `cooling_unit`; guessed from the other keys when absent.
    pub const COOLING_KIND: &str = "cooling_kind";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Fan,
    FilterFan,
    CoolingUnit,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Fan => "Fan",
            Kind::FilterFan => "Filter fan",
            Kind::CoolingUnit => "Cooling unit",
        }
    }

    fn parse(text: &str) -> Option<Kind> {
        match text.trim().to_lowercase().replace(['_', '-'], " ").as_str() {
            "fan" => Some(Kind::Fan),
            "filter fan" | "filterfan" | "fan with filter" => Some(Kind::FilterFan),
            "cooling unit" | "coolingunit" | "cooler" | "air conditioner" | "ac" => {
                Some(Kind::CoolingUnit)
            }
            _ => None,
        }
    }

    fn derate(self) -> f64 {
        match self {
            Kind::Fan => FAN_DERATE,
            Kind::FilterFan => FILTER_FAN_DERATE,
            Kind::CoolingUnit => 1.0,
        }
    }

    /// Opens the enclosure to the room air (dust, humidity, IP rating).
    pub fn opens_enclosure(self) -> bool {
        self != Kind::CoolingUnit
    }
}

/// Where a product's numbers come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// A part of this project; the text is its order number.
    Part(String),
    /// The person's own list.
    Mine,
    /// A generic size of the ladder: a planning value, not an article.
    Planning,
}

impl Source {
    pub fn is_real(&self) -> bool {
        !matches!(self, Source::Planning)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Product {
    pub name: String,
    pub kind: Kind,
    /// Free-blowing flow of a fan, m3/h.
    pub airflow_m3h: f64,
    /// Rated power of a cooling unit, W.
    pub cooling_w: f64,
    pub source: Source,
}

impl Product {
    /// Flow left in service, m3/h.
    pub fn installed_flow(&self) -> f64 {
        self.airflow_m3h * self.kind.derate()
    }

    fn size(&self) -> f64 {
        match self.kind {
            Kind::CoolingUnit => self.cooling_w,
            _ => self.airflow_m3h,
        }
    }
}

/// Generic sizes, labelled as planning values wherever they are shown.
pub fn planning_products() -> Vec<Product> {
    let mut out = Vec::new();
    for flow in [50.0, 100.0, 150.0, 250.0, 400.0, 600.0, 800.0] {
        out.push(Product {
            name: format!("Generic filter fan {flow:.0} m3/h"),
            kind: Kind::FilterFan,
            airflow_m3h: flow,
            cooling_w: 0.0,
            source: Source::Planning,
        });
    }
    for watts in [300.0, 500.0, 800.0, 1000.0, 1500.0, 2000.0, 3000.0, 4000.0] {
        out.push(Product {
            name: format!("Generic cooling unit {watts:.0} W"),
            kind: Kind::CoolingUnit,
            airflow_m3h: 0.0,
            cooling_w: watts,
            source: Source::Planning,
        });
    }
    out
}

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Float(v) => Some(*v),
        Value::Int(v) => Some(*v as f64),
        _ => None,
    }
}

fn positive(properties: &Properties, key: &str) -> Option<f64> {
    properties
        .get(key)
        .and_then(number)
        .filter(|v| v.is_finite() && *v > 0.0)
}

/// Fans and cooling units among the parts of the project: the parts that
/// carry `airflow_m3h` or `cooling_capacity_w`.
pub fn products_from_project(project: &Project) -> Vec<Product> {
    let mut out = Vec::new();
    for part in project.parts() {
        if let Some(p) = product_of_part(part) {
            out.push(p);
        }
    }
    out
}

fn product_of_part(part: &Part) -> Option<Product> {
    let flow = positive(&part.properties, keys::AIRFLOW_M3H);
    let power = positive(&part.properties, keys::COOLING_CAPACITY_W);
    if flow.is_none() && power.is_none() {
        return None;
    }
    let stated = match part.properties.get(keys::COOLING_KIND) {
        Some(Value::Text(t)) => Kind::parse(t),
        _ => None,
    };
    let kind = stated.unwrap_or(if power.is_some() && flow.is_none() {
        Kind::CoolingUnit
    } else if part.description.to_lowercase().contains("filter")
        || part.type_number.to_lowercase().contains("filter")
    {
        Kind::FilterFan
    } else {
        Kind::Fan
    });
    let name = if part.type_number.is_empty() {
        format!("{} {}", part.manufacturer, part.order_number)
    } else {
        format!("{} {}", part.manufacturer, part.type_number)
    };
    Some(Product {
        name: name.trim().to_owned(),
        kind,
        airflow_m3h: flow.unwrap_or(0.0),
        cooling_w: power.unwrap_or(0.0),
        source: Source::Part(part.order_number.clone()),
    })
}

/// A problem found while reading the person's list, with its 1-based line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ListError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

const CSV_HEADER: &str = "name;kind;airflow_m3h;cooling_w";

/// The person's own list: `name;kind;airflow_m3h;cooling_w`, `;`, `,` or tab
/// separated, decimal comma accepted with `;`. A header and blank lines are
/// skipped; good rows are returned with the errors of bad ones.
pub fn products_from_csv(text: &str) -> (Vec<Product>, Vec<ListError>) {
    let mut out = Vec::new();
    let mut errors = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let raw = raw.trim().trim_start_matches('\u{feff}');
        if raw.is_empty() {
            continue;
        }
        let sep = if raw.contains(';') {
            ';'
        } else if raw.contains('\t') {
            '\t'
        } else {
            ','
        };
        let cells: Vec<&str> = raw.split(sep).map(str::trim).collect();
        if line == 1 && cells[0].eq_ignore_ascii_case("name") {
            continue;
        }
        let mut fail = |message: String| errors.push(ListError { line, message });
        if cells.len() < 3 || cells[0].is_empty() {
            fail("needs a name, a kind and an air flow or a cooling power".into());
            continue;
        }
        let Some(kind) = Kind::parse(cells[1]) else {
            fail(format!(
                "'{}' is not a kind (fan, filter fan, cooling unit)",
                cells[1]
            ));
            continue;
        };
        let read = |cell: Option<&&str>| -> Result<f64, String> {
            let cell = cell.copied().unwrap_or("");
            if cell.is_empty() {
                return Ok(0.0);
            }
            let cell = if sep == ';' {
                cell.replace(',', ".")
            } else {
                cell.to_owned()
            };
            cell.parse::<f64>()
                .ok()
                .filter(|v| v.is_finite() && *v >= 0.0)
                .ok_or_else(|| format!("'{cell}' is not a positive number"))
        };
        let (flow, power) = match (read(cells.get(2)), read(cells.get(3))) {
            (Ok(f), Ok(p)) => (f, p),
            (Err(e), _) | (_, Err(e)) => {
                fail(e);
                continue;
            }
        };
        let size = if kind == Kind::CoolingUnit {
            power
        } else {
            flow
        };
        if size <= 0.0 {
            fail(match kind {
                Kind::CoolingUnit => "a cooling unit needs a cooling power in W".into(),
                _ => "a fan needs an air flow in m3/h".into(),
            });
            continue;
        }
        out.push(Product {
            name: cells[0].to_owned(),
            kind,
            airflow_m3h: flow,
            cooling_w: power,
            source: Source::Mine,
        });
    }
    (out, errors)
}

/// The list as CSV text that [`products_from_csv`] reads back (only the
/// person's own products).
pub fn products_to_csv(products: &[Product]) -> String {
    let mut out = String::from(CSV_HEADER);
    out.push('\n');
    for p in products.iter().filter(|p| p.source == Source::Mine) {
        let kind = match p.kind {
            Kind::Fan => "fan",
            Kind::FilterFan => "filter fan",
            Kind::CoolingUnit => "cooling unit",
        };
        out.push_str(&format!(
            "{};{kind};{};{}\n",
            p.name.replace(';', " "),
            p.airflow_m3h,
            p.cooling_w
        ));
    }
    out
}

/// What the selection is made for.
#[derive(Debug, Clone, PartialEq)]
pub struct Basis {
    /// Heat to carry out, W: device losses at their load factor, plus wire
    /// heat when the caller has it.
    pub loss_w: f64,
    /// Highest air temperature outside, degrees C.
    pub ambient_c: f64,
    /// Highest temperature allowed inside, degrees C.
    pub target_c: f64,
    /// Outside dimensions, mm.
    pub width_mm: f64,
    pub height_mm: f64,
    pub depth_mm: f64,
    pub material: Material,
    pub installation: Installation,
    /// The enclosure must stay closed (dust, spray, IP rating): fans are out.
    pub closed_required: bool,
}

/// The heat balance behind the proposal.
#[derive(Debug, Clone, PartialEq)]
pub struct Need {
    pub loss_w: f64,
    /// Effective surface, m2, and wall coefficient, W/(m2 K).
    pub surface_m2: f64,
    pub k: f64,
    /// `target - ambient`, K (negative when the target is below the ambient).
    pub delta_k: f64,
    /// What the walls carry at the target, W (negative: they bring heat in).
    pub wall_w: f64,
    /// Power the cooling product has to remove, W; 0 when the walls suffice.
    pub cooling_w: f64,
    /// Inside temperature with no cooling product, degrees C.
    pub natural_inside_c: f64,
    /// Installed air flow that would carry `cooling_w`, m3/h; `None` when no
    /// cooling is needed or a fan is out (`fan_blocked`).
    pub airflow_m3h: Option<f64>,
    /// Why a fan is out, if it is.
    pub fan_blocked: Option<String>,
}

impl Need {
    pub fn natural_ok(&self) -> bool {
        self.cooling_w <= 0.0
    }
}

/// A product, how many of it, and what that gives.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    pub product: Product,
    pub count: usize,
    /// Installed flow (fans) or rated power (units) of all of them.
    pub total: f64,
    /// Total over what is needed, as a factor (1.0 = exactly enough).
    pub headroom: f64,
    /// Inside temperature with this choice at the highest ambient, degrees C.
    pub inside_c: f64,
}

/// One kind of cooling and whether anything covers the need.
#[derive(Debug, Clone, PartialEq)]
pub struct Option_ {
    pub kind: Kind,
    /// `None`: no listed product is large enough (or the kind is out).
    pub choice: Option<Choice>,
    pub why_not: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    pub need: Need,
    pub options: Vec<Option_>,
    /// Index into `options` of the proposal; `None` when none is needed or
    /// none can be made.
    pub recommended: Option<usize>,
}

impl Selection {
    pub fn proposal(&self) -> Option<&Choice> {
        self.recommended
            .and_then(|i| self.options[i].choice.as_ref())
    }

    /// One sentence for a panel or a report.
    pub fn sentence(&self) -> String {
        let n = &self.need;
        if n.natural_ok() {
            return format!(
                "No cooling needed: the walls carry the {:.0} W at {:.0} °C inside (model estimate, planning values).",
                n.loss_w, n.natural_inside_c
            );
        }
        match self.proposal() {
            Some(c) => {
                let what = if c.count > 1 {
                    format!("{} x {}", c.count, c.product.name)
                } else {
                    c.product.name.clone()
                };
                let data = if c.product.source.is_real() {
                    "from part data"
                } else {
                    "planning size, not an article"
                };
                format!(
                    "Needs about {:.0} W of cooling: {what} ({data}) holds about {:.0} °C inside.",
                    n.cooling_w, c.inside_c
                )
            }
            None => format!(
                "Needs about {:.0} W of cooling and no listed product is large enough: use a larger enclosure or ask the maker.",
                n.cooling_w
            ),
        }
    }

    /// The lines a report prints: the sentence, the balance, one line per
    /// kind and the standing caveat.
    pub fn report_lines(&self, basis: &Basis) -> Vec<String> {
        let n = &self.need;
        let mut out = vec![self.sentence()];
        out.push(format!(
            "Balance: {:.0} W loss; walls carry {:.0} W at {:.0} °C inside and {:.0} °C outside (k {:.1} W/m2K, surface {:.2} m2).",
            n.loss_w, n.wall_w, basis.target_c, basis.ambient_c, n.k, n.surface_m2
        ));
        for o in &self.options {
            out.push(match (&o.choice, &o.why_not) {
                (Some(c), _) => format!(
                    "{}: {} x {} ({}), {:.0} {} for {:.0} needed, {:.0} °C inside.",
                    o.kind.label(),
                    c.count,
                    c.product.name,
                    if c.product.source.is_real() {
                        "part data"
                    } else {
                        "planning size"
                    },
                    c.total,
                    if o.kind == Kind::CoolingUnit {
                        "W"
                    } else {
                        "m3/h"
                    },
                    if o.kind == Kind::CoolingUnit {
                        n.cooling_w * UNIT_MARGIN
                    } else {
                        n.airflow_m3h.unwrap_or(0.0)
                    },
                    c.inside_c
                ),
                (None, Some(why)) => format!("{}: {why}", o.kind.label()),
                (None, None) => format!("{}: no listed product is large enough.", o.kind.label()),
            });
        }
        out.push(
            "Model estimate with planning values (IEC 60890 wall balance, typical filter derating); the maker's selection tool and datasheet curves decide."
                .to_owned(),
        );
        out
    }
}

/// The heat balance of `basis`.
pub fn need(basis: &Basis) -> Need {
    let surface = effective_surface(
        basis.width_mm,
        basis.height_mm,
        basis.depth_mm,
        basis.installation,
    );
    let k = basis.material.k();
    let delta = basis.target_c - basis.ambient_c;
    let wall = k * surface * delta;
    let cooling = (basis.loss_w - wall).max(0.0);
    let natural = if k * surface > 0.0 {
        basis.ambient_c + basis.loss_w / (k * surface)
    } else {
        f64::INFINITY
    };
    let fan_blocked = if cooling <= 0.0 {
        None
    } else if basis.closed_required {
        Some("the enclosure has to stay closed".to_owned())
    } else if delta < MIN_FAN_DELTA_K {
        Some(format!(
            "the target is only {delta:.0} K above the outside air (a fan needs at least {MIN_FAN_DELTA_K:.0} K)"
        ))
    } else {
        None
    };
    let airflow = (cooling > 0.0 && fan_blocked.is_none()).then(|| FAN_FACTOR * cooling / delta);
    Need {
        loss_w: basis.loss_w,
        surface_m2: surface,
        k,
        delta_k: delta,
        wall_w: wall,
        cooling_w: cooling,
        natural_inside_c: natural,
        airflow_m3h: airflow,
        fan_blocked,
    }
}

/// Proposes cooling for `basis` from `products`: real data first, planning
/// sizes when no real product is large enough.
pub fn select(basis: &Basis, products: &[Product]) -> Selection {
    let need = need(basis);
    let mut options = Vec::new();
    for kind in [Kind::FilterFan, Kind::Fan, Kind::CoolingUnit] {
        let own: Vec<&Product> = products.iter().filter(|p| p.kind == kind).collect();
        // The plain fan is only offered from real data: a generic one would
        // double the filter fan row with a worse promise.
        let own: Vec<&Product> = if kind == Kind::Fan {
            own.into_iter().filter(|p| p.source.is_real()).collect()
        } else {
            own
        };
        if own.is_empty() && kind == Kind::Fan {
            continue;
        }
        options.push(option_for(kind, &own, basis, &need));
    }
    let order: &[Kind] = if basis.closed_required {
        &[Kind::CoolingUnit]
    } else {
        &[Kind::FilterFan, Kind::Fan, Kind::CoolingUnit]
    };
    let recommended = if need.natural_ok() {
        None
    } else {
        order.iter().find_map(|k| {
            options
                .iter()
                .position(|o| o.kind == *k && o.choice.is_some())
        })
    };
    Selection {
        need,
        options,
        recommended,
    }
}

fn option_for(kind: Kind, products: &[&Product], basis: &Basis, need: &Need) -> Option_ {
    let none = |why: Option<String>| Option_ {
        kind,
        choice: None,
        why_not: why,
    };
    if need.natural_ok() {
        return none(Some("not needed".to_owned()));
    }
    if kind.opens_enclosure()
        && let Some(why) = &need.fan_blocked
    {
        return none(Some(format!("out: {why}")));
    }
    let kt = need.k * need.surface_m2;
    // Real data first, then the generic sizes.
    for real in [true, false] {
        let mut pool: Vec<&&Product> = products
            .iter()
            .filter(|p| p.source.is_real() == real && p.size() > 0.0)
            .collect();
        pool.sort_by(|a, b| a.size().total_cmp(&b.size()));
        let mut best: Option<Choice> = None;
        for p in pool {
            for count in 1..=if kind == Kind::CoolingUnit {
                1
            } else {
                MAX_FANS
            } {
                let (total, inside, headroom) = if kind == Kind::CoolingUnit {
                    let total = p.cooling_w * count as f64;
                    let wanted = need.cooling_w * UNIT_MARGIN;
                    let inside = if total >= need.cooling_w {
                        basis.target_c
                    } else {
                        basis.ambient_c + (basis.loss_w - total).max(0.0) / kt
                    };
                    (total, inside, total / wanted)
                } else {
                    let total = p.installed_flow() * count as f64;
                    let wanted = need.airflow_m3h.unwrap_or(f64::INFINITY);
                    let inside = basis.ambient_c + basis.loss_w / (kt + total / FAN_FACTOR);
                    (total, inside, total / wanted)
                };
                if headroom >= 1.0 {
                    let cand = Choice {
                        product: (*p).clone(),
                        count,
                        total,
                        headroom,
                        inside_c: inside,
                    };
                    // Fewest units, then the least total.
                    let better = best.as_ref().is_none_or(|b| {
                        (cand.count, cand.total).partial_cmp(&(b.count, b.total))
                            == Some(std::cmp::Ordering::Less)
                    });
                    if better {
                        best = Some(cand);
                    }
                    break;
                }
            }
        }
        if best.is_some() {
            return Option_ {
                kind,
                choice: best,
                why_not: None,
            };
        }
    }
    none(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 800 x 1000 x 300 mm sheet steel on a wall, free sides.
    fn basis(loss: f64, ambient: f64, target: f64) -> Basis {
        Basis {
            loss_w: loss,
            ambient_c: ambient,
            target_c: target,
            width_mm: 800.0,
            height_mm: 1000.0,
            depth_mm: 300.0,
            material: Material::Steel,
            installation: Installation::Wall,
            closed_required: false,
        }
    }

    #[test]
    fn the_wall_balance_matches_a_hand_calculation() {
        // Wall: top 0.8*0.3*1.4 = 0.336, bottom 0.8*0.3*0.5 = 0.12,
        // front 0.8*1.0*0.9 = 0.72, rear covered 0.8*1.0*0.5 = 0.4,
        // sides 2 * 0.3*1.0*0.9 = 0.54; total 2.116 m2.
        let n = need(&basis(500.0, 35.0, 40.0));
        assert!((n.surface_m2 - 2.116).abs() < 1e-9);
        // 5.5 * 2.116 * 5 K = 58.19 W through the walls.
        assert!((n.wall_w - 58.19).abs() < 1e-9);
        assert!((n.cooling_w - 441.81).abs() < 1e-9);
        // Flow: 3.1 * 441.81 / 5 = 273.9 m3/h.
        assert!((n.airflow_m3h.unwrap() - 273.9222).abs() < 1e-3);
        // No cooling: 35 + 500 / (5.5 * 2.116) = 77.96 degrees.
        assert!((n.natural_inside_c - (35.0 + 500.0 / 11.638)).abs() < 1e-9);
        assert!((n.natural_inside_c - 77.96).abs() < 0.01);
    }

    #[test]
    fn small_loss_needs_no_cooling() {
        let s = select(&basis(40.0, 35.0, 40.0), &planning_products());
        assert!(s.need.natural_ok());
        assert!(s.recommended.is_none());
        assert!(s.sentence().starts_with("No cooling needed"));
    }

    #[test]
    fn a_filter_fan_is_sized_with_the_filter_derating() {
        // Needs 273.9 m3/h installed. 400 m3/h free * 0.7 = 280 >= 273.9.
        let s = select(&basis(500.0, 35.0, 40.0), &planning_products());
        let c = s.proposal().unwrap();
        assert_eq!(c.product.kind, Kind::FilterFan);
        assert_eq!(c.product.airflow_m3h, 400.0);
        assert_eq!(c.count, 1);
        assert!((c.total - 280.0).abs() < 1e-9);
        // Inside: 35 + 500 / (11.638 + 280/3.1) = 39.90 degrees.
        assert!((c.inside_c - (35.0 + 500.0 / (11.638 + 280.0 / 3.1))).abs() < 1e-9);
        assert!((c.inside_c - 39.90).abs() < 0.01);
        assert!(c.inside_c <= 40.0);
        assert!(!c.product.source.is_real());
        assert!(s.sentence().contains("planning size"));
    }

    #[test]
    fn a_target_close_to_the_ambient_rules_a_fan_out() {
        let s = select(&basis(500.0, 35.0, 37.0), &planning_products());
        assert!(s.need.fan_blocked.is_some());
        assert_eq!(s.proposal().unwrap().product.kind, Kind::CoolingUnit);
    }

    #[test]
    fn a_closed_enclosure_gets_a_cooling_unit() {
        let mut b = basis(500.0, 35.0, 40.0);
        b.closed_required = true;
        let s = select(&b, &planning_products());
        let c = s.proposal().unwrap();
        assert_eq!(c.product.kind, Kind::CoolingUnit);
        // Needs 441.81 * 1.1 = 486 W: the 500 W unit is the first that covers it.
        assert_eq!(c.product.cooling_w, 500.0);
        assert!((c.inside_c - 40.0).abs() < 1e-9);
        assert!(s.options.iter().any(|o| o.kind == Kind::FilterFan
            && o.why_not.as_deref().is_some_and(|w| w.contains("closed"))));
    }

    #[test]
    fn a_target_below_the_ambient_adds_the_wall_gain() {
        // 30 degrees inside, 35 outside: the walls bring 5.5*2.116*5 = 58.19 W in.
        let n = need(&basis(500.0, 35.0, 30.0));
        assert!((n.wall_w + 58.19).abs() < 1e-9);
        assert!((n.cooling_w - 558.19).abs() < 1e-9);
        assert!(n.airflow_m3h.is_none());
    }

    #[test]
    fn real_data_beats_a_planning_size_even_when_larger() {
        let mine = Product {
            name: "Maker FF 500".into(),
            kind: Kind::FilterFan,
            airflow_m3h: 500.0,
            cooling_w: 0.0,
            source: Source::Mine,
        };
        let mut all = planning_products();
        all.push(mine);
        let s = select(&basis(500.0, 35.0, 40.0), &all);
        let c = s.proposal().unwrap();
        assert_eq!(c.product.name, "Maker FF 500");
        assert!(c.product.source.is_real());
        assert!(s.sentence().contains("from part data"));
    }

    #[test]
    fn two_small_fans_beat_nothing_when_one_is_too_weak() {
        let only = vec![Product {
            name: "Fan 150".into(),
            kind: Kind::FilterFan,
            airflow_m3h: 150.0,
            cooling_w: 0.0,
            source: Source::Mine,
        }];
        // 150 * 0.7 = 105 each; need 273.9: three give 315.
        let s = select(&basis(500.0, 35.0, 40.0), &only);
        let c = s.proposal().unwrap();
        assert_eq!(c.count, 3);
        assert!((c.total - 315.0).abs() < 1e-9);
    }

    #[test]
    fn nothing_large_enough_says_so() {
        let s = select(&basis(9000.0, 35.0, 40.0), &planning_products());
        // 9000 W is beyond every generic size.
        assert!(s.proposal().is_none() || s.proposal().unwrap().count > 1);
        let s = select(&basis(60_000.0, 35.0, 40.0), &planning_products());
        assert!(s.proposal().is_none());
        assert!(s.sentence().contains("no listed product is large enough"));
    }

    #[test]
    fn a_unit_that_is_too_weak_does_not_hold_the_target() {
        let weak = vec![Product {
            name: "Unit 300".into(),
            kind: Kind::CoolingUnit,
            airflow_m3h: 0.0,
            cooling_w: 300.0,
            source: Source::Mine,
        }];
        let s = select(&basis(500.0, 35.0, 40.0), &weak);
        assert!(s.proposal().is_none());
    }

    #[test]
    fn the_flow_rule_agrees_with_the_physics_of_air() {
        // 3600 / (rho * cp) at 35 degrees outside is within 3 percent of 3.1.
        let rho_cp = crate::setup::Air::at(35.0).rho_cp();
        let physical = 3600.0 / rho_cp;
        assert!(
            (physical - FAN_FACTOR).abs() / FAN_FACTOR < 0.03,
            "{physical}"
        );
    }

    #[test]
    fn csv_round_trips_and_reports_bad_rows() {
        let text = "name;kind;airflow_m3h;cooling_w\nFF 300;filter fan;300;\nUnit;cooling unit;;1,5\nBad;pump;1;\nNoSize;fan;;\n";
        let (list, errors) = products_from_csv(text);
        assert_eq!(list.len(), 2, "{errors:?}");
        // The unit's cooling power 1,5 is read with a decimal comma.
        assert_eq!(list[1].cooling_w, 1.5);
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0].line, 4);
        assert_eq!(errors[1].line, 5);
        let (list, errors) = products_from_csv("FF 300;filter fan;300;\nU;cooling unit;;1500\n");
        assert!(errors.is_empty());
        let back = products_from_csv(&products_to_csv(&list));
        assert_eq!(back.0, list);
    }

    #[test]
    fn parts_with_cooling_data_become_products() {
        let mut project = Project::new("P");
        let mut part = Part {
            id: novo_core::PartId::from(1),
            manufacturer: "Maker".into(),
            order_number: "FF-300".into(),
            type_number: "Filter fan 300".into(),
            description: String::new(),
            properties: Properties::default(),
        };
        part.properties
            .insert(keys::AIRFLOW_M3H.to_owned(), Value::Float(300.0));
        let p = product_of_part(&part).unwrap();
        assert_eq!(p.kind, Kind::FilterFan);
        assert_eq!(p.source, Source::Part("FF-300".into()));
        part.properties
            .insert(keys::COOLING_CAPACITY_W.to_owned(), Value::Float(1000.0));
        part.properties.remove(keys::AIRFLOW_M3H);
        assert_eq!(product_of_part(&part).unwrap().kind, Kind::CoolingUnit);
        let _ = &mut project;
    }
}
