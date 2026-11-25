//! Verification harness for imported EPLAN projects (opt-in, private data).
//!
//! `KOPERCAD_CRANE_EPJ=<export.epj>` names a project export;
//! `KOPERCAD_VERIFY_OUT=<dir>` (default: the system temp dir) receives
//! `verify.md` (ranked mismatch classes, per-page table) and `verify.json`
//! (every finding, for the side-by-side diff). Without the variable each test
//! returns at once, so CI stays green and no customer file ever enters the
//! repository. Warnings are never filtered: every finding of
//! `novo_checks::check` is classified against EPLAN's own net rules
//! ([`novo_interop::eplan::nets`]) and counted.
//!
//! The synthetic test at the bottom runs the same classifier on a small
//! generated drawing, so the classes themselves are covered in CI.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::PathBuf;

use novo_checks::{Problem, ProblemKind, check_with};
use novo_core::{Document, Endpoint, PageId, Project, SymbolId, SymbolInstance, SymbolRole};
use novo_interop::eplan::nets::{PinKey, ReconNets};
use novo_interop::eplan::pxf::PxfPlan;

/// One point of the drawing, with both worlds' view of it.
#[derive(Debug, Clone)]
pub struct Pt {
    pub key: PinKey,
    pub symbol: SymbolId,
    pub pin: u16,
    pub page: PageId,
    pub at: (i64, i64),
    pub recon_net: Option<usize>,
    pub net: Option<novo_core::NetId>,
}

/// What a free (unwired) point is, by how EPLAN's rules see it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum FreeClass {
    /// Alone in EPLAN's nets too: a genuinely open end (spare point or a
    /// device pin left free in the drawing).
    OpenInBoth,
    /// EPLAN puts it in a net of several points; KoperCAD does not join it.
    /// An importer fault (a wire is missing).
    JoinedByEplanOnly,
    /// Touches another point; EPLAN keeps them in different nets (stacked
    /// terminals, coincident pins of different potentials).
    TouchesSeparateNets,
    /// Touches another point of its own EPLAN net but KoperCAD has no wire
    /// (an importer fault, or a symbol bridge KoperCAD does not model).
    TouchesSameNet,
    /// The file point is not known to the reconstruction.
    Unmapped,
}

impl FreeClass {
    pub fn label(&self) -> &'static str {
        match self {
            FreeClass::OpenInBoth => "open end in EPLAN too",
            FreeClass::JoinedByEplanOnly => "EPLAN joins it, KoperCAD does not",
            FreeClass::TouchesSeparateNets => "touches a point EPLAN keeps in another net",
            FreeClass::TouchesSameNet => "touches a point of its own EPLAN net, no wire",
            FreeClass::Unmapped => "not in EPLAN reconstruction",
        }
    }
}

type GridKey = (PageId, i64, i64);

fn grid(page: PageId, p: novo_core::Point) -> GridKey {
    (
        page,
        (p.x * 1000.0).round() as i64,
        (p.y * 1000.0).round() as i64,
    )
}

/// Every drawn point with its two net memberships.
pub fn points(plan: &PxfPlan, project: &Project) -> Vec<Pt> {
    let mut symbols: Vec<&SymbolInstance> = project.symbols().collect();
    symbols.sort_by_key(|s| s.id);
    assert_eq!(
        symbols.len(),
        plan.drawn_symbol_count(),
        "symbols of a fresh import are the drawn ones, in order"
    );
    let netlist = project.netlist();
    let recon: &ReconNets = plan.recon();
    let mut out = Vec::new();
    for (key, si, pin) in plan.drawn_pins() {
        let sym = symbols[si];
        debug_assert_eq!(plan.drawn_symbol_key(si), Some(sym.symbol.0.as_str()));
        let Some(p) = sym.pins.get(pin as usize) else {
            continue;
        };
        let at = grid(sym.page, sym.placement.apply(p.offset));
        out.push(Pt {
            key: key.clone(),
            symbol: sym.id,
            pin,
            page: sym.page,
            at: (at.1, at.2),
            recon_net: recon.net_of(key),
            net: netlist.net_of(Endpoint::pin(sym.id, pin)),
        });
    }
    out
}

