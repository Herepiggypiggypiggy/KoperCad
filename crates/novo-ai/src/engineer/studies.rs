//! What the program calculates, for Claude to read: the heat estimate of a cabinet, the live
//! circuit simulation and the Autopilot plan, plus the edit that applies Autopilot.
//!
//! Everything here runs on the draft with the engines the windows use (`novo-thermal`,
//! `novo-sim`, `novo_reports::autopilot`). Results are estimates; each reply says so and the
//! system prompt tells Claude not to turn them into guarantees.

use super::cabinet::enclosure_arg;
use super::resolve::text_of;
use novo_checks::calc::tables::TableSet;
use novo_core::{Project, SymbolId, Tx};
use novo_numbering::{Mode, Reason, Scheme, Scope};
use novo_reports::autopilot::{self, Autopilot, Basis, Change, Proposal};
use novo_reports::{FerruleStandard, wire_cut_jobs_with};
use serde_json::{Value, json};
use std::collections::BTreeSet;

// ---------------------------------------------------------------- heat

/// Steady-state heat estimate of one enclosure at full load.
pub fn thermal(project: &Project, input: &Value) -> Result<Value, String> {
    let enclosure = enclosure_arg(project, input)?;
    let mut options = novo_thermal::bridge::BridgeOptions::default();
    if let Some(a) = input["ambient_c"].as_f64() {
        if !(-40.0..=80.0).contains(&a) {
            return Err("ambient_c must be between -40 and 80 degrees C".into());
        }
        options.ambient_c = a;
    }
    let bridge = novo_thermal::bridge::scenario_from_project(project, enclosure, &options)
        .map_err(|e| format!("{e}. Set it up with set_cabinet_plate, set_cabinet_interior and place_all_in_cabinet"))?;
    if bridge.report.placed == 0 {
        return Err("no device is placed on the plate yet, so there is no heat to estimate".into());
    }
    let result = if input["advice"].as_bool() == Some(true) {
        novo_thermal::simulate_steady_with_advice(&bridge.scenario)
    } else {
        novo_thermal::simulate_steady(&bridge.scenario)
    }
    .map_err(|e| format!("the heat model could not run: {e}"))?;
    let tag = |id: u64| {
        project
            .device(novo_core::DeviceId::from(id))
            .map_or_else(|| format!("#{id}"), |d| d.tag.to_string())
    };
    let names = |list: &[novo_thermal::bridge::DeviceRef]| -> Vec<String> {
        list.iter().map(|d| d.tag.clone()).collect()
    };
    let r = &bridge.report;
    let mut devices: Vec<_> = result.devices.iter().collect();
    devices.sort_by(|a, b| b.surface_c.total_cmp(&a.surface_c));
    Ok(json!({
        "ambient_c": options.ambient_c,
        "total_loss_w": (r.total_loss_w * 10.0).round() / 10.0,
        "air_at_half_height_c": result.air_at_half_height_c.round(),
        "air_at_top_c": result.air_at_top_c.round(),
        "converged": result.converged,
        "devices_hottest_first": devices.iter().map(|d| json!({
            "device": tag(d.id), "loss_w": d.loss_w,
            "surface_c": d.surface_c.round(),
            "surface_band_c": [d.surface_band_c.0.round(), d.surface_band_c.1.round()],
            "air_around_c": d.air_c.round(),
            "limit_max_ambient_c": d.rated_max_ambient_c,
            "margin_k": d.margin_k.map(|m| (m * 10.0).round() / 10.0),
            "status": format!("{:?}", d.status),
        })).collect::<Vec<_>>(),
        "hot_spots": result.hot_spots.iter().take(8).map(|h| json!({
            "temp_c": h.temp_c.round(), "reason": h.reason})).collect::<Vec<_>>(),
        "recommendations": result.recommendations.iter().take(5).map(|x| x.text.clone()).collect::<Vec<_>>(),
        "data_gaps": {
            "no_loss_value_counted_as_0_w": names(&r.without_loss),
            "size_is_a_guess": names(&r.guessed_size),
            "no_depth_assumed_60_mm": names(&r.without_depth),
            "not_placed_so_not_in_the_picture": names(&r.unplaced),
            "outside_the_interior": names(&r.outside),
        },
        "assumptions": result.assumptions.iter().take(10).map(|a| json!({
            "name": a.name, "value": a.value, "unit": a.unit, "low": a.low, "high": a.high})).collect::<Vec<_>>(),
        "notes": result.notes,
        "honesty": "a reduced-order 2D model with a stated band, not CFD and not a guarantee; check the maker's limits and a real measurement before relying on it",
    }))
}

