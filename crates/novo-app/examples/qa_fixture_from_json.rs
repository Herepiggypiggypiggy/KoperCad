//! Writes a `.novo` project file from the diffable JSON form, for QA fixtures
//! (the app opens `.novo` files only):
//!
//! ```text
//! cargo run -p novo-app --example qa_fixture_from_json -- assets/lessons/cabinet-layout/finished.json qa/fixtures/door-cutouts.novo
//! ```

use novo_core::Project;
use novo_storage::ProjectFile;

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(from), Some(to)) = (args.next(), args.next()) else {
        eprintln!("usage: qa_fixture_from_json <project.json> <out.novo>");
        std::process::exit(2);
    };
    let text = std::fs::read_to_string(&from).expect("read the JSON project");
    let project = Project::from_json(&text).expect("parse the JSON project");
    let _ = std::fs::remove_file(&to);
    ProjectFile::create(&to, &project).expect("write the project file");
    println!("{to}");
}
