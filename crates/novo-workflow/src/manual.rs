//! The offline user manual: pages written in `docs/manual/*.md` plus pages
//! generated from the live command registry, keymap and menus, so the
//! reference part cannot drift from the program.
//!
//! The site is plain HTML with one stylesheet, no script and no web font, so
//! it opens from a folder with no network. The app writes it under the user's
//! profile and opens `index.html` (Help > User manual, F1); `kopercad-cli
//! manual <dir>` writes the same files for printing or a shared drive.
//!
//! Where the user finds it: Help > User manual, F1 (help for the panel in
//! front), Ctrl+K "help" and "manual".

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::commands::{COMMANDS, CommandInfo};
use crate::keymap::Keymap;
use crate::menus::{self, MENUS};

/// Key and pointer behaviours that are not a single key press, so the keymap
/// cannot list them: (what you do, what happens, search words). The shortcut
/// sheet and the manual both read this list.
pub const GESTURES: &[(&str, &str, &[&str])] = &[
    (
        "Q (hold)",
        "Opens the place wheel; a tap places a symbol",
        &["wheel", "place", "radial"],
    ),
    (
        "Space (hold) + drag",
        "Pans the drawing",
        &["pan", "move", "scroll"],
    ),
    (
        "Shift while moving a symbol",
        "Does the other of leave wires / take wires along",
        &["move", "wires", "carry"],
    ),
    (
        "Shift+W while drawing a wire",
        "Switches between corners and straight lines",
        &["wire", "corner", "lines"],
    ),
    (
        "Q while drawing a wire",
        "Ends the wire on the part under the pointer",
        &["wire", "end", "part"],
    ),
    (
        "Tab while drawing a wire",
        "Cycles the connection point of a multi-pin part",
        &["wire", "pin", "cycle"],
    ),
    (
        "Mouse wheel",
        "Zooms around the pointer",
        &["zoom", "scroll"],
    ),
];

/// A written page: file stem, title, one-line summary (for the index) and the
/// Markdown source.
struct Source {
    stem: &'static str,
    title: &'static str,
    summary: &'static str,
    markdown: &'static str,
}

macro_rules! page {
    ($stem:literal, $title:literal, $summary:literal) => {
        Source {
            stem: $stem,
            title: $title,
            summary: $summary,
            markdown: include_str!(concat!("../../../docs/manual/", $stem, ".md")),
        }
    };
}

/// The written pages, in reading order.
const SOURCES: &[Source] = &[
    page!(
        "first-hour",
        "Your first hour",
        "Open the example, draw a circuit, save, print"
    ),
    page!(
        "motor-starter",
        "Workflow: a motor starter",
        "From an empty page to wire numbers and a parts list"
    ),
    page!(
        "cabinet",
        "Workflow: the cabinet",
        "Plate, rails, ducts, wire lengths and the 3D view"
    ),
    page!(
        "workshop",
        "Workflow: the workshop pack and Build mode",
        "Bench output, drilling template, guided assembly"
    ),
    page!(
        "handover",
        "Workflow: commissioning and handover",
        "Checks at the machine, punch list, revisions, the final set"
    ),
    page!(
        "numbers",
        "Model or exact? Reading the numbers",
        "Which numbers are exact and which are estimates"
    ),
    page!(
        "files",
        "Files, formats and backups",
        ".kopercad, .novo, .kcfield, exports, recovery"
    ),
    page!(
        "troubleshooting",
        "Troubleshooting and recovery",
        "When something looks wrong, and how to get your work back"
    ),
    page!(
        "glossary",
        "Glossary",
        "Terms used in the program and this manual"
    ),
];

/// Context help: a topic id the app asks for, and the page it opens. F1 in
/// the program picks the topic from what is in front.
pub const TOPICS: &[(&str, &str)] = &[
    ("start", "first-hour.html"),
    ("schematic", "first-hour.html#draw-a-circuit"),
    ("wiring", "first-hour.html#wires"),
    ("motor-starter", "motor-starter.html"),
    ("numbering", "motor-starter.html#wire-numbers"),
    ("parts", "motor-starter.html#parts"),
    ("cabinet", "cabinet.html"),
    ("3d", "cabinet.html#the-3d-view"),
    ("heat", "numbers.html#heat-and-airflow"),
    ("reports", "workshop.html#reports-and-bench-output"),
    ("workshop", "workshop.html"),
    ("build-mode", "workshop.html#build-mode"),
    ("commissioning", "handover.html"),
    ("revisions", "handover.html#revisions"),
    ("live", "motor-starter.html#try-it-live"),
    ("learn", "first-hour.html#learning-paths"),
    ("numbers", "numbers.html"),
    ("files", "files.html"),
    ("problems", "troubleshooting.html"),
    ("shortcuts", "shortcuts.html"),
    ("commands", "commands.html"),
];

