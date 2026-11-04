//! The enclosure shell as real layers (heat lane H1).
//!
//! Until this module the six walls were one number each: the IEC 60890
//! coefficient `k` (air to air, wall sheet and both films included) times a
//! surface factor `b`. Here each face is a stack of layers (sheet, insulation,
//! glazing, air gap) with thickness, conductivity and heat capacity, an inner
//! film and an outer film. From the stack come the air-to-air coefficient the
//! solvers use, the temperature of the inner and the outer surface, and the
//! heat capacity of the wall for the transient run.
//!
//! **What is exact and what is a model.** Series conduction through the
//! layers is exact for a flat 1D wall (tested against hand calculations, a
//! lumped cooling curve and the semi-infinite slab). The two films are not:
//! they are *calibrated* so that the default shell (one steel, stainless,
//! aluminium or plastic sheet) reproduces the IEC 60890 coefficient of the
//! material exactly, and the split between the inner and the outer film is an
//! assumption ([`INNER_FILM_SHARE`]). Heat spreads sideways in a real sheet;
//! the 1D wall does not show that. Everything here is labelled *model* until a
//! measured case says otherwise.
//!
//! **Contract for the other heat lanes** (see
//! `handoff/heat-research/ROADMAP.md`):
//! - H2 (radiation) reads `emissivity_in` / `emissivity_out` per face and
//!   `plate_emissivity`;
//! - H3 (outside) replaces [`Films::h_out`] by a temperature dependent film
//!   and fills [`Outside`] (sky, wind, sun); H1 only stores [`Outside`];
//! - H5 (mass) drives [`WallStack`] and reads [`ShellModel::capacity_j_per_k`];
//! - H7 (UI) reads [`ShellReport`] from `ThermalResult::shell`.

use crate::model::Enclosure;
use novo_layout::sizing::Material;

/// Stefan-Boltzmann constant, W/m2K4.
pub const SIGMA: f64 = 5.670_374e-8;
/// Share of the IEC film resistance that sits on the inside. An assumption:
/// the IEC coefficient does not say how the resistance divides.
pub const INNER_FILM_SHARE: f64 = 0.5;
/// Conductivity of still air, W/mK, and the linearised radiation coefficient
/// across an air gap between two surfaces of emissivity 0.9, W/m2K (about
/// 4.7 at 20 C). Together they give the usual R = 0.15 m2K/W of a 12 mm gap.
const K_AIR: f64 = 0.026;
const H_RAD_GAP: f64 = 4.7;

/// The six faces in the order of the balance fields. `FRONT` is the door.
pub use crate::setup::{BOTTOM, FRONT, LEFT, REAR, RIGHT, TOP};

/// Names of the faces in that order.
pub const FACE_NAMES: [&str; 6] = ["door", "rear", "left", "right", "top", "bottom"];

/// What a layer is made of.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayerMaterial {
    Steel,
    Stainless,
    Aluminium,
    /// Polyester or polycarbonate enclosure shell.
    Plastic,
    Glass,
    MineralWool,
    Foam,
    /// Still air between two surfaces: conduction plus radiation, no
    /// convection (a gap of up to about 20 mm).
    AirGap,
    /// User values: conductivity W/mK and volumetric heat capacity J/m3K.
    Custom {
        k: f64,
        rho_cp: f64,
    },
}

impl LayerMaterial {
    /// Conductivity, W/mK (planning values from the usual handbooks).
    pub fn k(self) -> f64 {
        match self {
            LayerMaterial::Steel => 50.0,
            LayerMaterial::Stainless => 15.0,
            LayerMaterial::Aluminium => 160.0,
            LayerMaterial::Plastic => 0.25,
            LayerMaterial::Glass => 1.0,
            LayerMaterial::MineralWool => 0.04,
            LayerMaterial::Foam => 0.025,
            LayerMaterial::AirGap => K_AIR,
            LayerMaterial::Custom { k, .. } => k,
        }
    }

    /// Density times specific heat, J/m3K.
    pub fn rho_cp(self) -> f64 {
        match self {
            LayerMaterial::Steel => 7850.0 * 490.0,
            LayerMaterial::Stainless => 7900.0 * 500.0,
            LayerMaterial::Aluminium => 2700.0 * 900.0,
            LayerMaterial::Plastic => 1500.0 * 1200.0,
            LayerMaterial::Glass => 2500.0 * 840.0,
            LayerMaterial::MineralWool => 60.0 * 840.0,
            LayerMaterial::Foam => 35.0 * 1400.0,
            LayerMaterial::AirGap => 1.2 * 1005.0,
            LayerMaterial::Custom { rho_cp, .. } => rho_cp,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            LayerMaterial::Steel => "steel",
            LayerMaterial::Stainless => "stainless",
            LayerMaterial::Aluminium => "aluminium",
            LayerMaterial::Plastic => "plastic",
            LayerMaterial::Glass => "glass",
            LayerMaterial::MineralWool => "mineral wool",
            LayerMaterial::Foam => "foam",
            LayerMaterial::AirGap => "air gap",
            LayerMaterial::Custom { .. } => "custom",
        }
    }
}

impl From<Material> for LayerMaterial {
    fn from(m: Material) -> Self {
        match m {
            Material::Steel => LayerMaterial::Steel,
            Material::Stainless => LayerMaterial::Stainless,
            Material::Aluminium => LayerMaterial::Aluminium,
            Material::Plastic => LayerMaterial::Plastic,
        }
    }
}

/// One layer of a wall.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layer {
    pub material: LayerMaterial,
    pub thickness_mm: f64,
}

impl Layer {
    pub fn new(material: LayerMaterial, thickness_mm: f64) -> Layer {
        Layer {
            material,
            thickness_mm,
        }
    }

    /// Thermal resistance, m2K/W.
    pub fn r(&self) -> f64 {
        let t = (self.thickness_mm / 1000.0).max(0.0);
        match self.material {
            LayerMaterial::AirGap if t > 0.0 => 1.0 / (K_AIR / t + H_RAD_GAP),
            LayerMaterial::AirGap => 0.0,
            m => t / m.k().max(1e-9),
        }
    }

