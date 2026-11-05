// SPDX-License-Identifier: GPL-3.0-or-later
//! `synth-midi-osc::routing`: Clavia Nord Drum 2 Dual MIDI Routing,
//! Non-Invasive ALSA Port Discovery, and Anti-Choke CC Thinning & Note-On Priority Scheduler.
//!
//! Provides the core routing and scheduling primitives required for real-time
//! control of the Clavia Nord Drum 2 physical/resonant modeling drum synthesizer.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::time::{Duration, Instant};
use synth_core::error::SynthError;

use crate::hardware::{
    scan_hardware_devices_with_roots, SupportedDeviceType,
};

// =============================================================================
// Constants & Specifications
// =============================================================================

/// Number of independent percussion voices on the Clavia Nord Drum 2.
pub const VOICE_COUNT: usize = 6;

/// Clavia manufacturer System Exclusive (SysEx) identifier.
pub const CLAVIA_MANUFACTURER_ID: u8 = 0x33;

/// Nord Drum 2 model identifier.
pub const NORD_DRUM_2_MODEL_ID: u8 = 0x12;

/// Default Global MIDI Channel for the Nord Drum 2 (1-based: Ch 10).
pub const DEFAULT_GLOBAL_CHANNEL: u8 = 10;

/// Global Channel Voice Focus MIDI Control Change (CC) number.
pub const GLOBAL_FOCUS_CC: u8 = 70;

/// Default MIDI trigger notes for Voices 1 through 6 on the Global Channel.
/// Voice 1 = 60 (C4), Voice 2 = 62 (D4), Voice 3 = 64 (E4),
/// Voice 4 = 65 (F4), Voice 5 = 67 (G4), Voice 6 = 69 (A4).
pub const DEFAULT_TRIGGER_NOTES: [u8; VOICE_COUNT] = [60, 62, 64, 65, 67, 69];

/// Default ALSA RawMIDI device path for the Behringer UMC1820 MIDI 1 interface.
pub const UMC1820_ALSA_DEVICE: &str = "hw:0,0,0";

/// Default ALSA RawMIDI device path for the Korg nanoKONTROL2 control surface.
pub const NANOKONTROL2_ALSA_DEVICE: &str = "hw:2,0,0";

/// Default ALSA RawMIDI device path for the Focusrite-Novation Peak USB MIDI 1 interface.
pub const PEAK_ALSA_DEVICE: &str = "hw:3,0,0";

/// Focusrite-Novation USB Vendor ID (`1235`).
pub const PEAK_USB_VENDOR: &str = "1235";

/// Novation Peak USB Product ID (`007e`).
pub const PEAK_USB_PRODUCT: &str = "007e";

/// Default ALSA Sequencer client ID for the Novation Peak (`28`).
pub const PEAK_SEQ_CLIENT: u32 = 28;

/// Default 1-based MIDI Receive Channel for the Novation Peak (`1`).
pub const PEAK_DEFAULT_CHANNEL: u8 = 1;

/// Maximum polyphonic voice count on the Novation Peak (`8` voices).
pub const PEAK_POLYPHONY: usize = 8;

/// Default hardware anti-choke rate thinning interval per (voice, CC) pair in milliseconds.
pub const DEFAULT_CC_THROTTLE_MS: u64 = 2;

// =============================================================================
// Safety & Control Modes
// =============================================================================

/// Operational safety mode for MIDI output transmission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MidiSafetyMode {
    /// Safe offline virtual loopback mode (default).
    /// Prevents any automated test or CLI run from transmitting unprompted MIDI
    /// messages to physical hardware interfaces.
    #[default]
    VirtualLoopback,
    /// Non-invasive inspection dry run mode.
    /// Inspects system hardware topology in read-only mode, but all outgoing MIDI
    /// traffic remains strictly directed to the virtual loopback sink.
    DryRun,
    /// Live hardware armed mode.
    /// Requires explicit operator authorization. Outgoing packets are sent to physical ALSA ports.
    LiveHardwareArmed,
}

impl MidiSafetyMode {
    /// Returns true if live hardware transmission is explicitly armed.
    #[must_use]
    pub const fn is_hardware_armed(&self) -> bool {
        matches!(self, Self::LiveHardwareArmed)
    }

    /// Returns true if the system is operating in a safe non-transmitting state.
    #[must_use]
    pub const fn is_safe(&self) -> bool {
        matches!(self, Self::VirtualLoopback | Self::DryRun)
    }

    /// Returns human-readable status description.
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::VirtualLoopback => "Virtual Loopback (Offline Safe)",
            Self::DryRun => "Dry-Run Inspection (Virtual Loopback Safe)",
            Self::LiveHardwareArmed => "Live Hardware Armed (hw:0,0,0)",
        }
    }
}

/// MIDI channel addressing mode for editing and triggering the 6 voices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MidiControlMode {
    /// Voices 1 through 6 are addressed on independent MIDI channels (Default: Ch 1..6).
    #[default]
    IndividualChannels,
    /// All voices are addressed on the Global Channel (default: Ch 10), prepending CC 70 on voice changes.
    GlobalFocus,
}

// =============================================================================
// MIDI Message Packets
// =============================================================================

/// Outgoing MIDI message packet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MidiPacket {
    /// Control Change message: `[0xBn | channel, cc, value]`. Channel is 0-indexed [0..15].
    ControlChange { channel: u8, cc: u8, value: u8 },
    /// Program Change message: `[0xCn | channel, program]`. Channel is 0-indexed [0..15].
    ProgramChange { channel: u8, program: u8 },
    /// Note On trigger message: `[0x9n | channel, note, velocity]`. Channel is 0-indexed [0..15].
    NoteOn { channel: u8, note: u8, velocity: u8 },
    /// Note Off trigger message: `[0x8n | channel, note, velocity]`. Channel is 0-indexed [0..15].
    NoteOff { channel: u8, note: u8, velocity: u8 },
    /// Raw System Exclusive frame: `[0xF0, ..., 0xF7]`.
    SysEx(Vec<u8>),
}

impl MidiPacket {
    /// Serializes packet to standard 31,250 baud MIDI wire bytes.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Self::ControlChange { channel, cc, value } => {
                vec![0xB0 | (channel & 0x0F), cc & 0x7F, value & 0x7F]
            }
            Self::ProgramChange { channel, program } => {
                vec![0xC0 | (channel & 0x0F), program & 0x7F]
            }
            Self::NoteOn { channel, note, velocity } => {
                vec![0x90 | (channel & 0x0F), note & 0x7F, velocity & 0x7F]
            }
            Self::NoteOff { channel, note, velocity } => {
                vec![0x80 | (channel & 0x0F), note & 0x7F, velocity & 0x7F]
            }
            Self::SysEx(raw) => raw.clone(),
        }
    }

    /// Returns true if this packet is a high-priority Note trigger event.
    #[must_use]
    pub const fn is_note(&self) -> bool {
        matches!(self, Self::NoteOn { .. } | Self::NoteOff { .. })
    }
}

// =============================================================================
// Dual MIDI Router
// =============================================================================

/// Dual MIDI Router supporting Individual Voice Channels (Ch 1..6) and
/// Global Channel (Ch 10) with automatic CC 70 Focus switching and configurable trigger notes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DualMidiRouter {
    pub control_mode: MidiControlMode,
    pub safety_mode: MidiSafetyMode,
    /// Global MIDI channel (1-based: 1..16, default 10).
    pub global_channel: u8,
    /// Per-voice individual MIDI channels (1-based: Some(1..16) or None if disabled).
    pub voice_channels: [Option<u8>; VOICE_COUNT],
    /// Per-voice trigger notes (0..127, default [60, 62, 64, 65, 67, 69]).
    pub trigger_notes: [u8; VOICE_COUNT],
    /// Current focused voice index on the hardware (1-based: 1..6).
    pub current_focus_voice: u8,
}

