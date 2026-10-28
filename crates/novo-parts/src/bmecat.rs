//! Reads a BMEcat product catalog (XML) into catalog parts.
//!
//! BMEcat is the exchange format several makers use to publish article data
//! with ETIM or ECLASS classification. Both versions in use are read: BMEcat
//! 2005 (`PRODUCT`, `SUPPLIER_PID`, `PRODUCT_DETAILS`, `PRODUCT_FEATURES`,
//! `PRODUCT_ORDER_DETAILS`, `PRODUCT_PRICE_DETAILS`) and BMEcat 1.2 (the same
//! elements under the names `ARTICLE`, `SUPPLIER_AID`, `ARTICLE_DETAILS`,
//! `ARTICLE_FEATURES`, `ARTICLE_ORDER_DETAILS`, `ARTICLE_PRICE_DETAILS`).
//! Elements it does not know are skipped.
//!
//! What a file yields:
//! - order number, manufacturer name and type number, descriptions (markup
//!   inside a description is dropped, the words are kept), keywords;
//! - the classification system and class, then every feature with name,
//!   value and unit. A feature written as an ETIM code (`EF000008`) is
//!   translated through the ETIM release the user supplied
//!   ([`crate::EtimModel`]); without it the code stays as it is and the
//!   notes say how many features could not be read;
//! - the list price with currency, the quantity it is for (`per 100`,
//!   `per 1000 m`) and the date it is valid from. Nothing is invented: a
//!   price with no currency, a zero or negative amount, or a quantity that
//!   is not a whole number is left out and listed in the notes.
//!
//! It was written from the public format descriptions and tested on
//! hand-made files in both versions, not on a maker's real file: expect to
//! adjust the mapping once a real file is at hand.

use std::collections::BTreeSet;

use novo_core::Value;
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use thiserror::Error;

use crate::catalog::CatalogPart;
use crate::csv::cell_value;
use crate::etim::EtimModel;
use crate::schema::Price;
use crate::sheet::property_key;

/// Property keys for the classification codes found in a file.
pub const ETIM_CLASS_KEY: &str = "etim_class";
pub const ECLASS_CLASS_KEY: &str = "eclass_class";

/// Notes of one kind listed individually before the rest are only counted.
const NOTES_PER_KIND: usize = 8;

#[derive(Debug, Error)]
pub enum BmecatError {
    #[error("the XML is not valid: {0}")]
    Xml(String),
    #[error(
        "this XML has no articles (no PRODUCT or ARTICLE elements), so it is not a BMEcat catalog"
    )]
    NotBmecat,
}

/// What a BMEcat file held.
#[derive(Debug, Clone, Default)]
pub struct BmecatRead {
    pub parts: Vec<CatalogPart>,
    /// `maker order: text` lines for things the user should look at: skipped
    /// articles, prices left out and why, customer prices.
    pub notes: Vec<String>,
    /// `2005`, `1.2`, or empty when the file does not say.
    pub version: String,
    /// ETIM feature, value or unit codes found that the supplied ETIM release
    /// (or none) could not translate, as distinct codes.
    pub etim_unread: BTreeSet<String>,
}

/// Parses BMEcat text. `default_manufacturer` is used for articles that
/// name no manufacturer (the catalog's supplier name is tried first).
pub fn import_bmecat(
    xml: &str,
    default_manufacturer: &str,
) -> Result<Vec<CatalogPart>, BmecatError> {
    read_bmecat(xml, default_manufacturer, None).map(|r| r.parts)
}

