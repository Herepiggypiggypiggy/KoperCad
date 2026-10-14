//! What texts show and where they sit.

use crate::entity::{SymbolRole, Text, TextContent, Value};
use crate::geom::Point;
use crate::id::{AnyId, TextId};
use crate::project::Project;

impl Project {
    /// The texts attached to an object, by id.
    pub fn texts_of(&self, object: impl Into<AnyId>) -> Vec<TextId> {
        self.referrers(object.into())
            .filter_map(|id| match id {
                AnyId::Text(t) => Some(t),
                _ => None,
            })
            .collect()
    }

    /// The point a text's offset is measured from: its attached symbol's
    /// origin, junction's position or the middle of its wire's first
    /// segment; the page origin for a free text.
    pub fn text_origin(&self, text: &Text) -> Point {
        let origin = match text.attached_to {
            Some(AnyId::Symbol(s)) => self.symbol(s).map(|s| s.placement.origin),
            Some(AnyId::Junction(j)) => self.junction(j).map(|j| j.position),
            Some(AnyId::Wire(w)) => self.wire(w).and_then(|w| {
                let start = self.endpoint_position(w.from)?;
                let end = match w.route.first() {
                    Some(&bend) => bend,
                    None => self.endpoint_position(w.to)?,
                };
                Some(Point::new((start.x + end.x) / 2.0, (start.y + end.y) / 2.0))
            }),
            _ => None,
        };
        origin.unwrap_or(Point::new(0.0, 0.0))
    }

    /// Where a text sits on its page: its origin plus its offset. The
    /// anchor names which point of the text box is there.
    pub fn text_position(&self, text: TextId) -> Option<Point> {
        let text = self.text(text)?;
        let origin = self.text_origin(text);
        let offset = text.placement.offset;
        Some(Point::new(origin.x + offset.x, origin.y + offset.y))
    }

    /// What a text shows right now. A label whose source is missing (a
    /// wire without a number, a symbol without a device) shows nothing.
    pub fn text_string(&self, text: TextId) -> Option<String> {
        let text = self.text(text)?;
        let attached = text.attached_to;
        let shown = match &text.content {
            TextContent::Literal { text } => Some(text.clone()),
            TextContent::DeviceTag => match attached {
                Some(AnyId::Symbol(s)) => {
                    let symbol = self.symbol(s)?;
                    match &symbol.role {
                        SymbolRole::Device { device, .. } => {
                            self.device(*device).map(|d| d.tag.to_string())
                        }
                        SymbolRole::Terminal { terminals } => {
                            self.terminal_designation(*terminals.first()?)
                        }
                        SymbolRole::Interruption { name } => Some(name.clone()),
                        SymbolRole::PotentialFeed { potential } => {
                            self.potential(*potential).map(|p| p.name.clone())
                        }
                        SymbolRole::Unassigned => None,
                    }
                }
                _ => None,
            },
            TextContent::WireNumber => match attached {
                Some(AnyId::Wire(w)) => self.wire(w)?.number.clone(),
                _ => None,
            },
            TextContent::Potential => match attached {
                Some(AnyId::Wire(w)) => self
                    .wire(w)?
                    .potential
                    .and_then(|p| self.potential(p))
                    .map(|p| p.name.clone()),
                _ => None,
            },
            TextContent::Property { key } => {
                let record = self.record(attached?)?;
                let value = record.properties().get(key).or_else(|| match &record {
                    crate::Record::Symbol(s) => self.device(s.device()?)?.properties.get(key),
                    _ => None,
                });
                value.map(|v| match v {
                    Value::Bool(b) => b.to_string(),
                    Value::Int(i) => i.to_string(),
                    Value::Float(f) => f.to_string(),
                    Value::Text(t) => t.clone(),
                })
            }
        };
        Some(shown.unwrap_or_default())
    }
}
