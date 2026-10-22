//! The text stress matrix: pages that put every kind of text in every
//! awkward place, so `text_conflicts` can find the collisions and a
//! regression test can hold the count down.
//!
//! One project holds all of it, one page per case, named `Sxx case`, so the same
//! file serves the headless finder, the QA scenarios (screenshots) and the
//! exporters. `matrix()` builds it; every case is a function so a fix thread
//! can add one beside the thing it fixes.

use novo_core::{
    CableCore, DeviceTag, Document, Endpoint, PageId, Placement, Point, Result, Rotation, SymbolId,
    TerminalKind, Tx, Wire,
};
use novo_editor::catalog::Catalog;
use novo_editor::edit::{self, PlaceOptions};

/// The eight placements of a symbol: four rotations, each plain and mirrored.
pub const ORIENTATIONS: [(Rotation, bool); 8] = [
    (Rotation::R0, false),
    (Rotation::R90, false),
    (Rotation::R180, false),
    (Rotation::R270, false),
    (Rotation::R0, true),
    (Rotation::R90, true),
    (Rotation::R180, true),
    (Rotation::R270, true),
];

pub fn orientation_name((r, m): (Rotation, bool)) -> String {
    let deg = match r {
        Rotation::R0 => 0,
        Rotation::R90 => 90,
        Rotation::R180 => 180,
        Rotation::R270 => 270,
    };
    format!("r{deg}{}", if m { "m" } else { "" })
}

/// A long Dutch description and tag material for the "long text" cases.
pub const LONG_DESCRIPTIONS: [&str; 4] = [
    "Hoofdschakelaar transportband sectie 3 noodstop keten",
    "Frequentieregelaar pomp P101 met remweerstand",
    "Thermische beveiliging motor aandrijving",
    "Veiligheidsrelais kat. 4 PL e deurcontact",
];

pub struct Builder<'a, 'b> {
    pub tx: &'a mut Tx<'b>,
    pub catalog: &'a Catalog,
}

impl Builder<'_, '_> {
    pub fn page(&mut self, name: &str) -> Result<PageId> {
        self.tx.add_page(name)
    }

    /// Places `symbol` at `(x, y)`; a refusal (a second main function of one
    /// device, say) is not an error for a stress page, the cell stays empty.
    pub fn put(
        &mut self,
        page: PageId,
        symbol: &str,
        at: (f64, f64),
        how: (Rotation, bool),
        options: &PlaceOptions,
    ) -> Option<SymbolId> {
        let sym = self.catalog.get(symbol)?;
        let placement = Placement {
            origin: Point::new(at.0, at.1),
            rotation: how.0,
            mirrored: how.1,
        };
        edit::place(self.tx, page, sym, placement, options).ok()
    }

    pub fn plain(
        &mut self,
        page: PageId,
        symbol: &str,
        at: (f64, f64),
        how: (Rotation, bool),
    ) -> Option<SymbolId> {
        self.put(page, symbol, at, how, &PlaceOptions::new())
    }
}

/// Every symbol of the library, one per cell, `cols` across, in one
/// orientation per page; cells are far enough apart that anything flagged
/// is the symbol's own text running into its own drawing.
fn every_symbol(b: &mut Builder<'_, '_>, how: (Rotation, bool), tag: &str) -> Result<()> {
    let ids: Vec<String> = b.catalog.library().iter().map(|s| s.id.clone()).collect();
    const PER_PAGE: usize = 48;
    for (n, chunk) in ids.chunks(PER_PAGE).enumerate() {
        let page = b.page(&format!("{tag} {} part {}", orientation_name(how), n + 1))?;
        for (k, id) in chunk.iter().enumerate() {
            let (col, row) = (k % 8, k / 8);
            b.plain(
                page,
                id,
                (30.0 + col as f64 * 44.0, 30.0 + row as f64 * 40.0),
                how,
            );
        }
    }
    Ok(())
}