/// The page to open for a context-help topic, or the manual's front page.
pub fn topic_file(topic: &str) -> &'static str {
    TOPICS
        .iter()
        .find(|(t, _)| *t == topic)
        .map_or("index.html", |(_, file)| file)
}

/// One generated file of the site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Images the pages may name as `images/<name>.png`. A page that names an
/// image that is not here shows a plain caption instead, so the manual is
/// complete without screenshots; `scripts/build-manual.sh` refreshes them
/// from the QA suite at 1920x1080.
pub type Images<'a> = &'a [(&'a str, &'a [u8])];

/// Builds every file of the site: `index.html`, the written pages,
/// `menus.html`, `commands.html`, `shortcuts.html`, `features.html`,
/// `style.css` and the images that exist.
pub fn build_site(version: &str, images: Images<'_>) -> Vec<SiteFile> {
    let have: Vec<&str> = images.iter().map(|(n, _)| *n).collect();
    let nav = nav_list();
    let mut files = Vec::new();
    let mut add = |name: &str, title: &str, body: String| {
        files.push(SiteFile {
            name: name.to_string(),
            bytes: shell(title, &nav, &body, version).into_bytes(),
        });
    };
    add("index.html", "User manual", index_body(version));
    for s in SOURCES {
        let body = render_markdown(s.markdown, &have);
        add(&format!("{}.html", s.stem), s.title, body);
    }
    add("features.html", "Feature index", features_body());
    add("menus.html", "Menu by menu", menus_body());
    add("commands.html", "All commands, A to Z", commands_body());
    add("shortcuts.html", "Keyboard shortcuts", shortcuts_body());
    files.push(SiteFile {
        name: "style.css".into(),
        bytes: STYLE.as_bytes().to_vec(),
    });
    for (name, bytes) in images {
        files.push(SiteFile {
            name: format!("images/{name}.png"),
            bytes: bytes.to_vec(),
        });
    }
    files
}

/// Writes the site into `dir` (created if needed) and returns the path of
/// `index.html`. Files that are not part of the site are left alone.
pub fn write_site(dir: &Path, version: &str, images: Images<'_>) -> std::io::Result<PathBuf> {
    for file in build_site(version, images) {
        let path = dir.join(&file.name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, file.bytes)?;
    }
    Ok(dir.join("index.html"))
}

/// The written pages' titles, for tests and the index.
pub fn page_titles() -> Vec<(&'static str, &'static str)> {
    SOURCES.iter().map(|s| (s.stem, s.title)).collect()
}

/// Every page file name in the site, in reading order.
pub fn page_files() -> Vec<String> {
    let mut v = vec!["index.html".to_string()];
    v.extend(SOURCES.iter().map(|s| format!("{}.html", s.stem)));
    v.extend(
        ["features", "menus", "commands", "shortcuts"]
            .iter()
            .map(|s| format!("{s}.html")),
    );
    v
}

// ---------------------------------------------------------------- shell