    /// Heat capacity per area, J/m2K.
    pub fn capacity(&self) -> f64 {
        self.material.rho_cp() * self.thickness_mm / 1000.0
    }
}

/// Surface finishes with planning values for the long-wave emissivity and the
/// absorptance of sunlight. Typical handbook figures, not measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finish {
    /// Light powder coat (RAL 7035 and similar).
    PowderLight,
    PowderDark,
    /// Galvanised sheet, bare.
    Galvanised,
    StainlessBrushed,
    AluminiumNatural,
    /// Grey plastic.
    Plastic,
}

impl Finish {
    pub fn emissivity(self) -> f64 {
        match self {
            Finish::PowderLight | Finish::PowderDark | Finish::Plastic => 0.90,
            Finish::Galvanised => 0.25,
            Finish::StainlessBrushed => 0.30,
            Finish::AluminiumNatural => 0.10,
        }
    }

    pub fn solar_absorptance(self) -> f64 {
        match self {
            Finish::PowderLight => 0.40,
            Finish::PowderDark => 0.85,
            Finish::Galvanised => 0.65,
            Finish::StainlessBrushed => 0.50,
            Finish::AluminiumNatural => 0.30,
            Finish::Plastic => 0.60,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Finish::PowderLight => "light powder coat",
            Finish::PowderDark => "dark powder coat",
            Finish::Galvanised => "galvanised",
            Finish::StainlessBrushed => "brushed stainless",
            Finish::AluminiumNatural => "natural aluminium",
            Finish::Plastic => "plastic",
        }
    }

    /// The usual finish of an enclosure material.
    pub fn of_material(m: Material) -> Finish {
        match m {
            Material::Steel => Finish::PowderLight,
            Material::Stainless => Finish::StainlessBrushed,
            Material::Aluminium => Finish::AluminiumNatural,
            Material::Plastic => Finish::Plastic,
        }
    }
}

/// One face: layers from the inside out and the two surfaces.
#[derive(Debug, Clone, PartialEq)]
pub struct FaceSpec {
    pub layers: Vec<Layer>,
    pub emissivity_in: f64,
    pub emissivity_out: f64,
    /// Share of the sunlight that is absorbed (used by lane H3).
    pub absorptance_solar: f64,
}

impl FaceSpec {
    pub fn single(material: Material, thickness_mm: f64) -> FaceSpec {
        let f = Finish::of_material(material);
        FaceSpec {
            layers: vec![Layer::new(material.into(), thickness_mm)],
            emissivity_in: f.emissivity(),
            emissivity_out: f.emissivity(),
            absorptance_solar: f.solar_absorptance(),
        }
    }

    pub fn set_finish_out(&mut self, f: Finish) {
        self.emissivity_out = f.emissivity();
        self.absorptance_solar = f.solar_absorptance();
    }

    /// Sum of the layer resistances, m2K/W.
    pub fn r_layers(&self) -> f64 {
        self.layers.iter().map(Layer::r).sum()
    }

    pub fn capacity(&self) -> f64 {
        self.layers.iter().map(Layer::capacity).sum()
    }

    pub fn thickness_mm(&self) -> f64 {
        self.layers.iter().map(|l| l.thickness_mm).sum()
    }
}

/// A window in the door: a share of the door area with its own layers.
#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    /// Share of the door area, 0 to 1.
    pub area_fraction: f64,
    pub layers: Vec<Layer>,
}

impl Window {
    pub fn single_glazed(area_fraction: f64) -> Window {
        Window {
            area_fraction,
            layers: vec![Layer::new(LayerMaterial::Glass, 4.0)],
        }
    }

    pub fn double_glazed(area_fraction: f64) -> Window {
        Window {
            area_fraction,
            layers: vec![
                Layer::new(LayerMaterial::Glass, 4.0),
                Layer::new(LayerMaterial::AirGap, 12.0),
                Layer::new(LayerMaterial::Glass, 4.0),
            ],
        }
    }
}

/// The two surface films, W/m2K. `h_in` is convection plus radiation of the
/// inner surface to the air and the contents, `h_out` of the outer surface to
/// the surroundings. Lane H3 replaces `h_out` by a temperature dependent value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Films {
    pub h_in: f64,
    pub h_out: f64,
}

/// How the outer film is found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutsideFilm {
    /// The IEC 60890 calibration: one constant film and the factors `b` of
    /// the installation (default; results equal the one-number wall).
    #[default]
    Iec,
    /// Lane H3: film from the actual outer surface temperature (natural and
    /// wind convection, radiation to room or sky), mounting gaps, sun. A
    /// model; see [`crate::outside`].
    Physical,
}

/// The surroundings (lane H3). Used when `film` is [`OutsideFilm::Physical`];
/// the other fields are stored only otherwise.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Outside {
    /// Effective sky temperature for the radiation of the roof and walls,
    /// degrees C; `None` = the ambient air.
    pub sky_c: Option<f64>,
    /// Wind or room air speed along the faces, m/s.
    pub wind_m_s: f64,
    /// Solar irradiance on a face that sees the sun fully, W/m2.
    pub sun_w_m2: f64,
    pub film: OutsideFilm,
    /// Air gap to a wall or neighbour in front of each face, mm (order of
    /// [`FACE_NAMES`]); `Some(0.0)` = touching. `None` = open, unless the
    /// installation says the face is covered (then [`ASSUMED_COVER_GAP_MM`]).
    pub gaps_mm: [Option<f64>; 6],
    /// Share of `sun_w_m2` that falls on each face, 0..1 (roof about 1, a
    /// wall facing the sun about 0.5).
    pub sun_share: [f64; 6],
    /// Set by [`crate::outside::resolve`]: the outer film per face, W/m2K.
    pub h_out_face: Option<[f64; 6]>,
    /// Set by [`crate::outside::resolve`]: the rise of the air temperature a
    /// face sees because of the sun (sol-air), K.
    pub sun_offset_k: [f64; 6],
}