/// The same symbols packed at a pitch typical of a busy sheet.
fn dense_symbols(b: &mut Builder<'_, '_>, pitch: f64) -> Result<()> {
    let ids: Vec<String> = b.catalog.library().iter().map(|s| s.id.clone()).collect();
    for (n, chunk) in ids.chunks(60).enumerate() {
        let page = b.page(&format!("dense {pitch:.0}mm part {}", n + 1))?;
        for (k, id) in chunk.iter().enumerate() {
            let (col, row) = (k % 12, k / 12);
            b.plain(
                page,
                id,
                (20.0 + col as f64 * pitch, 24.0 + row as f64 * pitch),
                (Rotation::R0, false),
            );
        }
    }
    Ok(())
}

/// Terminals in every awkward arrangement.
fn terminal_pages(b: &mut Builder<'_, '_>) -> Result<()> {
    let term = |k: &str| format!("iec/terminals/{k}");
    let strips: [(&str, &[&str]); 1] = [("terminals", &["terminal", "terminal_pe", "terminal_n"])];
    let _ = strips;
    // Resolve what terminal symbols the library has.
    let ids: Vec<String> = b
        .catalog
        .library()
        .iter()
        .filter(|s| s.id.contains("terminal"))
        .map(|s| s.id.clone())
        .collect();
    let _ = term;
    let plain = ids
        .iter()
        .find(|i| i.ends_with("/terminal"))
        .cloned()
        .unwrap_or_else(|| ids[0].clone());
    let pe = ids.iter().find(|i| i.contains("pe")).cloned();

    // Strip in a row: pitch 4, 5, 6, 8, 10 mm, each in the four rotations.
    for (title, how) in [
        ("row", (Rotation::R0, false)),
        ("row rotated", (Rotation::R90, false)),
        ("row mirrored", (Rotation::R0, true)),
        ("row rotated mirrored", (Rotation::R270, true)),
    ] {
        let page = b.page(&format!("terminals {title}"))?;
        for (r, pitch) in [4.0, 5.0, 6.0, 8.0, 10.0].into_iter().enumerate() {
            let y = 30.0 + r as f64 * 40.0;
            for k in 0..14 {
                let x = 30.0 + k as f64 * pitch;
                let horizontal = !matches!(how.0, Rotation::R90 | Rotation::R270);
                let at = if horizontal { (x, y) } else { (y + 200.0, x) };
                b.plain(page, &plain, at, how);
            }
        }
    }

    // A terminal placed left of (or above) the strip's first terminal: the strip name moves
    // to the new first one. Every orientation, plain and mirrored, strips built both ways.
    for (title, how) in [
        ("0", (Rotation::R0, false)),
        ("0 mirrored", (Rotation::R0, true)),
        ("90", (Rotation::R90, false)),
        ("90 mirrored", (Rotation::R90, true)),
        ("180", (Rotation::R180, false)),
        ("270", (Rotation::R270, false)),
    ] {
        let page = b.page(&format!("terminals placed left of strip {title}"))?;
        let vertical = matches!(how.0, Rotation::R90 | Rotation::R270);
        let pos = |along: f64, lane: f64| {
            if vertical {
                (60.0 + lane, 30.0 + along)
            } else {
                (30.0 + along, 60.0 + lane)
            }
        };
        // Strip 1: placed left to right (the usual way).
        for k in 0..8 {
            b.plain(page, &plain, pos(k as f64 * 6.0, 0.0), how);
        }
        // Strip 2: placed right to left, so every new terminal lands left of the strip.
        for k in (0..8).rev() {
            b.plain(page, &plain, pos(k as f64 * 6.0, 50.0), how);
        }
        // Strip 3: the first terminal, then two placed left of it at once-spaced 6 mm, then PE.
        b.plain(page, &plain, pos(24.0, 100.0), how);
        b.plain(page, &plain, pos(18.0, 100.0), how);
        b.plain(page, &plain, pos(12.0, 100.0), how);
        if let Some(pe) = &pe {
            b.plain(page, pe, pos(6.0, 100.0), how);
        }
    }
    // Strip of 12 with PE and N mixed in, and a second strip directly below,
    // a third below that, and a 2-level terminal.
    let page = b.page("terminals PE N mixed and stacked strips")?;
    for row in 0..4 {
        for k in 0..12 {
            let x = 40.0 + k as f64 * 6.0;
            let y = 40.0 + row as f64 * 12.0;
            let id = if let (Some(pe), true) = (&pe, k % 5 == 4) {
                pe.clone()
            } else {
                plain.clone()
            };
            b.plain(page, &id, (x, y), (Rotation::R0, false));
        }
    }
    // Long names: rename terminals to long strings and strip to a long tag.
    let page = b.page("terminals long names")?;
    let mut made = Vec::new();
    for k in 0..8 {
        if let Some(s) = b.plain(
            page,
            &plain,
            (40.0 + k as f64 * 6.0, 50.0),
            (Rotation::R0, false),
        ) {
            made.push(s);
        }
    }
    let names = [
        "L1.1",
        "24V-DC",
        "PE-HOOFD",
        "1234567",
        "N",
        "X-12",
        "A1/A2",
        "ABCDEFGHIJ",
    ];
    for (s, n) in made.iter().zip(names) {
        if let Some(inst) = b.tx.project().symbol(*s)
            && let novo_core::SymbolRole::Terminal { terminals } = &inst.role
        {
            let t = terminals[0];
            let _ = b.tx.update::<novo_core::Terminal>(t, |t| t.name = n.into());
        }
    }
    // Terminal rows at several strips next to each other horizontally.
    let page = b.page("terminals side by side strips")?;
    for strip in 0..4 {
        for k in 0..6 {
            b.plain(
                page,
                &plain,
                (30.0 + strip as f64 * 70.0 + k as f64 * 6.0, 60.0),
                (Rotation::R0, false),
            );
        }
    }
    let _ = TerminalKind::Feedthrough;
    Ok(())
}

