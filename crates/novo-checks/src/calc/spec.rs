//! A whole supply and its circuits as JSON: the input of
//! `kopercad-cli calc` and of the Python API, and the format a project's
//! circuits can be exported to for a second opinion.
//!
//! ```json
//! {
//!   "supply": { "voltage": 400,
//!               "transformer": { "kva": 630, "uk_percent": 4, "pk_w": 6500 } },
//!   "circuits": [
//!     { "name": "Q1", "load_a": 120, "phases": 3,
//!       "device": { "type": "mccb", "rated_a": 160, "icu_ka": 25,
//!                   "ir_a": 160, "tr_s": 12, "isd_x": 8, "tsd_s": 0.2, "ii_x": 12 },
//!       "cable": { "section_mm2": 70, "length_m": 40, "insulation": "XLPE",
//!                  "method": "C", "pe_mm2": 35 } },
//!     { "name": "M1", "from": "Q1", "load_a": 14, "power_factor": 0.85,
//!       "device": { "type": "mcb", "curve": "C", "rated_a": 16, "icu_ka": 10 },
//!       "cable": { "section_mm2": 2.5, "length_m": 20, "pe_mm2": 2.5 } }
//!   ]
//! }
//! ```
//!
//! `supply` has either `short_circuit_ka` (and optionally
//! `min_short_circuit_ka`) or a `transformer` (`kva`, `uk_percent`, and
//! optionally `pk_w`, `zero_ratio`, `network_sk_mva` as `[max, min]`).
//! A circuit with `"from": "<name>"` hangs off the end of that earlier
//! circuit's cable, which is how a feeder, a board and its branch
//! circuits are described. Every optional field that is left out is either
//! unknown (and the report says so) or defaulted with a line in the
//! report's assumptions (`power_factor` 1, `drop_limit_percent` 5,
//! `ambient_c` 30, `grouped_circuits` 1, `parallel` 1, `insulation` PVC,
//! `method` C).

use std::collections::HashMap;

use serde_json::Value;

use super::Insulation;
use super::Rule;
use super::cable::Method;
use super::circuit::{CableData, CircuitDesign, CircuitReport, Device, Row, Status, evaluate};
use super::curves::{Characteristic, Electronic, McbCurve, RelayClass};
use super::drop::Supply;
use super::network::{Branch, Network, NodeId, SourceModel, Transformer};
use super::selectivity::{Backup, Basis, PairTable, Status as SelStatus, backup, selectivity};
use super::short_circuit::Trip;
use super::tables::TableSet;

/// A parsed supply with its circuits, ready to evaluate.
#[derive(Debug, Clone)]
pub struct Plant {
    pub network: Network,
    pub circuits: Vec<PlantCircuit>,
}

/// One circuit of a [`Plant`].
#[derive(Debug, Clone)]
pub struct PlantCircuit {
    /// The node its device is connected to.
    pub at: NodeId,
    /// The node at the end of its cable.
    pub end: NodeId,
    pub design: CircuitDesign,
    /// Index of the circuit it is fed from (its upstream device), if any.
    pub parent: Option<usize>,
}

fn number(v: &Value, path: &str) -> Result<f64, String> {
    v.as_f64()
        .filter(|n| n.is_finite())
        .ok_or_else(|| format!("{path}: expected a number"))
}

fn opt_number(obj: &Value, key: &str, path: &str) -> Result<Option<f64>, String> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => number(v, &format!("{path}.{key}")).map(Some),
    }
}

fn req_number(obj: &Value, key: &str, path: &str) -> Result<f64, String> {
    opt_number(obj, key, path)?.ok_or_else(|| format!("{path}.{key}: missing"))
}

fn opt_text<'a>(obj: &'a Value, key: &str, path: &str) -> Result<Option<&'a str>, String> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.as_str())),
        Some(_) => Err(format!("{path}.{key}: expected text")),
    }
}

