//! Plain 2D fields on a regular grid, the shape the drawing code reads.
//! `NaN` marks a solid cell (device body, wall).

/// Grid geometry shared by scalar and vector fields.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grid {
    pub cols: usize,
    pub rows: usize,
    /// Cell size, mm.
    pub cell_w: f64,
    pub cell_h: f64,
}

impl Grid {
    pub fn width(&self) -> f64 {
        self.cols as f64 * self.cell_w
    }
    pub fn height(&self) -> f64 {
        self.rows as f64 * self.cell_h
    }
    /// Centre of a cell, mm.
    pub fn centre(&self, col: usize, row: usize) -> (f64, f64) {
        (
            (col as f64 + 0.5) * self.cell_w,
            (row as f64 + 0.5) * self.cell_h,
        )
    }
    /// Cell holding a point, clamped to the grid.
    pub fn cell_at(&self, x: f64, y: f64) -> (usize, usize) {
        let c = (x / self.cell_w).floor().clamp(0.0, (self.cols - 1) as f64) as usize;
        let r = (y / self.cell_h).floor().clamp(0.0, (self.rows - 1) as f64) as usize;
        (c, r)
    }
}

/// One value per cell, row by row from the top left.
#[derive(Debug, Clone, PartialEq)]
pub struct Scalar {
    pub grid: Grid,
    pub values: Vec<f32>,
}

/// Why a field could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldError {
    /// No columns or no rows.
    Empty,
    /// The number of values is not columns times rows.
    Length { expected: usize, found: usize },
}

impl std::fmt::Display for FieldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldError::Empty => write!(f, "the field has no cells"),
            FieldError::Length { expected, found } => {
                write!(f, "the field has {found} values, expected {expected}")
            }
        }
    }
}

impl std::error::Error for FieldError {}

fn check(grid: &Grid, lens: &[usize]) -> Result<(), FieldError> {
    if grid.cols == 0 || grid.rows == 0 {
        return Err(FieldError::Empty);
    }
    let expected = grid.cols * grid.rows;
    match lens.iter().find(|n| **n != expected) {
        Some(found) => Err(FieldError::Length {
            expected,
            found: *found,
        }),
        None => Ok(()),
    }
}

impl Scalar {
    /// A field from solver output: an error, not a panic, for a bad shape.
    pub fn try_new(grid: Grid, values: Vec<f32>) -> Result<Scalar, FieldError> {
        check(&grid, &[values.len()])?;
        Ok(Scalar { grid, values })
    }

    /// A field from data known to be well formed (tests, the synthetic
    /// scene). Panics on a bad shape; use [`Scalar::try_new`] for anything
    /// that comes from outside.
    pub fn new(grid: Grid, values: Vec<f32>) -> Scalar {
        Scalar::try_new(grid, values).expect("a well formed field")
    }

    pub fn at(&self, col: usize, row: usize) -> f32 {
        self.values[row * self.grid.cols + col]
    }

    /// Smallest and largest value, ignoring solids; `None` when there is none.
    pub fn extent(&self) -> Option<(f32, f32)> {
        let mut it = self.values.iter().copied().filter(|v| !v.is_nan());
        let first = it.next()?;
        Some(it.fold((first, first), |(lo, hi), v| (lo.min(v), hi.max(v))))
    }

