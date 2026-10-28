//! Input validation errors shared by the drive and machine engines.

use std::fmt;

/// A model input that cannot describe a physical machine. Engines return this instead of
/// dividing by zero or producing NaN.
#[derive(Debug, Clone, PartialEq)]
pub enum InputError {
    /// The value is NaN or infinite.
    NotFinite { field: &'static str },
    /// The value must be greater than zero.
    NotPositive { field: &'static str, value: f64 },
    /// The value must not be negative.
    Negative { field: &'static str, value: f64 },
    /// The value is outside `min..=max`.
    OutOfRange {
        field: &'static str,
        value: f64,
        min: f64,
        max: f64,
    },
    /// Pole counts are even numbers of at least 2.
    BadPoles { poles: u32 },
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFinite { field } => write!(f, "{field} must be a finite number"),
            Self::NotPositive { field, value } => {
                write!(f, "{field} must be greater than zero (got {value})")
            }
            Self::Negative { field, value } => {
                write!(f, "{field} must not be negative (got {value})")
            }
            Self::OutOfRange {
                field,
                value,
                min,
                max,
            } => write!(f, "{field} must be between {min} and {max} (got {value})"),
            Self::BadPoles { poles } => {
                write!(
                    f,
                    "pole count must be an even number of at least 2 (got {poles})"
                )
            }
        }
    }
}

impl std::error::Error for InputError {}

pub(crate) fn finite(field: &'static str, value: f64) -> Result<f64, InputError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(InputError::NotFinite { field })
    }
}

pub(crate) fn positive(field: &'static str, value: f64) -> Result<f64, InputError> {
    finite(field, value)?;
    if value > 0.0 {
        Ok(value)
    } else {
        Err(InputError::NotPositive { field, value })
    }
}

pub(crate) fn non_negative(field: &'static str, value: f64) -> Result<f64, InputError> {
    finite(field, value)?;
    if value >= 0.0 {
        Ok(value)
    } else {
        Err(InputError::Negative { field, value })
    }
}

/// A fraction in `(0, 1]`, for example an efficiency.
pub(crate) fn fraction(field: &'static str, value: f64) -> Result<f64, InputError> {
    finite(field, value)?;
    if value > 0.0 && value <= 1.0 {
        Ok(value)
    } else {
        Err(InputError::OutOfRange {
            field,
            value,
            min: 0.0,
            max: 1.0,
        })
    }
}

pub(crate) fn poles(poles: u32) -> Result<u32, InputError> {
    if poles >= 2 && poles.is_multiple_of(2) {
        Ok(poles)
    } else {
        Err(InputError::BadPoles { poles })
    }
}
