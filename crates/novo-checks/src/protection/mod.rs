//! Protection coordination: trip curves, selectivity, back-up protection
//! and short-circuit current rating of a panel (Lane B of "Protection and
//! cable engineering").
//!
//! Pure functions and plain data, like the calculation library: no
//! project, no I/O. Reading the model into these inputs is a separate
//! adapter step.
//!
//! | Module | Answers |
//! | --- | --- |
//! | [`curves`] | How fast does this device trip at this current? The one time-current model shared with the simulation |

pub mod curves;

pub use curves::{
    Band, Clears, CurveError, Family, FuseEnergy, Instantaneous, Lsi, McbCurve, RelayClass, Sample,
    TripCurve,
};