fn supply(v: &Value) -> Result<(f64, SourceModel), String> {
    let voltage = opt_number(v, "voltage", "supply")?.unwrap_or(400.0);
    if let Some(t) = v.get("transformer").filter(|t| !t.is_null()) {
        let mut tr = Transformer::new(
            req_number(t, "kva", "supply.transformer")?,
            req_number(t, "uk_percent", "supply.transformer")?,
            voltage,
        );
        tr.pk_w = opt_number(t, "pk_w", "supply.transformer")?;
        if let Some(z) = opt_number(t, "zero_ratio", "supply.transformer")? {
            tr.zero_ratio = z;
        }
        if let Some(Value::Array(a)) = t.get("network_sk_mva") {
            if a.len() != 2 {
                return Err("supply.transformer.network_sk_mva: expected [max, min]".into());
            }
            tr.network_sk_mva = Some((
                number(&a[0], "supply.transformer.network_sk_mva[0]")?,
                number(&a[1], "supply.transformer.network_sk_mva[1]")?,
            ));
        }
        let src = SourceModel::from_transformer(&tr)
            .ok_or("supply.transformer: values are not a valid transformer")?;
        return Ok((voltage, src));
    }
    let max = req_number(v, "short_circuit_ka", "supply")?;
    let min = opt_number(v, "min_short_circuit_ka", "supply")?;
    let src = SourceModel::from_short_circuit(max, min, voltage)
        .ok_or("supply: short-circuit currents must be positive and min ≤ max")?;
    Ok((voltage, src))
}

fn device(v: &Value, path: &str) -> Result<Device, String> {
    let rated = req_number(v, "rated_a", path)?;
    let kind = opt_text(v, "type", path)?
        .unwrap_or("none")
        .to_ascii_lowercase();
    let mut d = Device {
        id: opt_text(v, "id", path)?.map(str::to_owned),
        rated_a: rated,
        icu_ka: opt_number(v, "icu_ka", path)?,
        let_through_i2t: opt_number(v, "let_through_i2t", path)?,
        ..Device::default()
    };
    match kind.as_str() {
        "mcb" => {
            let letter = opt_text(v, "curve", path)?.ok_or(format!("{path}.curve: missing"))?;
            let trip = Trip::parse(letter).ok_or(format!("{path}.curve: B, C or D"))?;
            d.trip = Some(trip);
            d.characteristic =
                McbCurve::from_trip(trip).map(|curve| Characteristic::Mcb { curve, in_a: rated });
        }
        "fuse_gg" | "gg" => d.characteristic = Some(Characteristic::FuseGg { in_a: rated }),
        "relay" => {
            let class = opt_text(v, "class", path)?.ok_or(format!("{path}.class: missing"))?;
            d.characteristic = Some(Characteristic::Relay {
                class: RelayClass::parse(class)
                    .ok_or(format!("{path}.class: 10A, 10, 20 or 30"))?,
                ir_a: rated,
            });
        }
        "mccb" | "acb" => {
            d.characteristic = Some(Characteristic::Electronic(Electronic {
                in_a: rated,
                ir_a: req_number(v, "ir_a", path)?,
                tr_s: req_number(v, "tr_s", path)?,
                isd_x: req_number(v, "isd_x", path)?,
                tsd_s: req_number(v, "tsd_s", path)?,
                ii_x: req_number(v, "ii_x", path)?,
            }));
        }
        "none" => {
            if let Some(f) = opt_number(v, "trip_factor", path)? {
                d.trip = Some(Trip::Factor(f));
            }
        }
        other => return Err(format!("{path}.type: unknown device type '{other}'")),
    }
    Ok(d)
}

