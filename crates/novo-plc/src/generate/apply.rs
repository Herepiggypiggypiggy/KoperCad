//! Building what a [`Plan`] describes, inside the caller's transaction so
//! the whole import is one undo step: a card per planned card, a page per
//! card with its channels in a row, a field terminal on a strip for every
//! channel pin, and the wires between them.
//!
//! The page reads like a hand-drawn PLC page. Inputs have the field
//! terminals above the channels, outputs below, a relay output both. The
//! device tag and the signal text are live texts of the channel symbol, so
//! editing the channel's properties changes the page.

use novo_core::{
    DeviceId, DeviceTag, Direction, Endpoint, ModelError, Page, PageId, Placement, Point, Result,
    Rotation, SymbolId, SymbolRole, Terminal, TextAnchor, TextContent, TextPlacement, Tx, Value,
};
use novo_editor::catalog::model_pins;
use novo_editor::edit::draw_wire;
use novo_editor::geometry::Target;
use novo_editor::pages::add_page;
use novo_editor::pinkind::{Verdict, judge};
use novo_symbols::{Library, Symbol};
use novo_terminals::{next_free_number, next_order, strip_style, symbol_pins};

use crate::card::{ADDRESS_TEXT, CHANNEL, SIGNAL, SPARE, start_key};
use crate::commands::{NewCardLayout, add_card};
use crate::index::channel_key;
use crate::layout::{ChannelType, Space};
use crate::link::tags::TAG;

use super::FIELD;
use super::list::{IoItem, IoList, Note};
use super::plan::{Plan, PlannedCard};

/// What an import built.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Built {
    pub cards: Vec<DeviceId>,
    pub pages: Vec<PageId>,
    pub channels: usize,
    pub terminals: usize,
    /// Wires the pin-kind rule refused, and anything else worth saying.
    pub notes: Vec<Note>,
}

/// Distance from a channel pin to its terminal's internal pin, in mm.
const GAP: f64 = 12.0;
/// Left edge of the first channel column, in mm.
const MARGIN: f64 = 30.0;
/// The widest and narrowest column spacing, in mm.
const SPACING: (f64, f64) = (24.0, 14.0);

fn refused(what: &str, reason: String) -> ModelError {
    ModelError::Refused {
        what: what.into(),
        reason,
    }
}

/// The library symbol that draws a channel type.
fn channel_symbol(library: &Library, kind: ChannelType) -> Result<&Symbol> {
    library.by_function(kind.function()).next().ok_or_else(|| {
        refused(
            "import I/O list",
            format!("the symbol library has no {kind} symbol"),
        )
    })
}

/// Adds everything of `plan`. `library` supplies the channel symbols.
pub fn apply(tx: &mut Tx<'_>, library: &Library, list: &IoList, plan: &Plan) -> Result<Built> {
    let mut built = Built::default();
    let mut after: Option<PageId> = tx.project().pages_in_order().last().map(|p| p.id);
    for card in &plan.cards {
        let (device, page) = build_card(tx, library, list, card, after, &mut built)?;
        built.cards.push(device);
        built.pages.push(page);
        after = Some(page);
    }
    // The cards exist now: fill in the address texts drawn under the channels.
    crate::index::sync_address_texts(tx, library)?;
    Ok(built)
}

