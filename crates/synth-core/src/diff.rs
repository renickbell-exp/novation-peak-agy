// SPDX-License-Identifier: GPL-3.0-or-later
//! A/B Synthesizer patch comparison and selective parameter merging.

use crate::preset::SynthPatch;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Difference record for a single parameter between two patches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParamDiff {
    pub key: String,
    pub val_a: Option<f64>,
    pub val_b: Option<f64>,
    pub delta: Option<f64>,
    pub percent_change: Option<f64>,
}

/// Comparison result holding parameter additions, removals, and modifications between Patch A and Patch B.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatchDiff {
    pub patch_a_id: String,
    pub patch_b_id: String,
    pub patch_a_name: String,
    pub patch_b_name: String,
    pub identical: bool,
    /// Keys present in B but not in A.
    pub added: Vec<String>,
    /// Keys present in A but not in B.
    pub removed: Vec<String>,
    /// Keys present in both whose values differ by more than `epsilon`.
    pub modified: Vec<ParamDiff>,
    /// Number of parameters present in both with identical values.
    pub unchanged_count: usize,
}

impl PatchDiff {
    /// Compares two patches and computes differences within an absolute `epsilon` tolerance.
    pub fn compare(a: &SynthPatch, b: &SynthPatch, epsilon: f64) -> Self {
        let eps = epsilon.max(0.0);
        let mut all_keys = BTreeSet::new();
        for key in a.parameters.keys() {
            all_keys.insert(key.clone());
        }
        for key in b.parameters.keys() {
            all_keys.insert(key.clone());
        }

        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut modified = Vec::new();
        let mut unchanged_count = 0;

        for key in all_keys {
            let val_a = a.parameters.get(&key).copied();
            let val_b = b.parameters.get(&key).copied();

            match (val_a, val_b) {
                (None, Some(vb)) => {
                    added.push(key.clone());
                    modified.push(ParamDiff {
                        key,
                        val_a: None,
                        val_b: Some(vb),
                        delta: None,
                        percent_change: None,
                    });
                }
                (Some(va), None) => {
                    removed.push(key.clone());
                    modified.push(ParamDiff {
                        key,
                        val_a: Some(va),
                        val_b: None,
                        delta: None,
                        percent_change: None,
                    });
                }
                (Some(va), Some(vb)) => {
                    let diff = (va - vb).abs();
                    if diff > eps {
                        let delta = vb - va;
                        let percent_change = if va.abs() > f64::EPSILON {
                            Some((delta / va) * 100.0)
                        } else {
                            None
                        };
                        modified.push(ParamDiff {
                            key,
                            val_a: Some(va),
                            val_b: Some(vb),
                            delta: Some(delta),
                            percent_change,
                        });
                    } else {
                        unchanged_count += 1;
                    }
                }
                (None, None) => {}
            }
        }

        let identical = added.is_empty() && removed.is_empty() && modified.is_empty();

        Self {
            patch_a_id: a.meta.id.clone(),
            patch_b_id: b.meta.id.clone(),
            patch_a_name: a.meta.name.clone(),
            patch_b_name: b.meta.name.clone(),
            identical,
            added,
            removed,
            modified,
            unchanged_count,
        }
    }

    /// Total number of parameter differences (added + removed + modified).
    pub fn total_differences(&self) -> usize {
        self.added.len() + self.removed.len() + self.modified.len()
    }
}

/// Selectively merges specified keys from `source` patch into `base` patch, returning a new patch.
pub fn selective_merge(
    base: &SynthPatch,
    source: &SynthPatch,
    keys_to_merge: &[String],
) -> SynthPatch {
    let mut result = base.clone();
    for key in keys_to_merge {
        if let Some(src_val) = source.parameters.get(key) {
            result.parameters.insert(key.clone(), *src_val);
        }
    }
    result.touch();
    result
}

/// Selectively merges all keys matching a section prefix (e.g., "osc1_", "filter_") from `source` into `base`.
pub fn selective_merge_section(
    base: &SynthPatch,
    source: &SynthPatch,
    section_prefix: &str,
) -> SynthPatch {
    selective_merge_matching(base, source, |key| key.starts_with(section_prefix))
}

/// Selectively merges all keys satisfying a predicate from `source` into `base`.
pub fn selective_merge_matching<F>(
    base: &SynthPatch,
    source: &SynthPatch,
    predicate: F,
) -> SynthPatch
where
    F: Fn(&str) -> bool,
{
    let mut result = base.clone();
    for (key, val) in &source.parameters {
        if predicate(key) {
            result.parameters.insert(key.clone(), *val);
        }
    }
    result.touch();
    result
}