impl Default for Outside {
    fn default() -> Self {
        Outside {
            sky_c: None,
            wind_m_s: 0.0,
            sun_w_m2: 0.0,
            film: OutsideFilm::Iec,
            gaps_mm: [None; 6],
            sun_share: [0.0; 6],
            h_out_face: None,
            sun_offset_k: [0.0; 6],
        }
    }
}

/// Gap used for a face the installation marks as covered when no gap is
/// given (assumed, shown as such): a cabinet screwed to a wall stands off by
/// its feet or rails.
pub const ASSUMED_COVER_GAP_MM: f64 = 20.0;

/// Where a group of shell values comes from, for the honest label in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Provenance {
    /// Today's defaults, not from the chosen enclosure.
    #[default]
    Assumed,
    /// From the maker's catalogue or CAD data of the chosen enclosure.
    Catalogue,
    /// Measured on the real enclosure.
    Measured,
}

/// Provenance of each group of values in [`ShellModel`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShellProvenance {
    /// Sheet material, thickness and layers (walls and door).
    pub walls: Provenance,
    /// Paint / finish (emissivity, absorptance).
    pub finish: Provenance,
    /// Door window and gasket.
    pub door: Provenance,
    /// Plate setback and plate emissivity.
    pub plate: Provenance,
    /// Enclosure mass.
    pub mass: Provenance,
}

/// The whole shell. This is the single record the enclosure feed fills: when
/// an enclosure is chosen at cabinet start, the catalogue record is mapped
/// into `Params::shell` and `provenance` says which groups are real. No heat
/// lane keeps private enclosure defaults beside it.
#[derive(Debug, Clone, PartialEq)]
pub struct ShellModel {
    /// Order of [`FRONT`] (the door panel), [`REAR`], [`LEFT`], [`RIGHT`],
    /// [`TOP`], [`BOTTOM`].
    pub faces: [FaceSpec; 6],
    pub window: Option<Window>,
    /// Air gap between the mounting plate and the rear wall, mm. `None` = the
    /// plate lies on the wall (the plate path is the rear coefficient, as in
    /// the IEC calibration).
    pub plate_setback_mm: Option<f64>,
    /// Emissivity of the mounting plate (galvanised about 0.25 to 0.4).
    pub plate_emissivity: f64,
    pub films: Films,
    pub outside: Outside,
    pub provenance: ShellProvenance,
    /// Enclosure weight from the catalogue, kg, to cross-check the wall heat
    /// capacity ([`ShellModel::mass_check`]).
    pub mass_kg: Option<f64>,
}

/// What the shell can show, so a UI hides rows the engine cannot fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellCaps {
    pub layers: bool,
    pub emissivity: bool,
    pub door_window: bool,
    pub setback: bool,
    /// Lane H3: sun, wind, sky, mounting gaps.
    pub outside: bool,
}

/// Geometry of the six faces, m2.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceAreas {
    pub outer: [f64; 6],
    pub inner: [f64; 6],
}

impl FaceAreas {
    pub fn of(e: &Enclosure) -> FaceAreas {
        let (wo, ho, d) = (
            e.width_mm / 1000.0,
            e.height_mm / 1000.0,
            e.depth_mm / 1000.0,
        );
        let (wi, hi, di) = (
            e.interior_width_mm() / 1000.0,
            e.interior_height_mm() / 1000.0,
            e.interior_depth_mm() / 1000.0,
        );
        FaceAreas {
            outer: [wo * ho, wo * ho, d * ho, d * ho, wo * d, wo * d],
            inner: [wi * hi, wi * hi, di * hi, di * hi, wi * di, wi * di],
        }
    }
}

impl ShellModel {
    /// The default shell of an enclosure: one sheet of its material and
    /// `wall_mm`, films calibrated so the air-to-air coefficient equals the
    /// IEC 60890 value of the material. With this shell every result equals
    /// the one-number model.
    pub fn from_enclosure(e: &Enclosure) -> ShellModel {
        let face = FaceSpec::single(e.material, e.wall_mm);
        let r_iec = 1.0 / e.material.k();
        let r_films = (r_iec - face.r_layers()).max(0.2 * r_iec);
        let films = Films {
            h_in: 1.0 / (INNER_FILM_SHARE * r_films),
            h_out: 1.0 / ((1.0 - INNER_FILM_SHARE) * r_films),
        };
        ShellModel {
            faces: std::array::from_fn(|_| face.clone()),
            window: None,
            plate_setback_mm: None,
            plate_emissivity: 0.3,
            films,
            outside: Outside::default(),
            provenance: ShellProvenance::default(),
            mass_kg: None,
        }
    }

    pub fn caps(&self) -> ShellCaps {
        ShellCaps {
            layers: true,
            emissivity: true,
            door_window: true,
            setback: true,
            outside: true,
        }
    }

    /// Add an insulation layer on the inside of the wall of every face except
    /// the door unless `door` is set.
    pub fn add_insulation(&mut self, material: LayerMaterial, thickness_mm: f64, door: bool) {
        for (i, f) in self.faces.iter_mut().enumerate() {
            if i == FRONT && !door {
                continue;
            }
            f.layers.insert(0, Layer::new(material, thickness_mm));
        }
    }

    fn window_fraction(&self) -> f64 {
        self.window
            .as_ref()
            .map_or(0.0, |w| w.area_fraction.clamp(0.0, 1.0))
    }

    /// The films of a face: the shell's, with the outer film of lane H3 when
    /// it has been resolved.
    pub fn films_of(&self, face: usize) -> Films {
        Films {
            h_in: self.films.h_in,
            h_out: self
                .outside
                .h_out_face
                .map_or(self.films.h_out, |h| h[face].max(1.0e-6)),
        }
    }

    fn u_of(&self, face: usize, r_layers: f64) -> f64 {
        let f = self.films_of(face);
        1.0 / (1.0 / f.h_in + r_layers + 1.0 / f.h_out)
    }

