//! Scale benchmark: builds a big synthetic project and times what a person
//! waits for. Run in release mode:
//!
//! ```text
//! cargo run --release -p novo-tests --example scale_bench -- --size medium
//! cargo run --release -p novo-tests --example scale_bench -- --feeders 300 --out run.md
//! ```
//!
//! `--only a,b` runs just the steps whose names contain one of the words.
//! `--budget` fails (exit 1) when a step is over its budget in `BUDGETS`.

use std::time::Instant;

use novo_canvas::model::{PageScene, PlaceholderArt};
use novo_canvas::tessellate::{ItemMeshCache, Tessellator};
use novo_checks::calc::tables::TableSet;
use novo_core::{Document, Placement, Point, Project};
use novo_numbering::{Mode, Scheme, SchemeKind, Scope, plan};
use novo_tests::{BigSpec, big_project};

/// Step name prefix and its budget in milliseconds on the reference
/// container (release build). A step over budget fails `--budget`. The first
/// entry whose prefix matches wins, so each row has its own entry (a loose
/// budget on a shared prefix once covered rows it was not meant for).
const BUDGETS: &[(&str, f64)] = &[
    ("open file", 3000.0),
    ("save full", 5000.0),
    ("autosave", 500.0),
    ("page scene (one page", 100.0),
    ("page scene + tessellate (one page", 100.0),
    ("page scene + tessellate (every page", 1000.0),
    ("search", 100.0),
    ("edit one symbol", 100.0),
    ("undo edit", 100.0),
];

struct Row {
    name: String,
    ms: f64,
    note: String,
}

struct Bench {
    rows: Vec<Row>,
    only: Vec<String>,
}

impl Bench {
    fn wanted(&self, name: &str) -> bool {
        self.only.is_empty() || self.only.iter().any(|o| name.contains(o.as_str()))
    }

    fn time<R>(&mut self, name: &str, f: impl FnOnce() -> (R, String)) -> Option<R> {
        if !self.wanted(name) {
            return None;
        }
        let start = Instant::now();
        let (r, note) = f();
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        eprintln!("{name:<34} {ms:>10.1} ms  {note}");
        self.rows.push(Row {
            name: name.into(),
            ms,
            note,
        });
        Some(r)
    }
}