    /// Bilinear value at a point in mm, clamped to the grid. Solid cells are
    /// left out of the blend (their weight goes to the air cells), so a
    /// device body never drags the map towards nothing; `NaN` only when all
    /// four neighbours are solid.
    pub fn sample(&self, x: f64, y: f64) -> f32 {
        let g = &self.grid;
        let fx = (x / g.cell_w - 0.5).clamp(0.0, (g.cols - 1) as f64);
        let fy = (y / g.cell_h - 0.5).clamp(0.0, (g.rows - 1) as f64);
        let (c0, r0) = (fx.floor() as usize, fy.floor() as usize);
        let (c1, r1) = ((c0 + 1).min(g.cols - 1), (r0 + 1).min(g.rows - 1));
        let (tx, ty) = ((fx - c0 as f64) as f32, (fy - r0 as f64) as f32);
        let taps = [
            (self.at(c0, r0), (1.0 - tx) * (1.0 - ty)),
            (self.at(c1, r0), tx * (1.0 - ty)),
            (self.at(c0, r1), (1.0 - tx) * ty),
            (self.at(c1, r1), tx * ty),
        ];
        let (mut sum, mut weight) = (0.0, 0.0);
        for (v, w) in taps {
            if !v.is_nan() {
                sum += v * w;
                weight += w;
            }
        }
        if weight > 1e-6 {
            sum / weight
        } else {
            f32::NAN
        }
    }

    /// Fill solid cells from the nearest air cell, so a map drawn under a
    /// device body does not show a hole. Cheap: repeated 4-neighbour passes.
    pub fn filled(&self) -> Scalar {
        let mut v = self.values.clone();
        let (cols, rows) = (self.grid.cols, self.grid.rows);
        for _ in 0..(cols + rows) {
            if !v.iter().any(|x| x.is_nan()) {
                break;
            }
            let prev = v.clone();
            for r in 0..rows {
                for c in 0..cols {
                    if !prev[r * cols + c].is_nan() {
                        continue;
                    }
                    let mut sum = 0.0;
                    let mut n = 0;
                    for (dc, dr) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                        let (cc, rr) = (c as i32 + dc, r as i32 + dr);
                        if cc < 0 || rr < 0 || cc >= cols as i32 || rr >= rows as i32 {
                            continue;
                        }
                        let x = prev[rr as usize * cols + cc as usize];
                        if !x.is_nan() {
                            sum += x;
                            n += 1;
                        }
                    }
                    if n > 0 {
                        v[r * cols + c] = sum / n as f32;
                    }
                }
            }
        }
        Scalar {
            grid: self.grid,
            values: v,
        }
    }

    /// Resample onto a finer or coarser grid of `cols` by `rows` (bilinear),
    /// for the texture the map is drawn from.
    pub fn resampled(&self, cols: usize, rows: usize) -> Scalar {
        let grid = Grid {
            cols,
            rows,
            cell_w: self.grid.width() / cols as f64,
            cell_h: self.grid.height() / rows as f64,
        };
        let mut values = Vec::with_capacity(cols * rows);
        for r in 0..rows {
            for c in 0..cols {
                let (x, y) = grid.centre(c, r);
                values.push(self.sample(x, y));
            }
        }
        Scalar { grid, values }
    }
}

/// A vector per cell (`y` down, so rising air has negative `vy`).
#[derive(Debug, Clone, PartialEq)]
pub struct Vector {
    pub grid: Grid,
    pub vx: Vec<f32>,
    pub vy: Vec<f32>,
}

impl Vector {
    /// A field from solver output: an error, not a panic, for a bad shape.
    pub fn try_new(grid: Grid, vx: Vec<f32>, vy: Vec<f32>) -> Result<Vector, FieldError> {
        check(&grid, &[vx.len(), vy.len()])?;
        Ok(Vector { grid, vx, vy })
    }

    /// A field from data known to be well formed; see [`Scalar::new`].
    pub fn new(grid: Grid, vx: Vec<f32>, vy: Vec<f32>) -> Vector {
        Vector::try_new(grid, vx, vy).expect("a well formed field")
    }

    pub fn max_speed(&self) -> f32 {
        self.vx
            .iter()
            .zip(&self.vy)
            .map(|(x, y)| x.hypot(*y))
            .filter(|s| !s.is_nan())
            .fold(0.0, f32::max)
    }