/// Like [`import_bmecat`], with the notes and an optional ETIM release for
/// feature, unit and value codes.
pub fn read_bmecat(
    xml: &str,
    default_manufacturer: &str,
    etim: Option<&EtimModel>,
) -> Result<BmecatRead, BmecatError> {
    let mut reader = Reader::from_str(xml);
    let mut out = BmecatRead::default();
    let mut w = Walk::default();

    loop {
        match reader
            .read_event()
            .map_err(|e| BmecatError::Xml(e.to_string()))?
        {
            Event::Start(e) => {
                let name = local(e.name().as_ref());
                if is_block(&name)
                    && let Some(b) = w.bufs.last_mut()
                {
                    b.push(' ');
                }
                w.bufs.push(String::new());
                if !is_inline(&name) {
                    w.start(&name, &e, &mut out);
                }
                w.path.push(name);
            }
            Event::End(e) => {
                let name = local(e.name().as_ref());
                w.path.pop();
                let raw = w.bufs.pop().unwrap_or_default();
                if is_inline(&name) {
                    // Markup inside a text (`<b>bold</b>`): its words belong
                    // to the text around it.
                    if let Some(parent) = w.bufs.last_mut() {
                        parent.push_str(&raw);
                        if is_block(&name) {
                            parent.push(' ');
                        }
                    }
                } else {
                    w.end(
                        &name,
                        &plain_text(&raw),
                        default_manufacturer,
                        etim,
                        &mut out,
                    );
                }
            }
            Event::Empty(e) => {
                let name = local(e.name().as_ref());
                if is_block(&name)
                    && let Some(b) = w.bufs.last_mut()
                {
                    b.push(' ');
                }
            }
            Event::Text(t) => {
                let s = t.decode().map_err(|e| BmecatError::Xml(e.to_string()))?;
                w.text(&s);
            }
            Event::CData(t) => w.text(&String::from_utf8_lossy(&t.into_inner())),
            Event::GeneralRef(r) => {
                if let Ok(Some(c)) = r.resolve_char_ref() {
                    w.text(c.encode_utf8(&mut [0; 4]));
                } else {
                    match r
                        .decode()
                        .map_err(|e| BmecatError::Xml(e.to_string()))?
                        .as_ref()
                    {
                        "amp" => w.text("&"),
                        "lt" => w.text("<"),
                        "gt" => w.text(">"),
                        "quot" => w.text("\""),
                        "apos" => w.text("'"),
                        _ => {}
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if out.parts.is_empty() && w.articles == 0 {
        return Err(BmecatError::NotBmecat);
    }
    w.finish_notes(&mut out);
    Ok(out)
}

/// An element name without namespace prefix, with the BMEcat 1.2 names
/// turned into their 2005 names.
fn local(name: &[u8]) -> String {
    let s = String::from_utf8_lossy(name);
    let n = s.rsplit(':').next().unwrap_or("");
    match n {
        "ARTICLE" => "PRODUCT",
        "SUPPLIER_AID" => "SUPPLIER_PID",
        "ARTICLE_DETAILS" => "PRODUCT_DETAILS",
        "ARTICLE_FEATURES" => "PRODUCT_FEATURES",
        "ARTICLE_ORDER_DETAILS" => "PRODUCT_ORDER_DETAILS",
        "ARTICLE_PRICE_DETAILS" => "PRODUCT_PRICE_DETAILS",
        "ARTICLE_PRICE" => "PRODUCT_PRICE",
        "MANUFACTURER_AID" => "MANUFACTURER_PID",
        other => other,
    }
    .to_owned()
}

/// HTML that makers put inside a description.
fn is_inline(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "b" | "i"
            | "u"
            | "em"
            | "strong"
            | "span"
            | "sub"
            | "sup"
            | "font"
            | "a"
            | "small"
            | "big"
            | "tt"
            | "code"
            | "br"
            | "p"
            | "li"
            | "ul"
            | "ol"
            | "div"
            | "tr"
            | "td"
            | "th"
            | "table"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
    )
}

/// Markup that ends a line, so the words around it do not run together.
fn is_block(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "br" | "p"
            | "li"
            | "ul"
            | "ol"
            | "div"
            | "tr"
            | "td"
            | "th"
            | "table"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
    )
}

/// Text with markup that was written as escaped text (`&lt;b&gt;`) removed
/// and white space collapsed.
fn plain_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c == '<' {
            let rest = &raw[i + 1..];
            let tag = rest
                .strip_prefix('/')
                .unwrap_or(rest)
                .chars()
                .next()
                .is_some_and(|n| n.is_ascii_alphabetic());
            if tag && let Some(end) = rest.find('>') {
                // `<br>`, `</b>`: skip to the closing bracket, keep a space.
                out.push(' ');
                for _ in 0..=end {
                    chars.next();
                }
                continue;
            }
        }
        out.push(c);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// One price line of an article.
#[derive(Debug, Clone, Default)]
struct Tier {
    kind: String,
    amount: String,
    currency: String,
    factor: String,
    lower_bound: String,
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
    order_unit: String,
    content_unit: String,
    no_cu_per_ou: String,
    price_quantity: String,
    tiers: Vec<Tier>,
    tier: Tier,
    price_date: String,
}

#[derive(Default)]
struct Walk {
    path: Vec<String>,
    bufs: Vec<String>,
    supplier: String,
    catalog_currency: String,
    cur: Option<Draft>,
    system: String,
    fname: String,
    fvalues: Vec<String>,
    funit: String,
    datetime_type: String,
    /// PRODUCT elements seen, with or without an order number.
    articles: usize,
    skipped: Vec<String>,
}

impl Walk {
    fn text(&mut self, s: &str) {
        if self.bufs.is_empty() {
            self.bufs.push(String::new());
        }
        if let Some(b) = self.bufs.last_mut() {
            b.push_str(s);
        }
    }

    fn start(&mut self, name: &str, e: &BytesStart, out: &mut BmecatRead) {
        match name {
            "BMECAT" => {
                if let Some(v) = attr(e, "version") {
                    out.version = v;
                }
            }
            "PRODUCT" if self.cur.is_none() => {
                self.articles += 1;
                self.cur = Some(Draft::default());
            }
            "FEATURE" => {
                self.fname.clear();
                self.fvalues.clear();
                self.funit.clear();
            }
            "PRODUCT_FEATURES" => self.system.clear(),
            "PRODUCT_PRICE" => {
                if let Some(d) = self.cur.as_mut() {
                    d.tier = Tier {
                        kind: attr(e, "price_type").unwrap_or_default(),
                        ..Tier::default()
                    };
                }
            }
            "DATETIME" => self.datetime_type = attr(e, "type").unwrap_or_default(),
            _ => {}
        }
    }

    fn end(
        &mut self,
        name: &str,
        text: &str,
        default_manufacturer: &str,
        etim: Option<&EtimModel>,
        out: &mut BmecatRead,
    ) {
        if !text.is_empty() {
            if let Some(d) = self.cur.as_mut() {
                match name {
                    "SUPPLIER_PID" => d.order_number = text.to_owned(),
                    "DESCRIPTION_SHORT" => d.short = text.to_owned(),
                    "DESCRIPTION_LONG" => d.long = text.to_owned(),
                    "MANUFACTURER_NAME" => d.manufacturer = text.to_owned(),
                    "MANUFACTURER_PID" => d.type_number = text.to_owned(),
                    "MANUFACTURER_TYPE_DESCR" if d.type_number.is_empty() => {
                        d.type_number = text.to_owned()
                    }
                    "KEYWORD" => d.keywords.push(text.to_owned()),
                    "REFERENCE_FEATURE_SYSTEM_NAME" => self.system = text.to_owned(),
                    "REFERENCE_FEATURE_GROUP_ID" => d.class(&self.system, text),
                    "FNAME" => self.fname = text.to_owned(),
                    "FVALUE" => self.fvalues.push(text.to_owned()),
                    "FUNIT" => self.funit = text.to_owned(),
                    "ORDER_UNIT" => d.order_unit = text.to_owned(),
                    "CONTENT_UNIT" => d.content_unit = text.to_owned(),
                    "NO_CU_PER_OU" => d.no_cu_per_ou = text.to_owned(),
                    "PRICE_QUANTITY" => d.price_quantity = text.to_owned(),
                    "PRICE_AMOUNT" => d.tier.amount = text.to_owned(),
                    "PRICE_CURRENCY" => d.tier.currency = text.to_owned(),
                    "PRICE_FACTOR" => d.tier.factor = text.to_owned(),
                    "LOWER_BOUND" => d.tier.lower_bound = text.to_owned(),
                    "DATE"
                        if self.datetime_type == "valid_start_date"
                            && self.path.iter().any(|p| p == "PRODUCT_PRICE_DETAILS") =>
                    {
                        d.price_date = text.to_owned()
                    }
                    _ => {}
                }
            } else if name == "SUPPLIER_NAME" && self.path.iter().any(|p| p == "SUPPLIER") {
                self.supplier = text.to_owned();
            } else if name == "CURRENCY" && self.path.iter().any(|p| p == "HEADER") {
                self.catalog_currency = text.to_owned();
            }
        }
        match name {
            "FEATURE" => {
                if let Some(d) = self.cur.as_mut() {
                    let (n, v, u) = translate(&self.fname, &self.fvalues, &self.funit, etim, out);
                    d.feature(&n, &v, &u);
                }
            }
            "PRODUCT_PRICE" => {
                if let Some(d) = self.cur.as_mut() {
                    let t = std::mem::take(&mut d.tier);
                    if !t.amount.is_empty() {
                        d.tiers.push(t);
                    }
                }
            }
            "PRODUCT" => {
                if let Some(d) = self.cur.take() {
                    let mut notes = Vec::new();
                    match d.finish(
                        &self.supplier,
                        default_manufacturer,
                        &self.catalog_currency,
                        &mut notes,
                    ) {
                        Some(p) => {
                            out.notes.extend(notes);
                            out.parts.push(p);
                        }
                        None => self.skipped.push(skipped_label(self.articles)),
                    }
                }
            }
            _ => {}
        }
    }

    fn finish_notes(&mut self, out: &mut BmecatRead) {
        if !self.skipped.is_empty() {
            let n = self.skipped.len();
            let first: Vec<_> = self.skipped.iter().take(NOTES_PER_KIND).cloned().collect();
            out.notes.push(format!(
                "{n} article{} with no order number (SUPPLIER_PID) skipped: {}{}",
                if n == 1 { "" } else { "s" },
                first.join(", "),
                if n > first.len() { ", ..." } else { "" }
            ));
        }
        cap_notes(&mut out.notes);
    }
}

fn skipped_label(n: usize) -> String {
    format!("article {n}")
}

/// The same kind of note listed up to a limit, then counted.
fn cap_notes(notes: &mut Vec<String>) {
    use std::collections::BTreeMap;
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut kept = Vec::with_capacity(notes.len());
    let mut dropped: BTreeMap<String, usize> = BTreeMap::new();
    for n in notes.drain(..) {
        // The kind of a note is its text after `maker order: `.
        let kind = n.split_once(": ").map(|(_, t)| t).unwrap_or(&n).to_owned();
        let c = seen.entry(kind.clone()).or_default();
        *c += 1;
        if *c <= NOTES_PER_KIND {
            kept.push(n);
        } else {
            *dropped.entry(kind).or_default() += 1;
        }
    }
    for (kind, n) in dropped {
        kept.push(format!("...and {n} more: {kind}"));
    }
    *notes = kept;
}

fn attr(e: &BytesStart, name: &str) -> Option<String> {
    e.try_get_attribute(name)
        .ok()
        .flatten()
        .and_then(|a| a.normalized_value(quick_xml::XmlVersion::Implicit1_0).ok())
        .map(|v| v.trim().to_owned())
}

/// A feature with its ETIM codes translated, as `(name, values, unit)`.
fn translate(
    name: &str,
    values: &[String],
    unit: &str,
    etim: Option<&EtimModel>,
    out: &mut BmecatRead,
) -> (String, Vec<String>, String) {
    let mut one = |code: &str, found: Option<String>| -> String {
        match found {
            Some(t) => t,
            None => {
                out.etim_unread.insert(code.to_owned());
                code.to_owned()
            }
        }
    };
    let name = if EtimModel::is_feature_code(name) {
        one(name, etim.and_then(|m| m.feature(name)).map(str::to_owned))
    } else {
        name.to_owned()
    };
    let unit = if EtimModel::is_unit_code(unit) {
        one(unit, etim.and_then(|m| m.unit(unit)).map(str::to_owned))
    } else {
        unit.to_owned()
    };
    let values = values
        .iter()
        .map(|v| {
            if EtimModel::is_value_code(v) {
                one(v, etim.and_then(|m| m.value(v)).map(str::to_owned))
            } else {
                v.clone()
            }
        })
        .collect();
    (name, values, unit)
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

    /// The list price of the article, or why there is none.
    fn price(&self, catalog_currency: &str) -> Result<Option<(Price, Option<String>)>, String> {
        // List prices first; a price that is the customer's own is last.
        let rank = |k: &str| match k {
            "net_list" => 0,
            "gros_list" => 1,
            "net_customer" => 3,
            _ => 2,
        };
        let lower = |t: &Tier| number(&t.lower_bound).unwrap_or(1.0);
        let Some(t) = self.tiers.iter().min_by(|a, b| {
            rank(&a.kind)
                .cmp(&rank(&b.kind))
                .then(lower(a).total_cmp(&lower(b)))
        }) else {
            return Ok(None);
        };
        let amount =
            number(&t.amount).ok_or_else(|| format!("price \"{}\" is not a number", t.amount))?;
        let factor = if t.factor.trim().is_empty() {
            1.0
        } else {
            number(&t.factor)
                .filter(|f| *f > 0.0)
                .ok_or_else(|| format!("price factor \"{}\" is not a positive number", t.factor))?
        };
        let amount = amount * factor;
        if !amount.is_finite() || amount <= 0.0 {
            return Err("price is zero or negative".into());
        }
        let currency = [t.currency.as_str(), catalog_currency]
            .into_iter()
            .map(str::trim)
            .find(|c| !c.is_empty())
            .ok_or_else(|| "price has no currency".to_owned())?
            .to_ascii_uppercase();
        if currency.len() != 3 || !currency.bytes().all(|b| b.is_ascii_uppercase()) {
            return Err(format!(
                "currency \"{currency}\" is not a three-letter code"
            ));
        }

        // The price is for PRICE_QUANTITY order units; an order unit holds
        // NO_CU_PER_OU content units when a content unit is given.
        let quantity = if self.price_quantity.is_empty() {
            1.0
        } else {
            number(&self.price_quantity).ok_or_else(|| {
                format!("price quantity \"{}\" is not a number", self.price_quantity)
            })?
        };
        let (count, unit) = if self.content_unit.trim().is_empty() {
            (quantity, unit_label(&self.order_unit))
        } else {
            let per_ou = if self.no_cu_per_ou.trim().is_empty() {
                // One order unit holds one content unit only when they are
                // the same unit; "a drum of metres" holds an unknown number.
                if unit_label(&self.order_unit) != unit_label(&self.content_unit) {
                    return Err(format!(
                        "order unit {} holds an unknown number of {}; price left out",
                        unit_name(&self.order_unit),
                        unit_name(&self.content_unit)
                    ));
                }
                1.0
            } else {
                number(&self.no_cu_per_ou).ok_or_else(|| {
                    format!(
                        "number of content units \"{}\" is not a number",
                        self.no_cu_per_ou
                    )
                })?
            };
            (quantity * per_ou, unit_label(&self.content_unit))
        };
        if !(1.0..=1.0e9).contains(&count) || (count - count.round()).abs() > 1e-9 {
            return Err(format!(
                "the price is for {count} {}, not a whole number of units; left out",
                if unit.is_empty() { "pieces" } else { &unit }
            ));
        }
        let mut price = Price::new(
            amount,
            currency,
            count.round() as u32,
            self.price_date.clone(),
        );
        price.unit = unit;
        let mut notes = Vec::new();
        match t.kind.as_str() {
            "net_customer" => notes.push(
                "the file holds only a customer price (net_customer); check that it is the list price"
                    .to_owned(),
            ),
            "gros_list" => notes.push("the price is a gross list price and may include VAT".to_owned()),
            _ => {}
        }
        if (factor - 1.0).abs() > f64::EPSILON {
            notes.push(format!(
                "price multiplied by the file's price factor {factor}"
            ));
        }
        if let Some(read) = thousands_or_decimal(&t.amount) {
            notes.push(format!(
                "\"{}\" read as {read}; check it is not {}",
                t.amount.trim(),
                t.amount.trim().replace(['.', ','], "")
            ));
        }
        let note = (!notes.is_empty()).then(|| notes.join("; "));
        Ok(Some((price, note)))
    }

    fn finish(
        self,
        supplier: &str,
        default_manufacturer: &str,
        catalog_currency: &str,
        notes: &mut Vec<String>,
    ) -> Option<CatalogPart> {
        if self.order_number.trim().is_empty() {
            return None;
        }
        let manufacturer = [self.manufacturer.as_str(), supplier, default_manufacturer]
            .into_iter()
            .find(|m| !m.trim().is_empty())
            .unwrap_or_default()
            .to_owned();
        let label = format!("{manufacturer} {}", self.order_number.trim());
        let price = match self.price(catalog_currency) {
            Ok(Some((p, note))) => {
                if let Some(n) = note {
                    notes.push(format!("{label}: {n}"));
                }
                Some(p)
            }
            Ok(None) => None,
            Err(why) => {
                notes.push(format!("{label}: {why}"));
                None
            }
        };
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
        part.price = price;
        Some(part)
    }
}

/// The value of a price written with one separator and exactly three digits
/// after it (`1,500`, `1.500`): BMEcat means a decimal point, but a reader
/// from a country that groups thousands that way may mean 1500. The value
/// read, or `None` when the text is not of that shape.
fn thousands_or_decimal(text: &str) -> Option<f64> {
    let t = text.trim();
    let (whole, frac) = t.split_once(['.', ','])?;
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    (digits(whole)
        && whole.len() <= 3
        && !whole.starts_with('0')
        && frac.len() == 3
        && digits(frac))
    .then(|| number(t))
    .flatten()
}

/// A unit code for a message: its label, or "piece" when it is the default.
fn unit_name(code: &str) -> String {
    let c = code.trim();
    if c.is_empty() {
        return "pieces".into();
    }
    match unit_label(c) {
        l if l.is_empty() => "pieces".into(),
        l => l,
    }
}

/// A number as BMEcat writes it (decimal point); a decimal comma is read
/// when no point is present.
fn number(text: &str) -> Option<f64> {
    let t = text.trim();
    let f: f64 = if t.contains('.') {
        t.parse().ok()?
    } else {
        t.replace(',', ".").parse().ok()?
    };
    f.is_finite().then_some(f)
}

/// The short label of a UN/ECE unit code (Recommendation 20) as BMEcat
/// writes order and content units; empty for a piece. A code not listed
/// here is kept as written, so a price is never shown for the wrong unit.
fn unit_label(code: &str) -> String {
    let c = code.trim();
    match c.to_ascii_uppercase().as_str() {
        "" | "C62" | "PCE" | "EA" | "H87" | "PC" => String::new(),
        "MTR" | "M" | "LM" => "m".into(),
        "MMT" => "mm".into(),
        "CMT" => "cm".into(),
        "KGM" => "kg".into(),
        "GRM" => "g".into(),
        "LTR" => "l".into(),
        "PR" => "pair".into(),
        "SET" => "set".into(),
        "PK" => "pack".into(),
        "BX" => "box".into(),
        "CT" => "carton".into(),
        "RO" => "roll".into(),
        _ => c.to_owned(),
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

    fn catalog(version: &str, body: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<BMECAT version="{version}"><HEADER><CATALOG><CURRENCY>EUR</CURRENCY></CATALOG>
<SUPPLIER><SUPPLIER_NAME>Example Maker</SUPPLIER_NAME></SUPPLIER></HEADER>
<T_NEW_CATALOG>{body}</T_NEW_CATALOG></BMECAT>"#
        )
    }

    fn one(body: &str) -> BmecatRead {
        read_bmecat(&catalog("2005", body), "Fallback", None).unwrap()
    }

    fn priced(order: &str, price: &str, order_details: &str) -> String {
        format!(
            "<PRODUCT><SUPPLIER_PID>{order}</SUPPLIER_PID>{order_details}\
             <PRODUCT_PRICE_DETAILS>{price}</PRODUCT_PRICE_DETAILS></PRODUCT>"
        )
    }

    #[test]
    fn bmecat_1_2_is_read_like_2005() {
        let xml = r#"<?xml version="1.0"?>
<BMECAT version="1.2"><HEADER><SUPPLIER><SUPPLIER_NAME>Old Maker</SUPPLIER_NAME></SUPPLIER></HEADER>
<T_NEW_CATALOG><ARTICLE><SUPPLIER_AID>1-001</SUPPLIER_AID>
 <ARTICLE_DETAILS><DESCRIPTION_SHORT>Relay 24 V</DESCRIPTION_SHORT>
  <MANUFACTURER_AID>R-24</MANUFACTURER_AID><MANUFACTURER_NAME>Old Maker</MANUFACTURER_NAME>
  <KEYWORD>relay</KEYWORD></ARTICLE_DETAILS>
 <ARTICLE_FEATURES><REFERENCE_FEATURE_SYSTEM_NAME>ETIM-6.0</REFERENCE_FEATURE_SYSTEM_NAME>
  <REFERENCE_FEATURE_GROUP_ID>EC001437</REFERENCE_FEATURE_GROUP_ID>
  <FEATURE><FNAME>Poles</FNAME><FVALUE>2</FVALUE></FEATURE></ARTICLE_FEATURES>
 <ARTICLE_ORDER_DETAILS><ORDER_UNIT>C62</ORDER_UNIT><CONTENT_UNIT>C62</CONTENT_UNIT>
  <NO_CU_PER_OU>1</NO_CU_PER_OU><PRICE_QUANTITY>10</PRICE_QUANTITY></ARTICLE_ORDER_DETAILS>
 <ARTICLE_PRICE_DETAILS><DATETIME type="valid_start_date"><DATE>2026-01-01</DATE></DATETIME>
  <ARTICLE_PRICE price_type="net_list"><PRICE_AMOUNT>12.50</PRICE_AMOUNT>
  <PRICE_CURRENCY>EUR</PRICE_CURRENCY><TAX>0.19</TAX></ARTICLE_PRICE>
 </ARTICLE_PRICE_DETAILS></ARTICLE></T_NEW_CATALOG></BMECAT>"#;
        let r = read_bmecat(xml, "x", None).unwrap();
        assert_eq!(r.version, "1.2");
        let p = &r.parts[0];
        assert_eq!(p.order_number, "1-001");
        assert_eq!(p.manufacturer, "Old Maker");
        assert_eq!(p.type_number, "R-24");
        assert_eq!(p.description, "Relay 24 V");
        assert_eq!(p.keywords, vec!["relay"]);
        assert_eq!(
            p.data.get(ETIM_CLASS_KEY),
            Some(&Value::Text("EC001437".into()))
        );
        assert_eq!(p.data.get("poles"), Some(&Value::Int(2)));
        let price = p.price.as_ref().expect("the 1.2 price is read");
        assert_eq!(
            (price.amount, price.per, price.unit.as_str()),
            (12.5, 10, "")
        );
        assert_eq!(price.currency, "EUR");
        assert_eq!(price.date, "2026-01-01");
        assert_eq!(price.per_label(), "per 10");
        assert!(r.notes.is_empty(), "{:?}", r.notes);
    }

    #[test]
    fn prices_say_what_quantity_and_unit_they_are_for() {
        let tier = |amount: &str| {
            format!(
                "<PRODUCT_PRICE price_type=\"net_list\"><PRICE_AMOUNT>{amount}</PRICE_AMOUNT>\
                 <PRICE_CURRENCY>EUR</PRICE_CURRENCY></PRODUCT_PRICE>"
            )
        };
        let body = [
            // Terminal: price for 100 pieces.
            priced("T-1", &tier("42.00"), "<PRODUCT_ORDER_DETAILS><ORDER_UNIT>C62</ORDER_UNIT><PRICE_QUANTITY>100</PRICE_QUANTITY></PRODUCT_ORDER_DETAILS>"),
            // Cable: price for 1000 metres.
            priced("C-1", &tier("1850.00"), "<PRODUCT_ORDER_DETAILS><ORDER_UNIT>MTR</ORDER_UNIT><CONTENT_UNIT>MTR</CONTENT_UNIT><NO_CU_PER_OU>1</NO_CU_PER_OU><PRICE_QUANTITY>1000</PRICE_QUANTITY></PRODUCT_ORDER_DETAILS>"),
            // Cable sold on a 500 m drum, priced per drum.
            priced("C-2", &tier("900"), "<PRODUCT_ORDER_DETAILS><ORDER_UNIT>DR</ORDER_UNIT><CONTENT_UNIT>MTR</CONTENT_UNIT><NO_CU_PER_OU>500</NO_CU_PER_OU></PRODUCT_ORDER_DETAILS>"),
            // Contactor: no order details, one piece.
            priced("K-1", &tier("98.70"), ""),
            // A pack of ten pieces.
            priced("P-1", &tier("5"), "<PRODUCT_ORDER_DETAILS><ORDER_UNIT>PK</ORDER_UNIT><CONTENT_UNIT>C62</CONTENT_UNIT><NO_CU_PER_OU>10</NO_CU_PER_OU></PRODUCT_ORDER_DETAILS>"),
            // Priced per pair.
            priced("S-1", &tier("7"), "<PRODUCT_ORDER_DETAILS><ORDER_UNIT>PR</ORDER_UNIT></PRODUCT_ORDER_DETAILS>"),
        ]
        .concat();
        let r = one(&body);
        let price = |order: &str| {
            let p = r.parts.iter().find(|p| p.order_number == order).unwrap();
            let pr = p.price.as_ref().unwrap();
            (pr.amount, pr.per_label())
        };
        assert_eq!(price("T-1"), (42.0, "per 100".into()));
        assert_eq!(price("C-1"), (1850.0, "per 1000 m".into()));
        assert_eq!(price("C-2"), (900.0, "per 500 m".into()));
        assert_eq!(price("K-1"), (98.7, String::new()));
        assert_eq!(price("P-1"), (5.0, "per 10".into()));
        assert_eq!(price("S-1"), (7.0, "per pair".into()));
        assert!(r.notes.is_empty(), "{:?}", r.notes);
        // The currency came from the catalog header, not from the lines.
    }

    #[test]
    fn a_price_that_cannot_be_read_with_certainty_is_left_out_or_noted() {
        let tier = |kind: &str, amount: &str, extra: &str| {
            format!(
                "<PRODUCT_PRICE price_type=\"{kind}\"><PRICE_AMOUNT>{amount}</PRICE_AMOUNT>\
                 <PRICE_CURRENCY>EUR</PRICE_CURRENCY>{extra}</PRODUCT_PRICE>"
            )
        };
        let body = [
            // A drum of metres without the number of metres in it.
            priced("D-1", &tier("net_list", "900", ""), "<PRODUCT_ORDER_DETAILS><ORDER_UNIT>DR</ORDER_UNIT><CONTENT_UNIT>MTR</CONTENT_UNIT></PRODUCT_ORDER_DETAILS>"),
            // Same unit twice needs no count.
            priced("D-2", &tier("net_list", "2", ""), "<PRODUCT_ORDER_DETAILS><ORDER_UNIT>MTR</ORDER_UNIT><CONTENT_UNIT>MTR</CONTENT_UNIT></PRODUCT_ORDER_DETAILS>"),
            // Factors that are not a positive number.
            priced("F-1", &tier("net_list", "10", "<PRICE_FACTOR>abc</PRICE_FACTOR>"), ""),
            priced("F-2", &tier("net_list", "10", "<PRICE_FACTOR>0</PRICE_FACTOR>"), ""),
            priced("F-3", &tier("net_list", "10", "<PRICE_FACTOR>-2</PRICE_FACTOR>"), ""),
            // A gross price and a factor give both notes.
            priced("F-4", &tier("gros_list", "10", "<PRICE_FACTOR>0.5</PRICE_FACTOR>"), ""),
            // Three digits after one separator may be a thousands group.
            priced("T-1", &tier("net_list", "1,500", ""), ""),
            priced("T-2", &tier("net_list", "1.500", ""), ""),
            priced("T-3", &tier("net_list", "0.500", ""), ""),
            priced("T-4", &tier("net_list", "1.50", ""), ""),
            priced("T-5", &tier("net_list", "1,234.500", ""), ""),
        ]
        .concat();
        let r = one(&body);
        let part = |order: &str| r.parts.iter().find(|p| p.order_number == order).unwrap();
        let notes_for = |order: &str| -> Vec<&String> {
            r.notes
                .iter()
                .filter(|n| n.contains(&format!(" {order}:")))
                .collect()
        };

        assert!(part("D-1").price.is_none());
        let n = notes_for("D-1");
        assert!(
            n.len() == 1 && n[0].contains("order unit DR holds an unknown number of m"),
            "{n:?}"
        );
        assert_eq!(part("D-2").price.as_ref().unwrap().per_label(), "per m");
        assert!(notes_for("D-2").is_empty());

        for order in ["F-1", "F-2", "F-3"] {
            assert!(part(order).price.is_none(), "{order}");
            let n = notes_for(order);
            assert!(
                n.len() == 1 && n[0].contains("price factor"),
                "{order}: {n:?}"
            );
        }
        let n = notes_for("F-4");
        assert!(
            n.len() == 1 && n[0].contains("gross list price") && n[0].contains("price factor 0.5"),
            "{n:?}"
        );
        assert_eq!(part("F-4").price.as_ref().unwrap().amount, 5.0);

        for order in ["T-1", "T-2"] {
            let n = notes_for(order);
            assert!(
                n.len() == 1 && n[0].contains("read as 1.5; check it is not 1500"),
                "{order}: {n:?}"
            );
            assert_eq!(part(order).price.as_ref().unwrap().amount, 1.5);
        }
        for order in ["T-3", "T-4"] {
            assert!(
                notes_for(order).is_empty(),
                "{order}: {:?}",
                notes_for(order)
            );
            // A number with both separators is not guessed at.
            assert!(part("T-5").price.is_none());
            assert!(notes_for("T-5")[0].contains("not a number"));
        }
    }

    #[test]
    fn the_list_price_wins_over_a_customer_price_and_the_smallest_quantity_over_a_discount() {
        let line = |kind: &str, amount: &str, lower: &str| {
            format!(
                "<PRODUCT_PRICE price_type=\"{kind}\"><PRICE_AMOUNT>{amount}</PRICE_AMOUNT>\
                 <PRICE_CURRENCY>EUR</PRICE_CURRENCY><LOWER_BOUND>{lower}</LOWER_BOUND></PRODUCT_PRICE>"
            )
        };
        let both = [
            line("net_customer", "60", "1"),
            line("net_list", "100", "10"),
            line("net_list", "90", "100"),
            line("net_list", "110", "1"),
        ]
        .concat();
        let r = one(&priced("A", &both, ""));
        assert_eq!(r.parts[0].price.as_ref().unwrap().amount, 110.0);
        assert!(r.notes.is_empty(), "{:?}", r.notes);

        let only_customer = line("net_customer", "60", "1");
        let r = one(&priced("B", &only_customer, ""));
        assert_eq!(r.parts[0].price.as_ref().unwrap().amount, 60.0);
        assert!(r.notes[0].contains("customer price"), "{:?}", r.notes);

        let gross = line("gros_list", "119", "1");
        let r = one(&priced("C", &gross, ""));
        assert!(r.notes[0].contains("gross"), "{:?}", r.notes);
    }

    #[test]
    fn a_price_that_cannot_be_trusted_is_left_out_and_says_why() {
        let no_currency = "<PRODUCT_PRICE><PRICE_AMOUNT>5</PRICE_AMOUNT></PRODUCT_PRICE>";
        let xml = format!(
            r#"<BMECAT version="2005"><T_NEW_CATALOG>{}{}{}{}{}</T_NEW_CATALOG></BMECAT>"#,
            priced("N1", no_currency, ""),
            priced(
                "N2",
                "<PRODUCT_PRICE><PRICE_AMOUNT>0</PRICE_AMOUNT><PRICE_CURRENCY>EUR</PRICE_CURRENCY></PRODUCT_PRICE>",
                ""
            ),
            priced(
                "N3",
                "<PRODUCT_PRICE><PRICE_AMOUNT>abc</PRICE_AMOUNT><PRICE_CURRENCY>EUR</PRICE_CURRENCY></PRODUCT_PRICE>",
                ""
            ),
            priced(
                "N4",
                "<PRODUCT_PRICE><PRICE_AMOUNT>5</PRICE_AMOUNT><PRICE_CURRENCY>EUR</PRICE_CURRENCY></PRODUCT_PRICE>",
                "<PRODUCT_ORDER_DETAILS><ORDER_UNIT>MTR</ORDER_UNIT><PRICE_QUANTITY>0.5</PRICE_QUANTITY></PRODUCT_ORDER_DETAILS>"
            ),
            priced(
                "N5",
                "<PRODUCT_PRICE><PRICE_AMOUNT>5</PRICE_AMOUNT><PRICE_CURRENCY>Euro</PRICE_CURRENCY></PRODUCT_PRICE>",
                ""
            ),
        );
        let r = read_bmecat(&xml, "M", None).unwrap();
        assert_eq!(r.parts.len(), 5, "the articles are kept without a price");
        assert!(r.parts.iter().all(|p| p.price.is_none()));
        let text = r.notes.join("\n");
        for want in [
            "M N1: price has no currency",
            "M N2: price is zero or negative",
            "M N3: price \"abc\" is not a number",
            "M N4: the price is for 0.5 m, not a whole number",
            "M N5: currency \"EURO\" is not a three-letter code",
        ] {
            assert!(text.contains(want), "{want}\n{text}");
        }
    }

    #[test]
    fn words_inside_markup_are_kept_and_escaped_markup_is_removed() {
        let desc = |d: &str| {
            format!(
                "<PRODUCT><SUPPLIER_PID>X</SUPPLIER_PID><PRODUCT_DETAILS><DESCRIPTION_SHORT>{d}</DESCRIPTION_SHORT></PRODUCT_DETAILS></PRODUCT>"
            )
        };
        let text = |d: &str| one(&desc(d)).parts[0].description.clone();
        assert_eq!(text("UT 2,5 <b>bold</b> &amp; more"), "UT 2,5 bold & more");
        assert_eq!(text("Line one<br/>line two"), "Line one line two");
        assert_eq!(text("A<p>B</p><p>C</p>"), "A B C");
        assert_eq!(text("&lt;b&gt;bold&lt;/b&gt; text"), "bold text");
        assert_eq!(text("<![CDATA[<i>cdata</i> kept]]>"), "cdata kept");
        assert_eq!(text("  spaced \n  out  "), "spaced out");
        // A less-than sign that is not a tag stays.
        assert_eq!(text("U &lt; 30 V and I &gt; 2 A"), "U < 30 V and I > 2 A");
    }

    #[test]
    fn etim_codes_are_translated_with_a_release_and_counted_without_one() {
        let xml = catalog(
            "2005",
            "<PRODUCT><SUPPLIER_PID>E-1</SUPPLIER_PID><PRODUCT_FEATURES>\
             <REFERENCE_FEATURE_SYSTEM_NAME>ETIM-9.0</REFERENCE_FEATURE_SYSTEM_NAME>\
             <FEATURE><FNAME>EF900001</FNAME><FVALUE>5.2</FVALUE><FUNIT>EU900001</FUNIT></FEATURE>\
             <FEATURE><FNAME>EF900003</FNAME><FVALUE>EV900002</FVALUE></FEATURE>\
             <FEATURE><FNAME>Poles</FNAME><FVALUE>3</FVALUE></FEATURE>\
             <FEATURE><FNAME>EF999999</FNAME><FVALUE>1</FVALUE></FEATURE>\
             </PRODUCT_FEATURES></PRODUCT>",
        );
        let none = read_bmecat(&xml, "M", None).unwrap();
        assert_eq!(none.etim_unread.len(), 5, "{:?}", none.etim_unread);
        assert!(none.parts[0].data.contains_key("ef900001"), "as before");

        let model = EtimModel::from_files(
            "release.zip",
            &[
                (
                    "f.csv".into(),
                    b"Code;Desc EN\nEF900001;Test width\nEF900003;Test connection\nEF900004;x\n"
                        .to_vec(),
                ),
                (
                    "u.csv".into(),
                    b"Code;Desc EN;Abbreviation\nEU900001;Test millimetre;mm\nEU900002;x;y\n"
                        .to_vec(),
                ),
                (
                    "v.csv".into(),
                    b"Code;Desc EN\nEV900001;a\nEV900002;Test push-in\n".to_vec(),
                ),
            ],
        )
        .unwrap();
        let r = read_bmecat(&xml, "M", Some(&model)).unwrap();
        let d = &r.parts[0].data;
        assert_eq!(d.get("test_width"), Some(&Value::Text("5.2 mm".into())));
        assert_eq!(
            d.get("test_connection"),
            Some(&Value::Text("Test push-in".into()))
        );
        assert_eq!(d.get("poles"), Some(&Value::Int(3)));
        assert_eq!(
            r.etim_unread.iter().map(String::as_str).collect::<Vec<_>>(),
            ["EF999999"],
            "only the code the release does not know"
        );
    }

    #[test]
    fn an_article_without_an_order_number_is_reported_not_dropped_silently() {
        let r = one("<PRODUCT><SUPPLIER_PID>OK-1</SUPPLIER_PID></PRODUCT>\
             <PRODUCT><PRODUCT_DETAILS><DESCRIPTION_SHORT>x</DESCRIPTION_SHORT></PRODUCT_DETAILS></PRODUCT>\
             <PRODUCT><SUPPLIER_PID>  </SUPPLIER_PID></PRODUCT>");
        assert_eq!(r.parts.len(), 1);
        assert!(
            r.notes
                .iter()
                .any(|n| n.starts_with("2 articles with no order number")),
            "{:?}",
            r.notes
        );
    }

    #[test]
    fn a_catalog_of_only_articles_without_order_numbers_is_still_a_bmecat_file() {
        let r = one("<PRODUCT><PRODUCT_DETAILS/></PRODUCT>");
        assert!(r.parts.is_empty());
        assert!(!r.notes.is_empty());
    }

    #[test]
    fn many_notes_of_one_kind_are_listed_then_counted() {
        let tier = "<PRODUCT_PRICE><PRICE_AMOUNT>5</PRICE_AMOUNT></PRODUCT_PRICE>";
        let body: String = (0..20)
            .map(|i| priced(&format!("N{i}"), tier, ""))
            .collect();
        let xml = format!(r#"<BMECAT><T_NEW_CATALOG>{body}</T_NEW_CATALOG></BMECAT>"#);
        let r = read_bmecat(&xml, "M", None).unwrap();
        assert!(r.notes.len() <= NOTES_PER_KIND + 1, "{}", r.notes.len());
        assert!(
            r.notes.last().unwrap().starts_with("...and 12 more"),
            "{:?}",
            r.notes.last()
        );
    }

    #[test]
    fn a_large_catalog_with_prices_reads_in_reasonable_time() {
        let body: String = (0..20_000)
            .map(|i| {
                priced(
                    &format!("A-{i}"),
                    "<PRODUCT_PRICE price_type=\"net_list\"><PRICE_AMOUNT>1.50</PRICE_AMOUNT></PRODUCT_PRICE>",
                    "<PRODUCT_ORDER_DETAILS><ORDER_UNIT>C62</ORDER_UNIT><PRICE_QUANTITY>100</PRICE_QUANTITY></PRODUCT_ORDER_DETAILS>",
                )
            })
            .collect();
        let xml = catalog("2005", &body);
        let t = std::time::Instant::now();
        let r = read_bmecat(&xml, "M", None).unwrap();
        eprintln!("20000 articles in {:?}", t.elapsed());
        assert_eq!(r.parts.len(), 20_000);
        assert!(t.elapsed().as_secs() < 20, "{:?}", t.elapsed());
    }
}
