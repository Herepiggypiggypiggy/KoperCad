//! The one-sentence verdict at the top of the 3D Heat panel: is the cabinet
//! fine, tight or too hot, and which device decides it. Built from the
//! solver's per-device margins (`heat_spots::rows`), so it says exactly what
//! the hot-spot list says, shorter. It is a model result and is labelled so.

use novo_cfd::CfdSnapshot;
use novo_thermal::Status;

use super::heat_spots::{self, Row};

/// How the verdict reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tone {
    Fine,
    Tight,
    TooHot,
    /// Nothing to judge: no losses, or no limits known.
    Unknown,
}

/// The sentence in three parts so the device name can be a button.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Verdict {
    pub tone: Tone,
    pub lead: String,
    /// The device that decides it: id and tag.
    pub device: Option<(u64, String)>,
    pub tail: String,
}

impl Verdict {
    /// The whole sentence as plain text.
    #[cfg(test)]
    pub(crate) fn text(&self) -> String {
        match &self.device {
            Some((_, tag)) => format!("{}{tag}{}", self.lead, self.tail),
            None => format!("{}{}", self.lead, self.tail),
        }
    }
}

fn plural(n: usize) -> String {
    if n == 1 {
        String::from("1 device has")
    } else {
        format!("{n} devices have")
    }
}

/// The verdict for a result; `None` when the result has no device data at
/// all (a stub or synthetic field).
pub(crate) fn verdict(snap: &CfdSnapshot, tag_of: &dyn Fn(u64) -> String) -> Option<Verdict> {
    if snap.devices.is_empty() {
        return None;
    }
    let rows = heat_spots::rows(snap, tag_of, usize::MAX);
    let Some(first) = rows.first() else {
        return Some(Verdict {
            tone: Tone::Unknown,
            lead: String::from(
                "No device in this result gives off heat, so there is nothing to judge.",
            ),
            device: None,
            tail: String::new(),
        });
    };
    let no_limit = rows.iter().filter(|r| r.status == Status::NoLimit).count();
    let no_limit_note = if no_limit > 0 {
        format!(" {} no limit data.", plural(no_limit))
    } else {
        String::new()
    };
    let device = Some((first.id, first.tag.clone()));
    Some(match first.status {
        Status::Over => {
            let more = rows.iter().filter(|r| r.status == Status::Over).count() - 1;
            let more = if more > 0 {
                format!(" ({more} more over)")
            } else {
                String::new()
            };
            Verdict {
                tone: Tone::TooHot,
                lead: String::from("Too hot: "),
                device,
                tail: format!(
                    " reaches {:.0} °C, {:.0} K over its limit{more}.{no_limit_note}",
                    first.surface_c,
                    -first.margin_k.unwrap_or(0.0)
                ),
            }
        }
        Status::Marginal => Verdict {
            tone: Tone::Tight,
            lead: String::from("Tight: "),
            device,
            tail: marginal_tail(first, &no_limit_note),
        },
        Status::Ok => Verdict {
            tone: Tone::Fine,
            lead: String::from("Fine. Hottest "),
            device,
            tail: format!(
                " {:.0} °C, {:.0} K under its limit.{no_limit_note}",
                first.surface_c,
                first.margin_k.unwrap_or(0.0)
            ),
        },
        Status::NoLimit => Verdict {
            tone: Tone::Unknown,
            lead: String::from("No limits are known, so this cannot be judged. Hottest "),
            device,
            tail: format!(" {:.0} °C.", first.surface_c),
        },
    })
}

fn marginal_tail(r: &Row, no_limit_note: &str) -> String {
    let under = r.margin_k.unwrap_or(0.0);
    match r.margin_pessimistic_k {
        Some(p) if p < 0.0 => format!(
            " is {under:.0} K under its limit at {:.0} °C, but the model's uncertainty band reaches {:.0} K over.{no_limit_note}",
            r.surface_c, -p
        ),
        _ => format!(
            " is {under:.0} K under its limit at {:.0} °C.{no_limit_note}",
            r.surface_c
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::heat_3d_tests::snapshot;
    use novo_layout::Rect;
    use novo_thermal::DeviceResult;

    fn dev(
        id: u64,
        surface: f64,
        status: Status,
        margin: Option<f64>,
        pess: Option<f64>,
    ) -> DeviceResult {
        DeviceResult {
            id,
            footprint: Rect {
                x: 0.0,
                y: 0.0,
                w: 10.0,
                h: 10.0,
            },
            loss_w: 5.0,
            surface_c: surface,
            air_c: surface - 5.0,
            h_w_m2k: 8.0,
            plate_share: 0.2,
            surface_band_c: (surface, surface),
            air_band_c: (surface, surface),
            rated_max_ambient_c: None,
            rated_max_surface_c: None,
            margin_k: margin,
            margin_pessimistic_k: pess,
            status,
        }
    }

    fn tag(id: u64) -> String {
        format!("-F{id}")
    }

    fn with(devices: Vec<DeviceResult>) -> Option<Verdict> {
        let mut s = snapshot();
        s.devices = devices;
        verdict(&s, &tag)
    }

    #[test]
    fn a_cabinet_inside_its_limits_is_fine_and_names_the_hottest() {
        let v = with(vec![
            dev(1, 49.0, Status::Ok, Some(20.0), Some(15.0)),
            dev(2, 57.0, Status::Ok, Some(8.0), Some(5.0)),
        ])
        .unwrap();
        assert_eq!(v.tone, Tone::Fine);
        assert_eq!(v.device, Some((2, "-F2".into())));
        assert_eq!(v.text(), "Fine. Hottest -F2 57 °C, 8 K under its limit.");
    }

    #[test]
    fn a_device_over_its_limit_decides_and_the_others_are_counted() {
        let v = with(vec![
            dev(1, 90.0, Status::Over, Some(-3.0), Some(-6.0)),
            dev(2, 80.0, Status::Over, Some(-1.0), Some(-2.0)),
            dev(3, 50.0, Status::Ok, Some(30.0), Some(25.0)),
        ])
        .unwrap();
        assert_eq!(v.tone, Tone::TooHot);
        assert_eq!(
            v.text(),
            "Too hot: -F1 reaches 90 °C, 3 K over its limit (1 more over)."
        );
    }

    #[test]
    fn a_tight_device_says_where_the_uncertainty_band_goes() {
        let v = with(vec![dev(4, 70.0, Status::Marginal, Some(2.0), Some(-3.0))]).unwrap();
        assert_eq!(v.tone, Tone::Tight);
        assert!(v.text().contains("2 K under its limit at 70 °C"));
        assert!(v.text().contains("reaches 3 K over"));
    }

    #[test]
    fn devices_without_limits_are_mentioned_not_judged() {
        let v = with(vec![
            dev(1, 50.0, Status::Ok, Some(20.0), Some(20.0)),
            dev(2, 95.0, Status::NoLimit, None, None),
        ])
        .unwrap();
        assert!(
            v.text().ends_with("1 device has no limit data."),
            "{}",
            v.text()
        );
        let v = with(vec![dev(2, 95.0, Status::NoLimit, None, None)]).unwrap();
        assert_eq!(v.tone, Tone::Unknown);
        assert!(v.text().contains("cannot be judged") && v.text().contains("95 °C"));
    }

    #[test]
    fn no_devices_means_no_verdict_and_no_losses_means_nothing_to_judge() {
        assert!(with(vec![]).is_none());
        let mut d = dev(1, 30.0, Status::Ok, Some(50.0), Some(50.0));
        d.loss_w = 0.0;
        assert_eq!(with(vec![d]).unwrap().tone, Tone::Unknown);
    }
}
