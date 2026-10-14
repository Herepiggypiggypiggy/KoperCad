//! IEC 81346 reference designations (device tags) such as `=A1+CAB1-K12`.
//!
//! A tag has three aspects, each optionally made of several levels:
//! function (`=`), location (`+`) and product (`-`). The product aspect is
//! what people usually call the device tag (`-K12`); its last level splits
//! into a letter code (`K`) and a number (`12`), which is what automatic
//! numbering and smart duplicate work with.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Why a tag string could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TagError {
    #[error("tag `{0}` must start with `=`, `+` or `-`")]
    MissingPrefix(String),
    #[error("tag `{0}` has an empty level")]
    EmptyLevel(String),
    #[error("tag `{tag}` contains `{ch}`, which is not allowed in a level")]
    InvalidChar { tag: String, ch: char },
}

/// One aspect of a reference designation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Aspect {
    /// `=`: what the object does (plant, function).
    Function,
    /// `+`: where the object is (cabinet, room).
    Location,
    /// `-`: what the object is (the device itself).
    Product,
}

impl Aspect {
    pub fn prefix(self) -> char {
        match self {
            Aspect::Function => '=',
            Aspect::Location => '+',
            Aspect::Product => '-',
        }
    }

    fn from_prefix(ch: char) -> Option<Aspect> {
        match ch {
            '=' => Some(Aspect::Function),
            '+' => Some(Aspect::Location),
            '-' => Some(Aspect::Product),
            _ => None,
        }
    }
}

/// An IEC 81346 reference designation. Stored and serialized in its canonical
/// text form (`=A1+CAB1-K1`), with aspects ordered function, location, product.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DeviceTag {
    pub function: Vec<String>,
    pub location: Vec<String>,
    pub product: Vec<String>,
}

fn valid_level_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '.' | '_')
}

impl DeviceTag {
    /// A tag with only a product level, e.g. `DeviceTag::product("K1")` is `-K1`.
    pub fn product(code: impl Into<String>) -> Self {
        DeviceTag {
            product: vec![code.into()],
            ..DeviceTag::default()
        }
    }

    /// Parses a tag such as `=A1+CAB1-K1`. Whitespace between levels is
    /// ignored, and an empty string gives an empty tag.
    pub fn parse(text: &str) -> Result<Self, TagError> {
        let mut tag = DeviceTag::default();
        let mut current: Option<(Aspect, String)> = None;
        let finish = |tag: &mut DeviceTag, level: Option<(Aspect, String)>| {
            if let Some((aspect, text_level)) = level {
                if text_level.is_empty() {
                    return Err(TagError::EmptyLevel(text.to_owned()));
                }
                tag.levels_mut(aspect).push(text_level);
            }
            Ok(())
        };
        for ch in text.chars() {
            if let Some(aspect) = Aspect::from_prefix(ch) {
                finish(&mut tag, current.take())?;
                current = Some((aspect, String::new()));
            } else if ch.is_whitespace() {
                continue;
            } else if let Some((_, level)) = current.as_mut() {
                if !valid_level_char(ch) {
                    return Err(TagError::InvalidChar {
                        tag: text.to_owned(),
                        ch,
                    });
                }
                level.push(ch);
            } else {
                return Err(TagError::MissingPrefix(text.to_owned()));
            }
        }
        finish(&mut tag, current)?;
        Ok(tag)
    }

    /// Adds a function level, builder style: `DeviceTag::product("K1").with_function("A1")`.
    pub fn with_function(mut self, level: impl Into<String>) -> Self {
        self.function.push(level.into());
        self
    }

    /// Adds a location level, builder style.
    pub fn with_location(mut self, level: impl Into<String>) -> Self {
        self.location.push(level.into());
        self
    }

    pub fn levels(&self, aspect: Aspect) -> &[String] {
        match aspect {
            Aspect::Function => &self.function,
            Aspect::Location => &self.location,
            Aspect::Product => &self.product,
        }
    }

