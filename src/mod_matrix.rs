// SPDX-License-Identifier: GPL-3.0-or-later
//! Novation Peak 16-Slot Dual-Source Modulation Matrix (`src/mod_matrix.rs`).
//!
//! Each of the 16 modulation slots on the Novation Peak multiplies two selectable
//! modulation sources (`source_1 * source_2`, where `Direct` acts as unity `1.0`)
//! and routes the product to a target `ModDestination` with bipolar depth (`-64..=+63`).
//! All 16 slots are stored directly in `synth_core::SynthPatch` parameters
//! (`mod_slot_{1..16}_{src1,src2,depth,dst}`) and Undo/Redo operations push
//! `synth_core::BatchSetParamsCommand` directly onto `synth_core::CommandHistory`.

use serde::{Deserialize, Serialize};
use synth_core::{BatchSetParamsCommand, CommandHistory, SynthError, SynthPatch};

/// Number of modulation matrix slots per patch on the Novation Peak (`16`).
pub const MOD_MATRIX_SLOT_COUNT: usize = 16;

/// The 17 hardware modulation sources available in each Peak modulation slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum ModSource {
    Off = 0,
    Direct = 1,
    ModWheel = 2,
    Aftertouch = 3,
    ExpressionPedal = 4,
    Velocity = 5,
    KeyboardTracking = 6,
    Lfo1Unipolar = 7,
    Lfo1Bipolar = 8,
    Lfo2Unipolar = 9,
    Lfo2Bipolar = 10,
    AmpEnv = 11,
    ModEnv1 = 12,
    ModEnv2 = 13,
    Animate1 = 14,
    Animate2 = 15,
    CvModIn = 16,
}

impl ModSource {
    /// Returns all 17 `ModSource` variants in hardware index order.
    pub const fn all() -> [Self; 17] {
        [
            Self::Off,
            Self::Direct,
            Self::ModWheel,
            Self::Aftertouch,
            Self::ExpressionPedal,
            Self::Velocity,
            Self::KeyboardTracking,
            Self::Lfo1Unipolar,
            Self::Lfo1Bipolar,
            Self::Lfo2Unipolar,
            Self::Lfo2Bipolar,
            Self::AmpEnv,
            Self::ModEnv1,
            Self::ModEnv2,
            Self::Animate1,
            Self::Animate2,
            Self::CvModIn,
        ]
    }

    /// Converts a numeric index (`0..=16`) into a `ModSource`.
    pub const fn from_u8(val: u8) -> Self {
        match val {
            1 => Self::Direct,
            2 => Self::ModWheel,
            3 => Self::Aftertouch,
            4 => Self::ExpressionPedal,
            5 => Self::Velocity,
            6 => Self::KeyboardTracking,
            7 => Self::Lfo1Unipolar,
            8 => Self::Lfo1Bipolar,
            9 => Self::Lfo2Unipolar,
            10 => Self::Lfo2Bipolar,
            11 => Self::AmpEnv,
            12 => Self::ModEnv1,
            13 => Self::ModEnv2,
            14 => Self::Animate1,
            15 => Self::Animate2,
            16 => Self::CvModIn,
            _ => Self::Off,
        }
    }

    /// Human-readable display label matching the Peak OLED screen.
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Direct => "Direct (Unity)",
            Self::ModWheel => "Mod Wheel",
            Self::Aftertouch => "Aftertouch",
            Self::ExpressionPedal => "Expr Pedal",
            Self::Velocity => "Velocity",
            Self::KeyboardTracking => "Key Tracking",
            Self::Lfo1Unipolar => "LFO 1 (+)",
            Self::Lfo1Bipolar => "LFO 1 (+/-)",
            Self::Lfo2Unipolar => "LFO 2 (+)",
            Self::Lfo2Bipolar => "LFO 2 (+/-)",
            Self::AmpEnv => "Amp Env",
            Self::ModEnv1 => "Mod Env 1",
            Self::ModEnv2 => "Mod Env 2",
            Self::Animate1 => "Animate 1",
            Self::Animate2 => "Animate 2",
            Self::CvModIn => "CV Mod In",
        }
    }
}

