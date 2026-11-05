// SPDX-License-Identifier: GPL-3.0-or-later
//! Novation Peak Factory Preset Librarian, A/B Patch Diffing, Safe-Sweep Randomizer,
//! and CommandHistory Integration (`src/librarian.rs`).
//!
//! Directly reuses `synth_core::PresetLibrary`, `synth_core::SynthPatch`,
//! `synth_core::CommandHistory`, `synth_core::PatchDiff`, and
//! `synth_core::LockShieldedRandomizer` without duplicate wrapper structs.

use crate::mod_matrix::{write_slot_to_patch, ModDestination, ModSlot, ModSource};
use crate::params::PeakMidiCcMapSpec;
use crate::sysex::encode_patch_to_sysex;
use synth_core::{
    safe_sweep_range, CommandHistory, DeviceSynthesizerProfile, LockShieldedRandomizer,
    PatchSysex, PresetLibrary, ReplacePatchCommand, SynthError, SynthPatch,
};

/// Builds a `synth_core::PresetLibrary` populated with 16 curated Novation Peak factory patches
/// spanning Banks A, B, C, and D (`4` patches per bank), complete with 16-slot modulation
/// matrix routings and 527-byte SysEx dumps.
#[must_use]
pub fn build_peak_factory_library() -> PresetLibrary {
    let profile = DeviceSynthesizerProfile::novation_peak();
    let mut library = PresetLibrary::new();

    struct FactorySpec<'a> {
        name: &'a str,
        category: &'a str,
        bank: &'a str,
        slot: u32,
        tags: &'a [&'a str],
        overrides: &'a [(&'a str, f64)],
        mod_slots: &'a [ModSlot],
    }

    let specs: &[FactorySpec<'_>] = &[
        FactorySpec {
            name: "Oxford Warm Pad",
            category: "Pad",
            bank: "A",
            slot: 0,
            tags: &["warm", "poly", "oxford", "pad"],
            overrides: &[
                ("osc_1_shape", 48.0),
                ("osc_2_shape", 72.0),
                ("osc_2_fine", 70.0),
                ("osc_2_mix", 110.0),
                ("filter_cutoff_frequency", 78.0),
                ("filter_resonance", 24.0),
                ("amp_envelope_attack", 45.0),
                ("amp_envelope_release", 82.0),
                ("chorus_level", 54.0),
                ("reverb_level", 68.0),
            ],
            mod_slots: &[
                ModSlot::new(1, ModSource::Lfo1Bipolar, ModSource::Direct, 18, ModDestination::Osc1Shape),
                ModSlot::new(2, ModSource::Animate1, ModSource::Direct, 36, ModDestination::FilterCutoff),
            ],
        },
        FactorySpec {
            name: "State Variable Acid",
            category: "Bass",
            bank: "A",
            slot: 1,
            tags: &["acid", "ota", "bass", "mono"],
            overrides: &[
                ("osc_1_range", 32.0),
                ("osc_1_shape", 96.0),
                ("filter_cutoff_frequency", 42.0),
                ("filter_resonance", 58.0),
                ("filter_overdrive", 44.0),
                ("filter_post_drive", 36.0),
                ("mod_1_filter", 98.0),
                ("mod_envelope_1_decay", 38.0),
                ("distortion_level", 32.0),
            ],
            mod_slots: &[
                ModSlot::new(1, ModSource::Velocity, ModSource::Direct, 40, ModDestination::FilterCutoff),
                ModSlot::new(2, ModSource::Animate1, ModSource::Direct, 28, ModDestination::FilterOverdrive),
            ],
        },
        FactorySpec {
            name: "Triple NCO Detune Lead",
            category: "Lead",
            bank: "A",
            slot: 2,
            tags: &["nco", "lead", "detune", "anthemic"],
            overrides: &[
                ("osc_1_mix", 127.0),
                ("osc_2_mix", 120.0),
                ("osc_3_mix", 115.0),
                ("osc_2_fine", 74.0),
                ("osc_3_fine", 54.0),
                ("filter_cutoff_frequency", 104.0),
                ("filter_resonance", 30.0),
                ("delay_level", 48.0),
                ("reverb_level", 44.0),
            ],
            mod_slots: &[
                ModSlot::new(1, ModSource::ModWheel, ModSource::Lfo2Bipolar, 24, ModDestination::Osc123Pitch),
            ],
        },
        FactorySpec {
            name: "FPGA Wavetable Pluck",
            category: "Pluck",
            bank: "A",
            slot: 3,
            tags: &["fpga", "wavetable", "pluck", "digital"],
            overrides: &[
                ("osc_1_shape", 110.0),
                ("osc_2_shape", 25.0),
                ("osc_2_mix", 95.0),
                ("filter_cutoff_frequency", 62.0),
                ("mod_1_filter", 88.0),
                ("amp_envelope_decay", 40.0),
                ("amp_envelope_sustain", 18.0),
                ("delay_level", 62.0),
                ("delay_feedback", 48.0),
            ],
            mod_slots: &[
                ModSlot::new(1, ModSource::ModEnv2, ModSource::Direct, 48, ModDestination::Osc1Shape),
            ],
        },
        FactorySpec {
            name: "Ring Mod Metallic Bell",
            category: "Bell",
            bank: "B",
            slot: 0,
            tags: &["ring_mod", "bell", "fm", "metallic"],
            overrides: &[
                ("osc_1_mix", 64.0),
                ("osc_2_coarse", 83.0),
                ("ring_mod_mix", 118.0),
                ("filter_cutoff_frequency", 92.0),
                ("amp_envelope_decay", 78.0),
                ("amp_envelope_sustain", 0.0),
                ("amp_envelope_release", 88.0),
                ("reverb_level", 76.0),
            ],
            mod_slots: &[
                ModSlot::new(1, ModSource::ModEnv1, ModSource::Direct, 32, ModDestination::Osc2Pitch),
            ],
        },
        FactorySpec {
            name: "Subterranean Drone",
            category: "Drone",
            bank: "B",
            slot: 1,
            tags: &["drone", "dark", "cinema", "ambient"],
            overrides: &[
                ("osc_1_range", 32.0),
                ("osc_2_range", 32.0),
                ("osc_3_range", 32.0),
                ("osc_2_mix", 110.0),
                ("osc_3_mix", 100.0),
                ("noise_mix", 28.0),
                ("filter_cutoff_frequency", 48.0),
                ("filter_resonance", 42.0),
                ("amp_envelope_attack", 96.0),
                ("amp_envelope_release", 110.0),
                ("reverb_level", 84.0),
                ("reverb_time", 108.0),
            ],
            mod_slots: &[
                ModSlot::new(1, ModSource::Lfo1Bipolar, ModSource::Direct, 22, ModDestination::FilterCutoff),
                ModSlot::new(2, ModSource::Lfo2Bipolar, ModSource::Direct, 30, ModDestination::Pan),
            ],
        },
        FactorySpec {
            name: "Animate Morph Brass",
            category: "Brass",
            bank: "B",
            slot: 2,
            tags: &["brass", "poly", "animate", "expressive"],
            overrides: &[
                ("osc_1_mix", 127.0),
                ("osc_2_mix", 118.0),
                ("osc_2_fine", 68.0),
                ("filter_cutoff_frequency", 68.0),
                ("filter_overdrive", 28.0),
                ("amp_envelope_attack", 22.0),
                ("mod_envelope_1_attack", 28.0),
                ("mod_1_filter", 82.0),
                ("chorus_level", 36.0),
            ],
            mod_slots: &[
                ModSlot::new(1, ModSource::Animate1, ModSource::Direct, 45, ModDestination::FilterCutoff),
                ModSlot::new(2, ModSource::Animate2, ModSource::Direct, 38, ModDestination::Osc1VSync),
            ],
        },
        FactorySpec {
            name: "Lush Chorus Strings",
            category: "Strings",
            bank: "B",
            slot: 3,
            tags: &["strings", "chorus", "vintage", "ensemble"],
            overrides: &[
                ("osc_1_shape", 64.0),
                ("osc_2_shape", 80.0),
                ("osc_3_shape", 48.0),
                ("osc_2_mix", 112.0),
                ("osc_3_mix", 108.0),
                ("filter_cutoff_frequency", 86.0),
                ("amp_envelope_attack", 56.0),
                ("amp_envelope_release", 74.0),
                ("chorus_level", 78.0),
                ("chorus_rate", 42.0),
                ("reverb_level", 58.0),
            ],
            mod_slots: &[
                ModSlot::new(1, ModSource::Lfo1Bipolar, ModSource::Direct, 14, ModDestination::Osc1Shape),
                ModSlot::new(2, ModSource::Lfo2Bipolar, ModSource::Direct, 14, ModDestination::Osc2Shape),
            ],
        },
        FactorySpec {
            name: "Cybernetic Arp",
            category: "Arp",
            bank: "C",
            slot: 0,
            tags: &["arp", "sequencer", "electro", "konduktiva"],
            overrides: &[
                ("osc_1_vsync", 48.0),
                ("osc_2_mix", 90.0),
                ("filter_cutoff_frequency", 72.0),
                ("filter_resonance", 46.0),
                ("arp_clock_gate", 52.0),
                ("delay_level", 54.0),
                ("delay_time", 48.0),
            ],
            mod_slots: &[
                ModSlot::new(1, ModSource::Velocity, ModSource::Direct, 36, ModDestination::FilterCutoff),
            ],
        },
        FactorySpec {
            name: "Analog Overdrive Bass",
            category: "Bass",
            bank: "C",
            slot: 1,
            tags: &["bass", "overdrive", "aggressive", "hybrid"],
            overrides: &[
                ("osc_1_range", 32.0),
                ("osc_2_range", 32.0),
                ("osc_2_mix", 120.0),
                ("filter_cutoff_frequency", 54.0),
                ("filter_resonance", 36.0),
                ("filter_overdrive", 46.0),
                ("filter_post_drive", 42.0),
                ("distortion_level", 38.0),
            ],
            mod_slots: &[
                ModSlot::new(1, ModSource::ModEnv1, ModSource::Direct, 44, ModDestination::FilterCutoff),
            ],
        },
        FactorySpec {
            name: "Crystalline Keys",
            category: "Keys",
            bank: "C",
            slot: 2,
            tags: &["keys", "epiano", "crystal", "poly"],
            overrides: &[
                ("osc_1_shape", 20.0),
                ("osc_2_coarse", 76.0),
                ("osc_2_mix", 68.0),
                ("filter_cutoff_frequency", 88.0),
                ("amp_envelope_decay", 68.0),
                ("amp_envelope_sustain", 42.0),
                ("amp_envelope_release", 58.0),
                ("chorus_level", 44.0),
                ("reverb_level", 52.0),
            ],
            mod_slots: &[
                ModSlot::new(1, ModSource::Velocity, ModSource::Direct, 30, ModDestination::Osc2Level),
            ],
        },
        FactorySpec {
            name: "Drift Poly Synth",
            category: "Poly",
            bank: "C",
            slot: 3,
            tags: &["poly", "drift", "analog_feel", "vintage"],
            overrides: &[
                ("osc_1_fine", 62.0),
                ("osc_2_fine", 66.0),
                ("osc_2_mix", 115.0),
                ("filter_cutoff_frequency", 82.0),
                ("filter_resonance", 28.0),
                ("amp_envelope_attack", 14.0),
                ("amp_envelope_release", 48.0),
                ("reverb_level", 40.0),
            ],
            mod_slots: &[
                ModSlot::new(1, ModSource::Aftertouch, ModSource::Direct, 32, ModDestination::FilterCutoff),
            ],
        },
        FactorySpec {
            name: "Sync Lead 24MHz",
            category: "Lead",
            bank: "D",
            slot: 0,
            tags: &["sync", "lead", "vsync", "solo"],
            overrides: &[
                ("osc_1_vsync", 84.0),
                ("osc_2_vsync", 62.0),
                ("osc_2_mix", 104.0),
                ("filter_cutoff_frequency", 96.0),
                ("filter_overdrive", 34.0),
                ("delay_level", 50.0),
            ],
            mod_slots: &[
                ModSlot::new(1, ModSource::ModEnv1, ModSource::Direct, 52, ModDestination::Osc1VSync),
            ],
        },
        FactorySpec {
            name: "Dub Delay Chord",
            category: "Chord",
            bank: "D",
            slot: 1,
            tags: &["dub", "chord", "techno", "delay"],
            overrides: &[
                ("osc_1_mix", 120.0),
                ("osc_2_coarse", 71.0),
                ("osc_2_mix", 110.0),
                ("noise_mix", 34.0),
                ("filter_cutoff_frequency", 58.0),
                ("filter_resonance", 48.0),
                ("delay_level", 78.0),
                ("delay_feedback", 64.0),
                ("reverb_level", 70.0),
            ],
            mod_slots: &[
                ModSlot::new(1, ModSource::Lfo1Unipolar, ModSource::Direct, 38, ModDestination::FilterCutoff),
            ],
        },
        FactorySpec {
            name: "Dark Velvet Pad",
            category: "Pad",
            bank: "D",
            slot: 2,
            tags: &["dark", "velvet", "pad", "lush"],
            overrides: &[
                ("osc_1_shape", 36.0),
                ("osc_2_shape", 92.0),
                ("osc_2_mix", 118.0),
                ("osc_3_mix", 96.0),
                ("filter_cutoff_frequency", 52.0),
                ("filter_resonance", 22.0),
                ("amp_envelope_attack", 68.0),
                ("amp_envelope_release", 92.0),
                ("chorus_level", 64.0),
                ("reverb_level", 74.0),
            ],
            mod_slots: &[
                ModSlot::new(1, ModSource::Animate1, ModSource::Direct, 40, ModDestination::FilterCutoff),
                ModSlot::new(2, ModSource::Animate2, ModSource::Direct, 35, ModDestination::ReverbLevel),
            ],
        },
        FactorySpec {
            name: "Konduktiva Stress Init",
            category: "LiveCoding",
            bank: "D",
            slot: 3,
            tags: &["konduktiva", "stress_test", "channel_1", "safe_sweep"],
            overrides: &[
                ("osc_1_mix", 127.0),
                ("osc_2_mix", 96.0),
                ("osc_3_mix", 64.0),
                ("filter_cutoff_frequency", 84.0),
                ("filter_resonance", 32.0),
                ("lfo_1_rate", 64.0),
                ("lfo_1_filter_depth", 48.0),
                ("distortion_level", 16.0),
                ("chorus_level", 32.0),
                ("delay_level", 40.0),
                ("reverb_level", 48.0),
            ],
            mod_slots: &[
                ModSlot::new(1, ModSource::ModWheel, ModSource::Direct, 40, ModDestination::FilterCutoff),
                ModSlot::new(2, ModSource::Animate1, ModSource::Direct, 32, ModDestination::Osc1Shape),
            ],
        },
    ];

    for spec in specs {
        let mut patch = profile
            .create_default_patch(spec.name)
            .with_category(spec.category)
            .with_bank_slot(spec.bank, spec.slot);
        patch.meta.author = Some("Novation / Renick Bell".to_string());
        for &tag in spec.tags {
            patch.add_tag(tag);
        }
        for &(param_key, val) in spec.overrides {
            patch.set_param(param_key, val);
        }
        for slot in spec.mod_slots {
            write_slot_to_patch(&mut patch, slot);
        }
        let raw_sysex = encode_patch_to_sysex(&patch, false);
        patch.sysex = PatchSysex {
            raw_dump: Some(raw_sysex),
        };
        let _ = library.add_patch(patch);
    }

    library
}