/// Classifies every pin no wire ends on (the `unconnected_pin` population).
pub fn classify_free(
    plan: &PxfPlan,
    project: &Project,
    pts: &[Pt],
) -> Vec<(Pt, FreeClass, Vec<PinKey>)> {
    let wired: BTreeSet<(SymbolId, u16)> = project
        .wires()
        .flat_map(|w| w.ends())
        .filter_map(|e| e.as_pin().map(|p| (p.symbol, p.pin)))
        .collect();
    let recon = plan.recon();
    let mut size: BTreeMap<usize, usize> = BTreeMap::new();
    for p in pts {
        if let Some(n) = p.recon_net {
            *size.entry(n).or_default() += 1;
        }
    }
    let mut at: BTreeMap<GridKey, Vec<&Pt>> = BTreeMap::new();
    for p in pts {
        at.entry((p.page, p.at.0, p.at.1)).or_default().push(p);
    }
    let _ = recon;
    let mut out = Vec::new();
    for p in pts {
        if wired.contains(&(p.symbol, p.pin)) {
            continue;
        }
        let others: Vec<&Pt> = at[&(p.page, p.at.0, p.at.1)]
            .iter()
            .copied()
            .filter(|o| !(o.symbol == p.symbol && o.pin == p.pin))
            .collect();
        let class = match p.recon_net {
            None => FreeClass::Unmapped,
            Some(n) if !others.is_empty() => {
                if others.iter().any(|o| o.recon_net == Some(n)) {
                    FreeClass::TouchesSameNet
                } else {
                    FreeClass::TouchesSeparateNets
                }
            }
            Some(n) if size.get(&n).copied().unwrap_or(0) > 1 => FreeClass::JoinedByEplanOnly,
            Some(_) => FreeClass::OpenInBoth,
        };
        out.push((
            p.clone(),
            class,
            others.iter().map(|o| o.key.clone()).collect(),
        ));
    }
    out
}

/// A net-level comparison: how KoperCAD's nets partition the points EPLAN's
/// rules put in each net.
#[derive(Debug, Default)]
pub struct NetDiff {
    pub eplan_nets: usize,
    pub kopercad_nets: usize,
    /// EPLAN nets whose points KoperCAD splits over several nets.
    pub split: Vec<(usize, usize)>,
    /// KoperCAD nets that join points of several EPLAN nets.
    pub merged: Vec<(usize, usize)>,
}

pub fn net_diff(pts: &[Pt]) -> NetDiff {
    let mut by_recon: BTreeMap<usize, BTreeSet<Option<novo_core::NetId>>> = BTreeMap::new();
    let mut by_net: BTreeMap<novo_core::NetId, BTreeSet<usize>> = BTreeMap::new();
    for p in pts {
        let Some(r) = p.recon_net else { continue };
        by_recon.entry(r).or_default().insert(p.net);
        if let Some(n) = p.net {
            by_net.entry(n).or_default().insert(r);
        }
    }
    let mut d = NetDiff {
        eplan_nets: by_recon.len(),
        kopercad_nets: by_net.len(),
        ..NetDiff::default()
    };
    for (r, nets) in by_recon {
        // Points in no KoperCAD net count as one more part.
        if nets.len() > 1 {
            d.split.push((r, nets.len()));
        }
    }
    for (n, rs) in by_net {
        if rs.len() > 1 {
            d.merged.push((n.0 as usize, rs.len()));
        }
    }
    d
}

fn tally(problems: &[Problem]) -> BTreeMap<&'static str, usize> {
    let mut m = BTreeMap::new();
    for p in problems {
        *m.entry(p.kind.key()).or_default() += 1;
    }
    m
}