fn build_card(
    tx: &mut Tx<'_>,
    library: &Library,
    list: &IoList,
    planned: &PlannedCard,
    after: Option<PageId>,
    built: &mut Built,
) -> Result<(DeviceId, PageId)> {
    let device = add_card(
        tx,
        planned.tag.clone(),
        "From the I/O list",
        NewCardLayout::Typed(planned.layout.clone()),
        &planned.station,
        planned.slot,
    )?;
    if let Some(start) = planned.start {
        tx.set_property(
            device,
            start_key(planned.kind.space()),
            Some(Value::Int(i64::from(start))),
        )?;
    }
    let (page, _) = add_page(tx, after)?;
    let title = format!(
        "PLC {} {}{}",
        planned.tag,
        planned.layout,
        if planned.spares() > 0 {
            format!(" ({} spare)", planned.spares())
        } else {
            String::new()
        }
    );
    tx.update::<Page>(page, |p| p.title = title.clone())?;
    let strip_number = tx.project().next_tag_number(&DeviceTag::default(), "X");
    let strip = tx.add_terminal_strip(DeviceTag::parse(&format!("-X{strip_number}"))?)?;

    let symbol = channel_symbol(library, planned.kind)?;
    let pins = model_pins(symbol);
    let width = tx
        .project()
        .page(page)
        .map_or(420.0, |p| p.format.size_mm().0);
    let count = planned.channels.len().max(1) as f64;
    let spacing = ((width - 2.0 * MARGIN) / count).clamp(SPACING.1, SPACING.0);
    let has_up = pins.iter().any(|p| p.direction == Some(Direction::Up));
    let has_down = pins.iter().any(|p| p.direction == Some(Direction::Down));
    let y0 = match (has_up, has_down) {
        (true, true) => 140.0,
        (true, false) => 150.0,
        _ => 90.0,
    };

    tx.add_text(
        page,
        TextContent::Literal { text: title },
        TextPlacement {
            offset: Point::new(MARGIN, 24.0),
            anchor: TextAnchor::BottomLeft,
            ..TextPlacement::default()
        },
        None,
    )?;

    for (number, item) in planned.channels.iter().enumerate() {
        // A spare is drawn too, so the page shows the whole card; it has no terminal or wire.
        let item = item.as_ref().map(|&i| &list.items[i]);
        let x = MARGIN + spacing * (number as f64 + 0.5);
        let channel = place_channel(
            tx,
            device,
            planned.kind,
            number,
            symbol,
            &pins,
            page,
            (x, y0),
            item,
        )?;
        if item.is_some() {
            built.channels += 1;
        }
        // A spare shows its address too: the card's free channels are the ones to find.
        address_label(tx, page, channel, has_up, has_down)?;
        for (pin_index, pin) in pins.iter().enumerate() {
            let above = pin.direction == Some(Direction::Up);
            let pin_at = Point::new(x + pin.offset.x, y0 + pin.offset.y);
            if item.is_none() {
                if pin_index == 0 {
                    let far_y = if above {
                        pin_at.y - GAP - 6.0
                    } else {
                        pin_at.y + GAP + 6.0
                    };
                    label(tx, page, channel, pin_at, far_y, above)?;
                }
                continue;
            }
            let end = terminal_for(tx, strip, page, pin_at, above, built)?;
            wire(
                tx,
                library,
                symbol,
                pin.name.as_str(),
                page,
                channel,
                pin_index as u16,
                pin_at,
                end,
                built,
            )?;
            if pin_index == 0 {
                label(tx, page, channel, pin_at, end.far_y, above)?;
            }
        }
    }
    Ok((device, page))
}

#[allow(clippy::too_many_arguments)]
fn place_channel(
    tx: &mut Tx<'_>,
    device: DeviceId,
    kind: ChannelType,
    number: usize,
    symbol: &Symbol,
    pins: &[novo_core::Pin],
    page: PageId,
    (x, y): (f64, f64),
    item: Option<&IoItem>,
) -> Result<SymbolId> {
    let mut pins = pins.to_vec();
    if item.is_none() {
        // A spare channel is meant to stay free.
        for pin in &mut pins {
            pin.optional = true;
        }
    }
    let id = tx.place_symbol(
        page,
        symbol.id.as_str(),
        Placement::at(Point::new(x, y)),
        pins,
        SymbolRole::Device {
            device,
            main: false,
        },
    )?;
    let Some(item) = item else {
        // Marked, not keyed: the index keeps counting this channel as a spare.
        tx.set_property(id, SPARE, Some(Value::Bool(true)))?;
        tx.set_property(id, SIGNAL, Some(Value::from("Spare".to_owned())))?;
        // The channel it stands for, so the address text can follow the card.
        let number = u16::try_from(number)
            .map_err(|_| refused("import I/O list", "too many channels".into()))?;
        tx.set_property(id, CHANNEL, Some(Value::from(channel_key(kind, number))))?;
        tx.set_property(id, ADDRESS_TEXT, Some(Value::from(String::new())))?;
        return Ok(id);
    };
    let number = u16::try_from(number)
        .map_err(|_| refused("import I/O list", "too many channels".into()))?;
    tx.set_property(id, CHANNEL, Some(Value::from(channel_key(kind, number))))?;
    let signal = if item.description.is_empty() {
        item.tag.clone()
    } else {
        item.description.clone()
    };
    tx.set_property(id, SIGNAL, Some(Value::from(signal)))?;
    tx.set_property(id, ADDRESS_TEXT, Some(Value::from(String::new())))?;
    if !item.tag.is_empty() {
        tx.set_property(id, FIELD, Some(Value::from(item.tag.clone())))?;
    }
    if !item.plc_tag.is_empty() {
        tx.set_property(id, TAG, Some(Value::from(item.plc_tag.clone())))?;
    }
    Ok(id)
}

/// A terminal placed beside a channel pin.
#[derive(Clone, Copy)]
struct Placed {
    symbol: SymbolId,
    /// The terminal's inner pin, the one that meets the channel.
    near: Point,
    /// The y of its outer end, where the labels start.
    far_y: f64,
}

