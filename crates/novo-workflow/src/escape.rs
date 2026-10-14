//! The one Escape ladder (principle 4): cancel the step in progress, then
//! leave the tool, then clear the selection. One rung per press.

/// What the editor is doing when Escape is pressed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EditorState {
    /// A step is half done: a wire with its first click, a drag, an open
    /// insert search.
    pub step_in_progress: bool,
    /// A tool other than plain selection is active.
    pub tool_active: bool,
    pub has_selection: bool,
}

/// What one Escape press does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscapeAction {
    CancelStep,
    LeaveTool,
    ClearSelection,
    Nothing,
}

/// The rung of the ladder for this state.
pub fn escape(state: EditorState) -> EscapeAction {
    if state.step_in_progress {
        EscapeAction::CancelStep
    } else if state.tool_active {
        EscapeAction::LeaveTool
    } else if state.has_selection {
        EscapeAction::ClearSelection
    } else {
        EscapeAction::Nothing
    }
}

impl EditorState {
    /// The state after `action`, so the ladder can be walked in tests and
    /// by the app.
    pub fn after(self, action: EscapeAction) -> EditorState {
        match action {
            EscapeAction::CancelStep => EditorState {
                step_in_progress: false,
                ..self
            },
            EscapeAction::LeaveTool => EditorState {
                tool_active: false,
                ..self
            },
            EscapeAction::ClearSelection => EditorState {
                has_selection: false,
                ..self
            },
            EscapeAction::Nothing => self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ladder_goes_step_tool_selection() {
        let mut state = EditorState {
            step_in_progress: true,
            tool_active: true,
            has_selection: true,
        };
        let mut rungs = Vec::new();
        loop {
            let action = escape(state);
            rungs.push(action);
            if action == EscapeAction::Nothing {
                break;
            }
            state = state.after(action);
        }
        assert_eq!(
            rungs,
            [
                EscapeAction::CancelStep,
                EscapeAction::LeaveTool,
                EscapeAction::ClearSelection,
                EscapeAction::Nothing
            ]
        );
    }
}