/// The 37 hardware modulation destinations available in each Peak modulation slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum ModDestination {
    Off = 0,
    Osc123Pitch = 1,
    Osc1Pitch = 2,
    Osc2Pitch = 3,
    Osc3Pitch = 4,
    Osc1VSync = 5,
    Osc2VSync = 6,
    Osc3VSync = 7,
    Osc1Shape = 8,
    Osc2Shape = 9,
    Osc3Shape = 10,
    Osc1Level = 11,
    Osc2Level = 12,
    Osc3Level = 13,
    RingModLevel = 14,
    NoiseLevel = 15,
    FilterPostDrive = 16,
    FilterOverdrive = 17,
    FilterCutoff = 18,
    FilterResonance = 19,
    Lfo1Rate = 20,
    Lfo2Rate = 21,
    AmpEnvAttack = 22,
    AmpEnvDecay = 23,
    AmpEnvRelease = 24,
    ModEnv1Attack = 25,
    ModEnv1Decay = 26,
    ModEnv1Release = 27,
    ModEnv2Attack = 28,
    ModEnv2Decay = 29,
    ModEnv2Release = 30,
    VcaLevel = 31,
    Pan = 32,
    DistortionLevel = 33,
    ChorusLevel = 34,
    DelayLevel = 35,
    ReverbLevel = 36,
}

impl ModDestination {
    /// Returns all 37 `ModDestination` variants in hardware index order.
    pub const fn all() -> [Self; 37] {
        [
            Self::Off,
            Self::Osc123Pitch,
            Self::Osc1Pitch,
            Self::Osc2Pitch,
            Self::Osc3Pitch,
            Self::Osc1VSync,
            Self::Osc2VSync,
            Self::Osc3VSync,
            Self::Osc1Shape,
            Self::Osc2Shape,
            Self::Osc3Shape,
            Self::Osc1Level,
            Self::Osc2Level,
            Self::Osc3Level,
            Self::RingModLevel,
            Self::NoiseLevel,
            Self::FilterPostDrive,
            Self::FilterOverdrive,
            Self::FilterCutoff,
            Self::FilterResonance,
            Self::Lfo1Rate,
            Self::Lfo2Rate,
            Self::AmpEnvAttack,
            Self::AmpEnvDecay,
            Self::AmpEnvRelease,
            Self::ModEnv1Attack,
            Self::ModEnv1Decay,
            Self::ModEnv1Release,
            Self::ModEnv2Attack,
            Self::ModEnv2Decay,
            Self::ModEnv2Release,
            Self::VcaLevel,
            Self::Pan,
            Self::DistortionLevel,
            Self::ChorusLevel,
            Self::DelayLevel,
            Self::ReverbLevel,
        ]
    }

    /// Converts a numeric index (`0..=36`) into a `ModDestination`.
    pub const fn from_u8(val: u8) -> Self {
        match val {
            1 => Self::Osc123Pitch,
            2 => Self::Osc1Pitch,
            3 => Self::Osc2Pitch,
            4 => Self::Osc3Pitch,
            5 => Self::Osc1VSync,
            6 => Self::Osc2VSync,
            7 => Self::Osc3VSync,
            8 => Self::Osc1Shape,
            9 => Self::Osc2Shape,
            10 => Self::Osc3Shape,
            11 => Self::Osc1Level,
            12 => Self::Osc2Level,
            13 => Self::Osc3Level,
            14 => Self::RingModLevel,
            15 => Self::NoiseLevel,
            16 => Self::FilterPostDrive,
            17 => Self::FilterOverdrive,
            18 => Self::FilterCutoff,
            19 => Self::FilterResonance,
            20 => Self::Lfo1Rate,
            21 => Self::Lfo2Rate,
            22 => Self::AmpEnvAttack,
            23 => Self::AmpEnvDecay,
            24 => Self::AmpEnvRelease,
            25 => Self::ModEnv1Attack,
            26 => Self::ModEnv1Decay,
            27 => Self::ModEnv1Release,
            28 => Self::ModEnv2Attack,
            29 => Self::ModEnv2Decay,
            30 => Self::ModEnv2Release,
            31 => Self::VcaLevel,
            32 => Self::Pan,
            33 => Self::DistortionLevel,
            34 => Self::ChorusLevel,
            35 => Self::DelayLevel,
            36 => Self::ReverbLevel,
            _ => Self::Off,
        }
    }