    /// Surface factors of the faces: the IEC `b` of the installation, or with
    /// the physical outer film 1 (the film and the gaps carry the mounting)
    /// and 0 for a cabinet standing on the floor.
    pub fn b_factors(&self, e: &Enclosure) -> [f64; 6] {
        match self.outside.film {
            OutsideFilm::Iec => crate::setup::surface_factors(e.installation),
            OutsideFilm::Physical => {
                let mut b = [1.0; 6];
                if e.installation.on_floor {
                    b[BOTTOM] = 0.0;
                }
                b
            }
        }
    }

    /// Gap in front of a face, mm, with whether it is assumed: from
    /// `gaps_mm`, else the covered flag of the installation, else `None`
    /// (open).
    pub fn gap_of(&self, e: &Enclosure, face: usize) -> Option<(f64, bool)> {
        if let Some(g) = self.outside.gaps_mm[face] {
            return Some((g.max(0.0), false));
        }
        let i = e.installation;
        let covered = match face {
            REAR => i.rear_covered,
            LEFT => i.left_covered,
            RIGHT => i.right_covered,
            TOP => i.top_covered,
            _ => false,
        };
        covered.then_some((ASSUMED_COVER_GAP_MM, true))
    }

    /// Air-to-air coefficient of the panel (no window) of a face, W/m2K.
    pub fn u_panel(&self, face: usize) -> f64 {
        self.u_of(face, self.faces[face].r_layers())
    }

    /// Air-to-air coefficient of the window, if any.
    pub fn u_window(&self) -> Option<f64> {
        self.window
            .as_ref()
            .map(|w| self.u_of(FRONT, w.layers.iter().map(Layer::r).sum()))
    }

    /// Area weighted air-to-air coefficient of a face (the door includes its
    /// window), W/m2K.
    pub fn u_face(&self, face: usize) -> f64 {
        let f = if face == FRONT {
            self.window_fraction()
        } else {
            0.0
        };
        let up = self.u_panel(face);
        match (f > 0.0, self.u_window()) {
            (true, Some(uw)) => (1.0 - f) * up + f * uw,
            _ => up,
        }
    }

    /// Conductance of each face to the outside air, W/K, with the IEC 60890
    /// surface factors of the installation and the user's scale on top. For
    /// the default shell the sum is `k * A_effective` exactly.
    pub fn conductances_w_per_k(&self, e: &Enclosure, scale: f64) -> [f64; 6] {
        let a = FaceAreas::of(e);
        let b = self.b_factors(e);
        std::array::from_fn(|f| self.u_face(f) * scale * b[f] * a.outer[f])
    }

    /// Heat capacity of the wall of each face, J/K.
    pub fn capacity_per_face_j_per_k(&self, e: &Enclosure) -> [f64; 6] {
        let a = FaceAreas::of(e);
        std::array::from_fn(|f| {
            let wf = if f == FRONT {
                self.window_fraction()
            } else {
                0.0
            };
            let panel = self.faces[f].capacity();
            let win = self
                .window
                .as_ref()
                .map_or(0.0, |w| w.layers.iter().map(Layer::capacity).sum::<f64>());
            ((1.0 - wf) * panel + wf * win) * a.outer[f]
        })
    }

    /// Ratio of the catalogue mass to the mass of the modelled layers
    /// (steel-density equivalent of the capacity); about 1 when the layers
    /// match the real enclosure, well above 1 when the catalogue lists
    /// ribs, doors or fittings the layers do not carry. `None` without a mass.
    pub fn mass_check(&self, e: &Enclosure) -> Option<f64> {
        let m = self.mass_kg?;
        let cap = self.capacity_j_per_k(e);
        let modelled_kg = cap / (7850.0 * 490.0) * 7850.0;
        (modelled_kg > 0.0).then(|| m / modelled_kg)
    }

    /// Heat capacity of the whole shell, J/K.
    pub fn capacity_j_per_k(&self, e: &Enclosure) -> f64 {
        self.capacity_per_face_j_per_k(e).iter().sum()
    }

    /// Plate to outside conductance per m2 of plate, W/m2K: through the air
    /// gap and the rear wall when there is a setback, else the IEC series
    /// split (`kb` the rear coefficient per m2 of interior, `h_air` the film
    /// from air to plate).
    pub fn plate_to_outside_or(&self, kb: f64, h_air: f64) -> f64 {
        self.plate_to_outside(kb)
            .unwrap_or_else(|| 1.0 / (1.0 / kb - 1.0 / h_air))
    }

    /// The setback path of [`ShellModel::plate_to_outside_or`]; `None` when
    /// the plate lies on the wall.
    pub fn plate_to_outside(&self, kb: f64) -> Option<f64> {
        let s = self.plate_setback_mm.filter(|s| *s >= 1.0)? / 1000.0;
        let rear = &self.faces[REAR];
        let eps = 1.0
            / (1.0 / self.plate_emissivity.max(0.02) + 1.0 / rear.emissivity_in.max(0.02) - 1.0);
        let h_gap = K_AIR / s + 4.0 * SIGMA * 293.15_f64.powi(3) * eps;
        // Resistance from the wall's inner surface to the outside air.
        let r_total = 1.0 / kb.max(1e-9);
        let share_in = (1.0 / self.films.h_in) / (1.0 / self.u_panel(REAR));
        let r_wall_out = r_total * (1.0 - share_in.clamp(0.0, 0.95));
        Some(1.0 / (1.0 / h_gap + r_wall_out))
    }
}

/// Temperatures through one wall path at a given flux.
#[derive(Debug, Clone, PartialEq)]
pub struct PathTemps {
    /// Heat flux through the path, W/m2 (positive outwards).
    pub flux_w_m2: f64,
    pub inner_surface_c: f64,
    pub outer_surface_c: f64,
    /// Temperature at each layer interface from the inner surface to the
    /// outer surface (`layers + 1` values).
    pub interfaces_c: Vec<f64>,
}

fn path_temps(layers: &[Layer], films: Films, flux: f64, t_in_air: f64) -> PathTemps {
    let inner = t_in_air - flux / films.h_in;
    let mut ifc = vec![inner];
    let mut t = inner;
    for l in layers {
        t -= flux * l.r();
        ifc.push(t);
    }
    PathTemps {
        flux_w_m2: flux,
        inner_surface_c: inner,
        outer_surface_c: t,
        interfaces_c: ifc,
    }
}

