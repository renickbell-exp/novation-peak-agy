// SPDX-License-Identifier: GPL-3.0-or-later
//! Error types for synth-core domain operations.

use std::fmt;

/// Errors that can occur during synthesizer parameter, patch, or history manipulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SynthError {
    /// The parameter identifier was invalid or malformed.
    InvalidParamId(String),
    /// A value range definition was mathematically invalid.
    InvalidRange {
        min: String,
        max: String,
        reason: String,
    },
    /// A patch with the requested identifier could not be found.
    PatchNotFound(String),
    /// Patch data failed validation.
    ValidationError(String),
    /// JSON serialization or deserialization failed.
    SerializationError(String),
    /// Command history could not execute an undo or redo action.
    HistoryError(String),
}

impl fmt::Display for SynthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidParamId(id) => write!(f, "invalid parameter identifier: '{id}'"),
            Self::InvalidRange { min, max, reason } => {
                write!(f, "invalid range [{min}..{max}]: {reason}")
            }
            Self::PatchNotFound(id) => write!(f, "patch '{id}' not found in library"),
            Self::ValidationError(msg) => write!(f, "patch validation error: {msg}"),
            Self::SerializationError(msg) => write!(f, "serialization error: {msg}"),
            Self::HistoryError(msg) => write!(f, "command history error: {msg}"),
        }
    }
}

impl std::error::Error for SynthError {}
