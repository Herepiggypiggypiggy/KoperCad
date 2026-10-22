//! The EMC example (View > EMC view, "Open EMC example"): the motor starter
//! plus a page with a frequency converter, an unshielded motor cable, a PLC
//! with an analog input and a 24 V supply, so every layer of the EMC view has
//! something to show. Illustrative: the layout is a typical bad first draft
//! on purpose (signal wires beside the motor output), not a recommendation.

use novo_core::{
    DeviceTag, Document, Endpoint, Page, Placement, Point, PotentialKind, Result as ModelResult,
    Rotation, SymbolId, Tx,
};
use novo_editor::catalog::Catalog;
use novo_editor::edit::{PlaceOptions, draw_wire, place};
use novo_editor::geometry::Target;

use super::example::build_example_with;

/// What the project is called.
pub(crate) const EMC_EXAMPLE_NAME: &str = "Drive and signals (EMC example)";

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).expect("the demo tags are valid")
}

/// Draws the EMC example into a project that has only its empty first page.
pub(crate) fn build_emc_example(doc: &mut Document) -> ModelResult<novo_core::ChangeSet> {
    let catalog = Catalog::builtin();
    build_example_with(doc, |tx| draw(tx, &catalog))
}

fn put(
    tx: &mut Tx<'_>,
    catalog: &Catalog,
    page: novo_core::PageId,
    key: &str,
    at: (f64, f64),
    parent: Option<&str>,
) -> ModelResult<SymbolId> {
    let symbol = catalog.get(key).expect("the symbol is in the library");
    let mut options = PlaceOptions::new();
    options.parent = parent.map(tag);
    let placement = Placement {
        origin: Point::new(at.0, at.1),
        rotation: Rotation::R0,
        mirrored: false,
    };
    place(tx, page, symbol, placement, &options)
}

fn wire(
    tx: &mut Tx<'_>,
    page: novo_core::PageId,
    a: (SymbolId, u16),
    b: (SymbolId, u16),
) -> ModelResult<()> {
    let target = |tx: &Tx<'_>, (symbol, pin): (SymbolId, u16)| {
        let end = Endpoint::pin(symbol, pin);
        let at = tx
            .project()
            .endpoint_position(end)
            .expect("the pin has a position");
        Target::Endpoint(end, at)
    };
    let (from, to) = (target(tx, a), target(tx, b));
    draw_wire(tx, page, from, &[], to).map(|_| ())
}

fn draw(tx: &mut Tx<'_>, catalog: &Catalog) -> ModelResult<()> {
    let page = tx.add_page("3")?;
    tx.update::<Page>(page, |p| {
        p.title = "Drive and signals".into();
        p.structure = tag("=A1+CAB");
    })?;
    let l1 = tx.add_potential("L1", PotentialKind::Line)?;
    let l2 = tx.add_potential("L2", PotentialKind::Line)?;
    let l3 = tx.add_potential("L3", PotentialKind::Line)?;
    let feeds = [l1, l2, l3].map(|potential| {
        let mut options = PlaceOptions::new();
        options.potential = Some(potential);
        options
    });
    let feed_symbol = catalog
        .get("iec/connectivity/potential_feed")
        .expect("the feed symbol is in the library");
    let mut feed_ids = Vec::new();
    for (i, options) in feeds.iter().enumerate() {
        let placement = Placement::at(Point::new(40.0 + 8.0 * i as f64, 20.0));
        feed_ids.push(place(tx, page, feed_symbol, placement, options)?);
    }

    let drive = put(
        tx,
        catalog,
        page,
        "iec/drives/frequency_converter",
        (40.0, 40.0),
        Some("=A1+CAB-U1"),
    )?;
    for (i, feed) in feed_ids.iter().enumerate() {
        wire(tx, page, (*feed, 0), (drive, i as u16))?;
    }

    // The motor cable leaves through a terminal strip: U, V, W and PE.
    let mut motor_out = Vec::new();
    for (i, key) in [
        "iec/terminals/terminal",
        "iec/terminals/terminal",
        "iec/terminals/terminal",
        "iec/terminals/terminal_pe",
    ]
    .iter()
    .enumerate()
    {
        motor_out.push(put(
            tx,
            catalog,
            page,
            key,
            (40.0 + 8.0 * i as f64, 100.0),
            None,
        )?);
    }
    // Drive pins: L1 L2 L3 PE U V W PE2.
    for (terminal, pin) in motor_out.iter().zip([4u16, 5, 6, 7]) {
        wire(tx, page, (drive, pin), (*terminal, 0))?;
    }

    // 24 V supply, PLC and one analog input, with its field terminal.
    let supply = put(
        tx,
        catalog,
        page,
        "iec/supplies/power_supply_dc",
        (140.0, 40.0),
        Some("=A1+CAB-G1"),
    )?;
    let cpu = put(
        tx,
        catalog,
        page,
        "iec/plc/cpu",
        (200.0, 40.0),
        Some("=A1+CAB-A1"),
    )?;
    // Supply pins: L N PE L+ M. CPU pins: L+ M PE P1 P2 A B SH.
    wire(tx, page, (supply, 3), (cpu, 0))?;
    wire(tx, page, (supply, 4), (cpu, 1))?;
    let input = put(
        tx,
        catalog,
        page,
        "iec/plc/analog_input",
        (200.0, 80.0),
        Some("=A1+CAB-A2"),
    )?;
    let signal = put(
        tx,
        catalog,
        page,
        "iec/terminals/terminal",
        (120.0, 100.0),
        None,
    )?;
    wire(tx, page, (input, 0), (signal, 0))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_emc_example_builds_and_has_a_drive_a_plc_and_wires() {
        let mut doc = Document::default();
        build_emc_example(&mut doc).unwrap();
        let project = doc.project();
        for t in ["-U1", "-A1", "-A2", "-G1"] {
            assert!(
                project.devices().any(|d| d.tag.to_string().ends_with(t)),
                "no device {t}"
            );
        }
        assert!(
            project.wires().count() >= 12,
            "{} wires",
            project.wires().count()
        );
    }
}