/// Everything the real-file test and the synthetic test print.
pub fn report(plan: &PxfPlan, project: &Project) -> (String, String) {
    let netlist = project.netlist();
    let problems = check_with(project, &netlist);
    let pts = points(plan, project);
    let free = classify_free(plan, project, &pts);
    let diff = net_diff(&pts);
    let mut md = String::new();
    let _ = writeln!(md, "# EPLAN verification\n");
    let _ = writeln!(
        md,
        "points {} | EPLAN nets {} | KoperCAD nets {} | EPLAN nets split by KoperCAD {} | KoperCAD nets joining several EPLAN nets {}\n",
        pts.len(),
        diff.eplan_nets,
        diff.kopercad_nets,
        diff.split.len(),
        diff.merged.len()
    );
    if !diff.merged.is_empty() {
        let _ = writeln!(
            md,
            "KoperCAD nets that join several EPLAN nets (a short in the import):\n"
        );
        for (net, _) in &diff.merged {
            let mut groups: BTreeMap<usize, Vec<String>> = BTreeMap::new();
            for p in pts
                .iter()
                .filter(|p| p.net.is_some_and(|n| n.0 as usize == *net))
            {
                let (Some(r), Some(sym)) = (p.recon_net, project.symbol(p.symbol)) else {
                    continue;
                };
                let page = project.page(p.page).map_or("?", |pg| pg.name.as_str());
                groups.entry(r).or_default().push(format!(
                    "page {page} {} pin {}",
                    sym.symbol.0, sym.pins[p.pin as usize].name
                ));
            }
            let _ = writeln!(md, "- net {net}:");
            for (r, list) in groups {
                let _ = writeln!(
                    md,
                    "  - EPLAN net {r}: {}",
                    list.iter().take(3).cloned().collect::<Vec<_>>().join("; ")
                );
            }
        }
        let _ = writeln!(md);
    }
    let _ = writeln!(md, "## Check findings (none filtered)\n");
    for (k, n) in tally(&problems) {
        let _ = writeln!(md, "- {k}: {n}");
    }
    let mut classes: BTreeMap<FreeClass, usize> = BTreeMap::new();
    for (_, c, _) in &free {
        *classes.entry(c.clone()).or_default() += 1;
    }
    let _ = writeln!(
        md,
        "\n## Pins no wire ends on, by EPLAN's view ({})\n",
        free.len()
    );
    let mut ranked: Vec<_> = classes.iter().collect();
    ranked.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    for (c, n) in ranked {
        let _ = writeln!(md, "- {n}: {}", c.label());
    }
    // What kind of symbol the free, touching points belong to.
    let mut kinds: BTreeMap<(String, String), usize> = BTreeMap::new();
    for (p, c, others) in &free {
        if matches!(
            c,
            FreeClass::TouchesSeparateNets | FreeClass::TouchesSameNet
        ) {
            let sym = project.symbol(p.symbol);
            let role = sym.map_or("?", |s| role_name(&s.role)).to_owned();
            let _ = others;
            *kinds.entry((c.label().to_owned(), role)).or_default() += 1;
        }
    }
    let _ = writeln!(md, "\n### Touching points by owner role\n");
    for ((c, r), n) in &kinds {
        let _ = writeln!(md, "- {n}: {c} ({r})");
    }
    // Where each drawn pin sits against where the file has the point.
    let mut moved: BTreeMap<String, (usize, usize, f64)> = BTreeMap::new();
    for p in &pts {
        let (Some((fx, fy)), Some(sym)) = (plan.file_position(&p.key), project.symbol(p.symbol))
        else {
            continue;
        };
        let at = sym.placement.apply(sym.pins[p.pin as usize].offset);
        let d = (at.x - fx).abs() + (at.y - fy).abs();
        let e = moved.entry(sym.symbol.0.clone()).or_default();
        e.0 += 1;
        if d > 0.5 {
            e.1 += 1;
            e.2 += d;
        }
    }
    let total: usize = moved.values().map(|v| v.0).sum();
    let off: usize = moved.values().map(|v| v.1).sum();
    let _ = writeln!(
        md,
        "\n## Drawn pins against the file's own points: {off} of {total} are more than 0.5 mm away\n"
    );
    let mut ranked: Vec<_> = moved.iter().filter(|(_, v)| v.1 > 0).collect();
    ranked.sort_by_key(|(_, v)| std::cmp::Reverse(v.1));
    for (key, (n, off, sum)) in ranked.iter().take(25) {
        let _ = writeln!(
            md,
            "- {key}: {off} of {n} pins off, mean {:.1} mm",
            sum / *off as f64
        );
    }
    // Per page.
    let mut per_page: BTreeMap<PageId, BTreeMap<&'static str, usize>> = BTreeMap::new();
    for pr in &problems {
        if let Some(page) = pr.page {
            *per_page
                .entry(page)
                .or_default()
                .entry(pr.kind.key())
                .or_default() += 1;
        }
    }
    let mut rows: Vec<_> = per_page
        .iter()
        .map(|(p, m)| (m.values().sum::<usize>(), *p, m))
        .collect();
    rows.sort_by_key(|r| std::cmp::Reverse(r.0));
    let _ = writeln!(md, "\n## Pages with most findings\n");
    for (n, page, m) in rows.iter().take(40) {
        let name = project.page(*page).map_or("?".into(), |p| p.name.clone());
        let detail: Vec<String> = m.iter().map(|(k, v)| format!("{k} {v}")).collect();
        let _ = writeln!(md, "- {name}: {n} ({})", detail.join(", "));
    }
    let json = serde_json_lines(&problems, project, &free);
    (md, json)
}