    pub fn levels_mut(&mut self, aspect: Aspect) -> &mut Vec<String> {
        match aspect {
            Aspect::Function => &mut self.function,
            Aspect::Location => &mut self.location,
            Aspect::Product => &mut self.product,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.function.is_empty() && self.location.is_empty() && self.product.is_empty()
    }

    /// The last product level, e.g. `K12` for `=A1-K12`.
    pub fn device_code(&self) -> Option<&str> {
        self.product.last().map(String::as_str)
    }

    /// The letter code of the device, e.g. `K` for `-K12`. Empty when the tag
    /// has no product level or the level starts with a digit.
    pub fn letter_code(&self) -> &str {
        self.device_code().map_or("", |code| split_code(code).0)
    }

    /// The device number, e.g. `12` for `-K12`.
    pub fn number(&self) -> Option<u32> {
        self.device_code().and_then(|code| split_code(code).1)
    }

    /// The same tag with the device number replaced, keeping letter code and
    /// any suffix: `-K12.1` with number 3 becomes `-K3.1`. A tag without a
    /// product level gets one made of just the number.
    pub fn with_number(&self, number: u32) -> Self {
        let mut tag = self.clone();
        match tag.product.last_mut() {
            Some(code) => {
                let (letters, _, suffix) = split_code(code);
                *code = format!("{letters}{number}{suffix}");
            }
            None => tag.product.push(number.to_string()),
        }
        tag
    }

    /// The tag without its last product level: the structure the device sits
    /// in. `=A1+CAB1-K12` gives `=A1+CAB1`.
    pub fn parent(&self) -> DeviceTag {
        let mut tag = self.clone();
        tag.product.pop();
        tag
    }

    /// Whether two tags share function and location, i.e. they number in the
    /// same sequence.
    pub fn same_structure(&self, other: &DeviceTag) -> bool {
        self.function == other.function && self.location == other.location
    }
}

/// Splits a product level into letters, number and suffix: `K12.1` gives
/// `("K", Some(12), ".1")`.
pub fn split_code(code: &str) -> (&str, Option<u32>, &str) {
    let letters_end = code
        .char_indices()
        .find(|(_, ch)| ch.is_ascii_digit())
        .map_or(code.len(), |(i, _)| i);
    let (letters, rest) = code.split_at(letters_end);
    let digits_end = rest
        .char_indices()
        .find(|(_, ch)| !ch.is_ascii_digit())
        .map_or(rest.len(), |(i, _)| i);
    let (digits, suffix) = rest.split_at(digits_end);
    (letters, digits.parse().ok(), suffix)
}

impl fmt::Display for DeviceTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for aspect in [Aspect::Function, Aspect::Location, Aspect::Product] {
            for level in self.levels(aspect) {
                write!(f, "{}{}", aspect.prefix(), level)?;
            }
        }
        Ok(())
    }
}

impl FromStr for DeviceTag {
    type Err = TagError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        DeviceTag::parse(s)
    }
}

impl TryFrom<String> for DeviceTag {
    type Error = TagError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        DeviceTag::parse(&value)
    }
}

impl From<DeviceTag> for String {
    fn from(tag: DeviceTag) -> String {
        tag.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_aspects() {
        let tag = DeviceTag::parse("=PLANT+CAB1-K12").unwrap();
        assert_eq!(tag.function, ["PLANT"]);
        assert_eq!(tag.location, ["CAB1"]);
        assert_eq!(tag.product, ["K12"]);
        assert_eq!(tag.to_string(), "=PLANT+CAB1-K12");
    }

    #[test]
    fn canonicalizes_order_and_whitespace() {
        let tag: DeviceTag = "-Q1 +CAB1 =A1".parse().unwrap();
        assert_eq!(tag.to_string(), "=A1+CAB1-Q1");
    }

    #[test]
    fn keeps_multiple_levels() {
        let tag = DeviceTag::parse("=A1=B2-A3-K1").unwrap();
        assert_eq!(tag.function, ["A1", "B2"]);
        assert_eq!(tag.product, ["A3", "K1"]);
        assert_eq!(tag.to_string(), "=A1=B2-A3-K1");
        assert_eq!(tag.parent().to_string(), "=A1=B2-A3");
    }

    #[test]
    fn rejects_bad_tags() {
        assert!(matches!(
            DeviceTag::parse("K1"),
            Err(TagError::MissingPrefix(_))
        ));
        assert!(matches!(
            DeviceTag::parse("=-K1"),
            Err(TagError::EmptyLevel(_))
        ));
        assert!(matches!(DeviceTag::parse("-K1"), Ok(DeviceTag { .. })));
        assert!(matches!(
            DeviceTag::parse("-K#1"),
            Err(TagError::InvalidChar { ch: '#', .. })
        ));
        assert_eq!(DeviceTag::parse("").unwrap(), DeviceTag::default());
    }

    #[test]
    fn splits_letter_code_and_number() {
        assert_eq!(split_code("K12"), ("K", Some(12), ""));
        assert_eq!(split_code("Q1.1"), ("Q", Some(1), ".1"));
        assert_eq!(split_code("XD"), ("XD", None, ""));
        assert_eq!(split_code("12"), ("", Some(12), ""));
        let tag = DeviceTag::parse("+CAB1-K12.1").unwrap();
        assert_eq!(tag.letter_code(), "K");
        assert_eq!(tag.number(), Some(12));
        assert_eq!(tag.with_number(3).to_string(), "+CAB1-K3.1");
    }

    #[test]
    fn serializes_as_text() {
        let tag = DeviceTag::product("M1").with_function("A1");
        let json = serde_json::to_string(&tag).unwrap();
        assert_eq!(json, "\"=A1-M1\"");
        let back: DeviceTag = serde_json::from_str(&json).unwrap();
        assert_eq!(back, tag);
        assert!(serde_json::from_str::<DeviceTag>("\"K1\"").is_err());
    }
}
