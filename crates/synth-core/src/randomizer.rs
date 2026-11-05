// SPDX-License-Identifier: GPL-3.0-or-later
//! Lock-shielded synthesizer parameter randomizer supporting global,
//! section-scoped, and Gaussian-drift randomization.

use crate::param::{ParamDescriptor, ValueRange};
use crate::preset::SynthPatch;
use rand::distributions::Distribution;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_distr::Normal;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Scope configuration for randomization.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RandomizeScope {
    /// Randomize across all unlocked parameters in the patch.
    Global,
    /// Randomize only parameters belonging to the specified section/module.
    Section(String),
}

/// Randomization engine with parameter locking and Gaussian-drift generation.
#[derive(Debug, Clone)]
pub struct LockShieldedRandomizer {
    /// Explicitly locked parameter keys.
    locked_keys: HashSet<String>,
    /// Explicitly locked section/module names.
    locked_sections: HashSet<String>,
    /// Registered parameter metadata descriptors.
    descriptors: HashMap<String, ParamDescriptor>,
    /// Default range used for unknown parameters.
    default_range: ValueRange,
}

impl Default for LockShieldedRandomizer {
    fn default() -> Self {
        Self::new()
    }
}

impl LockShieldedRandomizer {
    /// Creates a new randomizer initialized with standard system parameter shields.
    pub fn new() -> Self {
        let mut locked_keys = HashSet::new();
        // Common system parameters that should not be randomized
        locked_keys.insert("bank_select_msb".to_string());
        locked_keys.insert("bank_select_lsb".to_string());
        locked_keys.insert("master_volume".to_string());
        locked_keys.insert("sustain_pedal".to_string());
        locked_keys.insert("expression_pedal".to_string());
        locked_keys.insert("all_notes_off".to_string());

        let mut locked_sections = HashSet::new();
        locked_sections.insert("System".to_string());

        Self {
            locked_keys,
            locked_sections,
            descriptors: HashMap::new(),
            default_range: ValueRange::midi_cc7(64.0),
        }
    }

    /// Locks an individual parameter key to prevent randomization.
    pub fn lock_param(&mut self, key: impl Into<String>) {
        self.locked_keys.insert(key.into());
    }

    /// Unlocks an individual parameter key.
    pub fn unlock_param(&mut self, key: &str) {
        self.locked_keys.remove(key);
    }

    /// Checks if an individual parameter key is locked.
    pub fn is_param_locked(&self, key: &str) -> bool {
        self.locked_keys.contains(key)
    }

    /// Locks an entire section/module name (e.g., "System", "Voice").
    pub fn lock_section(&mut self, section: impl Into<String>) {
        self.locked_sections.insert(section.into());
    }

    /// Unlocks an entire section/module.
    pub fn unlock_section(&mut self, section: &str) {
        self.locked_sections.remove(section);
    }

    /// Checks if a section is locked.
    pub fn is_section_locked(&self, section: &str) -> bool {
        self.locked_sections.contains(section)
    }

    /// Registers a parameter descriptor to provide ranges, module assignment, and lock flags.
    pub fn register_descriptor(&mut self, desc: ParamDescriptor) {
        let key = desc.id.as_str().to_string();
        if desc.is_locked {
            self.locked_keys.insert(key.clone());
        }
        self.descriptors.insert(key, desc);
    }

    /// Returns whether a parameter key is shielded either directly or by section.
    pub fn is_shielded(&self, key: &str) -> bool {
        if self.locked_keys.contains(key) {
            return true;
        }
        if let Some(desc) = self.descriptors.get(key) {
            if desc.is_locked || self.locked_sections.contains(&desc.module) {
                return true;
            }
        }
        false
    }

    /// Resolves the value range for a parameter key.
    fn resolve_range(&self, key: &str) -> &ValueRange {
        if let Some(desc) = self.descriptors.get(key) {
            &desc.range
        } else {
            &self.default_range
        }
    }

