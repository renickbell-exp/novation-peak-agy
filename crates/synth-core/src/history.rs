// SPDX-License-Identifier: GPL-3.0-or-later
//! Multi-step undo/redo command stack for synthesizer patches and parameters.

use crate::error::SynthError;
use crate::preset::SynthPatch;
use std::fmt;

/// Trait defining an undoable and redoable patch command.
pub trait PatchCommand: Send + Sync + fmt::Debug {
    /// Applies the forward action of the command to the patch.
    fn execute(&self, patch: &mut SynthPatch) -> Result<(), SynthError>;

    /// Inverts and rolls back the action of the command on the patch.
    fn undo(&self, patch: &mut SynthPatch) -> Result<(), SynthError>;

    /// Plain text description of the action.
    fn description(&self) -> &str;
}

/// Command modifying a single parameter value.
#[derive(Debug, Clone, PartialEq)]
pub struct SetParamCommand {
    pub key: String,
    pub old_val: f64,
    pub new_val: f64,
    pub desc: String,
}

impl SetParamCommand {
    /// Creates a new parameter modification command.
    pub fn new(key: impl Into<String>, old_val: f64, new_val: f64) -> Self {
        let key_str = key.into();
        let desc = format!("Change {} from {} to {}", key_str, old_val, new_val);
        Self {
            key: key_str,
            old_val,
            new_val,
            desc,
        }
    }
}

impl PatchCommand for SetParamCommand {
    fn execute(&self, patch: &mut SynthPatch) -> Result<(), SynthError> {
        patch.set_param(self.key.clone(), self.new_val);
        Ok(())
    }

    fn undo(&self, patch: &mut SynthPatch) -> Result<(), SynthError> {
        patch.set_param(self.key.clone(), self.old_val);
        Ok(())
    }

    fn description(&self) -> &str {
        &self.desc
    }
}

/// Command modifying a batch of parameters atomically.
#[derive(Debug, Clone, PartialEq)]
pub struct BatchSetParamsCommand {
    /// List of parameter changes: (key, old_value, new_value).
    pub changes: Vec<(String, f64, f64)>,
    pub desc: String,
}

impl BatchSetParamsCommand {
    /// Creates a new batch parameter modification command.
    pub fn new(changes: Vec<(String, f64, f64)>, description: impl Into<String>) -> Self {
        Self {
            changes,
            desc: description.into(),
        }
    }
}

impl PatchCommand for BatchSetParamsCommand {
    fn execute(&self, patch: &mut SynthPatch) -> Result<(), SynthError> {
        for (key, _, new_val) in &self.changes {
            patch.parameters.insert(key.clone(), *new_val);
        }
        patch.touch();
        Ok(())
    }

    fn undo(&self, patch: &mut SynthPatch) -> Result<(), SynthError> {
        for (key, old_val, _) in &self.changes {
            patch.parameters.insert(key.clone(), *old_val);
        }
        patch.touch();
        Ok(())
    }

    fn description(&self) -> &str {
        &self.desc
    }
}

/// Command replacing an entire patch document.
#[derive(Debug, Clone, PartialEq)]
pub struct ReplacePatchCommand {
    pub old_patch: SynthPatch,
    pub new_patch: SynthPatch,
    pub desc: String,
}

impl ReplacePatchCommand {
    /// Creates a new patch replacement command.
    pub fn new(
        old_patch: SynthPatch,
        new_patch: SynthPatch,
        description: impl Into<String>,
    ) -> Self {
        Self {
            old_patch,
            new_patch,
            desc: description.into(),
        }
    }
}

impl PatchCommand for ReplacePatchCommand {
    fn execute(&self, patch: &mut SynthPatch) -> Result<(), SynthError> {
        *patch = self.new_patch.clone();
        Ok(())
    }

    fn undo(&self, patch: &mut SynthPatch) -> Result<(), SynthError> {
        *patch = self.old_patch.clone();
        Ok(())
    }

    fn description(&self) -> &str {
        &self.desc
    }
}

