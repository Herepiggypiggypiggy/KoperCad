//! Generate pages from a spec sheet: one row per macro placement.
//!
//! A machine builder keeps a list of what a machine has: eight motors with
//! their power, three valves, a safety chain. [`generate`] turns that list into
//! pages: each row names a macro from the library and the values of its
//! parameters, rows with the same **Page** go on the same page (a new page is
//! started when one is full), and every placement is a linked block instance
//! with fresh tags, so the result is an ordinary project the user keeps editing.
//!
//! # The sheet
//!
//! | Column | Meaning |
//! | --- | --- |
//! | `Macro` | the macro to place (required) |
//! | `Page` | title of the page it goes on; rows with the same title share pages |
//! | `Structure` | function and location of the page, e.g. `=A1+CAB1` (first row of a page decides) |
//! | `When` | an expression over the project options (`opt.conveyor`); a row whose condition is false is left out |
//! | any other header | the macro parameter of that name; an empty cell takes the default |
//!
//! A header that is neither a known column nor a parameter of a macro the sheet
//! uses is reported, so a typo in `Powr` never silently becomes a default.
//!
//! # Rules
//!
//! - **All or nothing.** Every row is checked first (macro exists, values fit
//!   their ranges and lists, expressions evaluate); if any row fails, nothing is
//!   generated and every problem is returned with its line.
//! - **One undo step.**
//! - **Round trip.** [`spec_from_project`] writes the same sheet back from the
//!   placed blocks, so a sheet edited in Excel can regenerate the pages it
//!   came from ([`Mode::Replace`] removes the pages a previous run made).
//!
//! Layout is a simple column flow: macros stack down the page, a new column
//! starts when the page is full, a new page when the columns are. It is meant
//! as a first draft the designer arranges, not as final drawing.

use std::collections::BTreeMap;

use novo_core::{
    BlockDefinition, DeviceTag, ModelError, PageId, Point, Project, Record, Tx, Value,
};

use crate::expr::{Expr, Vars};
use crate::macro_place::{Report, WireNumbers};
use crate::options::{Assignment, vars as option_vars};
use crate::parametric::{
    self, ParametricOptions, Problem, base_name, instantiate, params, place_parametric,
};

/// Property on a generated page: the key of the group it was made for.
pub const GENERATED_KEY: &str = "kc.generated";

/// The most placements one generation makes (rows times repeats), so a
/// sheet cannot ask for a million devices.
pub const MAX_PLACEMENTS: usize = 2000;

const MARGIN_LEFT: f64 = 30.0;
const MARGIN_TOP: f64 = 40.0;
const MARGIN_BOTTOM: f64 = 40.0;
const MARGIN_RIGHT: f64 = 20.0;
const GAP: f64 = 20.0;
/// Padding around a symbol's origin and pins, in mm: symbol bodies are drawn
/// around their connection points.
const SYMBOL_PAD: f64 = 12.0;

const META: [&str; 4] = ["macro", "page", "structure", "when"];

/// A table of text cells with a header row.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SpecSheet {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl SpecSheet {
    /// Parses CSV (comma or semicolon separated, `"` quoting, a BOM is
    /// ignored). The first non-empty line is the header.
    pub fn from_csv(text: &str) -> SpecSheet {
        let text = text.trim_start_matches('\u{feff}');
        let first = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
        let sep = if first.matches(';').count() > first.matches(',').count() {
            ';'
        } else {
            ','
        };
        let mut lines = parse_csv(text, sep).into_iter();
        let headers = lines.next().unwrap_or_default();
        let rows = lines
            .filter(|r| r.iter().any(|c| !c.trim().is_empty()))
            .collect();
        SpecSheet { headers, rows }
    }

    /// Writes CSV with commas, quoting where needed.
    pub fn to_csv(&self) -> String {
        let quote = |c: &str| {
            if c.contains([',', '"', '\n', ';']) {
                format!("\"{}\"", c.replace('"', "\"\""))
            } else {
                c.to_owned()
            }
        };
        let mut out = String::new();
        for row in std::iter::once(&self.headers).chain(self.rows.iter()) {
            out.push_str(&row.iter().map(|c| quote(c)).collect::<Vec<_>>().join(","));
            out.push('\n');
        }
        out
    }

    fn column(&self, name: &str) -> Option<usize> {
        self.headers
            .iter()
            .position(|h| h.trim().eq_ignore_ascii_case(name))
    }
}

