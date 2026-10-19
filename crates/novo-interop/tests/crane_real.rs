//! Opt-in checks on a real crane configurator export. They run only when
//! `KOPERCAD_CRANE_EPJ` names a project export (`.epj`); otherwise each test
//! returns at once and passes, so CI and anyone without the file stay green.
//! The file is the user's own and never goes into the repository.

use std::io::BufReader;
use std::path::PathBuf;

use novo_interop::eplan::pxf::{PageClass, read_pxf};

fn real_file() -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var_os("KOPERCAD_CRANE_EPJ")?);
    p.is_file().then_some(p)
}

#[test]
fn the_reader_scopes_library_graphics_away_from_pages() {
    let Some(path) = real_file() else { return };
    let f = read_pxf(BufReader::new(std::fs::File::open(path).unwrap())).unwrap();
    let schematic = f
        .pages
        .iter()
        .filter(|p| p.class() == PageClass::Schematic)
        .count();
    eprintln!(
        "pages {} schematic {} devices {} cores {} ips {} nodes {} texts {} structures {} library {} symbols {} functions {}",
        f.pages.len(),
        schematic,
        f.devices.len(),
        f.cores.len(),
        f.interruptions.len(),
        f.nodes.len(),
        f.texts.len(),
        f.structures.len(),
        f.library_objects.values().sum::<usize>(),
        f.library.symbol_count(),
        f.library.function_count()
    );
    eprintln!("project {:?}", f.project);
    for p in f.pages.iter().take(40) {
        eprintln!(
            "{} {:?} sym {} nodes {} cells {}",
            p.label(),
            p.class(),
            p.symbols,
            p.nodes,
            p.report_cells
        );
    }
}

#[test]
fn probe_symbols() {
    use novo_interop::eplan::symbols::{EplanSymbol, SymbolMapper};
    use novo_symbols::Pack;
    use std::collections::BTreeMap;
    let Some(path) = real_file() else { return };
    let f = read_pxf(BufReader::new(std::fs::File::open(path).unwrap())).unwrap();
    let mapper = SymbolMapper::new(Pack::Iec);
    type Key = (i64, i64, (i64, i64, i64), i64);
    let mut groups: BTreeMap<Key, Vec<usize>> = BTreeMap::new();
    for (i, d) in f.devices.iter().enumerate() {
        let Some(s) = d.symbol else { continue };
        groups
            .entry((
                d.function_def,
                d.library_id,
                (s.number, s.variant, s.library),
                d.variant,
            ))
            .or_default()
            .push(i);
    }
    for (k, idx) in &groups {
        let d = &f.devices[idx[0]];
        let found = f.library.symbol(k.0, k.1, k.2, k.3, d.pins.len());
        let func = f.library.function(k.2);
        let fname = func
            .map(|x| x.name.get("en_US").to_string())
            .unwrap_or_default();
        let sname = found
            .map(|(s, q)| {
                format!(
                    "{:?} {} ports {}",
                    q,
                    s.name.get("en_US"),
                    s.variants[&(k.3 as u32)].len()
                )
            })
            .unwrap_or("NOT FOUND".into());
        let es = EplanSymbol {
            key: None,
            function: Some(fname.clone()),
            letters: Some(d.tag.letters.clone()),
            pins: d.pins.clone(),
        };
        let r = mapper.map(&es);
        eprintln!(
            "{:?} x{} | fn '{}' | sym {} | letters {} pins {:?} => {:?} {:?} unmapped {:?}",
            k,
            idx.len(),
            fname,
            sname,
            d.tag.letters,
            d.pins,
            r.symbol_id(),
            r.basis,
            r.pins.unmapped
        );
    }
}

#[test]
fn probe_orphan_cores() {
    let Some(path) = real_file() else { return };
    let f = read_pxf(BufReader::new(std::fs::File::open(path).unwrap())).unwrap();
    use std::collections::BTreeSet;
    let cable_ids: BTreeSet<_> = f
        .devices
        .iter()
        .filter(|d| d.role == novo_interop::eplan::pxf::DeviceRole::Cable)
        .filter_map(|d| d.id)
        .collect();
    for c in &f.cores {
        let ok = c.cable_ref.is_some_and(|r| cable_ids.contains(&r));
        if !ok {
            eprintln!(
                "orphan core {:?} ref {:?} num {} page {:?}",
                c.cable.full(),
                c.cable_ref,
                c.number,
                c.page
            );
        }
    }
}