/// Multi-step undo/redo command stack with bounded history capacity.
pub struct CommandHistory {
    commands: Vec<Box<dyn PatchCommand>>,
    cursor: usize,
    max_depth: usize,
}

impl fmt::Debug for CommandHistory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommandHistory")
            .field("cursor", &self.cursor)
            .field("len", &self.commands.len())
            .field("max_depth", &self.max_depth)
            .finish()
    }
}

impl CommandHistory {
    /// Default maximum undo/redo stack depth.
    pub const DEFAULT_MAX_DEPTH: usize = 100;

    /// Creates a new command history stack with the given capacity limit.
    pub fn new(max_depth: usize) -> Self {
        Self {
            commands: Vec::new(),
            cursor: 0,
            max_depth: max_depth.max(1),
        }
    }

    /// Pushes a command to the stack and executes it immediately against the patch.
    pub fn push_and_apply(
        &mut self,
        cmd: Box<dyn PatchCommand>,
        patch: &mut SynthPatch,
    ) -> Result<(), SynthError> {
        cmd.execute(patch)?;
        self.record(cmd);
        Ok(())
    }

    /// Records a command that has already been executed forward on the patch.
    ///
    /// Truncates any forward redo branch and enforces `max_depth` limits.
    pub fn record(&mut self, cmd: Box<dyn PatchCommand>) {
        // Discard forward redo branch if cursor is behind head
        if self.cursor < self.commands.len() {
            self.commands.truncate(self.cursor);
        }

        self.commands.push(cmd);
        self.cursor += 1;

        // Enforce maximum stack depth by dropping oldest entries
        if self.commands.len() > self.max_depth {
            let overflow = self.commands.len() - self.max_depth;
            self.commands.drain(0..overflow);
            self.cursor = self.cursor.saturating_sub(overflow);
        }
    }

    /// Reverts the most recent command on the stack.
    ///
    /// Returns the description of the reverted action on success, or `None` if stack is at start.
    pub fn undo(&mut self, patch: &mut SynthPatch) -> Result<Option<String>, SynthError> {
        if self.cursor == 0 {
            return Ok(None);
        }

        self.cursor -= 1;
        let cmd = match self.commands.get(self.cursor) {
            Some(c) => c,
            None => {
                return Err(SynthError::HistoryError(
                    "cursor index out of bounds during undo".to_string(),
                ))
            }
        };

        cmd.undo(patch)?;
        Ok(Some(cmd.description().to_string()))
    }

    /// Re-applies the next undone command on the stack.
    ///
    /// Returns the description of the reapplied action on success, or `None` if at the latest state.
    pub fn redo(&mut self, patch: &mut SynthPatch) -> Result<Option<String>, SynthError> {
        if self.cursor >= self.commands.len() {
            return Ok(None);
        }

        let cmd = match self.commands.get(self.cursor) {
            Some(c) => c,
            None => {
                return Err(SynthError::HistoryError(
                    "cursor index out of bounds during redo".to_string(),
                ))
            }
        };

        cmd.execute(patch)?;
        self.cursor += 1;
        Ok(Some(cmd.description().to_string()))
    }

    /// Returns whether an undo action is available.
    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    /// Returns whether a redo action is available.
    pub fn can_redo(&self) -> bool {
        self.cursor < self.commands.len()
    }

    /// Returns the current cursor position in the history stack.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Returns the total number of commands stored in history.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Returns whether the history stack contains no commands.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Returns the configured maximum stack depth.
    pub fn max_depth(&self) -> usize {
        self.max_depth
    }

    /// Description of the command that would be undone.
    pub fn undo_description(&self) -> Option<&str> {
        if self.cursor > 0 {
            self.commands.get(self.cursor - 1).map(|c| c.description())
        } else {
            None
        }
    }

    /// Description of the command that would be redone.
    pub fn redo_description(&self) -> Option<&str> {
        if self.cursor < self.commands.len() {
            self.commands.get(self.cursor).map(|c| c.description())
        } else {
            None
        }
    }

    /// Clears the history stack.
    pub fn clear(&mut self) {
        self.commands.clear();
        self.cursor = 0;
    }
}