/// Wires of every length, direction and number, with numbers set.
fn wire_pages(b: &mut Builder<'_, '_>) -> Result<()> {
    let contact = "iec/contacts/no";
    let page = b.page("wire numbers lengths")?;
    // Vertical chain of contacts joined by wires of growing length: 2, 4, 6, 8, 12, 20, 40 mm
    // between pins of two symbols placed that far apart.
    for (col, len) in [4.0, 8.0, 12.0, 16.0, 24.0, 40.0, 80.0]
        .into_iter()
        .enumerate()
    {
        let x = 30.0 + col as f64 * 26.0;
        let a = b.plain(page, contact, (x, 30.0), (Rotation::R0, false));
        let c = b.plain(page, contact, (x, 30.0 + 16.0 + len), (Rotation::R0, false));
        if let (Some(a), Some(c)) = (a, c)
            && let Ok(w) = b.tx.connect(Endpoint::pin(a, 1), Endpoint::pin(c, 0))
        {
            let _ =
                b.tx.update::<Wire>(w, |w| w.number = Some(format!("{}", 100 + col * 11)));
        }
    }
    // Horizontal wires between rotated contacts.
    for (row, len) in [4.0, 8.0, 16.0, 40.0].into_iter().enumerate() {
        let y = 160.0 + row as f64 * 24.0;
        let a = b.plain(page, contact, (30.0, y), (Rotation::R90, false));
        let c = b.plain(
            page,
            contact,
            (30.0 + 16.0 + len, y),
            (Rotation::R90, false),
        );
        if let (Some(a), Some(c)) = (a, c)
            && let Ok(w) = b.tx.connect(Endpoint::pin(a, 1), Endpoint::pin(c, 0))
        {
            let _ =
                b.tx.update::<Wire>(w, |w| w.number = Some(format!("L1.{}", 20 + row)));
        }
    }
    // Wires carrying long numbers.
    let page = b.page("wire numbers long text")?;
    for (col, text) in [
        "1",
        "12",
        "L1-2-3",
        "24V-DC-0042",
        "WIRE-NUMBER-LONG-123456",
    ]
    .into_iter()
    .enumerate()
    {
        let x = 30.0 + col as f64 * 40.0;
        let a = b.plain(page, contact, (x, 30.0), (Rotation::R0, false));
        let c = b.plain(page, contact, (x, 70.0), (Rotation::R0, false));
        if let (Some(a), Some(c)) = (a, c)
            && let Ok(w) = b.tx.connect(Endpoint::pin(a, 1), Endpoint::pin(c, 0))
        {
            let _ = b.tx.update::<Wire>(w, |w| w.number = Some(text.into()));
        }
    }
    // Bundles: parallel wires 4, 3, 2 mm apart each with a number.
    let page = b.page("wire bundles")?;
    for (blk, pitch) in [8.0, 6.0, 4.0, 3.0, 2.0].into_iter().enumerate() {
        for k in 0..6 {
            let x = 30.0 + blk as f64 * 60.0 + k as f64 * pitch;
            let a = b.plain(page, contact, (x, 30.0), (Rotation::R0, false));
            let c = b.plain(page, contact, (x, 80.0), (Rotation::R0, false));
            if let (Some(a), Some(c)) = (a, c)
                && let Ok(w) = b.tx.connect(Endpoint::pin(a, 1), Endpoint::pin(c, 0))
            {
                let _ =
                    b.tx.update::<Wire>(w, |w| w.number = Some(format!("{}", 10 + blk * 10 + k)));
            }
        }
    }
    // Crossing wires: one horizontal run crossing three vertical ones at the middle.
    let page = b.page("wires crossing")?;
    for k in 0..3 {
        let x = 60.0 + k as f64 * 8.0;
        let a = b.plain(page, contact, (x, 30.0), (Rotation::R0, false));
        let c = b.plain(page, contact, (x, 90.0), (Rotation::R0, false));
        if let (Some(a), Some(c)) = (a, c)
            && let Ok(w) = b.tx.connect(Endpoint::pin(a, 1), Endpoint::pin(c, 0))
        {
            let _ =
                b.tx.update::<Wire>(w, |w| w.number = Some(format!("{}", 200 + k)));
        }
    }
    let a = b.plain(page, contact, (30.0, 62.0), (Rotation::R90, false));
    let c = b.plain(page, contact, (110.0, 62.0), (Rotation::R90, false));
    if let (Some(a), Some(c)) = (a, c)
        && let Ok(w) = b.tx.connect(Endpoint::pin(a, 1), Endpoint::pin(c, 0))
    {
        let _ = b.tx.update::<Wire>(w, |w| w.number = Some("300".into()));
    }
    Ok(())
}