    /// Randomizes all unlocked parameters globally.
    ///
    /// - `amount_pct`: interpolation blend (0.0..100.0%).
    /// - `density_pct`: percentage of candidate parameters to mutate (0.0..100.0%).
    /// - `seed`: optional seed for repeatable pseudorandom sound design.
    pub fn randomize_global(
        &self,
        patch: &SynthPatch,
        amount_pct: f64,
        density_pct: f64,
        seed: Option<u64>,
    ) -> SynthPatch {
        self.randomize_internal(
            patch,
            RandomizeScope::Global,
            amount_pct,
            density_pct,
            None,
            seed,
        )
    }

    /// Randomizes only unlocked parameters belonging to the specified section/module.
    pub fn randomize_section(
        &self,
        patch: &SynthPatch,
        section: &str,
        amount_pct: f64,
        density_pct: f64,
        seed: Option<u64>,
    ) -> SynthPatch {
        self.randomize_internal(
            patch,
            RandomizeScope::Section(section.to_string()),
            amount_pct,
            density_pct,
            None,
            seed,
        )
    }

    /// Applies Gaussian drift randomization centered around existing parameter values.
    ///
    /// - `drift_pct`: Gaussian standard deviation scaling factor (1.0..100.0%).
    /// - `density_pct`: percentage of candidate parameters to mutate (0.0..100.0%).
    /// - `seed`: optional seed for repeatable pseudorandom sound design.
    pub fn randomize_gaussian_drift(
        &self,
        patch: &SynthPatch,
        drift_pct: f64,
        density_pct: f64,
        seed: Option<u64>,
    ) -> SynthPatch {
        let clamped_drift = drift_pct.clamp(1.0, 100.0);
        self.randomize_internal(
            patch,
            RandomizeScope::Global,
            100.0,
            density_pct,
            Some(clamped_drift),
            seed,
        )
    }

    /// Internal randomization worker.
    fn randomize_internal(
        &self,
        patch: &SynthPatch,
        scope: RandomizeScope,
        amount_pct: f64,
        density_pct: f64,
        gaussian_drift: Option<f64>,
        seed: Option<u64>,
    ) -> SynthPatch {
        let mut new_patch = patch.clone();
        let amount = (amount_pct.clamp(0.0, 100.0)) / 100.0;
        let density = (density_pct.clamp(0.0, 100.0)) / 100.0;

        let mut rng: StdRng = match seed {
            Some(s) => StdRng::seed_from_u64(s),
            None => {
                let mut fallback_seed = [0u8; 32];
                rand::thread_rng().fill(&mut fallback_seed);
                StdRng::from_seed(fallback_seed)
            }
        };

        // Collect candidate keys based on scope and shielding
        let mut candidates: Vec<String> = new_patch
            .parameters
            .keys()
            .filter(|key| {
                if self.is_shielded(key) {
                    return false;
                }
                match &scope {
                    RandomizeScope::Global => true,
                    RandomizeScope::Section(target_section) => {
                        if let Some(desc) = self.descriptors.get(*key) {
                            desc.module.eq_ignore_ascii_case(target_section)
                        } else {
                            key.starts_with(target_section)
                        }
                    }
                }
            })
            .cloned()
            .collect();

        if candidates.is_empty() {
            return new_patch;
        }

        // Apply density subset selection
        if density < 1.0 {
            candidates.shuffle(&mut rng);
            let target_count = ((candidates.len() as f64) * density).round() as usize;
            candidates.truncate(target_count);
        }

        // Mutate chosen parameters
        for key in &candidates {
            if let Some(current_val) = new_patch.parameters.get_mut(key) {
                let range = self.resolve_range(key);
                let span = range.max - range.min;

                let target_val = match gaussian_drift {
                    Some(drift) => {
                        let sigma = (span * (drift / 100.0) * 0.25).max(0.001);
                        let normal = match Normal::new(0.0, sigma) {
                            Ok(dist) => dist,
                            Err(_) => continue,
                        };
                        let sample: f64 = normal.sample(&mut rng);
                        (*current_val + sample).clamp(range.min, range.max)
                    }
                    None => {
                        let raw_sample: f64 = rng.gen_range(range.min..=range.max);
                        raw_sample
                    }
                };

                // Interpolate: current + (target - current) * amount
                let interpolated = *current_val + (target_val - *current_val) * amount;
                *current_val = range.quantize(interpolated);
            }
        }

        new_patch.touch();
        new_patch
    }
}
