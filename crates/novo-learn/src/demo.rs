//! The "Show me how" demonstration of a step (`demo` in the lesson file, see
//! docs/design/learn-layout.md section 4): what a person would do, as a
//! small timeline the application plays over the real screen. A moving
//! pointer, key caps, typed text, a click ripple, a ghost of the part that
//! lands and a wire being dragged.
//!
//! The demonstration never changes the drawing; "Do it for me" is separate
//! (the step's `solve`). A step either writes its own demonstration with
//! `demo = [...]`, or gets one derived from what it already says: the command
//! that ticks it, its `solve` script and its pointer.
//!
//! Pure: [`Demo::frame`] says what is on show at a time; the application
//! turns targets into screen positions and draws.

use serde::Deserialize;

use crate::check::Check;
use crate::lesson::{At, Lang, PlaceArgs, SolveOp, Step, Target};

/// One thing the demonstration does. A step lists these in order.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum DemoOp {
    /// `{ press = "tool.place" }`: the key (or chord) of a command, shown
    /// as the person's own key.
    Press(PressOp),
    /// `{ key = "Enter" }`: a key by its name.
    Key(KeyOp),
    /// `{ type = "mcb" }`: text typed letter by letter.
    Type(TypeOp),
    /// `{ hold = "tool.place", secs = 1.2 }`: a key held down.
    Hold(HoldOp),
    /// `{ move = { ui = "dock.place" } }`: the pointer glides to a target.
    Move(MoveOp),
    /// `{ click = true }`: a click where the pointer is.
    Click(ClickOp),
    /// `{ double_click = true }`.
    DoubleClick(DoubleClickOp),
    /// `{ right_click = true }`.
    RightClick(RightClickOp),
    /// `{ keys = ["Ctrl", "K"] }`: a chord of keys by name.
    Keys(KeysOp),
    /// `{ drag = { from = {...}, to = {...} } }`: press, drag, release.
    Drag(Box<DragOp>),
    /// `{ ghost = { symbol = ..., page = ..., x = ..., y = ... } }`: a
    /// faint copy of the part that lands, left on the page.
    Ghost(GhostOp),
    /// `{ say = "Words", say_nl = "Woorden" }`: a line of the how-to list
    /// that is not one of the above.
    Say(SayOp),
    /// `{ wait = 0.5 }`: a pause in seconds.
    Wait(WaitOp),
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PressOp {
    pub press: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyOp {
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypeOp {
    #[serde(rename = "type")]
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HoldOp {
    pub hold: String,
    #[serde(default = "default_hold")]
    pub secs: f32,
}

fn default_hold() -> f32 {
    1.2
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoveOp {
    #[serde(rename = "move")]
    pub to: Target,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClickOp {
    pub click: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DoubleClickOp {
    pub double_click: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RightClickOp {
    pub right_click: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeysOp {
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DragOp {
    pub drag: DragArgs,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DragArgs {
    pub from: Target,
    pub to: Target,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GhostOp {
    pub ghost: PlaceArgs,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SayOp {
    pub say: String,
    #[serde(default)]
    pub say_nl: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaitOp {
    pub wait: f32,
}

/// Seconds the pointer takes to glide to a target.
const MOVE_SECS: f32 = 0.9;
/// Seconds a key cap stays after its action.
const LINGER_SECS: f32 = 0.6;
/// Seconds a click ripple takes.
const CLICK_SECS: f32 = 0.55;
/// Seconds for each typed letter.
const LETTER_SECS: f32 = 0.14;
/// Seconds a key press takes.
const PRESS_SECS: f32 = 0.8;
/// Seconds a drag takes.
const DRAG_SECS: f32 = 1.3;
/// Seconds a ghost takes to fade in.
const GHOST_SECS: f32 = 0.6;
/// Seconds the finished picture stays before the demonstration repeats.
pub const END_HOLD_SECS: f32 = 1.6;

/// A demonstration: the operations and when each one runs.
#[derive(Debug, Clone, PartialEq)]
pub struct Demo {
    ops: Vec<DemoOp>,
    /// Start second of each operation.
    starts: Vec<f32>,
    /// Seconds each operation takes.
    lengths: Vec<f32>,
}

/// A key cap on show.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyCap {
    /// A command whose key the person's keymap names.
    pub command: Option<String>,
    /// Or a key by its name.
    pub literal: Option<String>,
    /// Pressed in right now.
    pub down: bool,
}

/// Where the pointer is: gliding from `from` to `to`, `k` of the way
/// (eased), or at `to` when `k` is 1. `from` is `None` before the first
/// move; the application starts the pointer at the card.
#[derive(Debug, Clone, PartialEq)]
pub struct Cursor {
    pub from: Option<Target>,
    pub to: Target,
    pub k: f32,
}

/// A numbered marker on the start or the end of a drag: where the wire
/// begins (1) and where it ends (2).
#[derive(Debug, Clone, PartialEq)]
pub struct Marker {
    pub target: Target,
    /// Counts up through the demonstration: 1 start and 2 end of the first
    /// wire, 3 and 4 of the second, ...
    pub number: u8,
    pub end: bool,
}

/// What is on show at one moment.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Frame {
    pub cursor: Option<Cursor>,
    pub keys: Vec<KeyCap>,
    /// Text typed so far, while it is on show.
    pub typed: Option<String>,
    /// A click ripple at the pointer, 0 to 1.
    pub ripple: Option<f32>,
    /// Ripples a click makes (a double click makes two).
    pub ripples: u8,
    /// The click is a right click (a second ring).
    pub right: bool,
    /// Start and end markers of every drag of the demonstration, on show
    /// the whole time so the person sees where a wire begins and ends.
    pub markers: Vec<Marker>,
    /// A ghost part, with how visible it is (0 to 1).
    pub ghost: Option<(PlaceArgs, f32)>,
    /// The wire being dragged, and how far (0 to 1); it stays at 1 once the
    /// drag is done, as the ghost of the wire the person is to draw.
    pub drag: Option<(Target, Target, f32)>,
    /// The how-to line being done now (an index into [`Demo::captions`]).
    pub caption: Option<usize>,
    /// Every operation is done.
    pub finished: bool,
}

/// One line of the how-to list.
#[derive(Debug, Clone, PartialEq)]
pub struct Caption {
    /// The key in `ui.toml` that holds the sentence, with `{arg}` in it.
    pub kind: &'static str,
    /// A command id for `demo_press` and `demo_hold`; the typed text for
    /// `demo_type`; the key name for `demo_key`; the author's words for
    /// `demo_say`.
    pub arg: String,
    pub arg_nl: Option<String>,
    /// The operation it belongs to.
    pub op: usize,
}

impl Caption {
    pub fn arg(&self, lang: Lang) -> &str {
        match (lang, &self.arg_nl) {
            (Lang::Nl, Some(nl)) => nl,
            _ => &self.arg,
        }
    }
}

impl Demo {
    /// The demonstration of a step: its own `how`, else one derived from the
    /// step. Empty when the step gives nothing to show.
    pub fn for_step(step: &Step) -> Demo {
        let ops = if step.demo.is_empty() {
            derive(step)
        } else {
            step.demo.clone()
        };
        Demo::new(ops)
    }

    pub fn new(ops: Vec<DemoOp>) -> Demo {
        let mut starts = Vec::with_capacity(ops.len());
        let mut lengths = Vec::with_capacity(ops.len());
        let mut at = 0.0;
        for op in &ops {
            let len = length(op);
            starts.push(at);
            lengths.push(len);
            at += len;
        }
        Demo {
            ops,
            starts,
            lengths,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn ops(&self) -> &[DemoOp] {
        &self.ops
    }

    /// Seconds from start to the last operation's end.
    pub fn duration(&self) -> f32 {
        self.starts
            .last()
            .zip(self.lengths.last())
            .map_or(0.0, |(s, l)| s + l)
    }

    /// Where each wire of the demonstration starts and ends, for markers
    /// that show before anything is played.
    pub fn wire_ends(&self) -> Vec<(&Target, &Target)> {
        self.ops
            .iter()
            .filter_map(|op| match op {
                DemoOp::Drag(d) => Some((&d.drag.from, &d.drag.to)),
                _ => None,
            })
            .collect()
    }

    /// The lines of the how-to list, in order.
    pub fn captions(&self) -> Vec<Caption> {
        let mut out = Vec::new();
        for (i, op) in self.ops.iter().enumerate() {
            let (kind, arg, arg_nl) = match op {
                DemoOp::Press(p) => ("demo_press", p.press.clone(), None),
                DemoOp::Key(k) => ("demo_key", k.key.clone(), None),
                DemoOp::Type(t) => ("demo_type", t.text.clone(), None),
                DemoOp::Hold(h) => ("demo_hold", h.hold.clone(), None),
                DemoOp::Click(_) => ("demo_click", String::new(), None),
                DemoOp::DoubleClick(_) => ("demo_double_click", String::new(), None),
                DemoOp::RightClick(_) => ("demo_right_click", String::new(), None),
                DemoOp::Keys(k) => ("demo_key", k.keys.join(" + "), None),
                DemoOp::Drag(_) => ("demo_drag", String::new(), None),
                DemoOp::Say(s) => ("demo_say", s.say.clone(), s.say_nl.clone()),
                DemoOp::Move(_) | DemoOp::Ghost(_) | DemoOp::Wait(_) => continue,
            };
            out.push(Caption {
                kind,
                arg,
                arg_nl,
                op: i,
            });
        }
        out
    }

    /// What is on show `t` seconds in.
    pub fn frame(&self, t: f32) -> Frame {
        let mut frame = Frame::default();
        let mut at: Option<Target> = None;
        let mut captions = 0usize;
        let mut current_caption = None;
        for (i, op) in self.ops.iter().enumerate() {
            let start = self.starts[i];
            let len = self.lengths[i];
            let local = t - start;
            let running = local >= 0.0 && local < len;
            let done = local >= len;
            let has_caption = !matches!(op, DemoOp::Move(_) | DemoOp::Ghost(_) | DemoOp::Wait(_));
            if has_caption {
                if running || (done && current_caption.is_none() && local < len + LINGER_SECS) {
                    current_caption = Some(captions);
                }
                captions += 1;
            }
            match op {
                DemoOp::Move(m) => {
                    if running {
                        frame.cursor = Some(Cursor {
                            from: at.clone(),
                            to: m.to.clone(),
                            k: ease(local / len),
                        });
                    }
                    if done {
                        at = Some(m.to.clone());
                    }
                }
                DemoOp::Press(PressOp { press: id }) | DemoOp::Hold(HoldOp { hold: id, .. }) => {
                    if local >= 0.0 && local < len + LINGER_SECS {
                        // Down in the middle of the action, up before the end.
                        let down = running && local > len * 0.2 && local < len * 0.85;
                        frame.keys.push(KeyCap {
                            command: Some(id.clone()),
                            literal: None,
                            down,
                        });
                    }
                }
                DemoOp::Key(k) => {
                    if local >= 0.0 && local < len + LINGER_SECS {
                        let down = running && local > len * 0.2 && local < len * 0.85;
                        frame.keys.push(KeyCap {
                            command: None,
                            literal: Some(k.key.clone()),
                            down,
                        });
                    }
                }
                DemoOp::Type(ty) => {
                    if local >= 0.0 && local < len + LINGER_SECS + 0.4 {
                        let shown = ((local / LETTER_SECS).floor().max(0.0) as usize)
                            .min(ty.text.chars().count());
                        frame.typed = Some(ty.text.chars().take(shown).collect());
                    }
                }
                DemoOp::Click(_) | DemoOp::DoubleClick(_) | DemoOp::RightClick(_) => {
                    if running {
                        frame.ripple = Some(local / len);
                        frame.ripples = if matches!(op, DemoOp::DoubleClick(_)) {
                            2
                        } else {
                            1
                        };
                        frame.right = matches!(op, DemoOp::RightClick(_));
                    }
                }
                DemoOp::Keys(k) => {
                    if local >= 0.0 && local < len + LINGER_SECS {
                        let down = running && local > len * 0.2 && local < len * 0.85;
                        frame.keys.push(KeyCap {
                            command: None,
                            literal: Some(k.keys.join(" + ")),
                            down,
                        });
                    }
                }
                DemoOp::Drag(d) => {
                    if running {
                        let k = ease(local / len);
                        frame.cursor = Some(Cursor {
                            from: Some(d.drag.from.clone()),
                            to: d.drag.to.clone(),
                            k,
                        });
                        frame.drag = Some((d.drag.from.clone(), d.drag.to.clone(), k));
                    } else if done {
                        at = Some(d.drag.to.clone());
                        // The ghost of the wire stays for the person to follow.
                        frame.drag = Some((d.drag.from.clone(), d.drag.to.clone(), 1.0));
                    }
                    // Wire 1 has markers 1 and 2, wire 2 has 3 and 4, ...
                    let first = frame.markers.len() as u8 + 1;
                    frame.markers.push(Marker {
                        target: d.drag.from.clone(),
                        number: first,
                        end: false,
                    });
                    frame.markers.push(Marker {
                        target: d.drag.to.clone(),
                        number: first + 1,
                        end: true,
                    });
                }
                DemoOp::Ghost(g) => {
                    if local >= 0.0 {
                        let alpha = (local / GHOST_SECS).clamp(0.0, 1.0);
                        frame.ghost = Some((g.ghost.clone(), alpha));
                    }
                }
                DemoOp::Say(_) | DemoOp::Wait(_) => {}
            }
        }
        if frame.cursor.is_none()
            && let Some(at) = at
        {
            frame.cursor = Some(Cursor {
                from: None,
                to: at,
                k: 1.0,
            });
        }
        frame.caption = current_caption.or(if t >= self.duration() && captions > 0 {
            Some(captions - 1)
        } else {
            None
        });
        frame.finished = t >= self.duration();
        frame
    }
}

fn length(op: &DemoOp) -> f32 {
    match op {
        DemoOp::Press(_) | DemoOp::Key(_) => PRESS_SECS,
        DemoOp::Type(t) => t.text.chars().count() as f32 * LETTER_SECS + 0.3,
        DemoOp::Hold(h) => h.secs.clamp(0.2, 5.0),
        DemoOp::Move(_) => MOVE_SECS,
        DemoOp::Click(_) | DemoOp::RightClick(_) => CLICK_SECS,
        DemoOp::DoubleClick(_) => CLICK_SECS * 1.6,
        DemoOp::Keys(_) => PRESS_SECS,
        DemoOp::Drag(_) => DRAG_SECS,
        DemoOp::Ghost(_) => GHOST_SECS,
        DemoOp::Say(_) => 0.8,
        DemoOp::Wait(w) => w.wait.clamp(0.0, 5.0),
    }
}

/// Smooth start and stop.
fn ease(k: f32) -> f32 {
    let k = k.clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

fn at_page(page: &str, x: f64, y: f64) -> Target {
    Target {
        at: Some(At {
            page: Some(page.to_owned()),
            x: Some(x),
            y: Some(y),
            ..At::default()
        }),
        ..Target::default()
    }
}

fn at_pin(pin: &str) -> Target {
    Target {
        at: Some(At {
            pin: Some(pin.to_owned()),
            ..At::default()
        }),
        ..Target::default()
    }
}

fn at_device(tag: &str) -> Target {
    Target {
        at: Some(At {
            device: Some(tag.to_owned()),
            ..At::default()
        }),
        ..Target::default()
    }
}

/// The wire checks of a check, looking into `all`.
fn wire_checks(check: &Check) -> Vec<&crate::check::WireCheck> {
    match check {
        Check::Wire(w) => vec![w],
        Check::All(list) => list.iter().flat_map(wire_checks).collect(),
        _ => Vec::new(),
    }
}

/// The demonstration of a step that wrote none: from the command that ticks
/// it, its `solve` script and its pointer.
fn derive(step: &Step) -> Vec<DemoOp> {
    let mut out = Vec::new();
    if let Some(Check::Command(id)) = &step.expect {
        out.push(DemoOp::Press(PressOp { press: id.clone() }));
    }
    for op in &step.solve {
        match op {
            SolveOp::Command(c) => out.push(DemoOp::Press(PressOp {
                press: c.command.clone(),
            })),
            SolveOp::Place(p) => {
                let a = &p.place;
                out.push(DemoOp::Move(MoveOp {
                    to: at_page(&a.page, a.x, a.y),
                }));
                out.push(DemoOp::Ghost(GhostOp { ghost: a.clone() }));
                out.push(DemoOp::Click(ClickOp { click: true }));
            }
            SolveOp::Wire(w) => {
                out.push(DemoOp::Move(MoveOp {
                    to: at_pin(&w.wire[0]),
                }));
                out.push(DemoOp::Drag(Box::new(DragOp {
                    drag: DragArgs {
                        from: at_pin(&w.wire[0]),
                        to: at_pin(&w.wire[1]),
                    },
                })));
            }
            SolveOp::Tee(t) => {
                out.push(DemoOp::Move(MoveOp {
                    to: at_pin(&t.tee.from),
                }));
                out.push(DemoOp::Drag(Box::new(DragOp {
                    drag: DragArgs {
                        from: at_pin(&t.tee.from),
                        to: at_pin(&t.tee.onto),
                    },
                })));
            }
            SolveOp::Key(k) => out.push(DemoOp::Key(KeyOp { key: k.key.clone() })),
            SolveOp::Select(s) if !s.select.contains('~') => {
                out.push(DemoOp::Move(MoveOp {
                    to: at_device(&s.select),
                }));
                out.push(DemoOp::Click(ClickOp { click: true }));
            }
            _ => {}
        }
    }
    // A step that is ticked by a wire says where the wire goes, even with no
    // script to solve it.
    if !out.iter().any(|op| matches!(op, DemoOp::Drag(_)))
        && let Some(check) = &step.expect
    {
        for wire in wire_checks(check) {
            out.push(DemoOp::Move(MoveOp {
                to: at_pin(&wire.from),
            }));
            out.push(DemoOp::Drag(Box::new(DragOp {
                drag: DragArgs {
                    from: at_pin(&wire.from),
                    to: at_pin(&wire.to),
                },
            })));
        }
    }
    if out.is_empty()
        && let Some(point) = &step.point
    {
        out.push(DemoOp::Move(MoveOp { to: point.clone() }));
        if point.ui.is_some() {
            out.push(DemoOp::Click(ClickOp { click: true }));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lesson::Lesson;

    fn lesson(steps: &str) -> Lesson {
        Lesson::parse(&format!(
            "format = 1\nid = \"t\"\nversion = 1\ntitle = \"T\"\nsummary = \"S\"\ntrack = \"basics\"\nproject = \"@blank\"\n{steps}"
        ))
        .unwrap()
    }

    #[test]
    fn a_step_writes_its_own_demonstration() {
        let l = lesson(
            r#"
[[step]]
id = "a"
title = "A"
text = "A"
demo = [
  { press = "tool.place" },
  { type = "mcb" },
  { key = "Enter" },
  { move = { ui = "dock.place" } },
  { click = true },
  { say = "Done", say_nl = "Klaar" },
]
"#,
        );
        let demo = Demo::for_step(&l.steps[0]);
        assert_eq!(demo.ops().len(), 6);
        let kinds: Vec<_> = demo.captions().iter().map(|c| c.kind).collect();
        assert_eq!(
            kinds,
            [
                "demo_press",
                "demo_type",
                "demo_key",
                "demo_click",
                "demo_say"
            ]
        );
        assert_eq!(demo.captions()[4].arg(Lang::Nl), "Klaar");
        assert_eq!(demo.captions()[4].arg(Lang::En), "Done");
    }

    #[test]
    fn a_bad_how_op_is_an_error() {
        let text = "format = 1\nid = \"t\"\nversion = 1\ntitle = \"T\"\nsummary = \"S\"\ntrack = \"basics\"\nproject = \"@blank\"\n[[step]]\nid = \"a\"\ntitle = \"A\"\ntext = \"A\"\nhow = [ { dance = true } ]\n";
        assert!(Lesson::parse(text).is_err());
    }

    #[test]
    fn typing_goes_letter_by_letter_and_keys_go_down_then_up() {
        let demo = Demo::new(vec![
            DemoOp::Press(PressOp {
                press: "tool.place".into(),
            }),
            DemoOp::Type(TypeOp { text: "mcb".into() }),
        ]);
        let early = demo.frame(0.05);
        assert_eq!(early.keys.len(), 1);
        assert!(!early.keys[0].down);
        let mid = demo.frame(0.4);
        assert!(mid.keys[0].down);
        let typing = demo.frame(PRESS_SECS + LETTER_SECS * 1.5);
        assert_eq!(typing.typed.as_deref(), Some("m"));
        let typed = demo.frame(PRESS_SECS + LETTER_SECS * 3.5);
        assert_eq!(typed.typed.as_deref(), Some("mcb"));
        assert!(demo.frame(demo.duration() + 0.1).finished);
    }

    #[test]
    fn the_pointer_glides_then_stays() {
        let to = Target {
            ui: Some(crate::lesson::UiPath::One("dock.place".into())),
            ..Target::default()
        };
        let demo = Demo::new(vec![DemoOp::Move(MoveOp { to: to.clone() })]);
        let mid = demo.frame(MOVE_SECS / 2.0).cursor.unwrap();
        assert!(mid.k > 0.4 && mid.k < 0.6 && mid.from.is_none());
        let after = demo.frame(MOVE_SECS + 0.5).cursor.unwrap();
        assert_eq!(after.k, 1.0);
        assert_eq!(after.to, to);
    }

    #[test]
    fn a_step_with_a_solve_script_gets_a_derived_demonstration() {
        let l = lesson(
            r#"
[[step]]
id = "a"
title = "A"
text = "A"
expect = { command = "view.fit" }
solve = [ { place = { symbol = "iec/coils/relay", page = "P", x = 80, y = 104 } } ]
"#,
        );
        let demo = Demo::for_step(&l.steps[0]);
        let kinds: Vec<_> = demo.ops().iter().map(std::mem::discriminant).collect();
        assert_eq!(kinds.len(), 4); // press, move, ghost, click
        assert!(matches!(demo.ops()[0], DemoOp::Press(_)));
        assert!(matches!(demo.ops()[2], DemoOp::Ghost(_)));
        let at_end = demo.frame(demo.duration() - 0.01);
        assert!(at_end.ghost.is_some_and(|(_, a)| a == 1.0));
    }

    #[test]
    fn a_pointer_only_step_points_and_clicks_a_control() {
        let l = lesson(
            r#"
[[step]]
id = "a"
title = "A"
text = "A"
point = { ui = "dock.place" }
"#,
        );
        let demo = Demo::for_step(&l.steps[0]);
        assert!(matches!(demo.ops()[0], DemoOp::Move(_)));
        assert!(matches!(demo.ops()[1], DemoOp::Click(_)));
    }

    #[test]
    fn a_wire_check_alone_gives_a_drag_between_its_pins() {
        let l = lesson(
            r#"
[[step]]
id = "a"
title = "A"
text = "A"
expect = { wire = { from = "-F1:2", to = "-S1:13" } }
"#,
        );
        let demo = Demo::for_step(&l.steps[0]);
        let ends = demo.wire_ends();
        assert_eq!(ends.len(), 1);
        let pin = |t: &Target| t.at.as_ref().and_then(|a| a.pin.clone());
        assert_eq!(pin(ends[0].0).as_deref(), Some("-F1:2"));
        assert_eq!(pin(ends[0].1).as_deref(), Some("-S1:13"));
        let frame = demo.frame(demo.duration() + 1.0);
        let numbers: Vec<_> = frame.markers.iter().map(|m| m.number).collect();
        assert_eq!(numbers, [1, 2]);
    }

    #[test]
    fn a_reading_step_has_nothing_to_show() {
        let l = lesson("[[step]]\nid = \"a\"\ntitle = \"A\"\ntext = \"A\"\n");
        assert!(Demo::for_step(&l.steps[0]).is_empty());
    }
}