    /// Human-readable display label matching the Peak OLED screen.
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Osc123Pitch => "Osc 1+2+3 Pitch",
            Self::Osc1Pitch => "Osc 1 Pitch",
            Self::Osc2Pitch => "Osc 2 Pitch",
            Self::Osc3Pitch => "Osc 3 Pitch",
            Self::Osc1VSync => "Osc 1 VSync",
            Self::Osc2VSync => "Osc 2 VSync",
            Self::Osc3VSync => "Osc 3 VSync",
            Self::Osc1Shape => "Osc 1 Shape",
            Self::Osc2Shape => "Osc 2 Shape",
            Self::Osc3Shape => "Osc 3 Shape",
            Self::Osc1Level => "Osc 1 Mix Level",
            Self::Osc2Level => "Osc 2 Mix Level",
            Self::Osc3Level => "Osc 3 Mix Level",
            Self::RingModLevel => "Ring Mod Level",
            Self::NoiseLevel => "Noise Level",
            Self::FilterPostDrive => "Filter Post-Drive",
            Self::FilterOverdrive => "Filter Overdrive",
            Self::FilterCutoff => "Filter Cutoff",
            Self::FilterResonance => "Filter Resonance",
            Self::Lfo1Rate => "LFO 1 Rate",
            Self::Lfo2Rate => "LFO 2 Rate",
            Self::AmpEnvAttack => "Amp Env Attack",
            Self::AmpEnvDecay => "Amp Env Decay",
            Self::AmpEnvRelease => "Amp Env Release",
            Self::ModEnv1Attack => "Mod Env 1 Attack",
            Self::ModEnv1Decay => "Mod Env 1 Decay",
            Self::ModEnv1Release => "Mod Env 1 Release",
            Self::ModEnv2Attack => "Mod Env 2 Attack",
            Self::ModEnv2Decay => "Mod Env 2 Decay",
            Self::ModEnv2Release => "Mod Env 2 Release",
            Self::VcaLevel => "VCA Gain",
            Self::Pan => "Stereo Pan",
            Self::DistortionLevel => "Distortion Level",
            Self::ChorusLevel => "Chorus Level",
            Self::DelayLevel => "Delay Level",
            Self::ReverbLevel => "Reverb Level",
        }
    }
}

/// A single slot (`1..=16`) in the Novation Peak 16-Slot Modulation Matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModSlot {
    /// 1-based slot index (`1..=16`).
    pub slot_index: u8,
    /// Primary modulation source (`ModSource`).
    pub source_1: ModSource,
    /// Secondary multiplier modulation source (`ModSource::Direct` for single-source routing).
    pub source_2: ModSource,
    /// Bipolar modulation depth (`-64..=63`).
    pub depth: i8,
    /// Target modulation destination (`ModDestination`).
    pub destination: ModDestination,
}

impl ModSlot {
    /// Creates an empty (inactive) modulation slot for `slot_index` (`1..=16`).
    #[must_use]
    pub fn empty(slot_index: u8) -> Self {
        Self {
            slot_index: slot_index.clamp(1, 16),
            source_1: ModSource::Off,
            source_2: ModSource::Direct,
            depth: 0,
            destination: ModDestination::Off,
        }
    }

    /// Creates a configured modulation slot (`slot_index` `1..=16`, `depth` clamped to `-64..=63`).
    #[must_use]
    pub fn new(
        slot_index: u8,
        source_1: ModSource,
        source_2: ModSource,
        depth: i8,
        destination: ModDestination,
    ) -> Self {
        Self {
            slot_index: slot_index.clamp(1, 16),
            source_1,
            source_2,
            depth: depth.clamp(-64, 63),
            destination,
        }
    }

    /// Returns `true` if this modulation slot actively modulates a destination.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        !matches!(self.source_1, ModSource::Off)
            && !matches!(self.destination, ModDestination::Off)
            && self.depth != 0
    }
}

