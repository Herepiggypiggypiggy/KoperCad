//! The accuracy statement the app quotes (numbers from `docs/thermal-validation.md`).

use crate::result::{AccuracyStatement, BenchmarkError};

/// Statement for the flow core, with the benchmark errors measured by the
/// validation suite (lane CFD-G). Update with the table when a lane lands.
pub fn flow_core_statement() -> AccuracyStatement {
    let b = |name: &str, error: f64| BenchmarkError {
        name: name.into(),
        error,
    };
    AccuracyStatement {
        headline: "Laminar natural convection matches published benchmarks within 1% in heat \
                   transfer and about 3% in peak velocity; not yet validated for radiation."
            .into(),
        benchmarks: vec![
            b("Heated cavity Ra 1e3, Nusselt (de Vahl Davis)", 0.0055),
            b("Heated cavity Ra 1e4, Nusselt (de Vahl Davis)", 0.0062),
            b("Heated cavity Ra 1e5, Nusselt (de Vahl Davis)", 0.0015),
            b("Heated cavity Ra 1e6, Nusselt (de Vahl Davis)", 0.0029),
            b(
                "Heated cavity Ra 1e5, peak velocity (de Vahl Davis)",
                0.0306,
            ),
            b(
                "Heated cavity Ra 1e4, Richardson-extrapolated Nusselt",
                0.0031,
            ),
        ],
        limits: vec![
            "Radiation is not yet validated; vents and fans are checked against analytic operating points, turbulence on a channel and a heated wall only."
                .into(),
            "Wall functions are not valid for natural-convection boundary layers thinner than the first cell: resolve the wall cells there (coarse wall cells read about 52% low)."
                .into(),
            "Sealed-enclosure air rise lies up to 15% below the IEC 60890 estimate and the 2D engine (stratified air)."
                .into(),
            "Results for a cabinet carry the uncertainty of the loss data and boundary conditions."
                .into(),
            "Time integration is first order: heat-up curves need small steps.".into(),
        ],
    }
}
