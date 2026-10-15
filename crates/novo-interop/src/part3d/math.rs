//! Small vector and transform helpers (f64, millimetres).

pub type V3 = [f64; 3];

pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn scale(a: V3, k: f64) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
pub fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
pub fn len(a: V3) -> f64 {
    dot(a, a).sqrt()
}
pub fn dist(a: V3, b: V3) -> f64 {
    len(sub(a, b))
}
/// Unit vector, or `fallback` for a zero vector.
pub fn unit_or(a: V3, fallback: V3) -> V3 {
    let l = len(a);
    if l < 1e-12 {
        fallback
    } else {
        scale(a, 1.0 / l)
    }
}
pub fn unit(a: V3) -> V3 {
    unit_or(a, [0.0, 0.0, 1.0])
}
pub fn lerp(a: V3, b: V3, t: f64) -> V3 {
    add(scale(a, 1.0 - t), scale(b, t))
}

/// A right-handed coordinate frame (STEP `AXIS2_PLACEMENT_3D`).
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub o: V3,
    pub x: V3,
    pub y: V3,
    pub z: V3,
}

impl Frame {
    pub const IDENTITY: Frame = Frame {
        o: [0.0; 3],
        x: [1.0, 0.0, 0.0],
        y: [0.0, 1.0, 0.0],
        z: [0.0, 0.0, 1.0],
    };

    /// Frame from an origin, a z axis and an optional x reference
    /// direction (made perpendicular to z).
    pub fn from_axes(o: V3, z: V3, x_ref: Option<V3>) -> Frame {
        let z = unit(z);
        let seed = x_ref.unwrap_or(if z[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        });
        let mut x = sub(seed, scale(z, dot(seed, z)));
        if len(x) < 1e-9 {
            let alt = if z[0].abs() < 0.9 {
                [1.0, 0.0, 0.0]
            } else {
                [0.0, 1.0, 0.0]
            };
            x = sub(alt, scale(z, dot(alt, z)));
        }
        let x = unit(x);
        let y = cross(z, x);
        Frame { o, x, y, z }
    }

    pub fn to_local(&self, p: V3) -> V3 {
        let d = sub(p, self.o);
        [dot(d, self.x), dot(d, self.y), dot(d, self.z)]
    }

    pub fn from_local(&self, l: V3) -> V3 {
        add(
            self.o,
            add(
                scale(self.x, l[0]),
                add(scale(self.y, l[1]), scale(self.z, l[2])),
            ),
        )
    }
}

/// A rigid transform: `p' = m * p + t`, `m` stored as columns.
#[derive(Debug, Clone, Copy)]
pub struct Xf {
    pub cols: [V3; 3],
    pub t: V3,
}

impl Xf {
    pub const IDENTITY: Xf = Xf {
        cols: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        t: [0.0; 3],
    };

    /// Local frame coordinates to world: the frame's axes are the columns.
    pub fn from_frame(f: &Frame) -> Xf {
        Xf {
            cols: [f.x, f.y, f.z],
            t: f.o,
        }
    }

    pub fn apply(&self, p: V3) -> V3 {
        add(
            self.t,
            add(
                scale(self.cols[0], p[0]),
                add(scale(self.cols[1], p[1]), scale(self.cols[2], p[2])),
            ),
        )
    }

    /// `self` after `other`: `(self ∘ other)(p) = self(other(p))`.
    pub fn after(&self, other: &Xf) -> Xf {
        Xf {
            cols: [
                self.apply_dir(other.cols[0]),
                self.apply_dir(other.cols[1]),
                self.apply_dir(other.cols[2]),
            ],
            t: self.apply(other.t),
        }
    }

    pub fn apply_dir(&self, d: V3) -> V3 {
        add(
            scale(self.cols[0], d[0]),
            add(scale(self.cols[1], d[1]), scale(self.cols[2], d[2])),
        )
    }

    /// Inverse of a rigid transform (the columns are orthonormal).
    pub fn inverse(&self) -> Xf {
        let c = self.cols;
        let rows = [
            [c[0][0], c[1][0], c[2][0]],
            [c[0][1], c[1][1], c[2][1]],
            [c[0][2], c[1][2], c[2][2]],
        ];
        // The transpose of an orthonormal matrix is its inverse; the
        // rows of the original become the columns.
        let tr = Xf {
            cols: [rows[0], rows[1], rows[2]],
            t: [0.0; 3],
        };
        let t = tr.apply_dir(self.t);
        Xf {
            cols: tr.cols,
            t: scale(t, -1.0),
        }
    }
}
