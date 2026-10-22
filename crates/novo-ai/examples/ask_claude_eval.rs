//! Runs the Ask Claude evaluation scenarios.
//!
//!   cargo run -p novo-ai --example ask_claude_eval -- [options]
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = novo_ai::eval_cli::run(&args, &mut std::io::stdout());
    std::process::exit(code);
}
