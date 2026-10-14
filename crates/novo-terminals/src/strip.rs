//! The terminal strip editor's model: one row per physical terminal, in
//! strip order, with what it is built from and where it is wired.

use std::collections::BTreeMap;

use novo_core::{
    DeviceId, DeviceKind, PartId, Project, Result, SymbolInstance, SymbolRole, Terminal,
    TerminalId, TerminalKind, Tx,
};

use crate::naming::{kind_label, next_free_number, next_order};
use crate::occurrence::{Occurrence, State, Target, occurrences};
use crate::settings::{JoinLink, SameName, Settings};
use crate::style::style_of;

/// What a terminal row is physically built from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Build {
    /// 1-in/1-out clamps: one per drawing, more where a side takes several
    /// wires; one for a spare.
    pub clamps: usize,
    /// Links between those clamps (`clamps - 1`).
    pub links: usize,
    pub link: JoinLink,
}

/// One row of the strip editor: one terminal as it is built.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub name: String,
    pub level: u8,
    /// The terminal records behind the row. More than one only when the
    /// project joins same-named terminals and several records carry the
    /// name (see [`join_duplicates`]).
    pub terminals: Vec<TerminalId>,
    pub kind: TerminalKind,
    pub jumper: Option<u32>,
    pub part: Option<PartId>,
    /// Drawings on pages, in page order.
    pub occurrences: Vec<Occurrence>,
    pub build: Build,
}

impl Row {
    /// A terminal that is not drawn anywhere.
    pub fn is_spare(&self) -> bool {
        self.occurrences.is_empty()
    }

    /// Whether the terminal passes through, ends, or is unwired. Joined
    /// drawings count together: wired on the internal side in one drawing and
    /// on the external side in another is through.
    pub fn state(&self) -> State {
        let internal = self.occurrences.iter().any(|o| o.internal.is_connected());
        let external = self.occurrences.iter().any(|o| o.external.is_connected());
        match (internal, external) {
            (true, true) => State::Through,
            (true, false) => State::InternalEnd,
            (false, true) => State::ExternalEnd,
            (false, false) => State::Open,
        }
    }

    /// Internal targets of every drawing.
    pub fn internal_targets(&self) -> Vec<&Target> {
        self.occurrences
            .iter()
            .flat_map(|o| &o.internal.targets)
            .collect()
    }

    /// External targets of every drawing.
    pub fn external_targets(&self) -> Vec<&Target> {
        self.occurrences
            .iter()
            .flat_map(|o| &o.external.targets)
            .collect()
    }
}

/// A terminal strip as the editor shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct StripView {
    pub strip: DeviceId,
    pub tag: String,
    pub rows: Vec<Row>,
}

/// Terminal strip devices, sorted by tag.
pub fn strips(project: &Project) -> Vec<DeviceId> {
    let mut strips: Vec<_> = project
        .devices()
        .filter(|d| d.kind == DeviceKind::TerminalStrip)
        .collect();
    strips.sort_by(|a, b| a.tag.cmp(&b.tag).then(a.id.cmp(&b.id)));
    strips.into_iter().map(|d| d.id).collect()
}

fn row(project: &Project, settings: Settings, group: &[&Terminal]) -> Row {
    let first = group[0];
    let occurrences: Vec<Occurrence> = group
        .iter()
        .flat_map(|t| occurrences(project, t.id))
        .collect();
    let clamps = if occurrences.is_empty() {
        group.len()
    } else {
        occurrences.iter().map(Occurrence::clamps).sum()
    };
    Row {
        name: first.name.clone(),
        level: first.level,
        terminals: group.iter().map(|t| t.id).collect(),
        kind: first.kind,
        jumper: first.jumper,
        part: style_of(project, first.id).and_then(|s| s.part),
        occurrences,
        build: Build {
            clamps,
            links: clamps.saturating_sub(1),
            link: settings.join_link,
        },
    }
}

/// The strip editor rows of a strip. With the "join" rule, terminals that
/// share a name and level are one row; with "refuse" each record is its own
/// row and [`crate::check`] flags the duplicates.
pub fn strip_view(project: &Project, strip: DeviceId) -> StripView {
    let settings = Settings::of(project);
    let terminals = project.terminals_of_strip(strip);
    let mut groups: Vec<Vec<&Terminal>> = Vec::new();
    match settings.same_name {
        SameName::Refuse => groups.extend(terminals.into_iter().map(|t| vec![t])),
        SameName::Join => {
            let mut index: BTreeMap<(String, u8), usize> = BTreeMap::new();
            for t in terminals {
                let key = (t.name.to_ascii_uppercase(), t.level);
                match index.get(&key) {
                    Some(&i) => groups[i].push(t),
                    None => {
                        index.insert(key, groups.len());
                        groups.push(vec![t]);
                    }
                }
            }
        }
    }
    StripView {
        strip,
        tag: project
            .device(strip)
            .map(|d| d.tag.to_string())
            .unwrap_or_default(),
        rows: groups.iter().map(|g| row(project, settings, g)).collect(),
    }
}

