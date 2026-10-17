//! The pages navigator's creation form: new pages, page groups and panels
//! (a page group is the function level of a page's structure, `=10D41`; a
//! panel is its location level, `+K1B1G1`). Pages with the same panel name
//! under different page groups share one panel, hence one cabinet layout.

use eframe::egui;
use novo_core::{Aspect, Command, DeviceTag, PageId, PageKind, Project, panel_contents_generator};

use super::{Level, NovoApp};

/// How the outline files the pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PageView {
    /// By page group (`=10D41`), then panel.
    Function,
    /// By panel (`+K1B1G1`), then group: a panel shows the pages of every
    /// group that uses it.
    Location,
}

impl PageView {
    pub fn label(self) -> &'static str {
        match self {
            PageView::Function => "Groups",
            PageView::Location => "Cabinets",
        }
    }

    pub fn hover(self) -> &'static str {
        match self {
            PageView::Function => "Page groups, with their cabinets inside",
            PageView::Location => "Cabinets, with their page groups inside",
        }
    }
}

/// The creation form's fields.
#[derive(Debug, Clone)]
pub(super) struct PageForm {
    pub group: String,
    /// Panel names separated by commas.
    pub panels: String,
    /// Pages to add to each panel.
    pub count: u32,
    pub title: String,
    /// The first field has taken focus (once, when the form opens).
    pub focused: bool,
}

impl PageForm {
    pub fn new(group: &str, panels: &str) -> Self {
        Self {
            group: group.trim_start_matches('=').into(),
            panels: panels.trim_start_matches('+').into(),
            count: 1,
            title: String::new(),
            focused: false,
        }
    }
}

/// The tidy form of a group (`=G1`) or cabinet (`+A1`) name typed by the
/// person, or an empty string when it is empty or not a plain name.
pub(super) fn clean_level(text: &str, aspect: Aspect) -> String {
    let prefix = if aspect == Aspect::Function { '=' } else { '+' };
    match level(text, prefix) {
        Ok(Some(tag)) => tag.to_string(),
        _ => String::new(),
    }
}

fn level(text: &str, prefix: char) -> Result<Option<DeviceTag>, String> {
    let text = text.trim().trim_start_matches(prefix).trim();
    if text.is_empty() {
        return Ok(None);
    }
    let tag = DeviceTag::parse(&format!("{prefix}{text}")).map_err(|e| e.to_string())?;
    if tag.product.is_empty() && (prefix == '=' && tag.location.is_empty() || prefix == '+') {
        Ok(Some(tag))
    } else {
        Err(format!("`{text}` is not a plain {prefix} level"))
    }
}

/// The command that renames a group (`aspect` Function) or panel (Location)
/// to `text`, which may carry its `=` or `+`. `None` when the name did not
/// change.
pub(super) fn rename_level_command(
    aspect: Aspect,
    from: &DeviceTag,
    text: &str,
) -> Result<Option<Command>, String> {
    let prefix = if aspect == Aspect::Function { '=' } else { '+' };
    let Some(to) = level(text, prefix)? else {
        return Err(format!("Name the {}", level_noun(aspect)));
    };
    let (from, to) = (from.levels(aspect).to_vec(), to.levels(aspect).to_vec());
    if from == to {
        return Ok(None);
    }
    Ok(Some(Command::RenameLevel { aspect, from, to }))
}

fn level_noun(aspect: Aspect) -> &'static str {
    if aspect == Aspect::Function {
        "page group"
    } else {
        "panel"
    }
}

/// Whether renaming `from` to the text `to` joins a group or panel that
/// already exists, so the report can say so.
pub(super) fn merges_with_existing(
    project: &Project,
    aspect: Aspect,
    from: &DeviceTag,
    to: &str,
) -> bool {
    let prefix = if aspect == Aspect::Function { '=' } else { '+' };
    let Ok(Some(to)) = level(to, prefix) else {
        return false;
    };
    let (from, to) = (from.levels(aspect), to.levels(aspect));
    let taken = |t: &DeviceTag| {
        let levels = t.levels(aspect);
        levels.starts_with(to) && !levels.starts_with(from)
    };
    project.pages().any(|p| taken(&p.structure))
        || project.structures().any(|s| taken(&s.tag))
        || project.devices().any(|d| taken(&d.tag))
        || project.cables().any(|c| taken(&c.tag))
}