// ---------------------------------------------------------- simulation

fn symbol_label(project: &Project, id: SymbolId) -> String {
    project
        .symbol(id)
        .and_then(|s| s.device())
        .and_then(|d| project.device(d))
        .map_or_else(|| format!("symbol #{}", id.raw()), |d| d.tag.to_string())
}

/// Operates hand devices in order and reports what the circuit does.
pub fn simulate(project: &Project, input: &Value) -> Result<Value, String> {
    use novo_sim::{Clock, MotorRun, PoleState, pins, rest_state, settle_timed};
    let steps = input["steps"].as_array().cloned().unwrap_or_default();
    if steps.len() > 30 {
        return Err("at most 30 steps".into());
    }
    let tags = |v: &Value| -> Vec<String> {
        v.as_array()
            .map(|a| a.iter().filter_map(text_of).collect())
            .unwrap_or_default()
    };
    let rest = rest_state(project);
    let mut manual: PoleState = rest.clone();
    let mut clock = Clock::new();
    let netlist = project.netlist();
    let forced = BTreeSet::new();
    let mut held: BTreeSet<SymbolId> = BTreeSet::new();
    let mut settled = settle_timed(project, &netlist, &manual, &forced, &held, &mut clock);
    held = settled.energized.clone();
    let mut log = Vec::new();
    for (i, step) in steps.iter().enumerate() {
        for (key, operated) in [("operate", true), ("release", false)] {
            for t in tags(&step[key]) {
                let poles = pins::poles_of_device(project, &t)
                    .map_err(|e| format!("step {}: {t}: {e:?}", i + 1))?;
                pins::operate_poles(&mut manual, &rest, &poles, operated);
            }
        }
        if let Some(s) = step["wait_s"].as_f64() {
            clock.advance(s.clamp(0.0, 3600.0));
        }
        // Twice: a coil that pulls in closes its seal-in contact, which keeps it in.
        for _ in 0..2 {
            settled = settle_timed(project, &netlist, &manual, &forced, &held, &mut clock);
            held = settled.energized.clone();
        }
        log.push(json!({
            "step": i + 1,
            "coils_on": settled.energized.iter().map(|s| symbol_label(project, *s)).collect::<Vec<_>>(),
        }));
    }
    let states = novo_sim::device_states(project, &netlist, &settled);
    let name = |s: SymbolId| symbol_label(project, s);
    let story = novo_sim::explain::story(project, &settled, &states, &name);
    let mut why = Vec::new();
    for t in tags(&input["explain"]).into_iter().take(6) {
        let device = pins::device_by_text(project, &t).map_err(|e| format!("{t}: {e:?}"))?;
        let Some(symbol) = project.symbols_of_device(device).first().map(|s| s.id) else {
            continue;
        };
        let answer =
            novo_sim::explain::answer_symbol(project, &netlist, &settled, &forced, symbol, &name);
        why.push(json!({
            "device": t,
            "dead": answer.dead,
            "lines": answer.lines.iter().map(|l| l.text.clone()).collect::<Vec<_>>(),
        }));
    }
    Ok(json!({
        "steps": log,
        "story": story,
        "supply_found": novo_sim::explain::has_supply(&netlist),
        "coils_on": settled.energized.iter().map(|s| symbol_label(project, *s)).collect::<Vec<_>>(),
        "lamps": states.lamps.iter().map(|(s, on)| json!({"lamp": name(*s), "lit": on})).collect::<Vec<_>>(),
        "motors": states.motors.iter().map(|(s, m)| json!({
            "motor": name(*s),
            "state": match m {
                MotorRun::Stopped => "stopped".to_owned(),
                MotorRun::PhaseLoss => "phase loss (hums)".to_owned(),
                MotorRun::Fault => "fault".to_owned(),
                MotorRun::Running { direction, speed } => format!("running {direction:?}, speed {speed}"),
            }})).collect::<Vec<_>>(),
        "why": why,
        "honesty": "a logic simulation of which contacts conduct, not a timing, current or fault-level study; nothing is stored in the project",
    }))
}

