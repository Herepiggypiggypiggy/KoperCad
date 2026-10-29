//! The crane picture's setup as the project keeps it: the size of the crane, where the bridge and
//! the trolley start and which travel axes drive them. The numbers live in the project info
//! properties (`crane3d.*`), so they travel with the file and undo like any other edit; a project
//! with none of them gets the defaults of the picture.

use novo_core::{Properties, Value};
use novo_view3d::crane::CraneLayout;

/// Everything about the picture that the project remembers.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct CraneSetup {
    pub layout: CraneLayout,
    pub bridge_m: f32,
    pub trolley_m: f32,
    pub bridge_axis: Option<String>,
    pub trolley_axis: Option<String>,
}

impl Default for CraneSetup {
    fn default() -> Self {
        let layout = CraneLayout::default();
        Self {
            // Near one end, so a travel drive that runs forward has room to go.
            bridge_m: layout.runway_m * 0.2,
            trolley_m: layout.span_m * 0.5,
            layout,
            bridge_axis: None,
            trolley_axis: None,
        }
    }
}

const PREFIX: &str = "crane3d.";
const NUMBERS: [&str; 6] = [
    "runway_m",
    "span_m",
    "rail_m",
    "hook_start_m",
    "bridge_m",
    "trolley_m",
];

fn key(name: &str) -> String {
    format!("{PREFIX}{name}")
}

fn number(props: &Properties, name: &str) -> Option<f32> {
    let v = match props.get(&key(name))? {
        Value::Float(f) => *f,
        Value::Int(i) => *i as f64,
        _ => return None,
    };
    v.is_finite().then_some(v as f32)
}

fn text(props: &Properties, name: &str) -> Option<String> {
    match props.get(&key(name))? {
        Value::Text(t) if !t.trim().is_empty() => Some(t.clone()),
        _ => None,
    }
}

impl CraneSetup {
    /// The setup the properties describe. A number that is missing, not a number or outside what
    /// the window allows falls back to the default, so a hand-edited file cannot break the picture.
    pub(super) fn from_properties(props: &Properties) -> Self {
        let d = Self::default();
        let pick = |name: &str, default: f32, lo: f32, hi: f32| {
            number(props, name)
                .filter(|v| (lo..=hi).contains(v))
                .unwrap_or(default)
        };
        let mut layout = d.layout;
        layout.runway_m = pick("runway_m", layout.runway_m, 4.0, 80.0);
        layout.span_m = pick("span_m", layout.span_m, 2.0, 40.0);
        layout.rail_m = pick("rail_m", layout.rail_m, 3.0, 30.0);
        layout.hook_start_m = pick("hook_start_m", layout.hook_start_m, 0.5, 25.0);
        layout.load_m = match props.get(&key("load")) {
            Some(Value::Bool(false)) => None,
            _ => layout.load_m,
        };
        Self {
            bridge_m: pick("bridge_m", layout.runway_m * 0.2, 0.0, layout.runway_m),
            trolley_m: pick("trolley_m", layout.span_m * 0.5, 0.0, layout.span_m),
            layout,
            bridge_axis: text(props, "bridge_axis"),
            trolley_axis: text(props, "trolley_axis"),
        }
    }

    /// Writes the setup into the properties, replacing what was there.
    pub(super) fn write_properties(&self, props: &mut Properties) {
        props.retain(|k, _| !k.starts_with(PREFIX));
        let l = &self.layout;
        let values = [
            l.runway_m,
            l.span_m,
            l.rail_m,
            l.hook_start_m,
            self.bridge_m,
            self.trolley_m,
        ];
        for (name, v) in NUMBERS.iter().zip(values) {
            props.insert(key(name), Value::Float(f64::from(v)));
        }
        props.insert(key("load"), Value::Bool(l.load_m.is_some()));
        for (name, axis) in [
            ("bridge_axis", &self.bridge_axis),
            ("trolley_axis", &self.trolley_axis),
        ] {
            if let Some(a) = axis {
                props.insert(key(name), Value::Text(a.clone()));
            }
        }
    }

    /// The same setup with the axes that the project no longer has taken out.
    pub(super) fn without_missing_axes(mut self, axes: &[String]) -> Self {
        for a in [&mut self.bridge_axis, &mut self.trolley_axis] {
            if a.as_ref().is_some_and(|t| !axes.contains(t)) {
                *a = None;
            }
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> CraneSetup {
        let mut s = CraneSetup::default();
        s.layout.runway_m = 22.0;
        s.layout.span_m = 12.5;
        s.layout.rail_m = 9.0;
        s.layout.load_m = None;
        s.bridge_m = 3.5;
        s.trolley_m = 1.25;
        s.bridge_axis = Some("=A1+FLD-M1".into());
        s.trolley_axis = Some("=A1+FLD-M2".into());
        s
    }

    #[test]
    fn a_setup_written_to_the_project_reads_back_the_same() {
        let mut props = Properties::new();
        sample().write_properties(&mut props);
        assert_eq!(CraneSetup::from_properties(&props), sample());
    }

    #[test]
    fn a_project_without_a_setup_gets_the_defaults() {
        let props = Properties::new();
        assert_eq!(CraneSetup::from_properties(&props), CraneSetup::default());
    }

    #[test]
    fn numbers_a_hand_edit_made_nonsense_fall_back_one_by_one() {
        let mut props = Properties::new();
        sample().write_properties(&mut props);
        props.insert(key("runway_m"), Value::Float(f64::NAN));
        props.insert(key("span_m"), Value::Text("wide".into()));
        props.insert(key("rail_m"), Value::Float(900.0));
        let s = CraneSetup::from_properties(&props);
        let d = CraneSetup::default();
        assert_eq!(s.layout.runway_m, d.layout.runway_m);
        assert_eq!(s.layout.span_m, d.layout.span_m);
        assert_eq!(s.layout.rail_m, d.layout.rail_m);
        assert_eq!(s.layout.hook_start_m, sample().layout.hook_start_m);
        assert_eq!(s.bridge_axis, sample().bridge_axis);
    }

    #[test]
    fn a_start_beyond_the_runway_falls_back_to_the_default_start() {
        let mut props = Properties::new();
        let mut s = sample();
        s.bridge_m = 40.0;
        s.write_properties(&mut props);
        let back = CraneSetup::from_properties(&props);
        assert_eq!(back.bridge_m, back.layout.runway_m * 0.2);
    }

    #[test]
    fn writing_replaces_the_old_setup_and_leaves_other_properties_alone() {
        let mut props = Properties::new();
        props.insert("tag.rule".into(), Value::Text("x".into()));
        sample().write_properties(&mut props);
        let mut plain = sample();
        plain.bridge_axis = None;
        plain.write_properties(&mut props);
        assert!(!props.contains_key(&key("bridge_axis")));
        assert!(props.contains_key("tag.rule"));
    }

    #[test]
    fn an_axis_the_project_no_longer_has_is_dropped() {
        let s = sample().without_missing_axes(&["=A1+FLD-M2".to_owned()]);
        assert_eq!(s.bridge_axis, None);
        assert_eq!(s.trolley_axis.as_deref(), Some("=A1+FLD-M2"));
    }
}
