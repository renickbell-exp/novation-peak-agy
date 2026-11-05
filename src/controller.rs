// SPDX-License-Identifier: GPL-3.0-or-later
//! Korg nanoKONTROL2 (`0944:0117`, `hw:2,0,0`) 3-Bank Tactile Controller Mapper (`src/controller.rs`).
//!
//! Directly reuses `synth_core::ControllerSurfaceMapping::nanokontrol2_novation_peak()`,
//! `synth_core::SoftTakeoverState`, `synth_core::CommandHistory`, and `synth_core::SynthPatch`.

use crate::midi::PeakHardwareEngine;
use crate::params::PeakMidiCcMapSpec;
use std::collections::BTreeMap;
use std::time::Instant;
use synth_core::{
    ControllerSurfaceMapping, SetParamCommand, SoftTakeoverMode, SoftTakeoverResult,
    SoftTakeoverState, SurfaceBankMapping, SurfaceControlBinding, SynthPatch,
};

/// Korg nanoKONTROL2 Track Left button CC (`58`) for stepping down a bank.
pub const NK2_TRACK_LEFT_CC: u8 = 58;
/// Korg nanoKONTROL2 Track Right button CC (`59`) for stepping up a bank.
pub const NK2_TRACK_RIGHT_CC: u8 = 59;
/// Korg nanoKONTROL2 Play button CC (`41`).
pub const NK2_PLAY_CC: u8 = 41;
/// Korg nanoKONTROL2 Stop button CC (`42`).
pub const NK2_STOP_CC: u8 = 42;

/// 3-Bank Korg nanoKONTROL2 controller surface manager for the Novation Peak.
#[derive(Debug, Clone)]
pub struct NanoKontrol2PeakController {
    pub mapping: ControllerSurfaceMapping,
    pub takeover_mode: SoftTakeoverMode,
    pub takeover_states: BTreeMap<u8, SoftTakeoverState>,
    pub solo_leds: [bool; 8],
    pub mute_leds: [bool; 8],
    pub rec_leds: [bool; 8],
    pub transport_playing: bool,
    pub konduktiva_player_rates: [f64; 8],
    cc_spec: PeakMidiCcMapSpec,
}

impl Default for NanoKontrol2PeakController {
    fn default() -> Self {
        let patch = synth_core::DeviceSynthesizerProfile::novation_peak()
            .create_default_patch("Init Patch");
        Self::new(&patch, SoftTakeoverMode::Catch)
    }
}

impl NanoKontrol2PeakController {
    /// Creates a new 3-bank nanoKONTROL2 controller initialized against `patch`.
    #[must_use]
    pub fn new(patch: &SynthPatch, takeover_mode: SoftTakeoverMode) -> Self {
        let mapping = ControllerSurfaceMapping::nanokontrol2_novation_peak();
        let mut ctrl = Self {
            mapping,
            takeover_mode,
            takeover_states: BTreeMap::new(),
            solo_leds: [false; 8],
            mute_leds: [false; 8],
            rec_leds: [false; 8],
            transport_playing: false,
            konduktiva_player_rates: [0.5, 0.5, 2.0, 4.0, 4.0, 2.0, 2.0, 1.0],
            cc_spec: PeakMidiCcMapSpec::load_embedded(),
        };
        ctrl.sync_bank_targets_from_patch(patch);
        ctrl
    }

    /// Returns the currently active `SurfaceBankMapping` (`1..=3`).
    #[must_use]
    pub fn active_bank_mapping(&self) -> &SurfaceBankMapping {
        let idx = (self.mapping.active_bank.saturating_sub(1) as usize)
            .min(self.mapping.banks.len().saturating_sub(1));
        &self.mapping.banks[idx]
    }

    /// Switches the active controller bank (`1..=3`) and resets soft-takeover catch states
    /// against the new bank's target parameters in `patch`.
    pub fn switch_bank(&mut self, bank_index_1_based: u32, patch: &SynthPatch) {
        self.mapping.active_bank = bank_index_1_based.clamp(1, 3);
        self.sync_bank_targets_from_patch(patch);
    }