fn parse_csv(text: &str, sep: char) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    cell.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                cell.push(c);
            }
        } else if c == '"' {
            quoted = true;
        } else if c == sep {
            row.push(std::mem::take(&mut cell));
        } else if c == '\n' || c == '\r' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            row.push(std::mem::take(&mut cell));
            rows.push(std::mem::take(&mut row));
        } else {
            cell.push(c);
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    rows
}

/// A problem in the sheet, with the line it is on (the header is line 1).
#[derive(Debug, Clone, PartialEq)]
pub struct SheetProblem {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for SheetProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

/// What to do with pages a previous run made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Add new pages next to what is there.
    #[default]
    Add,
    /// Remove the pages earlier runs generated first, then generate.
    Replace,
}

/// What a generation does or did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Generated {
    pub pages: Vec<PageId>,
    /// Placements made, with the tags they took (for the preview).
    pub placements: Vec<RowPlacement>,
    /// Rows left out because their `When` is false.
    pub skipped: Vec<usize>,
    /// Pages of an earlier run removed ([`Mode::Replace`]).
    pub replaced_pages: usize,
    /// Headers that matched nothing.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RowPlacement {
    pub line: usize,
    pub macro_name: String,
    pub page: PageId,
    pub reports: Vec<Report>,
}

/// Finds a macro by name.
pub type Lookup<'a> = &'a dyn Fn(&str) -> Option<BlockDefinition>;

struct Row {
    line: usize,
    macro_name: String,
    page: String,
    structure: Option<DeviceTag>,
    values: Vec<(String, Value)>,
    definition: BlockDefinition,
    extent: (f64, f64),
    copies: usize,
}

/// The rows ready to place, the lines left out by `When`, and header warnings.
type Prepared = (Vec<Row>, Vec<usize>, Vec<String>);