/// Plain sentences that say why a terminal is connected the way it is, for
/// the status bar and the properties panel. Terminal connection is never a
/// guess: these name the rule that applies.
pub fn explain(project: &Project, terminal: TerminalId) -> Vec<String> {
    let Some(t) = project.terminal(terminal) else {
        return Vec::new();
    };
    let settings = Settings::of(project);
    let designation = project
        .terminal_designation(terminal)
        .unwrap_or_else(|| t.name.clone());
    let view = strip_view(project, t.strip);
    let Some(row) = view.rows.iter().find(|r| r.terminals.contains(&terminal)) else {
        return Vec::new();
    };
    let page_name = |o: &Occurrence| {
        project
            .page(o.page)
            .map(|p| p.name.clone())
            .unwrap_or_default()
    };
    let mut out = vec![format!(
        "{designation} is a {} terminal, {}.",
        kind_label(row.kind),
        row.state().label()
    )];
    match row.occurrences.len() {
        0 => out.push(format!(
            "{designation} is a spare: it is not drawn on any page."
        )),
        1 => out.push(format!(
            "{designation} is drawn once, on page {}.",
            page_name(&row.occurrences[0])
        )),
        n => {
            let pages: Vec<String> = row.occurrences.iter().map(page_name).collect();
            out.push(match settings.same_name {
                SameName::Join => format!(
                    "{designation} is drawn {n} times (pages {}). The project joins terminals with the same name, so these are one terminal: {} clamps linked by {} {}.",
                    pages.join(", "),
                    row.build.clamps,
                    row.build.links,
                    plural(settings.join_link.label(), row.build.links),
                ),
                SameName::Refuse => format!(
                    "{designation} is drawn {n} times (pages {}). The project refuses duplicate terminal names, so only the first counts.",
                    pages.join(", ")
                ),
            });
        }
    }
    if row.terminals.len() > 1 && settings.same_name == SameName::Join {
        out.push(format!(
            "{} separate records carry this name; joining them makes the electrical connection match the strip.",
            row.terminals.len()
        ));
    }
    if let Some(group) = row.jumper {
        let others: Vec<String> = view
            .rows
            .iter()
            .filter(|r| r.jumper == Some(group) && r.name != row.name)
            .map(|r| r.name.clone())
            .collect();
        if !others.is_empty() {
            out.push(format!(
                "Jumpered to {} (jumper {group}).",
                others.join(", ")
            ));
        }
    }
    out
}

fn plural(word: &str, n: usize) -> String {
    if n == 1 {
        word.to_owned()
    } else {
        format!("{word}s")
    }
}

// ----- strip editor edits ----------------------------------------------------

/// Puts terminals in this order on their strip. Terminals not listed keep
/// their place after the listed ones.
pub fn reorder(tx: &mut Tx<'_>, strip: DeviceId, order: &[TerminalId]) -> Result<()> {
    let mut rest: Vec<TerminalId> = tx
        .project()
        .terminals_of_strip(strip)
        .into_iter()
        .map(|t| t.id)
        .filter(|id| !order.contains(id))
        .collect();
    let mut all: Vec<TerminalId> = order.to_vec();
    all.append(&mut rest);
    for (i, id) in all.into_iter().enumerate() {
        let position = i64::try_from(i + 1).unwrap_or(i64::MAX).saturating_mul(10);
        tx.update::<Terminal>(id, |t| t.order = position)?;
    }
    Ok(())
}

/// Adds `count` spare terminals at the end of a strip, numbered next free.
pub fn add_spares(tx: &mut Tx<'_>, strip: DeviceId, count: usize) -> Result<Vec<TerminalId>> {
    let mut added = Vec::with_capacity(count);
    for _ in 0..count {
        let name = next_free_number(tx.project(), strip);
        let order = next_order(tx.project(), strip);
        let id = tx.add_terminal(strip, name)?;
        tx.update::<Terminal>(id, |t| t.order = order)?;
        added.push(id);
    }
    Ok(added)
}

/// A jumper group number not yet used on the strip.
pub fn free_jumper(project: &Project, strip: DeviceId) -> u32 {
    project
        .terminals_of_strip(strip)
        .iter()
        .filter_map(|t| t.jumper)
        .max()
        .map_or(1, |j| j + 1)
}

/// Bridges terminals of one strip with a new jumper, or removes them from
/// any jumper with `None`. Returns the jumper group used.
pub fn set_jumper(
    tx: &mut Tx<'_>,
    terminals: &[TerminalId],
    jumper: Option<u32>,
) -> Result<Option<u32>> {
    for &id in terminals {
        tx.update::<Terminal>(id, |t| t.jumper = jumper)?;
    }
    Ok(jumper)
}