impl Default for DualMidiRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl DualMidiRouter {
    /// Creates a new dual MIDI router in safe virtual loopback mode.
    #[must_use]
    pub fn new() -> Self {
        Self {
            control_mode: MidiControlMode::IndividualChannels,
            safety_mode: MidiSafetyMode::VirtualLoopback,
            global_channel: DEFAULT_GLOBAL_CHANNEL,
            voice_channels: [Some(1), Some(2), Some(3), Some(4), Some(5), Some(6)],
            trigger_notes: DEFAULT_TRIGGER_NOTES,
            current_focus_voice: 1,
        }
    }

    /// Sets the control mode (IndividualChannels or GlobalFocus).
    #[must_use]
    pub fn with_control_mode(mut self, mode: MidiControlMode) -> Self {
        self.control_mode = mode;
        self
    }

    /// Sets the safety mode.
    #[must_use]
    pub fn with_safety_mode(mut self, mode: MidiSafetyMode) -> Self {
        self.safety_mode = mode;
        self
    }

    /// Sets the 1-based Global MIDI Channel (1..16).
    #[must_use]
    pub fn with_global_channel(mut self, channel: u8) -> Self {
        self.global_channel = channel.clamp(1, 16);
        self
    }

    /// Configures trigger note for a specific voice (1..6).
    pub fn set_trigger_note(&mut self, voice_index: u8, note: u8) -> Result<(), SynthError> {
        if voice_index < 1 || voice_index as usize > VOICE_COUNT {
            return Err(SynthError::ValidationError(format!(
                "Voice index {} out of range (1..{})",
                voice_index, VOICE_COUNT
            )));
        }
        self.trigger_notes[(voice_index - 1) as usize] = note.clamp(0, 127);
        Ok(())
    }

    /// Retrieves trigger note for a voice (1..6).
    #[must_use]
    pub fn get_trigger_note(&self, voice_index: u8) -> Option<u8> {
        if voice_index < 1 || voice_index as usize > VOICE_COUNT {
            None
        } else {
            Some(self.trigger_notes[(voice_index - 1) as usize])
        }
    }

    /// Configures the MIDI channel for an individual voice (1..6).
    pub fn set_voice_channel(
        &mut self,
        voice_index: u8,
        channel: Option<u8>,
    ) -> Result<(), SynthError> {
        if voice_index < 1 || voice_index as usize > VOICE_COUNT {
            return Err(SynthError::ValidationError(format!(
                "Voice index {} out of range (1..{})",
                voice_index, VOICE_COUNT
            )));
        }
        let clamped = channel.map(|c| c.clamp(1, 16));
        self.voice_channels[(voice_index - 1) as usize] = clamped;
        Ok(())
    }

    /// Retrieves the configured channel for a voice (1..6).
    #[must_use]
    pub fn get_voice_channel(&self, voice_index: u8) -> Option<u8> {
        if voice_index < 1 || voice_index as usize > VOICE_COUNT {
            None
        } else {
            self.voice_channels[(voice_index - 1) as usize]
        }
    }

    /// Explicitly switches hardware focus in GlobalFocus mode by emitting CC 70.
    pub fn switch_focus(&mut self, voice_index: u8) -> Option<MidiPacket> {
        let target = voice_index.clamp(1, 6);
        if self.control_mode == MidiControlMode::GlobalFocus && self.current_focus_voice != target {
            self.current_focus_voice = target;
            let wire_channel = self.global_channel.saturating_sub(1);
            Some(MidiPacket::ControlChange {
                channel: wire_channel,
                cc: GLOBAL_FOCUS_CC,
                value: target - 1,
            })
        } else {
            None
        }
    }

    /// Routes a parameter CC to the appropriate MIDI channel, prepending CC 70
    /// focus switching when operating in GlobalFocus mode.
    pub fn route_param_cc(&mut self, voice_index: u8, cc: u8, value: u8) -> Vec<MidiPacket> {
        if voice_index < 1 || voice_index as usize > VOICE_COUNT {
            return Vec::new();
        }

        let mut packets = Vec::with_capacity(2);
        match self.control_mode {
            MidiControlMode::IndividualChannels => {
                let voice_idx = (voice_index - 1) as usize;
                if let Some(ch) = self.voice_channels[voice_idx] {
                    packets.push(MidiPacket::ControlChange {
                        channel: ch.saturating_sub(1),
                        cc: cc & 0x7F,
                        value: value & 0x7F,
                    });
                }
            }
            MidiControlMode::GlobalFocus => {
                let target = voice_index;
                let wire_channel = self.global_channel.saturating_sub(1);
                if self.current_focus_voice != target {
                    // Prepend CC 70 Voice Focus
                    packets.push(MidiPacket::ControlChange {
                        channel: wire_channel,
                        cc: GLOBAL_FOCUS_CC,
                        value: target - 1,
                    });
                    self.current_focus_voice = target;
                }
                packets.push(MidiPacket::ControlChange {
                    channel: wire_channel,
                    cc: cc & 0x7F,
                    value: value & 0x7F,
                });
            }
        }
        packets
    }

    /// Routes a Note-On trigger message for a voice (1..6).
    #[must_use]
    pub fn route_note_on(&self, voice_index: u8, velocity: u8) -> Option<MidiPacket> {
        if voice_index < 1 || voice_index as usize > VOICE_COUNT {
            return None;
        }
        let voice_idx = (voice_index - 1) as usize;
        let vel = velocity.clamp(1, 127);

        match self.control_mode {
            MidiControlMode::IndividualChannels => {
                let ch = self.voice_channels[voice_idx]?;
                Some(MidiPacket::NoteOn {
                    channel: ch.saturating_sub(1),
                    note: self.trigger_notes[voice_idx],
                    velocity: vel,
                })
            }
            MidiControlMode::GlobalFocus => {
                let note = self.trigger_notes[voice_idx];
                Some(MidiPacket::NoteOn {
                    channel: self.global_channel.saturating_sub(1),
                    note,
                    velocity: vel,
                })
            }
        }
    }

    /// Routes a Note-Off trigger message for a voice (1..6).
    #[must_use]
    pub fn route_note_off(&self, voice_index: u8, velocity: u8) -> Option<MidiPacket> {
        if voice_index < 1 || voice_index as usize > VOICE_COUNT {
            return None;
        }
        let voice_idx = (voice_index - 1) as usize;
        let vel = velocity.clamp(0, 127);

        match self.control_mode {
            MidiControlMode::IndividualChannels => {
                let ch = self.voice_channels[voice_idx]?;
                Some(MidiPacket::NoteOff {
                    channel: ch.saturating_sub(1),
                    note: self.trigger_notes[voice_idx],
                    velocity: vel,
                })
            }
            MidiControlMode::GlobalFocus => {
                let note = self.trigger_notes[voice_idx];
                Some(MidiPacket::NoteOff {
                    channel: self.global_channel.saturating_sub(1),
                    note,
                    velocity: vel,
                })
            }
        }
    }
}

