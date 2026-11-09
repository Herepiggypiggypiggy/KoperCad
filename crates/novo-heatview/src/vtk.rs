//! Export of a solved cabinet as a VTK XML rectilinear grid (`.vtr`) and of
//! a heat-up as a ParaView collection (`.pvd`), for the user's own ParaView.
//!
//! Own writer from the public file format description; no VTK code, nothing
//! bundled. The solver grid is rectilinear with cell-centred values, which is
//! exactly VTK's `RectilinearGrid` with cell data, so nothing is resampled.
//!
//! Frame written (the same as the 3D view): `x` right, `y` UP, `z` from the
//! mounting plate towards the door. The solver's `y` points down, so the
//! `y` edges are mirrored about the top of the domain, the cell rows are
//! reversed and the `y` component of the velocity changes sign. Without that
//! the cabinet appears upside down in ParaView.

use std::fmt::Write as _;
use std::path::PathBuf;

use novo_cfd::{CfdSnapshot, MaterialKind};

/// Length unit of the coordinates in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LengthUnit {
    /// The project unit.
    #[default]
    Millimetres,
    Metres,
}

impl LengthUnit {
    fn scale(self) -> f64 {
        match self {
            LengthUnit::Millimetres => 1.0,
            LengthUnit::Metres => 1e-3,
        }
    }

    fn name(self) -> &'static str {
        match self {
            LengthUnit::Millimetres => "mm",
            LengthUnit::Metres => "m",
        }
    }
}

/// What goes beside the numbers.
#[derive(Debug, Clone, Default)]
pub struct VtkOptions {
    pub unit: LengthUnit,
    /// When the run was made (free text, e.g. `2026-11-04 08:15`).
    pub run_date: String,
    /// The cabinet or project the result belongs to.
    pub title: String,
    /// Seconds of a heat-up frame, `None` for a steady state.
    pub time_s: Option<f64>,
}

/// The honesty statement carried in the file, so a picture taken in
/// ParaView is not mistaken for a validated result.
pub fn statement(s: &CfdSnapshot, o: &VtkOptions) -> String {
    let status = if s.accuracy.validated {
        "validated against the KoperCAD validation suite"
    } else {
        "NOT validated against measurements; a model, not a guarantee"
    };
    let mut t = format!(
        "KoperCAD heat and airflow result. {}. Level: {}. Iterations: {}{}. Energy balance error: {:.2} %. Status: {}.",
        if o.title.is_empty() {
            "Cabinet"
        } else {
            &o.title
        },
        s.progress.fidelity.label(),
        s.progress.iteration,
        if s.progress.converged {
            ", converged"
        } else {
            ", NOT converged"
        },
        s.progress.energy_balance_rel * 100.0,
        status
    );
    if !s.accuracy.headline.is_empty() {
        let _ = write!(t, " {}", s.accuracy.headline);
    }
    if !o.run_date.is_empty() {
        let _ = write!(t, " Run: {}.", o.run_date);
    }
    t
}

/// Base64 (standard alphabet, padded).
fn b64(bytes: &[u8], out: &mut String) {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        out.push(A[(n >> 18) as usize & 63] as char);
        out.push(A[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 {
            A[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            A[n as usize & 63] as char
        } else {
            '='
        });
    }
}

/// One binary array: `UInt32` byte count, then the data, each base64 on its
/// own (what VTK writes for uncompressed inline data).
fn array(out: &mut String, ty: &str, name: &str, comps: usize, bytes: &[u8]) {
    let _ = write!(
        out,
        "<DataArray type=\"{ty}\" Name=\"{name}\" NumberOfComponents=\"{comps}\" format=\"binary\">"
    );
    b64(&(bytes.len() as u32).to_le_bytes(), out);
    b64(bytes, out);
    out.push_str("</DataArray>\n");
}

fn f32s(v: impl Iterator<Item = f32>) -> Vec<u8> {
    v.flat_map(f32::to_le_bytes).collect()
}

fn f64s(v: impl Iterator<Item = f64>) -> Vec<u8> {
    v.flat_map(f64::to_le_bytes).collect()
}

/// Text safe inside an XML comment or attribute.
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace("--", "- -")
}

