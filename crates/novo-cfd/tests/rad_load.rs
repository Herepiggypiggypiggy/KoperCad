//! The plate as a radiating surface and the conservation of the coupling load.

use novo_cfd::mesh::{Mesh, Region};
use novo_cfd::rad::{
    PatchKind, RadInputs, RadPlan, RadSettings, RadiationModel, RearPlate, ViewFactorSettings,
    WallBc,
};
use novo_cfd::{Grid3, MaterialInfo, MaterialKind};

fn mats() -> Vec<MaterialInfo> {
    let m = |id, kind| MaterialInfo {
        id,
        kind,
        name: format!("m{id}"),
        conductivity_w_mk: 1.0,
    };
    vec![
        m(0, MaterialKind::Air),
        m(1, MaterialKind::Plate),
        m(2, MaterialKind::Device(1)),
    ]
}

/// 100 mm cube, plate covering the rear wall up to z = 10 mm, a device
/// standing on it (z 10 to 40, x/y 30 to 60).
fn cabinet() -> (Mesh, RearPlate) {
    let grid = Grid3::uniform([10, 10, 10], [100.0; 3]);
    let dev = Region {
        material: 2,
        lo: [30.0, 30.0, 10.0],
        hi: [60.0, 60.0, 40.0],
    };
    let plate = RearPlate {
        cover: vec![1.0; 100],
        material: 1,
        front_z_mm: 10.0,
    };
    (Mesh::classify(grid, &[dev], mats()), plate)
}

fn settings() -> RadSettings {
    RadSettings {
        view_factors: ViewFactorSettings {
            rays_per_patch: 1500,
            ..Default::default()
        },
        ..RadSettings::default()
    }
}

#[test]
fn the_plate_is_a_surface_and_the_device_foot_is_not() {
    let (mesh, plate) = cabinet();
    let m = RadiationModel::build_with(&mesh, &settings(), Some(&plate)).unwrap();
    let kinds = |k: PatchKind| m.patches.patches.iter().filter(move |p| p.kind == k);
    assert!(kinds(PatchKind::Plate).count() > 0);
    assert!(kinds(PatchKind::Plate).all(|p| p.material == 1 && p.emissivity == 0.3));
    // No wall patch is left on the rear face, the plate covers it all.
    assert!(
        kinds(PatchKind::Wall).all(|p| p.normal != [0.0, 0.0, 1.0]),
        "rear wall still radiates"
    );
    // The foot of the device (low z face) is not a radiating patch.
    assert!(
        kinds(PatchKind::Solid).all(|p| p.normal != [0.0, 0.0, -1.0]),
        "device foot radiates"
    );
    // Plate area: the whole rear face.
    let area: f64 = kinds(PatchKind::Plate).map(|p| p.area_m2).sum();
    assert!((area - 0.01).abs() < 1e-12, "{area}");
    assert!(m.view_factors.summation_error() < 1e-10);
    assert!(m.view_factors.reciprocity_error() < 1e-10);
}

#[test]
fn without_a_plate_surface_the_rear_wall_radiates() {
    let (mesh, _) = cabinet();
    let m = RadiationModel::build(&mesh, &settings()).unwrap();
    assert!(m.patches.patches.iter().all(|p| p.kind != PatchKind::Plate));
}

#[test]
fn the_load_only_moves_heat_around() {
    let (mesh, plate) = cabinet();
    let m = RadiationModel::build_with(&mesh, &settings(), Some(&plate)).unwrap();
    let plan = RadPlan::new(m, mesh.grid.clone());
    let mut st = plan.state();
    // Hot device, warm air, cool plate sheet.
    let t_c: Vec<f64> = mesh
        .material
        .iter()
        .map(|&id| if id == 2 { 80.0 } else { 35.0 })
        .collect();
    let sheet_t = vec![30.0; 100];
    let walls = [WallBc::Overall {
        u: 3.0,
        t_out: 25.0,
    }; 6];
    let inp = RadInputs {
        t_c: &t_c,
        sheet_t_c: Some(&sheet_t),
        walls,
    };
    let mut load = plan.update(&mut st, &inp);
    // Run a few updates so the wall nodes and the relaxed fluxes settle.
    for _ in 0..30 {
        load = plan.update(&mut st, &inp);
    }
    let net = load.net_heat_w(&t_c, &sheet_t);
    let moved: f64 = load.power_by_material.iter().map(|p| p.1.abs()).sum();
    assert!(net.abs() < 1e-9 * moved, "net {net} of {moved}");
    // The device gives heat off, the plate and the walls take it.
    assert!(load.power_w(2) > 0.5, "{:?}", load.power_by_material);
    assert!(load.power_w(1) < 0.0);
    assert!(load.power_w(0) < 0.0);
    // Part of what the walls absorb leaves through them.
    assert!(load.wall_out_w.iter().sum::<f64>() > 0.0);
    // The device cells lose heat at their own temperature.
    let dev_cell = mesh.material.iter().position(|&m| m == 2).unwrap();
    assert!(load.cell_s[dev_cell] - load.cell_g[dev_cell] * 80.0 < 0.0);
    assert!(load.cell_g[dev_cell] > 0.0);
    // Adiabatic walls give everything back to the air.
    let adia = [WallBc::Overall { u: 0.0, t_out: 0.0 }; 6];
    let mut st2 = plan.state();
    let inp2 = RadInputs { walls: adia, ..inp };
    let mut l2 = plan.update(&mut st2, &inp2);
    for _ in 0..30 {
        l2 = plan.update(&mut st2, &inp2);
    }
    assert_eq!(l2.wall_out_w.iter().sum::<f64>(), 0.0);
    assert!(l2.net_heat_w(&t_c, &sheet_t).abs() < 1e-9 * moved);
}