/// Generates the standard JSON Schema (Draft-07) for `DualMidiRouter`.
pub fn dual_midi_router_json_schema() -> serde_json::Value {
    serde_json::json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "DualMidiRouter",
        "description": "Dual MIDI router routing and safety mode configuration",
        "type": "object",
        "required": ["control_mode", "safety_mode", "global_channel", "voice_channels", "trigger_notes", "current_focus_voice"],
        "properties": {
            "control_mode": { "type": "string", "enum": ["IndividualChannels", "GlobalFocus"] },
            "safety_mode": { "type": "string", "enum": ["VirtualLoopback", "DryRun", "LiveHardwareArmed"] },
            "global_channel": { "type": "integer", "minimum": 1, "maximum": 16 },
            "voice_channels": {
                "type": "array",
                "items": { "type": ["integer", "null"] },
                "minItems": 6,
                "maxItems": 6
            },
            "trigger_notes": {
                "type": "array",
                "items": { "type": "integer", "minimum": 0, "maximum": 127 },
                "minItems": 6,
                "maxItems": 6
            },
            "current_focus_voice": { "type": "integer", "minimum": 1, "maximum": 6 }
        }
    })
}

/// Validates and parses JSON into a `DualMidiRouter`.
pub fn validate_dual_midi_router_json(
    json_str: &str,
) -> Result<DualMidiRouter, synth_core::SynthError> {
    serde_json::from_str(json_str)
        .map_err(|e| synth_core::SynthError::SerializationError(e.to_string()))
}

/// Exports MIDI and OSC schemas and canonical hardware profiles across the 4 synth projects.
pub fn export_midi_osc_schemas(dir: &std::path::Path) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(dir)?;
    if let Ok(json) = serde_json::to_string_pretty(&crate::hardware::stored_device_profile_json_schema()) {
        let _ = std::fs::write(dir.join("stored-device-profile.schema.json"), json);
    }
    if let Ok(json) = serde_json::to_string_pretty(&dual_midi_router_json_schema()) {
        let _ = std::fs::write(dir.join("dual-midi-router.schema.json"), json);
    }
    // 1. ASM Hydrasynth profile
    let hydra_hw = crate::hardware::StoredDeviceProfile {
        identity_key: "asm_hydrasynth_usb".to_string(),
        vendor_id: "1fc9".to_string(),
        product_id: "0103".to_string(),
        serial: Some("HYDRA-001".to_string()),
        card_name: "Hydrasynth Keyboard".to_string(),
        device_type: crate::hardware::SupportedDeviceType::AsmHydrasynth,
        custom_alias: Some("ASM Hydrasynth Polyphonic Wavetable".to_string()),
        last_known_card_index: Some(1),
        last_known_seq_address: Some((20, 0)),
        last_known_usb_path: Some("1-1.2:1.0".to_string()),
        reconnect_count: 1,
        is_active: false,
    };
    if let Ok(json) = serde_json::to_string_pretty(&hydra_hw) {
        let _ = std::fs::write(dir.join("asm-hydrasynth.hardware.json"), json);
    }
    // 2. nkEditor nanoKONTROL2 profile
    let nk2_hw = crate::hardware::StoredDeviceProfile {
        identity_key: "korg_nanokontrol2_surface".to_string(),
        vendor_id: "0944".to_string(),
        product_id: "0117".to_string(),
        serial: None,
        card_name: "nanoKONTROL2".to_string(),
        device_type: crate::hardware::SupportedDeviceType::KorgNanoKontrol2,
        custom_alias: Some("Korg nanoKONTROL2 Surface".to_string()),
        last_known_card_index: Some(2),
        last_known_seq_address: Some((24, 0)),
        last_known_usb_path: Some("1-1.3:1.0".to_string()),
        reconnect_count: 1,
        is_active: false,
    };
    if let Ok(json) = serde_json::to_string_pretty(&nk2_hw) {
        let _ = std::fs::write(dir.join("korg-nanokontrol2.hardware.json"), json);
    }
    // 3. BS2 profile
    let bs2_hw = crate::hardware::StoredDeviceProfile {
        identity_key: "novation_bs2_usb".to_string(),
        vendor_id: "1235".to_string(),
        product_id: "0064".to_string(),
        serial: Some("BS2-99".to_string()),
        card_name: "Bass Station II".to_string(),
        device_type: crate::hardware::SupportedDeviceType::NovationBassStation2,
        custom_alias: Some("Novation Bass Station II".to_string()),
        last_known_card_index: Some(3),
        last_known_seq_address: Some((28, 0)),
        last_known_usb_path: Some("1-1.4:1.0".to_string()),
        reconnect_count: 1,
        is_active: false,
    };
    if let Ok(json) = serde_json::to_string_pretty(&bs2_hw) {
        let _ = std::fs::write(dir.join("novation-bs2.hardware.json"), json);
    }
    // 4. Clavia Nord Drum 2 router config
    let nd2_router = DualMidiRouter::new();
    if let Ok(json) = serde_json::to_string_pretty(&nd2_router) {
        let _ = std::fs::write(dir.join("clavia-nord-drum-2.hardware.json"), json);
    }
    // 5. Novation Peak USB & router config
    if let Ok(json) = serde_json::to_string_pretty(&peak_midi_router_json_schema()) {
        let _ = std::fs::write(dir.join("peak-midi-router.schema.json"), json);
    }
    let peak_hw = crate::hardware::StoredDeviceProfile {
        identity_key: "novation_peak_usb".to_string(),
        vendor_id: PEAK_USB_VENDOR.to_string(),
        product_id: PEAK_USB_PRODUCT.to_string(),
        serial: Some("PEAK-001".to_string()),
        card_name: "Peak".to_string(),
        device_type: crate::hardware::SupportedDeviceType::NovationPeak,
        custom_alias: Some("Focusrite-Novation Peak 8-Voice Hybrid".to_string()),
        last_known_card_index: Some(3),
        last_known_seq_address: Some((PEAK_SEQ_CLIENT, 0)),
        last_known_usb_path: Some("1-1.5:1.0".to_string()),
        reconnect_count: 1,
        is_active: false,
    };
    if let Ok(json) = serde_json::to_string_pretty(&peak_hw) {
        let _ = std::fs::write(dir.join("novation-peak.hardware.json"), json);
    }
    Ok(())
}

// =============================================================================
// Novation Peak 8-Voice Polyphonic MIDI Router
// =============================================================================

/// Polyphonic 8-voice MIDI router for the Focusrite-Novation Peak (`1235:007e`, `hw:3,0,0`).
///
/// Supports single-channel polyphonic routing (default Channel 1), automatic acoustic
/// safe-sweep clamping (`synth_core::safe_sweep_range`), optional +32 dual-CC pair emission,
/// Animate 1 & 2 performance holds (`CC 114` / `CC 115`), Bank Select + Program Change
/// (`CC 0` / `CC 32` / `PC`), 8-voice allocation with oldest-voice stealing, and full panic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeakMidiRouter {
    /// 1-based MIDI channel (`1..=16`, factory default `1`).
    pub midi_channel: u8,
    /// Operational hardware safety mode (default `VirtualLoopback`).
    pub safety_mode: MidiSafetyMode,
    /// When `true`, clamps continuous CC sweeps using `synth_core::safe_sweep_range(param_name)`.
    pub enforce_safe_sweep: bool,
    /// When `true`, also emits the paired +32 CC for Peak dual-CC parameters (`23/55`, `24/56`, `25/57`, `29/61`, `30/62`, `31/63`).
    pub emit_dual_cc_pairs: bool,
    /// Active held MIDI note numbers (`0..=127`), capped at `PEAK_POLYPHONY` (8 voices).
    pub active_voices: Vec<u8>,
    /// Current state of Animate 1 Hold (`CC 114`).
    pub animate_1_held: bool,
    /// Current state of Animate 2 Hold (`CC 115`).
    pub animate_2_held: bool,
}