/// Temperatures and heat of one face.
#[derive(Debug, Clone, PartialEq)]
pub struct FaceTemps {
    pub face: usize,
    /// Share of the face that gives off heat (the IEC factor `b`); 0 = no
    /// heat leaves (floor standing bottom). Covered faces carry the whole flux
    /// through this share.
    pub exposed_share: f64,
    /// Heat through the face, W.
    pub heat_w: f64,
    /// Air-to-air coefficient used (includes the scale), W/m2K.
    pub u_w_m2k: f64,
    /// Mean temperature of the inner side (air or plate) next to the face.
    pub inner_air_c: f64,
    pub inner_c: f64,
    pub outer_c: f64,
    /// Range over the face (cells of the solver grid).
    pub inner_min_c: f64,
    pub inner_max_c: f64,
    pub outer_min_c: f64,
    pub outer_max_c: f64,
    /// Layer interface temperatures of the panel at the mean.
    pub interfaces_c: Vec<f64>,
    /// Window of the door: inner and outer surface, when the face has one.
    pub window: Option<(f64, f64)>,
    /// Part of `heat_w` that leaves the outer surface by long-wave radiation
    /// and by convection, W (model: from the linearised radiation of the
    /// outer film).
    pub outer_radiation_w: f64,
    pub outer_convection_w: f64,
}

/// Whether a value is a model or an exact result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelStatus {
    Model,
    Iec,
    Exact,
}

/// What the shell reports back with a result. Stable face order, see
/// [`FACE_NAMES`]; `faces[FRONT]` is the door (its `window` is filled when
/// the door has one).
#[derive(Debug, Clone, PartialEq)]
pub struct ShellReport {
    pub faces: [FaceTemps; 6],
    /// Mean temperature of the mounting plate, which is the temperature of
    /// its rear side to within the sheet's gradient (a few hundredths of a K),
    /// degrees C.
    pub plate_rear_c: Option<f64>,
    pub plate_setback_mm: Option<f64>,
    /// Heat capacity of the shell, per face and total, J/K.
    pub capacity_per_face_j_per_k: [f64; 6],
    pub capacity_j_per_k: f64,
    pub caps: ShellCaps,
    pub provenance: ShellProvenance,
    pub mass_check: Option<f64>,
    /// Always `Model` until lane H6 has a measured or exact case for the
    /// terms; the IEC total is in `ThermalResult::balance`.
    pub status: ModelStatus,
}

impl ShellReport {
    pub fn door(&self) -> &FaceTemps {
        &self.faces[FRONT]
    }

    /// Heat through all faces, W.
    pub fn wall_heat_w(&self) -> f64 {
        self.faces.iter().map(|f| f.heat_w).sum()
    }
}

/// Samples next to a face: `(conductance W/K, temperature of the inner side)`
/// per solver cell.
pub type FaceSamples = Vec<(f64, f64)>;

/// Builds the report from the solver's per-cell wall data.
pub fn report(
    model: &ShellModel,
    e: &Enclosure,
    ambient_c: f64,
    scale: f64,
    samples: &[FaceSamples; 6],
    plate_rear_c: Option<f64>,
) -> ShellReport {
    let b = model.b_factors(e);
    let caps_j = model.capacity_per_face_j_per_k(e);
    let wf = model.window_fraction();
    let faces: [FaceTemps; 6] = std::array::from_fn(|f| {
        let sm = &samples[f];
        let g_sum: f64 = sm.iter().map(|s| s.0).sum();
        let u = model.u_face(f) * scale;
        let up = model.u_panel(f) * scale;
        let (t_mean, t_min, t_max) = if g_sum > 0.0 {
            (
                sm.iter().map(|s| s.0 * s.1).sum::<f64>() / g_sum,
                sm.iter().map(|s| s.1).fold(f64::INFINITY, f64::min),
                sm.iter().map(|s| s.1).fold(f64::NEG_INFINITY, f64::max),
            )
        } else {
            (ambient_c, ambient_c, ambient_c)
        };
        // The air the face sees: ambient plus the sun's rise (sol-air).
        let ambient_c = ambient_c + model.outside.sun_offset_k[f];
        let heat_w = sm.iter().map(|s| s.0 * (s.1 - ambient_c)).sum::<f64>();
        let temps = |t_in: f64, u_panel: f64| {
            path_temps(
                &model.faces[f].layers,
                model.films_of(f),
                u_panel * (t_in - ambient_c),
                t_in,
            )
        };
        let dead = b[f] <= 0.0;
        let mean = temps(t_mean, up);
        let lo = temps(t_min, up);
        let hi = temps(t_max, up);
        // A face that gives off no heat sits at its inside temperature.
        let outer = |p: &PathTemps, t_in: f64| if dead { t_in } else { p.outer_surface_c };
        let window = if f == FRONT {
            model.window.as_ref().filter(|_| wf > 0.0).map(|w| {
                let uw = model.u_window().unwrap_or(up) * scale;
                let p = path_temps(
                    &w.layers,
                    model.films_of(FRONT),
                    uw * (t_mean - ambient_c),
                    t_mean,
                );
                (
                    p.inner_surface_c,
                    if dead { t_mean } else { p.outer_surface_c },
                )
            })
        } else {
            None
        };
        // Radiation share of the outer film at the mean outer surface.
        let t_o = outer(&mean, t_mean);
        let tm = 0.5 * (t_o + ambient_c) + 273.15;
        let h_rad = (4.0 * SIGMA * model.faces[f].emissivity_out * tm.powi(3))
            .min(0.9 * model.films_of(f).h_out);
        let rad_share = h_rad / model.films_of(f).h_out;
        FaceTemps {
            face: f,
            exposed_share: b[f],
            heat_w,
            u_w_m2k: u,
            inner_air_c: t_mean,
            inner_c: mean.inner_surface_c,
            outer_c: t_o,
            inner_min_c: lo.inner_surface_c,
            inner_max_c: hi.inner_surface_c,
            outer_min_c: outer(&lo, t_min),
            outer_max_c: outer(&hi, t_max),
            interfaces_c: mean.interfaces_c,
            window,
            outer_radiation_w: heat_w * rad_share,
            outer_convection_w: heat_w * (1.0 - rad_share),
        }
    });
    ShellReport {
        faces,
        plate_rear_c,
        plate_setback_mm: model.plate_setback_mm,
        capacity_per_face_j_per_k: caps_j,
        capacity_j_per_k: caps_j.iter().sum(),
        caps: model.caps(),
        provenance: model.provenance,
        mass_check: model.mass_check(e),
        status: ModelStatus::Model,
    }
}