/// The order of `siblings` after `dragged` is dropped on the upper or lower
/// half of `target`. `None` when nothing would move: the drop is on the page
/// itself, either page is not among the siblings, or the order stays.
pub(super) fn order_after_drop(
    siblings: &[PageId],
    dragged: PageId,
    target: PageId,
    after: bool,
) -> Option<Vec<PageId>> {
    if dragged == target || !siblings.contains(&dragged) || !siblings.contains(&target) {
        return None;
    }
    let mut order: Vec<PageId> = siblings.iter().copied().filter(|p| *p != dragged).collect();
    let at = order.iter().position(|p| *p == target)? + usize::from(after);
    order.insert(at, dragged);
    (order != siblings).then_some(order)
}

/// The order after moving `page` one place up or down among `siblings`;
/// `None` at either end.
pub(super) fn order_after_step(
    siblings: &[PageId],
    page: PageId,
    down: bool,
) -> Option<Vec<PageId>> {
    let at = siblings.iter().position(|p| *p == page)?;
    let neighbour = if down {
        siblings.get(at + 1)
    } else {
        at.checked_sub(1).and_then(|n| siblings.get(n))
    }?;
    order_after_drop(siblings, page, *neighbour, down)
}

/// Whether `panel` already has its own contents page.
pub(super) fn has_panel_contents(project: &Project, panel: &DeviceTag) -> bool {
    let id = panel_contents_generator(&panel.location);
    project
        .pages()
        .any(|p| matches!(&p.kind, PageKind::Generated { generator } if *generator == id))
}

/// The command that adds the contents page of `panel`: a generated page in
/// the panel (no page group) that lists the panel's pages. One per panel.
pub(super) fn panel_contents_command(
    project: &Project,
    panel: &DeviceTag,
) -> Result<Command, String> {
    if panel.location.is_empty() {
        return Err("Name a panel".into());
    }
    if has_panel_contents(project, panel) {
        return Err(format!("{panel} already has a contents page"));
    }
    let structure = DeviceTag {
        location: panel.location.clone(),
        ..Default::default()
    };
    Ok(Command::AddPage {
        name: "0".into(),
        title: format!("Contents {structure}"),
        kind: PageKind::Generated {
            generator: panel_contents_generator(&structure.location),
        },
        format: Default::default(),
        structure,
    })
}

/// What a rename of `from` touches, for the line that reports it:
/// "6 pages, 41 devices, 3 cables".
pub(super) fn level_users(project: &Project, aspect: Aspect, from: &DeviceTag) -> String {
    let levels = from.levels(aspect);
    let uses = |t: &DeviceTag| t.levels(aspect).starts_with(levels);
    let count = |n: usize, noun: &str| format!("{n} {noun}{}", if n == 1 { "" } else { "s" });
    format!(
        "{}, {}, {}",
        count(
            project.pages().filter(|p| uses(&p.structure)).count(),
            "page"
        ),
        count(project.devices().filter(|d| uses(&d.tag)).count(), "device"),
        count(project.cables().filter(|c| uses(&c.tag)).count(), "cable"),
    )
}