const STYLE: &str = "\
:root{--bg:#fff;--fg:#1d2330;--mut:#5b6475;--acc:#b5651d;--line:#d9dde5;--code:#f1f3f7;--nav:#f6f7fa}\
@media (prefers-color-scheme:dark){:root{--bg:#161a22;--fg:#e4e8f0;--mut:#9aa3b5;--acc:#e0a060;--line:#323a4a;--code:#222836;--nav:#1c212b}}\
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--fg);font:16px/1.55 system-ui,Segoe UI,sans-serif}\
.wrap{display:flex;min-height:100vh}nav{width:15rem;flex:none;background:var(--nav);border-right:1px solid var(--line);padding:1rem;font-size:.9rem}\
nav a{display:block;padding:.2rem 0;color:var(--fg);text-decoration:none}nav a:hover{color:var(--acc)}nav .here{font-weight:600;color:var(--acc)}\
nav h2{font-size:.75rem;text-transform:uppercase;letter-spacing:.06em;color:var(--mut);margin:1rem 0 .3rem}\
main{max-width:52rem;padding:1.5rem 2rem 4rem;min-width:0}h1{margin-top:0}h2{margin-top:2rem;border-bottom:1px solid var(--line);padding-bottom:.2rem}\
a{color:var(--acc)}code,kbd{background:var(--code);padding:.05em .35em;border-radius:4px;font-size:.92em}\
kbd{border:1px solid var(--line)}pre{background:var(--code);padding:.8rem;border-radius:6px;overflow:auto}pre code{padding:0;background:none}\
table{border-collapse:collapse;width:100%;margin:1rem 0;font-size:.93rem}th,td{border:1px solid var(--line);padding:.3rem .5rem;text-align:left;vertical-align:top}\
th{background:var(--nav)}blockquote{margin:1rem 0;padding:.1rem 1rem;border-left:4px solid var(--acc);background:var(--nav)}\
figure{margin:1rem 0}figure img{max-width:100%;border:1px solid var(--line);border-radius:6px}figcaption{color:var(--mut);font-size:.88rem}\
.foot{margin-top:3rem;color:var(--mut);font-size:.85rem}\
@media (max-width:760px){.wrap{display:block}nav{width:auto;border-right:0;border-bottom:1px solid var(--line)}main{padding:1rem}}\
@media print{nav{display:none}main{max-width:none;padding:0}a{color:inherit}}\
";

fn nav_list() -> Vec<(String, String)> {
    let mut v = vec![("index.html".to_string(), "Start here".to_string())];
    v.extend(
        SOURCES
            .iter()
            .map(|s| (format!("{}.html", s.stem), s.title.to_string())),
    );
    v.push(("features.html".into(), "Feature index".into()));
    v.push(("menus.html".into(), "Menu by menu".into()));
    v.push(("commands.html".into(), "All commands".into()));
    v.push(("shortcuts.html".into(), "Keyboard shortcuts".into()));
    v
}

fn shell(title: &str, nav: &[(String, String)], body: &str, version: &str) -> String {
    let mut out = String::new();
    let _ = write!(
        out,
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>{} - KoperCAD manual</title><link rel=\"stylesheet\" href=\"style.css\"></head>\
         <body><div class=\"wrap\"><nav><strong>KoperCAD manual</strong>",
        esc(title)
    );
    out.push_str("<h2>Guides</h2>");
    for (i, (file, label)) in nav.iter().enumerate() {
        if i == nav.len() - 4 {
            out.push_str("<h2>Reference</h2>");
        }
        let here = if label == title || (title == "User manual" && file == "index.html") {
            " class=\"here\""
        } else {
            ""
        };
        let _ = write!(out, "<a{here} href=\"{file}\">{}</a>", esc(label));
    }
    let _ = write!(
        out,
        "</nav><main>{body}<p class=\"foot\">KoperCAD {} by KoperVonk. This manual is part of the \
         program and works offline. Reference pages are generated from the program's own command list, \
         so they match this build.</p></main></div></body></html>",
        esc(version)
    );
    out
}

// ---------------------------------------------------------------- generated pages

fn index_body(version: &str) -> String {
    let mut s = String::new();
    let _ = write!(
        s,
        "<h1>KoperCAD user manual</h1><p>Version {}. Press <kbd>F1</kbd> in the program for the page that fits what \
         you are looking at, or <kbd>Ctrl</kbd>+<kbd>K</kbd> and type <code>help</code>.</p>",
        esc(version)
    );
    s.push_str("<h2>By task</h2><table><tr><th>Page</th><th>What it covers</th></tr>");
    for src in SOURCES {
        let _ = write!(
            s,
            "<tr><td><a href=\"{}.html\">{}</a></td><td>{}</td></tr>",
            src.stem,
            esc(src.title),
            esc(src.summary)
        );
    }
    s.push_str("</table><h2>By menu and by command</h2><table><tr><th>Page</th><th>What it covers</th></tr>");
    for (file, label, what) in [
        (
            "features.html",
            "Feature index",
            "Every area of the program and the page that explains it",
        ),
        (
            "menus.html",
            "Menu by menu",
            "Each menu, its submenus and what every entry does",
        ),
        (
            "commands.html",
            "All commands, A to Z",
            "Every command with its id, menu and key",
        ),
        (
            "shortcuts.html",
            "Keyboard shortcuts",
            "Keys, hold and tap, from the live key map",
        ),
    ] {
        let _ = write!(
            s,
            "<tr><td><a href=\"{file}\">{label}</a></td><td>{what}</td></tr>"
        );
    }
    s.push_str("</table>");
    s
}

