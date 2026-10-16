//! Copy, cut and paste in the drawing (Edit menu, Ctrl+C, Ctrl+X, Ctrl+V,
//! Ctrl+Shift+V, Ctrl+Alt+V). The engine is `novo_workflow::clipboard`:
//! a copy is versioned JSON text on the system clipboard, so it pastes on
//! another page, in another project and in another running KoperCAD.
//!
//! A paste puts the drawing on the pointer as a ghost that snaps to the
//! grid; a click drops it, as one undo step, and selects it. Paste in place
//! drops it at the position it was copied from. The window system turns
//! Ctrl+C, Ctrl+X and Ctrl+V into copy, cut and paste events instead of key
//! presses, so those are read here and not from the keymap.

use eframe::egui;
use novo_core::{AnyId, Point, SymbolId};
use novo_editor::{EditorKey, edit};
use novo_workflow::clipboard::{Clip, Parsed, paste};
use novo_workflow::duplicate::TagMode;

use super::{Level, NovoApp};

/// What the Edit commands and the system clipboard events hold between a
/// paste starting and the click that drops it.
#[derive(Default)]
pub(crate) struct ClipState {
    /// The last copy made here, for the menu and for pasting when the
    /// system clipboard is not readable.
    pub(super) last: Option<Clip>,
    /// A paste on the pointer, waiting for its click.
    pub(super) pending: Option<(Clip, TagMode)>,
}

/// A clipboard event read from the window system.
enum ClipEvent {
    Copy,
    Cut,
    Paste(String, egui::Modifiers),
}

impl NovoApp {
    /// Names the open project, so a paste knows whether it is in the project
    /// the copy came from.
    fn clip_source(&self) -> String {
        match self.backend.file() {
            Some(path) => path.display().to_string(),
            None => format!("untitled:{}", self.project_name().unwrap_or_default()),
        }
    }

    /// Takes this frame's copy, cut and paste events when the drawing has
    /// the keyboard (a text field keeps its own).
    pub(super) fn clipboard_events(&mut self, ctx: &egui::Context) {
        if self.key_focus(ctx) != novo_workflow::keymap::Focus::Canvas
            || self.workspace() != super::workspace::Workspace::Schematic
        {
            return;
        }
        let mut found: Vec<ClipEvent> = Vec::new();
        ctx.input_mut(|input| {
            let modifiers = input.modifiers;
            input.events.retain(|event| match event {
                egui::Event::Copy => {
                    found.push(ClipEvent::Copy);
                    false
                }
                egui::Event::Cut => {
                    found.push(ClipEvent::Cut);
                    false
                }
                egui::Event::Paste(text) => {
                    found.push(ClipEvent::Paste(text.clone(), modifiers));
                    false
                }
                _ => true,
            });
        });
        for event in found {
            match event {
                ClipEvent::Copy => self.copy_selection(ctx, false),
                ClipEvent::Cut => self.copy_selection(ctx, true),
                ClipEvent::Paste(text, modifiers) => match Clip::parse(&text) {
                    Parsed::Clip(clip) => {
                        let id = if modifiers.alt {
                            "edit.paste_in_place"
                        } else if modifiers.shift {
                            "edit.paste_keep_tags"
                        } else {
                            "edit.paste"
                        };
                        self.start_paste(*clip, id);
                    }
                    Parsed::TooNew(_) => self.message(
                        Level::Info,
                        "That copy comes from a newer KoperCAD; update to paste it",
                    ),
                    Parsed::NotOurs => {
                        self.message(Level::Info, "The clipboard holds no KoperCAD drawing")
                    }
                },
            }
        }
    }

    /// `edit.copy` and `edit.cut`: the selection goes to the clipboard; a
    /// cut then deletes it, as one undo step.
    pub(super) fn copy_selection(&mut self, ctx: &egui::Context, cut: bool) {
        let selection: Vec<AnyId> = self.selection();
        let source = self.clip_source();
        let clip = self
            .backend
            .document()
            .and_then(|d| Clip::capture(d.project(), &selection, &source, cut));
        let Some(clip) = clip else {
            self.message(
                Level::Info,
                if cut {
                    "Select something to cut"
                } else {
                    "Select something to copy"
                },
            );
            return;
        };
        ctx.copy_text(clip.to_text());
        let n = clip.len();
        self.wb.clip.last = Some(clip);
        if cut {
            self.editor_key(EditorKey::Delete);
        }
        let verb = if cut { "Cut" } else { "Copied" };
        let what = if n == 1 { "object" } else { "objects" };
        self.message(Level::Info, format!("{verb} {n} {what}"));
    }