fn cable(v: &Value, path: &str, label: &str, notes: &mut Vec<String>) -> Result<CableData, String> {
    let insulation = match opt_text(v, "insulation", path)? {
        None => {
            notes.push(format!("{label}: cable insulation not given, PVC assumed"));
            Insulation::Pvc
        }
        Some(t) => match t.to_ascii_uppercase().as_str() {
            "PVC" => Insulation::Pvc,
            "XLPE" | "EPR" => Insulation::Xlpe,
            _ => return Err(format!("{path}.insulation: PVC or XLPE")),
        },
    };
    let method = match opt_text(v, "method", path)? {
        None => {
            notes.push(format!("{label}: installation method not given, C assumed"));
            Method::C
        }
        Some(t) => Method::parse(t).ok_or(format!("{path}.method: B1, B2 or C"))?,
    };
    let ambient_c = match opt_number(v, "ambient_c", path)? {
        Some(a) => a,
        None => {
            notes.push(format!(
                "{path}: ambient temperature not given, 30 °C assumed"
            ));
            30.0
        }
    };
    let whole = |key: &str, default: u32, notes: &mut Vec<String>| -> Result<u32, String> {
        match opt_number(v, key, path)? {
            Some(n) if n >= 1.0 && n.fract() == 0.0 => Ok(n as u32),
            Some(_) => Err(format!("{path}.{key}: expected a whole number ≥ 1")),
            None => {
                if key == "grouped_circuits" {
                    notes.push(format!("{label}: grouping not given, this circuit alone"));
                }
                Ok(default)
            }
        }
    };
    Ok(CableData {
        section_mm2: req_number(v, "section_mm2", path)?,
        insulation,
        method,
        ambient_c,
        grouped_circuits: whole("grouped_circuits", 1, notes)?,
        length_m: req_number(v, "length_m", path)?,
        parallel: whole("parallel", 1, notes)?,
        pe_mm2: opt_number(v, "pe_mm2", path)?,
    })
}

/// Parses the JSON described in the module documentation.
pub fn parse(root: &Value) -> Result<Plant, String> {
    let (voltage, source) = supply(root.get("supply").ok_or("supply: missing")?)?;
    let mut network = Network::new(voltage, source);
    let Some(Value::Array(list)) = root.get("circuits") else {
        return Err("circuits: expected a list".into());
    };
    let mut ends: HashMap<String, (NodeId, usize)> = HashMap::new();
    let mut circuits = Vec::new();
    for (i, c) in list.iter().enumerate() {
        let path = format!("circuits[{i}]");
        let name = opt_text(c, "name", &path)?
            .ok_or(format!("{path}.name: missing"))?
            .to_owned();
        if ends.contains_key(&name) {
            return Err(format!("{path}.name: '{name}' is used twice"));
        }
        let (at, parent) = match opt_text(c, "from", &path)? {
            None => (network.root(), None),
            Some(from) => {
                let (node, index) = *ends
                    .get(from)
                    .ok_or(format!("{path}.from: '{from}' is not an earlier circuit"))?;
                (node, Some(index))
            }
        };
        let mut notes = Vec::new();
        let phases = opt_number(c, "phases", &path)?.unwrap_or(3.0);
        let supply_kind = match phases as u32 {
            3 => Supply::ThreePhase,
            1 | 2 => Supply::TwoWire,
            _ => return Err(format!("{path}.phases: 1 or 3")),
        };
        let power_factor = match opt_number(c, "power_factor", &path)? {
            Some(p) => p,
            None => {
                notes.push(format!(
                    "{name}: power factor not given, 1 assumed for the voltage drop"
                ));
                1.0
            }
        };
        let drop_limit = match opt_number(c, "drop_limit_percent", &path)? {
            Some(p) => p,
            None => {
                notes.push(format!("{name}: voltage drop limit not given, 5 % assumed"));
                5.0
            }
        };
        let dev = device(
            c.get("device").ok_or(format!("{path}.device: missing"))?,
            &format!("{path}.device"),
        )?;
        let cab = cable(
            c.get("cable").ok_or(format!("{path}.cable: missing"))?,
            &format!("{path}.cable"),
            &name,
            &mut notes,
        )?;
        let design = CircuitDesign {
            name: name.clone(),
            load_current_a: req_number(c, "load_a", &path)?,
            power_factor,
            supply: supply_kind,
            voltage: opt_number(c, "voltage", &path)?.unwrap_or(match supply_kind {
                Supply::ThreePhase => voltage,
                Supply::TwoWire => voltage / 3.0_f64.sqrt(),
            }),
            device: dev,
            cable: cab,
            drop_limit_percent: drop_limit,
            notes,
        };
        let branch = Branch::cable(
            design.cable.section_mm2,
            design.cable.length_m,
            design.cable.parallel,
            design.cable.insulation,
            design.cable.pe_mm2,
        )
        .ok_or(format!("{path}.cable: section and length must be positive"))?;
        let end = network.add_node(at, &name, branch);
        ends.insert(name, (end, circuits.len()));
        circuits.push(PlantCircuit {
            at,
            end,
            design,
            parent,
        });
    }
    Ok(Plant { network, circuits })
}