/// A string as VTK writes string arrays: one integer per byte.
fn codes(s: &str) -> String {
    s.bytes()
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Why a snapshot cannot be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VtkError {
    /// Array lengths do not match the grid.
    Inconsistent,
}

impl std::fmt::Display for VtkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the result does not match its grid and cannot be exported")
    }
}

impl std::error::Error for VtkError {}

/// The `.vtr` text of a snapshot. Cell arrays: `Temperature` (degC),
/// `Velocity` (m/s, `y` up), `Pressure` (Pa), `TurbulentViscosityRatio`,
/// `Material` (0 air, else the id listed in the header comments), `Solid`
/// (1 where the cell is not air, to threshold on) and `WallHeatFlux`
/// (W/m2 on solid faces).
pub fn write_rectilinear(s: &CfdSnapshot, o: &VtkOptions) -> Result<String, VtkError> {
    if !s.is_consistent() {
        return Err(VtkError::Inconsistent);
    }
    let g = &s.grid;
    let k = o.unit.scale();
    let top = *g.y_edges_mm.last().unwrap_or(&0.0);
    let x = g.x_edges_mm.iter().map(|v| v * k);
    let y = g.y_edges_mm.iter().rev().map(|v| (top - v) * k);
    let z = g.z_edges_mm.iter().map(|v| v * k);
    // Solver cell of each VTK cell, x fastest, then y (bottom first), then z.
    let mut order = Vec::with_capacity(g.cells());
    for kk in 0..g.nz {
        for jv in 0..g.ny {
            for i in 0..g.nx {
                order.push(g.index(i, g.ny - 1 - jv, kk));
            }
        }
    }

    let st = statement(s, o);
    let mut out = String::with_capacity(g.cells() * 40 + 4096);
    out.push_str("<?xml version=\"1.0\"?>\n");
    let _ = writeln!(out, "<!-- {} -->", escape(&st));
    let _ = writeln!(
        out,
        "<!-- Frame: x right, y UP, z from the mounting plate towards the door. Length unit: {}. Temperature degC, velocity m/s, pressure Pa. -->",
        o.unit.name()
    );
    for m in &s.materials {
        let kind = match m.kind {
            MaterialKind::Air => "air".to_owned(),
            MaterialKind::Device(id) => format!("device {id}"),
            MaterialKind::Plate => "plate".to_owned(),
            MaterialKind::Wall => "wall".to_owned(),
            MaterialKind::Rail => "rail".to_owned(),
            MaterialKind::Duct => "duct".to_owned(),
        };
        let _ = writeln!(
            out,
            "<!-- Material {}: {} ({kind}) -->",
            m.id,
            escape(&m.name)
        );
    }
    out.push_str(
        "<VTKFile type=\"RectilinearGrid\" version=\"1.0\" byte_order=\"LittleEndian\" header_type=\"UInt32\">\n",
    );
    let ext = format!("0 {} 0 {} 0 {}", g.nx, g.ny, g.nz);
    let _ = writeln!(out, "<RectilinearGrid WholeExtent=\"{ext}\">");
    let _ = writeln!(out, "<Piece Extent=\"{ext}\">");

    out.push_str("<FieldData>\n");
    let _ = writeln!(
        out,
        "<DataArray type=\"String\" Name=\"Statement\" NumberOfTuples=\"1\" format=\"ascii\">{}</DataArray>",
        codes(&st)
    );
    let nums: [(&str, f64); 7] = [
        ("LengthUnitInMetres", 1e-3 / k),
        ("Validated", f64::from(u8::from(s.accuracy.validated))),
        ("Converged", f64::from(u8::from(s.progress.converged))),
        ("Iterations", s.progress.iteration as f64),
        ("EnergyBalanceError", s.progress.energy_balance_rel),
        ("FidelityCells", s.progress.fidelity.target_cells() as f64),
        ("TimeSeconds", o.time_s.unwrap_or(0.0)),
    ];
    for (name, v) in nums {
        let _ = writeln!(
            out,
            "<DataArray type=\"Float64\" Name=\"{name}\" NumberOfTuples=\"1\" format=\"ascii\">{v}</DataArray>"
        );
    }
    out.push_str("</FieldData>\n");

    out.push_str("<Coordinates>\n");
    array(&mut out, "Float64", "x", 1, &f64s(x));
    array(&mut out, "Float64", "y", 1, &f64s(y));
    array(&mut out, "Float64", "z", 1, &f64s(z));
    out.push_str("</Coordinates>\n");

    out.push_str("<CellData Scalars=\"Temperature\" Vectors=\"Velocity\">\n");
    let pick = |v: &[f32]| f32s(order.iter().map(|&c| v[c]));
    array(
        &mut out,
        "Float32",
        "Temperature",
        1,
        &pick(&s.temperature_c),
    );
    let vel = f32s(order.iter().flat_map(|&c| {
        let v = s.velocity[c];
        [v[0], -v[1], v[2]]
    }));
    array(&mut out, "Float32", "Velocity", 3, &vel);
    array(&mut out, "Float32", "Pressure", 1, &pick(&s.pressure_pa));
    array(
        &mut out,
        "Float32",
        "TurbulentViscosityRatio",
        1,
        &pick(&s.turb_visc_ratio),
    );
    let mat: Vec<u8> = order
        .iter()
        .flat_map(|&c| s.material[c].to_le_bytes())
        .collect();
    array(&mut out, "UInt16", "Material", 1, &mat);
    let solid: Vec<u8> = order
        .iter()
        .map(|&c| u8::from(s.material[c] != 0))
        .collect();
    array(&mut out, "UInt8", "Solid", 1, &solid);
    array(
        &mut out,
        "Float32",
        "WallHeatFlux",
        1,
        &pick(&s.wall_flux_w_m2),
    );
    out.push_str("</CellData>\n");
    out.push_str("</Piece>\n</RectilinearGrid>\n</VTKFile>\n");
    Ok(out)
}

