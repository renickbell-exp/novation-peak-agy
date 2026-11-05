// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthesizer patch representation and preset library CRUD manager.

use crate::error::SynthError;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Helper returning current ISO 8601 UTC timestamp string.
fn current_timestamp() -> String {
    Utc::now().to_rfc3339()
}

fn default_category() -> String {
    "Init".to_string()
}

fn default_color() -> String {
    "#888888".to_string()
}

fn default_bank() -> String {
    "A".to_string()
}

/// Metadata attributes for a synthesizer preset patch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatchMeta {
    pub id: String,
    pub name: String,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default = "default_bank")]
    pub bank: String,
    #[serde(default)]
    pub slot: u32,
    #[serde(
        default = "current_timestamp",
        alias = "createdAt",
        rename = "createdAt"
    )]
    pub created_at: String,
    #[serde(
        default = "current_timestamp",
        alias = "modifiedAt",
        rename = "modifiedAt"
    )]
    pub modified_at: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl Default for PatchMeta {
    fn default() -> Self {
        let ts = current_timestamp();
        Self {
            id: Uuid::new_v4().to_string(),
            name: "Init Patch".to_string(),
            category: default_category(),
            color: default_color(),
            favorite: false,
            bank: default_bank(),
            slot: 0,
            created_at: ts.clone(),
            modified_at: ts,
            tags: Vec::new(),
            author: None,
            notes: None,
        }
    }
}

/// SysEx dump container for hardware patch dumps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PatchSysex {
    #[serde(
        default,
        alias = "rawDump",
        rename = "rawDump",
        skip_serializing_if = "Option::is_none"
    )]
    pub raw_dump: Option<Vec<u8>>,
}

/// Complete synthesizer patch document holding metadata, parameters, and SysEx data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SynthPatch {
    pub meta: PatchMeta,
    #[serde(default)]
    pub parameters: BTreeMap<String, f64>,
    #[serde(default)]
    pub sysex: PatchSysex,
}

impl SynthPatch {
    /// Creates a new patch with the given name and a generated UUID.
    pub fn new(name: impl Into<String>) -> Self {
        let meta = PatchMeta {
            name: name.into(),
            ..Default::default()
        };
        Self {
            meta,
            parameters: BTreeMap::new(),
            sysex: PatchSysex::default(),
        }
    }