    /// `edit.paste`, `edit.paste_keep_tags` and `edit.paste_in_place` from
    /// the menu or the command field: what was last copied here.
    pub(super) fn paste_command(&mut self, id: &str) {
        match self.wb.clip.last.clone() {
            Some(clip) => self.start_paste(clip, id),
            None => self.message(Level::Info, "Copy something first"),
        }
    }

    /// Puts `clip` on the pointer, or drops it at once for paste in place.
    fn start_paste(&mut self, clip: Clip, id: &str) {
        if self.current_page.is_none() || clip.is_empty() {
            return;
        }
        let mode = if id == "edit.paste_keep_tags" || clip.cut {
            TagMode::KeepIfFree
        } else {
            TagMode::NextFree
        };
        if id == "edit.paste_in_place" {
            self.drop_paste(&clip, mode, Point::ZERO);
            return;
        }
        let Some(view) = self.canvas.schematic() else {
            return;
        };
        let grid = view.editor.grid;
        let snap = |v: f64| {
            if grid > 0.0 {
                (v / grid).round() * grid
            } else {
                v
            }
        };
        let origin = Point::new(snap(clip.origin.x), snap(clip.origin.y));
        let (symbols, paths) = clip.outline();
        view.editor.begin_paste(symbols, paths, origin);
        self.wb.clip.pending = Some((clip, mode));
        self.message(Level::Info, "Click to drop the paste · Esc cancels");
    }

    /// The click that drops a paste in hand, or Esc that cancelled it.
    pub(super) fn after_paste_click(&mut self) {
        let Some(view) = self.canvas.schematic() else {
            return;
        };
        let at = view.editor.take_paste_request();
        let pasting = view.editor.pasting();
        match (at, self.wb.clip.pending.take()) {
            (Some(at), Some((clip, mode))) => {
                let grid = self.canvas.schematic().map_or(0.0, |v| v.editor.grid);
                let snap = |v: f64| {
                    if grid > 0.0 {
                        (v / grid).round() * grid
                    } else {
                        v
                    }
                };
                let origin = Point::new(snap(clip.origin.x), snap(clip.origin.y));
                self.drop_paste(&clip, mode, at - origin);
            }
            (None, pending) if pasting => self.wb.clip.pending = pending,
            _ => {}
        }
    }

    /// Pastes `clip` on the shown page moved by `offset`: one undo step,
    /// joined to what it touches, and selected.
    fn drop_paste(&mut self, clip: &Clip, mode: TagMode, offset: Point) {
        let Some(page) = self.current_page else {
            return;
        };
        let source = self.clip_source();
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let result = doc.transact("Paste", |tx| {
            let pasted = paste(tx, clip, page, offset, mode, &source)?;
            let symbols: Vec<SymbolId> = pasted.copied.symbols.values().copied().collect();
            edit::settle(tx, &symbols)?;
            Ok(pasted)
        });
        match result {
            Ok((pasted, changes)) => {
                self.changed_outside_canvas(&changes);
                if let Some(view) = self.canvas.schematic() {
                    view.select_ids(&pasted.new_ids());
                }
                let n = pasted.new_ids().len();
                let mut text = format!("Pasted {n} {}", if n == 1 { "object" } else { "objects" });
                let tags: Vec<String> = pasted
                    .renamed
                    .iter()
                    .map(|(_, new)| new.to_string())
                    .take(4)
                    .collect();
                if !tags.is_empty() {
                    text += &format!(" · new tags {}", tags.join(", "));
                    if pasted.renamed.len() > tags.len() {
                        text += ", …";
                    }
                }
                if !pasted.orphans.is_empty() {
                    let list: Vec<String> = pasted.orphans.iter().map(|t| t.to_string()).collect();
                    text += &format!(" · {} has no coil here", list.join(", "));
                }
                self.message(Level::Info, text);
            }
            Err(e) => self.message(Level::Error, format!("Could not paste: {e}")),
        }
    }
}
