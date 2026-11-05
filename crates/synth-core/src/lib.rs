// SPDX-License-Identifier: GPL-3.0-or-later
//! `synth-core`: Shared synthesizer parameter abstractions, preset library CRUD,
//! A/B patch diffing, lock-shielded randomization, and multi-step command history.
//!
//! Designed for cross-project reuse across `nkeditor`, `asm-hydrasynth-editor`,
//! `bs2-td3-manager`, and `nord-drum-2-manager`.

pub mod diff;
pub mod error;
pub mod history;
pub mod param;
pub mod preset;
pub mod profiles;
pub mod randomizer;

pub use diff::{
    selective_merge, selective_merge_matching, selective_merge_section, ParamDiff, PatchDiff,
};
pub use error::SynthError;
pub use history::{
    BatchSetParamsCommand, CommandHistory, PatchCommand, ReplacePatchCommand, SetParamCommand,
};
pub use param::{
    safe_sweep_range, ParamAddress, ParamDescriptor, ParamId, ResponseCurve, SafeSweepBounds,
    SoftTakeoverMode, SoftTakeoverResult, SoftTakeoverState, ValueRange,
};
pub use preset::{PatchMeta, PatchSysex, PresetLibrary, SortField, SynthPatch};
pub use profiles::{
    controller_mapping_json_schema, device_profile_json_schema, export_all_schemas,
    preset_json_schema, validate_controller_mapping_json, validate_preset_json,
    validate_profile_json, ControlElementKind, ControllerSurfaceMapping, DeviceCategory,
    DeviceSynthesizerProfile, PhysicalControlDescriptor, SurfaceBankMapping, SurfaceControlBinding,
};
pub use randomizer::{LockShieldedRandomizer, RandomizeScope};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_param_id_normalization() {
        let id1 = ParamId::from_name("Filter 1 Cutoff");
        assert_eq!(id1.as_str(), "filter_1_cutoff");

        let id2 = ParamId::from_name("OSC 1 WavScan");
        assert_eq!(id2.as_str(), "osc_1_wavscan");

        let id3 = ParamId::from_name("   LFO 2 Rate--- ");
        assert_eq!(id3.as_str(), "lfo_2_rate");

        let id_direct = ParamId::new("amp_env_attack");
        assert_eq!(id_direct.as_str(), "amp_env_attack");
    }

    #[test]
    fn test_param_addresses() {
        let addr_cc7 = ParamAddress::cc7(74);
        assert!(addr_cc7.is_cc7());
        assert!(!addr_cc7.is_cc14());

        let addr_cc14 = ParamAddress::cc14(16, 48);
        assert!(addr_cc14.is_cc14());

        let addr_nrpn = ParamAddress::nrpn(0, 72);
        assert!(addr_nrpn.is_nrpn());

        let addr_sysex = ParamAddress::sysex(32, 2);
        assert!(addr_sysex.is_sysex());
    }

    #[test]
    fn test_value_range_normalization_and_quantization() {
        let cc7_range = ValueRange::midi_cc7(64.0);
        assert_eq!(cc7_range.normalize(0.0), 0.0);
        assert_eq!(cc7_range.normalize(63.5), 0.5);
        assert_eq!(cc7_range.normalize(127.0), 1.0);
        assert_eq!(cc7_range.denormalize(0.5), 64.0);

        let bipolar = ValueRange::bipolar(-100.0, 100.0, 0.0, "cents");
        assert_eq!(bipolar.normalize(0.0), 0.5);
        assert_eq!(bipolar.denormalize(0.0), -100.0);
        assert_eq!(bipolar.denormalize(1.0), 100.0);
        assert_eq!(bipolar.format_value(25.0), "25 cents");

        // Invalid range check
        let invalid = ValueRange::new(100.0, 50.0, 60.0, 1.0, "");
        assert!(invalid.is_err());
    }

    #[test]
    fn test_response_curves() {
        let curves = [
            ResponseCurve::Linear,
            ResponseCurve::Exponential,
            ResponseCurve::Logarithmic,
            ResponseCurve::SCurve,
        ];

        for curve in &curves {
            // Endpoints must always be 0.0 and 1.0
            assert!((curve.transform(0.0) - 0.0).abs() < 1e-6);
            assert!((curve.transform(1.0) - 1.0).abs() < 1e-6);

            // Invertibility roundtrip
            for step in 1..9 {
                let x = (step as f64) * 0.1;
                let curved = curve.transform(x);
                let recovered = curve.inverse(curved);
                assert!(
                    (x - recovered).abs() < 1e-3,
                    "curve {:?} roundtrip failed at x = {x}: got {recovered}",
                    curve
                );
            }
        }
    }

    #[test]
    fn test_soft_takeover_jump_mode() {
        let mut takeover = SoftTakeoverState::new(SoftTakeoverMode::Jump, 50.0);
        assert!(takeover.is_caught);

        let res = takeover.update(75.0);
        assert_eq!(res.value, 75.0);
        assert!(res.updated);
        assert!(res.is_caught);
        assert_eq!(res.delta, 0.0);
    }

    #[test]
    fn test_soft_takeover_catch_mode() {
        let mut takeover =
            SoftTakeoverState::new(SoftTakeoverMode::Catch, 80.0).with_threshold(1.0);
        assert!(!takeover.is_caught);

        // Physical controller starts far away (20.0)
        let res1 = takeover.update(20.0);
        assert!(!res1.is_caught);
        assert!(!res1.updated);
        assert_eq!(res1.value, 80.0);
        assert_eq!(takeover.delta(), Some(-60.0));
        assert_eq!(takeover.abs_delta(), Some(60.0));

        // Controller moves closer (50.0)
        let res2 = takeover.update(50.0);
        assert!(!res2.is_caught);
        assert!(!res2.updated);
        assert_eq!(res2.value, 80.0);

        // Controller crosses target (85.0)
        let res3 = takeover.update(85.0);
        assert!(res3.is_caught);
        assert!(res3.updated);
        assert_eq!(res3.value, 85.0);

        // Once caught, subsequent movements update directly
        let res4 = takeover.update(90.0);
        assert!(res4.is_caught);
        assert!(res4.updated);
        assert_eq!(res4.value, 90.0);
    }

    #[test]
    fn test_soft_takeover_pickup_mode() {
        let mut takeover =
            SoftTakeoverState::new(SoftTakeoverMode::Pickup, 80.0).with_threshold(2.0);
        assert!(!takeover.is_caught);

        // Initial movement from 20 to 30
        takeover.update(20.0);
        let res = takeover.update(30.0);
        assert!(res.updated);
        // Target moved proportionally closer to 30
        assert!(res.value < 80.0);
    }

    #[test]
    fn test_synth_patch_crud_and_json_serialization() {
        let mut patch = SynthPatch::new("Acid Lead 01")
            .with_category("Bass")
            .with_bank_slot("B", 14);

        patch.set_param("filter_cutoff", 95.0);
        patch.set_param("resonance", 110.0);
        patch.add_tag("acid");
        patch.add_tag("analogue");

        assert_eq!(patch.get_param("filter_cutoff"), Some(95.0));
        assert!(patch.has_tag("acid"));
        assert_eq!(patch.tags().len(), 2);

        // JSON serialization
        let json = patch.to_json().expect("patch to json failed");
        assert!(json.contains("Acid Lead 01"));
        assert!(json.contains("filter_cutoff"));

        // Roundtrip deserialization
        let deserialized = SynthPatch::from_json(&json).expect("from json failed");
        assert_eq!(deserialized.meta.name, "Acid Lead 01");
        assert_eq!(deserialized.meta.bank, "B");
        assert_eq!(deserialized.meta.slot, 14);
        assert_eq!(deserialized.get_param("filter_cutoff"), Some(95.0));
        assert_eq!(deserialized.get_param("resonance"), Some(110.0));
    }

    #[test]
    fn test_preset_library_crud_and_search() {
        let mut lib = PresetLibrary::new();
        assert!(lib.is_empty());

        let mut p1 = SynthPatch::new("Sub Bass Punch").with_category("Bass");
        p1.add_tag("deep");
        p1.meta.favorite = true;

        let mut p2 = SynthPatch::new("Liquid Acid Lead").with_category("Lead");
        p2.add_tag("acid");

        let id1 = p1.meta.id.clone();
        let id2 = p2.meta.id.clone();

        lib.add_patch(p1).expect("add p1 failed");
        lib.add_patch(p2).expect("add p2 failed");
        assert_eq!(lib.len(), 2);

        // Get patch
        assert_eq!(
            lib.get_patch(&id1).map(|p| p.meta.name.as_str()),
            Some("Sub Bass Punch")
        );
        assert_eq!(
            lib.get_patch(&id2).map(|p| p.meta.name.as_str()),
            Some("Liquid Acid Lead")
        );

        // Search
        let search_results = lib.search("acid");
        assert_eq!(search_results.len(), 1);
        assert_eq!(search_results[0].meta.name, "Liquid Acid Lead");

        // Favorites filter
        let favs = lib.filter_favorites();
        assert_eq!(favs.len(), 1);
        assert_eq!(favs[0].meta.name, "Sub Bass Punch");

        // Remove patch
        let removed = lib.remove_patch(&id1);
        assert!(removed.is_some());
        assert_eq!(lib.len(), 1);
        assert!(lib.get_patch(&id1).is_none());
    }

    #[test]
    fn test_patch_diff_and_selective_merge() {
        let mut p_a = SynthPatch::new("Patch A");
        p_a.set_param("osc1_wave", 1.0);
        p_a.set_param("filter_cutoff", 50.0);
        p_a.set_param("lfo1_rate", 20.0);

        let mut p_b = SynthPatch::new("Patch B");
        p_b.set_param("osc1_wave", 2.0); // changed
        p_b.set_param("filter_cutoff", 50.0); // unchanged
        p_b.set_param("sub_osc_level", 80.0); // added in B
                                              // lfo1_rate is removed in B

        let diff = PatchDiff::compare(&p_a, &p_b, 0.001);
        assert!(!diff.identical);
        assert_eq!(diff.added, vec!["sub_osc_level"]);
        assert_eq!(diff.removed, vec!["lfo1_rate"]);
        assert_eq!(diff.unchanged_count, 1);
        assert_eq!(diff.modified.len(), 3); // 1 changed + 1 added + 1 removed records in diff details

        // Selective merge: merge only "sub_osc_level" from B into A
        let merged = selective_merge(&p_a, &p_b, &["sub_osc_level".to_string()]);
        assert_eq!(merged.get_param("sub_osc_level"), Some(80.0));
        assert_eq!(merged.get_param("osc1_wave"), Some(1.0)); // original A value preserved
        assert_eq!(merged.get_param("lfo1_rate"), Some(20.0)); // original A value preserved

        // Selective merge section
        let merged_section = selective_merge_section(&p_a, &p_b, "osc1_");
        assert_eq!(merged_section.get_param("osc1_wave"), Some(2.0));
    }

    #[test]
    fn test_lock_shielded_randomizer() {
        let mut randomizer = LockShieldedRandomizer::new();

        // Register descriptors
        let cutoff_desc = ParamDescriptor::new(
            "Filter Cutoff",
            "Filter",
            ParamAddress::cc7(74),
            ValueRange::midi_cc7(64.0),
        );
        let res_desc = ParamDescriptor::new(
            "Filter Resonance",
            "Filter",
            ParamAddress::cc7(71),
            ValueRange::midi_cc7(0.0),
        );
        let osc_desc = ParamDescriptor::new(
            "Osc 1 Wave",
            "Oscillator",
            ParamAddress::cc7(70),
            ValueRange::new(0.0, 3.0, 0.0, 1.0, "").expect("osc range"),
        );

        randomizer.register_descriptor(cutoff_desc);
        randomizer.register_descriptor(res_desc);
        randomizer.register_descriptor(osc_desc);

        // Lock Filter Cutoff
        randomizer.lock_param("filter_cutoff");

        let mut patch = SynthPatch::new("Init Sound");
        patch.set_param("filter_cutoff", 42.0);
        patch.set_param("filter_resonance", 10.0);
        patch.set_param("osc_1_wave", 0.0);
        patch.set_param("master_volume", 100.0); // system param locked by default

        // Global randomize with fixed seed
        let randomized = randomizer.randomize_global(&patch, 100.0, 100.0, Some(12345));

        // Locked cutoff and default-locked master_volume MUST NOT change
        assert_eq!(randomized.get_param("filter_cutoff"), Some(42.0));
        assert_eq!(randomized.get_param("master_volume"), Some(100.0));

        // Unlocked resonance should have mutated
        let new_res = randomized.get_param("filter_resonance").unwrap_or(0.0);
        assert!((new_res - 10.0).abs() > f64::EPSILON);

        // Gaussian drift test
        let drifted = randomizer.randomize_gaussian_drift(&patch, 10.0, 100.0, Some(54321));
        assert_eq!(drifted.get_param("filter_cutoff"), Some(42.0));
        assert_eq!(drifted.get_param("master_volume"), Some(100.0));
        let drifted_res = drifted.get_param("filter_resonance").unwrap_or(0.0);
        assert!((drifted_res - 10.0).abs() < 40.0); // within 10% drift bounds
    }

    #[test]
    fn test_command_history_multi_step_undo_redo() {
        let mut history = CommandHistory::new(5);
        let mut patch = SynthPatch::new("Bass 1");
        patch.set_param("filter_cutoff", 50.0);

        // Step 1: Change cutoff to 70
        let cmd1 = Box::new(SetParamCommand::new("filter_cutoff", 50.0, 70.0));
        history
            .push_and_apply(cmd1, &mut patch)
            .expect("cmd1 failed");
        assert_eq!(patch.get_param("filter_cutoff"), Some(70.0));
        assert!(history.can_undo());
        assert!(!history.can_redo());
        assert_eq!(history.cursor(), 1);

        // Step 2: Change cutoff to 90
        let cmd2 = Box::new(SetParamCommand::new("filter_cutoff", 70.0, 90.0));
        history
            .push_and_apply(cmd2, &mut patch)
            .expect("cmd2 failed");
        assert_eq!(patch.get_param("filter_cutoff"), Some(90.0));
        assert_eq!(history.cursor(), 2);

        // Step 3: Undo step 2
        let undo_desc = history.undo(&mut patch).expect("undo failed");
        assert!(undo_desc.is_some());
        assert_eq!(patch.get_param("filter_cutoff"), Some(70.0));
        assert_eq!(history.cursor(), 1);
        assert!(history.can_redo());

        // Step 4: Undo step 1
        history.undo(&mut patch).expect("undo 1 failed");
        assert_eq!(patch.get_param("filter_cutoff"), Some(50.0));
        assert_eq!(history.cursor(), 0);
        assert!(!history.can_undo());

        // Step 5: Redo step 1
        history.redo(&mut patch).expect("redo 1 failed");
        assert_eq!(patch.get_param("filter_cutoff"), Some(70.0));
        assert_eq!(history.cursor(), 1);

        // Step 6: Push new command while in history branch (truncates redo branch)
        let cmd3 = Box::new(SetParamCommand::new("filter_cutoff", 70.0, 110.0));
        history
            .push_and_apply(cmd3, &mut patch)
            .expect("cmd3 failed");
        assert_eq!(patch.get_param("filter_cutoff"), Some(110.0));
        assert_eq!(history.cursor(), 2);
        assert_eq!(history.len(), 2);
        assert!(!history.can_redo());
    }

    #[test]
    fn test_command_history_bounded_capacity() {
        let mut history = CommandHistory::new(3);
        let mut patch = SynthPatch::new("Capacity Test");

        for i in 1..=5 {
            let cmd = Box::new(SetParamCommand::new("val", (i - 1) as f64, i as f64));
            history
                .push_and_apply(cmd, &mut patch)
                .expect("push failed");
        }

        assert_eq!(history.len(), 3);
        assert_eq!(history.cursor(), 3);

        // Can undo at most 3 times
        assert!(history.undo(&mut patch).unwrap().is_some());
        assert!(history.undo(&mut patch).unwrap().is_some());
        assert!(history.undo(&mut patch).unwrap().is_some());
        assert!(!history.can_undo());
    }

    #[test]
    fn test_four_synth_profiles_in_synth_core() {
        // 1. nkEditor / Korg nanoKONTROL2
        let nano = DeviceSynthesizerProfile::korg_nanokontrol2();
        assert_eq!(nano.id, "korg_nanokontrol2");
        assert_eq!(nano.category, DeviceCategory::ControllerSurface);
        assert_eq!(nano.controller_controls.len(), 51);

        // 2. ASM Hydrasynth
        let hydra = DeviceSynthesizerProfile::asm_hydrasynth();
        assert_eq!(hydra.id, "asm_hydrasynth");
        assert_eq!(hydra.category, DeviceCategory::Synthesizer);
        assert!(hydra.parameters.len() >= 65);

        // 3. BS2 and TD-3-MD
        let bs2 = DeviceSynthesizerProfile::novation_bass_station_2();
        assert_eq!(bs2.id, "novation_bass_station_2");
        assert!(bs2.parameters.len() >= 45);

        let td3 = DeviceSynthesizerProfile::behringer_td3_md();
        assert_eq!(td3.id, "behringer_td3_md");
        assert_eq!(td3.parameters.len(), 19);

        // 4. Clavia Nord Drum 2
        let nd2 = DeviceSynthesizerProfile::clavia_nord_drum_2();
        assert_eq!(nd2.id, "clavia_nord_drum_2");
        assert_eq!(nd2.category, DeviceCategory::Synthesizer);
        assert_eq!(nd2.parameters.len(), 202); // 186 voice params (31 x 6) + 16 global kit params
        assert_eq!(nd2.controller_controls.len(), 8); // 6 channel buttons + main dial + trig button

        // 5. Novation Peak
        let peak = DeviceSynthesizerProfile::novation_peak();
        assert_eq!(peak.id, "novation_peak");
        assert_eq!(peak.category, DeviceCategory::Synthesizer);
        assert_eq!(peak.parameters.len(), 74);
        assert_eq!(peak.controller_controls.len(), 8);
    }

    #[test]
    fn test_safe_sweep_bounds_parity_with_konduktiva_peak() {
        assert_eq!(
            safe_sweep_range("Filter resonance"),
            SafeSweepBounds { lo: 0, hi: 62 }
        );
        assert_eq!(
            safe_sweep_range("Distortion level"),
            SafeSweepBounds { lo: 0, hi: 48 }
        );
        assert_eq!(
            safe_sweep_range("Filter post-drive"),
            SafeSweepBounds { lo: 0, hi: 50 }
        );
        assert_eq!(
            safe_sweep_range("Delay feedback"),
            SafeSweepBounds { lo: 0, hi: 70 }
        );
        assert_eq!(
            safe_sweep_range("Chorus feedback"),
            SafeSweepBounds { lo: 0, hi: 70 }
        );
        assert_eq!(
            safe_sweep_range("Reverb level"),
            SafeSweepBounds { lo: 0, hi: 90 }
        );
        assert_eq!(
            safe_sweep_range("Filter cutoff"),
            SafeSweepBounds { lo: 0, hi: 127 }
        );
    }

    #[test]
    fn test_nord_drum_2_synth_patch_crud_and_preset_library() {
        let profile = DeviceSynthesizerProfile::clavia_nord_drum_2();
        let mut patch = profile.create_default_patch("80s Electro Kit");

        assert_eq!(patch.meta.name, "80s Electro Kit");
        assert_eq!(patch.parameters.len(), 202);

        // Verify Voice 1 and Voice 6 parameters initialized from profile
        assert_eq!(patch.get_param("voice_1_tone_wave"), Some(0.0));
        assert_eq!(patch.get_param("voice_1_choke_group"), Some(0.0));
        assert_eq!(patch.get_param("voice_6_echo_time_msb"), Some(64.0));
        assert_eq!(patch.get_param("global_channel"), Some(10.0));
        assert_eq!(patch.get_param("global_focus_cc"), Some(70.0));
        assert_eq!(patch.get_param("voice_1_trigger_note"), Some(60.0));

        // Update patch metadata
        patch.meta.category = "Percussion".to_string();
        patch.meta.author = Some("Renick Bell".to_string());
        patch.meta.bank = "A".to_string();
        patch.meta.slot = 12;
        patch.meta.tags = vec!["electro".to_string(), "6_voice".to_string(), "nord".to_string()];

        // Store into PresetLibrary
        let mut lib = PresetLibrary::new();
        lib.add_patch(patch.clone()).expect("Add patch failed");
        assert_eq!(lib.len(), 1);

        // Search by category, query, and tag
        let by_cat = lib.filter_category("Percussion");
        assert_eq!(by_cat.len(), 1);

        let search_results = lib.search("electro");
        assert_eq!(search_results.len(), 1);

        let by_tag = lib.filter_tag("nord");
        assert_eq!(by_tag.len(), 1);

        // JSON serialization round-trip
        let json_str = patch.to_json().expect("Patch JSON serialization failed");
        let restored = SynthPatch::from_json(&json_str).expect("Patch JSON deserialization failed");

        assert_eq!(restored.meta.name, "80s Electro Kit");
        assert_eq!(restored.meta.category, "Percussion");
        assert_eq!(restored.meta.bank, "A");
        assert_eq!(restored.meta.slot, 12);
        assert_eq!(restored.parameters.len(), 202);
        assert_eq!(restored.get_param("voice_1_choke_group"), Some(0.0));
        assert_eq!(restored.get_param("global_channel"), Some(10.0));
    }

    #[test]
    fn test_nord_drum_2_patch_diff_and_selective_merge() {
        let profile = DeviceSynthesizerProfile::clavia_nord_drum_2();
        let kit_a = profile.create_default_patch("Kit Alpha");
        let mut kit_b = profile.create_default_patch("Kit Beta");

        // Mutate Voice 1 Tone Decay, Voice 1 Choke Group, Voice 4 Distortion, and Global Channel in Kit B
        kit_b.set_param("voice_1_tone_decay", 110.0);
        kit_b.set_param("voice_1_choke_group", 1.0);
        kit_b.set_param("voice_4_distortion_amount", 95.0);
        kit_b.set_param("global_channel", 11.0);

        let diff = PatchDiff::compare(&kit_a, &kit_b, 0.001);
        assert_eq!(diff.total_differences(), 4);
        assert!(diff.modified.iter().any(|d| d.key == "voice_1_tone_decay"));
        assert!(diff.modified.iter().any(|d| d.key == "voice_1_choke_group"));
        assert!(diff.modified.iter().any(|d| d.key == "voice_4_distortion_amount"));
        assert!(diff.modified.iter().any(|d| d.key == "global_channel"));

        // Selectively merge Voice 1 from Kit B into Kit A
        let merged = selective_merge_matching(&kit_a, &kit_b, |key| key.starts_with("voice_1_"));
        assert_eq!(merged.get_param("voice_1_tone_decay"), Some(110.0));
        assert_eq!(merged.get_param("voice_1_choke_group"), Some(1.0));
        // Other modifications remain as Kit A defaults
        assert_eq!(merged.get_param("voice_4_distortion_amount"), Some(0.0));
        assert_eq!(merged.get_param("global_channel"), Some(10.0));
    }

    #[test]
    fn test_nord_drum_2_lock_shielded_randomizer() {
        let profile = DeviceSynthesizerProfile::clavia_nord_drum_2();
        let mut randomizer = LockShieldedRandomizer::new();

        // Register all parameters from the Nord Drum 2 profile
        for p in &profile.parameters {
            randomizer.register_descriptor(p.clone());
        }

        // Lock all Voice 1 kick drum parameters plus global communication parameters
        randomizer.lock_section("Voice 1 Tone");
        randomizer.lock_param("voice_1_choke_group");
        randomizer.lock_param("global_channel");
        randomizer.lock_param("global_focus_cc");

        assert!(randomizer.is_shielded("voice_1_tone_wave"));
        assert!(randomizer.is_shielded("voice_1_tone_decay"));
        assert!(randomizer.is_shielded("voice_1_choke_group"));
        assert!(randomizer.is_shielded("global_channel"));
        assert!(!randomizer.is_shielded("voice_2_noise_filter_freq"));

        let base_patch = profile.create_default_patch("Locked Base Kit");
        let randomized = randomizer.randomize_global(&base_patch, 100.0, 100.0, Some(99999));

        // Locked parameters MUST remain strictly unchanged
        assert_eq!(
            randomized.get_param("voice_1_tone_wave"),
            base_patch.get_param("voice_1_tone_wave")
        );
        assert_eq!(
            randomized.get_param("voice_1_tone_decay"),
            base_patch.get_param("voice_1_tone_decay")
        );
        assert_eq!(
            randomized.get_param("voice_1_choke_group"),
            base_patch.get_param("voice_1_choke_group")
        );
        assert_eq!(
            randomized.get_param("global_channel"),
            base_patch.get_param("global_channel")
        );

        // Unlocked Voice 2 parameter should have changed
        let new_v2_decay = randomized.get_param("voice_2_tone_decay").unwrap_or(0.0);
        let orig_v2_decay = base_patch.get_param("voice_2_tone_decay").unwrap_or(0.0);
        assert!((new_v2_decay - orig_v2_decay).abs() > f64::EPSILON);
    }

    #[test]
    fn test_nord_drum_2_command_history() {
        let profile = DeviceSynthesizerProfile::clavia_nord_drum_2();
        let mut patch = profile.create_default_patch("History Kit");
        let mut history = CommandHistory::new(10);

        // 1. Single parameter change on Voice 1 Choke Group (0.0 -> 2.0)
        let cmd1 = Box::new(SetParamCommand::new("voice_1_choke_group", 0.0, 2.0));
        history
            .push_and_apply(cmd1, &mut patch)
            .expect("Command 1 failed");
        assert_eq!(patch.get_param("voice_1_choke_group"), Some(2.0));
        assert_eq!(history.cursor(), 1);

        // 2. Batch parameter change across Voice 3 and Voice 6
        let batch_changes = vec![
            ("voice_3_mix_balance".to_string(), 64.0, 100.0),
            ("voice_6_echo_feedback".to_string(), 0.0, 24.0),
        ];
        let cmd2 = Box::new(BatchSetParamsCommand::new(
            batch_changes,
            "Edit V3 Mix & V6 Echo",
        ));
        history
            .push_and_apply(cmd2, &mut patch)
            .expect("Command 2 failed");
        assert_eq!(patch.get_param("voice_3_mix_balance"), Some(100.0));
        assert_eq!(patch.get_param("voice_6_echo_feedback"), Some(24.0));
        assert_eq!(history.cursor(), 2);

        // 3. Undo batch command
        let desc = history.undo(&mut patch).expect("Undo batch failed");
        assert_eq!(desc.as_deref(), Some("Edit V3 Mix & V6 Echo"));
        assert_eq!(patch.get_param("voice_3_mix_balance"), Some(64.0));
        assert_eq!(patch.get_param("voice_6_echo_feedback"), Some(0.0));
        assert_eq!(patch.get_param("voice_1_choke_group"), Some(2.0));

        // 4. Undo single parameter command
        history.undo(&mut patch).expect("Undo single failed");
        assert_eq!(patch.get_param("voice_1_choke_group"), Some(0.0));
        assert!(!history.can_undo());

        // 5. Redo both
        history.redo(&mut patch).expect("Redo 1 failed");
        assert_eq!(patch.get_param("voice_1_choke_group"), Some(2.0));
        history.redo(&mut patch).expect("Redo 2 failed");
        assert_eq!(patch.get_param("voice_3_mix_balance"), Some(100.0));
        assert_eq!(patch.get_param("voice_6_echo_feedback"), Some(24.0));
    }
}