fn role_name(r: &SymbolRole) -> &'static str {
    match r {
        SymbolRole::Unassigned => "unassigned",
        SymbolRole::Device { main: true, .. } => "device main",
        SymbolRole::Device { .. } => "device",
        SymbolRole::Terminal { .. } => "terminal",
        SymbolRole::Interruption { .. } => "interruption",
        _ => "other",
    }
}

fn serde_json_lines(
    problems: &[Problem],
    project: &Project,
    free: &[(Pt, FreeClass, Vec<PinKey>)],
) -> String {
    let mut s = String::new();
    for p in problems {
        let page = p
            .page
            .and_then(|id| project.page(id))
            .map_or(String::new(), |pg| pg.name.clone());
        let (x, y) = p.position.map_or((0.0, 0.0), |p| (p.x, p.y));
        let _ = writeln!(
            s,
            "{{\"kind\":{:?},\"page\":{:?},\"x\":{x:.2},\"y\":{y:.2},\"message\":{:?}}}",
            p.kind.key(),
            page,
            p.message
        );
    }
    for (p, c, others) in free {
        let page = project
            .page(p.page)
            .map_or(String::new(), |pg| pg.name.clone());
        let _ = writeln!(
            s,
            "{{\"free_pin\":{:?},\"page\":{:?},\"class\":{:?},\"touches\":{:?}}}",
            p.key.0,
            page,
            c.label(),
            others.iter().map(|k| k.0.clone()).collect::<Vec<_>>()
        );
    }
    s
}

/// Geometry per file page for the vector comparison with EPLAN's PDF:
/// wires as polylines, pins and junctions, in page millimetres (y down, the
/// PDF's own frame).
pub fn geometry_json(plan: &PxfPlan, project: &Project) -> String {
    use novo_core::Point;
    let pts = points(plan, project);
    let free = classify_free(plan, project, &pts);
    let netlist = project.netlist();
    let mut pages: Vec<_> = project.pages().collect();
    pages.sort_by_key(|p| p.id);
    let sources = plan.page_sources();
    let at = |e: &Endpoint| -> Option<Point> {
        match e {
            Endpoint::Pin { symbol, pin } => {
                let s = project.symbol(*symbol)?;
                Some(s.placement.apply(s.pins.get(*pin as usize)?.offset))
            }
            Endpoint::Junction { junction } => Some(project.junction(*junction)?.position),
            _ => None,
        }
    };
    let mut s = String::from("[");
    for (i, page) in pages.iter().enumerate() {
        let Some(src) = sources.get(i) else { continue };
        if i > 0 {
            s.push(',');
        }
        let _ = write!(
            s,
            "{{\"source\":{src},\"name\":{:?},\"title\":{:?},\"wires\":[",
            page.name, page.title
        );
        let mut first = true;
        for w in project.wires().filter(|w| w.page == page.id) {
            let (Some(a), Some(b)) = (at(&w.from), at(&w.to)) else {
                continue;
            };
            if !first {
                s.push(',');
            }
            first = false;
            let mut pts = vec![a];
            pts.extend(w.route.iter().copied());
            pts.push(b);
            let list: Vec<String> = pts
                .iter()
                .map(|p| format!("[{:.3},{:.3}]", p.x, p.y))
                .collect();
            let _ = write!(s, "[{}]", list.join(","));
        }
        s.push_str("],\"pins\":[");
        let mut first = true;
        for sym in project.symbols().filter(|x| x.page == page.id) {
            for (pi, p) in sym.pins.iter().enumerate() {
                let q = sym.placement.apply(p.offset);
                if !first {
                    s.push(',');
                }
                first = false;
                let net = netlist.net_of(Endpoint::pin(sym.id, pi as u16));
                let _ = write!(
                    s,
                    "[{:.3},{:.3},{:?},{}]",
                    q.x,
                    q.y,
                    p.name,
                    net.map_or(-1, |n| i64::from(n.0))
                );
            }
        }
        s.push_str("],\"free\":[");
        let list: Vec<String> = free
            .iter()
            .filter(|(p, _, _)| p.page == page.id)
            .map(|(p, c, _)| {
                let sym = project.symbol(p.symbol);
                let at = sym.map_or(Point::new(0.0, 0.0), |s| {
                    s.placement.apply(s.pins[p.pin as usize].offset)
                });
                let name = sym.map_or("", |s| s.pins[p.pin as usize].name.as_str());
                format!(
                    "[{:.3},{:.3},{:?},{:?},{:?}]",
                    at.x,
                    at.y,
                    c.label(),
                    name,
                    sym.map_or("", |s| s.symbol.0.as_str())
                )
            })
            .collect();
        s.push_str(&list.join(","));
        s.push_str("],\"junctions\":[");
        let list: Vec<String> = project
            .junctions()
            .filter(|j| j.page == page.id)
            .map(|j| format!("[{:.3},{:.3}]", j.position.x, j.position.y))
            .collect();
        s.push_str(&list.join(","));
        s.push_str("]}");
    }
    s.push(']');
    s
}

