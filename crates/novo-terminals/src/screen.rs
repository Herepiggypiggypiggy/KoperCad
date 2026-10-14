//! The terminal connection screen: per strip, every terminal with what is
//! wired to it inside the cabinet and in the field.
//!
//! This is the data and a plain-text rendering. The drawn page plugs into
//! the generated-page framework of WS-13 once that lands; it reads the same
//! [`Screen`].

use std::fmt::Write as _;

use novo_core::{DeviceId, Project};

use crate::occurrence::Target;
use crate::strings;
use crate::strip::strip_view;

/// One line of a terminal connection screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenRow {
    /// Designations reached from the internal side (`-K1:14`).
    pub internal: Vec<String>,
    /// Terminal name within the strip.
    pub terminal: String,
    /// Terminal kind label (`feed-through`, `PE`, ...).
    pub kind: String,
    /// Designations reached from the external side (`-M1:U1`).
    pub external: Vec<String>,
    /// Cable and core of external wires (`-W1 BK`).
    pub cables: Vec<String>,
    /// Jumper group, if bridged.
    pub jumper: Option<u32>,
    /// Pages the terminal is drawn on.
    pub pages: Vec<String>,
    /// How the terminal is built, e.g. `2 clamps + 1 connector`.
    pub build: String,
    pub spare: bool,
}

/// The terminal connection screen of one strip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screen {
    pub strip: DeviceId,
    pub tag: String,
    pub rows: Vec<ScreenRow>,
}

/// Distinct items in first-seen order.
fn distinct(items: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(item.clone()))
        .collect()
}

fn designations(targets: &[&Target]) -> Vec<String> {
    distinct(targets.iter().map(|t| t.designation.clone()))
}

/// Builds the connection screen of a strip from the model.
pub fn screen(project: &Project, strip: DeviceId) -> Screen {
    let view = strip_view(project, strip);
    let rows = view
        .rows
        .iter()
        .map(|row| {
            let external = row.external_targets();
            let cables = distinct(
                row.occurrences
                    .iter()
                    .flat_map(|o| &o.external.wires)
                    .filter_map(|&w| {
                        let (cable, core) = project.core_of_wire(w)?;
                        let cable = project.cable(cable)?;
                        Some(format!("{} {}", cable.tag, cable.cores[core].name))
                    }),
            );
            let pages = distinct(
                row.occurrences
                    .iter()
                    .filter_map(|o| project.page(o.page).map(|p| p.name.clone())),
            );
            let build = strings::build(row.build.clamps, row.build.links, row.build.link);
            ScreenRow {
                internal: designations(&row.internal_targets()),
                terminal: row.name.clone(),
                kind: strings::kind_label(row.kind).to_owned(),
                external: designations(&external),
                cables,
                jumper: row.jumper,
                pages,
                build,
                spare: row.is_spare(),
            }
        })
        .collect();
    Screen {
        strip,
        tag: view.tag,
        rows,
    }
}

impl Screen {
    /// A plain-text table: internal targets, terminal, external targets,
    /// cable, jumper, pages.
    pub fn to_text(&self) -> String {
        let cells: Vec<[String; 6]> = self
            .rows
            .iter()
            .map(|r| {
                [
                    r.internal.join(", "),
                    if r.spare {
                        strings::screen_spare(&r.terminal)
                    } else {
                        r.terminal.clone()
                    },
                    r.external.join(", "),
                    r.cables.join(", "),
                    r.jumper.map(|j| j.to_string()).unwrap_or_default(),
                    r.pages.join(", "),
                ]
            })
            .collect();
        let header = strings::SCREEN_HEADER;
        let mut widths = header.map(|h| h.chars().count());
        for row in &cells {
            for (w, c) in widths.iter_mut().zip(row) {
                *w = (*w).max(c.chars().count());
            }
        }
        let mut out = String::new();
        let _ = writeln!(out, "{}", strings::screen_title(&self.tag));
        let line = |out: &mut String, row: &[String]| {
            let padded: Vec<String> = row
                .iter()
                .zip(widths)
                .map(|(c, w)| format!("{c:<w$}"))
                .collect();
            let _ = writeln!(out, "{}", padded.join(" | ").trim_end());
        };
        line(&mut out, &header.map(String::from));
        line(&mut out, &widths.map(|w| "-".repeat(w)));
        for row in &cells {
            line(&mut out, row);
        }
        out
    }
}
