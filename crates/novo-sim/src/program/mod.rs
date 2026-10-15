//! A PLC program interpreter: IEC 61131-3 Structured Text and a ladder
//! notation, scanned against an I/O image.
//!
//! The subset is the one that drives control cabinets: BOOL, the integer
//! types, REAL and TIME; `IF`, `CASE`, `FOR`, `WHILE`, `REPEAT`; the standard
//! timers (`TON`, `TOF`, `TP`), counters (`CTU`, `CTD`, `CTUD`), edge
//! detectors (`R_TRIG`, `F_TRIG`) and bistables (`RS`, `SR`); the usual
//! numeric functions; direct addresses (`%IX0.0`, `%QX0.1`, `%QW2`, `%MX1.0`).
//! Not in the subset yet: arrays, structures, strings, user-defined
//! functions and function blocks.

mod address;
mod ast;
mod blocks;
mod lexer;
mod model;
mod ops;
mod parser;
mod value;
mod vm;

pub use address::{Address, Size, Space};
pub use ast::{VarClass, VarDecl, VarType};
pub use blocks::FbKind;
pub use lexer::ParseError;
pub use model::{Element, ElementKind, Program, Rung};
pub use value::{DataType, Value, format_time};
pub use vm::{Runtime, RuntimeError};