fn out_dir() -> PathBuf {
    std::env::var_os("KOPERCAD_VERIFY_OUT")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

#[test]
fn real_project_verification_report() {
    let Some(path) = std::env::var_os("KOPERCAD_CRANE_EPJ").map(PathBuf::from) else {
        return;
    };
    if !path.is_file() {
        return;
    }
    let (plan, _, _) = novo_interop::eplan::crane::plan_crane(
        &Project::new("x"),
        &path,
        &novo_interop::eplan::pxf::PxfOptions::default(),
    )
    .unwrap();
    let mut doc = Document::new(Project::new("x"));
    plan.apply(&mut doc).unwrap();
    let (md, json) = report(&plan, doc.project());
    let dir = out_dir();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("verify.md"), &md).unwrap();
    std::fs::write(dir.join("verify.jsonl"), json).unwrap();
    std::fs::write(
        dir.join("geometry.json"),
        geometry_json(&plan, doc.project()),
    )
    .unwrap();
    eprintln!("{md}");
}

#[test]
fn problem_kind_keys_used_by_the_report_exist() {
    // The report ranks by `ProblemKind::key`; keep the classes it names.
    assert_eq!(ProblemKind::UnconnectedPin.key(), "check.unconnected_pin");
}

/// The classifier and the report run on the synthetic crane folder that
/// ships with the repository, so the harness itself is covered in CI.
#[test]
fn the_report_runs_on_the_synthetic_crane_fixture() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../novo-interop/tests/fixtures/crane/output");
    let (plan, _, _) = novo_interop::eplan::crane::plan_crane(
        &Project::new("x"),
        &fixture,
        &novo_interop::eplan::pxf::PxfOptions::default(),
    )
    .unwrap();
    let mut doc = Document::new(Project::new("x"));
    plan.apply(&mut doc).unwrap();
    let pts = points(&plan, doc.project());
    assert_eq!(pts.len(), plan.drawn_pins().count());
    // Wires were drawn from EPLAN's nets, so KoperCAD splits none of them.
    let diff = net_diff(&pts);
    assert!(diff.split.is_empty(), "{diff:?}");
    let (md, json) = report(&plan, doc.project());
    assert!(md.contains("# EPLAN verification"));
    assert!(md.contains("Pins no wire ends on"));
    assert!(!json.is_empty() || md.contains("none filtered"));
    let geo = geometry_json(&plan, doc.project());
    assert!(geo.starts_with("[{\"source\""), "{geo}");
}