fn arg(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut spec = match arg(&args, "--size").as_deref() {
        Some("small") => BigSpec::SMALL,
        Some("large") => BigSpec::LARGE,
        _ => BigSpec::MEDIUM,
    };
    if args.iter().any(|a| a == "--uniform") {
        spec = spec.uniform();
    }
    if let Some(n) = arg(&args, "--feeders") {
        spec.feeders = n.parse().expect("--feeders needs a number");
    }
    let mut b = Bench {
        rows: Vec::new(),
        only: arg(&args, "--only")
            .map(|s| s.split(',').map(str::to_owned).collect())
            .unwrap_or_default(),
    };
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("big.kopercad");

    let mut doc = Document::default();
    let big = b
        .time("generate", || {
            let big = big_project(&mut doc, spec).expect("generator");
            let p = doc.project();
            let note = format!(
                "{} pages, {} devices, {} wires, {} symbols, {} records",
                p.pages().count(),
                p.devices().count(),
                p.wires().count(),
                p.all::<novo_core::SymbolInstance>().count(),
                p.records().count()
            );
            (big, note)
        })
        .unwrap_or_else(|| big_project(&mut doc, spec).expect("generator"));
    let project = doc.project();
    let pages = project.pages().count();
    let wires = project.wires().count();

    // `--save-project FILE`: keep the generated project to open in the app (the
    // QA scenario `qa/scenarios/40-scale-frames.qa` opens it).
    if let Some(out) = arg(&args, "--save-project") {
        let _ = std::fs::remove_file(&out);
        novo_storage::ProjectFile::create(std::path::Path::new(&out), project).expect("save");
        eprintln!("saved the generated project to {out}");
    }

    // ---- file ---------------------------------------------------------
    b.time("save full", || {
        novo_storage::ProjectFile::create(&path, project).expect("create");
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        ((), format!("{:.1} MB on disk", size as f64 / 1e6))
    });
    b.time("to_json", || {
        let j = project.to_json();
        ((), format!("{:.1} MB", j.len() as f64 / 1e6))
    });
    let json = project.to_json();
    b.time("from_json", || {
        (Project::from_json(&json).expect("json"), String::new())
    });
    if path.exists() {
        b.time("open file", || {
            (
                novo_storage::ProjectFile::read(&path).expect("read"),
                String::new(),
            )
        });
    }

    // ---- model queries -------------------------------------------------
    b.time("netlist (cold)", || {
        let n = project.netlist();
        let note = format!("{} nets", n.nets().len());
        (n, note)
    });
    b.time("netlist (again, cached?)", || {
        (project.netlist(), String::new())
    });
    b.time("search tag", || {
        let hits = project.search(&big.probe_tag);
        let note = format!("{} hits for {}", hits.len(), big.probe_tag);
        (hits, note)
    });
    b.time("search word", || {
        let hits = project.search("Motor");
        let note = format!("{} hits", hits.len());
        (hits, note)
    });
    b.time("cross references of one device", || {
        let d = big.devices[big.devices.len() / 2];
        (project.cross_references(d), String::new())
    });

    // ---- redraw ---------------------------------------------------------
    let mid = big.pages[big.pages.len() / 2];
    // What "redraw" measures here: building the page scene from the model and
    // turning it into triangles on the CPU. It is NOT a frame: no GPU, no egui
    // layout, no upload or rasterisation. It is the model-side cost a frame
    // pays after an edit; unchanged items come from the mesh cache.
    b.time("page scene (one page, placeholder art)", || {
        let s = PageScene::new(project, mid, &PlaceholderArt);
        let note = format!(
            "{} items; model side only, not a frame",
            s.scene().iter().count()
        );
        (s, note)
    });
    b.time("page scene + tessellate (one page)", || {
        let s = PageScene::new(project, mid, &PlaceholderArt);
        let mesh = Tessellator::default().scene_cached(s.scene(), &mut ItemMeshCache::default());
        let note = format!("{} indices; CPU triangles, no GPU", mesh.mesh.indices.len());
        ((), note)
    });
    // Every page, with the real symbol artwork: mean and slowest page.
    if let Ok(library) = novo_py::library() {
        let art = novo_editor::art::LibraryArt::new(
            library,
            novo_pdf::PdfOptions::default().style.symbol,
        );
        b.time("page scene + tessellate (every page, symbol art)", || {
            let tess = Tessellator::default();
            let mut worst = (0.0_f64, 0);
            let mut items = 0;
            for (k, &page) in big.pages.iter().enumerate() {
                let start = Instant::now();
                let s = PageScene::new(project, page, &art);
                let _ = tess.scene_cached(s.scene(), &mut ItemMeshCache::default());
                let ms = start.elapsed().as_secs_f64() * 1000.0;
                items += s.scene().iter().count();
                if ms > worst.0 {
                    worst = (ms, k);
                }
            }
            (
                (),
                format!(
                    "{pages} pages, {items} items; slowest page {:.2} ms (page {}); CPU only",
                    worst.0,
                    worst.1 + 1
                ),
            )
        });
    }

    // ---- edits ----------------------------------------------------------
    let probe = big.probe;
    b.time("edit one symbol", || {
        doc.transact("Move", |tx| {
            tx.move_symbol(probe, Placement::at(Point::new(-50.0, -50.0)))
        })
        .expect("move");
        ((), String::new())
    });
    b.time("undo edit", || {
        doc.undo().expect("undo");
        ((), String::new())
    });
    b.time("autosave one edit", || {
        let mut file = novo_storage::ProjectFile::open(&path).expect("open");
        let (_, changes) = doc
            .transact("Move", |tx| {
                tx.move_symbol(probe, Placement::at(Point::new(-40.0, -40.0)))
            })
            .expect("move");
        file.save_changes(doc.project(), &changes).expect("save");
        ((), String::new())
    });
    let project = doc.project();

    // ---- machine options (a copy of the project, so later steps are untouched)
    if b.wanted("options") {
        use novo_core::AnyId;
        use novo_workflow::options::{self, OptionDef};
        let mut tied = project.clone();
        let defs = [
            OptionDef::yes_no("conveyor", "Conveyor", true),
            OptionDef::yes_no("fan", "Fan", false),
            OptionDef::choice("voltage", "Voltage", &["400 V", "480 V"], "400 V"),
            OptionDef::yes_no("export", "Export pack", false),
        ];
        let pages: Vec<_> = tied.pages_in_order().iter().map(|p| p.id).collect();
        let devices: Vec<_> = tied.devices().map(|d| d.id).collect();
        tied.transact("Options", |tx| {
            for def in &defs {
                options::define_option(tx, def.clone())?;
            }
            for (i, page) in pages.iter().enumerate().filter(|(i, _)| i % 15 == 0) {
                let cond = [
                    "opt.conveyor",
                    "opt.fan",
                    "opt.voltage == \"480 V\"",
                    "opt.export",
                ][i / 15 % 4];
                options::set_condition(tx, AnyId::Page(*page), Some(cond))?;
            }
            for (i, device) in devices.iter().enumerate().filter(|(i, _)| i % 40 == 0) {
                let cond = ["opt.fan", "opt.export", "opt.conveyor"][i / 40 % 3];
                options::set_condition(tx, AnyId::Device(*device), Some(cond))?;
            }
            Ok(())
        })
        .expect("tie options");
        let assignment = options::current(&tied);
        b.time("options: evaluate (status chip, once per edit)", || {
            let e = options::evaluate(&tied, &assignment);
            let note = format!("{} objects left out", e.inactive.len());
            ((), note)
        });
        b.time("options: machine copy (documents, once per edit)", || {
            let copy = options::machine_project(&tied, &assignment).expect("copy");
            let note = format!(
                "{} pages, {} devices kept",
                copy.pages().count(),
                copy.devices().count()
            );
            ((), note)
        });
    }

    // ---- engines --------------------------------------------------------
    b.time("number wires (plan, fill)", || {
        let p = plan(
            project,
            &Scheme::new(SchemeKind::Sequential),
            Mode::Fill,
            &Scope::Project,
        );
        let note = format!("{wires} wires");
        (p, note)
    });
    if let Some(p) = b.time("number wires (plan again)", || {
        (
            plan(
                project,
                &Scheme::new(SchemeKind::Sequential),
                Mode::Fill,
                &Scope::Project,
            ),
            String::new(),
        )
    }) {
        b.time("number wires (apply)", || {
            doc.transact("Number wires", |tx| p.apply(tx))
                .expect("apply");
            ((), String::new())
        });
    }
    let project = doc.project();
    b.time("design checks", || {
        let problems = novo_checks::check(project);
        let note = format!("{} problems", problems.len());
        (problems, note)
    });
    b.time("wire cut jobs", || {
        let jobs = novo_reports::wire_cut_jobs(project);
        let note = format!("{} jobs", jobs.len());
        (jobs, note)
    });
    for report in novo_reports::standard_reports() {
        let name = format!("report {}", report.id());
        b.time(&name, || {
            let out = report.generate(project);
            let rows: usize = out.sections.iter().map(|s| s.rows.len()).sum();
            ((), format!("{rows} rows"))
        });
    }
    b.time("autopilot plan", || {
        let a = novo_reports::autopilot::plan_for(project, &TableSet::builtin());
        let note = format!("{} proposals", a.proposals.len());
        (a, note)
    });
    if let Ok(library) = novo_py::library() {
        b.time("xref index", || {
            (novo_xref::XrefIndex::build(project, library), String::new())
        });
        b.time("pdf export (all pages)", || {
            let opts = novo_pdf::PdfOptions::default();
            let art = novo_editor::art::LibraryArt::new(library, opts.style.symbol);
            let bytes = novo_pdf::export_pdf_with_art(project, &opts, &art).expect("pdf");
            (
                (),
                format!("{:.1} MB, {pages} pages", bytes.len() as f64 / 1e6),
            )
        });
    }

    // ---- 3D and heat (first cabinet; one cabinet is what a view shows) ---
    let cab = big.cabinets[0];
    let routed = b
        .time("route all wires (whole project)", || {
            let r = novo_routing::model::route_project(project);
            let note = format!("{} cabinets routed", r.plans.len());
            (r, note)
        })
        .unwrap_or_else(|| novo_routing::model::route_project(project));
    b.time("3d scene + wires (one cabinet)", || {
        let mut options = novo_view3d::build::model::options(project, cab, false);
        options.wires = novo_view3d::build::model::wires_with(project, cab, &routed);
        let s = novo_view3d::build::model::scene(project, cab, &options).expect("plate");
        let note = format!("{} items, {} wires", s.items.len(), options.wires.len());
        ((), note)
    });
    b.time("3d scenes (all cabinets, routed)", || {
        let n = big.cabinets.len();
        for &c in &big.cabinets {
            let mut options = novo_view3d::build::model::options(project, c, false);
            options.wires = novo_view3d::build::model::wires_with(project, c, &routed);
            let _ = novo_view3d::build::model::scene(project, c, &options);
        }
        ((), format!("{n} cabinets"))
    });
    let scenario = b.time("heat scenario (one cabinet)", || {
        let s = novo_thermal::bridge::scenario_from_project(
            project,
            cab,
            &novo_thermal::bridge::BridgeOptions::default(),
        )
        .expect("scenario");
        (s.scenario, String::new())
    });
    if let Some(s) = scenario {
        b.time("heat steady 2D, central run only", || {
            let o = novo_thermal::Options {
                bands: false,
                advice: false,
            };
            let r = novo_thermal::simulate_steady_with(&s, o).expect("thermal");
            let g = &r.grid;
            (
                (),
                format!(
                    "{}x{} cells, {} iterations, converged {}, stalled {}",
                    g.cols, g.rows, r.iterations, r.converged, r.stalled
                ),
            )
        });
        b.time("heat steady 2D, with bands", || {
            let r = novo_thermal::simulate_steady(&s).expect("thermal");
            ((), format!("{} devices", r.devices.len()))
        });
        if b.wanted("heat cfd") {
            use novo_cfd::{CabinetSolver, Control, Fidelity, Solver};
            b.time("heat cfd coarse (one cabinet)", || {
                let r = CabinetSolver::default()
                    .solve(&s, Fidelity::Coarse, &Control::new(), &mut |_| {})
                    .expect("cfd");
                let g = &r.grid;
                ((), format!("{}x{}x{} cells", g.nx, g.ny, g.nz))
            });
        }
    }

    // ---- report ---------------------------------------------------------
    let mut md = "| Step | Time (ms) | Notes |\n| --- | ---: | --- |\n".to_string();
    for r in &b.rows {
        md.push_str(&format!("| {} | {:.1} | {} |\n", r.name, r.ms, r.note));
    }
    println!(
        "Scale benchmark: {} feeders, {} pages (release: {})\n\n{md}",
        spec.feeders,
        spec.pages(),
        !cfg!(debug_assertions)
    );
    if let Some(out) = arg(&args, "--out") {
        std::fs::write(&out, &md).expect("write --out");
    }
    if args.iter().any(|a| a == "--budget") {
        let mut failed = false;
        for r in &b.rows {
            if let Some((_, limit)) = BUDGETS.iter().find(|(p, _)| r.name.starts_with(p))
                && r.ms > *limit
            {
                eprintln!(
                    "OVER BUDGET: {} took {:.0} ms, budget {limit:.0} ms",
                    r.name, r.ms
                );
                failed = true;
            }
        }
        if failed {
            std::process::exit(1);
        }
    }
}
