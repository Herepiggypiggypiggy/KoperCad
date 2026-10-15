//! A small CAEX (IEC 62424) element tree and its serialiser. CAEX is the XML
//! schema AutomationML (IEC 62714) is written in.

use std::fmt::Write as _;

/// One XML element with attributes, children and text.
#[derive(Debug, Clone, Default)]
pub(crate) struct El {
    pub name: &'static str,
    pub attrs: Vec<(&'static str, String)>,
    pub children: Vec<El>,
    pub text: Option<String>,
}

impl El {
    pub fn new(name: &'static str) -> El {
        El {
            name,
            ..El::default()
        }
    }

    pub fn attr(mut self, key: &'static str, value: impl Into<String>) -> El {
        self.attrs.push((key, value.into()));
        self
    }

    pub fn child(mut self, child: El) -> El {
        self.children.push(child);
        self
    }

    pub fn text(mut self, text: impl Into<String>) -> El {
        self.text = Some(text.into());
        self
    }

    pub fn push(&mut self, child: El) {
        self.children.push(child);
    }

    /// `<Attribute Name=.. AttributeDataType=..><Value>..</Value></Attribute>`
    pub fn attribute(name: impl Into<String>, data_type: &str, value: impl Into<String>) -> El {
        El::new("Attribute")
            .attr("Name", name)
            .attr("AttributeDataType", data_type)
            .child(El::new("Value").text(value))
    }
}

/// XML text with `& < > "` and control characters made safe.
pub(crate) fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c if (c as u32) < 0x20 && !matches!(c, '\n' | '\t') => {}
            c => out.push(c),
        }
    }
    out
}

/// Position of a child in the CAEX sequence of its parent (stable within
/// one name, so the order the writer pushed them in is kept).
fn rank(name: &str) -> u8 {
    match name {
        "Description" => 0,
        "Version" => 1,
        "Attribute" => 6,
        "ExternalInterface" => 7,
        "InternalElement" => 8,
        "SupportedRoleClass" => 9,
        "RoleRequirements" => 10,
        "InternalLink" => 11,
        _ => 5,
    }
}

pub(crate) fn write(el: &El, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    let _ = write!(out, "{pad}<{}", el.name);
    for (k, v) in &el.attrs {
        let _ = write!(out, " {k}=\"{}\"", escape(v));
    }
    match (&el.text, el.children.is_empty()) {
        (None, true) => out.push_str("/>\n"),
        (Some(t), true) => {
            let _ = writeln!(out, ">{}</{}>", escape(t), el.name);
        }
        (text, false) => {
            out.push_str(">\n");
            if let Some(t) = text {
                let _ = writeln!(out, "{pad}  {}", escape(t));
            }
            // CAEX fixes the order of an element's children.
            let mut children: Vec<&El> = el.children.iter().collect();
            children.sort_by_key(|c| rank(c.name));
            for c in children {
                write(c, depth + 1, out);
            }
            let _ = writeln!(out, "{pad}</{}>", el.name);
        }
    }
}

/// A stable GUID for a record: the same project always gives the same ids, so
/// a second export of an unchanged project is byte for byte the first.
pub(crate) fn guid(kind: &str, key: &str) -> String {
    fn fnv(seed: u64, text: &str) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325 ^ seed;
        for b in text.bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        h
    }
    let text = format!("kopercad:{kind}:{key}");
    let (a, b) = (fnv(1, &text), fnv(0x9e37_79b9_7f4a_7c15, &text));
    // Version 5 style: version nibble 5, variant bits 10.
    let hi = (a & !0xF000) | 0x5000;
    let lo = (b & 0x3FFF_FFFF_FFFF_FFFF) | 0x8000_0000_0000_0000;
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        (hi >> 32) as u32,
        ((hi >> 16) & 0xFFFF) as u16,
        (hi & 0xFFFF) as u16,
        (lo >> 48) as u16,
        lo & 0xFFFF_FFFF_FFFF
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guids_are_stable_and_distinct() {
        let a = guid("device", "1");
        assert_eq!(a, guid("device", "1"));
        assert_ne!(a, guid("device", "2"));
        assert_eq!(a.len(), 36);
        assert_eq!(&a[14..15], "5");
    }

    #[test]
    fn text_is_escaped() {
        let el = El::new("Value").text("a<b & \"c\"");
        let mut s = String::new();
        write(&el, 0, &mut s);
        assert_eq!(s, "<Value>a&lt;b &amp; &quot;c&quot;</Value>\n");
    }
}
