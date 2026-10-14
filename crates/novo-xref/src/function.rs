//! What a placed symbol does, as far as cross references care.

use novo_core::SymbolKey;

/// Looks up the function type of a symbol (`contact.no`, `coil.relay`).
/// [`novo_symbols::Library`] implements it; tests and tools without a
/// library can use [`FromKey`].
pub trait Functions {
    fn function(&self, symbol: &SymbolKey) -> Option<&str>;
}

impl Functions for novo_symbols::Library {
    fn function(&self, symbol: &SymbolKey) -> Option<&str> {
        self.get(&symbol.0).map(|s| s.function.as_str())
    }
}

/// Knows no functions; every symbol is classified from its key alone.
#[derive(Debug, Clone, Copy, Default)]
pub struct FromKey;

impl Functions for FromKey {
    fn function(&self, _symbol: &SymbolKey) -> Option<&str> {
        None
    }
}

/// How a function shows up in a contact image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ContactKind {
    /// A main (power) contact, `1-2`, `3-4`, `5-6`.
    Power,
    /// Normally open auxiliary contact, `13-14`.
    NormallyOpen,
    /// Normally closed auxiliary contact, `21-22`.
    NormallyClosed,
    /// Changeover contact, `11-12-14`.
    Changeover,
    /// Anything else drawn for the device (a coil, a lamp, a second coil).
    Other,
}

impl ContactKind {
    /// Classifies a symbol by its function type, or by its library key
    /// (`iec/contacts/no`) when the function is unknown.
    pub fn classify(function: Option<&str>, key: &SymbolKey) -> ContactKind {
        if let Some(function) = function {
            return Self::from_function(function);
        }
        let name = key.0.rsplit('/').next().unwrap_or("");
        let in_contacts = key.0.contains("contact");
        match name {
            "main" | "main_1p" | "main_3p" | "main_4p" => ContactKind::Power,
            n if n.starts_with("main_") && in_contacts => ContactKind::Power,
            "no" => ContactKind::NormallyOpen,
            "nc" => ContactKind::NormallyClosed,
            "co" => ContactKind::Changeover,
            n if n.ends_with("_no") => ContactKind::NormallyOpen,
            n if n.ends_with("_nc") => ContactKind::NormallyClosed,
            n if n.ends_with("_co") => ContactKind::Changeover,
            _ => ContactKind::Other,
        }
    }

    fn from_function(function: &str) -> ContactKind {
        let mut parts = function.split('.');
        if parts.next() != Some("contact") {
            return ContactKind::Other;
        }
        match parts.next() {
            Some("main") => ContactKind::Power,
            Some("no") => ContactKind::NormallyOpen,
            Some("nc") => ContactKind::NormallyClosed,
            Some("co") => ContactKind::Changeover,
            _ => ContactKind::Other,
        }
    }

    /// True for every kind of contact.
    pub fn is_contact(self) -> bool {
        self != ContactKind::Other
    }

    /// EN 50005 function digits of an auxiliary contact, after the ordinal:
    /// normally closed `1 2`, normally open `3 4`, changeover `1 2 4`.
    pub(crate) fn function_digits(self) -> Option<&'static [char]> {
        match self {
            ContactKind::NormallyOpen => Some(&['3', '4']),
            ContactKind::NormallyClosed => Some(&['1', '2']),
            ContactKind::Changeover => Some(&['1', '2', '4']),
            _ => None,
        }
    }

    /// True when connection designations `pins` (sorted, as in
    /// [`Reference::pins`](crate::Reference::pins)) already say what kind
    /// of contact this is: `13-14` is normally open by EN 50005, `1-2 3-4`
    /// single digits are main contacts.
    pub fn numbered_as(self, pins: &str) -> bool {
        let terms: Vec<&str> = pins.split([' ', '-']).filter(|t| !t.is_empty()).collect();
        if terms.is_empty() {
            return false;
        }
        match self {
            ContactKind::Power => terms
                .iter()
                .all(|t| t.len() == 1 && t.as_bytes()[0].is_ascii_digit()),
            _ => self.function_digits().is_some_and(|digits| {
                terms.len() == digits.len()
                    && terms.iter().zip(digits).all(|(t, d)| {
                        let b = t.as_bytes();
                        b.len() == 2 && (b'1'..=b'9').contains(&b[0]) && b[1] == *d as u8
                    })
            }),
        }
    }

    /// Short label used in text contact images.
    pub fn label(self) -> &'static str {
        match self {
            ContactKind::Power => "main",
            ContactKind::NormallyOpen => "NO",
            ContactKind::NormallyClosed => "NC",
            ContactKind::Changeover => "CO",
            ContactKind::Other => "",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(s: &str) -> SymbolKey {
        SymbolKey(s.into())
    }

    #[test]
    fn classify_by_function() {
        let k = key("x");
        assert_eq!(
            ContactKind::classify(Some("contact.no.push"), &k),
            ContactKind::NormallyOpen
        );
        assert_eq!(
            ContactKind::classify(Some("contact.nc.thermal"), &k),
            ContactKind::NormallyClosed
        );
        assert_eq!(
            ContactKind::classify(Some("contact.main"), &k),
            ContactKind::Power
        );
        assert_eq!(
            ContactKind::classify(Some("contact.co"), &k),
            ContactKind::Changeover
        );
        assert_eq!(
            ContactKind::classify(Some("coil.relay"), &k),
            ContactKind::Other
        );
    }

    #[test]
    fn classify_by_key() {
        let c = |s| ContactKind::classify(None, &key(s));
        assert_eq!(c("iec/contacts/no"), ContactKind::NormallyOpen);
        assert_eq!(c("iec/contacts/nc"), ContactKind::NormallyClosed);
        assert_eq!(c("iec/contacts/thermal_nc"), ContactKind::NormallyClosed);
        assert_eq!(c("iec/contactors/main_3p"), ContactKind::Power);
        assert_eq!(c("iec/coils/relay"), ContactKind::Other);
    }

    #[test]
    fn numbering_tells_the_kind() {
        use ContactKind::*;
        assert!(NormallyOpen.numbered_as("13-14"));
        assert!(NormallyClosed.numbered_as("21-22"));
        assert!(Changeover.numbered_as("11-12-14"));
        assert!(Power.numbered_as("1-2 3-4 5-6"));
        assert!(!NormallyOpen.numbered_as("21-22"));
        assert!(!NormallyOpen.numbered_as("A-B"));
        assert!(!NormallyOpen.numbered_as(""));
        assert!(!Power.numbered_as("L1-T1"));
    }

    #[test]
    fn builtin_library_knows_contacts() {
        let lib = novo_symbols::Library::builtin().unwrap();
        let kind = |s: &str| {
            let k = key(s);
            ContactKind::classify(lib.function(&k), &k)
        };
        assert_eq!(kind("iec/contacts/no"), ContactKind::NormallyOpen);
        assert_eq!(kind("iec/contacts/nc"), ContactKind::NormallyClosed);
        assert_eq!(kind("iec/contactors/main_3p"), ContactKind::Power);
        assert_eq!(kind("iec/coils/relay"), ContactKind::Other);
    }
}