/// The `.pvd` collection that ties the frames of a heat-up together:
/// `(seconds, file name)` per frame, names relative to the `.pvd`.
pub fn write_pvd(frames: &[(f64, String)]) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\"?>\n<VTKFile type=\"Collection\" version=\"0.1\" byte_order=\"LittleEndian\">\n<Collection>\n",
    );
    for (t, file) in frames {
        let _ = writeln!(
            out,
            "<DataSet timestep=\"{t}\" group=\"\" part=\"0\" file=\"{}\"/>",
            escape(file)
        );
    }
    out.push_str("</Collection>\n</VTKFile>\n");
    out
}

/// A small `pvpython` script that opens the file coloured by temperature,
/// to run from ParaView's Python shell (View > Python Shell > Run Script).
/// Only a convenience; the `.vtr` opens without it.
pub fn paraview_script(data_file: &str) -> String {
    let file = data_file.replace('\\', "\\\\").replace('\'', "\\'");
    format!(
        "# Written by KoperCAD. Temperature in degC, velocity in m/s, y points up.\n\
         # A model result: read the Statement field data of the file before quoting it.\n\
         from paraview.simple import *\n\
         data = OpenDataFile('{file}')\n\
         view = GetActiveViewOrCreate('RenderView')\n\
         show = Show(data, view)\n\
         ColorBy(show, ('CELLS', 'Temperature'))\n\
         show.RescaleTransferFunctionToDataRange(True, False)\n\
         show.SetScalarBarVisibility(view, True)\n\
         view.ResetCamera()\n\
         Render()\n"
    )
}