/// Potential feeds in a tight row and with long names.
fn feed_pages(b: &mut Builder<'_, '_>) -> Result<()> {
    let feed = b
        .catalog
        .library()
        .iter()
        .find(|s| s.id.contains("potential_feed") && !s.id.ends_with("_pe"))
        .map(|s| s.id.clone());
    let Some(feed) = feed else { return Ok(()) };
    for (title, pitch) in [("8mm", 8.0), ("6mm", 6.0), ("4mm", 4.0), ("20mm", 20.0)] {
        let page = b.page(&format!("feeds pitch {title}"))?;
        for k in 0..12 {
            b.plain(
                page,
                &feed,
                (30.0 + k as f64 * pitch, 40.0),
                (Rotation::R0, false),
            );
        }
        for k in 0..12 {
            b.plain(
                page,
                &feed,
                (30.0 + k as f64 * pitch, 90.0),
                (Rotation::R90, false),
            );
        }
    }
    Ok(())
}

/// Cables with many cores attached to wires, long tags.
fn cable_pages(b: &mut Builder<'_, '_>) -> Result<()> {
    let contact = "iec/contacts/no";
    let page = b.page("cables and cores")?;
    let mut wires = Vec::new();
    for k in 0..6 {
        let x = 30.0 + k as f64 * 8.0;
        let a = b.plain(page, contact, (x, 30.0), (Rotation::R0, false));
        let c = b.plain(page, contact, (x, 80.0), (Rotation::R0, false));
        if let (Some(a), Some(c)) = (a, c)
            && let Ok(w) = b.tx.connect(Endpoint::pin(a, 1), Endpoint::pin(c, 0))
        {
            let _ =
                b.tx.update::<Wire>(w, |w| w.number = Some(format!("{}", 400 + k)));
            wires.push(w);
        }
    }
    let names = ["BN", "BU", "BK", "GY", "GNYE", "WH"];
    let cores: Vec<CableCore> = names.iter().map(|n| CableCore::new(*n)).collect();
    if let Ok(tag) = DeviceTag::parse("-W1234567")
        && let Ok(cable) = b.tx.add_cable(tag, cores)
    {
        for (i, w) in wires.iter().enumerate() {
            let _ = b.tx.assign_core(cable, i, Some(*w));
        }
    }
    // A second cable across a longer run.
    let mut wires = Vec::new();
    for k in 0..4 {
        let x = 160.0 + k as f64 * 5.0;
        let a = b.plain(page, contact, (x, 30.0), (Rotation::R0, false));
        let c = b.plain(page, contact, (x, 120.0), (Rotation::R0, false));
        if let (Some(a), Some(c)) = (a, c)
            && let Ok(w) = b.tx.connect(Endpoint::pin(a, 1), Endpoint::pin(c, 0))
        {
            wires.push(w);
        }
    }
    let cores: Vec<CableCore> = ["1", "2", "3", "PE"]
        .iter()
        .map(|n| CableCore::new(*n))
        .collect();
    if let Ok(tag) = DeviceTag::parse("-W2")
        && let Ok(cable) = b.tx.add_cable(tag, cores)
    {
        for (i, w) in wires.iter().enumerate() {
            let _ = b.tx.assign_core(cable, i, Some(*w));
        }
    }
    Ok(())
}

