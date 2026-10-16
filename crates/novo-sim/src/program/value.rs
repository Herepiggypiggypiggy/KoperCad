//! Data types and values of the IEC 61131-3 subset.

use std::fmt;

/// An elementary data type. Integer types wrap to their width when stored,
/// as they do on a real controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataType {
    Bool,
    Sint,
    Int,
    Dint,
    Lint,
    Usint,
    Uint,
    Udint,
    Ulint,
    Byte,
    Word,
    Dword,
    Real,
    Lreal,
    Time,
}

impl DataType {
    /// The type named `name` (case-insensitive), if it is elementary.
    pub fn parse(name: &str) -> Option<DataType> {
        Some(match name.to_ascii_uppercase().as_str() {
            "BOOL" => DataType::Bool,
            "SINT" => DataType::Sint,
            "INT" => DataType::Int,
            "DINT" => DataType::Dint,
            "LINT" => DataType::Lint,
            "USINT" => DataType::Usint,
            "UINT" => DataType::Uint,
            "UDINT" => DataType::Udint,
            "ULINT" => DataType::Ulint,
            "BYTE" => DataType::Byte,
            "WORD" => DataType::Word,
            "DWORD" => DataType::Dword,
            "REAL" => DataType::Real,
            "LREAL" => DataType::Lreal,
            "TIME" => DataType::Time,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            DataType::Bool => "BOOL",
            DataType::Sint => "SINT",
            DataType::Int => "INT",
            DataType::Dint => "DINT",
            DataType::Lint => "LINT",
            DataType::Usint => "USINT",
            DataType::Uint => "UINT",
            DataType::Udint => "UDINT",
            DataType::Ulint => "ULINT",
            DataType::Byte => "BYTE",
            DataType::Word => "WORD",
            DataType::Dword => "DWORD",
            DataType::Real => "REAL",
            DataType::Lreal => "LREAL",
            DataType::Time => "TIME",
        }
    }

    pub fn is_integer(self) -> bool {
        !matches!(
            self,
            DataType::Bool | DataType::Real | DataType::Lreal | DataType::Time
        )
    }

    pub fn is_real(self) -> bool {
        matches!(self, DataType::Real | DataType::Lreal)
    }

    /// The zero value of the type.
    pub fn zero(self) -> Value {
        match self {
            DataType::Bool => Value::Bool(false),
            DataType::Real | DataType::Lreal => Value::Real(0.0),
            DataType::Time => Value::Time(0),
            _ => Value::Int(0),
        }
    }

    /// Wraps an integer to the width of the type.
    fn wrap(self, v: i64) -> i64 {
        match self {
            DataType::Sint => i64::from(v as i8),
            DataType::Int => i64::from(v as i16),
            DataType::Dint => i64::from(v as i32),
            DataType::Usint | DataType::Byte => i64::from(v as u8),
            DataType::Uint | DataType::Word => i64::from(v as u16),
            DataType::Udint | DataType::Dword => i64::from(v as u32),
            _ => v,
        }
    }

    /// `v` converted to this type: numbers cross between integer and real
    /// (reals round to the nearest integer), integers wrap, a bool is 0/1.
    /// `None` when the conversion makes no sense (time to bool).
    pub fn coerce(self, v: Value) -> Option<Value> {
        Some(match (self, v) {
            (DataType::Bool, Value::Bool(b)) => Value::Bool(b),
            (DataType::Bool, Value::Int(i)) => Value::Bool(i != 0),
            (DataType::Time, Value::Time(t)) => Value::Time(t),
            (DataType::Time, Value::Int(i)) => Value::Time(i),
            (t, Value::Bool(b)) if t.is_integer() => Value::Int(i64::from(b)),
            (t, Value::Int(i)) if t.is_integer() => Value::Int(t.wrap(i)),
            (t, Value::Real(r)) if t.is_integer() => Value::Int(t.wrap(r.round() as i64)),
            (t, Value::Time(x)) if t.is_integer() => Value::Int(t.wrap(x)),
            (DataType::Real, Value::Real(r)) => Value::Real(f64::from(r as f32)),
            (DataType::Lreal, Value::Real(r)) => Value::Real(r),
            (t, Value::Int(i)) if t.is_real() => Value::Real(i as f64),
            (t, Value::Bool(b)) if t.is_real() => Value::Real(f64::from(u8::from(b))),
            _ => return None,
        })
    }
}

impl fmt::Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A run-time value. A `Time` is in microseconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Real(f64),
    Time(i64),
}

impl Value {
    pub fn as_bool(self) -> bool {
        match self {
            Value::Bool(b) => b,
            Value::Int(i) | Value::Time(i) => i != 0,
            Value::Real(r) => r != 0.0,
        }
    }

    pub fn as_int(self) -> i64 {
        match self {
            Value::Bool(b) => i64::from(b),
            Value::Int(i) | Value::Time(i) => i,
            Value::Real(r) => r.round() as i64,
        }
    }

    pub fn as_real(self) -> f64 {
        match self {
            Value::Bool(b) => f64::from(u8::from(b)),
            Value::Int(i) | Value::Time(i) => i as f64,
            Value::Real(r) => r,
        }
    }

    /// Microseconds, for a time (or a plain number taken as such).
    pub fn as_time(self) -> i64 {
        self.as_int()
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Bool(b) => f.write_str(if *b { "TRUE" } else { "FALSE" }),
            Value::Int(i) => write!(f, "{i}"),
            Value::Real(r) => write!(f, "{r}"),
            Value::Time(t) => f.write_str(&format_time(*t)),
        }
    }
}

/// `T#1h2m3s4ms` for a time in microseconds.
pub fn format_time(us: i64) -> String {
    if us == 0 {
        return "T#0s".into();
    }
    let sign = if us < 0 { "-" } else { "" };
    let mut rest = us.unsigned_abs();
    let mut out = format!("T#{sign}");
    for (unit, size) in [
        ("d", 86_400_000_000u64),
        ("h", 3_600_000_000),
        ("m", 60_000_000),
        ("s", 1_000_000),
        ("ms", 1_000),
        ("us", 1),
    ] {
        if rest >= size {
            out.push_str(&format!("{}{unit}", rest / size));
            rest %= size;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integers_wrap_to_their_width() {
        assert_eq!(
            DataType::Int.coerce(Value::Int(32768)),
            Some(Value::Int(-32768))
        );
        assert_eq!(DataType::Byte.coerce(Value::Int(257)), Some(Value::Int(1)));
        assert_eq!(DataType::Int.coerce(Value::Real(2.5)), Some(Value::Int(3)));
        assert_eq!(DataType::Bool.coerce(Value::Time(3)), None);
    }

    #[test]
    fn times_print_in_iec_form() {
        assert_eq!(format_time(5_000_000), "T#5s");
        assert_eq!(format_time(3_723_004_000), "T#1h2m3s4ms");
    }
}