/// Evaluates every circuit of a parsed plant. Circuits fed from another
/// circuit also get a *Selectivity* row against the device upstream, and a
/// breaking-capacity failure is re-judged as back-up protection when
/// `pairs` (maker tables) proves the pair.
pub fn evaluate_all(
    plant: &Plant,
    tables: &TableSet,
    pairs: Option<&PairTable>,
) -> Vec<CircuitReport> {
    let mut reports: Vec<CircuitReport> = plant
        .circuits
        .iter()
        .map(|c| evaluate(&plant.network, c.at, &c.design, tables))
        .collect();
    for (i, c) in plant.circuits.iter().enumerate() {
        let Some(parent) = c.parent else { continue };
        let up = &plant.circuits[parent].design.device;
        let down = &c.design.device;
        let Some(ik) = reports[i].at_device.as_ref().map(|f| f.ik3_max_a) else {
            continue;
        };
        let (up_id, down_id) = (
            up.id.as_deref().unwrap_or(""),
            down.id.as_deref().unwrap_or(""),
        );

        // Back-up for a device that cannot break the current alone.
        let backed = backup(pairs, up_id, down_id, down.icu_ka, ik);
        if let Some(row) = reports[i]
            .rows
            .iter_mut()
            .find(|r| r.title == "Breaking capacity" && r.status == Status::Fail)
        {
            match backed {
                Backup::Covered { limit_ka } => {
                    row.status = Status::Pass;
                    row.text = format!(
                        "{}; backed up by {} up to {} kA (maker table)",
                        row.text, plant.circuits[parent].design.name, limit_ka
                    );
                }
                Backup::NotCovered { limit_ka } => {
                    row.text = match limit_ka {
                        Some(l) => format!(
                            "{}; the table allows back-up by {} only up to {} kA",
                            row.text, plant.circuits[parent].design.name, l
                        ),
                        None => format!(
                            "{}; no back-up proven (no maker table for this pair)",
                            row.text
                        ),
                    };
                }
                Backup::NotNeeded | Backup::Unknown => {}
            }
        }

        // Selectivity with the device above.
        let s = selectivity(
            pairs,
            up_id,
            down_id,
            down.icu_ka,
            up.characteristic.as_ref(),
            down.characteristic.as_ref(),
            ik,
        );
        let basis = match s.basis {
            Basis::MakerTable => "maker table",
            Basis::GenericBands => "guaranteed bands of the standards, conservative",
        };
        reports[i].rows.push(Row {
            rule: Rule::BreakingCapacity,
            title: "Selectivity",
            status: match s.status {
                SelStatus::Total => Status::Pass,
                SelStatus::Partial | SelStatus::None => Status::Warning,
                SelStatus::Unknown => Status::Unknown,
            },
            value: s.limit_a.map(|l| l / 1000.0),
            limit: Some(ik / 1000.0),
            unit: "kA",
            text: format!(
                "against {}: {} ({basis})",
                plant.circuits[parent].design.name, s.text
            ),
        });
    }
    reports
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> Value {
        json!({
            "supply": { "voltage": 400, "transformer": { "kva": 630, "uk_percent": 4, "pk_w": 6500 } },
            "circuits": [
                { "name": "Q1", "load_a": 120, "phases": 3,
                  "device": { "type": "mccb", "rated_a": 160, "icu_ka": 36,
                              "ir_a": 160, "tr_s": 12, "isd_x": 8, "tsd_s": 0.2, "ii_x": 12 },
                  "cable": { "section_mm2": 70, "length_m": 40, "insulation": "XLPE", "method": "C", "pe_mm2": 35 } },
                { "name": "M1", "from": "Q1", "load_a": 14, "power_factor": 0.85,
                  "device": { "type": "mcb", "curve": "C", "rated_a": 16, "icu_ka": 10 },
                  "cable": { "section_mm2": 2.5, "length_m": 20, "pe_mm2": 2.5 } }
            ]
        })
    }

    #[test]
    fn a_feeder_and_its_branch_circuit() {
        let plant = parse(&sample()).unwrap();
        assert_eq!(plant.circuits.len(), 2);
        let reports = evaluate_all(&plant, &TableSet::builtin(), None);
        let q1 = &reports[0];
        let m1 = &reports[1];
        // The branch sees less short-circuit current than the feeder start.
        assert!(
            m1.at_device.as_ref().unwrap().ik3_max_a < q1.at_device.as_ref().unwrap().ik3_max_a
        );
        // The 10 kA miniature breaker sits behind 40 m of 70 mm²: still below 10 kA?
        let bc = m1
            .rows
            .iter()
            .find(|r| r.title == "Breaking capacity")
            .unwrap();
        assert!(matches!(bc.status, Status::Pass | Status::Fail));
        // Assumptions from the defaults are carried into the report.
        assert!(m1.assumptions.iter().any(|a| a.contains("ambient")));
        // M1 hangs off Q1, so it is judged for selectivity against it.
        assert!(m1.rows.iter().any(|r| r.title == "Selectivity"));
        assert!(!q1.rows.iter().any(|r| r.title == "Selectivity"));
        assert!(q1.assumptions.iter().any(|a| a.contains("KT")));
    }

    #[test]
    fn errors_name_the_field() {
        let mut bad = sample();
        bad["circuits"][1]["from"] = json!("nowhere");
        assert!(parse(&bad).unwrap_err().contains("circuits[1].from"));
        let mut bad = sample();
        bad["circuits"][0]["device"]["type"] = json!("magic");
        assert!(parse(&bad).unwrap_err().contains("device.type"));
        let mut bad = sample();
        bad["circuits"][0]["cable"]["length_m"] = json!("long");
        assert!(parse(&bad).unwrap_err().contains("length_m"));
        let mut bad = sample();
        bad["circuits"][1]["name"] = json!("Q1");
        assert!(parse(&bad).unwrap_err().contains("used twice"));
        assert!(parse(&json!({})).unwrap_err().contains("supply"));
    }

    #[test]
    fn a_stated_supply_current_works_too() {
        let v = json!({
            "supply": { "short_circuit_ka": 10, "min_short_circuit_ka": 6 },
            "circuits": [ { "name": "A", "load_a": 10,
                "device": { "type": "mcb", "curve": "B", "rated_a": 10, "icu_ka": 6 },
                "cable": { "section_mm2": 1.5, "length_m": 10 } } ]
        });
        let plant = parse(&v).unwrap();
        let r = &evaluate_all(&plant, &TableSet::builtin(), None)[0];
        // No protective conductor given: the disconnection row is unknown, not a pass.
        let d = r.rows.iter().find(|x| x.title == "Disconnection").unwrap();
        assert_eq!(d.status, Status::Unknown);
    }
}