    /// Synchronizes `SoftTakeoverState` target values for all controls in the active bank.
    pub fn sync_bank_targets_from_patch(&mut self, patch: &SynthPatch) {
        let bank = self.active_bank_mapping().clone();
        for binding in &bank.controls {
            if let Some(ref param_key) = binding.target_param {
                let target_val = if param_key.starts_with("player_") {
                    64.0
                } else {
                    patch.get_param(param_key).unwrap_or(64.0)
                };
                if let Some(existing) = self.takeover_states.get_mut(&binding.hardware_cc) {
                    existing.reset_target(target_val);
                } else {
                    self.takeover_states.insert(
                        binding.hardware_cc,
                        SoftTakeoverState::new(self.takeover_mode, target_val).with_threshold(2.0),
                    );
                }
            }
        }
    }

    /// Looks up the active bank's binding for a given physical `hardware_cc`.
    #[must_use]
    pub fn binding_for_cc(&self, hardware_cc: u8) -> Option<&SurfaceControlBinding> {
        self.active_bank_mapping()
            .controls
            .iter()
            .find(|b| b.hardware_cc == hardware_cc)
    }

    /// Processes an incoming physical MIDI CC message from the Korg nanoKONTROL2 (`hw:2,0,0`),
    /// arbitrating soft takeover, updating `patch` via `synth_core::CommandHistory`, and
    /// scheduling outgoing Peak MIDI messages on `engine`.
    pub fn handle_surface_cc(
        &mut self,
        hardware_cc: u8,
        value: u8,
        patch: &mut SynthPatch,
        history: &mut synth_core::CommandHistory,
        engine: &mut PeakHardwareEngine,
        now: Instant,
    ) -> Option<SoftTakeoverResult> {
        // Bank switching via Track Left (CC 58) / Track Right (CC 59) on button press (value >= 64)
        if hardware_cc == NK2_TRACK_LEFT_CC && value >= 64 {
            let next_bank = self.mapping.active_bank.saturating_sub(1).max(1);
            self.switch_bank(next_bank, patch);
            return None;
        }
        if hardware_cc == NK2_TRACK_RIGHT_CC && value >= 64 {
            let next_bank = (self.mapping.active_bank + 1).min(3);
            self.switch_bank(next_bank, patch);
            return None;
        }

        // Transport Play (CC 41) / Stop & Panic (CC 42)
        if hardware_cc == NK2_PLAY_CC && value >= 64 {
            self.transport_playing = true;
            return None;
        }
        if hardware_cc == NK2_STOP_CC && value >= 64 {
            self.transport_playing = false;
            engine.panic(now);
            return None;
        }

        let binding = self.binding_for_cc(hardware_cc)?.clone();

        // Handle Animate 1 & 2 Hold toggles in Bank 3 (CC 32 / CC 33)
        if binding.action == "animate_1_toggle" && value >= 64 {
            self.solo_leds[0] = !self.solo_leds[0];
            let held = self.solo_leds[0];
            patch.set_param("animate_1_hold", if held { 127.0 } else { 0.0 });
            engine.send_animate_hold(1, held);
            return None;
        }
        if binding.action == "animate_2_toggle" && value >= 64 {
            self.solo_leds[1] = !self.solo_leds[1];
            let held = self.solo_leds[1];
            patch.set_param("animate_2_hold", if held { 127.0 } else { 0.0 });
            engine.send_animate_hold(2, held);
            return None;
        }

        // Handle Konduktiva player rate sliders in Bank 3 (CC 0..7)
        if binding.action == "konduktiva_player_rate" {
            let lane_idx = (hardware_cc as usize).min(7);
            let takeover = self
                .takeover_states
                .entry(hardware_cc)
                .or_insert_with(|| SoftTakeoverState::new(self.takeover_mode, 64.0));
            let res = takeover.update(value as f64);
            if res.updated {
                // Map 0..127 -> 0.25..8.0 beats/sec
                let rate = 0.25 + (res.value / 127.0) * 7.75;
                self.konduktiva_player_rates[lane_idx] = rate;
            }
            return Some(res);
        }

        // Standard synth parameter knob/slider with soft takeover
        let param_key = binding.target_param.as_deref()?;
        let current_target = patch.get_param(param_key).unwrap_or(64.0);
        let takeover = self
            .takeover_states
            .entry(hardware_cc)
            .or_insert_with(|| SoftTakeoverState::new(self.takeover_mode, current_target));
        let res = takeover.update(value as f64);

        if res.updated {
            let new_val = res.value.round().clamp(0.0, 127.0);
            let _ = history.push_and_apply(
                Box::new(SetParamCommand::new(param_key, current_target, new_val)),
                patch,
            );
            if let Some(entry) = self.cc_spec.resolve_cc_by_name(param_key) {
                engine.schedule_named_cc(entry.cc, &entry.param, new_val as u8);
            }
        }

        Some(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use synth_core::{CommandHistory, DeviceSynthesizerProfile};

    #[test]
    fn test_nanokontrol2_three_banks_and_soft_takeover_catch() {
        let profile = DeviceSynthesizerProfile::novation_peak();
        let mut patch = profile.create_default_patch("Controller Test");
        patch.set_param("osc_1_coarse", 80.0);
        patch.set_param("filter_cutoff_frequency", 100.0);

        let mut history = CommandHistory::new(32);
        let mut engine = PeakHardwareEngine::default();
        let mut ctrl = NanoKontrol2PeakController::new(&patch, SoftTakeoverMode::Catch);
        let t0 = Instant::now();

        // Bank 1 Knob 1 (CC 16) is mapped to osc_1_coarse (target = 80.0)
        assert_eq!(ctrl.mapping.active_bank, 1);
        let res1 = ctrl
            .handle_surface_cc(16, 20, &mut patch, &mut history, &mut engine, t0)
            .expect("takeover result");
        assert!(!res1.is_caught);
        assert_eq!(patch.get_param("osc_1_coarse"), Some(80.0));

        // Cross target (20 -> 82) catches and updates patch + schedules CC 14
        let res2 = ctrl
            .handle_surface_cc(16, 82, &mut patch, &mut history, &mut engine, t0)
            .expect("takeover result");
        assert!(res2.is_caught);
        assert!(res2.updated);
        assert_eq!(patch.get_param("osc_1_coarse"), Some(82.0));

        // Switch to Bank 2 via Track Right (CC 59)
        ctrl.handle_surface_cc(NK2_TRACK_RIGHT_CC, 127, &mut patch, &mut history, &mut engine, t0);
        assert_eq!(ctrl.mapping.active_bank, 2);

        // In Bank 2, Knob 1 (CC 16) is Filter Cutoff (target = 100.0) -> must re-catch!
        let res_b2 = ctrl
            .handle_surface_cc(16, 50, &mut patch, &mut history, &mut engine, t0)
            .expect("bank 2 takeover");
        assert!(!res_b2.is_caught);
        assert_eq!(patch.get_param("filter_cutoff_frequency"), Some(100.0));

        let res_b2_catch = ctrl
            .handle_surface_cc(16, 102, &mut patch, &mut history, &mut engine, t0)
            .expect("bank 2 catch");
        assert!(res_b2_catch.is_caught);
        assert_eq!(patch.get_param("filter_cutoff_frequency"), Some(102.0));

        // Switch to Bank 3 via Track Right (CC 59) and toggle Animate 1 Hold (CC 32)
        ctrl.handle_surface_cc(NK2_TRACK_RIGHT_CC, 127, &mut patch, &mut history, &mut engine, t0);
        assert_eq!(ctrl.mapping.active_bank, 3);
        ctrl.handle_surface_cc(32, 127, &mut patch, &mut history, &mut engine, t0);
        assert!(ctrl.solo_leds[0]);
        assert_eq!(patch.get_param("animate_1_hold"), Some(127.0));
    }
}
