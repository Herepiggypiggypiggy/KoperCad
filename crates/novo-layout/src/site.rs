//! Where a device belongs: inside the cabinet, on its door, or out in the
//! field (a motor, a valve, a limit switch). Only the first takes room on
//! the mounting plate, so `Place all`, the enclosure sizing and the heat
//! model must know which is which.
//!
//! The answer is read, in this order, from
//! 1. the device's own `mounting_site` property (`plate`, `door`, `field`),
//! 2. the same key on one of its parts, or a part with `mounting` =
//!    `door_cutout` (the door),
//! 3. the letter code of its tag: motors (`M`) and actuators (`Y`) are in
//!    the field. Pilot devices (`H`, `S`, `P`) go to the door by the
//!    surface rules, not here (see [`crate::surface_rules`]).
//!
//! Anything else is the plate. The letter codes are a default, so a motor
//! that really sits in the cabinet (a fan on the plate, a test rig) is set
//! to `plate` on the device and then counts as any other device.

use novo_core::{Device, Part, Project, Value};

use crate::drill::MOUNTING_KEY;

/// Device (and part) property that names the site.
pub const SITE_KEY: &str = "mounting_site";

/// Letter codes whose devices are outside the cabinet unless told otherwise.
const FIELD_CODES: [&str; 2] = ["M", "Y"];

/// Where a device is mounted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Site {
    /// On the mounting plate inside the cabinet.
    Plate,
    /// In a cut-out of the cabinet door.
    Door,
    /// Outside the cabinet, on the machine.
    Field,
}

impl Site {
    pub fn as_str(self) -> &'static str {
        match self {
            Site::Plate => "plate",
            Site::Door => "door",
            Site::Field => "field",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Site::Plate => "In the cabinet",
            Site::Door => "On the door",
            Site::Field => "In the field",
        }
    }

    pub fn parse(text: &str) -> Option<Site> {
        match text.trim().to_ascii_lowercase().as_str() {
            "plate" | "cabinet" | "panel" | "inside" => Some(Site::Plate),
            "door" => Some(Site::Door),
            "field" | "outside" | "machine" => Some(Site::Field),
            _ => None,
        }
    }
}

/// Where the answer came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteSource {
    /// Said on the device or on one of its parts.
    Stated,
    /// Taken from the letter code of the tag.
    LetterCode,
    /// Nothing says otherwise.
    Default,
}

fn site_property(properties: &novo_core::Properties) -> Option<Site> {
    match properties.get(SITE_KEY) {
        Some(Value::Text(t)) => Site::parse(t),
        _ => None,
    }
}

/// What the device or its parts say about where it is mounted; `None` when
/// they say nothing readable.
pub fn stated_site(project: &Project, device: &Device) -> Option<Site> {
    if let Some(site) = site_property(&device.properties) {
        return Some(site);
    }
    for part in device.parts.iter().filter_map(|p| project.get::<Part>(*p)) {
        if let Some(site) = site_property(&part.properties) {
            return Some(site);
        }
        if let Some(Value::Text(t)) = part.properties.get(MOUNTING_KEY)
            && t.trim().eq_ignore_ascii_case("door_cutout")
        {
            return Some(Site::Door);
        }
    }
    None
}

/// The site of a device and how that was decided. A device on the door
/// because of its letter code (`H`, `S`, `P`) reads as [`Site::Plate`] here:
/// that is the surface rules' business.
pub fn site_of(project: &Project, device: &Device) -> (Site, SiteSource) {
    if let Some(site) = stated_site(project, device) {
        return (site, SiteSource::Stated);
    }
    let code = device.tag.letter_code().to_ascii_uppercase();
    if FIELD_CODES.contains(&code.as_str()) {
        return (Site::Field, SiteSource::LetterCode);
    }
    (Site::Plate, SiteSource::Default)
}

/// True when the device is outside the cabinet and takes no room on a plate.
pub fn is_field(project: &Project, device: &Device) -> bool {
    site_of(project, device).0 == Site::Field
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_read_in_either_spelling() {
        assert_eq!(Site::parse(" Field "), Some(Site::Field));
        assert_eq!(Site::parse("cabinet"), Some(Site::Plate));
        assert_eq!(Site::parse("DOOR"), Some(Site::Door));
        assert_eq!(Site::parse("roof"), None);
    }
}