/// Area, what it is for, where to find it, page.
const FEATURES: &[(&str, &str, &str, &str)] = &[
    (
        "Schematic drawing",
        "Symbols, wires, tags, cross references",
        "Ctrl+1, Q to place, W to wire",
        "first-hour.html",
    ),
    (
        "Wire and potential numbering",
        "Schemes, stable fill, renumber, locks",
        "Edit > Wire numbers, Ctrl+K \"numbering\"",
        "motor-starter.html#wire-numbers",
    ),
    (
        "Parts catalogue",
        "Assign parts, import maker data and tables",
        "Parts panel, File > Import parts",
        "motor-starter.html#parts",
    ),
    (
        "Terminals and cables",
        "Terminal strips, connection screens, cable cores",
        "Ctrl+3, Ctrl+4",
        "motor-starter.html#terminals-and-cables",
    ),
    (
        "Live simulation",
        "Operate switches, see coils pull in",
        "Simulate menu",
        "motor-starter.html#try-it-live",
    ),
    (
        "Cabinet layout",
        "Plate, rails, ducts, place all",
        "Ctrl+2",
        "cabinet.html",
    ),
    (
        "3D cabinet view and heat",
        "Rotate, section, wire runs, temperatures",
        "Ctrl+6",
        "cabinet.html#the-3d-view",
    ),
    (
        "Reports and bench output",
        "Parts list, wire list, cut list, labels, PDF, Excel",
        "Reports menu",
        "workshop.html#reports-and-bench-output",
    ),
    (
        "Build mode",
        "Guided assembly at the plate",
        "Reports > Build mode",
        "workshop.html#build-mode",
    ),
    (
        "Commissioning",
        "Checks at the machine, punch list, field file",
        "Ctrl+7",
        "handover.html",
    ),
    (
        "Revisions",
        "Issue, compare, lock pages",
        "File > Revisions",
        "handover.html#revisions",
    ),
    (
        "Standards and design checks",
        "Issues pill, standards report",
        "Check menu",
        "numbers.html#standards-checks",
    ),
    (
        "Learn",
        "Lessons that run in the program",
        "Help > Learn",
        "first-hour.html#learning-paths",
    ),
    (
        "Import and export",
        "DXF, STEP, AutomationML, PDF, Excel",
        "File menu",
        "files.html",
    ),
    (
        "Scripting",
        "kopercad-cli and the Python module",
        "Command line",
        "files.html#command-line-and-python",
    ),
];

fn features_body() -> String {
    let mut s = String::from(
        "<h1>Feature index</h1><table><tr><th>Area</th><th>For</th><th>Where you find it</th><th>Page</th></tr>",
    );
    for (area, what, find, page) in FEATURES {
        let _ = write!(
            s,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td><a href=\"{page}\">Read</a></td></tr>",
            esc(area),
            esc(what),
            esc(find)
        );
    }
    s.push_str("</table>");
    s
}

fn key_text(map: &Keymap, info: &CommandInfo) -> String {
    map.shortcut_text(info.id).unwrap_or_default()
}

fn menus_body() -> String {
    let map = Keymap::workflow();
    let mut s = String::from(
        "<h1>Menu by menu</h1><p>Generated from the program's menu bar. A greyed entry in the program waits for something \
         (a project, a selection); the command search lists it with the reason.</p>",
    );
    for menu in MENUS {
        let entries = menus::entries(menu);
        if entries.is_empty() {
            continue;
        }
        let _ = write!(s, "<h2 id=\"{}\">{}</h2>", slug(menu), esc(menu));
        s.push_str("<table><tr><th>Entry</th><th>Submenu</th><th>Key</th><th>Also found by typing</th></tr>");
        for (info, place) in entries {
            let _ = write!(
                s,
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(info.label),
                esc(place.submenu.unwrap_or("")),
                kbd(&key_text(&map, info)),
                esc(&info.keywords.join(", "))
            );
        }
        s.push_str("</table>");
    }
    s
}

