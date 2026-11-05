// SPDX-License-Identifier: GPL-3.0-or-later
//! Novation Peak 74-CC parameter registry, dual-CC pair resolver, and parity bridge
//! with `js/konduktiva-peak/midi-cc-map.json` and `synth_core::DeviceSynthesizerProfile::novation_peak()`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use synth_core::{
    safe_sweep_range, DeviceSynthesizerProfile, ParamAddress, ParamId, SafeSweepBounds,
};

/// Raw JSON string of `js/konduktiva-peak/midi-cc-map.json` embedded at compile time.
pub const EMBEDDED_MIDI_CC_MAP_JSON: &str = include_str!("../js/konduktiva-peak/midi-cc-map.json");

/// A single entry in `js/konduktiva-peak/midi-cc-map.json`'s `cc_map` array.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeakCcMapEntry {
    pub cc: u8,
    pub param: String,
    pub range: String,
    pub notes: String,
}

impl PeakCcMapEntry {
    /// Returns the normalized `synth_core::ParamId` for this CC parameter.
    pub fn param_id(&self) -> ParamId {
        ParamId::from_name(&self.param)
    }

    /// Parses the `"0-127"` range string into `[min, max]`.
    pub fn parsed_range(&self) -> [u8; 2] {
        let mut parts = self.range.split('-');
        let lo = parts
            .next()
            .and_then(|s| s.trim().parse::<u8>().ok())
            .unwrap_or(0);
        let hi = parts
            .next()
            .and_then(|s| s.trim().parse::<u8>().ok())
            .unwrap_or(127);
        [lo, hi]
    }

    /// Returns the safe sweep bounds (`{ lo, hi }`) for automated CC modulation.
    pub fn safe_sweep_bounds(&self) -> SafeSweepBounds {
        safe_sweep_range(&self.param)
    }

    /// Returns the paired +32 CC number if this parameter is part of a Peak dual-CC pair.
    pub fn dual_cc_partner(&self) -> Option<u8> {
        dual_cc_pair(self.cc)
    }
}

/// A recommended stress-test CC entry from `js/konduktiva-peak/midi-cc-map.json`'s `_meta`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeakStressCcEntry {
    pub cc: u8,
    pub param: String,
    pub why: String,
}

/// Metadata block `_meta` in `js/konduktiva-peak/midi-cc-map.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeakMidiCcMapMeta {
    pub device: String,
    pub usb_id: String,
    pub usb_name: String,
    pub source: String,
    pub confidence: String,
    pub resolution: String,
    pub receive_channel: String,
    pub caveats: Vec<String>,
    pub recommended_for_stress_test: Vec<PeakStressCcEntry>,
}

/// Top-level deserialized structure of `js/konduktiva-peak/midi-cc-map.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeakMidiCcMapSpec {
    #[serde(rename = "_meta")]
    pub meta: PeakMidiCcMapMeta,
    pub cc_map: Vec<PeakCcMapEntry>,
}

impl PeakMidiCcMapSpec {
    /// Loads and parses the embedded `js/konduktiva-peak/midi-cc-map.json`.
    pub fn load_embedded() -> Self {
        serde_json::from_str(EMBEDDED_MIDI_CC_MAP_JSON)
            .expect("Embedded js/konduktiva-peak/midi-cc-map.json must be valid JSON")
    }

    /// Resolves a CC entry by exact CC number (`0..=127`).
    pub fn resolve_cc_by_number(&self, cc_num: u8) -> Option<&PeakCcMapEntry> {
        self.cc_map.iter().find(|e| e.cc == cc_num)
    }

    /// Resolves a parameter name to a CC number, searching `recommended_for_stress_test` first
    /// and then `cc_map` (matching `resolveCc` in `js/konduktiva-peak/src/cc.js`).
    pub fn resolve_cc(&self, query: &str) -> Option<u8> {
        let needle = query.trim().to_lowercase();
        for ent in &self.meta.recommended_for_stress_test {
            if ent.param.to_lowercase().contains(&needle) {
                return Some(ent.cc);
            }
        }
        for ent in &self.cc_map {
            if ent.param.to_lowercase().contains(&needle)
                || ent.param_id().as_str().contains(&needle)
            {
                return Some(ent.cc);
            }
        }
        None
    }

    /// Resolves a CC entry by case-insensitive exact or substring match on `param` or normalized ID.
    pub fn resolve_cc_by_name(&self, query: &str) -> Option<&PeakCcMapEntry> {
        self.resolve_cc(query)
            .and_then(|cc| self.resolve_cc_by_number(cc))
    }