impl Default for PeakMidiRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl PeakMidiRouter {
    /// Creates a new Novation Peak router in safe `VirtualLoopback` mode on MIDI Channel 1.
    #[must_use]
    pub fn new() -> Self {
        Self {
            midi_channel: PEAK_DEFAULT_CHANNEL,
            safety_mode: MidiSafetyMode::VirtualLoopback,
            enforce_safe_sweep: true,
            emit_dual_cc_pairs: false,
            active_voices: Vec::with_capacity(PEAK_POLYPHONY),
            animate_1_held: false,
            animate_2_held: false,
        }
    }

    /// Builder method setting the 1-based MIDI channel (`1..=16`).
    #[must_use]
    pub fn with_midi_channel(mut self, channel: u8) -> Self {
        self.midi_channel = channel.clamp(1, 16);
        self
    }

    /// Builder method setting the operational safety mode.
    #[must_use]
    pub fn with_safety_mode(mut self, mode: MidiSafetyMode) -> Self {
        self.safety_mode = mode;
        self
    }

    /// Builder method enabling or disabling safe-sweep clamping.
    #[must_use]
    pub fn with_safe_sweep(mut self, enforce: bool) -> Self {
        self.enforce_safe_sweep = enforce;
        self
    }

    /// Builder method enabling or disabling +32 dual-CC pair emission.
    #[must_use]
    pub fn with_dual_cc_pairs(mut self, enable: bool) -> Self {
        self.emit_dual_cc_pairs = enable;
        self
    }

    /// Returns the 0-indexed wire MIDI channel (`0..=15`).
    #[must_use]
    pub const fn wire_channel(&self) -> u8 {
        self.midi_channel.saturating_sub(1) & 0x0F
    }

    /// Routes a parameter CC, applying `synth_core::safe_sweep_range(param_name)` if `enforce_safe_sweep` is true,
    /// and appending the paired +32 CC if `emit_dual_cc_pairs` is true.
    pub fn route_cc(&self, cc: u8, param_name: &str, value: u8) -> Vec<MidiPacket> {
        let clamped_val = if self.enforce_safe_sweep {
            synth_core::safe_sweep_range(param_name).clamp(value)
        } else {
            value & 0x7F
        };

        let ch = self.wire_channel();
        let mut out = vec![MidiPacket::ControlChange {
            channel: ch,
            cc: cc & 0x7F,
            value: clamped_val,
        }];

        if self.emit_dual_cc_pairs {
            if let Some(partner_cc) = match cc {
                23 => Some(55),
                24 => Some(56),
                25 => Some(57),
                29 => Some(61),
                30 => Some(62),
                31 => Some(63),
                _ => None,
            } {
                out.push(MidiPacket::ControlChange {
                    channel: ch,
                    cc: partner_cc,
                    value: clamped_val,
                });
            }
        }

        out
    }

    /// Routes a Note-On event, performing 8-voice polyphonic voice allocation (emitting a Note-Off
    /// for the oldest active voice if all 8 voices are already occupied).
    pub fn route_note_on(&mut self, note: u8, velocity: u8) -> Vec<MidiPacket> {
        let n = note & 0x7F;
        let vel = velocity.clamp(1, 127);
        let ch = self.wire_channel();
        let mut packets = Vec::with_capacity(2);

        if let Some(pos) = self.active_voices.iter().position(|&active| active == n) {
            self.active_voices.remove(pos);
        } else if self.active_voices.len() >= PEAK_POLYPHONY {
            let stolen = self.active_voices.remove(0);
            packets.push(MidiPacket::NoteOff {
                channel: ch,
                note: stolen,
                velocity: 0,
            });
        }

        self.active_voices.push(n);
        packets.push(MidiPacket::NoteOn {
            channel: ch,
            note: n,
            velocity: vel,
        });
        packets
    }

    /// Routes a Note-Off event and releases the voice from `active_voices`.
    pub fn route_note_off(&mut self, note: u8, velocity: u8) -> MidiPacket {
        let n = note & 0x7F;
        if let Some(pos) = self.active_voices.iter().position(|&active| active == n) {
            self.active_voices.remove(pos);
        }
        MidiPacket::NoteOff {
            channel: self.wire_channel(),
            note: n,
            velocity: velocity & 0x7F,
        }
    }

    /// Routes an Animate 1 (`index = 1`, `CC 114`) or Animate 2 (`index = 2`, `CC 115`) performance hold toggle.
    pub fn route_animate_hold(&mut self, index: u8, held: bool) -> Option<MidiPacket> {
        let cc = match index {
            1 => {
                self.animate_1_held = held;
                114
            }
            2 => {
                self.animate_2_held = held;
                115
            }
            _ => return None,
        };
        Some(MidiPacket::ControlChange {
            channel: self.wire_channel(),
            cc,
            value: if held { 127 } else { 0 },
        })
    }

    /// Routes a Bank Select (`CC 0 = 0`, `CC 32 = bank_lsb`) + Program Change (`0..=127`) sequence.
    pub fn route_bank_program(&self, bank_lsb: u8, program: u8) -> Vec<MidiPacket> {
        let ch = self.wire_channel();
        vec![
            MidiPacket::ControlChange {
                channel: ch,
                cc: 0,
                value: 0,
            },
            MidiPacket::ControlChange {
                channel: ch,
                cc: 32,
                value: bank_lsb.clamp(1, 4),
            },
            MidiPacket::ProgramChange {
                channel: ch,
                program: program & 0x7F,
            },
        ]
    }

    /// Emits a full Peak panic sequence matching `silencePeak()` in `js/konduktiva-peak/src/midi.js`:
    /// `CC 120 = 0` (All Sound Off), `CC 123 = 0` (All Notes Off), and explicit `NoteOff` on all 128 notes (`0..=127`).
    pub fn route_panic(&mut self) -> Vec<MidiPacket> {
        self.active_voices.clear();
        self.animate_1_held = false;
        self.animate_2_held = false;
        let ch = self.wire_channel();
        let mut packets = Vec::with_capacity(130);
        packets.push(MidiPacket::ControlChange {
            channel: ch,
            cc: 120,
            value: 0,
        });
        packets.push(MidiPacket::ControlChange {
            channel: ch,
            cc: 123,
            value: 0,
        });
        for note in 0..=127u8 {
            packets.push(MidiPacket::NoteOff {
                channel: ch,
                note,
                velocity: 0,
            });
        }
        packets
    }
}

/// Generates the standard JSON Schema (Draft-07) for `PeakMidiRouter`.
pub fn peak_midi_router_json_schema() -> serde_json::Value {
    serde_json::json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "PeakMidiRouter",
        "description": "Novation Peak 8-voice polyphonic MIDI router and safety configuration",
        "type": "object",
        "required": ["midi_channel", "safety_mode", "enforce_safe_sweep", "emit_dual_cc_pairs"],
        "properties": {
            "midi_channel": { "type": "integer", "minimum": 1, "maximum": 16, "default": 1 },
            "safety_mode": { "type": "string", "enum": ["VirtualLoopback", "DryRun", "LiveHardwareArmed"] },
            "enforce_safe_sweep": { "type": "boolean", "default": true },
            "emit_dual_cc_pairs": { "type": "boolean", "default": false }
        }
    })
}