    /// Sets the category for this patch.
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.meta.category = category.into();
        self
    }

    /// Sets the bank and slot indices for this patch.
    pub fn with_bank_slot(mut self, bank: impl Into<String>, slot: u32) -> Self {
        self.meta.bank = bank.into();
        self.meta.slot = slot;
        self
    }

    /// Sets or updates a parameter value and updates the modified timestamp.
    pub fn set_param(&mut self, key: impl Into<String>, value: f64) {
        self.parameters.insert(key.into(), value);
        self.touch();
    }

    /// Retrieves a parameter value by key.
    pub fn get_param(&self, key: &str) -> Option<f64> {
        self.parameters.get(key).copied()
    }

    /// Removes a parameter value by key.
    pub fn remove_param(&mut self, key: &str) -> Option<f64> {
        let removed = self.parameters.remove(key);
        if removed.is_some() {
            self.touch();
        }
        removed
    }

    /// Checks if a parameter key exists in this patch.
    pub fn has_param(&self, key: &str) -> bool {
        self.parameters.contains_key(key)
    }

    /// Formats a canonical 1-based multi-voice parameter key (`"v{voice}_{param}"`).
    /// If `param` already starts with `"v{voice}_"`, it is normalized without double-prefixing.
    pub fn voice_param_key(voice: u8, param: &str) -> String {
        let prefix = format!("v{voice}_");
        let trimmed = param.trim();
        if let Some(rest) = trimmed.strip_prefix(&prefix) {
            format!("{prefix}{rest}")
        } else {
            format!("{prefix}{trimmed}")
        }
    }

    /// Retrieves a per-voice parameter value for 1-based `voice` (e.g., `1..=6` on Nord Drum 2).
    pub fn get_voice_param(&self, voice: u8, param: &str) -> Option<f64> {
        let key = Self::voice_param_key(voice, param);
        self.parameters.get(&key).copied()
    }

    /// Sets or updates a per-voice parameter value for 1-based `voice` and updates `modified_at`.
    pub fn set_voice_param(&mut self, voice: u8, param: &str, value: f64) {
        let key = Self::voice_param_key(voice, param);
        self.parameters.insert(key, value);
        self.touch();
    }

    /// Returns all `(unprefixed_param_name, value)` pairs belonging to 1-based `voice`.
    pub fn voice_params(&self, voice: u8) -> BTreeMap<String, f64> {
        let prefix = format!("v{voice}_");
        let mut out = BTreeMap::new();
        for (k, &v) in &self.parameters {
            if let Some(stripped) = k.strip_prefix(&prefix) {
                out.insert(stripped.to_string(), v);
            }
        }
        out
    }

    /// Copies all per-voice parameters from `src_voice` to `dst_voice`.
    pub fn copy_voice(&mut self, src_voice: u8, dst_voice: u8) {
        if src_voice == dst_voice {
            return;
        }
        let src_map = self.voice_params(src_voice);
        for (param_name, val) in src_map {
            let dst_key = Self::voice_param_key(dst_voice, &param_name);
            self.parameters.insert(dst_key, val);
        }
        self.touch();
    }

    /// Swaps all per-voice parameters between `voice_a` and `voice_b`.
    pub fn swap_voices(&mut self, voice_a: u8, voice_b: u8) {
        if voice_a == voice_b {
            return;
        }
        let map_a = self.voice_params(voice_a);
        let map_b = self.voice_params(voice_b);
        let prefix_a = format!("v{voice_a}_");
        let prefix_b = format!("v{voice_b}_");
        self.parameters
            .retain(|k, _| !k.starts_with(&prefix_a) && !k.starts_with(&prefix_b));
        for (param_name, val) in map_a {
            self.parameters
                .insert(Self::voice_param_key(voice_b, &param_name), val);
        }
        for (param_name, val) in map_b {
            self.parameters
                .insert(Self::voice_param_key(voice_a, &param_name), val);
        }
        self.touch();
    }

    /// Adds a tag to the patch metadata if not already present.
    pub fn add_tag(&mut self, tag: impl Into<String>) {
        let tag_str = tag.into();
        if !self.tags().iter().any(|t| t.eq_ignore_ascii_case(&tag_str)) {
            self.meta.tags.push(tag_str);
            self.touch();
        }
    }

    /// Removes a tag from the patch metadata.
    pub fn remove_tag(&mut self, tag: &str) {
        let before_len = self.meta.tags.len();
        self.meta.tags.retain(|t| !t.eq_ignore_ascii_case(tag));
        if self.meta.tags.len() != before_len {
            self.touch();
        }
    }

    /// Returns a slice of tags.
    pub fn tags(&self) -> &[String] {
        &self.meta.tags
    }

    /// Checks if the patch has the specified tag.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.meta.tags.iter().any(|t| t.eq_ignore_ascii_case(tag))
    }

    /// Updates the `modified_at` timestamp to the current UTC time.
    pub fn touch(&mut self) {
        self.meta.modified_at = current_timestamp();
    }

    /// Validates the structure and essential fields of this patch.
    pub fn validate(&self) -> Result<(), SynthError> {
        if self.meta.id.trim().is_empty() {
            return Err(SynthError::ValidationError(
                "patch id cannot be empty".to_string(),
            ));
        }
        if self.meta.name.trim().is_empty() {
            return Err(SynthError::ValidationError(
                "patch name cannot be empty".to_string(),
            ));
        }
        Ok(())
    }

    /// Serializes the patch to formatted JSON string.
    pub fn to_json(&self) -> Result<String, SynthError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| SynthError::SerializationError(e.to_string()))
    }

    /// Deserializes a patch from a JSON string.
    pub fn from_json(json_str: &str) -> Result<Self, SynthError> {
        let patch: Self = serde_json::from_str(json_str)
            .map_err(|e| SynthError::SerializationError(e.to_string()))?;
        patch.validate()?;
        Ok(patch)
    }
}

/// Available sort fields for the preset library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SortField {
    #[default]
    Name,
    Category,
    Color,
    BankSlot,
    CreatedAt,
    ModifiedAt,
}

/// Preset library managing an in-memory collection of synthesizer patches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PresetLibrary {
    patches: Vec<SynthPatch>,
}

impl PresetLibrary {
    /// Creates a new empty preset library.
    pub fn new() -> Self {
        Self {
            patches: Vec::new(),
        }
    }

    /// Returns the number of patches in the library.
    pub fn len(&self) -> usize {
        self.patches.len()
    }

    /// Returns whether the library is empty.
    pub fn is_empty(&self) -> bool {
        self.patches.is_empty()
    }

    /// Clears all patches from the library.
    pub fn clear(&mut self) {
        self.patches.clear();
    }

    /// Adds a patch to the library after validation.
    pub fn add_patch(&mut self, patch: SynthPatch) -> Result<(), SynthError> {
        patch.validate()?;
        if self.patches.iter().any(|p| p.meta.id == patch.meta.id) {
            return Err(SynthError::ValidationError(format!(
                "patch with id '{}' already exists in library",
                patch.meta.id
            )));
        }
        self.patches.push(patch);
        Ok(())
    }

    /// Retrieves an immutable reference to a patch by ID.
    pub fn get_patch(&self, id: &str) -> Option<&SynthPatch> {
        self.patches.iter().find(|p| p.meta.id == id)
    }

    /// Retrieves a mutable reference to a patch by ID.
    pub fn get_patch_mut(&mut self, id: &str) -> Option<&mut SynthPatch> {
        self.patches.iter_mut().find(|p| p.meta.id == id)
    }

    /// Removes a patch by ID and returns it if found.
    pub fn remove_patch(&mut self, id: &str) -> Option<SynthPatch> {
        if let Some(pos) = self.patches.iter().position(|p| p.meta.id == id) {
            Some(self.patches.remove(pos))
        } else {
            None
        }
    }