/// Long tags and descriptions on ordinary devices.
fn long_text_pages(b: &mut Builder<'_, '_>) -> Result<()> {
    let page = b.page("long tags and descriptions")?;
    let kinds = [
        "iec/contactors/coil",
        "iec/switches/pushbutton_no",
        "iec/motors/motor_3ph",
        "iec/protection/fuse",
    ];
    let ids: Vec<String> = b.catalog.library().iter().map(|s| s.id.clone()).collect();
    for (k, id) in ids
        .iter()
        .filter(|i| kinds.iter().any(|k| i == k) || i.contains("coil") || i.contains("motor"))
        .take(10)
        .enumerate()
    {
        let tag = [
            "=PLANT-LONG+CABINET-01-12",
            "=A1+K3",
            "+SCHAKELKAST",
            "-Q1234567890",
        ][k % 4];
        let opts = PlaceOptions {
            parent: DeviceTag::parse(&format!("{tag}-F{}", 100 + k)).ok(),
            ..PlaceOptions::new()
        };
        let sym = b.put(
            page,
            id,
            (40.0 + (k % 5) as f64 * 70.0, 40.0 + (k / 5) as f64 * 60.0),
            (Rotation::R0, false),
            &opts,
        );
        if let Some(sym) = sym
            && let Some(inst) = b.tx.project().symbol(sym)
            && let novo_core::SymbolRole::Device { device, .. } = inst.role
        {
            let d = LONG_DESCRIPTIONS[k % LONG_DESCRIPTIONS.len()].to_string();
            let _ =
                b.tx.update::<novo_core::Device>(device, |dev| dev.description = d);
        }
    }
    Ok(())
}

/// Devices placed on top of each other and half over each other, the way a
/// macro dropped on existing content lands.
fn overlap_pages(b: &mut Builder<'_, '_>) -> Result<()> {
    let page = b.page("symbols on top of each other")?;
    let ids = [
        "iec/contactors/coil",
        "iec/contacts/no",
        "iec/contacts/nc",
        "iec/switches/pushbutton_no",
    ];
    for (k, off) in [0.0, 2.0, 4.0, 8.0].into_iter().enumerate() {
        for (i, id) in ids.iter().enumerate() {
            b.plain(
                page,
                id,
                (40.0 + k as f64 * 50.0 + off, 40.0 + i as f64 * 30.0 + off),
                (Rotation::R0, false),
            );
        }
    }
    Ok(())
}

