//! The command line for the Ask Claude evaluation scenarios.
//!
//!   kopercad-cli ask-claude-eval [options]   (or cargo run -p novo-ai --example ask_claude_eval -- [options])
//!
//! Without `--real` the scripted model answers: no network, no key. With
//! `--real` your own Anthropic API key is used (the ANTHROPIC_API_KEY
//! environment variable, or the key saved by the app); the run costs a few
//! cents per scenario. The key is never printed or written anywhere.
//!
//!   --only ID        run one scenario (repeatable)
//!   --list           list the scenarios
//!   --model ID       model for --real (default: the app's default)
//!   --effort LEVEL   low, medium or high
//!   --budget USD     stop a scenario past this estimated cost (default 0.60)
//!   --out DIR        write report.md (and replay files with --replay)
//!   --verbose        print every tool call and its answer
//!   --replay         also write KOPERCAD_AI_REPLAY files (scripted replies) to --out

use crate::eval::{self, Mode};
use crate::key::{ApiKey, KeyStore};
use std::io::Write;
use std::path::PathBuf;

pub fn run(args: &[String], out_w: &mut dyn Write) -> i32 {
    let mut args = args.iter().cloned();
    let (mut real, mut verbose, mut list, mut replay) = (false, false, false, false);
    let mut only: Vec<String> = Vec::new();
    let (mut model, mut effort, mut budget, mut out) = (
        None::<String>,
        "medium".to_string(),
        0.60f64,
        None::<PathBuf>,
    );
    while let Some(a) = args.next() {
        match a.as_str() {
            "--real" => real = true,
            "--verbose" => verbose = true,
            "--list" => list = true,
            "--replay" => replay = true,
            "--only" => only.extend(args.next()),
            "--model" => model = args.next(),
            "--effort" => effort = args.next().unwrap_or(effort),
            "--budget" => budget = args.next().and_then(|v| v.parse().ok()).unwrap_or(budget),
            "--out" => out = args.next().map(PathBuf::from),
            other => {
                eprintln!("unknown option {other}");
                return 2;
            }
        }
    }
    let scenarios: Vec<_> = eval::builtin()
        .into_iter()
        .filter(|s| only.is_empty() || only.contains(&s.id))
        .collect();
    if list {
        for s in &scenarios {
            let _ = writeln!(out_w, "{:28} {:9} {}", s.id, s.category, s.title);
        }
        return 0;
    }
    let mode = if real {
        let key = std::env::var("ANTHROPIC_API_KEY")
            .ok()
            .and_then(|k| ApiKey::parse(&k).ok())
            .or_else(|| KeyStore::in_profile().and_then(|s| s.load().map(|(k, _)| k)));
        let Some(key) = key else {
            eprintln!(
                "No API key. Set ANTHROPIC_API_KEY (it starts with sk-ant-) or save one in \
                 KoperCAD under Help > Ask Claude."
            );
            return 2;
        };
        Mode::Real {
            key,
            model: model.unwrap_or_else(|| crate::models::DEFAULT_MODEL.to_string()),
            effort,
            budget_usd: budget,
        }
    } else {
        Mode::Scripted
    };
    let _ = writeln!(out_w, "Mode: {}", mode.label());
    let mut reports = Vec::new();
    for s in &scenarios {
        let r = eval::run(s, &mode);
        let _ = writeln!(
            out_w,
            "{:28} {}",
            r.id,
            if r.passed() { "pass" } else { "FAIL" }
        );
        if verbose || !r.passed() {
            for c in r.checks.iter().filter(|c| verbose || !c.ok) {
                let _ = writeln!(
                    out_w,
                    "    {} {} {}",
                    if c.ok { "ok  " } else { "FAIL" },
                    c.name,
                    c.detail
                );
            }
            if let Some(e) = &r.error {
                let _ = writeln!(out_w, "    error: {e}");
            }
        }
        if verbose {
            for l in &r.log {
                let reply: String = l.reply.chars().take(400).collect();
                let _ = writeln!(
                    out_w,
                    "  > {} {}\n    {}{reply}",
                    l.tool,
                    l.input,
                    if l.is_error { "ERROR " } else { "" }
                );
            }
        }
        reports.push(r);
    }
    let md = eval::markdown(&mode, &reports);
    if let Some(dir) = &out {
        std::fs::create_dir_all(dir).expect("create the output folder");
        let name = if real {
            "report-real.md"
        } else {
            "report-scripted.md"
        };
        std::fs::write(dir.join(name), &md).expect("write the report");
        if replay {
            for s in &scenarios {
                let file = dir.join(format!("{}.replay.json", s.id));
                std::fs::write(file, serde_json::to_string_pretty(&s.replay()).unwrap()).unwrap();
            }
        }
        let _ = writeln!(out_w, "Report: {}", dir.join(name).display());
    }
    let passed = reports.iter().filter(|r| r.passed()).count();
    let _ = writeln!(out_w, "{passed} of {} passed", reports.len());
    i32::from(passed != reports.len())
}
