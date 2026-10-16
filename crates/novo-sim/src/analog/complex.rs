//! A minimal complex number: the solver needs nothing more than field
//! arithmetic, so there is no dependency for it.

use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    pub const ZERO: Complex = Complex { re: 0.0, im: 0.0 };
    pub const ONE: Complex = Complex { re: 1.0, im: 0.0 };
    pub const J: Complex = Complex { re: 0.0, im: 1.0 };

    pub const fn new(re: f64, im: f64) -> Self {
        Complex { re, im }
    }

    pub const fn real(re: f64) -> Self {
        Complex { re, im: 0.0 }
    }

    /// `magnitude ∠ angle` with the angle in radians.
    pub fn polar(magnitude: f64, angle: f64) -> Self {
        Complex::new(magnitude * angle.cos(), magnitude * angle.sin())
    }

    pub fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }

    /// Angle in radians, -π to π.
    pub fn arg(self) -> f64 {
        self.im.atan2(self.re)
    }

    pub fn conj(self) -> Self {
        Complex::new(self.re, -self.im)
    }

    pub fn recip(self) -> Self {
        Complex::ONE / self
    }
}

impl Add for Complex {
    type Output = Complex;
    fn add(self, o: Complex) -> Complex {
        Complex::new(self.re + o.re, self.im + o.im)
    }
}

impl AddAssign for Complex {
    fn add_assign(&mut self, o: Complex) {
        *self = *self + o;
    }
}

impl Sub for Complex {
    type Output = Complex;
    fn sub(self, o: Complex) -> Complex {
        Complex::new(self.re - o.re, self.im - o.im)
    }
}

impl Mul for Complex {
    type Output = Complex;
    fn mul(self, o: Complex) -> Complex {
        Complex::new(
            self.re * o.re - self.im * o.im,
            self.re * o.im + self.im * o.re,
        )
    }
}

impl Mul<f64> for Complex {
    type Output = Complex;
    fn mul(self, k: f64) -> Complex {
        Complex::new(self.re * k, self.im * k)
    }
}

impl Div for Complex {
    type Output = Complex;
    fn div(self, o: Complex) -> Complex {
        let d = o.re * o.re + o.im * o.im;
        Complex::new(
            (self.re * o.re + self.im * o.im) / d,
            (self.im * o.re - self.re * o.im) / d,
        )
    }
}

impl Neg for Complex {
    type Output = Complex;
    fn neg(self) -> Complex {
        Complex::new(-self.re, -self.im)
    }
}

impl fmt::Display for Complex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.4}{:+.4}j", self.re, self.im)
    }
}