fn commands_body() -> String {
    let map = Keymap::workflow();
    let mut list: Vec<&CommandInfo> = COMMANDS.iter().collect();
    list.sort_by_key(|c| c.label.to_lowercase());
    let mut s = String::from(
        "<h1>All commands, A to Z</h1><p>Press <kbd>Ctrl</kbd>+<kbd>K</kbd> and type any word from the label or \
         the search words to run one.</p><table><tr><th>Command</th><th>Menu</th><th>Key</th><th>Id</th></tr>",
    );
    for info in list {
        let menu = menus::place(info).map_or_else(
            || "command search only".to_string(),
            |p| match p.submenu {
                Some(sub) => format!("{} > {sub}", p.menu),
                None => p.menu.to_string(),
            },
        );
        let _ = write!(
            s,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td><code>{}</code></td></tr>",
            esc(info.label),
            esc(&menu),
            kbd(&key_text(&map, info)),
            esc(info.id)
        );
    }
    s.push_str("</table>");
    s
}

fn shortcuts_body() -> String {
    let map = Keymap::workflow();
    let mut s = String::from(
        "<h1>Keyboard shortcuts</h1><p>These are the default keys. If you changed them (Help > Open keymap file), \
         the program's own sheet (<kbd>?</kbd>) shows yours.</p>",
    );
    s.push_str("<h2 id=\"hold-and-tap\">Hold and tap</h2><table><tr><th>You do</th><th>What happens</th></tr>");
    for (what, result, _) in GESTURES {
        let _ = write!(s, "<tr><td>{}</td><td>{}</td></tr>", esc(what), esc(result));
    }
    s.push_str("</table>");
    let mut by_group: BTreeMap<&str, Vec<(&CommandInfo, String)>> = BTreeMap::new();
    for info in COMMANDS {
        let key = key_text(&map, info);
        if !key.is_empty() {
            by_group.entry(info.group).or_default().push((info, key));
        }
    }
    for (group, rows) in by_group {
        let _ = write!(
            s,
            "<h2 id=\"{}\">{}</h2><table><tr><th>Key</th><th>Command</th></tr>",
            slug(group),
            esc(group)
        );
        for (info, key) in rows {
            let _ = write!(
                s,
                "<tr><td>{}</td><td>{}</td></tr>",
                kbd(&key),
                esc(info.label)
            );
        }
        s.push_str("</table>");
    }
    s
}

// ---------------------------------------------------------------- markdown

fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn kbd(keys: &str) -> String {
    if keys.is_empty() {
        return String::new();
    }
    keys.split(" / ")
        .map(|k| {
            k.split('+')
                .map(|p| format!("<kbd>{}</kbd>", esc(p)))
                .collect::<Vec<_>>()
                .join("+")
        })
        .collect::<Vec<_>>()
        .join(" or ")
}

/// A heading's anchor: lower case, letters and digits, dashes between words.
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

/// The headings of a Markdown source as anchors, for link checks.
pub fn anchors(markdown: &str) -> Vec<String> {
    markdown
        .lines()
        .filter(|l| l.starts_with('#'))
        .map(|l| slug(l.trim_start_matches('#').trim()))
        .collect()
}

