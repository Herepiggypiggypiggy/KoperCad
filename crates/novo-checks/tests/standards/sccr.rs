//! The short-circuit current rating calculation (simplified UL 508A SB).

use super::*;
use novo_core::*;

fn component<'a>(r: &'a standards::SccrReport, tag: &str) -> &'a standards::SccrComponent {
    r.components
        .iter()
        .find(|c| c.tag.ends_with(tag))
        .unwrap_or_else(|| panic!("no component {tag}"))
}

fn sccr(g: &Golden) -> standards::SccrReport {
    g.report().sccr.expect("an SCCR report")
}

#[test]
fn a_current_limiting_fuse_covers_the_components_behind_it() {
    let g = golden();
    let r = sccr(&g);
    // Q1 65, F1 100 (limiter, 2 kA let-through), K1 5 and F2 5 covered up to F1's 100.
    assert_eq!(r.panel_ka, Some(65.0));
    assert!(r.complete);
    assert_eq!(component(&r, "-Q1").effective_ka, Some(65.0));
    assert_eq!(component(&r, "-F1").effective_ka, Some(100.0));
    assert_eq!(component(&r, "-K1").effective_ka, Some(100.0));
    assert_eq!(component(&r, "-K1").rating_ka, Some(5.0));
    assert_eq!(component(&r, "-K1").limited_by, Some(g.f1));
    assert_eq!(r.limiting, vec![g.q1]);
    // Supply order: the disconnector first.
    assert_eq!(r.components[0].device, g.q1);
}

#[test]
fn without_a_limiter_the_weakest_component_sets_the_rating() {
    let mut g = golden();
    let f1 = g.f1;
    g.prop(f1, "current_limiting", false);
    let r = sccr(&g);
    assert_eq!(r.panel_ka, Some(5.0));
    assert_eq!(r.limiting.len(), 2, "K1 and F2");
    assert_fails(&g, "ul508a.sccr-adequate", "rating is 5 kA (limited by =A1+CAB-K1, =A1+CAB-F2), below the 10 kA available");
    // The marked 65 kA is then above the calculation.
    assert_fails(&g, "ul508a.sccr-marking", "marked rating, 65 kA, is above the calculated 5 kA");
}

#[test]
fn a_let_through_above_the_components_rating_does_not_cover_it() {
    let mut g = golden();
    let f1 = g.f1;
    g.prop(f1, "let_through_ka", 8.0);
    let r = sccr(&g);
    assert_eq!(component(&r, "-K1").effective_ka, Some(5.0));
    assert_eq!(component(&r, "-K1").limited_by, None);
    assert_eq!(r.panel_ka, Some(5.0));
}

#[test]
fn a_limiter_without_a_rating_of_its_own_covers_nothing() {
    let mut g = golden();
    let f1 = g.f1;
    g.clear(f1, "sccr_ka");
    let r = sccr(&g);
    assert!(!r.complete);
    assert_eq!(component(&r, "-K1").effective_ka, Some(5.0));
    assert_eq!(component(&r, "-F1").rating_ka, None);
    assert_eq!(component(&r, "-F1").effective_ka, None);
}

#[test]
fn a_protective_device_falls_back_to_its_interrupting_rating() {
    let mut g = golden();
    let f1 = g.f1;
    g.clear(f1, "sccr_ka");
    g.prop(f1, "breaking_capacity_ka", 50.0);
    let r = sccr(&g);
    let c = component(&r, "-F1");
    assert_eq!(c.rating_ka, Some(50.0));
    assert_eq!(c.source, standards::SccrSource::Interrupting);
    // A contactor does not: its interrupting data is not a rating.
    let k1 = g.k1;
    g.clear(k1, "sccr_ka");
    g.prop(k1, "breaking_capacity_ka", 50.0);
    assert_eq!(component(&sccr(&g), "-K1").rating_ka, None);
}

#[test]
fn a_part_can_state_the_rating() {
    let mut g = golden();
    let k1 = g.k1;
    g.clear(k1, "sccr_ka");
    g.edit(|tx, h| {
        let part = tx.add_part("Maker", "K-9")?;
        set(tx, part, "sccr_ka", 10.0);
        tx.assign_part(h.k1, part)
    });
    let c = sccr(&g).components.into_iter().find(|c| c.tag.ends_with("-K1")).unwrap();
    assert_eq!((c.rating_ka, c.source), (Some(10.0), standards::SccrSource::Marked));
}