    /// Bilinear vector at a point in mm, clamped to the grid.
    pub fn sample(&self, x: f64, y: f64) -> (f32, f32) {
        let g = &self.grid;
        let fx = (x / g.cell_w - 0.5).clamp(0.0, (g.cols - 1) as f64);
        let fy = (y / g.cell_h - 0.5).clamp(0.0, (g.rows - 1) as f64);
        let (c0, r0) = (fx.floor() as usize, fy.floor() as usize);
        let (c1, r1) = ((c0 + 1).min(g.cols - 1), (r0 + 1).min(g.rows - 1));
        let (tx, ty) = ((fx - c0 as f64) as f32, (fy - r0 as f64) as f32);
        let lerp = |v: &[f32]| {
            let a = v[r0 * g.cols + c0] * (1.0 - tx) + v[r0 * g.cols + c1] * tx;
            let b = v[r1 * g.cols + c0] * (1.0 - tx) + v[r1 * g.cols + c1] * tx;
            a * (1.0 - ty) + b * ty
        };
        (lerp(&self.vx), lerp(&self.vy))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(cols: usize, rows: usize) -> Grid {
        Grid {
            cols,
            rows,
            cell_w: 10.0,
            cell_h: 10.0,
        }
    }

    #[test]
    fn sample_interpolates_between_centres() {
        let s = Scalar::new(grid(2, 1), vec![0.0, 10.0]);
        assert_eq!(s.sample(5.0, 5.0), 0.0);
        assert_eq!(s.sample(15.0, 5.0), 10.0);
        assert!((s.sample(10.0, 5.0) - 5.0).abs() < 1e-5);
        assert_eq!(s.sample(-50.0, 5.0), 0.0, "clamped");
    }

    #[test]
    fn solids_do_not_drag_the_blend() {
        let s = Scalar::new(grid(2, 1), vec![20.0, f32::NAN]);
        assert_eq!(s.sample(10.0, 5.0), 20.0);
        let all = Scalar::new(grid(1, 1), vec![f32::NAN]);
        assert!(all.sample(5.0, 5.0).is_nan());
    }

    #[test]
    fn extent_skips_solids() {
        let s = Scalar::new(grid(3, 1), vec![f32::NAN, 4.0, 9.0]);
        assert_eq!(s.extent(), Some((4.0, 9.0)));
        assert_eq!(Scalar::new(grid(1, 1), vec![f32::NAN]).extent(), None);
    }

    #[test]
    fn filled_removes_every_hole() {
        let mut v = vec![30.0; 25];
        v[12] = f32::NAN;
        v[13] = f32::NAN;
        let f = Scalar::new(grid(5, 5), v).filled();
        assert!(f.values.iter().all(|x| *x == 30.0));
    }

    #[test]
    fn resample_keeps_the_extent() {
        let s = Scalar::new(grid(2, 2), vec![0.0, 10.0, 20.0, 30.0]);
        let r = s.resampled(8, 8);
        let (lo, hi) = r.extent().unwrap();
        assert!(lo >= 0.0 && hi <= 30.0 && hi > 25.0);
        assert!((r.grid.width() - 20.0).abs() < 1e-9);
    }

    #[test]
    fn bad_shapes_are_errors_not_panics() {
        assert_eq!(
            Scalar::try_new(grid(2, 2), vec![0.0; 3]),
            Err(FieldError::Length {
                expected: 4,
                found: 3
            })
        );
        assert_eq!(Scalar::try_new(grid(0, 2), vec![]), Err(FieldError::Empty));
        assert_eq!(
            Vector::try_new(grid(2, 1), vec![0.0; 2], vec![0.0; 5]),
            Err(FieldError::Length {
                expected: 2,
                found: 5
            })
        );
        assert!(Scalar::try_new(grid(2, 2), vec![0.0; 4]).is_ok());
        assert!(FieldError::Empty.to_string().contains("no cells"));
    }

    #[test]
    fn vector_sample_and_speed() {
        let v = Vector::new(grid(2, 1), vec![3.0, 3.0], vec![-4.0, -4.0]);
        assert_eq!(v.sample(10.0, 5.0), (3.0, -4.0));
        assert!((v.max_speed() - 5.0).abs() < 1e-6);
    }
}