/// Validates and parses JSON into a `PeakMidiRouter`.
pub fn validate_peak_midi_router_json(
    json_str: &str,
) -> Result<PeakMidiRouter, synth_core::SynthError> {
    serde_json::from_str(json_str)
        .map_err(|e| synth_core::SynthError::SerializationError(e.to_string()))
}

// =============================================================================
// Anti-Choke CC Thinning & Note-On Priority Scheduler
// =============================================================================

/// Pending Control Change message awaiting scheduled transmission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingCcMessage {
    pub voice: u8,
    pub cc: u8,
    pub value: u8,
    pub received_time: Instant,
}

/// Operational metrics tracked by the Anti-Choke scheduler.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulerMetrics {
    pub notes_dispatched: u64,
    pub ccs_dispatched: u64,
    pub ccs_thinned: u64,
    pub focus_switches_dispatched: u64,
    pub loopback_packets_captured: u64,
}

/// Anti-Choke MIDI CC Thinning & Note-On Priority scheduler.
///
/// Prevents hardware serial choke on dense knob sweeps or LFO modulation by
/// deduplicating intermediate values per `(voice, cc)` pair and enforcing a
/// minimum transmission interval (default 2ms), while prioritizing Note-On
/// trigger messages so drum hits are never delayed or dropped.
#[derive(Debug, Clone)]
pub struct AntiChokeScheduler {
    pub router: DualMidiRouter,
    pub throttle_duration: Duration,
    note_queue: VecDeque<MidiPacket>,
    pending_cc: HashMap<(u8, u8), PendingCcMessage>,
    last_sent_cc_times: HashMap<(u8, u8), Instant>,
    loopback_log: Vec<MidiPacket>,
    metrics: SchedulerMetrics,
}

impl Default for AntiChokeScheduler {
    fn default() -> Self {
        Self::new(DualMidiRouter::default())
    }
}

impl AntiChokeScheduler {
    /// Creates a new Anti-Choke scheduler wrapping a dual MIDI router.
    #[must_use]
    pub fn new(router: DualMidiRouter) -> Self {
        Self {
            router,
            throttle_duration: Duration::from_millis(DEFAULT_CC_THROTTLE_MS),
            note_queue: VecDeque::new(),
            pending_cc: HashMap::new(),
            last_sent_cc_times: HashMap::new(),
            loopback_log: Vec::new(),
            metrics: SchedulerMetrics::default(),
        }
    }

    /// Sets the minimum interval between CC messages for the same (voice, cc) pair.
    #[must_use]
    pub fn with_throttle_duration(mut self, dur: Duration) -> Self {
        self.throttle_duration = dur;
        self
    }

    /// Queues a high-priority Note-On trigger message.
    ///
    /// Note triggers are prioritized ahead of pending CC messages and are never thinned.
    pub fn schedule_note_on(&mut self, voice_index: u8, velocity: u8) -> Option<MidiPacket> {
        let pkt = self.router.route_note_on(voice_index, velocity)?;
        self.note_queue.push_back(pkt.clone());
        Some(pkt)
    }

    /// Queues a high-priority Note-Off release message.
    pub fn schedule_note_off(&mut self, voice_index: u8, velocity: u8) -> Option<MidiPacket> {
        let pkt = self.router.route_note_off(voice_index, velocity)?;
        self.note_queue.push_back(pkt.clone());
        Some(pkt)
    }

    /// Schedules a parameter CC message with rate thinning and deduplication.
    ///
    /// If an identical value is already pending for this `(voice, cc)` pair, it is
    /// discarded. If a new value arrives before the previous value has flushed,
    /// the pending value is updated to the latest state and the thinned count increments.
    pub fn schedule_cc(&mut self, voice_index: u8, cc: u8, value: u8, now: Instant) {
        if voice_index < 1 || voice_index as usize > VOICE_COUNT {
            return;
        }
        let key = (voice_index, cc);
        let val7 = value & 0x7F;

        if let Some(existing) = self.pending_cc.get_mut(&key) {
            if existing.value == val7 {
                // Exact duplicate: drop silently
                self.metrics.ccs_thinned = self.metrics.ccs_thinned.saturating_add(1);
                return;
            }
            // Dense burst update: overwrite with latest parameter value
            existing.value = val7;
            existing.received_time = now;
            self.metrics.ccs_thinned = self.metrics.ccs_thinned.saturating_add(1);
        } else {
            self.pending_cc.insert(
                key,
                PendingCcMessage {
                    voice: voice_index,
                    cc,
                    value: val7,
                    received_time: now,
                },
            );
        }
    }

    /// Drains all ready messages at time `now`.
    ///
    /// High-priority Note events are always emitted first. Eligible pending CCs
    /// (whose throttle duration has elapsed since the last transmission for that
    /// voice and CC) are emitted subsequently.
    pub fn drain_ready(&mut self, now: Instant) -> Vec<MidiPacket> {
        let mut dispatched = Vec::new();

        // 1. Drain high-priority Note triggers first
        while let Some(note_pkt) = self.note_queue.pop_front() {
            self.metrics.notes_dispatched = self.metrics.notes_dispatched.saturating_add(1);
            dispatched.push(note_pkt);
        }

        // 2. Identify eligible CCs that have passed the throttle duration
        let eligible_keys: Vec<(u8, u8)> = self
            .pending_cc
            .keys()
            .copied()
            .filter(|key| match self.last_sent_cc_times.get(key) {
                Some(&last) => now.duration_since(last) >= self.throttle_duration,
                None => true,
            })
            .collect();

        // Sort keys for deterministic output ordering
        let mut sorted_keys = eligible_keys;
        sorted_keys.sort_unstable();

        for key in sorted_keys {
            if let Some(msg) = self.pending_cc.remove(&key) {
                let pkts = self.router.route_param_cc(msg.voice, msg.cc, msg.value);
                for pkt in pkts {
                    if let MidiPacket::ControlChange { cc, .. } = &pkt {
                        if *cc == GLOBAL_FOCUS_CC {
                            self.metrics.focus_switches_dispatched =
                                self.metrics.focus_switches_dispatched.saturating_add(1);
                        } else {
                            self.metrics.ccs_dispatched =
                                self.metrics.ccs_dispatched.saturating_add(1);
                        }
                    }
                    dispatched.push(pkt);
                }
                self.last_sent_cc_times.insert(key, now);
            }
        }

        // 3. Record into loopback sink if operating in safe mode
        if self.router.safety_mode.is_safe() {
            self.metrics.loopback_packets_captured = self
                .metrics
                .loopback_packets_captured
                .saturating_add(dispatched.len() as u64);
            self.loopback_log.extend(dispatched.clone());
        }

        dispatched
    }

    /// Current scheduler performance and throughput metrics.
    #[must_use]
    pub const fn metrics(&self) -> SchedulerMetrics {
        self.metrics
    }

    /// Inspection slice of the in-memory loopback sink log.
    #[must_use]
    pub fn loopback_log(&self) -> &[MidiPacket] {
        &self.loopback_log
    }

    /// Clears the in-memory loopback log.
    pub fn clear_loopback_log(&mut self) {
        self.loopback_log.clear();
    }