/// Changes the kind of a terminal and redraws it with its strip's symbol for
/// that kind.
pub fn set_kind(tx: &mut Tx<'_>, terminal: TerminalId, kind: TerminalKind) -> Result<()> {
    tx.update::<Terminal>(terminal, |t| t.kind = kind)?;
    if let Some(style) = style_of(tx.project(), terminal) {
        let symbols: Vec<_> = tx
            .project()
            .symbols_of_terminal(terminal)
            .into_iter()
            .filter(
                |s| matches!(&s.role, SymbolRole::Terminal { terminals } if terminals.len() == 1),
            )
            .map(|s| s.id)
            .collect();
        for id in symbols {
            tx.update::<SymbolInstance>(id, |s| s.symbol = style.symbol.clone())?;
        }
    }
    Ok(())
}

/// Merges terminal records that share a name and level on a strip into the
/// first of them, so the drawings become one terminal electrically as well.
/// Returns the records removed.
pub fn join_duplicates(tx: &mut Tx<'_>, strip: DeviceId) -> Result<Vec<TerminalId>> {
    let mut keep: BTreeMap<(String, u8), TerminalId> = BTreeMap::new();
    let mut merges: Vec<(TerminalId, TerminalId)> = Vec::new();
    for t in tx.project().terminals_of_strip(strip) {
        let key = (t.name.to_ascii_uppercase(), t.level);
        match keep.get(&key) {
            Some(&into) => merges.push((t.id, into)),
            None => {
                keep.insert(key, t.id);
            }
        }
    }
    for &(gone, into) in &merges {
        let symbols: Vec<_> = tx
            .project()
            .symbols_of_terminal(gone)
            .into_iter()
            .map(|s| s.id)
            .collect();
        for symbol in symbols {
            tx.update::<SymbolInstance>(symbol, |s| {
                if let SymbolRole::Terminal { terminals } = &mut s.role {
                    for t in terminals.iter_mut() {
                        if *t == gone {
                            *t = into;
                        }
                    }
                }
            })?;
        }
        tx.remove(gone)?;
    }
    Ok(merges.into_iter().map(|(gone, _)| gone).collect())
}

/// What [`renumber`] would do: each numbered terminal with its current and
/// new name, in strip order. Terminals that keep their number are included,
/// so a preview can show the whole strip.
pub fn renumber_preview(project: &Project, strip: DeviceId) -> Vec<(TerminalId, String, String)> {
    let numbered: Vec<(TerminalId, String)> = project
        .terminals_of_strip(strip)
        .into_iter()
        .filter(|t| !t.name.is_empty() && t.name.chars().all(|c| c.is_ascii_digit()))
        .map(|t| (t.id, t.name.clone()))
        .collect();
    // Records that share a number keep sharing it (a joined terminal stays
    // joined).
    let mut new_names: Vec<(String, String)> = Vec::new();
    for (_, old) in &numbered {
        if !new_names.iter().any(|(o, _)| o == old) {
            let next = (new_names.len() + 1).to_string();
            new_names.push((old.clone(), next));
        }
    }
    numbered
        .into_iter()
        .map(|(id, old)| {
            let new = new_names
                .iter()
                .find(|(o, _)| *o == old)
                .map(|(_, n)| n.clone())
                .expect("every old name is mapped");
            (id, old, new)
        })
        .collect()
}

/// Renumbers the numbered terminals of a strip `1, 2, 3, ...` in strip
/// order, as [`renumber_preview`] shows. PE and N terminals and other named
/// ones keep their names.
pub fn renumber(tx: &mut Tx<'_>, strip: DeviceId) -> Result<()> {
    for (id, _, name) in renumber_preview(tx.project(), strip) {
        tx.update::<Terminal>(id, |t| t.name = name)?;
    }
    Ok(())
}

/// Moves a terminal to another level of a multi-level terminal (from 1).
pub fn set_level(tx: &mut Tx<'_>, terminal: TerminalId, level: u8) -> Result<()> {
    tx.update::<Terminal>(terminal, |t| t.level = level.max(1))
}

/// What [`rename`] would do: terminals of the strip whose name contains
/// `find`, with the name after replacing every occurrence by `replace`.
/// Case-sensitive; an empty `find` matches nothing.
pub fn rename_preview(
    project: &Project,
    strip: DeviceId,
    find: &str,
    replace: &str,
) -> Vec<(TerminalId, String, String)> {
    if find.is_empty() {
        return Vec::new();
    }
    project
        .terminals_of_strip(strip)
        .into_iter()
        .filter(|t| t.name.contains(find))
        .map(|t| (t.id, t.name.clone(), t.name.replace(find, replace)))
        .collect()
}

/// Find and replace in the terminal names of a strip, as one edit.
pub fn rename(tx: &mut Tx<'_>, strip: DeviceId, find: &str, replace: &str) -> Result<usize> {
    let changes = rename_preview(tx.project(), strip, find, replace);
    for (id, _, name) in &changes {
        tx.update::<Terminal>(*id, |t| t.name = name.clone())?;
    }
    Ok(changes.len())
}