/// A coil with contact images on other pages: the cross-reference table next to the coil
/// grows with the number of contacts.
fn xref_pages(b: &mut Builder<'_, '_>) -> Result<()> {
    let coil = "iec/coils/contactor";
    let no = "iec/contacts/no";
    let nc = "iec/contacts/nc";
    for (n, contacts) in [1usize, 3, 6, 12].into_iter().enumerate() {
        let tag = format!("-K9{n}0");
        let opts = PlaceOptions {
            parent: DeviceTag::parse(&tag).ok(),
            ..PlaceOptions::new()
        };
        let a = b.page(&format!("xref coil with {contacts} contacts"))?;
        b.put(a, coil, (60.0, 50.0), (Rotation::R0, false), &opts);
        // Neighbours of the coil, as on a real control page.
        b.plain(a, "iec/contacts/no", (110.0, 50.0), (Rotation::R0, false));
        b.plain(a, "iec/coils/relay", (160.0, 50.0), (Rotation::R0, false));
        let c = b.page(&format!("xref contacts of {contacts}"))?;
        for k in 0..contacts {
            let id = if k % 3 == 2 { nc } else { no };
            b.put(
                c,
                id,
                (30.0 + (k % 6) as f64 * 40.0, 40.0 + (k / 6) as f64 * 50.0),
                (Rotation::R0, false),
                &opts,
            );
        }
    }
    Ok(())
}

/// Free texts and attached texts, including long Dutch ones, next to devices.
fn free_text_pages(b: &mut Builder<'_, '_>) -> Result<()> {
    let page = b.page("free text next to devices")?;
    let at = |x: f64, y: f64| novo_core::TextPlacement {
        offset: Point::new(x, y),
        ..Default::default()
    };
    let texts = [
        "Noodstop keten sectie 3",
        "Let op: spanning blijft aanwezig na uitschakelen hoofdschakelaar",
        "24 V DC",
        "Frequentieregelaar aandrijving transportband",
    ];
    for (k, t) in texts.iter().enumerate() {
        let x = 30.0 + k as f64 * 70.0;
        let s = b
            .plain(
                page,
                "iec/contactors/coil",
                (x, 50.0),
                (Rotation::R0, false),
            )
            .or_else(|| {
                b.plain(
                    page,
                    "iec/coils/contactor",
                    (x, 50.0),
                    (Rotation::R0, false),
                )
            });
        let _ = b.tx.add_text(
            page,
            novo_core::TextContent::Literal { text: (*t).into() },
            at(x + 6.0, 54.0),
            s.map(novo_core::AnyId::from),
        );
        let _ = b.tx.add_text(
            page,
            novo_core::TextContent::Literal { text: (*t).into() },
            at(x - 6.0, 38.0),
            None,
        );
    }
    Ok(())
}

/// Content right at the frame, in the corners and over the title block.
fn frame_pages(b: &mut Builder<'_, '_>) -> Result<()> {
    let page = b.page("content at the frame and title block")?;
    for (k, (x, y)) in [
        (2.0, 2.0),
        (6.0, 6.0),
        (200.0, 6.0),
        (330.0, 6.0),
        (6.0, 250.0),
        (330.0, 250.0),
        (300.0, 270.0),
        (380.0, 280.0),
    ]
    .into_iter()
    .enumerate()
    {
        let id = [
            "iec/contacts/no",
            "iec/coils/contactor",
            "iec/switches/pushbutton_no",
        ][k % 3];
        b.plain(page, id, (x, y), (Rotation::R0, false));
    }
    Ok(())
}

/// Builds the whole matrix into `doc` as one undo step. Returns the pages.
pub fn matrix(doc: &mut Document) -> Result<Vec<PageId>> {
    let catalog = Catalog::builtin();
    let (_, _) = doc.transact("Text stress matrix", |tx| {
        let mut b = Builder {
            tx,
            catalog: &catalog,
        };
        for how in ORIENTATIONS {
            every_symbol(&mut b, how, "symbols")?;
        }
        for pitch in [32.0, 24.0, 16.0, 10.0] {
            dense_symbols(&mut b, pitch)?;
        }
        terminal_pages(&mut b)?;
        wire_pages(&mut b)?;
        feed_pages(&mut b)?;
        cable_pages(&mut b)?;
        long_text_pages(&mut b)?;
        overlap_pages(&mut b)?;
        xref_pages(&mut b)?;
        free_text_pages(&mut b)?;
        frame_pages(&mut b)?;
        Ok(())
    })?;
    Ok(doc
        .project()
        .pages_in_order()
        .iter()
        .map(|p| p.id)
        .collect())
}