/// A small Markdown renderer for the manual's own subset: headings,
/// paragraphs, lists (bullets and numbers), tables, fenced code, quotes,
/// images, links, `code`, **bold** and *italic*. Raw HTML in the source is
/// shown as text. Links to `x.md` and `x.md#a` become `x.html` and `x.html#a`.
pub fn render_markdown(src: &str, images: &[&str]) -> String {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let t = line.trim_end();
        if t.trim().is_empty() {
            i += 1;
        } else if let Some(rest) = t.strip_prefix("```") {
            let _ = rest;
            let mut code = String::new();
            i += 1;
            while i < lines.len() && !lines[i].starts_with("```") {
                code.push_str(lines[i]);
                code.push('\n');
                i += 1;
            }
            i += 1;
            let _ = write!(out, "<pre><code>{}</code></pre>", esc(&code));
        } else if t.starts_with('#') {
            let level = t.chars().take_while(|c| *c == '#').count().min(4);
            let text = t[level..].trim();
            let _ = write!(
                out,
                "<h{level} id=\"{}\">{}</h{level}>",
                slug(text),
                inline(text, images)
            );
            i += 1;
        } else if t.starts_with('|') && lines.get(i + 1).is_some_and(|n| is_rule(n)) {
            let head = cells(t);
            i += 2;
            out.push_str("<table><tr>");
            for h in &head {
                let _ = write!(out, "<th>{}</th>", inline(h, images));
            }
            out.push_str("</tr>");
            while i < lines.len() && lines[i].trim_start().starts_with('|') {
                out.push_str("<tr>");
                for c in cells(lines[i]) {
                    let _ = write!(out, "<td>{}</td>", inline(&c, images));
                }
                out.push_str("</tr>");
                i += 1;
            }
            out.push_str("</table>");
        } else if t.starts_with("> ") || t == ">" {
            let mut text = String::new();
            while i < lines.len() && lines[i].starts_with('>') {
                text.push_str(lines[i].trim_start_matches('>').trim());
                text.push(' ');
                i += 1;
            }
            let _ = write!(
                out,
                "<blockquote><p>{}</p></blockquote>",
                inline(text.trim(), images)
            );
        } else if bullet(t).is_some() || number(t).is_some() {
            let ordered = number(t).is_some();
            out.push_str(if ordered { "<ol>" } else { "<ul>" });
            while i < lines.len() {
                let l = lines[i].trim_end();
                let item = if ordered { number(l) } else { bullet(l) };
                match item {
                    Some(first) => {
                        let mut text = first.to_string();
                        i += 1;
                        while i < lines.len()
                            && lines[i].starts_with("  ")
                            && !lines[i].trim().is_empty()
                            && bullet(lines[i].trim()).is_none()
                        {
                            text.push(' ');
                            text.push_str(lines[i].trim());
                            i += 1;
                        }
                        let _ = write!(out, "<li>{}</li>", inline(&text, images));
                    }
                    None => break,
                }
            }
            out.push_str(if ordered { "</ol>" } else { "</ul>" });
        } else {
            let mut text = String::new();
            while i < lines.len() && !lines[i].trim().is_empty() && !starts_block(lines[i]) {
                text.push_str(lines[i].trim());
                text.push(' ');
                i += 1;
            }
            if text.is_empty() {
                // A block-looking line that no rule took: show it, never loop.
                text = format!("{} ", lines[i].trim());
                i += 1;
            }
            let body = inline(text.trim(), images);
            if body.starts_with("<figure") {
                out.push_str(&body);
            } else {
                let _ = write!(out, "<p>{body}</p>");
            }
        }
    }
    out
}

fn starts_block(l: &str) -> bool {
    let t = l.trim_end();
    t.starts_with('#')
        || t.starts_with("```")
        || t.starts_with('|')
        || t.starts_with('>')
        || bullet(t).is_some()
        || number(t).is_some()
}

fn bullet(l: &str) -> Option<&str> {
    l.strip_prefix("- ").or_else(|| l.strip_prefix("* "))
}

fn number(l: &str) -> Option<&str> {
    let digits = l.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    l[digits..].strip_prefix(". ")
}

