//! Size of a part from its STEP file (ISO 10303-21 text).
//!
//! Reads the model's `CARTESIAN_POINT` entities and returns the box that
//! holds them, in millimetres. This is a quick estimate, not geometry
//! processing: it ignores placement transforms of assemblies and treats all
//! points alike, so the result is [`StepBox::approximate`] and the axes are
//! those of the model, which may not be width, height and depth as a user
//! sees the part. Binary STEP and zipped STEP are not read.

use thiserror::Error;

use crate::catalog::Dimensions;

/// Extents of the points of a STEP model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepBox {
    /// Size along the model's X, Y and Z axes, in mm.
    pub size_mm: [f64; 3],
    pub points: usize,
}

impl StepBox {
    /// Always true for now: see the module comment.
    pub fn approximate(&self) -> bool {
        true
    }

    /// As catalog dimensions, with the smallest extent as depth and the
    /// largest as height: a guess that suits most DIN rail parts (a
    /// terminal is tall and thin, a contactor tall and wide).
    pub fn as_dimensions(&self) -> Dimensions {
        let mut s = self.size_mm;
        s.sort_by(|a, b| a.total_cmp(b));
        Dimensions {
            width_mm: s[1],
            height_mm: s[2],
            depth_mm: s[0],
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StepError {
    #[error("this is not a STEP file (no ISO-10303-21 header)")]
    NotStep,
    #[error("the STEP file has no points to measure")]
    NoPoints,
    #[error("the STEP file uses a length unit KoperCAD does not know")]
    UnknownUnit,
}

/// Measures STEP text.
pub fn step_box(text: &str) -> Result<StepBox, StepError> {
    if !text
        .trim_start_matches('\u{feff}')
        .trim_start()
        .starts_with("ISO-10303-21")
    {
        return Err(StepError::NotStep);
    }
    let scale = unit_scale_mm(text)?;
    let upper = text.to_ascii_uppercase();
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    let mut points = 0usize;
    let mut from = 0;
    while let Some(i) = upper[from..].find("CARTESIAN_POINT") {
        let start = from + i + "CARTESIAN_POINT".len();
        from = start;
        // CARTESIAN_POINT('name',(x,y,z)): take the inner parenthesis.
        let Some(open) = text[start..].find("(") else {
            break;
        };
        let rest = &text[start + open + 1..];
        let Some(inner) = rest.find('(') else { break };
        let Some(close) = rest[inner..].find(')') else {
            break;
        };
        let list = &rest[inner + 1..inner + close];
        let nums: Vec<f64> = list
            .split(',')
            .filter_map(|n| n.trim().parse::<f64>().ok())
            .collect();
        if nums.len() == 3 {
            for k in 0..3 {
                min[k] = min[k].min(nums[k]);
                max[k] = max[k].max(nums[k]);
            }
            points += 1;
        }
    }
    if points == 0 {
        return Err(StepError::NoPoints);
    }
    Ok(StepBox {
        size_mm: [
            (max[0] - min[0]) * scale,
            (max[1] - min[1]) * scale,
            (max[2] - min[2]) * scale,
        ],
        points,
    })
}

/// Millimetres per model unit, from the first length unit declared.
fn unit_scale_mm(text: &str) -> Result<f64, StepError> {
    let upper = text.to_ascii_uppercase();
    let Some(i) = upper.find("LENGTH_UNIT") else {
        // No unit declared: STEP's default for CAD exports is millimetres.
        return Ok(1.0);
    };
    // The whole entity: from the previous `;` to the next one.
    let from = upper[..i].rfind(';').map_or(0, |p| p + 1);
    let to = upper[i..].find(';').map_or(upper.len(), |p| i + p);
    let window = &upper[from..to];
    if window.contains(".MILLI.") && window.contains(".METRE.") {
        Ok(1.0)
    } else if window.contains(".CENTI.") && window.contains(".METRE.") {
        Ok(10.0)
    } else if window.contains("$,.METRE.") {
        Ok(1000.0)
    } else if window.contains(".METRE.") {
        Err(StepError::UnknownUnit)
    } else if window.contains("INCH") {
        Ok(25.4)
    } else {
        Err(StepError::UnknownUnit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(unit: &str, points: &[&str]) -> String {
        let mut s = String::from("ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\n");
        s.push_str(unit);
        s.push('\n');
        for (i, p) in points.iter().enumerate() {
            s.push_str(&format!("#{}=CARTESIAN_POINT('',({p}));\n", i + 1));
        }
        s.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
        s
    }

    const MM: &str = "#90=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));";

    #[test]
    fn measures_the_box_in_mm() {
        let t = file(
            MM,
            &["0.,0.,0.", "5.2,0.,0.", "5.2,42.5,-1.E-01", "0.,0.,35.5"],
        );
        let b = step_box(&t).unwrap();
        assert_eq!(b.points, 4);
        assert!((b.size_mm[0] - 5.2).abs() < 1e-9);
        assert!((b.size_mm[1] - 42.5).abs() < 1e-9);
        assert!((b.size_mm[2] - 35.6).abs() < 1e-9);
        let d = b.as_dimensions();
        assert_eq!((d.depth_mm, d.width_mm, d.height_mm), (5.2, 35.6, 42.5));
    }

    #[test]
    fn converts_metres_and_inches() {
        let m = "#9=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT($,.METRE.));";
        let b = step_box(&file(m, &["0.,0.,0.", "0.05,0.,0."])).unwrap();
        assert!((b.size_mm[0] - 50.0).abs() < 1e-9);
        let i = "#9=(CONVERSION_BASED_UNIT('INCH',#8)LENGTH_UNIT()NAMED_UNIT(#7));";
        let b = step_box(&file(i, &["0.,0.,0.", "1.,0.,0."])).unwrap();
        assert!((b.size_mm[0] - 25.4).abs() < 1e-9);
    }

    #[test]
    fn points_may_span_lines_and_have_names() {
        let t = "ISO-10303-21;\nDATA;\n#1=CARTESIAN_POINT('Origin',\n (0.,0.,\n0.));\n#2 = CARTESIAN_POINT ( 'p' , ( 10. , 20. , 30. ) ) ;\nENDSEC;";
        let b = step_box(t).unwrap();
        assert_eq!(b.size_mm, [10.0, 20.0, 30.0]);
    }

    #[test]
    fn rejects_other_files() {
        assert_eq!(step_box("hello"), Err(StepError::NotStep));
        assert_eq!(
            step_box("ISO-10303-21;\nDATA;\nENDSEC;"),
            Err(StepError::NoPoints)
        );
        let odd = "ISO-10303-21;\n#9=(LENGTH_UNIT()SI_UNIT(.KILO.,.METRE.));\n#1=CARTESIAN_POINT('',(0.,0.,0.));";
        assert_eq!(step_box(odd), Err(StepError::UnknownUnit));
    }
}
