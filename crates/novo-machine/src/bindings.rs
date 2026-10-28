//! Bindings between machine sensors and schematic symbols (lanes L4, L5).

/// A virtual sensor on the machine, for example an upper hoist limit switch.
#[derive(Debug, Clone, PartialEq)]
pub struct SensorBinding {
    pub name: String,
    /// Device tag of the bound schematic symbol, for example `=A+B-S1`.
    pub device_tag: String,
    /// Axis position in millimetres at which the sensor actuates.
    pub actuate_at_mm: f64,
}