// ----------------------------------------------------------- autopilot

/// The plan the Autopilot window would show: sizes and colours from the calculations plus wire
/// numbers from the project's scheme.
pub fn autopilot_plan(project: &Project) -> Autopilot {
    let standard = FerruleStandard::of(project);
    let jobs = wire_cut_jobs_with(project, &standard);
    let mut plan = autopilot::plan(project, &TableSet::builtin(), &jobs);
    let scheme = Scheme::load(project);
    let numbers = novo_numbering::plan(project, &scheme, Mode::Fill, &Scope::Project);
    let extra: Vec<Proposal> = numbers
        .changed_groups()
        .filter_map(|g| {
            let new = g.new.clone()?;
            let potential = g
                .potential
                .as_deref()
                .map(|p| format!(" on {p}"))
                .unwrap_or_default();
            let why = match g.reason {
                Reason::New => "no number yet: the next free number of the project's scheme",
                Reason::Healed => {
                    "the wires of this net carry different numbers: they follow the rest"
                }
                Reason::Replaced => "the old number was used twice or is off the scheme",
                _ => "follows the project's numbering scheme",
            };
            Some(Proposal {
                id: 0,
                wires: g.wires.clone(),
                label: format!("{}{potential}", g.at),
                change: Change::Number(new),
                reason: why.to_owned(),
                basis: Basis::Rule,
            })
        })
        .collect();
    plan.extend(extra);
    plan
}

pub fn autopilot_json(project: &Project) -> Value {
    let plan = autopilot_plan(project);
    let kinds = ["Size", "Colour", "Number", "Cable", "Entry point"];
    json!({
        "counts": kinds.iter().map(|k| (k.to_string(), plan.count(k))).collect::<std::collections::BTreeMap<_, _>>(),
        "proposals": plan.proposals.iter().take(60).map(|p| json!({
            "kind": p.change.kind(), "connection": p.label, "value": p.change.value(),
            "why": p.reason.chars().take(160).collect::<String>(), "basis": p.basis.label(), "wires": p.wires.len(),
        })).collect::<Vec<_>>(),
        "proposals_total": plan.proposals.len(),
        "could_not_size": plan.unresolved,
        "left_to_the_person": plan.left_to_you,
        "tables": plan.tables,
        "planning_values": plan.planning_values,
        "tip": "run_autopilot applies it (assumed rows only with include_assumed)",
    })
}