/// Builds a `synth_core::LockShieldedRandomizer` pre-configured with all 74 Novation Peak
/// parameters and default locks on performance switches (`glide_on`, `animate_1_hold`, `animate_2_hold`).
#[must_use]
pub fn build_peak_randomizer() -> LockShieldedRandomizer {
    let profile = DeviceSynthesizerProfile::novation_peak();
    let mut randomizer = LockShieldedRandomizer::new();
    for desc in &profile.parameters {
        randomizer.register_descriptor(desc.clone());
    }
    randomizer.lock_param("glide_on");
    randomizer.lock_param("animate_1_hold");
    randomizer.lock_param("animate_2_hold");
    randomizer.lock_param("arp_clock_gate");
    randomizer
}

/// Randomizes a `synth_core::SynthPatch` using `synth_core::LockShieldedRandomizer` and
/// clamps every unlocked continuous CC parameter to its acoustic `SafeSweepBounds`
/// (`Resonance <= 62`, `Distortion <= 48`, `Drive <= 50`, `Feedback <= 70`, `FX <= 90`).
#[must_use]
pub fn randomize_patch_safe(
    randomizer: &LockShieldedRandomizer,
    patch: &SynthPatch,
    amount_pct: f64,
    probability_pct: f64,
    seed: Option<u64>,
) -> SynthPatch {
    let mut mutated = randomizer.randomize_global(patch, amount_pct, probability_pct, seed);
    let spec = PeakMidiCcMapSpec::load_embedded();
    for entry in &spec.cc_map {
        let key = entry.param_id();
        if randomizer.is_shielded(key.as_str()) {
            continue;
        }
        if let Some(val) = mutated.get_param(key.as_str()) {
            let bounds = safe_sweep_range(&entry.param);
            let clamped = (val.round() as u8).clamp(bounds.lo, bounds.hi) as f64;
            mutated.set_param(key.as_str(), clamped);
        }
    }
    mutated
}