/// Reads a single `ModSlot` (`1..=16`) directly from a `synth_core::SynthPatch`.
#[must_use]
pub fn read_slot_from_patch(patch: &SynthPatch, slot_index: u8) -> ModSlot {
    let idx = slot_index.clamp(1, 16);
    let src1 = patch
        .get_param(&format!("mod_slot_{idx}_src1"))
        .unwrap_or(0.0)
        .round()
        .clamp(0.0, 16.0) as u8;
    let src2 = patch
        .get_param(&format!("mod_slot_{idx}_src2"))
        .unwrap_or(1.0)
        .round()
        .clamp(0.0, 16.0) as u8;
    let depth = patch
        .get_param(&format!("mod_slot_{idx}_depth"))
        .unwrap_or(0.0)
        .round()
        .clamp(-64.0, 63.0) as i8;
    let dst = patch
        .get_param(&format!("mod_slot_{idx}_dst"))
        .unwrap_or(0.0)
        .round()
        .clamp(0.0, 36.0) as u8;

    ModSlot {
        slot_index: idx,
        source_1: ModSource::from_u8(src1),
        source_2: ModSource::from_u8(src2),
        depth,
        destination: ModDestination::from_u8(dst),
    }
}

/// Reads all 16 modulation matrix slots directly from a `synth_core::SynthPatch`.
#[must_use]
pub fn read_all_slots_from_patch(patch: &SynthPatch) -> [ModSlot; MOD_MATRIX_SLOT_COUNT] {
    std::array::from_fn(|i| read_slot_from_patch(patch, (i + 1) as u8))
}

/// Writes a single `ModSlot` directly into a `synth_core::SynthPatch`.
pub fn write_slot_to_patch(patch: &mut SynthPatch, slot: &ModSlot) {
    let idx = slot.slot_index.clamp(1, 16);
    patch.set_param(format!("mod_slot_{idx}_src1"), slot.source_1 as u8 as f64);
    patch.set_param(format!("mod_slot_{idx}_src2"), slot.source_2 as u8 as f64);
    patch.set_param(
        format!("mod_slot_{idx}_depth"),
        slot.depth.clamp(-64, 63) as f64,
    );
    patch.set_param(
        format!("mod_slot_{idx}_dst"),
        slot.destination as u8 as f64,
    );
}

/// Applies a `ModSlot` edit to a `synth_core::SynthPatch` through `synth_core::CommandHistory`
/// using `BatchSetParamsCommand` so the entire slot change can be undone/redone atomically.
pub fn apply_slot_with_history(
    history: &mut CommandHistory,
    patch: &mut SynthPatch,
    slot: &ModSlot,
) -> Result<(), SynthError> {
    let idx = slot.slot_index.clamp(1, 16);
    let prev = read_slot_from_patch(patch, idx);
    let changes = vec![
        (
            format!("mod_slot_{idx}_src1"),
            prev.source_1 as u8 as f64,
            slot.source_1 as u8 as f64,
        ),
        (
            format!("mod_slot_{idx}_src2"),
            prev.source_2 as u8 as f64,
            slot.source_2 as u8 as f64,
        ),
        (
            format!("mod_slot_{idx}_depth"),
            prev.depth as f64,
            slot.depth.clamp(-64, 63) as f64,
        ),
        (
            format!("mod_slot_{idx}_dst"),
            prev.destination as u8 as f64,
            slot.destination as u8 as f64,
        ),
    ];
    let desc = format!(
        "Mod Slot {}: {} x {} -> {} ({:+})",
        idx,
        slot.source_1.display_name(),
        slot.source_2.display_name(),
        slot.destination.display_name(),
        slot.depth
    );
    history.push_and_apply(Box::new(BatchSetParamsCommand::new(changes, desc)), patch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use synth_core::DeviceSynthesizerProfile;

    #[test]
    fn test_mod_matrix_slot_read_write_and_command_history_undo_redo() {
        let profile = DeviceSynthesizerProfile::novation_peak();
        let mut patch = profile.create_default_patch("Mod Matrix Test");
        let mut history = CommandHistory::new(16);

        let slot1 = ModSlot::new(
            1,
            ModSource::Animate1,
            ModSource::Direct,
            42,
            ModDestination::FilterCutoff,
        );
        apply_slot_with_history(&mut history, &mut patch, &slot1).expect("apply slot 1");

        let read1 = read_slot_from_patch(&mut patch, 1);
        assert_eq!(read1, slot1);
        assert!(read1.is_active());

        // Undo restores initial empty slot
        history.undo(&mut patch).expect("undo slot 1");
        let undone = read_slot_from_patch(&patch, 1);
        assert!(!undone.is_active());
        assert_eq!(undone.depth, 0);

        // Redo reapplies slot 1
        history.redo(&mut patch).expect("redo slot 1");
        assert_eq!(read_slot_from_patch(&patch, 1), slot1);
    }
}