fn terminal_for(
    tx: &mut Tx<'_>,
    strip: DeviceId,
    page: PageId,
    pin_at: Point,
    above: bool,
    built: &mut Built,
) -> Result<Placed> {
    let kind = novo_core::TerminalKind::Feedthrough;
    let name = next_free_number(tx.project(), strip);
    let order = next_order(tx.project(), strip);
    let terminal = tx.add_terminal(strip, name)?;
    tx.update::<Terminal>(terminal, |t| {
        t.kind = kind;
        t.order = order;
    })?;
    let key = strip_style(tx.project(), strip, kind).symbol;
    let mut pins = symbol_pins(&key)?;
    let length = pins[1].offset.y;
    // The field side waits for the field cable, which is not drawn yet.
    pins[1].optional = true;
    // Pin `1` is the cabinet side and meets the channel; `2` is the field.
    let (origin, rotation, far_y) = if above {
        let y = pin_at.y - GAP;
        (Point::new(pin_at.x, y), Rotation::R180, y - length)
    } else {
        let y = pin_at.y + GAP;
        (Point::new(pin_at.x, y), Rotation::R0, y + length)
    };
    let symbol = tx.place_symbol(
        page,
        key,
        Placement {
            origin,
            rotation,
            mirrored: false,
        },
        pins,
        SymbolRole::Terminal {
            terminals: vec![terminal],
        },
    )?;
    built.terminals += 1;
    Ok(Placed {
        symbol,
        near: origin,
        far_y,
    })
}

/// Wires a channel pin to the inner pin of its terminal, after the same
/// pin-kind rule the automatic wiring uses.
#[allow(clippy::too_many_arguments)]
fn wire(
    tx: &mut Tx<'_>,
    library: &Library,
    symbol: &Symbol,
    pin_name: &str,
    page: PageId,
    channel: SymbolId,
    pin: u16,
    pin_at: Point,
    end: Placed,
    built: &mut Built,
) -> Result<()> {
    if let Some(terminal) = library.get("iec/terminals/terminal")
        && let Verdict::Refuse(why) = judge((symbol, pin_name), (terminal, "1"))
    {
        built.notes.push(Note {
            line: None,
            text: format!("no wire from {} {pin_name}: {why}", symbol.name),
        });
        return Ok(());
    }
    draw_wire(
        tx,
        page,
        Target::Endpoint(Endpoint::pin(channel, pin), pin_at),
        &[],
        Target::Endpoint(Endpoint::pin(end.symbol, 0), end.near),
    )?;
    Ok(())
}

/// The channel's address, on the side of the box that has no terminal wire.
fn address_label(
    tx: &mut Tx<'_>,
    page: PageId,
    channel: SymbolId,
    has_up: bool,
    has_down: bool,
) -> Result<()> {
    let (offset, anchor) = match (has_up, has_down) {
        // The box hangs below the pin: the address goes under the box.
        (true, false) => (Point::new(0.0, 12.5), TextAnchor::TopCenter),
        (false, true) => (Point::new(0.0, -2.5), TextAnchor::BottomCenter),
        _ => (Point::new(8.0, 8.0), TextAnchor::MiddleLeft),
    };
    tx.add_text(
        page,
        TextContent::Property {
            key: ADDRESS_TEXT.into(),
        },
        TextPlacement {
            offset,
            rotation: Rotation::R0,
            anchor,
        },
        Some(channel.into()),
    )?;
    Ok(())
}

/// The device tag and the signal text, at the field end of the column.
fn label(
    tx: &mut Tx<'_>,
    page: PageId,
    channel: SymbolId,
    pin_at: Point,
    far_y: f64,
    above: bool,
) -> Result<()> {
    let origin_y = tx
        .project()
        .symbol(channel)
        .map_or(pin_at.y, |s| s.placement.origin.y);
    let rel = far_y - origin_y;
    let (tag_anchor, tag_y, text_y, text_rotation, text_anchor) = if above {
        (
            TextAnchor::BottomCenter,
            rel - 1.0,
            rel - 7.0,
            Rotation::R270,
            TextAnchor::BottomRight,
        )
    } else {
        (
            TextAnchor::TopCenter,
            rel + 1.0,
            rel + 7.0,
            Rotation::R90,
            TextAnchor::BottomRight,
        )
    };
    let field = |key: &str| TextContent::Property { key: key.into() };
    tx.add_text(
        page,
        field(FIELD),
        TextPlacement {
            offset: Point::new(0.0, tag_y),
            rotation: Rotation::R0,
            anchor: tag_anchor,
        },
        Some(channel.into()),
    )?;
    tx.add_text(
        page,
        field(SIGNAL),
        TextPlacement {
            offset: Point::new(1.5, text_y),
            rotation: text_rotation,
            anchor: text_anchor,
        },
        Some(channel.into()),
    )?;
    Ok(())
}

/// The spaces a plan's cards use, for messages.
pub fn spaces(plan: &Plan) -> Vec<Space> {
    let mut out: Vec<Space> = plan.cards.iter().map(|c| c.kind.space()).collect();
    out.sort();
    out.dedup();
    out
}