/// Checks every row and prepares it; nothing is changed.
fn prepare(
    sheet: &SpecSheet,
    lookup: Lookup<'_>,
    options: &Vars,
) -> Result<Prepared, Vec<SheetProblem>> {
    let mut problems = Vec::new();
    let mut rows = Vec::new();
    let mut skipped = Vec::new();
    let col_macro = sheet.column("macro");
    let Some(col_macro) = col_macro else {
        return Err(vec![SheetProblem {
            line: 1,
            message: "the sheet has no 'Macro' column".into(),
        }]);
    };
    let (col_page, col_structure, col_when) = (
        sheet.column("page"),
        sheet.column("structure"),
        sheet.column("when"),
    );
    let mut used_headers: Vec<bool> = sheet
        .headers
        .iter()
        .map(|h| META.iter().any(|m| h.trim().eq_ignore_ascii_case(m)) || h.trim().is_empty())
        .collect();
    for (i, cells) in sheet.rows.iter().enumerate() {
        let line = i + 2;
        let cell = |c: Option<usize>| {
            c.and_then(|c| cells.get(c))
                .map(|s| s.trim().to_owned())
                .unwrap_or_default()
        };
        let name = cell(Some(col_macro));
        if name.is_empty() {
            problems.push(SheetProblem {
                line,
                message: "no macro named".into(),
            });
            continue;
        }
        let when = cell(col_when);
        if !when.is_empty() {
            match Expr::parse(&when)
                .and_then(|e| e.eval(options))
                .and_then(|v| v.as_bool())
            {
                Ok(true) => {}
                Ok(false) => {
                    skipped.push(line);
                    continue;
                }
                Err(e) => {
                    problems.push(SheetProblem {
                        line,
                        message: format!("When '{when}': {e}"),
                    });
                    continue;
                }
            }
        }
        let Some(definition) = lookup(&name) else {
            problems.push(SheetProblem {
                line,
                message: format!("macro '{name}' is not in the library"),
            });
            continue;
        };
        let declared = params(&definition);
        let mut given: Vec<(String, Value)> = Vec::new();
        for (c, header) in sheet.headers.iter().enumerate() {
            let h = header.trim();
            if h.is_empty() || META.iter().any(|m| h.eq_ignore_ascii_case(m)) {
                continue;
            }
            let Some(p) = declared.iter().find(|p| p.name.eq_ignore_ascii_case(h)) else {
                continue;
            };
            used_headers[c] = true;
            let text = cell(Some(c));
            if !text.is_empty() {
                given.push((p.name.clone(), Value::Text(text)));
            }
        }
        let mut vars = options.clone();
        vars.insert("index".into(), crate::expr::Val::Num(1.0));
        let inst = match instantiate(&definition, &given, &vars) {
            Ok(i) => i,
            Err(ps) => {
                problems.extend(ps.into_iter().map(|p: Problem| SheetProblem {
                    line,
                    message: format!("{name}: {p}"),
                }));
                continue;
            }
        };
        let structure = {
            let s = cell(col_structure);
            if s.is_empty() {
                None
            } else {
                match DeviceTag::parse(&s) {
                    Ok(t) => Some(t),
                    Err(e) => {
                        problems.push(SheetProblem {
                            line,
                            message: format!("Structure '{s}': {e}"),
                        });
                        continue;
                    }
                }
            }
        };
        let (w, h) = extent(&inst.definition);
        rows.push(Row {
            line,
            macro_name: base_name(&definition).to_owned(),
            page: {
                let p = cell(col_page);
                if p.is_empty() { "Generated".into() } else { p }
            },
            structure,
            copies: inst.count,
            values: given,
            extent: (
                w + inst.pitch.x.abs() * (inst.count as f64 - 1.0),
                h + inst.pitch.y.abs() * (inst.count as f64 - 1.0),
            ),
            definition,
        });
    }
    let total: usize = rows.iter().map(|r| r.copies).sum();
    if total > MAX_PLACEMENTS {
        problems.push(SheetProblem {
            line: 1,
            message: format!(
                "the sheet asks for {total} placements, more than the limit of {MAX_PLACEMENTS}"
            ),
        });
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    let warnings = sheet
        .headers
        .iter()
        .zip(&used_headers)
        .filter(|(_, used)| !**used)
        .map(|(h, _)| format!("column '{h}' matches no parameter of the macros used"))
        .collect();
    Ok((rows, skipped, warnings))
}

/// Width and height of a definition's drawing in mm (symbols, pins, wires,
/// texts), with a padding around symbols.
fn extent(def: &BlockDefinition) -> (f64, f64) {
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    let mut at = |x: f64, y: f64, pad: f64| {
        min_x = min_x.min(x - pad);
        min_y = min_y.min(y - pad);
        max_x = max_x.max(x + pad);
        max_y = max_y.max(y + pad);
    };
    for m in &def.members {
        match m {
            Record::Symbol(s) => {
                let o = s.placement.origin;
                at(o.x, o.y, SYMBOL_PAD);
                for pin in &s.pins {
                    at(o.x + pin.offset.x, o.y + pin.offset.y, SYMBOL_PAD * 0.5);
                }
            }
            Record::Junction(j) => at(j.position.x, j.position.y, 1.0),
            Record::Wire(w) => w.route.iter().for_each(|p| at(p.x, p.y, 1.0)),
            Record::Text(t) if t.attached_to.is_none() => {
                at(t.placement.offset.x, t.placement.offset.y, 5.0)
            }
            _ => {}
        }
    }
    if min_x > max_x {
        return (SYMBOL_PAD * 2.0, SYMBOL_PAD * 2.0);
    }
    (max_x - min_x, max_y - min_y)
}

fn snap(v: f64, grid: f64) -> f64 {
    (v / grid).round() * grid
}

/// The template-coordinate corner a definition's drawing starts at.
fn corner(def: &BlockDefinition) -> Point {
    let (mut min_x, mut min_y) = (f64::MAX, f64::MAX);
    for m in &def.members {
        match m {
            Record::Symbol(s) => {
                let o = s.placement.origin;
                min_x = min_x.min(o.x - SYMBOL_PAD);
                min_y = min_y.min(o.y - SYMBOL_PAD);
                for pin in &s.pins {
                    min_x = min_x.min(o.x + pin.offset.x - SYMBOL_PAD * 0.5);
                    min_y = min_y.min(o.y + pin.offset.y - SYMBOL_PAD * 0.5);
                }
            }
            Record::Junction(j) => {
                min_x = min_x.min(j.position.x - 1.0);
                min_y = min_y.min(j.position.y - 1.0);
            }
            Record::Wire(w) => {
                for p in &w.route {
                    min_x = min_x.min(p.x - 1.0);
                    min_y = min_y.min(p.y - 1.0);
                }
            }
            Record::Text(t) if t.attached_to.is_none() => {
                min_x = min_x.min(t.placement.offset.x - 5.0);
                min_y = min_y.min(t.placement.offset.y - 5.0);
            }
            _ => {}
        }
    }
    if min_x == f64::MAX {
        Point::new(-SYMBOL_PAD, -SYMBOL_PAD)
    } else {
        Point::new(min_x, min_y)
    }
}

/// Checks the sheet and returns what a generation would do, without
/// changing the project.
pub fn preview(
    project: &Project,
    sheet: &SpecSheet,
    lookup: Lookup<'_>,
    assignment: &Assignment,
    mode: Mode,
) -> Result<Generated, Vec<SheetProblem>> {
    let mut copy = project.clone();
    let result = copy.transact("Preview generate", |tx| {
        generate(tx, sheet, lookup, assignment, mode)
    });
    match result {
        Ok((Ok(g), _)) => Ok(g),
        Ok((Err(p), _)) => Err(p),
        Err(e) => Err(vec![SheetProblem {
            line: 0,
            message: e.to_string(),
        }]),
    }
}

/// Generates the pages. Use inside one `Document::transact` for one undo
/// step. The outer error is a model error (the transaction is rolled back by
/// the caller); the inner one is the sheet's problems, in which case nothing
/// was changed.
pub fn generate(
    tx: &mut Tx<'_>,
    sheet: &SpecSheet,
    lookup: Lookup<'_>,
    assignment: &Assignment,
    mode: Mode,
) -> Result<Result<Generated, Vec<SheetProblem>>, ModelError> {
    let options = option_vars(assignment);
    let (rows, skipped, warnings) = match prepare(sheet, lookup, &options) {
        Ok(ok) => ok,
        Err(p) => return Ok(Err(p)),
    };
    let mut out = Generated {
        skipped,
        warnings,
        ..Generated::default()
    };
    if mode == Mode::Replace {
        let old: Vec<PageId> = tx
            .project()
            .pages()
            .filter(|p| p.properties.contains_key(GENERATED_KEY))
            .map(|p| p.id)
            .collect();
        out.replaced_pages = old.len();
        for page in old {
            // A placed device lives on without its symbols, so the devices
            // the page's blocks made go first (and free their tags).
            let devices: Vec<novo_core::DeviceId> = tx
                .project()
                .block_instances()
                .filter(|i| i.page == page)
                .flat_map(|i| i.members.iter())
                .filter_map(|m| match m.actual {
                    novo_core::AnyId::Device(d) => Some(d),
                    _ => None,
                })
                .collect();
            for d in devices {
                if tx.project().contains(d.into()) {
                    tx.remove(d)?;
                }
            }
            if tx.project().contains(page.into()) {
                tx.remove(page)?;
            }
        }
    }

    // Group rows by page title, in order of first appearance.
    let mut order: Vec<String> = Vec::new();
    let mut groups: BTreeMap<String, Vec<&Row>> = BTreeMap::new();
    for row in &rows {
        if !groups.contains_key(&row.page) {
            order.push(row.page.clone());
        }
        groups.entry(row.page.clone()).or_default().push(row);
    }
    for title in order {
        let rows = &groups[&title];
        let structure = rows
            .iter()
            .find_map(|r| r.structure.clone())
            .unwrap_or_default();
        let mut cursor = Flow::new(tx, &title, &structure)?;
        out.pages.push(cursor.page);
        for row in rows {
            let (w, h) = row.extent;
            if cursor.needs_room(w, h) {
                cursor = cursor.next_page(tx, &title, &structure)?;
                out.pages.push(cursor.page);
            }
            let corner = corner(&row.definition);
            let at = cursor.take(w, h);
            // On the snap grid, so the pins of a generated block line up with what the
            // designer places next (a feed row, a wire corner); the corner is only a
            // padded bounding box and sits off the grid.
            let origin = Point::new(
                snap(at.x - corner.x, novo_symbols::GRID),
                snap(at.y - corner.y, novo_symbols::GRID),
            );
            let placed = place_parametric(
                tx,
                &row.definition,
                cursor.page,
                origin,
                &ParametricOptions {
                    values: row.values.clone(),
                    options: options.clone(),
                    wire_numbers: WireNumbers::Clear,
                },
            )?;
            match placed {
                Ok(list) => out.placements.push(RowPlacement {
                    line: row.line,
                    macro_name: row.macro_name.clone(),
                    page: cursor.page,
                    reports: list.into_iter().map(|p| p.report).collect(),
                }),
                // Checked in `prepare`; a change between the two would be a bug.
                Err(ps) => {
                    return Err(ModelError::Format(format!(
                        "line {}: {}",
                        row.line,
                        ps.iter()
                            .map(|p| p.to_string())
                            .collect::<Vec<_>>()
                            .join("; ")
                    )));
                }
            }
        }
    }
    Ok(Ok(out))
}

/// Where the next macro goes on the current page.
struct Flow {
    page: PageId,
    width: f64,
    height: f64,
    x: f64,
    y: f64,
    column_width: f64,
    number: usize,
}

impl Flow {
    fn new(tx: &mut Tx<'_>, title: &str, structure: &DeviceTag) -> Result<Flow, ModelError> {
        Flow::open(tx, title, structure, 1)
    }

    fn open(
        tx: &mut Tx<'_>,
        title: &str,
        structure: &DeviceTag,
        number: usize,
    ) -> Result<Flow, ModelError> {
        let name = tx.project().next_page_name(structure);
        let page = tx.add_page(name)?;
        let shown = if number == 1 {
            title.to_owned()
        } else {
            format!("{title} ({number})")
        };
        tx.update::<novo_core::Page>(page, |p| {
            p.title = shown;
            p.structure = structure.clone();
            p.properties
                .insert(GENERATED_KEY.into(), Value::Text(title.to_owned()));
        })?;
        let format = tx
            .project()
            .page(page)
            .map(|p| p.format)
            .unwrap_or_default();
        let (width, height) = format.size_mm();
        Ok(Flow {
            page,
            width,
            height,
            x: MARGIN_LEFT,
            y: MARGIN_TOP,
            column_width: 0.0,
            number,
        })
    }

    fn next_page(
        &self,
        tx: &mut Tx<'_>,
        title: &str,
        structure: &DeviceTag,
    ) -> Result<Flow, ModelError> {
        Flow::open(tx, title, structure, self.number + 1)
    }

    /// True when a w x h block fits neither below in this column nor in a new
    /// column on this page, and the page already holds something.
    fn needs_room(&self, w: f64, h: f64) -> bool {
        let empty = self.column_width == 0.0 && self.y == MARGIN_TOP;
        if empty {
            return false;
        }
        let fits_below = self.y + h <= self.height - MARGIN_BOTTOM;
        let next_x = self.x + self.column_width + GAP;
        let fits_column = next_x + w <= self.width - MARGIN_RIGHT;
        !fits_below && !fits_column
    }

    fn take(&mut self, w: f64, h: f64) -> Point {
        let fits_below = self.y + h <= self.height - MARGIN_BOTTOM || self.column_width == 0.0;
        if !fits_below {
            self.x += self.column_width + GAP;
            self.y = MARGIN_TOP;
            self.column_width = 0.0;
        }
        let at = Point::new(self.x, self.y);
        self.y += h + GAP;
        self.column_width = self.column_width.max(w);
        at
    }
}

/// Writes the sheet back from the project: one row per placed block, in page
/// and drawing order, with the values the user asked for (calculated ones are
/// left out: they follow from the others). Generated pages carry their group
/// title, so regenerating from this sheet recreates the same page groups.
pub fn spec_from_project(project: &Project) -> SpecSheet {
    struct Line {
        page: String,
        order: (usize, novo_core::BlockInstanceId),
        macro_name: String,
        structure: String,
        values: BTreeMap<String, String>,
    }
    let mut lines: Vec<Line> = Vec::new();
    let mut columns: Vec<String> = Vec::new();
    let page_rank: BTreeMap<PageId, usize> = project
        .pages_in_order()
        .into_iter()
        .enumerate()
        .map(|(i, p)| (p.id, i))
        .collect();
    for inst in project.block_instances() {
        let Some(def) = project.block_definition(inst.definition) else {
            continue;
        };
        let page = project.page(inst.page);
        let title = page
            .map(|p| match p.properties.get(GENERATED_KEY) {
                Some(Value::Text(t)) => t.clone(),
                _ => p.title.clone(),
            })
            .unwrap_or_default();
        let structure = page.map(|p| p.structure.to_string()).unwrap_or_default();
        let asked = parametric::asked_values(def, &project.block_values(inst.id));
        let mut values = BTreeMap::new();
        for (name, value) in asked {
            // Only what differs from the default is worth a cell.
            let is_default = def
                .parameters
                .iter()
                .any(|p| p.name == name && p.default == value);
            if is_default {
                continue;
            }
            if !columns.contains(&name) {
                columns.push(name.clone());
            }
            values.insert(name, value.to_string());
        }
        lines.push(Line {
            page: title,
            order: (
                page_rank.get(&inst.page).copied().unwrap_or(usize::MAX),
                inst.id,
            ),
            macro_name: base_name(def).to_owned(),
            structure,
            values,
        });
    }
    // Page order, then the order the blocks were placed in.
    lines.sort_by_key(|l| l.order);
    columns.sort();
    let mut headers = vec!["Macro".to_string(), "Page".into(), "Structure".into()];
    headers.extend(columns.iter().cloned());
    let rows = lines
        .into_iter()
        .map(|l| {
            let mut row = vec![l.macro_name, l.page, l.structure];
            row.extend(
                columns
                    .iter()
                    .map(|c| l.values.get(c).cloned().unwrap_or_default()),
            );
            row
        })
        .collect();
    SpecSheet { headers, rows }
}

/// Which macros a sheet uses, in order of first use, for loading them.
pub fn macros_used(sheet: &SpecSheet) -> Vec<String> {
    let Some(c) = sheet.column("macro") else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for row in &sheet.rows {
        if let Some(name) = row.get(c).map(|s| s.trim()).filter(|s| !s.is_empty())
            && !out.iter().any(|o| o.eq_ignore_ascii_case(name))
        {
            out.push(name.to_owned());
        }
    }
    out
}