/// Applies safe-sweep-clamped randomization to `patch` and records the full patch replacement
/// directly onto `synth_core::CommandHistory` via `synth_core::ReplacePatchCommand`.
pub fn apply_safe_randomization_with_history(
    history: &mut CommandHistory,
    randomizer: &LockShieldedRandomizer,
    patch: &mut SynthPatch,
    amount_pct: f64,
    probability_pct: f64,
    seed: Option<u64>,
) -> Result<(), SynthError> {
    let next_patch = randomize_patch_safe(randomizer, patch, amount_pct, probability_pct, seed);
    let cmd = ReplacePatchCommand::new(
        patch.clone(),
        next_patch,
        format!("Safe-Sweep Randomize ({:.0}%)", amount_pct),
    );
    history.push_and_apply(Box::new(cmd), patch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use synth_core::{selective_merge_section, PatchDiff};

    #[test]
    fn test_factory_library_has_16_patches_across_4_banks_with_sysex() {
        let lib = build_peak_factory_library();
        assert_eq!(lib.len(), 16);
        assert_eq!(lib.filter_category("Bass").len(), 2);
        assert_eq!(lib.filter_tag("konduktiva").len(), 2);

        for patch in lib.all_patches() {
            let dump = patch.sysex.raw_dump.as_ref().expect("SysEx dump present");
            assert_eq!(dump.len(), 527);
        }
    }

    #[test]
    fn test_safe_sweep_randomizer_clamps_resonance_distortion_and_drive_with_undo() {
        let lib = build_peak_factory_library();
        let mut patch = lib.all_patches()[0].clone();
        let mut randomizer = build_peak_randomizer();
        randomizer.lock_param("filter_cutoff_frequency");
        let orig_cutoff = patch.get_param("filter_cutoff_frequency");

        let mut history = CommandHistory::new(16);
        for seed in 1..=10 {
            apply_safe_randomization_with_history(
                &mut history,
                &randomizer,
                &mut patch,
                100.0,
                100.0,
                Some(seed * 7919),
            )
            .expect("safe randomize");

            // Locked cutoff must never change
            assert_eq!(patch.get_param("filter_cutoff_frequency"), orig_cutoff);
            // Sensitive parameters must never exceed SafeSweepBounds
            assert!(patch.get_param("filter_resonance").unwrap() <= 62.0);
            assert!(patch.get_param("distortion_level").unwrap() <= 48.0);
            assert!(patch.get_param("filter_overdrive").unwrap() <= 50.0);
            assert!(patch.get_param("filter_post_drive").unwrap() <= 50.0);
            assert!(patch.get_param("delay_feedback").unwrap() <= 70.0);
            assert!(patch.get_param("reverb_level").unwrap() <= 90.0);
        }

        assert_eq!(history.len(), 10);
        // Undo all 10 randomizations back to original factory patch
        for _ in 0..10 {
            history.undo(&mut patch).expect("undo randomization");
        }
        let diff = PatchDiff::compare(&lib.all_patches()[0], &patch, 0.001);
        assert!(diff.identical);
    }

    #[test]
    fn test_ab_patch_diff_and_section_merge_between_factory_patches() {
        let lib = build_peak_factory_library();
        let all = lib.all_patches();
        let pad = &all[0]; // Oxford Warm Pad
        let acid = &all[1]; // State Variable Acid

        let diff = PatchDiff::compare(pad, acid, 0.001);
        assert!(!diff.identical);
        assert!(diff.total_differences() >= 8);

        let merged = selective_merge_section(pad, acid, "filter_");
        assert_eq!(
            merged.get_param("filter_resonance"),
            acid.get_param("filter_resonance")
        );
        assert_eq!(
            merged.get_param("osc_1_shape"),
            pad.get_param("osc_1_shape")
        );
    }
}
