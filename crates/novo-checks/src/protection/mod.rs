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
//! | [`sccr`] | Short-circuit current rating of a panel (UL 508A Supplement SB method): weakest component, combinations, current limiting, transformers |
//! | [`selectivity`] | Is the upstream device selective with the downstream one, up to which current; back-up protection |

//!
//! ```
//! use novo_checks::protection::{Clears, McbCurve, TripCurve};
//!
//! // A C16 breaker is certain to have tripped within 0.1 s at 160 A and
//! // certain not to have tripped within 0.1 s at 80 A.
//! let c16 = TripCurve::mcb(16.0, McbCurve::C).unwrap();
//! assert_eq!(c16.clears(160.0, 0.1), Clears::Yes);
//! assert_eq!(c16.clears(80.0, 0.05), Clears::No);
//! ```

pub mod coordination;
pub mod curves;
pub mod devices;
pub mod sccr;
pub mod selectivity;

pub use curves::{
    Band, Clears, CurveError, Family, FuseEnergy, Instantaneous, Lsi, McbCurve, RelayClass, Sample,
    TripCurve,
};