pub fn run_autopilot(tx: &mut Tx<'_>, args: &Value) -> Result<String, String> {
    let plan = autopilot_plan(tx.project());
    let include_assumed = args["include_assumed"].as_bool() == Some(true);
    let kinds: Option<Vec<String>> = args["kinds"]
        .as_array()
        .map(|a| a.iter().filter_map(text_of).collect());
    let accepted: BTreeSet<usize> = plan
        .proposals
        .iter()
        .filter(|p| include_assumed || p.basis != Basis::Assumed)
        .filter(|p| {
            kinds
                .as_ref()
                .is_none_or(|k| k.iter().any(|x| x.eq_ignore_ascii_case(p.change.kind())))
        })
        .map(|p| p.id)
        .collect();
    if accepted.is_empty() {
        return Err("Autopilot has nothing to fill in with these settings".into());
    }
    let planning = plan.proposals.iter().any(|p| {
        accepted.contains(&p.id)
            && matches!(p.change, Change::Size(_))
            && matches!(p.basis, Basis::Planning | Basis::Assumed)
    });
    let (sized, coloured, numbered, cabled, entries) =
        plan.apply(tx, Some(&accepted)).map_err(|e| e.to_string())?;
    let mut parts = Vec::new();
    for (n, what) in [
        (sized, "sizes"),
        (coloured, "colours"),
        (numbered, "wire numbers"),
        (cabled, "cables"),
        (entries, "cabinet entry points"),
    ] {
        if n > 0 {
            parts.push(format!("{n} {what}"));
        }
    }
    if parts.is_empty() {
        return Err("Autopilot had nothing to change".into());
    }
    Ok(format!(
        "Autopilot: fill in {}{}",
        parts.join(", "),
        if planning {
            " (sizes are planning values, check them)"
        } else {
            ""
        }
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engineer::{Resources, actions};

    fn starter() -> (Project, Resources) {
        let res = Resources::new(Vec::new()).with_catalog(novo_parts::Catalog::starter());
        let s = crate::eval::builtin()
            .into_iter()
            .find(|s| s.id == "edit-add-run-lamp")
            .unwrap();
        let doc = crate::eval::base_document(&s.setup, &res).unwrap();
        (doc.project().clone(), res)
    }

    fn run(p: &mut Project, res: &Resources, a: &str, args: Value) -> Result<String, String> {
        actions::run_on(p, &res.env(), a, &args)
    }

    #[test]
    fn the_seal_in_holds_until_stop_is_pressed() {
        let (p, _) = starter();
        let r = simulate(
            &p,
            &json!({"steps": [{"operate": ["-S2"]}, {"release": ["-S2"]}]}),
        )
        .unwrap();
        assert_eq!(r["coils_on"], json!(["-K1"]), "{r}");
        assert!(
            r["motors"][0]["state"]
                .as_str()
                .unwrap()
                .starts_with("running")
        );
        let r = simulate(&p, &json!({"steps": [
            {"operate": ["-S2"]}, {"release": ["-S2"]}, {"operate": ["-S1"]}, {"release": ["-S1"]}]})).unwrap();
        assert_eq!(
            r["coils_on"],
            json!([]),
            "stop drops the contactor out: {r}"
        );
        let r = simulate(&p, &json!({"steps": [{"operate": ["-Z9"]}]}));
        assert!(r.is_err());
        let r = simulate(&p, &json!({"explain": ["-K1"]})).unwrap();
        assert_eq!(r["why"][0]["dead"], true, "not started: the coil is dead");
    }

    #[test]
    fn autopilot_is_read_then_applied() {
        let (mut p, res) = starter();
        let plan = autopilot_json(&p);
        assert!(plan["counts"]["Size"].as_u64().unwrap() > 0);
        let said = run(&mut p, &res, "run_autopilot", json!({"kinds": ["Colour"]})).unwrap();
        assert!(
            said.contains("colours") && !said.contains("sizes"),
            "{said}"
        );
        assert_eq!(autopilot_json(&p)["counts"]["Colour"], 0);
        let said = run(&mut p, &res, "run_autopilot", json!({})).unwrap();
        assert!(said.contains("planning values"), "{said}");
        assert!(p.wires().any(|w| w.cross_section_mm2.is_some()));
        // Assumed rows (no rating anywhere) wait for include_assumed.
        assert!(autopilot_json(&p)["proposals_total"].as_u64().unwrap() > 0);
        run(
            &mut p,
            &res,
            "run_autopilot",
            json!({"include_assumed": true}),
        )
        .unwrap();
        assert_eq!(autopilot_json(&p)["counts"]["Size"], 0);
        assert!(run(&mut p, &res, "run_autopilot", json!({})).is_err());
    }

    #[test]
    fn heat_needs_a_cabinet_and_then_names_its_gaps() {
        let (mut p, res) = starter();
        let why = thermal(&p, &json!({})).unwrap_err();
        assert!(why.contains("no enclosure"), "{why}");
        run(
            &mut p,
            &res,
            "add_cabinet",
            json!({"tag": "+CAB1", "width": 600, "height": 800}),
        )
        .unwrap();
        let why = thermal(&p, &json!({})).unwrap_err();
        assert!(
            why.contains("interior") || why.contains("no device"),
            "{why}"
        );
        run(
            &mut p,
            &res,
            "set_cabinet_interior",
            json!({"width": 800, "height": 1000, "depth": 300, "door_hinge": "left"}),
        )
        .unwrap();
        run(&mut p, &res, "place_all_in_cabinet", json!({})).unwrap();
        let r = thermal(&p, &json!({"ambient_c": 40})).unwrap();
        assert_eq!(r["ambient_c"], 40.0);
        assert!(
            r["devices_hottest_first"].as_array().unwrap().len() >= 5,
            "{r}"
        );
        assert!(r["honesty"].as_str().unwrap().contains("not CFD"));
        assert!(
            r["data_gaps"]["no_loss_value_counted_as_0_w"]
                .as_array()
                .is_some()
        );
        assert!(thermal(&p, &json!({"ambient_c": 500})).is_err());
    }
}