#[test]
fn project_defaults_fill_unmarked_components_and_are_labelled() {
    let mut g = golden();
    let k1 = g.k1;
    g.clear(k1, "sccr_ka");
    assert_fails(&g, "ul508a.sccr-components", "=A1+CAB-K1 has no short-circuit rating");
    g.project_prop("standards.ul508a.default_sccr_ka.contactor", Some(5.0.into()));
    let r = sccr(&g);
    let c = component(&r, "-K1");
    assert_eq!((c.rating_ka, c.source), (Some(5.0), standards::SccrSource::Default));
    assert_passes(&g, "ul508a.sccr-components");
}

#[test]
fn unknown_ratings_leave_the_panel_rating_undetermined_unless_one_already_fails() {
    let mut g = golden();
    let (f2, q1) = (g.f2, g.q1);
    g.clear(f2, "sccr_ka");
    assert_unknown(&g, "ul508a.sccr-adequate", "panel's rating is not determined");
    // A known component below the supply fails whatever the rest is.
    g.prop(q1, "sccr_ka", 5.0);
    g.project_prop("supply.short_circuit_ka", Some(10.0.into()));
    assert_fails(&g, "ul508a.sccr-adequate", "=A1+CAB-Q1 is rated 5 kA, below the 10 kA available");
}

#[test]
fn the_available_fault_current_must_be_stated() {
    let mut g = golden();
    g.project_prop("supply.short_circuit_ka", None);
    assert_unknown(&g, "ul508a.sccr-adequate", "available fault current");
}

#[test]
fn the_marked_rating_is_required_and_not_above_the_calculation() {
    let mut g = golden();
    g.project_prop("panel.sccr_ka", Some(100.0.into()));
    assert_fails(&g, "ul508a.sccr-marking", "marked rating, 100 kA, is above the calculated 65 kA");
    g.project_prop("panel.sccr_ka", Some(10.0.into()));
    assert_passes(&g, "ul508a.sccr-marking");
    g.project_prop("panel.sccr_ka", None);
    assert_fails(&g, "ul508a.sccr-marking", "not marked: state `panel.sccr_ka` (calculated: 65 kA)");
}

#[test]
fn terminal_strips_on_power_circuits_are_components() {
    let mut g = golden();
    let (page, m1_sym, f2_sym) = (g.page, g.m1_sym, g.f2_sym);
    let wire = g.power_wires[4];
    let strip = g.edit(|tx, _| {
        tx.remove(wire)?;
        let strip = tx.add_terminal_strip(tag("=A1+CAB-X1"))?;
        let t = tx.add_terminal(strip, "1")?;
        let sym = tx.place_symbol(
            page,
            "iec/terminals/terminal",
            Placement::at(Point::new(5.0, 100.0)),
            vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 8.0)],
            SymbolRole::Terminal { terminals: vec![t] },
        )?;
        tx.update::<SymbolInstance>(sym, |s| s.bridges = vec![vec![0, 1]])?;
        tx.connect(Endpoint::pin(f2_sym, 1), Endpoint::pin(sym, 0))?;
        tx.connect(Endpoint::pin(sym, 1), Endpoint::pin(m1_sym, 0))?;
        Ok(strip)
    });
    let r = sccr(&g);
    let c = component(&r, "-X1");
    assert_eq!(c.kind, "terminal");
    assert_eq!(c.rating_ka, None);
    assert_fails(&g, "ul508a.sccr-components", "=A1+CAB-X1 has no short-circuit rating");
    g.prop(strip, "sccr_ka", 30.0);
    // The golden machine's fuse is current limiting and covers it; switch it
    // off to see the terminal's own rating.
    let f1 = g.f1;
    g.prop(f1, "current_limiting", false);
    assert_eq!(component(&sccr(&g), "-X1").effective_ka, Some(30.0));
    // The panel is still held down by the weakest component elsewhere.
    assert!(sccr(&g).panel_ka.unwrap() <= 30.0);
}

#[test]
fn control_circuit_devices_are_not_power_components() {
    let g = golden();
    let r = sccr(&g);
    assert!(r.components.iter().all(|c| !c.tag.ends_with("-F3") && !c.tag.ends_with("-S0")));
    assert_eq!(r.components.len(), 4);
}

#[test]
fn a_project_with_no_power_components_has_no_calculation() {
    let mut doc = Document::default();
    doc.transact("empty", |tx| {
        tx.add_page("1")?;
        standards::select(tx, &[standards::StandardSet::Ul508a])
    })
    .unwrap();
    assert!(standards::compliance(doc.project()).sccr.is_none());
}
