//! Outside film, mounting gaps, wind and sun in the 2D solver (lane H3).
//! The correlations are tested in `src/outside.rs`; these check the wiring:
//! the default stays the IEC wall, the physical film moves results the right
//! way and keeps the heat balance.
mod common;
use common::*;
use novo_thermal::shell::*;
use novo_thermal::*;

fn base() -> Scenario {
    scenario(vec![device(1, 300.0, 400.0, 100.0, 200.0, 80.0)])
}

fn physical(f: impl Fn(&mut Outside)) -> Scenario {
    let mut sc = base();
    let mut sh = ShellModel::from_enclosure(&sc.enclosure);
    sh.outside.film = OutsideFilm::Physical;
    f(&mut sh.outside);
    sc.params.shell = Some(sh);
    sc
}

fn rise(sc: Scenario) -> f64 {
    quick(sc).balance.mean_rise_k
}

#[test]
fn default_film_is_the_iec_wall_and_resolve_is_a_no_op() {
    let sc = base();
    assert_eq!(outside::resolve(&sc).unwrap(), sc);
    assert!(sc.shell().caps().outside);
}

#[test]
fn physical_film_stays_near_the_iec_wall_without_sun_or_wind() {
    let iec = rise(base());
    let phys = rise(physical(|_| {}));
    // Different models of the same wall: same order, not equal.
    assert!(phys > 0.6 * iec && phys < 1.6 * iec, "{phys} vs {iec}");
}

#[test]
fn resolved_scenario_is_stable_and_balanced() {
    let sc = physical(|_| {});
    let r1 = outside::resolve(&sc).unwrap();
    assert_eq!(outside::resolve(&r1).unwrap(), r1);
    let res = quick(sc);
    let b = &res.balance;
    let walls = b.front_w + b.rear_w + b.left_w + b.right_w + b.top_w + b.bottom_w;
    assert!(
        (walls - b.loss_w).abs() < 0.03 * b.loss_w,
        "{walls} {}",
        b.loss_w
    );
}

#[test]
fn wind_cools_and_sun_heats() {
    let still = rise(physical(|_| {}));
    let windy = rise(physical(|o| o.wind_m_s = 5.0));
    assert!(windy < still, "{windy} vs {still}");
    let sunny = rise(physical(|o| {
        o.sun_w_m2 = 800.0;
        o.sun_share = [0.0, 0.0, 0.5, 0.0, 1.0, 0.0];
    }));
    assert!(sunny > still + 1.0, "{sunny} vs {still}");
}

#[test]
fn dark_paint_in_the_sun_is_hotter_than_white() {
    let paint = |abs: f64| {
        let mut sc = physical(|o| {
            o.sun_w_m2 = 800.0;
            o.sun_share = [0.5, 0.0, 0.0, 0.0, 1.0, 0.0];
        });
        let mut sh = sc.shell();
        for f in &mut sh.faces {
            f.absorptance_solar = abs;
        }
        sc.params.shell = Some(sh);
        let r = quick(sc);
        r.shell.faces[TOP].outer_c
    };
    assert!(paint(0.9) > paint(0.25) + 5.0);
}

#[test]
fn wall_behind_the_cabinet_matters_with_its_gap() {
    let touching = rise(physical(|o| o.gaps_mm[REAR] = Some(0.0)));
    let narrow = rise(physical(|o| o.gaps_mm[REAR] = Some(15.0)));
    let free = rise(physical(|_| {}));
    assert!(
        touching > narrow && narrow >= free - 1e-6,
        "{touching} {narrow} {free}"
    );
}

#[test]
fn neighbours_on_both_sides_heat_the_cabinet() {
    let free = rise(physical(|_| {}));
    let row = rise(physical(|o| {
        o.gaps_mm[LEFT] = Some(5.0);
        o.gaps_mm[RIGHT] = Some(5.0);
    }));
    assert!(row > free, "{row} vs {free}");
}

#[test]
fn covered_flag_without_a_gap_uses_the_assumed_gap() {
    let mut sc = physical(|_| {});
    sc.enclosure.installation.rear_covered = true;
    let sh = sc.shell();
    assert_eq!(
        sh.gap_of(&sc.enclosure, REAR),
        Some((ASSUMED_COVER_GAP_MM, true))
    );
    assert!(rise(sc) > rise(physical(|_| {})) - 1e-6);
}
