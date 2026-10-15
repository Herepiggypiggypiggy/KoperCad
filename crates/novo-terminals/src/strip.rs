//! The terminal strip editor's model: one row per physical terminal, in
//! strip order, with what it is built from and where it is wired.

use std::collections::HashMap;

use novo_core::{
    DeviceId, DeviceKind, JoinLink, ModelError, PartId, Project, Result, SameName, SymbolInstance,
    SymbolRole, Terminal, TerminalId, TerminalKind, TerminalRules, Tx, natural_cmp,
};

use crate::geometry::redraw;
use crate::naming::{next_free_number, next_order};
use crate::occurrence::{Occurrence, State, Target, occurrences};
use crate::rules::rules;
use crate::strings;
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
    /// The terminal records behind the row. More than one only under the
    /// Join rule when several records carry the name; the netlist joins
    /// them, and [`join_duplicates`] tidies them into one record.
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

/// Terminal strip devices, sorted by tag with numbers by value (`-X2`
/// before `-X10`).
pub fn strips(project: &Project) -> Vec<DeviceId> {
    let mut strips: Vec<_> = project
        .devices()
        .filter(|d| d.kind == DeviceKind::TerminalStrip)
        .map(|d| (d.tag.to_string(), d.id))
        .collect();
    strips.sort_by(|a, b| natural_cmp(&a.0, &b.0).then(a.1.cmp(&b.1)));
    strips.into_iter().map(|(_, id)| id).collect()
}

fn row(project: &Project, rules: TerminalRules, group: &[&Terminal]) -> Row {
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
        part: crate::style::terminal_part(project, first.id),
        occurrences,
        build: Build {
            clamps,
            links: clamps.saturating_sub(1),
            link: rules.join_link,
        },
    }
}

