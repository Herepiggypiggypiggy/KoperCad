//! Heat view engine: everything the pictures of a thermal result need that is
//! not drawing. Pure data in, plain geometry out, so the Cabinet view, the 3D
//! view and the tests all use the same code.
//!
//! Frame: millimetres, `y` down, like `novo-thermal`. Fields are sampled at
//! cell centres. Spec: `docs/design/thermal-visuals.md`.

pub mod compare;
pub mod contour;
pub mod field;
pub mod hotspot;
pub mod range;
pub mod streamline;
pub mod synthetic;

pub use field::{FieldError, Scalar, Vector};