/// The one undoable command a form makes: the group and panel nodes that do
/// not exist yet (panels are enclosures), then the new pages numbered on from
/// the highest in each group and panel, or the page move.
pub(super) fn form_command(project: &Project, form: &PageForm) -> Result<Command, String> {
    let group = level(&form.group, '=')?;
    let mut panels = Vec::new();
    for name in form.panels.split([',', ';']) {
        if let Some(panel) = level(name, '+')? {
            panels.push(panel);
        }
    }
    let mut commands = Vec::new();
    let mut node = |tag: &DeviceTag, enclosure: bool| {
        if project.structure_by_tag(tag).is_none()
            && !commands
                .iter()
                .any(|c| matches!(c, Command::AddStructure { tag: t, .. } if t == tag))
        {
            commands.push(Command::AddStructure {
                tag: tag.clone(),
                description: String::new(),
                enclosure,
            });
        }
    };
    if let Some(group) = &group {
        node(group, false);
    }
    for panel in &panels {
        node(panel, true);
    }
    let structure_of = |panel: Option<&DeviceTag>| DeviceTag {
        function: group
            .as_ref()
            .map(|g| g.function.clone())
            .unwrap_or_default(),
        location: panel.map(|p| p.location.clone()).unwrap_or_default(),
        product: Vec::new(),
    };
    if group.is_none() && panels.is_empty() {
        return Err("Name a page group or a panel".into());
    }
    if form.count > 0 {
        let targets: Vec<Option<&DeviceTag>> = if panels.is_empty() {
            vec![None]
        } else {
            panels.iter().map(Some).collect()
        };
        for panel in targets {
            let structure = structure_of(panel);
            let first = project
                .next_page_name(&structure)
                .parse::<u32>()
                .unwrap_or(1);
            for n in first..first + form.count {
                commands.push(Command::AddPage {
                    name: n.to_string(),
                    title: form.title.trim().into(),
                    kind: Default::default(),
                    format: Default::default(),
                    structure: structure.clone(),
                });
            }
        }
    }
    if commands.is_empty() {
        return Err("Nothing new to add".into());
    }
    Ok(Command::Batch {
        label: "New pages".into(),
        commands,
    })
}

impl NovoApp {
    pub(super) fn open_page_form(&mut self, form: PageForm) {
        self.wb.page_form = Some(form);
    }