    /// Returns true if no Note or CC events are currently queued.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.note_queue.is_empty() && self.pending_cc.is_empty()
    }

    /// Clears all pending queues and resets metrics.
    pub fn clear(&mut self) {
        self.note_queue.clear();
        self.pending_cc.clear();
        self.last_sent_cc_times.clear();
        self.loopback_log.clear();
        self.metrics = SchedulerMetrics::default();
    }
}

// =============================================================================
// Non-Invasive ALSA Port Discovery
// =============================================================================

/// Physical or virtual ALSA MIDI endpoint discovered on Linux.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlsaPortEndpoint {
    pub card_index: u32,
    pub rawmidi_device: String,
    pub seq_client: Option<u32>,
    pub seq_port: Option<u32>,
    pub card_name: String,
    pub device_type: SupportedDeviceType,
    pub is_virtual_mock: bool,
}

/// Consolidated discovery status for Nord Drum 2 workstation hardware.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NordDrumDiscoveryResult {
    pub umc1820: Option<AlsaPortEndpoint>,
    pub nanokontrol2: Option<AlsaPortEndpoint>,
    pub total_devices_scanned: usize,
    pub safety_mode: MidiSafetyMode,
}

impl NordDrumDiscoveryResult {
    /// Returns true if the physical or simulated UMC1820 port is discovered.
    #[must_use]
    pub const fn has_nord_drum_port(&self) -> bool {
        self.umc1820.is_some()
    }

    /// Returns true if the physical or simulated nanoKONTROL2 surface is discovered.
    #[must_use]
    pub const fn has_controller(&self) -> bool {
        self.nanokontrol2.is_some()
    }
}

/// Performs non-invasive ALSA port discovery.
///
/// In `VirtualLoopback` mode, returns simulated loopback endpoints without
/// querying live host devices, ensuring automated builds and tests remain 100% safe.
/// In `DryRun` or `LiveHardwareArmed` mode, non-invasively inspects `/sys/class/sound`
/// and `/proc/asound/seq/clients` without sending hardware signals.
pub fn discover_nord_drum_hardware(safety_mode: MidiSafetyMode) -> NordDrumDiscoveryResult {
    if safety_mode == MidiSafetyMode::VirtualLoopback {
        return NordDrumDiscoveryResult {
            umc1820: Some(AlsaPortEndpoint {
                card_index: 0,
                rawmidi_device: UMC1820_ALSA_DEVICE.to_string(),
                seq_client: Some(16),
                seq_port: Some(0),
                card_name: "UMC1820".to_string(),
                device_type: SupportedDeviceType::BehringerUmc1820,
                is_virtual_mock: true,
            }),
            nanokontrol2: Some(AlsaPortEndpoint {
                card_index: 2,
                rawmidi_device: NANOKONTROL2_ALSA_DEVICE.to_string(),
                seq_client: Some(24),
                seq_port: Some(0),
                card_name: "nanoKONTROL2".to_string(),
                device_type: SupportedDeviceType::KorgNanoKontrol2,
                is_virtual_mock: true,
            }),
            total_devices_scanned: 2,
            safety_mode: MidiSafetyMode::VirtualLoopback,
        };
    }

    discover_nord_drum_hardware_with_roots(
        Path::new("/sys/class/sound"),
        Path::new("/dev/snd/by-id"),
        Path::new("/dev/snd/by-path"),
        Path::new("/proc/asound/seq/clients"),
        safety_mode,
    )
}

/// Performs non-invasive ALSA port discovery with customizable filesystem roots
/// for deterministic unit testing.
pub fn discover_nord_drum_hardware_with_roots(
    sys_class_sound: &Path,
    dev_snd_by_id: &Path,
    dev_snd_by_path: &Path,
    proc_asound_seq_clients: &Path,
    safety_mode: MidiSafetyMode,
) -> NordDrumDiscoveryResult {
    let devices = scan_hardware_devices_with_roots(
        sys_class_sound,
        dev_snd_by_id,
        dev_snd_by_path,
        proc_asound_seq_clients,
    );

    let mut umc_endpoint = None;
    let mut nk2_endpoint = None;

    for dev in &devices {
        if dev.device_type == SupportedDeviceType::BehringerUmc1820 {
            umc_endpoint = Some(AlsaPortEndpoint {
                card_index: dev.alsa_card_index,
                rawmidi_device: format!("hw:{},0,0", dev.alsa_card_index),
                seq_client: dev.alsa_seq_client,
                seq_port: dev.alsa_seq_port,
                card_name: dev.card_name.clone(),
                device_type: dev.device_type,
                is_virtual_mock: false,
            });
        }
        if dev.device_type == SupportedDeviceType::KorgNanoKontrol2 {
            nk2_endpoint = Some(AlsaPortEndpoint {
                card_index: dev.alsa_card_index,
                rawmidi_device: format!("hw:{},0,0", dev.alsa_card_index),
                seq_client: dev.alsa_seq_client,
                seq_port: dev.alsa_seq_port,
                card_name: dev.card_name.clone(),
                device_type: dev.device_type,
                is_virtual_mock: false,
            });
        }
    }

    NordDrumDiscoveryResult {
        umc1820: umc_endpoint,
        nanokontrol2: nk2_endpoint,
        total_devices_scanned: devices.len(),
        safety_mode,
    }
}

/// Consolidated ALSA hardware discovery result for the Novation Peak workstation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeakDiscoveryResult {
    pub peak: Option<AlsaPortEndpoint>,
    pub umc1820: Option<AlsaPortEndpoint>,
    pub nanokontrol2: Option<AlsaPortEndpoint>,
    pub total_devices_scanned: usize,
    pub safety_mode: MidiSafetyMode,
}

impl PeakDiscoveryResult {
    /// Returns `true` if the Novation Peak USB MIDI endpoint (`hw:3,0,0` or simulated mock) is available.
    #[must_use]
    pub const fn has_peak_usb(&self) -> bool {
        self.peak.is_some()
    }

    /// Returns `true` if the UMC1820 5-pin DIN MIDI interface (`hw:0,0,0` or simulated mock) is available.
    #[must_use]
    pub const fn has_umc1820_din(&self) -> bool {
        self.umc1820.is_some()
    }

    /// Returns `true` if the Korg nanoKONTROL2 surface (`hw:2,0,0` or simulated mock) is available.
    #[must_use]
    pub const fn has_controller(&self) -> bool {
        self.nanokontrol2.is_some()
    }
}

/// Performs non-invasive ALSA port discovery for the Novation Peak workstation.
///
/// In `VirtualLoopback` mode, returns simulated loopback endpoints for `Peak MIDI 1` (`hw:3,0,0`),
/// `UMC1820` (`hw:0,0,0`), and `nanoKONTROL2` (`hw:2,0,0`) without opening physical ports.
pub fn discover_peak_hardware(safety_mode: MidiSafetyMode) -> PeakDiscoveryResult {
    if safety_mode == MidiSafetyMode::VirtualLoopback {
        return PeakDiscoveryResult {
            peak: Some(AlsaPortEndpoint {
                card_index: 3,
                rawmidi_device: PEAK_ALSA_DEVICE.to_string(),
                seq_client: Some(PEAK_SEQ_CLIENT),
                seq_port: Some(0),
                card_name: "Peak".to_string(),
                device_type: SupportedDeviceType::NovationPeak,
                is_virtual_mock: true,
            }),
            umc1820: Some(AlsaPortEndpoint {
                card_index: 0,
                rawmidi_device: UMC1820_ALSA_DEVICE.to_string(),
                seq_client: Some(16),
                seq_port: Some(0),
                card_name: "UMC1820".to_string(),
                device_type: SupportedDeviceType::BehringerUmc1820,
                is_virtual_mock: true,
            }),
            nanokontrol2: Some(AlsaPortEndpoint {
                card_index: 2,
                rawmidi_device: NANOKONTROL2_ALSA_DEVICE.to_string(),
                seq_client: Some(24),
                seq_port: Some(0),
                card_name: "nanoKONTROL2".to_string(),
                device_type: SupportedDeviceType::KorgNanoKontrol2,
                is_virtual_mock: true,
            }),
            total_devices_scanned: 3,
            safety_mode: MidiSafetyMode::VirtualLoopback,
        };
    }

    discover_peak_hardware_with_roots(
        Path::new("/sys/class/sound"),
        Path::new("/dev/snd/by-id"),
        Path::new("/dev/snd/by-path"),
        Path::new("/proc/asound/seq/clients"),
        safety_mode,
    )
}