fn is_rule(l: &str) -> bool {
    let t = l.trim();
    t.starts_with('|') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

fn cells(l: &str) -> Vec<String> {
    let t = l.trim().trim_start_matches('|').trim_end_matches('|');
    t.split('|').map(|c| c.trim().to_string()).collect()
}

/// Inline markup. A bare image line becomes a figure.
fn inline(text: &str, images: &[&str]) -> String {
    if let Some(fig) = figure(text, images) {
        return fig;
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '`' => {
                if let Some(end) = find(&chars, i + 1, "`") {
                    let code: String = chars[i + 1..end].iter().collect();
                    let _ = write!(out, "<code>{}</code>", esc(&code));
                    i = end + 1;
                    continue;
                }
            }
            '*' if chars.get(i + 1) == Some(&'*') => {
                if let Some(end) = find(&chars, i + 2, "**") {
                    let inner: String = chars[i + 2..end].iter().collect();
                    let _ = write!(out, "<strong>{}</strong>", inline(&inner, images));
                    i = end + 2;
                    continue;
                }
            }
            '*' => {
                if let Some(end) = find(&chars, i + 1, "*")
                    && end > i + 1
                {
                    let inner: String = chars[i + 1..end].iter().collect();
                    let _ = write!(out, "<em>{}</em>", inline(&inner, images));
                    i = end + 1;
                    continue;
                }
            }
            '[' => {
                if let Some(close) = find(&chars, i + 1, "](")
                    && let Some(end) = find(&chars, close + 2, ")")
                {
                    let label: String = chars[i + 1..close].iter().collect();
                    let target: String = chars[close + 2..end].iter().collect();
                    let _ = write!(
                        out,
                        "<a href=\"{}\">{}</a>",
                        esc(&link_target(&target)),
                        inline(&label, images)
                    );
                    i = end + 1;
                    continue;
                }
            }
            _ => {}
        }
        out.push_str(&esc(&c.to_string()));
        i += 1;
    }
    out
}

fn figure(text: &str, images: &[&str]) -> Option<String> {
    let rest = text.strip_prefix("![")?;
    let (alt, rest) = rest.split_once("](")?;
    let path = rest.strip_suffix(')')?;
    let name = path.strip_prefix("images/")?.strip_suffix(".png")?;
    if images.contains(&name) {
        Some(format!(
            "<figure><img src=\"{}\" alt=\"{}\"><figcaption>{}</figcaption></figure>",
            esc(path),
            esc(alt),
            esc(alt)
        ))
    } else {
        Some(format!(
            "<figure><figcaption>Screenshot: {} (made by the QA suite when the manual is built)</figcaption></figure>",
            esc(alt)
        ))
    }
}

fn find(chars: &[char], from: usize, pat: &str) -> Option<usize> {
    let p: Vec<char> = pat.chars().collect();
    (from..chars.len().saturating_sub(p.len() - 1)).find(|&k| chars[k..k + p.len()] == p[..])
}

fn link_target(target: &str) -> String {
    if target.starts_with("http://") || target.starts_with("https://") {
        return target.to_string();
    }
    match target.split_once('#') {
        Some((file, anchor)) => format!("{}#{anchor}", html_name(file)),
        None => html_name(target),
    }
}