/// Where a user-installed ParaView may be, best first: an explicit path,
/// then the places a standard user installs to without admin rights
/// (`%LOCALAPPDATA%`, the profile, its Downloads), then `Program Files`.
/// `list_dir` names the entries of a folder (empty when it is missing).
pub fn paraview_candidates(
    override_path: Option<&str>,
    local_app_data: Option<&str>,
    user_profile: Option<&str>,
    list_dir: &dyn Fn(&str) -> Vec<String>,
) -> Vec<PathBuf> {
    let mut found = Vec::new();
    if let Some(p) = override_path.filter(|p| !p.is_empty()) {
        found.push(PathBuf::from(p));
    }
    let mut roots: Vec<String> = Vec::new();
    for base in [local_app_data, user_profile].into_iter().flatten() {
        for sub in ["", "\\Programs", "\\Downloads", "\\Apps"] {
            roots.push(format!("{base}{sub}"));
        }
    }
    roots.push("C:\\Program Files".into());
    for root in roots {
        let mut names: Vec<String> = list_dir(&root)
            .into_iter()
            .filter(|n| n.to_ascii_lowercase().starts_with("paraview"))
            .collect();
        // The highest version name first.
        names.sort();
        names.reverse();
        for n in names {
            found.push(PathBuf::from(format!("{root}\\{n}\\bin\\paraview.exe")));
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_cfd::{AccuracyStatement, Fidelity, Grid3, MaterialInfo, Progress, Residuals};

    fn unb64(s: &str) -> Vec<u8> {
        let val = |c: u8| match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            _ => 63,
        };
        let mut out = Vec::new();
        for c in s.as_bytes().chunks(4) {
            let n = c.iter().take_while(|&&b| b != b'=').count();
            let mut v = 0u32;
            for (i, &b) in c.iter().enumerate() {
                v |= u32::from(if b == b'=' { 0 } else { val(b) }) << (18 - 6 * i);
            }
            out.extend_from_slice(&v.to_be_bytes()[1..n]);
        }
        out
    }

    /// Reads array `name` back out of our own file: (type, components, bytes).
    fn read(text: &str, name: &str) -> (String, usize, Vec<u8>) {
        let tag = format!("Name=\"{name}\" NumberOfComponents=\"");
        let at = text.find(&tag).unwrap_or_else(|| panic!("no array {name}"));
        let head = &text[text[..at].rfind("<DataArray").unwrap()..at];
        let ty = head
            .split("type=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        let rest = &text[at + tag.len()..];
        let comps: usize = rest.split('"').next().unwrap().parse().unwrap();
        let body = rest.split('>').nth(1).unwrap().split('<').next().unwrap();
        // The 4-byte count is the first 8 base64 characters.
        let (h, d) = body.split_at(8);
        let n = u32::from_le_bytes(unb64(h)[..4].try_into().unwrap()) as usize;
        let bytes = unb64(d);
        assert_eq!(bytes.len(), n, "{name} byte count");
        (ty.to_owned(), comps, bytes)
    }

    fn floats(b: &[u8]) -> Vec<f32> {
        b.chunks(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect()
    }

    fn doubles(b: &[u8]) -> Vec<f64> {
        b.chunks(8)
            .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
            .collect()
    }

    /// 2 x 3 x 2 cells, nothing symmetric: temperature = 100 j + 10 i + k,
    /// velocity (0.5, j + 1, -0.25) with y down, one solid cell, uneven `y`.
    fn snap() -> CfdSnapshot {
        let mut grid = Grid3::uniform([2, 3, 2], [200.0, 300.0, 100.0]);
        grid.y_edges_mm = vec![0.0, 50.0, 150.0, 300.0];
        let n = grid.cells();
        let mut temperature_c = vec![0.0; n];
        let mut velocity = vec![[0.0; 3]; n];
        let mut material = vec![0u16; n];
        for k in 0..2 {
            for j in 0..3 {
                for i in 0..2 {
                    let c = grid.index(i, j, k);
                    temperature_c[c] = (100 * j + 10 * i + k) as f32;
                    velocity[c] = [0.5, (j + 1) as f32, -0.25];
                }
            }
        }
        material[grid.index(1, 0, 1)] = 3;
        CfdSnapshot {
            grid,
            temperature_c,
            velocity,
            pressure_pa: (0..n).map(|c| c as f32).collect(),
            turb_visc_ratio: vec![0.0; n],
            material,
            materials: vec![MaterialInfo {
                id: 3,
                kind: MaterialKind::Device(7),
                name: "Drive <A&B>".into(),
                conductivity_w_mk: 100.0,
            }],
            wall_flux_w_m2: (0..n).map(|c| 10.0 * c as f32).collect(),
            devices: vec![],
            hot_spots: vec![],
            progress: Progress {
                fidelity: Fidelity::Coarse,
                iteration: 321,
                converged: false,
                energy_balance_rel: 0.0123,
                residuals: Residuals::default(),
                elapsed_s: 0.0,
                monitor_drift_k: 0.0,
            },
            notes: vec![],
            accuracy: AccuracyStatement::default(),
            heat_paths: vec![],
            radiation_on: false,
            opening_flows: vec![],
            plate_rear_c: None,
            plate_field_c: None,
            wall_field_c: None,
        }
    }

    fn opts() -> VtkOptions {
        VtkOptions {
            run_date: "2026-11-04 08:15".into(),
            title: "Cabinet K1".into(),
            ..VtkOptions::default()
        }
    }

    #[test]
    fn y_is_up_the_rows_follow_and_velocity_y_flips() {
        let t = write_rectilinear(&snap(), &opts()).unwrap();
        // Edges mirrored about the top (300): 0 150 250 300.
        assert_eq!(doubles(&read(&t, "y").2), vec![0.0, 150.0, 250.0, 300.0]);
        // The first VTK row (bottom) is the solver's last row (j = 2).
        let temp = floats(&read(&t, "Temperature").2);
        assert_eq!(&temp[..6], &[200.0, 210.0, 100.0, 110.0, 0.0, 10.0]);
        // Second layer (k = 1): same rows, +1.
        assert_eq!(&temp[6..8], &[201.0, 211.0]);
        let (_, comps, vb) = read(&t, "Velocity");
        let vel = floats(&vb);
        assert_eq!(comps, 3);
        // Bottom row is solver j = 2 (y component 3, down) -> -3 (up).
        assert_eq!(&vel[..3], &[0.5, -3.0, -0.25]);
        // Top row is solver j = 0 -> -1.
        assert_eq!(&vel[12..15], &[0.5, -1.0, -0.25]);
    }

    #[test]
    fn a_hot_cell_low_in_the_cabinet_stays_low() {
        // The same asymmetric case read the way ParaView does: the height
        // of the hottest cell centre is that of the solver's, measured up.
        let s = snap();
        let t = write_rectilinear(&s, &opts()).unwrap();
        let temp = floats(&read(&t, "Temperature").2);
        let y = doubles(&read(&t, "y").2);
        let hot = temp
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0;
        let row = (hot / 2) % 3;
        let centre_up = 0.5 * (y[row] + y[row + 1]);
        // Solver: hottest is row j = 2 with centre y = 225 mm down of 300.
        assert!((centre_up - (300.0 - 225.0)).abs() < 1e-9);
    }

    #[test]
    fn units_are_millimetres_or_metres_and_named_in_the_file() {
        let s = snap();
        let mm = write_rectilinear(&s, &opts()).unwrap();
        assert_eq!(doubles(&read(&mm, "x").2), vec![0.0, 100.0, 200.0]);
        assert!(mm.contains("Length unit: mm"));
        assert!(
            mm.contains("Name=\"LengthUnitInMetres\" NumberOfTuples=\"1\" format=\"ascii\">0.001<")
        );
        let m = write_rectilinear(
            &s,
            &VtkOptions {
                unit: LengthUnit::Metres,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(doubles(&read(&m, "x").2), vec![0.0, 0.1, 0.2]);
        assert_eq!(doubles(&read(&m, "z").2), vec![0.0, 0.05, 0.1]);
        assert!(m.contains("Length unit: m."));
        assert!(m.contains(">1<"), "LengthUnitInMetres is 1 for metres");
        // Temperatures and speeds do not change with the length unit.
        assert_eq!(read(&mm, "Temperature").2, read(&m, "Temperature").2);
    }

    #[test]
    fn values_are_cell_data_and_solid_cells_are_flagged() {
        let s = snap();
        let t = write_rectilinear(&s, &opts()).unwrap();
        assert!(t.contains("<CellData"));
        assert!(!t.contains("<PointData"));
        assert!(t.contains("WholeExtent=\"0 2 0 3 0 2\""));
        // One value per cell, not per point.
        assert_eq!(floats(&read(&t, "Pressure").2).len(), 12);
        let solid = read(&t, "Solid").2;
        // Solver (i 1, j 0, k 1) -> VTK row 2 of layer 1, x 1.
        let want = 6 + 2 * 2 + 1;
        assert_eq!(solid.iter().filter(|&&b| b == 1).count(), 1);
        assert_eq!(solid[want], 1);
        let mat = read(&t, "Material").2;
        assert_eq!(&mat[want * 2..want * 2 + 2], &3u16.to_le_bytes());
        assert!(t.contains("Material 3: Drive &lt;A&amp;B&gt; (device 7)"));
        // Wall flux follows the same reordering: solver cell 0 is the
        // last VTK row of layer 0, x 0.
        let flux = floats(&read(&t, "WallHeatFlux").2);
        assert_eq!(flux[4], 0.0);
    }

    #[test]
    fn the_file_says_it_is_a_model_not_a_validated_result() {
        let t = write_rectilinear(&snap(), &opts()).unwrap();
        for want in [
            "NOT validated",
            "Coarse 3D",
            "NOT converged",
            "Iterations: 321",
            "Run: 2026-11-04 08:15",
            "Cabinet K1",
        ] {
            assert!(t.contains(want), "{want}");
        }
        assert!(t.contains("Name=\"Validated\" NumberOfTuples=\"1\" format=\"ascii\">0<"));
        // The same sentence is in the field data, as character codes.
        let st = statement(&snap(), &opts());
        assert!(t.contains(&codes(&st)));
        let mut ok = snap();
        ok.accuracy.validated = true;
        let t2 = write_rectilinear(&ok, &opts()).unwrap();
        assert!(t2.contains("validated against the KoperCAD validation suite"));
        assert!(!t2.contains("NOT validated"));
    }

    #[test]
    fn the_file_is_binary_and_round_trips_exactly() {
        let s = snap();
        let t = write_rectilinear(&s, &opts()).unwrap();
        assert!(t.contains("format=\"binary\""));
        assert!(t.contains("header_type=\"UInt32\""));
        // Four bytes per cell for temperature: far below a text dump.
        let (ty, _, b) = read(&t, "Temperature");
        assert_eq!(ty, "Float32");
        assert_eq!(b.len(), 12 * 4);
        // Undo the frame change and get the solver's own array back.
        let temp = floats(&b);
        let mut back = vec![0.0f32; 12];
        for kk in 0..2 {
            for jv in 0..3 {
                for i in 0..2 {
                    back[s.grid.index(i, 2 - jv, kk)] = temp[(kk * 3 + jv) * 2 + i];
                }
            }
        }
        assert_eq!(back, s.temperature_c);
        // Base64 padding for every remainder.
        for n in 0..4usize {
            let bytes: Vec<u8> = (0..n as u8).collect();
            let mut text = String::new();
            b64(&bytes, &mut text);
            assert_eq!(unb64(&text), bytes);
        }
    }

    #[test]
    fn a_mismatched_snapshot_is_refused() {
        let mut s = snap();
        s.pressure_pa.pop();
        assert_eq!(
            write_rectilinear(&s, &opts()).unwrap_err(),
            VtkError::Inconsistent
        );
    }

    #[test]
    fn the_collection_lists_every_frame_with_its_time() {
        let pvd = write_pvd(&[(0.0, "a 0000.vtr".into()), (60.0, "a 0001.vtr".into())]);
        assert!(pvd.contains("type=\"Collection\""));
        assert!(pvd.contains("timestep=\"0\" group=\"\" part=\"0\" file=\"a 0000.vtr\""));
        assert!(pvd.contains("timestep=\"60\" group=\"\" part=\"0\" file=\"a 0001.vtr\""));
    }

    #[test]
    fn the_script_escapes_the_path() {
        let s = paraview_script("C:\\Users\\o'neil\\a.vtr");
        assert!(s.contains("OpenDataFile('C:\\\\Users\\\\o\\'neil\\\\a.vtr')"));
        assert!(s.contains("'CELLS', 'Temperature'"));
    }

    #[test]
    fn paraview_is_looked_for_in_user_places_before_program_files() {
        let list = |dir: &str| -> Vec<String> {
            match dir {
                "C:\\Users\\a\\AppData\\Local\\Programs" => {
                    vec![
                        "Notepad".into(),
                        "ParaView 5.11.0".into(),
                        "ParaView 5.12.1".into(),
                    ]
                }
                "C:\\Program Files" => vec!["ParaView 5.10.0".into()],
                _ => vec![],
            }
        };
        let c = paraview_candidates(
            Some("D:\\pv\\paraview.exe"),
            Some("C:\\Users\\a\\AppData\\Local"),
            Some("C:\\Users\\a"),
            &list,
        );
        let s: Vec<String> = c.iter().map(|p| p.display().to_string()).collect();
        assert_eq!(s[0], "D:\\pv\\paraview.exe");
        assert_eq!(
            s[1],
            "C:\\Users\\a\\AppData\\Local\\Programs\\ParaView 5.12.1\\bin\\paraview.exe"
        );
        assert!(s[2].contains("5.11.0"));
        assert!(s.last().unwrap().contains("Program Files"));
        assert!(!s.iter().any(|p| p.contains("Notepad")));
    }
}
