//! Contact kinds and how many of each a device has.
//!
//! Classification follows the WS-10 cross references (`novo-xref`, PR #31):
//! a symbol's library key names its function (`iec/contacts/no`,
//! `iec/contactors/main_3p`). Once that crate is on main the two should share
//! one classifier, ideally from the symbol library's `function` field.

use novo_core::SymbolKey;

/// What kind of contact a placed symbol is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ContactKind {
    /// A main (power) contact set, `1-2 3-4 5-6`.
    Power,
    /// Normally open auxiliary contact, `13-14`.
    NormallyOpen,
    /// Normally closed auxiliary contact, `21-22`.
    NormallyClosed,
    /// Changeover contact, `11-12-14`.
    Changeover,
    /// Anything else (a coil, a lamp, a motor).
    Other,
}

impl ContactKind {
    /// Classifies a symbol by the last segment of its library key.
    pub fn of_key(key: &SymbolKey) -> ContactKind {
        let name = key.0.rsplit('/').next().unwrap_or("");
        match name {
            "main" | "main_1p" | "main_3p" | "main_4p" => ContactKind::Power,
            n if n.starts_with("main_") && key.0.contains("contact") => ContactKind::Power,
            "no" => ContactKind::NormallyOpen,
            "nc" => ContactKind::NormallyClosed,
            "co" => ContactKind::Changeover,
            n if n.ends_with("_no") => ContactKind::NormallyOpen,
            n if n.ends_with("_nc") => ContactKind::NormallyClosed,
            n if n.ends_with("_co") => ContactKind::Changeover,
            _ => ContactKind::Other,
        }
    }

    /// Plain name used in messages.
    pub fn label(self) -> &'static str {
        match self {
            ContactKind::Power => "main",
            ContactKind::NormallyOpen => "NO",
            ContactKind::NormallyClosed => "NC",
            ContactKind::Changeover => "changeover",
            ContactKind::Other => "other",
        }
    }
}

/// The device property that says how many contacts of a kind the device
/// has, until parts (WS-07) carry it: `contacts.main`, `contacts.no`,
/// `contacts.nc`, `contacts.co`, as integers. The same keys WS-10 reads.
pub fn contact_limit_property(kind: ContactKind) -> Option<&'static str> {
    match kind {
        ContactKind::Power => Some("contacts.main"),
        ContactKind::NormallyOpen => Some("contacts.no"),
        ContactKind::NormallyClosed => Some("contacts.nc"),
        ContactKind::Changeover => Some("contacts.co"),
        ContactKind::Other => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_library_keys() {
        let kind = |k: &str| ContactKind::of_key(&SymbolKey(k.into()));
        assert_eq!(kind("iec/contacts/no"), ContactKind::NormallyOpen);
        assert_eq!(kind("iec/contacts/nc"), ContactKind::NormallyClosed);
        assert_eq!(kind("iec/contacts/thermal_nc"), ContactKind::NormallyClosed);
        assert_eq!(kind("iec/contacts/co"), ContactKind::Changeover);
        assert_eq!(kind("iec/contactors/main_3p"), ContactKind::Power);
        assert_eq!(
            kind("iec/operators/pushbutton_no"),
            ContactKind::NormallyOpen
        );
        assert_eq!(kind("iec/coils/relay"), ContactKind::Other);
        assert_eq!(kind("iec/motors/motor_3ph"), ContactKind::Other);
    }
}
