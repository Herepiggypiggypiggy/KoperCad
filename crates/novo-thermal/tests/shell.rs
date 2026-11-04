//! The layered shell in the 2D solver (lane H1): the default shell equals the
//! one-number wall, insulation, a door window and a plate setback move the
//! result the right way, and the reported surface temperatures obey the
//! balance.
mod common;
use common::*;
use novo_thermal::shell::*;
use novo_thermal::*;

fn base() -> Scenario {
    scenario(vec![device(1, 300.0, 400.0, 100.0, 200.0, 80.0)])
}

#[test]
fn default_shell_equals_the_one_number_wall() {
    let plain = quick(base());
    let mut sc = base();
    sc.params.shell = Some(ShellModel::from_enclosure(&sc.enclosure));
    let explicit = quick(sc);
    assert!((plain.balance.mean_rise_k - explicit.balance.mean_rise_k).abs() < 1e-9);
    assert!(
        (plain.devices[0].surface_c - explicit.devices[0].surface_c).abs() < 1e-9,
        "default shell must not move any result"
    );
    // And the IEC comparison stays inside the old agreement: sum of face heat
    // equals the loss.
    let b = &plain.balance;
    let walls = b.front_w + b.rear_w + b.left_w + b.right_w + b.top_w + b.bottom_w;
    assert!((walls - b.loss_w).abs() < 0.02 * b.loss_w);
    assert!((plain.shell.wall_heat_w() - walls).abs() < 1e-6);
}

#[test]
fn wall_surfaces_lie_between_the_air_and_the_ambient() {
    let r = quick(base());
    let amb = r.ambient_c;
    for f in &r.shell.faces {
        if f.exposed_share == 0.0 {
            continue;
        }
        assert!(f.outer_c >= amb - 1e-9, "{f:?}");
        assert!(
            f.inner_c >= f.outer_c - 1e-9,
            "inner colder than outer {f:?}"
        );
        assert!(f.inner_c <= f.inner_air_c + 1e-9);
        assert!(f.outer_min_c <= f.outer_c && f.outer_c <= f.outer_max_c);
    }
    // The outer film carries the face's heat: h_out * A * (T_out - T_amb)
    // equals the face heat (exposed area b*A).
    let sc = base();
    let a = FaceAreas::of(&sc.enclosure);
    let h_out = sc.shell().films.h_out;
    for fi in [LEFT, RIGHT, TOP] {
        let f = &r.shell.faces[fi];
        let q = h_out * f.exposed_share * a.outer[fi] * (f.outer_c - amb);
        assert!(
            (q / f.heat_w - 1.0).abs() < 0.02,
            "face {fi}: {q} vs {}",
            f.heat_w
        );
    }
    // Hot air rises: the top wall is warmer than the bottom one inside.
    assert!(r.shell.faces[TOP].inner_air_c > r.shell.faces[BOTTOM].inner_air_c);
}

#[test]
fn insulation_keeps_the_heat_in() {
    let plain = quick(base());
    let mut sc = base();
    let mut shell = ShellModel::from_enclosure(&sc.enclosure);
    shell.add_insulation(LayerMaterial::MineralWool, 20.0, true);
    sc.params.shell = Some(shell);
    let ins = quick(sc);
    assert!(ins.balance.mean_rise_k > 2.0 * plain.balance.mean_rise_k);
    // Outside of the wall stays cool, inside of the wall is nearly at air temperature.
    let f = &ins.shell.faces[LEFT];
    assert!(f.outer_c - ins.ambient_c < 0.2 * (f.inner_air_c - ins.ambient_c));
}

#[test]
fn a_door_window_loses_more_than_an_insulated_door() {
    // Plain: single 4 mm glass has less resistance than a 20 mm foam door.
    let mut sc = base();
    let mut shell = ShellModel::from_enclosure(&sc.enclosure);
    shell.add_insulation(LayerMaterial::Foam, 20.0, true);
    sc.params.shell = Some(shell.clone());
    let closed = quick(sc.clone());
    shell.window = Some(Window::single_glazed(0.3));
    sc.params.shell = Some(shell.clone());
    let window = quick(sc.clone());
    assert!(window.balance.front_w > 1.3 * closed.balance.front_w);
    assert!(window.shell.door().window.is_some());
    let (wi, wo) = window.shell.door().window.unwrap();
    assert!(wi > wo);
    // Double glazing is better than single.
    shell.window = Some(Window::double_glazed(0.3));
    sc.params.shell = Some(shell);
    let dbl = quick(sc);
    assert!(dbl.balance.front_w < window.balance.front_w);
}

#[test]
fn setback_makes_the_plate_hotter() {
    let plain = quick(base());
    let mut sc = base();
    let mut shell = ShellModel::from_enclosure(&sc.enclosure);
    shell.plate_setback_mm = Some(20.0);
    sc.params.shell = Some(shell);
    let gap = quick(sc);
    let p0 = plain.shell.plate_rear_c.unwrap();
    let p1 = gap.shell.plate_rear_c.unwrap();
    assert!(p1 > p0 + 0.5, "plate {p0} -> {p1}");
}

#[test]
fn wall_capacity_matches_the_layers() {
    let sc = base();
    let r = quick(sc.clone());
    let a = FaceAreas::of(&sc.enclosure);
    let area: f64 = a.outer.iter().sum();
    let want = 7850.0 * 490.0 * 0.002 * area;
    assert!((r.shell.capacity_j_per_k / want - 1.0).abs() < 1e-9);
}

#[test]
fn an_extra_heat_source_adds_its_heat_to_the_balance() {
    let plain = quick(base());
    let mut sc = base();
    sc.params.sources.push(HeatSource {
        id: 1,
        rect: novo_layout::Rect {
            x: 100.0,
            y: 100.0,
            w: 400.0,
            h: 30.0,
        },
        power_w: 40.0,
        blockage: None,
    });
    let hot = quick(sc);
    // 80 W of device plus 40 W of busbar: the walls and openings carry 120 W.
    assert!(
        (hot.balance.loss_w - 120.0).abs() < 1e-6,
        "{}",
        hot.balance.loss_w
    );
    assert!(hot.balance.residual_rel.abs() < 0.02);
    // The mean rise follows the total heat (IEC: rise proportional to loss).
    let ratio = hot.balance.mean_rise_k / plain.balance.mean_rise_k;
    assert!((ratio - 1.5).abs() < 0.2, "rise ratio {ratio}");
    let amb = hot.ambient_c;
    let walls: f64 = hot.shell.faces.iter().map(|f| f.heat_w).sum();
    assert!((walls / 120.0 - 1.0).abs() < 0.02 && walls > 0.0 && amb > 0.0);
}

#[test]
fn a_negative_source_is_refused() {
    let mut sc = base();
    sc.params.sources.push(HeatSource {
        id: 1,
        rect: novo_layout::Rect {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
        },
        power_w: -5.0,
        blockage: None,
    });
    assert!(simulate_steady(&sc).is_err());
}