/// The strip editor rows of a strip. Under the Join rule, records that
/// share a name and level are one row, exactly as the netlist joins them;
/// under Refuse and Off each record is its own row.
pub fn strip_view(project: &Project, strip: DeviceId) -> StripView {
    let rules = rules(project);
    let terminals = project.terminals_of_strip(strip);
    let mut groups: Vec<Vec<&Terminal>> = Vec::new();
    match rules.same_name {
        SameName::Refuse | SameName::Off => {
            groups.extend(terminals.into_iter().map(|t| vec![t]));
        }
        SameName::Join => {
            let mut index: HashMap<(&str, u8), usize> = HashMap::new();
            for t in terminals {
                match index.get(&(t.name.as_str(), t.level)) {
                    Some(&i) => groups[i].push(t),
                    None => {
                        index.insert((t.name.as_str(), t.level), groups.len());
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
        rows: groups.iter().map(|g| row(project, rules, g)).collect(),
    }
}

/// Plain sentences that say why a terminal is connected the way it is, for
/// the status bar and the properties panel. Terminal connection is never a
/// guess: these name the rule that applies.
pub fn explain(project: &Project, terminal: TerminalId) -> Vec<String> {
    let Some(t) = project.terminal(terminal) else {
        return Vec::new();
    };
    let rules = rules(project);
    let designation = project
        .terminal_designation(terminal)
        .unwrap_or_else(|| t.name.clone());
    let on_strip = project.terminals_of_strip(t.strip);
    let same_name: Vec<&Terminal> = on_strip
        .iter()
        .copied()
        .filter(|o| o.name == t.name && o.level == t.level)
        .collect();
    let group = if rules.same_name == SameName::Join {
        same_name.clone()
    } else {
        vec![t]
    };
    let row = row(project, rules, &group);
    let page_name = |o: &Occurrence| {
        project
            .page(o.page)
            .map(|p| p.name.clone())
            .unwrap_or_default()
    };
    let mut out = vec![strings::explain_kind_and_state(
        &designation,
        row.kind,
        row.state(),
    )];
    let pages: Vec<String> = row.occurrences.iter().map(page_name).collect();
    match pages.as_slice() {
        [] => out.push(strings::explain_spare(&designation)),
        [page] => out.push(strings::explain_drawn_once(&designation, page)),
        _ => out.push(match rules.same_name {
            SameName::Join => strings::explain_drawn_joined(
                &designation,
                &pages,
                row.build.clamps,
                row.build.links,
                rules.join_link,
            ),
            SameName::Refuse => strings::explain_drawn_refused(&designation, &pages),
            SameName::Off => strings::explain_drawn_one_record(&designation, &pages),
        }),
    }
    if same_name.len() > 1 {
        out.push(match rules.same_name {
            SameName::Join => strings::explain_records_joined(same_name.len()),
            SameName::Refuse | SameName::Off => strings::explain_records_separate(same_name.len()),
        });
    }
    if let Some(group) = t.jumper {
        let mut others: Vec<String> = on_strip
            .iter()
            .filter(|o| o.jumper == Some(group) && o.name != t.name)
            .map(|o| o.name.clone())
            .collect();
        others.sort_by(|a, b| natural_cmp(a, b));
        others.dedup();
        if !others.is_empty() {
            out.push(strings::explain_jumpered(&others, group));
        }
    }
    out
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
/// any jumper with `None`. Returns the jumper group used. Terminals of
/// several strips are refused: the netlist bridges jumper groups per strip.
pub fn set_jumper(
    tx: &mut Tx<'_>,
    terminals: &[TerminalId],
    jumper: Option<u32>,
) -> Result<Option<u32>> {
    let mut strip = None;
    for &id in terminals {
        let t = tx
            .project()
            .terminal(id)
            .ok_or(ModelError::NotFound(id.into()))?;
        if jumper.is_some() && strip.is_some_and(|s| s != t.strip) {
            return Err(ModelError::Format(strings::not_on_one_strip()));
        }
        strip = Some(t.strip);
    }
    for &id in terminals {
        tx.update::<Terminal>(id, |t| t.jumper = jumper)?;
    }
    // Connected terminals are drawn with their own symbol (see
    // `style::style_of`): redraw the drawings that follow the strip's style.
    for &id in terminals {
        if let Some(style) = style_of(tx.project(), id) {
            let symbols: Vec<_> = tx
                .project()
                .symbols_of_terminal(id)
                .into_iter()
                .filter(|s| crate::style::draws_only_inherited(tx.project(), s))
                .map(|s| s.id)
                .collect();
            for symbol in symbols {
                redraw(tx, symbol, &style.symbol)?;
            }
        }
    }
    Ok(jumper)
}

/// Connects terminals of one strip, the Connect terminals command: they
/// share a new jumper group (`free_jumper`) and are drawn as connected
/// terminals. Returns the group. Fewer than two terminals are refused.
pub fn connect_terminals(tx: &mut Tx<'_>, terminals: &[TerminalId]) -> Result<u32> {
    let mut ids = terminals.to_vec();
    ids.sort();
    ids.dedup();
    let Some(first) = ids.first().and_then(|&t| tx.project().terminal(t)) else {
        return Err(ModelError::Format(strings::not_on_one_strip()));
    };
    if ids.len() < 2 {
        return Err(ModelError::Format(strings::connect_needs_two()));
    }
    let group = free_jumper(tx.project(), first.strip);
    set_jumper(tx, &ids, Some(group))?;
    Ok(group)
}

/// Changes the kind of a terminal and redraws it with its strip's symbol for
/// that kind, refitting the connection points to the new symbol. A terminal
/// with a symbol override of its own keeps its drawing. Refused, with
/// nothing changed, when a drawing's wires leave no room for the new symbol
/// (see [`crate::redraw`]).
pub fn set_kind(tx: &mut Tx<'_>, terminal: TerminalId, kind: TerminalKind) -> Result<()> {
    tx.update::<Terminal>(terminal, |t| t.kind = kind)?;
    if let Some(style) = style_of(tx.project(), terminal) {
        // A terminal drawn with its own symbol keeps it: that symbol does
        // not depend on the kind, and may be one the library does not know.
        let symbols: Vec<_> = tx
            .project()
            .symbols_of_terminal(terminal)
            .into_iter()
            .filter(|s| crate::style::draws_only_inherited(tx.project(), s))
            .map(|s| s.id)
            .collect();
        for id in symbols {
            redraw(tx, id, &style.symbol)?;
        }
    }
    Ok(())
}

/// Merges terminal records that share a name and level on a strip into the
/// first of them in strip order, so each name is one record. Under the Join
/// rule the netlist already treats them as one terminal; merging is the
/// tidy-up before switching to Refuse, and under Off it is how the user
/// joins them on purpose. The kept record takes a removed record's jumper,
/// part and properties where it has none of its own; its kind wins.
/// Returns the records removed.
pub fn join_duplicates(tx: &mut Tx<'_>, strip: DeviceId) -> Result<Vec<TerminalId>> {
    let groups: Vec<Vec<TerminalId>> = tx
        .project()
        .terminal_designation_groups()
        .into_iter()
        .filter(|g| {
            tx.project()
                .terminal(g[0])
                .is_some_and(|t| t.strip == strip)
        })
        .collect();
    let mut removed = Vec::new();
    for group in groups {
        let into = group[0];
        for &gone in &group[1..] {
            let old = tx
                .project()
                .terminal(gone)
                .ok_or(ModelError::NotFound(gone.into()))?
                .clone();
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
            tx.update::<Terminal>(into, |t| {
                t.jumper = t.jumper.or(old.jumper);
                t.part = t.part.or(old.part);
                for (key, value) in old.properties {
                    t.properties.entry(key).or_insert(value);
                }
            })?;
            removed.push(gone);
        }
    }
    Ok(removed)
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
    let changes: Vec<(TerminalId, String)> = renumber_preview(tx.project(), strip)
        .into_iter()
        .filter(|(_, old, new)| old != new)
        .map(|(id, _, new)| (id, new))
        .collect();
    set_names(tx, strip, &changes)
}

/// Gives several terminals of one strip new names in one transaction.
/// novo-core checks every terminal update on its own, so under the Refuse
/// rule a name that is still held by another record being renamed (a swap,
/// or `3 -> 2` while `2 -> 3` is still to come) would be refused in
/// between. Every record therefore first takes a temporary name with a
/// prefix no terminal of the strip uses, then its final name. Only the end
/// state has to be valid; the transaction stays one undo step.
fn set_names(tx: &mut Tx<'_>, strip: DeviceId, changes: &[(TerminalId, String)]) -> Result<()> {
    if changes.is_empty() {
        return Ok(());
    }
    let mut prefix = String::from("\u{1}renaming~");
    while tx
        .project()
        .terminals_of_strip(strip)
        .iter()
        .any(|t| t.name.starts_with(&prefix))
    {
        prefix.push('~');
    }
    for (i, (id, _)) in changes.iter().enumerate() {
        tx.update::<Terminal>(*id, |t| t.name = format!("{prefix}{i}"))?;
    }
    for (id, name) in changes {
        tx.update::<Terminal>(*id, |t| t.name = name.clone())?;
    }
    Ok(())
}

/// Moves a terminal to another level of a multi-level terminal (from 1).
/// Refused when the level is 0, when another terminal of the strip already
/// has this name on that level, or when a multi-level drawing of the
/// terminal has fewer levels.
pub fn set_level(tx: &mut Tx<'_>, terminal: TerminalId, level: u8) -> Result<()> {
    let t = tx
        .project()
        .terminal(terminal)
        .ok_or(ModelError::NotFound(terminal.into()))?;
    if level == 0 {
        return Err(ModelError::InvalidValue {
            id: terminal.into(),
            field: "level",
        });
    }
    if t.level == level {
        return Ok(());
    }
    if let Some(other) = tx
        .project()
        .terminals_of_strip(t.strip)
        .into_iter()
        .find(|o| o.id != terminal && o.name == t.name && o.level == level)
    {
        return Err(ModelError::AlreadyExists(other.id.into()));
    }
    for symbol in tx.project().symbols_of_terminal(terminal) {
        let SymbolRole::Terminal { terminals } = &symbol.role else {
            continue;
        };
        let mut distinct = terminals.clone();
        distinct.sort();
        distinct.dedup();
        if distinct.len() > 1 && usize::from(level) > distinct.len() {
            return Err(ModelError::InvalidValue {
                id: terminal.into(),
                field: "level",
            });
        }
    }
    tx.update::<Terminal>(terminal, |t| t.level = level)
}

/// Why a name in a [`rename_preview`] cannot be applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameProblem {
    /// The new name would be empty.
    Empty,
    /// Another terminal of the strip has (or would get) this name on the
    /// same level. Under the Join rule that would silently connect them.
    Taken(TerminalId),
}

/// One terminal [`rename`] would change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameChange {
    pub terminal: TerminalId,
    pub old: String,
    pub new: String,
    /// Set when the change cannot be applied; [`rename`] then refuses the
    /// whole edit.
    pub problem: Option<RenameProblem>,
}

/// What [`rename`] would do: terminals of the strip whose name contains
/// `find`, with the name after replacing every occurrence by `replace`, and
/// any problem with the result (shown before the edit is applied).
/// Case-sensitive; an empty `find` matches nothing; names that would not
/// change are left out.
pub fn rename_preview(
    project: &Project,
    strip: DeviceId,
    find: &str,
    replace: &str,
) -> Vec<RenameChange> {
    if find.is_empty() {
        return Vec::new();
    }
    let terminals = project.terminals_of_strip(strip);
    let renamed: HashMap<TerminalId, String> = terminals
        .iter()
        .filter(|t| t.name.contains(find))
        .map(|t| (t.id, t.name.replace(find, replace)))
        .filter(|(id, new)| project.terminal(*id).is_some_and(|t| t.name != *new))
        .collect();
    let final_name = |t: &Terminal| renamed.get(&t.id).unwrap_or(&t.name).clone();
    terminals
        .iter()
        .filter_map(|t| {
            let new = renamed.get(&t.id)?.clone();
            let problem = if new.trim().is_empty() {
                Some(RenameProblem::Empty)
            } else {
                terminals
                    .iter()
                    .find(|o| {
                        o.id != t.id
                            && o.level == t.level
                            && final_name(o) == new
                            // Records that already shared a name may keep
                            // sharing it.
                            && o.name != t.name
                    })
                    .map(|o| RenameProblem::Taken(o.id))
            };
            Some(RenameChange {
                terminal: t.id,
                old: t.name.clone(),
                new,
                problem,
            })
        })
        .collect()
}

/// Find and replace in the terminal names of a strip, as one edit. Refused
/// with nothing changed when [`rename_preview`] shows a problem: an empty
/// name ([`ModelError::InvalidValue`]) or a name taken by another terminal
/// ([`ModelError::AlreadyExists`] naming it).
pub fn rename(tx: &mut Tx<'_>, strip: DeviceId, find: &str, replace: &str) -> Result<usize> {
    let changes = rename_preview(tx.project(), strip, find, replace);
    for change in &changes {
        match change.problem {
            Some(RenameProblem::Empty) => {
                return Err(ModelError::InvalidValue {
                    id: change.terminal.into(),
                    field: "name",
                });
            }
            Some(RenameProblem::Taken(other)) => {
                return Err(ModelError::AlreadyExists(other.into()));
            }
            None => {}
        }
    }
    let names: Vec<(TerminalId, String)> = changes
        .iter()
        .map(|c| (c.terminal, c.new.clone()))
        .collect();
    set_names(tx, strip, &names)?;
    Ok(changes.len())
}
