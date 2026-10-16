//! UL 508A rules: the short-circuit current rating of the panel (Supplement
//! SB, simplified), motor branch and overload sizing, conductor ampacity,
//! bonding and the panel nameplate.

use novo_core::AnyId;

use super::rules::*;
use super::shared;
use super::{Env, Sink, StandardSet, keys};

pub(super) fn run(e: &Env<'_>, s: &mut Sink) {
    let set = StandardSet::Ul508a;
    if !e.on(set) {
        return;
    }
    sccr_rules(e, s);
    shared::motor_branch_size(e, s, UL_MOTOR_BRANCH_SIZE);
    shared::overload_setting(e, s, UL_OVERLOAD_SETTING);
    shared::ampacity_rule(e, s, set, UL_CONDUCTOR_AMPACITY);
    shared::ground_size_awg(e, s, UL_GROUND_SIZE);
    shared::nameplate(e, s, UL_NAMEPLATE, true);
}

fn at(symbols: &[novo_core::SymbolId]) -> Vec<AnyId> {
    symbols.iter().map(|s| AnyId::Symbol(*s)).collect()
}

fn sccr_rules(e: &Env<'_>, s: &mut Sink) {
    let Some(report) = s.sccr.clone() else {
        return;
    };
    let kilo = super::report::kilo;
    // Every component has a rating.
    for c in &report.components {
        if c.rating_ka.is_some() {
            s.pass(UL_SCCR_COMPONENTS);
        } else {
            s.fail(
                UL_SCCR_COMPONENTS,
                at(&c.symbols),
                format!(
                    "{} has no short-circuit rating: state `{}` on the device or its part{}, or enter the unmarked value for type \"{}\" under `standards.ul508a.default_sccr_ka.{}`",
                    c.tag,
                    keys::SCCR_KA,
                    if matches!(c.kind, "breaker" | "fuse" | "disconnect" | "motor_protector") {
                        format!(" (a protective device may state `{}`)", keys::BREAKING_CAPACITY_KA)
                    } else {
                        String::new()
                    },
                    c.kind,
                    c.kind
                ),
            );
        }
    }

    // The calculated rating covers the available current.
    match (report.available_ka, report.panel_ka) {
        (None, _) => s.unknown(
            UL_SCCR_ADEQUATE,
            e.anchor(),
            format!(
                "State the available fault current at the supply (`{}`)",
                keys::SUPPLY_SHORT_CIRCUIT_KA
            ),
        ),
        (Some(available), Some(panel)) if report.complete => {
            if panel + 1e-9 >= available {
                s.pass(UL_SCCR_ADEQUATE);
            } else {
                let limiting: Vec<_> = report
                    .components
                    .iter()
                    .filter(|c| report.limiting.contains(&c.device))
                    .collect();
                let names = limiting
                    .iter()
                    .map(|c| c.tag.clone())
                    .collect::<Vec<_>>()
                    .join(", ");
                let marks: Vec<_> = limiting.iter().flat_map(|c| at(&c.symbols)).collect();
                s.fail(
                    UL_SCCR_ADEQUATE,
                    if marks.is_empty() { e.anchor() } else { marks },
                    format!(
                        "The panel's short-circuit current rating is {} kA (limited by {names}), below the {} kA available at the supply",
                        kilo(panel),
                        kilo(available)
                    ),
                );
            }
        }
        (Some(available), Some(upper)) if upper + 1e-9 < available => {
            let limiting: Vec<_> = report
                .components
                .iter()
                .filter(|c| report.limiting.contains(&c.device))
                .collect();
            let names = limiting
                .iter()
                .map(|c| c.tag.clone())
                .collect::<Vec<_>>()
                .join(", ");
            let marks: Vec<_> = limiting.iter().flat_map(|c| at(&c.symbols)).collect();
            s.fail(
                UL_SCCR_ADEQUATE,
                if marks.is_empty() { e.anchor() } else { marks },
                format!(
                    "{names} is rated {} kA, below the {} kA available at the supply, whatever the unrated components turn out to be",
                    kilo(upper),
                    kilo(available)
                ),
            );
        }
        (Some(_), _) => s.unknown(
            UL_SCCR_ADEQUATE,
            e.anchor(),
            "Some power components have no short-circuit rating, so the panel's rating is not determined".to_owned(),
        ),
    }

    // The marked rating exists and is not above the calculated one.
    if report.components.is_empty() {
        return;
    }
    match e.params.number(keys::PANEL_SCCR_KA) {
        None => s.fail(
            UL_SCCR_MARKING,
            e.anchor(),
            format!(
                "The panel's short-circuit current rating is not marked: state `{}`{}",
                keys::PANEL_SCCR_KA,
                match (report.complete, report.panel_ka) {
                    (true, Some(p)) => format!(" (calculated: {} kA)", kilo(p)),
                    _ => String::new(),
                }
            ),
        ),
        Some(marked) => match report.panel_ka {
            Some(calc) if report.complete && marked > calc + 1e-9 => s.fail(
                UL_SCCR_MARKING,
                e.anchor(),
                format!(
                    "The marked rating, {} kA, is above the calculated {} kA",
                    kilo(marked),
                    kilo(calc)
                ),
            ),
            Some(upper) if !report.complete && marked > upper + 1e-9 => s.fail(
                UL_SCCR_MARKING,
                e.anchor(),
                format!(
                    "The marked rating, {} kA, is above the {} kA of a rated component",
                    kilo(marked),
                    kilo(upper)
                ),
            ),
            _ => s.pass(UL_SCCR_MARKING),
        },
    }
}