/// Transient 1D conduction through one wall path (implicit finite volumes,
/// stable for any step). Used to check the wall's thermal mass against
/// analytic cases and by lane H5 for day/night runs.
#[derive(Debug, Clone)]
pub struct WallStack {
    /// Cell widths, m, conductivities, W/mK, and volumetric heat capacities.
    dx: Vec<f64>,
    k: Vec<f64>,
    c: Vec<f64>,
    /// Temperatures of the cells, degrees C.
    pub t: Vec<f64>,
    h_in: f64,
    h_out: f64,
}

impl WallStack {
    /// `cells_per_mm` sets the resolution (0.5 = one cell per 2 mm, at least
    /// two cells per layer).
    pub fn new(layers: &[Layer], films: Films, t0: f64, cells_per_mm: f64) -> WallStack {
        let (mut dx, mut k, mut c) = (Vec::new(), Vec::new(), Vec::new());
        for l in layers {
            let t = l.thickness_mm / 1000.0;
            if t <= 0.0 {
                continue;
            }
            let n = ((l.thickness_mm * cells_per_mm).ceil() as usize).clamp(2, 400);
            // The effective conductivity of an air gap includes its radiation.
            let kk = t / l.r().max(1e-12);
            for _ in 0..n {
                dx.push(t / n as f64);
                k.push(kk);
                c.push(l.material.rho_cp());
            }
        }
        let n = dx.len();
        WallStack {
            dx,
            k,
            c,
            t: vec![t0; n],
            h_in: films.h_in,
            h_out: films.h_out,
        }
    }

    pub fn cells(&self) -> usize {
        self.t.len()
    }

    /// Advance by `dt` seconds with the given air temperatures.
    pub fn step(&mut self, dt: f64, t_in_air: f64, t_out_air: f64) {
        let n = self.t.len();
        if n == 0 {
            return;
        }
        let g_in = 1.0 / (1.0 / self.h_in + 0.5 * self.dx[0] / self.k[0]);
        let g_out = 1.0 / (1.0 / self.h_out + 0.5 * self.dx[n - 1] / self.k[n - 1]);
        let mut a = vec![0.0; n]; // sub
        let mut b = vec![0.0; n]; // diag
        let mut cc = vec![0.0; n]; // super
        let mut d = vec![0.0; n];
        for i in 0..n {
            let cap = self.c[i] * self.dx[i] / dt;
            b[i] = cap;
            d[i] = cap * self.t[i];
            if i == 0 {
                b[i] += g_in;
                d[i] += g_in * t_in_air;
            } else {
                let g = 1.0 / (0.5 * self.dx[i - 1] / self.k[i - 1] + 0.5 * self.dx[i] / self.k[i]);
                b[i] += g;
                a[i] = -g;
            }
            if i == n - 1 {
                b[i] += g_out;
                d[i] += g_out * t_out_air;
            } else {
                let g = 1.0 / (0.5 * self.dx[i] / self.k[i] + 0.5 * self.dx[i + 1] / self.k[i + 1]);
                b[i] += g;
                cc[i] = -g;
            }
        }
        // Thomas algorithm.
        for i in 1..n {
            let m = a[i] / b[i - 1];
            b[i] -= m * cc[i - 1];
            d[i] -= m * d[i - 1];
        }
        self.t[n - 1] = d[n - 1] / b[n - 1];
        for i in (0..n - 1).rev() {
            self.t[i] = (d[i] - cc[i] * self.t[i + 1]) / b[i];
        }
    }

    /// Inner surface temperature for the given inside air, degrees C.
    pub fn inner_surface_c(&self, t_in_air: f64) -> f64 {
        let g = 1.0 / (1.0 / self.h_in + 0.5 * self.dx[0] / self.k[0]);
        let q = g * (t_in_air - self.t[0]);
        t_in_air - q / self.h_in
    }

    /// Outer surface temperature for the given outside air, degrees C.
    pub fn outer_surface_c(&self, t_out_air: f64) -> f64 {
        let n = self.t.len();
        let g = 1.0 / (1.0 / self.h_out + 0.5 * self.dx[n - 1] / self.k[n - 1]);
        let q = g * (self.t[n - 1] - t_out_air);
        t_out_air + q / self.h_out
    }

    /// Heat stored relative to `t_ref`, J/m2.
    pub fn stored_j_m2(&self, t_ref: f64) -> f64 {
        (0..self.t.len())
            .map(|i| self.c[i] * self.dx[i] * (self.t[i] - t_ref))
            .sum()
    }