fn html_name(file: &str) -> String {
    file.strip_suffix(".md")
        .map_or_else(|| file.to_string(), |stem| format!("{stem}.html"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site() -> Vec<SiteFile> {
        build_site("test", &[])
    }

    fn text(files: &[SiteFile], name: &str) -> String {
        let f = files.iter().find(|f| f.name == name).expect(name);
        String::from_utf8(f.bytes.clone()).unwrap()
    }

    #[test]
    fn renders_the_subset() {
        let html = render_markdown(
            "# Title\n\nSome **bold**, *soft* and `code <x>` with [a link](other.md#part).\n\n- one\n- two\n\n1. a\n2. b\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n\n> note\n\n```\nraw <b>\n```\n",
            &[],
        );
        assert!(html.contains("<h1 id=\"title\">Title</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<em>soft</em>"));
        assert!(html.contains("<code>code &lt;x&gt;</code>"));
        assert!(html.contains("href=\"other.html#part\""));
        assert!(html.contains("<ul><li>one</li><li>two</li></ul>"));
        assert!(html.contains("<ol><li>a</li><li>b</li></ol>"));
        assert!(html.contains("<th>A</th>") && html.contains("<td>2</td>"));
        assert!(html.contains("<blockquote>"));
        assert!(html.contains("raw &lt;b&gt;"));
    }

    #[test]
    fn raw_html_is_shown_as_text() {
        let html = render_markdown("<script>alert(1)</script>", &[]);
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn a_missing_screenshot_is_a_caption_not_a_broken_image() {
        let none = render_markdown("![The cabinet](images/cabinet.png)", &[]);
        assert!(!none.contains("<img"));
        assert!(none.contains("The cabinet"));
        let some = render_markdown("![The cabinet](images/cabinet.png)", &["cabinet"]);
        assert!(some.contains("<img src=\"images/cabinet.png\""));
    }

    #[test]
    fn every_page_is_built_and_the_site_has_no_script_or_network_reference() {
        let files = site();
        for name in page_files() {
            let html = text(&files, &name);
            assert!(html.contains("<title>"), "{name}");
            assert!(!html.contains("<script"), "{name} has a script");
            assert!(
                !html.contains("src=\"http"),
                "{name} loads from the network"
            );
            assert!(
                !html.contains("href=\"http") || !html.contains("rel=\"stylesheet\" href=\"http")
            );
        }
        assert!(files.iter().any(|f| f.name == "style.css"));
    }

    #[test]
    fn every_link_between_pages_lands_on_a_page_and_an_anchor() {
        let files = site();
        let mut ids: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for f in files.iter().filter(|f| f.name.ends_with(".html")) {
            let html = String::from_utf8(f.bytes.clone()).unwrap();
            let found = html
                .split("id=\"")
                .skip(1)
                .filter_map(|r| r.split('"').next())
                .map(str::to_string)
                .collect();
            ids.insert(f.name.clone(), found);
        }
        let mut checked = 0;
        for f in files.iter().filter(|f| f.name.ends_with(".html")) {
            let html = String::from_utf8(f.bytes.clone()).unwrap();
            for part in html.split("href=\"").skip(1) {
                let target = part.split('"').next().unwrap();
                if target.starts_with("http") || target == "style.css" {
                    continue;
                }
                let (file, anchor) = target.split_once('#').unwrap_or((target, ""));
                let page = ids
                    .get(file)
                    .unwrap_or_else(|| panic!("{} links to missing page {target}", f.name));
                if !anchor.is_empty() {
                    assert!(
                        page.iter().any(|a| a == anchor),
                        "{} links to {target} but {file} has no such heading",
                        f.name
                    );
                }
                checked += 1;
            }
        }
        assert!(checked > 40, "only {checked} links checked");
    }

    #[test]
    fn context_topics_and_features_point_at_real_places() {
        let files = site();
        let check = |target: &str| {
            let (file, anchor) = target.split_once('#').unwrap_or((target, ""));
            let html = text(&files, file);
            assert!(
                anchor.is_empty() || html.contains(&format!("id=\"{anchor}\"")),
                "{target}"
            );
        };
        for (topic, target) in TOPICS {
            assert!(!topic.is_empty());
            check(target);
        }
        for (_, _, _, target) in FEATURES {
            check(target);
        }
        assert_eq!(topic_file("no such topic"), "index.html");
        assert_eq!(topic_file("cabinet"), "cabinet.html");
    }

    #[test]
    fn the_reference_pages_list_every_command_and_every_key() {
        let files = site();
        let commands = text(&files, "commands.html");
        for info in COMMANDS {
            assert!(
                commands.contains(&format!("<code>{}</code>", esc(info.id))),
                "{} missing from the command list",
                info.id
            );
        }
        let shortcuts = text(&files, "shortcuts.html");
        let map = Keymap::workflow();
        for info in COMMANDS {
            if let Some(key) = map.shortcut_text(info.id) {
                assert!(
                    shortcuts.contains(&esc(info.label)),
                    "{} ({key}) missing from the shortcut list",
                    info.id
                );
            }
        }
        for (what, _, _) in GESTURES {
            assert!(shortcuts.contains(&esc(what)), "{what}");
        }
    }

    #[test]
    fn the_menu_page_lists_every_menu_entry() {
        let files = site();
        let page = text(&files, "menus.html");
        for menu in MENUS {
            for (info, _) in menus::entries(menu) {
                assert!(page.contains(&esc(info.label)), "{} / {}", menu, info.id);
            }
        }
    }

    #[test]
    fn help_commands_exist_for_the_manual() {
        for id in ["help.manual", "help.context"] {
            assert!(COMMANDS.iter().any(|c| c.id == id), "{id}");
        }
    }

    #[test]
    fn slug_matches_the_heading_ids() {
        assert_eq!(slug("The 3D view"), "the-3d-view");
        assert_eq!(slug("Try it live!"), "try-it-live");
        assert_eq!(anchors("# A b\n## C"), vec!["a-b", "c"]);
    }
}
