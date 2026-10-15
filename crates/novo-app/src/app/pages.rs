//! The pages navigator's creation form: new pages, page groups and panels
//! (a page group is the function level of a page's structure, `=10D41`; a
//! panel is its location level, `+K1B1G1`). Pages with the same panel name
//! under different page groups share one panel, hence one cabinet layout.

use eframe::egui;
use novo_core::{Command, DeviceTag, PageId, Project};

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
            PageView::Function => "Function",
            PageView::Location => "Location",
        }
    }

    pub fn hover(self) -> &'static str {
        match self {
            PageView::Function => "Pages by page group",
            PageView::Location => "Pages by panel, across page groups",
        }
    }
}

/// What the form was opened for; only the heading and the first focus differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FormKind {
    Pages,
    Group,
    Panel,
    Move(PageId),
}

/// The creation form's fields.
#[derive(Debug, Clone)]
pub(super) struct PageForm {
    pub kind: FormKind,
    pub group: String,
    /// Panel names separated by commas.
    pub panels: String,
    /// Pages to add to each panel.
    pub count: u32,
    pub title: String,
}

impl PageForm {
    pub fn new(kind: FormKind, group: &str, panels: &str) -> Self {
        Self {
            kind,
            group: group.trim_start_matches('=').into(),
            panels: panels.trim_start_matches('+').into(),
            count: match kind {
                FormKind::Pages | FormKind::Panel => 1,
                FormKind::Group | FormKind::Move(_) => 0,
            },
            title: String::new(),
        }
    }

    fn heading(&self) -> &'static str {
        match self.kind {
            FormKind::Pages => "New pages",
            FormKind::Group => "New page group",
            FormKind::Panel => "New panel",
            FormKind::Move(_) => "Move page",
        }
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
    if let FormKind::Move(page) = form.kind {
        if panels.len() > 1 {
            return Err("A page goes to one panel".into());
        }
        commands.push(Command::SetPageStructure {
            page,
            structure: structure_of(panels.first()),
        });
        return Ok(Command::Batch {
            label: "Move page".into(),
            commands,
        });
    }
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
        label: form.heading().into(),
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
            ui.heading(form.heading());
            ui.add_space(4.0);
            egui::Grid::new("page_form_grid").num_columns(2).show(ui, |ui| {
                ui.label("Page group");
                let group = ui.add(
                    egui::TextEdit::singleline(&mut form.group)
                        .hint_text("10D41")
                        .desired_width(200.0),
                );
                if matches!(form.kind, FormKind::Group | FormKind::Pages) {
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
                let single = matches!(form.kind, FormKind::Move(_));
                ui.label(if single { "Panel" } else { "Panels" });
                let panel = ui
                    .add(
                        egui::TextEdit::singleline(&mut form.panels)
                            .hint_text(if single { "K1B1G1" } else { "K1B1G1, K2" })
                            .desired_width(200.0),
                    )
                    .on_hover_text("Pages with the same panel name in different page groups share one cabinet layout");
                if matches!(form.kind, FormKind::Panel | FormKind::Move(_)) {
                    panel.request_focus();
                }
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
                if !matches!(form.kind, FormKind::Move(_)) {
                    ui.label("Pages per panel");
                    ui.add(egui::DragValue::new(&mut form.count).range(0..=99));
                    ui.end_row();
                    ui.label("Title");
                    ui.add(egui::TextEdit::singleline(&mut form.title).desired_width(200.0));
                    ui.end_row();
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let ok = ui.button("Create (Enter)").clicked();
                let cancel = ui.button("Cancel (Esc)").clicked();
                (ok, cancel)
            })
            .inner
        });
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
            let mut form = PageForm::new(FormKind::Pages, group, "K1B1G1");
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
        let mut more = PageForm::new(FormKind::Pages, "10D41", "K1B1G1");
        more.count = 1;
        run(&mut doc, &more).unwrap();
        assert!(doc.project().pages().any(|p| p.name == "3"));
        doc.undo().unwrap();
        assert_eq!(doc.project().pages().count(), 4);
    }

    #[test]
    fn groups_alone_and_bad_input() {
        let mut doc = Document::default();
        run(&mut doc, &PageForm::new(FormKind::Group, "12A", "")).unwrap();
        assert_eq!(doc.project().page_groups().len(), 1);
        assert_eq!(doc.project().pages().count(), 0);
        assert!(form_command(doc.project(), &PageForm::new(FormKind::Pages, "", "")).is_err());
        assert!(form_command(doc.project(), &PageForm::new(FormKind::Pages, "A/B", "")).is_err());
        assert!(form_command(doc.project(), &PageForm::new(FormKind::Group, "12A", "")).is_err());
    }
}