    /// Heat flow from the inner side into the wall, W/m2.
    pub fn flux_in_w_m2(&self, t_in_air: f64) -> f64 {
        let g = 1.0 / (1.0 / self.h_in + 0.5 * self.dx[0] / self.k[0]);
        g * (t_in_air - self.t[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_layout::Rect;
    use novo_layout::sizing::Installation;

    fn enclosure(material: Material, inst: Installation) -> Enclosure {
        Enclosure {
            width_mm: 800.0,
            height_mm: 1000.0,
            depth_mm: 300.0,
            wall_mm: 2.0,
            material,
            installation: inst,
            plate: Rect {
                x: 20.0,
                y: 20.0,
                w: 700.0,
                h: 900.0,
            },
        }
    }

    fn iec_conductance(e: &Enclosure) -> f64 {
        let a = FaceAreas::of(e);
        let b = crate::setup::surface_factors(e.installation);
        (0..6).map(|f| e.material.k() * b[f] * a.outer[f]).sum()
    }

    #[test]
    fn default_shell_reproduces_iec_for_every_material_and_mounting() {
        let mounts = [
            Installation::FreeStanding,
            Installation::Wall,
            Installation::default(),
        ];
        for m in [
            Material::Steel,
            Material::Stainless,
            Material::Aluminium,
            Material::Plastic,
        ] {
            for inst in mounts {
                let e = enclosure(m, inst);
                let s = ShellModel::from_enclosure(&e);
                let g: f64 = s.conductances_w_per_k(&e, 1.0).iter().sum();
                let want = iec_conductance(&e);
                assert!(
                    (g / want - 1.0).abs() < 1e-9,
                    "{m:?} {inst:?}: shell {g} against IEC {want}"
                );
                for f in 0..6 {
                    assert!((s.u_face(f) / m.k() - 1.0).abs() < 1e-9);
                }
            }
        }
    }

    #[test]
    fn series_resistance_matches_hand_calculation() {
        // Steel sheet 2 mm with 30 mm foam: U = 1 / (1/5.5 - R_sheet + R_foam + R_sheet).
        let e = enclosure(Material::Steel, Installation::FreeStanding);
        let mut s = ShellModel::from_enclosure(&e);
        s.add_insulation(LayerMaterial::Foam, 30.0, true);
        let r = 1.0 / 5.5 + 0.030 / 0.025;
        assert!((s.u_panel(REAR) - 1.0 / r).abs() < 1e-9);
        // 5.5 -> 0.7: insulation cuts the loss by a factor of eight.
        assert!(s.u_panel(REAR) < 0.8);
    }

    #[test]
    fn air_gap_resistance_is_the_handbook_value() {
        let r = Layer::new(LayerMaterial::AirGap, 12.0).r();
        assert!((r - 0.147).abs() < 0.01, "R of a 12 mm gap {r}");
    }

    #[test]
    fn double_glazing_beats_single() {
        let e = enclosure(Material::Steel, Installation::FreeStanding);
        let mut s = ShellModel::from_enclosure(&e);
        s.window = Some(Window::single_glazed(0.3));
        let single = s.u_face(FRONT);
        s.window = Some(Window::double_glazed(0.3));
        let double = s.u_face(FRONT);
        assert!(double < single);
        // A single glass pane (4 mm) hardly adds to the films: window U is
        // about the panel's.
        assert!((single / s.u_panel(FRONT) - 1.0).abs() < 0.05);
        // Door U is the area weighted mean of panel and window.
        let want = 0.7 * s.u_panel(FRONT) + 0.3 * s.u_window().unwrap();
        assert!((double - want).abs() < 1e-12);
    }

    #[test]
    fn surface_temperatures_follow_the_flux() {
        let e = enclosure(Material::Steel, Installation::FreeStanding);
        let s = ShellModel::from_enclosure(&e);
        let samples: [FaceSamples; 6] = std::array::from_fn(|_| vec![(1.0, 50.0)]);
        let r = report(&s, &e, 30.0, 1.0, &samples, None);
        let side = &r.faces[LEFT];
        // flux = U * dT; inner drop = flux / h_in; outer rise = flux / h_out.
        let q = 5.5 * 20.0;
        assert!((side.inner_c - (50.0 - q / s.films.h_in)).abs() < 1e-9);
        assert!((side.outer_c - (30.0 + q / s.films.h_out)).abs() < 0.01);
        // Equal films: the surfaces sit half way between the two airs.
        assert!((side.inner_c - 40.0).abs() < 0.01 && (side.outer_c - 40.0).abs() < 0.01);
        // Floor standing: no heat through the bottom, outer at the inside value.
        let bottom = &r.faces[BOTTOM];
        assert_eq!(bottom.exposed_share, 0.0);
        assert!((bottom.outer_c - 50.0).abs() < 1e-9);
    }

    #[test]
    fn insulation_moves_the_drop_into_the_insulation() {
        let e = enclosure(Material::Steel, Installation::FreeStanding);
        let mut s = ShellModel::from_enclosure(&e);
        s.add_insulation(LayerMaterial::MineralWool, 40.0, true);
        let samples: [FaceSamples; 6] = std::array::from_fn(|_| vec![(1.0, 60.0)]);
        let r = report(&s, &e, 20.0, 1.0, &samples, None);
        let f = &r.faces[LEFT];
        // inner surface close to the air, outer close to ambient, most of the
        // 40 K inside the wool.
        assert!(f.inner_c > 55.0 && f.outer_c < 25.0, "{f:?}");
        let drop_wool = f.interfaces_c[0] - f.interfaces_c[1];
        assert!(drop_wool > 30.0);
    }

    #[test]
    fn temperatures_never_leave_the_two_air_temperatures() {
        let e = enclosure(Material::Plastic, Installation::Wall);
        let s = ShellModel::from_enclosure(&e);
        let samples: [FaceSamples; 6] =
            std::array::from_fn(|_| vec![(1.0, 35.0), (2.0, 55.0), (1.0, 45.0)]);
        let r = report(&s, &e, 25.0, 1.0, &samples, None);
        for f in &r.faces {
            assert!(f.outer_min_c <= f.outer_c && f.outer_c <= f.outer_max_c);
            assert!(f.inner_max_c <= 55.0 && f.outer_min_c >= 25.0);
        }
    }

    #[test]
    fn radiation_share_follows_the_emissivity() {
        let e = enclosure(Material::Steel, Installation::FreeStanding);
        let mut s = ShellModel::from_enclosure(&e);
        let samples: [FaceSamples; 6] = std::array::from_fn(|_| vec![(2.0, 55.0)]);
        let painted = report(&s, &e, 25.0, 1.0, &samples, None);
        s.faces[LEFT].emissivity_out = 0.1;
        let bright = report(&s, &e, 25.0, 1.0, &samples, None);
        assert!(painted.faces[LEFT].outer_radiation_w > 2.0 * bright.faces[LEFT].outer_radiation_w);
        let f = &painted.faces[LEFT];
        assert!((f.outer_radiation_w + f.outer_convection_w - f.heat_w).abs() < 1e-9);
    }

    #[test]
    fn provenance_defaults_to_assumed_and_mass_checks_against_layers() {
        let e = enclosure(Material::Steel, Installation::FreeStanding);
        let mut s = ShellModel::from_enclosure(&e);
        assert_eq!(s.provenance.walls, Provenance::Assumed);
        assert!(s.mass_check(&e).is_none());
        let a = FaceAreas::of(&e);
        let sheet_kg = 7850.0 * 0.002 * a.outer.iter().sum::<f64>();
        s.mass_kg = Some(sheet_kg);
        assert!((s.mass_check(&e).unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn capacity_adds_up() {
        let e = enclosure(Material::Steel, Installation::FreeStanding);
        let s = ShellModel::from_enclosure(&e);
        let a = FaceAreas::of(&e);
        let area: f64 = a.outer.iter().sum();
        let want = 7850.0 * 490.0 * 0.002 * area;
        assert!((s.capacity_j_per_k(&e) / want - 1.0).abs() < 1e-9);
    }

    // ---- WallStack against analytic cases ----

    fn erfc(x: f64) -> f64 {
        // Abramowitz and Stegun 7.1.26, absolute error below 1.5e-7.
        let z = x.abs();
        let t = 1.0 / (1.0 + 0.327_591_1 * z);
        let y = t
            * (0.254_829_592
                + t * (-0.284_496_736
                    + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
        let e = y * (-z * z).exp();
        if x >= 0.0 { e } else { 2.0 - e }
    }

    #[test]
    fn thin_plate_cools_like_a_lumped_body() {
        // 3 mm aluminium, Biot << 1: T - T_air follows exp(-t / tau) with
        // tau = C / U (U from the two films in series).
        let films = Films {
            h_in: 8.0,
            h_out: 12.0,
        };
        let layers = [Layer::new(LayerMaterial::Aluminium, 3.0)];
        let mut w = WallStack::new(&layers, films, 60.0, 1.0);
        let c = LayerMaterial::Aluminium.rho_cp() * 0.003;
        // Both airs at 20 C; the plate starts at 60 C. Both sides are air so
        // the lumped sink conductance is h_in + h_out.
        let tau = c / (films.h_in + films.h_out);
        let dt = 1.0;
        let mut t_s = 0.0;
        for _ in 0..600 {
            w.step(dt, 20.0, 20.0);
            t_s += dt;
        }
        let want = 20.0 + 40.0 * (-t_s / tau).exp();
        let got = 0.5 * (w.t[0] + w.t[w.cells() - 1]);
        assert!((got - want).abs() < 0.15, "lumped {want} against {got}");
    }

    #[test]
    fn thick_slab_matches_the_semi_infinite_solution() {
        // Surface held at the air temperature (huge h), the far side far away:
        // T(x,t) = T0 + (Ts - T0) * erfc(x / (2 sqrt(alpha t))).
        let k = 1.0;
        let rho_cp = 2.0e6;
        let alpha = k / rho_cp;
        let m = LayerMaterial::Custom { k, rho_cp };
        let layers = [Layer::new(m, 400.0)];
        let films = Films {
            h_in: 1.0e7,
            h_out: 1.0e-6,
        };
        let mut w = WallStack::new(&layers, films, 20.0, 0.25);
        let t_end = 3600.0 * 3.0;
        let dt = 10.0;
        let mut t = 0.0;
        while t < t_end {
            w.step(dt, 80.0, 20.0);
            t += dt;
        }
        // Penetration depth 2 sqrt(alpha t) = 0.19 m < 0.4 m.
        let dx = 0.4 / w.cells() as f64;
        for depth_m in [0.02, 0.05, 0.10] {
            let i = (depth_m / dx) as usize;
            let x = (i as f64 + 0.5) * dx;
            let want = 20.0 + 60.0 * erfc(x / (2.0 * (alpha * t_end).sqrt()));
            assert!(
                (w.t[i] - want).abs() < 0.8,
                "x {x}: model {} analytic {want}",
                w.t[i]
            );
        }
    }

    #[test]
    fn stack_reaches_the_steady_profile_and_conserves_energy() {
        let layers = [
            Layer::new(LayerMaterial::Steel, 2.0),
            Layer::new(LayerMaterial::MineralWool, 20.0),
        ];
        let films = Films {
            h_in: 8.0,
            h_out: 10.0,
        };
        let mut w = WallStack::new(&layers, films, 20.0, 0.5);
        for _ in 0..40_000 {
            w.step(5.0, 50.0, 20.0);
        }
        let r_total: f64 = 1.0 / 8.0 + layers.iter().map(Layer::r).sum::<f64>() + 1.0 / 10.0;
        let q = 30.0 / r_total;
        let flux_in = w.flux_in_w_m2(50.0);
        assert!(
            (flux_in / q - 1.0).abs() < 1e-3,
            "steady flux {flux_in} vs {q}"
        );
        // Steady inner surface temperature.
        let ts = 50.0 - q / 8.0;
        assert!((w.inner_surface_c(50.0) - ts).abs() < 0.02);
        assert!(w.stored_j_m2(20.0) > 0.0);
    }

    #[test]
    fn time_constant_of_a_wall_with_mass() {
        // Foam sheet between two films: the step response of the stored heat
        // passes 63 % at roughly the lumped time constant for a Biot-small
        // body. Use a 5 mm aluminium plate with a 2 mm skin film pair.
        let films = Films {
            h_in: 10.0,
            h_out: 10.0,
        };
        let layers = [Layer::new(LayerMaterial::Aluminium, 5.0)];
        let mut w = WallStack::new(&layers, films, 20.0, 1.0);
        let c = LayerMaterial::Aluminium.rho_cp() * 0.005;
        let tau = c / 20.0;
        let mut t = 0.0;
        // inside 70 C, outside 70 C: the plate is driven to 70 C from both sides
        while t < tau {
            w.step(1.0, 70.0, 70.0);
            t += 1.0;
        }
        let rise = (w.t[0] - 20.0) / 50.0;
        assert!(
            (rise - (1.0 - (-1.0f64).exp())).abs() < 0.01,
            "63 % check {rise}"
        );
    }
}
