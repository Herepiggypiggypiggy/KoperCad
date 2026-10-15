//! Reads a BMEcat product catalog (XML) into catalog parts.
//!
//! BMEcat is the exchange format several makers use to publish article data
//! with ETIM or ECLASS classification. This reader covers the elements every
//! BMEcat 1.2 and 2005 file has: `PRODUCT` with `SUPPLIER_PID`,
//! `PRODUCT_DETAILS` (descriptions, manufacturer name and number, keywords)
//! and `PRODUCT_FEATURES` (classification system and class, then features
//! with name, value and unit). Elements it does not know are skipped.
//!
//! It was written from the public format description and tested on small
//! hand-made files, not on a maker's real file: expect to adjust the mapping
//! once a real file is at hand.

use novo_core::Value;
use quick_xml::Reader;
use quick_xml::events::Event;
use thiserror::Error;

use crate::catalog::CatalogPart;
use crate::csv::cell_value;
use crate::sheet::property_key;

/// Property keys for the classification codes found in a file.
pub const ETIM_CLASS_KEY: &str = "etim_class";
pub const ECLASS_CLASS_KEY: &str = "eclass_class";

#[derive(Debug, Error)]
pub enum BmecatError {
    #[error("the XML is not valid: {0}")]
    Xml(String),
    #[error("this XML is not a BMEcat catalog (no PRODUCT elements)")]
    NotBmecat,
}

