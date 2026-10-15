//! Compare two results: the difference map and a device table.

use crate::field::Scalar;

/// `b` minus `a` on `a`'s grid (B resampled when the grids differ). Cells
/// solid in either are solid in the result.
pub fn difference(a: &Scalar, b: &Scalar) -> Scalar {
    let same = a.grid == b.grid;
    let values = (0..a.grid.rows)
        .flat_map(|r| (0..a.grid.cols).map(move |c| (c, r)))
        .map(|(c, r)| {
            let av = a.at(c, r);
            let bv = if same {
                b.at(c, r)
            } else {
                let (x, y) = a.grid.centre(c, r);
                b.sample(x, y)
            };
            if av.is_nan() { f32::NAN } else { bv - av }
        })
        .collect();
    Scalar::new(a.grid, values)
}

/// A symmetric range around zero for the difference map, at least 1 K so a
/// near-identical pair does not blow noise up into colour.
pub fn symmetric_extent(diff: &Scalar) -> f32 {
    diff.extent()
        .map_or(1.0, |(lo, hi)| lo.abs().max(hi.abs()).max(1.0))
}

/// One device in both results.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceRow {
    pub id: u64,
    pub a: f64,
    pub b: f64,
}

impl DeviceRow {
    pub fn delta(&self) -> f64 {
        self.b - self.a
    }
}

/// Rows sorted by the size of the change, biggest first, then by id.
pub fn device_table(rows: &mut [DeviceRow]) {
    rows.sort_by(|x, y| {
        y.delta()
            .abs()
            .total_cmp(&x.delta().abs())
            .then(x.id.cmp(&y.id))
    });
}

/// One line of verdict: peak and count over the limit before and after.
pub fn verdict_line(peak_a: f64, peak_b: f64, over_a: usize, over_b: usize) -> String {
    format!("Peak {peak_a:.0} → {peak_b:.0} °C, devices over limit {over_a} → {over_b}.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::Grid;

    fn g(cols: usize) -> Grid {
        Grid {
            cols,
            rows: 1,
            cell_w: 10.0,
            cell_h: 10.0,
        }
    }

    #[test]
    fn difference_is_b_minus_a_and_keeps_solids() {
        let a = Scalar::new(g(3), vec![50.0, f32::NAN, 40.0]);
        let b = Scalar::new(g(3), vec![45.0, 30.0, 41.0]);
        let d = difference(&a, &b);
        assert_eq!(d.at(0, 0), -5.0);
        assert!(d.at(1, 0).is_nan());
        assert_eq!(d.at(2, 0), 1.0);
        assert_eq!(symmetric_extent(&d), 5.0);
    }

    #[test]
    fn different_grids_are_resampled() {
        let a = Scalar::new(g(2), vec![10.0, 10.0]);
        let b = Scalar::new(g(4), vec![12.0; 4]);
        let d = difference(&a, &b);
        assert!(d.values.iter().all(|v| (*v - 2.0).abs() < 1e-5));
    }

    #[test]
    fn extent_has_a_floor() {
        let a = Scalar::new(g(2), vec![10.0, 10.0]);
        assert_eq!(symmetric_extent(&difference(&a, &a)), 1.0);
    }

    #[test]
    fn table_sorts_by_change() {
        let mut rows = vec![
            DeviceRow {
                id: 1,
                a: 50.0,
                b: 50.5,
            },
            DeviceRow {
                id: 2,
                a: 78.0,
                b: 71.0,
            },
            DeviceRow {
                id: 3,
                a: 49.0,
                b: 50.0,
            },
        ];
        device_table(&mut rows);
        assert_eq!(rows.iter().map(|r| r.id).collect::<Vec<_>>(), vec![2, 3, 1]);
        assert_eq!(
            verdict_line(78.0, 71.0, 1, 0),
            "Peak 78 → 71 °C, devices over limit 1 → 0."
        );
    }
}
