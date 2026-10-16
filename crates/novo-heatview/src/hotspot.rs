//! Hot spots: the strongest local maxima of a field, kept apart.

use crate::field::Scalar;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Peak {
    pub x: f64,
    pub y: f64,
    pub value: f32,
}

/// Up to `count` local maxima, hottest first, at least `min_separation` mm
/// apart. A peak must be above the field mean by `min_above_mean` and be the
/// largest value in its 8 neighbours (equal neighbours count as the same
/// peak: the first in reading order wins). Solid cells are ignored.
pub fn peaks(field: &Scalar, count: usize, min_separation: f64, min_above_mean: f32) -> Vec<Peak> {
    let g = field.grid;
    let valid: Vec<f32> = field
        .values
        .iter()
        .copied()
        .filter(|v| !v.is_nan())
        .collect();
    if valid.is_empty() || count == 0 {
        return Vec::new();
    }
    let mean = valid.iter().sum::<f32>() / valid.len() as f32;
    let mut cands = Vec::new();
    for r in 0..g.rows {
        for c in 0..g.cols {
            let v = field.at(c, r);
            if v.is_nan() || v - mean < min_above_mean {
                continue;
            }
            let mut is_peak = true;
            'n: for dr in -1i32..=1 {
                for dc in -1i32..=1 {
                    if (dr, dc) == (0, 0) {
                        continue;
                    }
                    let (cc, rr) = (c as i32 + dc, r as i32 + dr);
                    if cc < 0 || rr < 0 || cc >= g.cols as i32 || rr >= g.rows as i32 {
                        continue;
                    }
                    let n = field.at(cc as usize, rr as usize);
                    let earlier = (rr, cc) < (r as i32, c as i32);
                    if !n.is_nan() && (n > v || (n == v && earlier)) {
                        is_peak = false;
                        break 'n;
                    }
                }
            }
            if is_peak {
                let (x, y) = g.centre(c, r);
                cands.push(Peak { x, y, value: v });
            }
        }
    }
    cands.sort_by(|a, b| b.value.total_cmp(&a.value));
    let mut out: Vec<Peak> = Vec::new();
    for p in cands {
        if out
            .iter()
            .all(|q| (q.x - p.x).hypot(q.y - p.y) >= min_separation)
        {
            out.push(p);
            if out.len() == count {
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::Grid;

    fn two_bumps() -> Scalar {
        let grid = Grid {
            cols: 30,
            rows: 10,
            cell_w: 10.0,
            cell_h: 10.0,
        };
        let values = (0..300)
            .map(|i| {
                let (c, r) = ((i % 30) as f32, (i / 30) as f32);
                let a = 60.0 - ((c - 5.0).hypot(r - 5.0)) * 5.0;
                let b = 50.0 - ((c - 24.0).hypot(r - 4.0)) * 5.0;
                a.max(b).max(35.0)
            })
            .collect();
        Scalar::new(grid, values)
    }

    #[test]
    fn finds_both_bumps_hottest_first() {
        let p = peaks(&two_bumps(), 3, 60.0, 2.0);
        assert_eq!(p.len(), 2);
        assert!(p[0].value > p[1].value);
        assert!((p[0].x - 55.0).abs() < 1.0 && (p[0].y - 55.0).abs() < 1.0);
    }

    #[test]
    fn separation_and_count_limit_the_list() {
        assert_eq!(peaks(&two_bumps(), 1, 60.0, 2.0).len(), 1);
        assert_eq!(peaks(&two_bumps(), 3, 500.0, 2.0).len(), 1);
    }

    #[test]
    fn a_flat_field_has_no_peaks() {
        let grid = Grid {
            cols: 4,
            rows: 4,
            cell_w: 1.0,
            cell_h: 1.0,
        };
        assert!(peaks(&Scalar::new(grid, vec![40.0; 16]), 3, 1.0, 1.0).is_empty());
    }
}
