//! A panel as data (`*.panel.toml`): the add-on says what is on it, the program draws it with
//! its own widgets in the theme's own colours (`docs/design/visual-redesign-addons.md`
//! section 5). An add-on has no way to paint a pixel, set a colour or open a window.
//!
//! This is the base widget set of add-on API 1. Lane C3 extends it (rocker with steps, joystick
//! with detents, tabs, tables) by adding variants here; unknown kinds are refused at install,
//! so an older program never shows half a panel.
//!
//! ```toml
//! [[widget]]
//! kind = "heading"
//! text = "Radio"
//!
//! [[widget]]
//! kind = "row"
//! widgets = [
//!   { kind = "button", id = "start", text = "Start" },
//!   { kind = "lamp", id = "main", text = "Main", state = "ok" },
//! ]
//! ```

use serde::{Deserialize, Serialize};

use crate::error::{AddonError, Result};
use crate::manifest::valid_id;

/// Most widgets in one panel, nested ones included.
pub const MAX_WIDGETS: usize = 200;
/// Deepest nesting of `row`.
pub const MAX_DEPTH: usize = 2;
/// Longest text on a widget.
pub const MAX_TEXT: usize = 200;
/// Largest layout file.
pub const MAX_BYTES: usize = 128 * 1024;

/// A meaning, not a colour: the program resolves it with its palette, so dark, light and High
/// contrast work without the add-on knowing. A lamp always carries text as well.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    #[default]
    Neutral,
    Accent,
    Ok,
    Warn,
    Error,
    Info,
}

/// One thing on a panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum Widget {
    Heading {
        text: String,
    },
    Label {
        text: String,
        /// Drawn in the weaker text colour.
        #[serde(default)]
        weak: bool,
    },
    Separator,
    /// A button. Pressing it sends `id` to the add-on's engine (C0b-2); with no engine it does
    /// nothing, and the program says so in the tooltip.
    Button {
        id: String,
        text: String,
        #[serde(default)]
        hint: String,
    },
    /// An indicator: a filled or hollow mark, its text, never colour alone.
    Lamp {
        id: String,
        text: String,
        #[serde(default)]
        state: Role,
        #[serde(default)]
        lit: bool,
    },
    /// Widgets side by side.
    Row {
        widgets: Vec<Widget>,
    },
}

/// A parsed, validated panel layout.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PanelLayout {
    #[serde(default, rename = "widget")]
    pub widgets: Vec<Widget>,
}

impl PanelLayout {
    pub fn parse(bytes: &[u8]) -> Result<PanelLayout> {
        let bad = |m: String| Err(AddonError::Manifest(format!("panel layout: {m}")));
        if bytes.len() > MAX_BYTES {
            return bad("larger than 128 KiB".into());
        }
        let Ok(text) = std::str::from_utf8(bytes) else {
            return bad("not UTF-8 text".into());
        };
        let layout: PanelLayout =
            toml::from_str(text).map_err(|e| AddonError::Manifest(format!("panel layout: {e}")))?;
        let mut count = 0;
        check(&layout.widgets, 0, &mut count)?;
        Ok(layout)
    }

    /// Every id of a button or lamp, in order.
    pub fn ids(&self) -> Vec<&str> {
        fn walk<'a>(ws: &'a [Widget], out: &mut Vec<&'a str>) {
            for w in ws {
                match w {
                    Widget::Button { id, .. } | Widget::Lamp { id, .. } => out.push(id),
                    Widget::Row { widgets } => walk(widgets, out),
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.widgets, &mut out);
        out
    }
}

fn check(ws: &[Widget], depth: usize, count: &mut usize) -> Result<()> {
    let bad = |m: String| Err(AddonError::Manifest(format!("panel layout: {m}")));
    for w in ws {
        *count += 1;
        if *count > MAX_WIDGETS {
            return bad(format!("more than {MAX_WIDGETS} widgets"));
        }
        let text_ok = |t: &str| {
            !t.is_empty()
                && t.chars().count() <= MAX_TEXT
                && !t.chars().any(|c| c.is_control() && c != '\n')
        };
        match w {
            Widget::Heading { text } | Widget::Label { text, .. } => {
                if !text_ok(text) {
                    return bad("a text is empty, too long or has control characters".into());
                }
            }
            Widget::Separator => {}
            Widget::Button { id, text, hint } => {
                if !valid_id(id, 1, 32) || !text_ok(text) || hint.chars().count() > MAX_TEXT {
                    return bad(format!("button {id:?} has a bad id, text or hint"));
                }
            }
            Widget::Lamp { id, text, .. } => {
                if !valid_id(id, 1, 32) || !text_ok(text) {
                    return bad(format!("lamp {id:?} has a bad id or text"));
                }
            }
            Widget::Row { widgets } => {
                if depth + 1 > MAX_DEPTH {
                    return bad("rows nest deeper than 2 levels".into());
                }
                if widgets.is_empty() {
                    return bad("a row is empty".into());
                }
                check(widgets, depth + 1, count)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_lists_ids() {
        let l = PanelLayout::parse(
            br#"
[[widget]]
kind = "heading"
text = "Radio"
[[widget]]
kind = "row"
widgets = [
  { kind = "button", id = "start", text = "Start" },
  { kind = "lamp", id = "main", text = "Main", state = "ok", lit = true },
]
[[widget]]
kind = "separator"
"#,
        )
        .unwrap();
        assert_eq!(l.widgets.len(), 3);
        assert_eq!(l.ids(), vec!["start", "main"]);
    }

    #[test]
    fn hostile_layouts_are_refused() {
        for (why, text) in [
            (
                "unknown kind",
                "[[widget]]\nkind = \"webview\"\nurl = \"http://x\"\n",
            ),
            (
                "colour",
                "[[widget]]\nkind = \"label\"\ntext = \"a\"\ncolor = \"#f00\"\n",
            ),
            ("empty text", "[[widget]]\nkind = \"label\"\ntext = \"\"\n"),
            (
                "bad id",
                "[[widget]]\nkind = \"button\"\nid = \"../x\"\ntext = \"a\"\n",
            ),
            ("empty row", "[[widget]]\nkind = \"row\"\nwidgets = []\n"),
            (
                "deep",
                "[[widget]]\nkind = \"row\"\nwidgets = [{ kind = \"row\", widgets = [{ kind = \"row\", widgets = [{ kind = \"separator\" }] }] }]\n",
            ),
            (
                "role",
                "[[widget]]\nkind = \"lamp\"\nid = \"a\"\ntext = \"a\"\nstate = \"pink\"\n",
            ),
        ] {
            assert!(PanelLayout::parse(text.as_bytes()).is_err(), "{why}");
        }
        let many = "[[widget]]\nkind = \"separator\"\n".repeat(MAX_WIDGETS + 1);
        assert!(
            PanelLayout::parse(many.as_bytes())
                .unwrap_err()
                .to_string()
                .contains("200")
        );
    }
}