    /// Updates an existing patch in the library with matching ID.
    pub fn update_patch(&mut self, patch: SynthPatch) -> Result<(), SynthError> {
        patch.validate()?;
        if let Some(pos) = self.patches.iter().position(|p| p.meta.id == patch.meta.id) {
            self.patches[pos] = patch;
            Ok(())
        } else {
            Err(SynthError::PatchNotFound(patch.meta.id))
        }
    }

    /// Returns a slice of all patches.
    pub fn all_patches(&self) -> &[SynthPatch] {
        &self.patches
    }

    /// Searches patches matching a case-insensitive query in name, category, or tags.
    pub fn search(&self, query: &str) -> Vec<&SynthPatch> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return self.patches.iter().collect();
        }
        self.patches
            .iter()
            .filter(|p| {
                p.meta.name.to_lowercase().contains(&q)
                    || p.meta.category.to_lowercase().contains(&q)
                    || p.meta.tags.iter().any(|t| t.to_lowercase().contains(&q))
            })
            .collect()
    }

    /// Filters patches marked as favorite.
    pub fn filter_favorites(&self) -> Vec<&SynthPatch> {
        self.patches.iter().filter(|p| p.meta.favorite).collect()
    }

    /// Filters patches by bank.
    pub fn filter_bank(&self, bank: &str) -> Vec<&SynthPatch> {
        self.patches
            .iter()
            .filter(|p| p.meta.bank.eq_ignore_ascii_case(bank))
            .collect()
    }

    /// Filters patches by category.
    pub fn filter_category(&self, category: &str) -> Vec<&SynthPatch> {
        self.patches
            .iter()
            .filter(|p| p.meta.category.eq_ignore_ascii_case(category))
            .collect()
    }

    /// Filters patches having a specific tag.
    pub fn filter_tag(&self, tag: &str) -> Vec<&SynthPatch> {
        self.patches.iter().filter(|p| p.has_tag(tag)).collect()
    }

    /// Sorts the library in-place by the specified field.
    pub fn sort_by(&mut self, field: SortField, ascending: bool) {
        self.patches.sort_by(|a, b| {
            let ordering = match field {
                SortField::Name => a.meta.name.to_lowercase().cmp(&b.meta.name.to_lowercase()),
                SortField::Category => a
                    .meta
                    .category
                    .to_lowercase()
                    .cmp(&b.meta.category.to_lowercase()),
                SortField::Color => a.meta.color.cmp(&b.meta.color),
                SortField::BankSlot => {
                    let bank_cmp = a.meta.bank.cmp(&b.meta.bank);
                    if bank_cmp.is_eq() {
                        a.meta.slot.cmp(&b.meta.slot)
                    } else {
                        bank_cmp
                    }
                }
                SortField::CreatedAt => a.meta.created_at.cmp(&b.meta.created_at),
                SortField::ModifiedAt => a.meta.modified_at.cmp(&b.meta.modified_at),
            };

            if ascending {
                ordering
            } else {
                ordering.reverse()
            }
        });
    }

    /// Serializes the library to formatted JSON.
    pub fn to_json(&self) -> Result<String, SynthError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| SynthError::SerializationError(e.to_string()))
    }

    /// Deserializes a library from JSON string.
    pub fn from_json(json_str: &str) -> Result<Self, SynthError> {
        let library: Self = serde_json::from_str(json_str)
            .map_err(|e| SynthError::SerializationError(e.to_string()))?;
        for patch in &library.patches {
            patch.validate()?;
        }
        Ok(library)
    }
}

#[cfg(test)]
mod multi_voice_tests {
    use super::*;

    #[test]
    fn test_synth_patch_native_multi_voice_helpers() {
        let mut patch = SynthPatch::new("ND2 Multi-Voice Kit");
        patch.set_voice_param(1, "tone_pitch", 48.0);
        patch.set_voice_param(1, "noise_decay", 64.0);
        patch.set_voice_param(2, "tone_pitch", 72.0);
        patch.set_voice_param(2, "noise_decay", 30.0);

        assert_eq!(SynthPatch::voice_param_key(1, "tone_pitch"), "v1_tone_pitch");
        assert_eq!(
            SynthPatch::voice_param_key(1, "v1_tone_pitch"),
            "v1_tone_pitch"
        );
        assert_eq!(patch.get_voice_param(1, "tone_pitch"), Some(48.0));
        assert_eq!(patch.get_voice_param(2, "tone_pitch"), Some(72.0));

        let v1_map = patch.voice_params(1);
        assert_eq!(v1_map.len(), 2);
        assert_eq!(v1_map.get("tone_pitch"), Some(&48.0));

        patch.copy_voice(1, 3);
        assert_eq!(patch.get_voice_param(3, "tone_pitch"), Some(48.0));
        assert_eq!(patch.get_voice_param(3, "noise_decay"), Some(64.0));

        patch.swap_voices(1, 2);
        assert_eq!(patch.get_voice_param(1, "tone_pitch"), Some(72.0));
        assert_eq!(patch.get_voice_param(2, "tone_pitch"), Some(48.0));
    }
}
