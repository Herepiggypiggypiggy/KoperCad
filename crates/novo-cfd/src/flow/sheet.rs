//! The mounting plate as a thin sheet on the rear wall.
//!
//! A 2.5 mm steel plate is thinner than any cell the flow grid can afford, so
//! it is not meshed. It is one temperature per (x, y) column of the rear face
//! instead, the way the 2D engine treats it: it conducts sideways, stores
//! heat, loses heat to the outside through the rear wall, and exchanges heat
//! with the cell in front of it. A device standing on the plate touches it
//! through a contact resistance (`h_contact`, W/m2K of footprint); bare plate
//! gives off heat to the air through a film (`h_air`).
//!
//! The exchange with the cell in front is implicit in the cell equation of
//! [`super::FlowSolver`]; this module holds the configuration and the
//! per-column bookkeeping.

/// Configuration of the sheet. Columns are numbered `i + nx * j` over the
/// x and y cells of the rear face.
#[derive(Debug, Clone, PartialEq)]
pub struct PlateSheet {
    /// Share of each column's footprint that lies on the plate, 0 to 1.
    /// Precondition: one entry per x-y cell of the mesh (`nx * ny`);
    /// `FlowSolver::new` panics with a clear message otherwise.
    pub cover: Vec<f64>,
    /// In-plane conductance of the sheet, `k * thickness`, W/K.
    pub kp: f64,
    /// Heat capacity per area, J/m2K (already divided by the false-transient
    /// speed-up the solid cells use).
    pub cap: f64,
    /// Contact between a solid standing on the plate and the plate, W/m2K.
    pub h_contact: f64,
    /// Film between bare plate and the air in front of it, W/m2K.
    pub h_air: f64,
    /// Plate to outside air through the rear wall, W/m2K of plate.
    pub u_out: f64,
    /// Outside air, degrees C.
    pub t_out: f64,
}

/// Per-column constants derived from the sheet and the grid.
#[derive(Debug, Clone)]
pub(crate) struct SheetGeometry {
    pub nx: usize,
    /// Plate area of each column, m2.
    pub area: Vec<f64>,
    /// Lateral conductance to the next column in +x and +y, W/K.
    pub g_x: Vec<f64>,
    pub g_y: Vec<f64>,
}

impl SheetGeometry {
    pub fn new(sheet: &PlateSheet, dx: &[f64], dy: &[f64], cx: &[f64], cy: &[f64]) -> Self {
        let (nx, ny) = (dx.len(), dy.len());
        assert_eq!(sheet.cover.len(), nx * ny, "plate cover and mesh differ");
        let mut area = vec![0.0; nx * ny];
        let mut g_x = vec![0.0; nx * ny];
        let mut g_y = vec![0.0; nx * ny];
        for j in 0..ny {
            for i in 0..nx {
                let c = i + nx * j;
                area[c] = sheet.cover[c] * dx[i] * dy[j];
                if i + 1 < nx {
                    let f = sheet.cover[c].min(sheet.cover[c + 1]);
                    g_x[c] = sheet.kp * f * dy[j] / (cx[i + 1] - cx[i]);
                }
                if j + 1 < ny {
                    let f = sheet.cover[c].min(sheet.cover[c + nx]);
                    g_y[c] = sheet.kp * f * dx[i] / (cy[j + 1] - cy[j]);
                }
            }
        }
        SheetGeometry { nx, area, g_x, g_y }
    }
}