/// Performs non-invasive ALSA port discovery for the Novation Peak with customizable filesystem roots.
pub fn discover_peak_hardware_with_roots(
    sys_class_sound: &Path,
    dev_snd_by_id: &Path,
    dev_snd_by_path: &Path,
    proc_asound_seq_clients: &Path,
    safety_mode: MidiSafetyMode,
) -> PeakDiscoveryResult {
    let devices = scan_hardware_devices_with_roots(
        sys_class_sound,
        dev_snd_by_id,
        dev_snd_by_path,
        proc_asound_seq_clients,
    );

    let mut peak_endpoint = None;
    let mut umc_endpoint = None;
    let mut nk2_endpoint = None;

    for dev in &devices {
        let ep = AlsaPortEndpoint {
            card_index: dev.alsa_card_index,
            rawmidi_device: format!("hw:{},0,0", dev.alsa_card_index),
            seq_client: dev.alsa_seq_client,
            seq_port: dev.alsa_seq_port,
            card_name: dev.card_name.clone(),
            device_type: dev.device_type,
            is_virtual_mock: false,
        };
        match dev.device_type {
            SupportedDeviceType::NovationPeak => peak_endpoint = Some(ep),
            SupportedDeviceType::BehringerUmc1820 => umc_endpoint = Some(ep),
            SupportedDeviceType::KorgNanoKontrol2 => nk2_endpoint = Some(ep),
            _ => {}
        }
    }

    PeakDiscoveryResult {
        peak: peak_endpoint,
        umc1820: umc_endpoint,
        nanokontrol2: nk2_endpoint,
        total_devices_scanned: devices.len(),
        safety_mode,
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safety_mode_defaults_and_guards() {
        let default_mode = MidiSafetyMode::default();
        assert_eq!(default_mode, MidiSafetyMode::VirtualLoopback);
        assert!(default_mode.is_safe());
        assert!(!default_mode.is_hardware_armed());

        let dry_mode = MidiSafetyMode::DryRun;
        assert!(dry_mode.is_safe());
        assert!(!dry_mode.is_hardware_armed());

        let armed = MidiSafetyMode::LiveHardwareArmed;
        assert!(!armed.is_safe());
        assert!(armed.is_hardware_armed());
    }

    #[test]
    fn dual_midi_router_individual_channels_mode() {
        let mut router = DualMidiRouter::new();
        assert_eq!(router.control_mode, MidiControlMode::IndividualChannels);

        // Parameter CC to Voice 2 CC 50 Value 80 -> should go to Ch 2 (wire channel 1)
        let pkts = router.route_param_cc(2, 50, 80);
        assert_eq!(pkts.len(), 1);
        assert_eq!(
            pkts[0],
            MidiPacket::ControlChange {
                channel: 1,
                cc: 50,
                value: 80
            }
        );

        // Note-On to Voice 1 -> should go to Ch 1 (wire channel 0) with default trigger note 60
        let note_pkt = router.route_note_on(1, 100).expect("Note packet expected");
        assert_eq!(
            note_pkt,
            MidiPacket::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100
            }
        );

        // Note-Off to Voice 1
        let note_off = router.route_note_off(1, 64).expect("Note off expected");
        assert_eq!(
            note_off,
            MidiPacket::NoteOff {
                channel: 0,
                note: 60,
                velocity: 64
            }
        );
    }

    #[test]
    fn dual_midi_router_global_focus_mode_cc70_prepending() {
        let mut router = DualMidiRouter::new()
            .with_control_mode(MidiControlMode::GlobalFocus)
            .with_global_channel(10);
        router.current_focus_voice = 1;

        // Message to Voice 3: should prepend CC 70 value 2 (Voice 3 is 0-indexed 2) on Ch 10 (wire channel 9)
        let pkts1 = router.route_param_cc(3, 46, 15);
        assert_eq!(pkts1.len(), 2);
        assert_eq!(
            pkts1[0],
            MidiPacket::ControlChange {
                channel: 9,
                cc: GLOBAL_FOCUS_CC,
                value: 2
            }
        );
        assert_eq!(
            pkts1[1],
            MidiPacket::ControlChange {
                channel: 9,
                cc: 46,
                value: 15
            }
        );
        assert_eq!(router.current_focus_voice, 3);

        // Subsequent message to Voice 3: same focus, should NOT prepend CC 70
        let pkts2 = router.route_param_cc(3, 52, 64);
        assert_eq!(pkts2.len(), 1);
        assert_eq!(
            pkts2[0],
            MidiPacket::ControlChange {
                channel: 9,
                cc: 52,
                value: 64
            }
        );

        // Switching back to Voice 1: prepends CC 70 value 0
        let pkts3 = router.route_param_cc(1, 46, 30);
        assert_eq!(pkts3.len(), 2);
        assert_eq!(
            pkts3[0],
            MidiPacket::ControlChange {
                channel: 9,
                cc: GLOBAL_FOCUS_CC,
                value: 0
            }
        );
        assert_eq!(
            pkts3[1],
            MidiPacket::ControlChange {
                channel: 9,
                cc: 46,
                value: 30
            }
        );
    }

    #[test]
    fn configurable_global_trigger_notes() {
        let mut router = DualMidiRouter::new().with_control_mode(MidiControlMode::GlobalFocus);

        // Default trigger notes are [60, 62, 64, 65, 67, 69]
        let n1 = router.route_note_on(1, 120).unwrap();
        let n6 = router.route_note_on(6, 120).unwrap();
        assert_eq!(
            n1,
            MidiPacket::NoteOn {
                channel: 9,
                note: 60,
                velocity: 120
            }
        );
        assert_eq!(
            n6,
            MidiPacket::NoteOn {
                channel: 9,
                note: 69,
                velocity: 120
            }
        );

        // Configure custom trigger notes: Voice 1 -> 36 (Acoustic Kick), Voice 2 -> 38 (Snare)
        assert!(router.set_trigger_note(1, 36).is_ok());
        assert!(router.set_trigger_note(2, 38).is_ok());
        assert_eq!(router.get_trigger_note(1), Some(36));
        assert_eq!(router.get_trigger_note(2), Some(38));

        let kick = router.route_note_on(1, 127).unwrap();
        let snare = router.route_note_on(2, 110).unwrap();
        assert_eq!(
            kick,
            MidiPacket::NoteOn {
                channel: 9,
                note: 36,
                velocity: 127
            }
        );
        assert_eq!(
            snare,
            MidiPacket::NoteOn {
                channel: 9,
                note: 38,
                velocity: 110
            }
        );

        // Invalid voice index validation
        assert!(router.set_trigger_note(0, 40).is_err());
        assert!(router.set_trigger_note(7, 40).is_err());
    }

    #[test]
    fn anti_choke_scheduler_burst_deduplication_and_thinning() {
        let router = DualMidiRouter::new().with_safety_mode(MidiSafetyMode::VirtualLoopback);
        let mut scheduler = AntiChokeScheduler::new(router)
            .with_throttle_duration(Duration::from_millis(5));

        let t0 = Instant::now();

        // Simulate dense burst: 10 Rapid CC updates for Voice 1 CC 46 within the same millisecond
        for val in 0..10 {
            scheduler.schedule_cc(1, 46, val, t0);
        }

        let metrics_after_burst = scheduler.metrics();
        // First schedule added, subsequent 9 were thinned
        assert_eq!(metrics_after_burst.ccs_thinned, 9);

        // Draining immediately at t0: should emit ONLY the latest value (9)
        let drained1 = scheduler.drain_ready(t0);
        assert_eq!(drained1.len(), 1);
        assert_eq!(
            drained1[0],
            MidiPacket::ControlChange {
                channel: 0,
                cc: 46,
                value: 9
            }
        );
        assert_eq!(scheduler.metrics().ccs_dispatched, 1);

        // Immediately scheduling another update before 5ms elapse
        let t1 = t0 + Duration::from_millis(2);
        scheduler.schedule_cc(1, 46, 20, t1);

        // Attempt drain at t1 (< 5ms since t0): should hold off because throttle duration has not elapsed
        let drained2 = scheduler.drain_ready(t1);
        assert!(drained2.is_empty());

        // Drain at t2 (t0 + 6ms): now eligible!
        let t2 = t0 + Duration::from_millis(6);
        let drained3 = scheduler.drain_ready(t2);
        assert_eq!(drained3.len(), 1);
        assert_eq!(
            drained3[0],
            MidiPacket::ControlChange {
                channel: 0,
                cc: 46,
                value: 20
            }
        );
        assert_eq!(scheduler.metrics().ccs_dispatched, 2);
    }

    #[test]
    fn anti_choke_scheduler_note_on_priority() {
        let router = DualMidiRouter::new()
            .with_control_mode(MidiControlMode::GlobalFocus)
            .with_safety_mode(MidiSafetyMode::VirtualLoopback);
        let mut scheduler = AntiChokeScheduler::new(router);
        let t0 = Instant::now();

        // Queue CC updates first
        scheduler.schedule_cc(2, 50, 70, t0);
        scheduler.schedule_cc(3, 52, 80, t0);

        // Queue Note-On trigger later
        scheduler.schedule_note_on(1, 125);

        // Drain: Note-On must appear FIRST in the output stream!
        let drained = scheduler.drain_ready(t0);
        assert!(drained.len() >= 2);
        assert!(
            drained[0].is_note(),
            "Note-On must be dispatched ahead of CC stream"
        );
        assert_eq!(
            drained[0],
            MidiPacket::NoteOn {
                channel: 9,
                note: 60,
                velocity: 125
            }
        );

        // Verify captured in loopback log
        assert_eq!(scheduler.loopback_log().len(), drained.len());
        assert_eq!(scheduler.metrics().notes_dispatched, 1);
    }

    #[test]
    fn non_invasive_alsa_port_discovery_virtual_loopback() {
        let res = discover_nord_drum_hardware(MidiSafetyMode::VirtualLoopback);
        assert_eq!(res.safety_mode, MidiSafetyMode::VirtualLoopback);
        assert!(res.has_nord_drum_port());
        assert!(res.has_controller());

        let umc = res.umc1820.unwrap();
        assert_eq!(umc.rawmidi_device, "hw:0,0,0");
        assert_eq!(umc.seq_client, Some(16));
        assert_eq!(umc.seq_port, Some(0));
        assert!(umc.is_virtual_mock);

        let nk2 = res.nanokontrol2.unwrap();
        assert_eq!(nk2.rawmidi_device, "hw:2,0,0");
        assert_eq!(nk2.seq_client, Some(24));
        assert_eq!(nk2.seq_port, Some(0));
        assert!(nk2.is_virtual_mock);
    }

    #[test]
    fn midi_packet_serialization() {
        let cc = MidiPacket::ControlChange {
            channel: 9,
            cc: 70,
            value: 3,
        };
        assert_eq!(cc.to_bytes(), vec![0xB9, 70, 3]);

        let note = MidiPacket::NoteOn {
            channel: 0,
            note: 60,
            velocity: 127,
        };
        assert_eq!(note.to_bytes(), vec![0x90, 60, 127]);

        let note_off = MidiPacket::NoteOff {
            channel: 0,
            note: 60,
            velocity: 0,
        };
        assert_eq!(note_off.to_bytes(), vec![0x80, 60, 0]);
    }

    #[test]
    fn peak_midi_router_polyphony_safe_sweep_and_panic() {
        let mut router = PeakMidiRouter::new().with_dual_cc_pairs(true);
        assert_eq!(router.midi_channel, 1);
        assert_eq!(router.wire_channel(), 0);
        assert!(router.safety_mode.is_safe());

        // Safe sweep clamps resonance (CC 79) to <= 62
        let res_pkts = router.route_cc(79, "Filter resonance", 120);
        assert_eq!(res_pkts.len(), 1);
        assert_eq!(
            res_pkts[0],
            MidiPacket::ControlChange {
                channel: 0,
                cc: 79,
                value: 62
            }
        );

        // Dual CC pair on Cutoff (CC 29 -> 29 and 61)
        let cutoff_pkts = router.route_cc(29, "Filter cutoff (Frequency)", 100);
        assert_eq!(cutoff_pkts.len(), 2);
        assert_eq!(
            cutoff_pkts[0],
            MidiPacket::ControlChange {
                channel: 0,
                cc: 29,
                value: 100
            }
        );
        assert_eq!(
            cutoff_pkts[1],
            MidiPacket::ControlChange {
                channel: 0,
                cc: 61,
                value: 100
            }
        );

        // 8-voice allocation + 9th voice steals oldest note (60)
        for n in 60..68 {
            let pkts = router.route_note_on(n, 100);
            assert_eq!(pkts.len(), 1);
        }
        assert_eq!(router.active_voices.len(), 8);

        let steal_pkts = router.route_note_on(72, 110);
        assert_eq!(steal_pkts.len(), 2);
        assert_eq!(
            steal_pkts[0],
            MidiPacket::NoteOff {
                channel: 0,
                note: 60,
                velocity: 0
            }
        );
        assert_eq!(
            steal_pkts[1],
            MidiPacket::NoteOn {
                channel: 0,
                note: 72,
                velocity: 110
            }
        );
        assert_eq!(router.active_voices.len(), 8);

        // Panic emits CC 120, CC 123, and 128 NoteOffs (130 packets total)
        let panic_pkts = router.route_panic();
        assert_eq!(panic_pkts.len(), 130);
        assert!(router.active_voices.is_empty());
    }

    #[test]
    fn peak_alsa_port_discovery_virtual_loopback() {
        let disc = discover_peak_hardware(MidiSafetyMode::VirtualLoopback);
        assert!(disc.has_peak_usb());
        assert!(disc.has_umc1820_din());
        assert!(disc.has_controller());
        let peak = disc.peak.unwrap();
        assert_eq!(peak.rawmidi_device, "hw:3,0,0");
        assert_eq!(peak.seq_client, Some(28));
        assert!(peak.is_virtual_mock);
    }
}