/// Parses BMEcat text. `default_manufacturer` is used for articles that
/// name no manufacturer (the catalog's supplier name is tried first).
pub fn import_bmecat(
    xml: &str,
    default_manufacturer: &str,
) -> Result<Vec<CatalogPart>, BmecatError> {
    let mut reader = Reader::from_str(xml);

    let mut path: Vec<String> = Vec::new();
    let mut supplier = String::new();
    let mut parts = Vec::new();
    let mut cur: Option<Draft> = None;
    let mut system = String::new();
    let mut fname = String::new();
    let mut fvalues: Vec<String> = Vec::new();
    let mut funit = String::new();
    let mut buf = String::new();

    loop {
        match reader
            .read_event()
            .map_err(|e| BmecatError::Xml(e.to_string()))?
        {
            Event::Start(e) => {
                let name = local(e.name().as_ref());
                buf.clear();
                if name == "PRODUCT" && cur.is_none() {
                    cur = Some(Draft::default());
                }
                if name == "FEATURE" {
                    fname.clear();
                    fvalues.clear();
                    funit.clear();
                }
                if name == "PRODUCT_FEATURES" {
                    system.clear();
                }
                path.push(name);
            }
            Event::End(e) => {
                let name = local(e.name().as_ref());
                path.pop();
                let text = std::mem::take(&mut buf).trim().to_owned();
                if !text.is_empty() {
                    if let Some(d) = cur.as_mut() {
                        match name.as_str() {
                            "SUPPLIER_PID" => d.order_number = text,
                            "DESCRIPTION_SHORT" => d.short = text,
                            "DESCRIPTION_LONG" => d.long = text,
                            "MANUFACTURER_NAME" => d.manufacturer = text,
                            "MANUFACTURER_PID" => d.type_number = text,
                            "MANUFACTURER_TYPE_DESCR" if d.type_number.is_empty() => {
                                d.type_number = text
                            }
                            "KEYWORD" => d.keywords.push(text),
                            "REFERENCE_FEATURE_SYSTEM_NAME" => system = text,
                            "REFERENCE_FEATURE_GROUP_ID" => d.class(&system, &text),
                            "FNAME" => fname = text,
                            "FVALUE" => fvalues.push(text),
                            "FUNIT" => funit = text,
                            _ => {}
                        }
                    } else if name == "SUPPLIER_NAME" && path.iter().any(|p| p == "SUPPLIER") {
                        supplier = text;
                    }
                }
                match name.as_str() {
                    "FEATURE" => {
                        if let Some(d) = cur.as_mut() {
                            d.feature(&fname, &fvalues, &funit);
                        }
                    }
                    "PRODUCT" => {
                        if let Some(d) = cur.take()
                            && let Some(p) = d.finish(&supplier, default_manufacturer)
                        {
                            parts.push(p);
                        }
                    }
                    _ => {}
                }
            }
            Event::Text(t) => {
                buf.push_str(&t.decode().map_err(|e| BmecatError::Xml(e.to_string()))?);
            }
            Event::CData(t) => {
                buf.push_str(&String::from_utf8_lossy(&t.into_inner()));
            }
            Event::GeneralRef(r) => {
                if let Ok(Some(c)) = r.resolve_char_ref() {
                    buf.push(c);
                } else {
                    match r
                        .decode()
                        .map_err(|e| BmecatError::Xml(e.to_string()))?
                        .as_ref()
                    {
                        "amp" => buf.push('&'),
                        "lt" => buf.push('<'),
                        "gt" => buf.push('>'),
                        "quot" => buf.push('"'),
                        "apos" => buf.push('\''),
                        _ => {}
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if parts.is_empty() {
        return Err(BmecatError::NotBmecat);
    }
    Ok(parts)
}

/// An element name without namespace prefix.
fn local(name: &[u8]) -> String {
    let s = String::from_utf8_lossy(name);
    s.rsplit(':').next().unwrap_or("").to_owned()
}

#[derive(Default)]
struct Draft {
    order_number: String,
    manufacturer: String,
    type_number: String,
    short: String,
    long: String,
    keywords: Vec<String>,
    data: novo_core::Properties,
}

impl Draft {
    fn class(&mut self, system: &str, id: &str) {
        let s = system.to_ascii_uppercase();
        let key = if s.contains("ETIM") {
            ETIM_CLASS_KEY
        } else if s.contains("ECLASS") || s.contains("ECL@SS") {
            ECLASS_CLASS_KEY
        } else {
            return;
        };
        self.data
            .insert(key.into(), Value::Text(id.trim().to_owned()));
    }

    fn feature(&mut self, name: &str, values: &[String], unit: &str) {
        let Some(key) = property_key(name) else {
            return;
        };
        if values.is_empty() {
            return;
        }
        let joined = values.join(", ");
        let value = match (values.len(), unit.trim()) {
            (1, "") => cell_value(&joined),
            (_, "") => Value::Text(joined),
            (_, u) => Value::Text(format!("{joined} {u}")),
        };
        self.data.insert(key, value);
    }

    fn finish(self, supplier: &str, default_manufacturer: &str) -> Option<CatalogPart> {
        if self.order_number.trim().is_empty() {
            return None;
        }
        let manufacturer = [self.manufacturer.as_str(), supplier, default_manufacturer]
            .into_iter()
            .find(|m| !m.trim().is_empty())
            .unwrap_or_default()
            .to_owned();
        let description = if self.short.is_empty() {
            self.long
        } else {
            self.short
        };
        let mut part = CatalogPart::new(manufacturer, self.order_number.trim());
        part.type_number = self.type_number;
        part.description = description;
        part.keywords = self.keywords;
        part.data = self.data;
        Some(part)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<BMECAT version="2005" xmlns="http://www.bmecat.org/bmecat/2005">
 <HEADER><SUPPLIER><SUPPLIER_NAME>Example Maker</SUPPLIER_NAME></SUPPLIER></HEADER>
 <T_NEW_CATALOG>
  <PRODUCT>
   <SUPPLIER_PID>9000-0001</SUPPLIER_PID>
   <PRODUCT_DETAILS>
    <DESCRIPTION_SHORT>Feed-through terminal &amp; marker</DESCRIPTION_SHORT>
    <MANUFACTURER_PID>FT-2.5</MANUFACTURER_PID>
    <MANUFACTURER_NAME>Example Maker</MANUFACTURER_NAME>
    <KEYWORD>terminal</KEYWORD>
   </PRODUCT_DETAILS>
   <PRODUCT_FEATURES>
    <REFERENCE_FEATURE_SYSTEM_NAME>ETIM-8.0</REFERENCE_FEATURE_SYSTEM_NAME>
    <REFERENCE_FEATURE_GROUP_ID>EC000897</REFERENCE_FEATURE_GROUP_ID>
    <FEATURE><FNAME>Rated current</FNAME><FVALUE>24</FVALUE><FUNIT>A</FUNIT></FEATURE>
    <FEATURE><FNAME>Poles</FNAME><FVALUE>2</FVALUE></FEATURE>
   </PRODUCT_FEATURES>
  </PRODUCT>
  <PRODUCT>
   <SUPPLIER_PID>9000-0002</SUPPLIER_PID>
   <PRODUCT_DETAILS><DESCRIPTION_LONG>End plate</DESCRIPTION_LONG></PRODUCT_DETAILS>
  </PRODUCT>
  <PRODUCT><PRODUCT_DETAILS><DESCRIPTION_SHORT>no id</DESCRIPTION_SHORT></PRODUCT_DETAILS></PRODUCT>
 </T_NEW_CATALOG>
</BMECAT>"#;

    #[test]
    fn reads_articles_with_class_and_features() {
        let parts = import_bmecat(SAMPLE, "Fallback").unwrap();
        assert_eq!(parts.len(), 2, "the article without an id is skipped");
        let a = &parts[0];
        assert_eq!(a.manufacturer, "Example Maker");
        assert_eq!(a.order_number, "9000-0001");
        assert_eq!(a.type_number, "FT-2.5");
        assert_eq!(a.description, "Feed-through terminal & marker");
        assert_eq!(a.keywords, vec!["terminal"]);
        assert_eq!(
            a.data.get(ETIM_CLASS_KEY),
            Some(&Value::Text("EC000897".into()))
        );
        assert_eq!(
            a.data.get("rated_current"),
            Some(&Value::Text("24 A".into()))
        );
        assert_eq!(a.data.get("poles"), Some(&Value::Int(2)));
    }

    #[test]
    fn manufacturer_falls_back_to_the_supplier_then_the_default() {
        let parts = import_bmecat(SAMPLE, "Fallback").unwrap();
        assert_eq!(parts[1].manufacturer, "Example Maker");
        assert_eq!(parts[1].description, "End plate");
        let bare = "<BMECAT><T_NEW_CATALOG><PRODUCT><SUPPLIER_PID>1</SUPPLIER_PID></PRODUCT></T_NEW_CATALOG></BMECAT>";
        assert_eq!(
            import_bmecat(bare, "Fallback").unwrap()[0].manufacturer,
            "Fallback"
        );
    }

    #[test]
    fn rejects_other_xml() {
        assert!(matches!(
            import_bmecat("<a><b/></a>", "x"),
            Err(BmecatError::NotBmecat)
        ));
        assert!(matches!(
            import_bmecat("<a><b></a>", "x"),
            Err(BmecatError::Xml(_))
        ));
    }
}