    /// Resolves the five musical CCs used by the stress kit (`cutoff`, `reso`, `mod`, `reverb`, `delay`),
    /// matching `buildCcSet()` in `js/konduktiva-peak/src/cc.js`.
    pub fn build_cc_set(&self) -> BTreeMap<String, u8> {
        let mut out = BTreeMap::new();
        out.insert(
            "cutoff".to_string(),
            self.resolve_cc("cutoff").unwrap_or(74),
        );
        out.insert(
            "reso".to_string(),
            self.resolve_cc("resonance").unwrap_or(71),
        );
        out.insert(
            "mod".to_string(),
            self.resolve_cc("mod wheel").unwrap_or(1),
        );
        out.insert(
            "reverb".to_string(),
            self.resolve_cc("reverb level").unwrap_or(91),
        );
        out.insert(
            "delay".to_string(),
            self.resolve_cc("delay level").unwrap_or(93),
        );
        out
    }
}

/// Returns the paired CC (`cc + 32` or `cc - 32`) for Peak parameters that use dual-CC pairs
/// on hardware firmware (`23/55`, `24/56`, `25/57`, `29/61`, `30/62`, `31/63`).
pub fn dual_cc_pair(cc: u8) -> Option<u8> {
    match cc {
        23 => Some(55),
        24 => Some(56),
        25 => Some(57),
        29 => Some(61),
        30 => Some(62),
        31 => Some(63),
        55 => Some(23),
        56 => Some(24),
        57 => Some(25),
        61 => Some(29),
        62 => Some(30),
        63 => Some(31),
        _ => None,
    }
}

/// Verifies that every entry in `js/konduktiva-peak/midi-cc-map.json` exists in
/// `synth_core::DeviceSynthesizerProfile::novation_peak()` with identical CC address and bounds.
pub fn verify_profile_parity_with_cc_map() -> Result<usize, String> {
    let spec = PeakMidiCcMapSpec::load_embedded();
    let profile = DeviceSynthesizerProfile::novation_peak();

    if spec.cc_map.len() != profile.parameters.len() {
        return Err(format!(
            "Parameter count mismatch: midi-cc-map.json has {}, profile has {}",
            spec.cc_map.len(),
            profile.parameters.len()
        ));
    }

    for entry in &spec.cc_map {
        let key = entry.param_id();
        let desc = profile.get_param(key.as_str()).ok_or_else(|| {
            format!(
                "Parameter '{}' (key '{}', CC {}) missing from DeviceSynthesizerProfile::novation_peak()",
                entry.param, key, entry.cc
            )
        })?;
        if desc.address != ParamAddress::cc7(entry.cc) {
            return Err(format!(
                "CC mismatch for '{}': expected CC {}, got {:?}",
                entry.param, entry.cc, desc.address
            ));
        }
        let [lo, hi] = entry.parsed_range();
        if desc.range.min as u8 != lo || desc.range.max as u8 != hi {
            return Err(format!(
                "Range mismatch for '{}': expected {}..={}, got {}..={}",
                entry.param, lo, hi, desc.range.min, desc.range.max
            ));
        }
    }

    Ok(spec.cc_map.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_midi_cc_map_loads_74_entries_and_9_stress_ccs() {
        let spec = PeakMidiCcMapSpec::load_embedded();
        assert_eq!(spec.meta.device, "Novation Peak");
        assert_eq!(spec.meta.usb_id, "1235:007e");
        assert_eq!(spec.cc_map.len(), 74);
        assert_eq!(spec.meta.recommended_for_stress_test.len(), 9);
    }

    #[test]
    fn test_profile_parity_with_embedded_cc_map() {
        let count = verify_profile_parity_with_cc_map().expect("Parity check failed");
        assert_eq!(count, 74);
    }

    #[test]
    fn test_resolve_cc_and_build_cc_set() {
        let spec = PeakMidiCcMapSpec::load_embedded();
        let cutoff = spec.resolve_cc_by_name("Filter cutoff").expect("cutoff");
        assert_eq!(cutoff.cc, 29);
        assert_eq!(cutoff.dual_cc_partner(), Some(61));

        let reso = spec.resolve_cc_by_number(79).expect("resonance");
        assert_eq!(reso.param, "Filter resonance");
        assert_eq!(reso.safe_sweep_bounds(), SafeSweepBounds { lo: 0, hi: 62 });

        let cc_set = spec.build_cc_set();
        assert_eq!(cc_set.len(), 5);
        assert_eq!(cc_set.get("cutoff"), Some(&29));
        assert_eq!(cc_set.get("reso"), Some(&79));
        assert_eq!(cc_set.get("mod"), Some(&1));
        assert_eq!(cc_set.get("reverb"), Some(&112));
        assert_eq!(cc_set.get("delay"), Some(&108));
    }

    #[test]
    fn test_dual_cc_pairs() {
        assert_eq!(dual_cc_pair(29), Some(61));
        assert_eq!(dual_cc_pair(61), Some(29));
        assert_eq!(dual_cc_pair(23), Some(55));
        assert_eq!(dual_cc_pair(30), Some(62));
        assert_eq!(dual_cc_pair(79), None);
    }
}