    /// The form as a modal; Enter creates, Esc cancels.
    pub(super) fn page_form_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut form) = self.wb.page_form.take() else {
            return;
        };
        let (groups, panels) = self
            .backend
            .document()
            .map(|d| {
                let p = d.project();
                (
                    p.page_groups()
                        .iter()
                        .map(|g| g.to_string())
                        .collect::<Vec<_>>(),
                    p.panels()
                        .iter()
                        .map(|g| g.location.to_string())
                        .collect::<Vec<_>>(),
                )
            })
            .unwrap_or_default();
        let modal = egui::Modal::new(egui::Id::new("page_form")).show(ctx, |ui| {
            ui.set_width(360.0);
            super::dialog_ui::title(ui, "New pages", "");
            ui.add_space(4.0);
            egui::Grid::new("page_form_grid").num_columns(2).show(ui, |ui| {
                ui.label("Page group");
                let group = ui.add(
                    egui::TextEdit::singleline(&mut form.group)
                        .hint_text("10D41")
                        .desired_width(200.0),
                );
                if !form.focused {
                    group.request_focus();
                }
                ui.end_row();
                if !groups.is_empty() {
                    ui.label("");
                    ui.horizontal_wrapped(|ui| {
                        for g in &groups {
                            if ui.small_button(g).clicked() {
                                form.group = g.trim_start_matches('=').into();
                            }
                        }
                    });
                    ui.end_row();
                }
                ui.label("Panels");
                ui
                    .add(
                        egui::TextEdit::singleline(&mut form.panels)
                            .hint_text("K1B1G1, K2")
                            .desired_width(200.0),
                    )
                    .on_hover_text("Pages with the same panel name in different page groups share one cabinet layout");
                ui.end_row();
                if !panels.is_empty() {
                    ui.label("");
                    ui.horizontal_wrapped(|ui| {
                        for p in &panels {
                            if ui.small_button(p).clicked() {
                                form.panels = p.trim_start_matches('+').into();
                            }
                        }
                    });
                    ui.end_row();
                }
                ui.label("Pages per panel");
                ui.add(egui::DragValue::new(&mut form.count).range(0..=99));
                ui.end_row();
                ui.label("Title");
                ui.add(egui::TextEdit::singleline(&mut form.title).desired_width(200.0));
                ui.end_row();
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let ok = novo_theme::widgets::primary_button(ui, "Create (Enter)").clicked();
                let cancel = novo_theme::widgets::ghost_button(ui, "Cancel (Esc)").clicked();
                (ok, cancel)
            })
            .inner
        });
        form.focused = true;
        let (ok, cancel) = modal.inner;
        let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        if cancel || modal.should_close() {
            return;
        }
        if ok || enter {
            let command = self
                .backend
                .document()
                .map(|d| form_command(d.project(), &form));
            match command {
                Some(Ok(command)) => match self.execute(command) {
                    Some(Ok(_)) | None => return,
                    Some(Err(e)) => self.message(Level::Error, format!("Could not add: {e}")),
                },
                Some(Err(e)) => self.message(Level::Warning, e),
                None => return,
            }
        }
        self.wb.page_form = Some(form);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::Document;

    fn run(doc: &mut Document, form: &PageForm) -> Result<(), String> {
        let command = form_command(doc.project(), form)?;
        doc.execute(command).map(|_| ()).map_err(|e| e.to_string())
    }

    #[test]
    fn two_pages_in_a_panel_under_two_groups_make_one_panel() {
        let mut doc = Document::default();
        for group in ["10D41", "=11D21"] {
            let mut form = PageForm::new(group, "K1B1G1");
            form.count = 2;
            run(&mut doc, &form).unwrap();
        }
        let panels = doc.project().panels();
        assert_eq!(panels.len(), 1);
        assert_eq!(panels[0].pages.len(), 4);
        assert_eq!(panels[0].groups.len(), 2);
        // The panel node is created once, as an enclosure.
        let nodes: Vec<_> = doc.project().structures().collect();
        assert_eq!(nodes.iter().filter(|s| s.enclosure).count(), 1);
        // More pages carry on numbering.
        let mut more = PageForm::new("10D41", "K1B1G1");
        more.count = 1;
        run(&mut doc, &more).unwrap();
        assert!(doc.project().pages().any(|p| p.name == "3"));
        doc.undo().unwrap();
        assert_eq!(doc.project().pages().count(), 4);
    }

    #[test]
    fn a_panel_renames_from_the_outline_text() {
        let mut doc = Document::default();
        let mut form = PageForm::new("10D41", "K1B1G1");
        form.count = 2;
        run(&mut doc, &form).unwrap();
        let panel = DeviceTag::parse("+K1B1G1").unwrap();

        let same = rename_level_command(Aspect::Location, &panel, "+K1B1G1").unwrap();
        assert!(same.is_none());
        assert!(rename_level_command(Aspect::Location, &panel, "  ").is_err());
        assert!(rename_level_command(Aspect::Location, &panel, "a/b").is_err());
        assert_eq!(
            level_users(doc.project(), Aspect::Location, &panel),
            "2 pages, 0 devices, 0 cables"
        );

        let command = rename_level_command(Aspect::Location, &panel, "K2").unwrap();
        doc.execute(command.unwrap()).unwrap();
        let panels = doc.project().panels();
        assert_eq!(panels.len(), 1);
        assert_eq!(panels[0].location.to_string(), "+K2");
        assert_eq!(panels[0].pages.len(), 2);
        doc.undo().unwrap();
        assert_eq!(doc.project().panels()[0].location.to_string(), "+K1B1G1");
    }

    #[test]
    fn a_drop_or_step_moves_one_page_and_nothing_else() {
        let mut doc = Document::default();
        let mut form = PageForm::new("10D41", "K1B1G1");
        form.count = 4;
        run(&mut doc, &form).unwrap();
        let first = doc.project().pages_in_order()[0].id;
        let ids = doc.project().sibling_pages(first);
        let (a, b, c, d) = (ids[0], ids[1], ids[2], ids[3]);

        assert_eq!(order_after_drop(&ids, d, a, false), Some(vec![d, a, b, c]));
        assert_eq!(order_after_drop(&ids, a, c, true), Some(vec![b, c, a, d]));
        // Dropping next to where it already is, or on itself, moves nothing.
        assert_eq!(order_after_drop(&ids, b, a, true), None);
        assert_eq!(order_after_drop(&ids, b, b, true), None);
        // A page of another group and panel is not a sibling.
        run(&mut doc, &PageForm::new("11D21", "K9")).unwrap();
        let other = doc.project().pages_in_order().last().unwrap().id;
        assert_eq!(order_after_drop(&ids, b, other, true), None);
        assert_eq!(order_after_step(&ids, a, false), None);
        assert_eq!(order_after_step(&ids, d, true), None);
        assert_eq!(order_after_step(&ids, b, true), Some(vec![a, c, b, d]));

        let order = order_after_drop(&ids, d, a, false).unwrap();
        doc.execute(Command::ReorderPages { pages: order }).unwrap();
        assert_eq!(doc.project().sibling_pages(a), [d, a, b, c]);
        doc.undo().unwrap();
        assert_eq!(doc.project().sibling_pages(a), ids);
    }

    #[test]
    fn a_rename_onto_a_name_in_use_is_a_merge() {
        let mut doc = Document::default();
        for panel in ["K1", "K2"] {
            run(&mut doc, &PageForm::new("10D41", panel)).unwrap();
        }
        let k1 = DeviceTag::parse("+K1").unwrap();
        assert!(merges_with_existing(
            doc.project(),
            Aspect::Location,
            &k1,
            "+K2"
        ));
        assert!(!merges_with_existing(
            doc.project(),
            Aspect::Location,
            &k1,
            "K3"
        ));
        assert!(!merges_with_existing(
            doc.project(),
            Aspect::Location,
            &k1,
            "bad/name"
        ));
    }

    #[test]
    fn a_panel_gets_one_contents_page_that_lists_its_pages() {
        let mut doc = Document::default();
        let mut form = PageForm::new("10D41", "K1B1G1");
        form.count = 2;
        run(&mut doc, &form).unwrap();
        let panel = DeviceTag::parse("+K1B1G1").unwrap();
        assert!(!has_panel_contents(doc.project(), &panel));
        let command = panel_contents_command(doc.project(), &panel).unwrap();
        doc.execute(command).unwrap();
        assert!(has_panel_contents(doc.project(), &panel));
        assert!(panel_contents_command(doc.project(), &panel).is_err());
        assert!(panel_contents_command(doc.project(), &DeviceTag::default()).is_err());
        // It reads first, ahead of the drawn pages, shows in the outline with
        // the panel's pages in it, and follows a rename.
        let first = doc.project().pages_in_order()[0];
        assert_eq!(first.title, "Contents +K1B1G1");
        let mut reports = novo_reports::LiveReports::standard();
        let rows = crate::backend::page_rows(doc.project(), &mut reports);
        assert!(rows.iter().any(|r| r.page == first.id), "{rows:?}");
        assert_eq!(
            reports.get(doc.project(), "toc@+K1B1G1").unwrap().sections[0]
                .rows
                .len(),
            3
        );
        let rename = rename_level_command(Aspect::Location, &panel, "K2")
            .unwrap()
            .unwrap();
        doc.execute(rename).unwrap();
        let moved = DeviceTag::parse("+K2").unwrap();
        assert!(has_panel_contents(doc.project(), &moved));
        assert!(!has_panel_contents(doc.project(), &panel));
    }

    #[test]
    fn bad_input_is_refused() {
        let doc = Document::default();
        assert!(form_command(doc.project(), &PageForm::new("", "")).is_err());
        assert!(form_command(doc.project(), &PageForm::new("A/B", "")).is_err());
        // A group alone, with no pages, is just the group.
        let mut group = PageForm::new("12A", "");
        group.count = 0;
        let mut doc = doc;
        run(&mut doc, &group).unwrap();
        assert_eq!(doc.project().page_groups().len(), 1);
        assert_eq!(doc.project().pages().count(), 0);
        assert!(
            form_command(doc.project(), &group).is_err(),
            "nothing new the second time"
        );
    }
}
