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
//! | [`devices`] | A protective device: curve, breaking capacity, let-through, maker tables (user data) |
//! | [`coordination`] | A chain of devices from supply to load: breaking capacity, back-up, selectivity, disconnection time, conductor withstand |
//! | [`selectivity`] | Is the upstream device selective with the downstream one, up to which current; back-up protection |

pub mod coordination;
pub mod curves;
pub mod devices;
pub mod selectivity;

pub use curves::{
    Band, Clears, CurveError, Family, FuseEnergy, Instantaneous, Lsi, McbCurve, RelayClass, Sample,
    TripCurve,
};
