// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthesizer device profile abstraction, controller surface layouts,
//! and portable JSON schema interchange for cross-project integration.
//!
//! Provides canonical specifications for:
//! - ASM Hydrasynth (Oscillators, Mutators, Filters 1/2, Envelopes 1-5, LFOs 1-5, Macros 1-8, Ribbon, FX)
//! - Korg nanoKONTROL2 (8 Channel strips, Knobs CC 16-23, Sliders CC 0-7, Solo/Mute/Rec LEDs, Transport)
//! - Akai MIDImix (8 Channel strips, 24 Knobs, 8 Sliders, Master Slider, Mute/Rec buttons)
//! - Novation Bass Station II (Complete analog synthesis engine, 14-bit Cutoff, AFX parameters)
//! - Behringer TD-3-MD / TD-3-MO (Acid diode ladder, modded-out sub-osc, overdrive, filter FM)

use crate::error::SynthError;
use crate::param::{ParamAddress, ParamDescriptor, ResponseCurve, ValueRange};
use crate::preset::SynthPatch;
use serde::{Deserialize, Serialize};

/// Category classification for synthesizer and controller hardware.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceCategory {
    /// Sound synthesis hardware or software engine.
    Synthesizer,
    /// Physical MIDI or USB controller surface.
    ControllerSurface,
    /// Hybrid unit containing both sound engine and tactile controller surface.
    Hybrid,
}

/// Physical input control element kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlElementKind {
    RotaryKnob,
    LinearSlider,
    PushButton,
    Encoder,
    RibbonStrip,
    VelocityPad,
}

/// Tactile control element descriptor on a physical controller surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhysicalControlDescriptor {
    pub id: String,
    pub name: String,
    pub kind: ControlElementKind,
    pub cc: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<u8>,
    pub has_led: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub led_cc: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track_index: Option<usize>,
    pub group: String,
}

impl PhysicalControlDescriptor {
    /// Creates a new physical control descriptor.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        kind: ControlElementKind,
        cc: u8,
        group: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind,
            cc,
            channel: None,
            has_led: false,
            led_cc: None,
            track_index: None,
            group: group.into(),
        }
    }

    /// Sets the MIDI channel for this control.
    pub fn with_channel(mut self, ch: u8) -> Self {
        self.channel = Some(ch);
        self
    }

    /// Marks that this control features an indicator LED with an associated MIDI CC.
    pub fn with_led(mut self, led_cc: u8) -> Self {
        self.has_led = true;
        self.led_cc = Some(led_cc);
        self
    }

    /// Associates this control with a specific channel track index (0-based).
    pub fn with_track_index(mut self, track: usize) -> Self {
        self.track_index = Some(track);
        self
    }
}

/// Complete synthesizer or controller surface device profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceSynthesizerProfile {
    pub id: String,
    pub name: String,
    pub manufacturer: String,
    pub version: String,
    pub category: DeviceCategory,
    pub default_midi_channel: u8,
    pub parameters: Vec<ParamDescriptor>,
    pub modules: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub controller_controls: Vec<PhysicalControlDescriptor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl DeviceSynthesizerProfile {
    /// Creates a new empty device synthesizer profile.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        manufacturer: impl Into<String>,
        version: impl Into<String>,
        category: DeviceCategory,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            manufacturer: manufacturer.into(),
            version: version.into(),
            category,
            default_midi_channel: 0,
            parameters: Vec::new(),
            modules: Vec::new(),
            controller_controls: Vec::new(),
            tags: Vec::new(),
        }
    }

    /// Sets default MIDI channel (0..15).
    pub fn with_default_channel(mut self, ch: u8) -> Self {
        self.default_midi_channel = ch.clamp(0, 15);
        self
    }

    /// Adds a parameter descriptor, automatically tracking module names.
    pub fn add_parameter(&mut self, param: ParamDescriptor) {
        if !self.modules.iter().any(|m| m == &param.module) {
            self.modules.push(param.module.clone());
        }
        self.parameters.push(param);
    }

    /// Adds a physical controller element descriptor.
    pub fn add_control(&mut self, control: PhysicalControlDescriptor) {
        self.controller_controls.push(control);
    }

    /// Adds a descriptive tag.
    pub fn add_tag(&mut self, tag: impl Into<String>) {
        self.tags.push(tag.into());
    }

    /// Look up a parameter descriptor by normalized identifier key.
    pub fn get_param(&self, key: &str) -> Option<&ParamDescriptor> {
        self.parameters.iter().find(|p| p.id.as_str() == key)
    }

    /// Look up a parameter descriptor by standard 7-bit Continuous Controller number.
    pub fn get_param_by_cc(&self, cc: u8) -> Option<&ParamDescriptor> {
        self.parameters.iter().find(|p| match p.address {
            ParamAddress::Cc7(c) => c == cc,
            ParamAddress::Cc14Bit { msb, .. } => msb == cc,
            _ => false,
        })
    }

    /// Returns all parameters belonging to a given module.
    pub fn params_for_module(&self, module: &str) -> Vec<&ParamDescriptor> {
        self.parameters
            .iter()
            .filter(|p| p.module.eq_ignore_ascii_case(module))
            .collect()
    }

    /// Returns all parameters belonging to a specific voice (1..6) on multi-voice synthesizers like Nord Drum 2.
    pub fn params_for_voice(&self, voice: u8) -> Vec<&ParamDescriptor> {
        let prefix = format!("voice_{}_", voice);
        let voice_module_prefix = format!("Voice {}", voice);
        self.parameters
            .iter()
            .filter(|p| p.id.as_str().starts_with(&prefix) && p.module.starts_with(&voice_module_prefix))
            .collect()
    }

    /// Returns all global / kit-level parameters on multi-voice synthesizers like Nord Drum 2.
    pub fn global_params(&self) -> Vec<&ParamDescriptor> {
        self.parameters
            .iter()
            .filter(|p| p.module.eq_ignore_ascii_case("Global") || p.module.eq_ignore_ascii_case("Kit"))
            .collect()
    }

    /// Creates an initialized `SynthPatch` with all parameters set to their default values.
    pub fn create_default_patch(&self, patch_name: &str) -> SynthPatch {
        let mut patch = SynthPatch::new(patch_name);
        for p in &self.parameters {
            patch.set_param(p.id.as_str(), p.range.default_val);
        }
        patch
    }

    /// Serializes profile to JSON string.
    pub fn to_json(&self) -> Result<String, SynthError> {
        serde_json::to_string_pretty(self).map_err(|e| SynthError::SerializationError(e.to_string()))
    }

    /// Deserializes profile from JSON string.
    pub fn from_json(json_str: &str) -> Result<Self, SynthError> {
        serde_json::from_str(json_str).map_err(|e| SynthError::SerializationError(e.to_string()))
    }

    // =========================================================================
    // Ready-to-Use Hardware Profiles
    // =========================================================================

    /// ASM Hydrasynth profile derived from the official manual and `reference/asm-cc-map.js`.
    ///
    /// Includes Oscillators 1-3, Mutators 1-4, Filters 1-2, Envelopes 1-5,
    /// LFOs 1-5, Macros 1-8, Ribbon Controller, Pre/Post FX, Delay, Reverb, Mixer, and Arp.
    pub fn asm_hydrasynth() -> Self {
        let mut p = Self::new(
            "asm_hydrasynth",
            "ASM Hydrasynth",
            "Ashun Sound Machines",
            "2.2.0",
            DeviceCategory::Synthesizer,
        )
        .with_default_channel(0);

        p.add_tag("wavetable");
        p.add_tag("mutator");
        p.add_tag("polyphonic");
        p.add_tag("mpe");

        let cc_entries: &[(u8, &str, &str, f64, f64, f64, &str)] = &[
            // System
            (0, "System", "Bank Select MSB", 0.0, 127.0, 0.0, ""),
            (1, "System", "Modulation Wheel", 0.0, 127.0, 0.0, ""),
            (7, "System", "Master Volume", 0.0, 127.0, 100.0, ""),
            (11, "System", "Expression Pedal", 0.0, 127.0, 127.0, ""),
            (32, "System", "Bank Select LSB", 0.0, 127.0, 0.0, ""),
            (64, "System", "Sustain Pedal", 0.0, 127.0, 0.0, ""),
            (123, "System", "All Notes Off", 0.0, 127.0, 0.0, ""),
            // Voice
            (5, "Voice", "Glide Time", 0.0, 127.0, 0.0, ""),
            (66, "Voice", "Glide", 0.0, 127.0, 0.0, ""),
            (95, "Voice", "Detune", 0.0, 127.0, 0.0, ""),
            (117, "Voice", "Stereo Width", 0.0, 127.0, 0.0, ""),
            // Ribbon
            (4, "Ribbon", "Ribbon Mod", 0.0, 127.0, 64.0, ""),
            (67, "Ribbon", "Ribbon Pitch", 0.0, 127.0, 64.0, "cents"),
            // Macros 1..8
            (16, "Macros", "Macro 1", 0.0, 127.0, 0.0, ""),
            (17, "Macros", "Macro 2", 0.0, 127.0, 0.0, ""),
            (18, "Macros", "Macro 3", 0.0, 127.0, 0.0, ""),
            (19, "Macros", "Macro 4", 0.0, 127.0, 0.0, ""),
            (20, "Macros", "Macro 5", 0.0, 127.0, 0.0, ""),
            (21, "Macros", "Macro 6", 0.0, 127.0, 0.0, ""),
            (22, "Macros", "Macro 7", 0.0, 127.0, 0.0, ""),
            (23, "Macros", "Macro 8", 0.0, 127.0, 0.0, ""),
            // Oscillators 1, 2, 3
            (24, "OSC 1", "OSC1 WavScan", 0.0, 127.0, 0.0, ""),
            (111, "OSC 1", "OSC1 Cent", 0.0, 127.0, 64.0, "cents"),
            (118, "Mixer", "OSC1 FRate", 0.0, 127.0, 0.0, ""),
            (26, "OSC 2", "OSC2 WavScan", 0.0, 127.0, 0.0, ""),
            (112, "OSC 2", "OSC2 Cent", 0.0, 127.0, 64.0, "cents"),
            (119, "Mixer", "OSC2 FRate", 0.0, 127.0, 0.0, ""),
            (113, "OSC 3", "OSC3 Cent", 0.0, 127.0, 64.0, "cents"),
            (114, "Mixer", "OSC3 FRate", 0.0, 127.0, 0.0, ""),
            // Mutators 1..4
            (29, "Mutator 1", "Mutator1 Ratio", 0.0, 127.0, 0.0, ""),
            (30, "Mutator 1", "Mutator1 Depth", 0.0, 127.0, 0.0, ""),
            (31, "Mutator 1", "Mutator1 Dry/Wet", 0.0, 127.0, 0.0, ""),
            (33, "Mutator 2", "Mutator2 Ratio", 0.0, 127.0, 0.0, ""),
            (34, "Mutator 2", "Mutator2 Depth", 0.0, 127.0, 0.0, ""),
            (35, "Mutator 2", "Mutator2 Dry/Wet", 0.0, 127.0, 0.0, ""),
            (36, "Mutator 3", "Mutator3 Ratio", 0.0, 127.0, 0.0, ""),
            (37, "Mutator 3", "Mutator3 Depth", 0.0, 127.0, 0.0, ""),
            (39, "Mutator 3", "Mutator3 Dry/Wet", 0.0, 127.0, 0.0, ""),
            (40, "Mutator 4", "Mutator4 Ratio", 0.0, 127.0, 0.0, ""),
            (41, "Mutator 4", "Mutator4 Depth", 0.0, 127.0, 0.0, ""),
            (42, "Mutator 4", "Mutator4 Dry/Wet", 0.0, 127.0, 0.0, ""),
            // Mixer
            (3, "Mixer", "Noise Vol", 0.0, 127.0, 0.0, ""),
            (8, "Mixer", "Noise Pan", 0.0, 127.0, 64.0, ""),
            (9, "Mixer", "Ring Mod Vol", 0.0, 127.0, 0.0, ""),
            (10, "Mixer", "Ring Mod Pan", 0.0, 127.0, 64.0, ""),
            (43, "Mixer", "RM12 Depth", 0.0, 127.0, 0.0, ""),
            (44, "Mixer", "OSC1 Vol", 0.0, 127.0, 100.0, ""),
            (45, "Mixer", "OSC1 Pan", 0.0, 127.0, 64.0, ""),
            (46, "Mixer", "OSC2 Vol", 0.0, 127.0, 100.0, ""),
            (47, "Mixer", "OSC2 Pan", 0.0, 127.0, 64.0, ""),
            (48, "Mixer", "OSC3 Vol", 0.0, 127.0, 0.0, ""),
            (49, "Mixer", "OSC3 Pan", 0.0, 127.0, 64.0, ""),
            (115, "Mixer", "Noise FRate", 0.0, 127.0, 0.0, ""),
            (116, "Mixer", "Ring Mod FRate", 0.0, 127.0, 0.0, ""),
            // Filter 1
            (50, "Filter 1", "Filter1 Drive", 0.0, 127.0, 0.0, ""),
            (51, "Filter 1", "Filter1 Keytrack", 0.0, 127.0, 64.0, ""),
            (52, "Filter 1", "Filter1 LFO1amt", 0.0, 127.0, 64.0, ""),
            (53, "Filter 1", "Filter1 Vel Env", 0.0, 127.0, 64.0, ""),
            (54, "Filter 1", "Filter1 ENV1amt", 0.0, 127.0, 64.0, ""),
            (71, "Filter 1", "Filter1 Res", 0.0, 127.0, 0.0, ""),
            (74, "Filter 1", "Filter1 Cutoff", 0.0, 127.0, 127.0, ""),
            // Filter 2
            (55, "Filter 2", "Filter2 Cutoff", 0.0, 127.0, 127.0, ""),
            (56, "Filter 2", "Filter2 Res", 0.0, 127.0, 0.0, ""),
            (57, "Filter 2", "Filter2 Type", 0.0, 127.0, 0.0, ""),
            (58, "Filter 2", "Filter2 Keytrack", 0.0, 127.0, 64.0, ""),
            (59, "Filter 2", "Filter2 LFO1amt", 0.0, 127.0, 64.0, ""),
            (60, "Filter 2", "Filter2 Vel Env", 0.0, 127.0, 64.0, ""),
            (61, "Filter 2", "Filter2 ENV1amt", 0.0, 127.0, 64.0, ""),
            // Amp
            (62, "Amp", "Amp LFO2amt", 0.0, 127.0, 64.0, ""),
            // Envelopes 1..5
            (81, "ENV 1", "ENV1 Attack", 0.0, 127.0, 0.0, "ms"),
            (82, "ENV 1", "ENV1 Decay", 0.0, 127.0, 64.0, "ms"),
            (83, "ENV 1", "ENV1 Sustain", 0.0, 127.0, 100.0, "%"),
            (84, "ENV 1", "ENV1 Release", 0.0, 127.0, 32.0, "ms"),
            (85, "ENV 2", "ENV2 Attack", 0.0, 127.0, 0.0, "ms"),
            (86, "ENV 2", "ENV2 Decay", 0.0, 127.0, 64.0, "ms"),
            (87, "ENV 2", "ENV2 Sustain", 0.0, 127.0, 100.0, "%"),
            (88, "ENV 2", "ENV2 Release", 0.0, 127.0, 32.0, "ms"),
            (89, "ENV 3", "ENV3 Attack", 0.0, 127.0, 0.0, "ms"),
            (90, "ENV 3", "ENV3 Decay", 0.0, 127.0, 64.0, "ms"),
            (96, "ENV 3", "ENV3 Sustain", 0.0, 127.0, 100.0, "%"),
            (97, "ENV 3", "ENV3 Release", 0.0, 127.0, 32.0, "ms"),
            (25, "ENV 4", "ENV4 Attack", 0.0, 127.0, 0.0, "ms"),
            (27, "ENV 4", "ENV4 Decay", 0.0, 127.0, 64.0, "ms"),
            (125, "ENV 4", "ENV4 Sustain", 0.0, 127.0, 100.0, "%"),
            (124, "ENV 4", "ENV4 Release", 0.0, 127.0, 32.0, "ms"),
            (102, "ENV 5", "ENV5 Attack", 0.0, 127.0, 0.0, "ms"),
            (103, "ENV 5", "ENV5 Decay", 0.0, 127.0, 64.0, "ms"),
            (104, "ENV 5", "ENV5 Sustain", 0.0, 127.0, 100.0, "%"),
            (105, "ENV 5", "ENV5 Release", 0.0, 127.0, 32.0, "ms"),
            // LFOs 1..5
            (70, "LFO 1", "LFO1 Gain", 0.0, 127.0, 0.0, ""),
            (72, "LFO 1", "LFO1 Rate", 0.0, 127.0, 64.0, "Hz"),
            (28, "LFO 2", "LFO2 Gain", 0.0, 127.0, 0.0, ""),
            (73, "LFO 2", "LFO2 Rate", 0.0, 127.0, 64.0, "Hz"),
            (75, "LFO 3", "LFO3 Gain", 0.0, 127.0, 0.0, ""),
            (76, "LFO 3", "LFO3 Rate", 0.0, 127.0, 64.0, "Hz"),
            (77, "LFO 4", "LFO4 Gain", 0.0, 127.0, 0.0, ""),
            (78, "LFO 4", "LFO4 Rate", 0.0, 127.0, 64.0, "Hz"),
            (79, "LFO 5", "LFO5 Gain", 0.0, 127.0, 0.0, ""),
            (80, "LFO 5", "LFO5 Rate", 0.0, 127.0, 64.0, "Hz"),
            // FX (Pre-FX, Delay, Reverb, Post-FX)
            (12, "Pre-FX", "PRE-FX Param1", 0.0, 127.0, 0.0, ""),
            (13, "Pre-FX", "PRE-FX Param2", 0.0, 127.0, 0.0, ""),
            (93, "Pre-FX", "PRE-FX Mix", 0.0, 127.0, 0.0, "%"),
            (14, "Delay", "Delay Feedback", 0.0, 127.0, 0.0, "%"),
            (15, "Delay", "Delay Time", 0.0, 127.0, 64.0, "ms"),
            (63, "Delay", "Delay Wet Tone", 0.0, 127.0, 64.0, ""),
            (92, "Delay", "Delay Dry/Wet", 0.0, 127.0, 0.0, "%"),
            (65, "Reverb", "Reverb Time", 0.0, 127.0, 64.0, "ms"),
            (67, "Reverb", "Reverb Tone", 0.0, 127.0, 64.0, ""),
            (91, "Reverb", "Reverb Dry/Wet", 0.0, 127.0, 0.0, "%"),
            (68, "Post-FX", "POST-FX Param1", 0.0, 127.0, 0.0, ""),
            (69, "Post-FX", "POST-FX Param2", 0.0, 127.0, 0.0, ""),
            (94, "Post-FX", "POST-FX Mix", 0.0, 127.0, 0.0, "%"),
            // Arpeggiator
            (106, "ARP", "ARP Division", 0.0, 127.0, 0.0, "div"),
            (107, "ARP", "ARP Gate", 0.0, 127.0, 64.0, "%"),
            (108, "ARP", "ARP Mode", 0.0, 127.0, 0.0, "mode"),
            (109, "ARP", "ARP Ratchet", 0.0, 127.0, 0.0, "ratchet"),
            (110, "ARP", "ARP Chance", 0.0, 127.0, 127.0, "%"),
            (120, "ARP", "ARP Octave", 0.0, 127.0, 0.0, "oct"),
            (122, "ARP", "ARP Length", 0.0, 127.0, 0.0, "len"),
        ];

        for &(cc, module, param_name, min, max, def_val, unit) in cc_entries {
            let range = ValueRange::new(min, max, def_val, 1.0, unit)
                .unwrap_or_else(|_| ValueRange::midi_cc7(def_val));
            let mut desc = ParamDescriptor::new(param_name, module, ParamAddress::cc7(cc), range);
            if module.contains("Filter") && param_name.contains("Cutoff") {
                desc = desc.with_curve(ResponseCurve::Exponential);
            }
            p.add_parameter(desc);
        }

        p
    }

    /// Korg nanoKONTROL2 surface profile featuring 8 channel strips with rotary knobs,
    /// linear sliders, solo/mute/rec buttons with LED feedback, and DAW transport controls.
    pub fn korg_nanokontrol2() -> Self {
        let mut p = Self::new(
            "korg_nanokontrol2",
            "Korg nanoKONTROL2",
            "Korg",
            "1.0",
            DeviceCategory::ControllerSurface,
        )
        .with_default_channel(0);

        p.add_tag("usb_controller");
        p.add_tag("8_faders");
        p.add_tag("led_feedback");

        // 8 Channel Strips: Sliders CC 0..7, Knobs CC 16..23, Solo CC 32..39, Mute CC 48..55, Rec CC 64..71
        for i in 0..8 {
            let slider_cc = i as u8;
            p.add_control(
                PhysicalControlDescriptor::new(
                    format!("slider_{}", i + 1),
                    format!("Slider {}", i + 1),
                    ControlElementKind::LinearSlider,
                    slider_cc,
                    "Sliders",
                )
                .with_track_index(i),
            );

            let knob_cc = 16 + i as u8;
            p.add_control(
                PhysicalControlDescriptor::new(
                    format!("knob_{}", i + 1),
                    format!("Knob {}", i + 1),
                    ControlElementKind::RotaryKnob,
                    knob_cc,
                    "Knobs",
                )
                .with_track_index(i),
            );

            let solo_cc = 32 + i as u8;
            p.add_control(
                PhysicalControlDescriptor::new(
                    format!("solo_{}", i + 1),
                    format!("Solo {}", i + 1),
                    ControlElementKind::PushButton,
                    solo_cc,
                    "Solo",
                )
                .with_led(solo_cc)
                .with_track_index(i),
            );

            let mute_cc = 48 + i as u8;
            p.add_control(
                PhysicalControlDescriptor::new(
                    format!("mute_{}", i + 1),
                    format!("Mute {}", i + 1),
                    ControlElementKind::PushButton,
                    mute_cc,
                    "Mute",
                )
                .with_led(mute_cc)
                .with_track_index(i),
            );

            let rec_cc = 64 + i as u8;
            p.add_control(
                PhysicalControlDescriptor::new(
                    format!("rec_{}", i + 1),
                    format!("Rec {}", i + 1),
                    ControlElementKind::PushButton,
                    rec_cc,
                    "Rec",
                )
                .with_led(rec_cc)
                .with_track_index(i),
            );
        }

        // Transport Controls
        let transport_defs = &[
            ("track_prev", "Track Previous", 58, false),
            ("track_next", "Track Next", 59, false),
            ("cycle", "Cycle", 46, true),
            ("marker_set", "Marker Set", 60, false),
            ("marker_prev", "Marker Previous", 61, false),
            ("marker_next", "Marker Next", 62, false),
            ("rewind", "Rewind", 43, true),
            ("fast_forward", "Fast Forward", 44, true),
            ("stop", "Stop", 42, true),
            ("play", "Play", 41, true),
            ("record", "Record", 45, true),
        ];

        for &(id, name, cc, has_led) in transport_defs {
            let mut ctrl = PhysicalControlDescriptor::new(
                id,
                name,
                ControlElementKind::PushButton,
                cc,
                "Transport",
            );
            if has_led {
                ctrl = ctrl.with_led(cc);
            }
            p.add_control(ctrl);
        }

        p
    }

    /// Akai MIDImix surface profile featuring 8 channel strips with 3 knobs each,
    /// 8 channel faders, 1 master fader, mute buttons, and rec arm buttons.
    pub fn akai_midimix() -> Self {
        let mut p = Self::new(
            "akai_midimix",
            "Akai MIDImix",
            "Akai Professional",
            "1.0",
            DeviceCategory::ControllerSurface,
        )
        .with_default_channel(0);

        p.add_tag("usb_controller");
        p.add_tag("24_knobs");
        p.add_tag("master_fader");

        let knob_top_ccs = [16, 20, 24, 28, 46, 50, 54, 58];
        let knob_mid_ccs = [17, 21, 25, 29, 47, 51, 55, 59];
        let knob_low_ccs = [18, 22, 26, 30, 48, 52, 56, 60];
        let fader_ccs = [19, 23, 27, 31, 49, 53, 57, 61];
        let mute_ccs = [1, 4, 7, 10, 13, 16, 19, 22];
        let rec_ccs = [3, 6, 9, 12, 15, 18, 21, 24];

        for i in 0..8 {
            p.add_control(
                PhysicalControlDescriptor::new(
                    format!("knob_top_{}", i + 1),
                    format!("Gain {}", i + 1),
                    ControlElementKind::RotaryKnob,
                    knob_top_ccs[i],
                    "Knobs Top",
                )
                .with_track_index(i),
            );

            p.add_control(
                PhysicalControlDescriptor::new(
                    format!("knob_mid_{}", i + 1),
                    format!("Send {}", i + 1),
                    ControlElementKind::RotaryKnob,
                    knob_mid_ccs[i],
                    "Knobs Mid",
                )
                .with_track_index(i),
            );

            p.add_control(
                PhysicalControlDescriptor::new(
                    format!("knob_low_{}", i + 1),
                    format!("Pan {}", i + 1),
                    ControlElementKind::RotaryKnob,
                    knob_low_ccs[i],
                    "Knobs Low",
                )
                .with_track_index(i),
            );

            p.add_control(
                PhysicalControlDescriptor::new(
                    format!("fader_{}", i + 1),
                    format!("Fader {}", i + 1),
                    ControlElementKind::LinearSlider,
                    fader_ccs[i],
                    "Faders",
                )
                .with_track_index(i),
            );

            p.add_control(
                PhysicalControlDescriptor::new(
                    format!("mute_{}", i + 1),
                    format!("Mute {}", i + 1),
                    ControlElementKind::PushButton,
                    mute_ccs[i],
                    "Mute",
                )
                .with_led(mute_ccs[i])
                .with_track_index(i),
            );

            p.add_control(
                PhysicalControlDescriptor::new(
                    format!("rec_arm_{}", i + 1),
                    format!("Rec Arm {}", i + 1),
                    ControlElementKind::PushButton,
                    rec_ccs[i],
                    "Rec Arm",
                )
                .with_led(rec_ccs[i])
                .with_track_index(i),
            );
        }

        // Master Fader
        p.add_control(PhysicalControlDescriptor::new(
            "master_fader",
            "Master Fader",
            ControlElementKind::LinearSlider,
            62,
            "Master",
        ));

        // Bank Switch Buttons
        p.add_control(PhysicalControlDescriptor::new(
            "bank_left",
            "Bank Left",
            ControlElementKind::PushButton,
            25,
            "Bank",
        ));
        p.add_control(PhysicalControlDescriptor::new(
            "bank_right",
            "Bank Right",
            ControlElementKind::PushButton,
            26,
            "Bank",
        ));

        p
    }

    /// Novation Bass Station II profile covering dual analog oscillators, sub-oscillator,
    /// multi-mode filter with 14-bit Cutoff resolution, dual envelopes, dual LFOs,
    /// distortion, oscillator filter modulation, and AFX Mode key overlay support.
    pub fn novation_bass_station_2() -> Self {
        let mut p = Self::new(
            "novation_bass_station_2",
            "Novation Bass Station II",
            "Novation",
            "2.5",
            DeviceCategory::Synthesizer,
        )
        .with_default_channel(0);

        p.add_tag("analog_monosynth");
        p.add_tag("14bit_cutoff");
        p.add_tag("afx_mode");

        let step_range = |min: f64, max: f64, def: f64, step: f64, unit: &str| {
            ValueRange::new(min, max, def, step, unit)
                .unwrap_or_else(|_| ValueRange::midi_cc7(def))
        };

        // Oscillator 1
        p.add_parameter(ParamDescriptor::new("osc1_range", "Oscillator 1", ParamAddress::cc7(70), step_range(0.0, 3.0, 1.0, 1.0, "oct")));
        p.add_parameter(ParamDescriptor::new("osc1_fine_tune", "Oscillator 1", ParamAddress::cc7(26), ValueRange::bipolar(-100.0, 100.0, 0.0, "cents")));
        p.add_parameter(ParamDescriptor::new("osc1_coarse_tune", "Oscillator 1", ParamAddress::cc7(27), ValueRange::bipolar(-12.0, 12.0, 0.0, "st")));
        p.add_parameter(ParamDescriptor::new("osc1_mod_env_depth", "Oscillator 1", ParamAddress::cc7(71), ValueRange::bipolar(-64.0, 63.0, 0.0, "")));
        p.add_parameter(ParamDescriptor::new("osc1_lfo1_depth", "Oscillator 1", ParamAddress::cc7(28), ValueRange::midi_cc7(0.0)));
        p.add_parameter(ParamDescriptor::new("osc1_mod_env_pw", "Oscillator 1", ParamAddress::cc7(72), ValueRange::bipolar(-64.0, 63.0, 0.0, "")));
        p.add_parameter(ParamDescriptor::new("osc1_lfo2_pw", "Oscillator 1", ParamAddress::cc7(73), ValueRange::midi_cc7(0.0)));
        p.add_parameter(ParamDescriptor::new("osc1_manual_pw", "Oscillator 1", ParamAddress::cc7(74), ValueRange::midi_cc7(64.0)));
        p.add_parameter(ParamDescriptor::new("osc1_waveform", "Oscillator 1", ParamAddress::nrpn(0, 72), step_range(0.0, 3.0, 2.0, 1.0, "wave")));

        // Oscillator 2
        p.add_parameter(ParamDescriptor::new("osc2_range", "Oscillator 2", ParamAddress::cc7(75), step_range(0.0, 3.0, 1.0, 1.0, "oct")));
        p.add_parameter(ParamDescriptor::new("osc2_fine_tune", "Oscillator 2", ParamAddress::cc7(29), ValueRange::bipolar(-100.0, 100.0, 0.0, "cents")));
        p.add_parameter(ParamDescriptor::new("osc2_coarse_tune", "Oscillator 2", ParamAddress::cc7(30), ValueRange::bipolar(-12.0, 12.0, 0.0, "st")));
        p.add_parameter(ParamDescriptor::new("osc2_mod_env_depth", "Oscillator 2", ParamAddress::cc7(76), ValueRange::bipolar(-64.0, 63.0, 0.0, "")));
        p.add_parameter(ParamDescriptor::new("osc2_lfo1_depth", "Oscillator 2", ParamAddress::cc7(31), ValueRange::midi_cc7(0.0)));
        p.add_parameter(ParamDescriptor::new("osc2_mod_env_pw", "Oscillator 2", ParamAddress::cc7(77), ValueRange::bipolar(-64.0, 63.0, 0.0, "")));
        p.add_parameter(ParamDescriptor::new("osc2_lfo2_pw", "Oscillator 2", ParamAddress::cc7(78), ValueRange::midi_cc7(0.0)));
        p.add_parameter(ParamDescriptor::new("osc2_manual_pw", "Oscillator 2", ParamAddress::cc7(79), ValueRange::midi_cc7(64.0)));
        p.add_parameter(ParamDescriptor::new("osc2_waveform", "Oscillator 2", ParamAddress::nrpn(0, 82), step_range(0.0, 3.0, 3.0, 1.0, "wave")));
        p.add_parameter(ParamDescriptor::new("osc_sync", "Oscillator 2", ParamAddress::cc7(110), step_range(0.0, 1.0, 0.0, 1.0, "")));

        // Mixer
        p.add_parameter(ParamDescriptor::new("sub_osc_octave", "Mixer", ParamAddress::cc7(80), step_range(0.0, 1.0, 0.0, 1.0, "oct")));
        p.add_parameter(ParamDescriptor::new("sub_osc_wave", "Mixer", ParamAddress::cc7(81), step_range(0.0, 2.0, 0.0, 1.0, "wave")));
        p.add_parameter(ParamDescriptor::new("osc1_level", "Mixer", ParamAddress::cc7(20), ValueRange::midi_cc7(127.0)));
        p.add_parameter(ParamDescriptor::new("osc2_level", "Mixer", ParamAddress::cc7(21), ValueRange::midi_cc7(0.0)));
        p.add_parameter(ParamDescriptor::new("sub_osc_level", "Mixer", ParamAddress::cc7(22), ValueRange::midi_cc7(0.0)));
        p.add_parameter(ParamDescriptor::new("noise_level", "Mixer", ParamAddress::cc7(23), ValueRange::midi_cc7(0.0)));
        p.add_parameter(ParamDescriptor::new("ring_mod_level", "Mixer", ParamAddress::cc7(24), ValueRange::midi_cc7(0.0)));

        // Filter
        p.add_parameter(ParamDescriptor::new("filter_type", "Filter", ParamAddress::cc7(83), step_range(0.0, 1.0, 0.0, 1.0, "type")));
        p.add_parameter(ParamDescriptor::new("filter_slope", "Filter", ParamAddress::cc7(106), step_range(0.0, 1.0, 1.0, 1.0, "slope")));
        p.add_parameter(ParamDescriptor::new("filter_shape", "Filter", ParamAddress::cc7(84), step_range(0.0, 2.0, 0.0, 1.0, "shape")));
        p.add_parameter(ParamDescriptor::new("filter_cutoff", "Filter", ParamAddress::cc14(16, 48), ValueRange::midi_cc14(12000.0)).with_curve(ResponseCurve::Exponential));
        p.add_parameter(ParamDescriptor::new("filter_resonance", "Filter", ParamAddress::cc7(82), ValueRange::midi_cc7(0.0)));
        p.add_parameter(ParamDescriptor::new("filter_overdrive", "Filter", ParamAddress::cc7(114), ValueRange::midi_cc7(0.0)));
        p.add_parameter(ParamDescriptor::new("filter_mod_env_depth", "Filter", ParamAddress::cc7(85), ValueRange::bipolar(-64.0, 63.0, 0.0, "")));
        p.add_parameter(ParamDescriptor::new("filter_lfo2_depth", "Filter", ParamAddress::cc7(17), ValueRange::bipolar(-64.0, 63.0, 0.0, "")));

        // Envelopes
        p.add_parameter(ParamDescriptor::new("amp_attack", "Envelopes", ParamAddress::cc7(90), ValueRange::midi_cc7(0.0)));
        p.add_parameter(ParamDescriptor::new("amp_decay", "Envelopes", ParamAddress::cc7(91), ValueRange::midi_cc7(60.0)));
        p.add_parameter(ParamDescriptor::new("amp_sustain", "Envelopes", ParamAddress::cc7(92), ValueRange::midi_cc7(127.0)));
        p.add_parameter(ParamDescriptor::new("amp_release", "Envelopes", ParamAddress::cc7(93), ValueRange::midi_cc7(20.0)));
        p.add_parameter(ParamDescriptor::new("mod_attack", "Envelopes", ParamAddress::cc7(102), ValueRange::midi_cc7(0.0)));
        p.add_parameter(ParamDescriptor::new("mod_decay", "Envelopes", ParamAddress::cc7(103), ValueRange::midi_cc7(60.0)));
        p.add_parameter(ParamDescriptor::new("mod_sustain", "Envelopes", ParamAddress::cc7(104), ValueRange::midi_cc7(0.0)));
        p.add_parameter(ParamDescriptor::new("mod_release", "Envelopes", ParamAddress::cc7(105), ValueRange::midi_cc7(20.0)));

        // LFOs
        p.add_parameter(ParamDescriptor::new("lfo1_speed", "LFO 1", ParamAddress::cc7(18), ValueRange::midi_cc7(64.0)));
        p.add_parameter(ParamDescriptor::new("lfo1_delay", "LFO 1", ParamAddress::cc7(86), ValueRange::midi_cc7(0.0)));
        p.add_parameter(ParamDescriptor::new("lfo1_wave", "LFO 1", ParamAddress::cc7(88), step_range(0.0, 3.0, 0.0, 1.0, "wave")));
        p.add_parameter(ParamDescriptor::new("lfo2_speed", "LFO 2", ParamAddress::cc7(19), ValueRange::midi_cc7(64.0)));
        p.add_parameter(ParamDescriptor::new("lfo2_delay", "LFO 2", ParamAddress::cc7(87), ValueRange::midi_cc7(0.0)));
        p.add_parameter(ParamDescriptor::new("lfo2_wave", "LFO 2", ParamAddress::cc7(89), step_range(0.0, 3.0, 0.0, 1.0, "wave")));

        // Effects & Extended
        p.add_parameter(ParamDescriptor::new("distortion", "Effects", ParamAddress::cc7(94), ValueRange::midi_cc7(0.0)));
        p.add_parameter(ParamDescriptor::new("osc_filter_mod", "Effects", ParamAddress::cc7(115), ValueRange::midi_cc7(0.0)));
        p.add_parameter(ParamDescriptor::new("portamento_time", "Portamento", ParamAddress::cc7(5), ValueRange::midi_cc7(0.0)));

        p
    }

    /// Behringer TD-3-MD profile covering the classic diode ladder bass engine
    /// plus modded-out extended controls (sub-oscillator, overdrive, filter FM, muffler,
    /// independent normal and accent decay, variable slide time, and sweep speed).
    pub fn behringer_td3_md() -> Self {
        let mut p = Self::new(
            "behringer_td3_md",
            "Behringer TD-3-MD",
            "Behringer",
            "1.2",
            DeviceCategory::Synthesizer,
        )
        .with_default_channel(1);

        p.add_tag("acid_bass");
        p.add_tag("diode_ladder");
        p.add_tag("modded_out");

        let step_range = |min: f64, max: f64, def: f64, step: f64, unit: &str| {
            ValueRange::new(min, max, def, step, unit)
                .unwrap_or_else(|_| ValueRange::midi_cc7(def))
        };

        p.add_parameter(ParamDescriptor::new("waveform", "Oscillator", ParamAddress::cc7(70), step_range(0.0, 1.0, 0.0, 1.0, "Saw/Square")));
        p.add_parameter(ParamDescriptor::new("tune", "Oscillator", ParamAddress::cc7(75), ValueRange::bipolar(-12.0, 12.0, 0.0, "st")));
        p.add_parameter(ParamDescriptor::new("filter_cutoff", "Filter", ParamAddress::cc7(74), ValueRange::midi_cc7(64.0)).with_curve(ResponseCurve::Exponential));
        p.add_parameter(ParamDescriptor::new("filter_resonance", "Filter", ParamAddress::cc7(71), ValueRange::midi_cc7(80.0)));
        p.add_parameter(ParamDescriptor::new("filter_env_mod", "Filter", ParamAddress::cc7(12), ValueRange::midi_cc7(75.0)));
        p.add_parameter(ParamDescriptor::new("filter_normal_decay", "Envelope", ParamAddress::cc7(72), ValueRange::midi_cc7(64.0)).with_curve(ResponseCurve::Exponential));
        p.add_parameter(ParamDescriptor::new("filter_accent_decay", "Envelope", ParamAddress::cc7(76), ValueRange::midi_cc7(64.0)).with_curve(ResponseCurve::Exponential));
        p.add_parameter(ParamDescriptor::new("filter_accent_level", "Envelope", ParamAddress::cc7(13), ValueRange::midi_cc7(85.0)));
        p.add_parameter(ParamDescriptor::new("accent_velocity_threshold", "Performance", ParamAddress::cc7(64), ValueRange::new(1.0, 127.0, 100.0, 1.0, "").unwrap_or_else(|_| ValueRange::midi_cc7(100.0))));
        p.add_parameter(ParamDescriptor::new("slide_time", "Performance", ParamAddress::cc7(5), ValueRange::midi_cc7(48.0)).with_curve(ResponseCurve::Exponential));
        p.add_parameter(ParamDescriptor::new("sub_osc_level", "SubOscillator", ParamAddress::cc7(21), ValueRange::midi_cc7(0.0)));
        p.add_parameter(ParamDescriptor::new("sub_osc_octave", "SubOscillator", ParamAddress::cc7(22), step_range(0.0, 2.0, 0.0, 1.0, "Mode")));
        p.add_parameter(ParamDescriptor::new("overdrive_drive", "Overdrive", ParamAddress::cc7(94), ValueRange::midi_cc7(0.0)));
        p.add_parameter(ParamDescriptor::new("overdrive_tone", "Overdrive", ParamAddress::cc7(95), ValueRange::midi_cc7(64.0)));
        p.add_parameter(ParamDescriptor::new("filter_fm_depth", "Filter", ParamAddress::cc7(115), ValueRange::midi_cc7(0.0)));
        p.add_parameter(ParamDescriptor::new("muffler_mode", "Overdrive", ParamAddress::cc7(83), step_range(0.0, 2.0, 0.0, 1.0, "Mode")));
        p.add_parameter(ParamDescriptor::new("soft_attack", "Envelope", ParamAddress::cc7(73), ValueRange::midi_cc7(0.0)).with_curve(ResponseCurve::Exponential));
        p.add_parameter(ParamDescriptor::new("filter_tracking", "Filter", ParamAddress::cc7(77), ValueRange::midi_cc7(40.0)));
        p.add_parameter(ParamDescriptor::new("accent_sweep_speed", "Envelope", ParamAddress::cc7(78), step_range(0.0, 2.0, 1.0, 1.0, "Speed")));

        p
    }

    /// Clavia Nord Drum 2 profile covering 6 drum voices (Tone, Noise, Click, Mix, EQ, Distortion/Crush, Echo, Choke Group)
    /// totaling 186 synthesis parameters (31 parameters per voice across 6 voices) plus 16 Global Kit parameters.
    pub fn clavia_nord_drum_2() -> Self {
        let mut p = Self::new(
            "clavia_nord_drum_2",
            "Clavia Nord Drum 2",
            "Clavia",
            "3.00",
            DeviceCategory::Synthesizer,
        )
        .with_default_channel(10);

        p.add_tag("drum_synthesizer");
        p.add_tag("physical_modeling");
        p.add_tag("percussion");
        p.add_tag("6_channel");

        // Add 31 parameters for each of the 6 voices (186 total)
        for voice in 1..=6 {
            let prefix = format!("voice_{}", voice);
            let tone_sec = format!("Voice {} Tone", voice);
            let noise_sec = format!("Voice {} Noise", voice);
            let click_sec = format!("Voice {} Click", voice);
            let mix_sec = format!("Voice {} Mix", voice);
            let fx_sec = format!("Voice {} FX", voice);

            // 1. Tone Wave (CC 46)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_tone_wave", prefix),
                &tone_sec,
                ParamAddress::cc7(46),
                ValueRange::midi_cc7(0.0),
            ));
            // 2. Tone Pitch MSB (CC 30)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_tone_pitch_msb", prefix),
                &tone_sec,
                ParamAddress::cc7(30),
                ValueRange::midi_cc7(60.0),
            ));
            // 3. Tone Pitch LSB (CC 62)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_tone_pitch_lsb", prefix),
                &tone_sec,
                ParamAddress::cc7(62),
                ValueRange::midi_cc7(64.0),
            ));
            // 4. Tone Spectra (CC 52)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_tone_spectra", prefix),
                &tone_sec,
                ParamAddress::cc7(52),
                ValueRange::midi_cc7(64.0),
            ));
            // 5. Tone Timbre (CC 53)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_tone_timbre", prefix),
                &tone_sec,
                ParamAddress::cc7(53),
                ValueRange::midi_cc7(64.0),
            ));
            // 6. Tone Timbre Env (CC 54)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_tone_timbre_env", prefix),
                &tone_sec,
                ParamAddress::cc7(54),
                ValueRange::midi_cc7(64.0),
            ));
            // 7. Tone Timbre Decay (CC 49)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_tone_timbre_decay", prefix),
                &tone_sec,
                ParamAddress::cc7(49),
                ValueRange::midi_cc7(64.0),
            ));
            // 8. Tone Decay (CC 50)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_tone_decay", prefix),
                &tone_sec,
                ParamAddress::cc7(50),
                ValueRange::midi_cc7(64.0),
            ));
            // 9. Tone Decay Mode (CC 48)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_tone_decay_mode", prefix),
                &tone_sec,
                ParamAddress::cc7(48),
                ValueRange::midi_cc7(0.0),
            ));
            // 10. Tone Bend Amount (CC 55)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_tone_bend_amount", prefix),
                &tone_sec,
                ParamAddress::cc7(55),
                ValueRange::midi_cc7(64.0),
            ));
            // 11. Tone Bend Time (CC 56)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_tone_bend_time", prefix),
                &tone_sec,
                ParamAddress::cc7(56),
                ValueRange::midi_cc7(32.0),
            ));
            // 12. Tone Punch (CC 57)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_tone_punch", prefix),
                &tone_sec,
                ParamAddress::cc7(57),
                ValueRange::midi_cc7(0.0),
            ));

            // 13. Noise Filter Freq (CC 14)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_noise_filter_freq", prefix),
                &noise_sec,
                ParamAddress::cc7(14),
                ValueRange::midi_cc7(64.0),
            ));
            // 14. Noise Filter Type (CC 15)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_noise_filter_type", prefix),
                &noise_sec,
                ParamAddress::cc7(15),
                ValueRange::midi_cc7(0.0),
            ));
            // 15. Noise Filter Env (CC 16)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_noise_filter_env", prefix),
                &noise_sec,
                ParamAddress::cc7(16),
                ValueRange::midi_cc7(64.0),
            ));
            // 16. Noise Resonance (CC 17)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_noise_resonance", prefix),
                &noise_sec,
                ParamAddress::cc7(17),
                ValueRange::midi_cc7(0.0),
            ));
            // 17. Noise Attack Rate (CC 18)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_noise_attack_rate", prefix),
                &noise_sec,
                ParamAddress::cc7(18),
                ValueRange::midi_cc7(0.0),
            ));
            // 18. Noise Decay Mode (CC 19)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_noise_decay_mode", prefix),
                &noise_sec,
                ParamAddress::cc7(19),
                ValueRange::midi_cc7(0.0),
            ));
            // 19. Noise Decay (CC 21)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_noise_decay", prefix),
                &noise_sec,
                ParamAddress::cc7(21),
                ValueRange::midi_cc7(64.0),
            ));
            // 20. Noise Decay Lo Dyn (CC 20)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_noise_decay_lo_dyn", prefix),
                &noise_sec,
                ParamAddress::cc7(20),
                ValueRange::midi_cc7(64.0),
            ));

            // 21. Click Type (CC 58)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_click_type", prefix),
                &click_sec,
                ParamAddress::cc7(58),
                ValueRange::midi_cc7(0.0),
            ));
            // 22. Click Level (CC 59)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_click_level", prefix),
                &click_sec,
                ParamAddress::cc7(59),
                ValueRange::midi_cc7(64.0),
            ));

            // 23. Mix Balance (CC 23)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_mix_balance", prefix),
                &mix_sec,
                ParamAddress::cc7(23),
                ValueRange::midi_cc7(64.0),
            ));
            // 24. Distortion Amount (CC 24)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_distortion_amount", prefix),
                &fx_sec,
                ParamAddress::cc7(24),
                ValueRange::midi_cc7(0.0),
            ));
            // 25. Distortion Type (CC 25)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_distortion_type", prefix),
                &fx_sec,
                ParamAddress::cc7(25),
                ValueRange::midi_cc7(0.0),
            ));
            // 26. EQ Frequency (CC 26)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_eq_frequency", prefix),
                &fx_sec,
                ParamAddress::cc7(26),
                ValueRange::midi_cc7(64.0),
            ));
            // 27. EQ Gain (CC 27)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_eq_gain", prefix),
                &fx_sec,
                ParamAddress::cc7(27),
                ValueRange::midi_cc7(64.0),
            ));
            // 28. Echo Amount (CC 28)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_echo_amount", prefix),
                &fx_sec,
                ParamAddress::cc7(28),
                ValueRange::midi_cc7(0.0),
            ));
            // 29. Echo Feedback (CC 29)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_echo_feedback", prefix),
                &fx_sec,
                ParamAddress::cc7(29),
                ValueRange::midi_cc7(0.0),
            ));
            // 30. Echo Time MSB (CC 60)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_echo_time_msb", prefix),
                &fx_sec,
                ParamAddress::cc7(60),
                ValueRange::midi_cc7(64.0),
            ));
            // 31. Choke Group (CC 63)
            p.add_parameter(ParamDescriptor::new(
                format!("{}_choke_group", prefix),
                &fx_sec,
                ParamAddress::cc7(63),
                ValueRange::midi_cc7(0.0),
            ));
        }

        // Global Kit parameters (16 total: global_channel, global_focus_cc, kit_bank, kit_program, 6 voice channels, 6 trigger notes)
        p.add_parameter(ParamDescriptor::new(
            "global_channel",
            "Global",
            ParamAddress::none(),
            ValueRange::new(1.0, 16.0, 10.0, 1.0, "ch").unwrap_or_else(|_| ValueRange::midi_cc7(10.0)),
        ));
        p.add_parameter(ParamDescriptor::new(
            "global_focus_cc",
            "Global",
            ParamAddress::cc7(70),
            ValueRange::midi_cc7(70.0),
        ));
        p.add_parameter(ParamDescriptor::new(
            "kit_bank",
            "Global",
            ParamAddress::none(),
            ValueRange::new(0.0, 7.0, 0.0, 1.0, "bank").unwrap_or_else(|_| ValueRange::midi_cc7(0.0)),
        ));
        p.add_parameter(ParamDescriptor::new(
            "kit_program",
            "Global",
            ParamAddress::none(),
            ValueRange::new(0.0, 49.0, 0.0, 1.0, "prog").unwrap_or_else(|_| ValueRange::midi_cc7(0.0)),
        ));
        for voice in 1..=6 {
            p.add_parameter(ParamDescriptor::new(
                format!("voice_{}_channel", voice),
                "Global",
                ParamAddress::none(),
                ValueRange::new(-1.0, 16.0, voice as f64, 1.0, "ch").unwrap_or_else(|_| ValueRange::midi_cc7(voice as f64)),
            ));
            let default_note = match voice {
                1 => 60.0,
                2 => 62.0,
                3 => 64.0,
                4 => 65.0,
                5 => 67.0,
                _ => 69.0,
            };
            p.add_parameter(ParamDescriptor::new(
                format!("voice_{}_trigger_note", voice),
                "Global",
                ParamAddress::none(),
                ValueRange::new(0.0, 127.0, default_note, 1.0, "note").unwrap_or_else(|_| ValueRange::midi_cc7(default_note)),
            ));
        }

        // Physical Hardware Panel Controls
        for voice in 1..=6 {
            p.add_control(
                PhysicalControlDescriptor::new(
                    format!("ch_btn_{}", voice),
                    format!("Channel {} Button", voice),
                    ControlElementKind::PushButton,
                    70 + (voice as u8) - 1,
                    "Voice Select",
                )
                .with_track_index(voice - 1),
            );
        }
        p.add_control(PhysicalControlDescriptor::new(
            "main_dial",
            "Value Dial / Encoder",
            ControlElementKind::Encoder,
            0,
            "Navigation",
        ));
        p.add_control(PhysicalControlDescriptor::new(
            "trig_button",
            "Trig Audition Button",
            ControlElementKind::PushButton,
            0,
            "Audition",
        ));

        p
    }

    /// Novation Peak 8-Voice Polyphonic Hybrid Synthesizer profile derived from
    /// `js/konduktiva-peak/midi-cc-map.json` (`1235:007e`, factory default MIDI Channel 1).
    ///
    /// Covers all 74 7-bit MIDI Continuous Controller parameters across:
    /// - System & Mod Wheel (CC 1)
    /// - Voice & Glide (CC 5, 35)
    /// - New Oxford NCO Oscillators 1, 2, 3 (CC 3, 9, 12, 14..22, 33, 34, 37..44, 65, 71..73, 119)
    /// - Mixer with Ring Mod & Noise (CC 23..27)
    /// - State-Variable OTA Analogue Filter with Pre/Post Drive (CC 28, 29, 36, 75, 76, 78, 79, 80)
    /// - LFO 1 & LFO 2 with Sync & Fade (CC 30, 31, 81..85)
    /// - Amp Envelope & Mod Envelopes 1 & 2 (CC 86..95, 103, 117)
    /// - Analogue Distortion & Digital FX: Chorus, Delay, Reverb (CC 104, 105, 107..110, 112, 113, 118)
    /// - Animate 1 & 2 Performance Hold Macros & Arpeggiator Gate (CC 114, 115, 116)
    pub fn novation_peak() -> Self {
        let mut p = Self::new(
            "novation_peak",
            "Novation Peak",
            "Focusrite-Novation",
            "2.1",
            DeviceCategory::Synthesizer,
        )
        .with_default_channel(0); // 0-indexed wire channel 0 = MIDI Channel 1

        p.add_tag("hybrid_polyphonic");
        p.add_tag("8_voice");
        p.add_tag("new_oxford_oscillators");
        p.add_tag("ota_multimode_filter");
        p.add_tag("konduktiva_companion");

        let cc_entries: &[(u8, &str, &str, f64, &str)] = &[
            (1, "System", "Mod wheel (modulation)", 0.0, ""),
            (3, "Osc 1", "Osc 1 range", 64.0, "oct"),
            (5, "Voice", "Glide time", 0.0, "ms"),
            (9, "Osc 1", "Osc 1 mod 2 pitch", 64.0, ""),
            (12, "Osc 1", "Osc 1 shape", 64.0, ""),
            (14, "Osc 1", "Osc 1 coarse", 64.0, "st"),
            (15, "Osc 1", "Osc 1 fine", 64.0, "cents"),
            (16, "Osc 1", "Osc 1 LFO 2 pitch", 64.0, ""),
            (17, "Osc 2", "Osc 2 coarse", 64.0, "st"),
            (18, "Osc 2", "Osc 2 fine", 64.0, "cents"),
            (19, "Osc 2", "Osc 2 LFO 2 pitch", 64.0, ""),
            (20, "Osc 3", "Osc 3 coarse", 64.0, "st"),
            (21, "Osc 3", "Osc 3 fine", 64.0, "cents"),
            (22, "Osc 3", "Osc 3 LFO 2 pitch", 64.0, ""),
            (23, "Mixer", "Osc 1 mix", 127.0, ""),
            (24, "Mixer", "Osc 2 mix", 0.0, ""),
            (25, "Mixer", "Osc 3 mix", 0.0, ""),
            (26, "Mixer", "Ring mod mix", 0.0, ""),
            (27, "Mixer", "Noise mix", 0.0, ""),
            (28, "Filter", "LFO 1 -> filter depth", 64.0, ""),
            (29, "Filter", "Filter cutoff (Frequency)", 100.0, "Hz"),
            (30, "LFO 1", "LFO 1 rate", 64.0, "Hz"),
            (31, "LFO 2", "LFO 2 rate", 64.0, "Hz"),
            (33, "Osc 1", "Osc 1 LFO 1 shape", 64.0, ""),
            (34, "Osc 1", "Osc 1 vsync", 0.0, ""),
            (35, "Voice", "Glide on", 0.0, ""),
            (36, "Filter", "Filter post drive", 0.0, "dB"),
            (37, "Osc 2", "Osc 2 range", 64.0, "oct"),
            (38, "Osc 2", "Osc 2 mod 2 pitch", 64.0, ""),
            (39, "Osc 2", "Osc 2 shape", 64.0, ""),
            (40, "Osc 2", "Osc 2 mod 1 shape", 64.0, ""),
            (41, "Osc 2", "Osc 2 LFO 1 shape", 64.0, ""),
            (42, "Osc 2", "Osc 2 vsync", 0.0, ""),
            (43, "Osc 3", "Osc 3 mod 2 pitch", 64.0, ""),
            (44, "Osc 3", "Osc 3 vsync", 0.0, ""),
            (65, "Osc 3", "Osc 3 range", 64.0, "oct"),
            (71, "Osc 3", "Osc 3 shape", 64.0, ""),
            (72, "Osc 3", "Osc 3 mod 1 pitch", 64.0, ""),
            (73, "Osc 3", "Osc 3 LFO 1 shape", 64.0, ""),
            (75, "Filter", "Filter track", 64.0, "%"),
            (76, "Filter", "Osc 3 -> filter", 64.0, ""),
            (78, "Filter", "Mod 1 -> filter", 64.0, ""),
            (79, "Filter", "Filter resonance", 16.0, ""),
            (80, "Filter", "Filter overdrive", 0.0, "dB"),
            (81, "LFO 1", "LFO 1 sync rate", 64.0, "div"),
            (82, "LFO 1", "LFO 1 fade", 0.0, "ms"),
            (83, "LFO 2", "LFO 2 range", 0.0, ""),
            (84, "LFO 2", "LFO 2 sync rate", 64.0, "div"),
            (85, "LFO 2", "LFO 2 fade", 0.0, "ms"),
            (86, "Amp Env", "Amp envelope attack", 4.0, "ms"),
            (87, "Amp Env", "Amp envelope decay", 64.0, "ms"),
            (88, "Amp Env", "Amp envelope sustain", 100.0, "%"),
            (89, "Amp Env", "Amp envelope release", 32.0, "ms"),
            (90, "Mod Env 1", "Mod envelope 1 attack", 4.0, "ms"),
            (91, "Mod Env 1", "Mod envelope 1 decay", 64.0, "ms"),
            (92, "Mod Env 1", "Mod envelope 1 sustain", 64.0, "%"),
            (93, "Mod Env 1", "Mod envelope 1 release", 32.0, "ms"),
            (94, "Mod Env 2", "Mod envelope 2 attack", 4.0, "ms"),
            (95, "Mod Env 2", "Mod envelope 2 decay", 64.0, "ms"),
            (103, "Mod Env 2", "Mod envelope 2 release", 32.0, "ms"),
            (104, "FX Distortion", "Distortion level", 0.0, "dB"),
            (105, "FX Chorus", "Chorus level", 0.0, "%"),
            (107, "FX Chorus", "Chorus feedback", 64.0, "%"),
            (108, "FX Delay", "Delay level", 0.0, "%"),
            (109, "FX Delay", "Delay time", 64.0, "ms"),
            (110, "FX Delay", "Delay feedback", 32.0, "%"),
            (112, "FX Reverb", "Reverb level", 0.0, "%"),
            (113, "FX Reverb", "Reverb time", 64.0, "ms"),
            (114, "Animate", "Animate 1 hold", 0.0, ""),
            (115, "Animate", "Animate 2 hold", 0.0, ""),
            (116, "Arp", "Arp clock gate", 64.0, "%"),
            (117, "Mod Env 2", "Mod envelope 2 sustain", 64.0, "%"),
            (118, "FX Chorus", "Chorus rate", 48.0, "Hz"),
            (119, "Osc 1", "Osc 1 mod 1 shape", 64.0, ""),
        ];

        for &(cc, module, param_name, def_val, unit) in cc_entries {
            let range = ValueRange::new(0.0, 127.0, def_val, 1.0, unit)
                .unwrap_or_else(|_| ValueRange::midi_cc7(def_val));
            let mut desc = ParamDescriptor::new(param_name, module, ParamAddress::cc7(cc), range);
            if cc == 29 {
                desc = desc.with_curve(ResponseCurve::Exponential);
            } else if (86..=95).contains(&cc) || cc == 103 {
                desc = desc.with_curve(ResponseCurve::Logarithmic);
            }
            p.add_parameter(desc);
        }

        // Physical Novation Peak Hardware Panel Controls
        p.add_control(PhysicalControlDescriptor::new(
            "cutoff_knob",
            "Filter Frequency Rotary Dial",
            ControlElementKind::RotaryKnob,
            29,
            "Filter",
        ));
        p.add_control(PhysicalControlDescriptor::new(
            "resonance_knob",
            "Filter Resonance Rotary Dial",
            ControlElementKind::RotaryKnob,
            79,
            "Filter",
        ));
        p.add_control(
            PhysicalControlDescriptor::new(
                "animate_1_btn",
                "Animate 1 Hold Button",
                ControlElementKind::PushButton,
                114,
                "Performance",
            )
            .with_led(114),
        );
        p.add_control(
            PhysicalControlDescriptor::new(
                "animate_2_btn",
                "Animate 2 Hold Button",
                ControlElementKind::PushButton,
                115,
                "Performance",
            )
            .with_led(115),
        );
        p.add_control(PhysicalControlDescriptor::new(
            "amp_attack_slider",
            "Amp Env Attack Fader",
            ControlElementKind::LinearSlider,
            86,
            "Envelopes",
        ));
        p.add_control(PhysicalControlDescriptor::new(
            "amp_decay_slider",
            "Amp Env Decay Fader",
            ControlElementKind::LinearSlider,
            87,
            "Envelopes",
        ));
        p.add_control(PhysicalControlDescriptor::new(
            "amp_sustain_slider",
            "Amp Env Sustain Fader",
            ControlElementKind::LinearSlider,
            88,
            "Envelopes",
        ));
        p.add_control(PhysicalControlDescriptor::new(
            "amp_release_slider",
            "Amp Env Release Fader",
            ControlElementKind::LinearSlider,
            89,
            "Envelopes",
        ));

        p
    }
}

// =============================================================================
// Portable JSON Schemas
// =============================================================================

/// Generates the standard JSON Schema (Draft-07 compliant) for `SynthPatch`.
/// Compatible with `reference/asm-preset-manager.js` and `nkEditor`.
pub fn preset_json_schema() -> serde_json::Value {
    serde_json::json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "SynthPatch",
        "description": "Standard cross-project synthesizer patch document",
        "type": "object",
        "required": ["meta", "parameters"],
        "properties": {
            "meta": {
                "type": "object",
                "required": ["id", "name"],
                "properties": {
                    "id": { "type": "string", "description": "Unique UUID v4 or identifier" },
                    "name": { "type": "string", "description": "Patch display name" },
                    "category": { "type": "string", "default": "Init" },
                    "color": { "type": "string", "pattern": "^#[0-9a-fA-F]{6}$", "default": "#888888" },
                    "favorite": { "type": "boolean", "default": false },
                    "bank": { "type": "string", "default": "A" },
                    "slot": { "type": "integer", "minimum": 0, "default": 0 },
                    "createdAt": { "type": "string", "format": "date-time" },
                    "modifiedAt": { "type": "string", "format": "date-time" },
                    "tags": {
                        "type": "array",
                        "items": { "type": "string" },
                        "default": []
                    },
                    "author": { "type": ["string", "null"] },
                    "notes": { "type": ["string", "null"] }
                }
            },
            "parameters": {
                "type": "object",
                "description": "Key-value map of normalized parameter identifiers to floating point values",
                "additionalProperties": { "type": "number" }
            },
            "sysex": {
                "type": "object",
                "properties": {
                    "rawDump": {
                        "type": ["array", "null"],
                        "items": { "type": "integer", "minimum": 0, "maximum": 255 }
                    }
                }
            }
        }
    })
}

/// Generates the standard JSON Schema for `DeviceSynthesizerProfile`.
pub fn device_profile_json_schema() -> serde_json::Value {
    serde_json::json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "DeviceSynthesizerProfile",
        "description": "Device synthesis engine and controller layout specification",
        "type": "object",
        "required": ["id", "name", "manufacturer", "version", "category", "default_midi_channel", "parameters"],
        "properties": {
            "id": { "type": "string" },
            "name": { "type": "string" },
            "manufacturer": { "type": "string" },
            "version": { "type": "string" },
            "category": {
                "type": "string",
                "enum": ["synthesizer", "controller_surface", "hybrid"]
            },
            "default_midi_channel": { "type": "integer", "minimum": 0, "maximum": 15 },
            "parameters": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["id", "name", "module", "address", "range"],
                    "properties": {
                        "id": { "type": "string" },
                        "name": { "type": "string" },
                        "module": { "type": "string" },
                        "address": { "type": "object" },
                        "range": { "type": "object" },
                        "curve": { "type": "string" },
                        "is_locked": { "type": "boolean" }
                    }
                }
            },
            "modules": {
                "type": "array",
                "items": { "type": "string" }
            },
            "controller_controls": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["id", "name", "kind", "cc", "has_led", "group"],
                    "properties": {
                        "id": { "type": "string" },
                        "name": { "type": "string" },
                        "kind": { "type": "string" },
                        "cc": { "type": "integer", "minimum": 0, "maximum": 127 },
                        "channel": { "type": ["integer", "null"] },
                        "has_led": { "type": "boolean" },
                        "led_cc": { "type": ["integer", "null"] },
                        "track_index": { "type": ["integer", "null"] },
                        "group": { "type": "string" }
                    }
                }
            },
            "tags": {
                "type": "array",
                "items": { "type": "string" }
            }
        }
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceControlBinding {
    pub hardware_cc: u8,
    pub label: String,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_device: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_param: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub led_feedback_cc: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceBankMapping {
    pub bank_index: u32,
    pub name: String,
    pub target_mode: String,
    pub controls: Vec<SurfaceControlBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControllerSurfaceMapping {
    pub surface_name: String,
    pub version: String,
    pub active_bank: u32,
    pub banks: Vec<SurfaceBankMapping>,
}

impl ControllerSurfaceMapping {
    #[must_use]
    pub fn nanokontrol2_nord_drum_2() -> Self {
        let controls = vec![
            SurfaceControlBinding {
                hardware_cc: 0,
                label: "V1 Level".to_string(),
                action: "set_param".to_string(),
                target_device: Some("clavia_nord_drum_2".to_string()),
                target_param: Some("voice_1_level".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 16,
                label: "V1 Pan".to_string(),
                action: "set_param".to_string(),
                target_device: Some("clavia_nord_drum_2".to_string()),
                target_param: Some("voice_1_pan".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 32,
                label: "V1 Solo".to_string(),
                action: "solo_toggle".to_string(),
                target_device: Some("clavia_nord_drum_2".to_string()),
                target_param: Some("voice_1_solo".to_string()),
                led_feedback_cc: Some(32),
            },
            SurfaceControlBinding {
                hardware_cc: 48,
                label: "V1 Mute".to_string(),
                action: "mute_toggle".to_string(),
                target_device: Some("clavia_nord_drum_2".to_string()),
                target_param: Some("voice_1_mute".to_string()),
                led_feedback_cc: Some(48),
            },
        ];

        Self {
            surface_name: "Korg nanoKONTROL2".to_string(),
            version: "1.0.0".to_string(),
            active_bank: 1,
            banks: vec![SurfaceBankMapping {
                bank_index: 1,
                name: "Nord Drum 2 Performance Mixer".to_string(),
                target_mode: "MixerAndPads".to_string(),
                controls,
            }],
        }
    }

    /// 3-Bank Korg nanoKONTROL2 tactile surface mapping for the Novation Peak:
    /// - Bank 1: Oscillators 1-3 & Mixer (Knobs = Osc 1-3 Coarse/Shape/VSyn/Detune; Sliders = Osc 1-3 Mix, Ring Mod, Noise, Glide, Mod Wheel, Arp Gate)
    /// - Bank 2: State-Variable OTA Filter, Envelopes & LFOs (Knobs = Cutoff, Resonance, Overdrive, Post Drive, LFO 1/2 Rate & Depth; Sliders = Amp & Mod Env 1 ADSR)
    /// - Bank 3: FX, Animate 1/2 Performance Hold & Konduktiva Live Companion (Knobs = Distortion, Chorus, Delay, Reverb; Sliders = 8 Konduktiva Player Lanes)
    #[must_use]
    pub fn nanokontrol2_novation_peak() -> Self {
        let bank1_controls = vec![
            SurfaceControlBinding {
                hardware_cc: 16,
                label: "Osc 1 Coarse".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("osc_1_coarse".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 17,
                label: "Osc 1 Shape".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("osc_1_shape".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 18,
                label: "Osc 2 Coarse".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("osc_2_coarse".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 19,
                label: "Osc 2 Shape".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("osc_2_shape".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 20,
                label: "Osc 3 Coarse".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("osc_3_coarse".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 21,
                label: "Osc 3 Shape".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("osc_3_shape".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 22,
                label: "Osc 1 VSync".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("osc_1_vsync".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 23,
                label: "Osc 2 VSync".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("osc_2_vsync".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 0,
                label: "Osc 1 Mix".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("osc_1_mix".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 1,
                label: "Osc 2 Mix".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("osc_2_mix".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 2,
                label: "Osc 3 Mix".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("osc_3_mix".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 3,
                label: "Ring Mod Mix".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("ring_mod_mix".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 4,
                label: "Noise Mix".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("noise_mix".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 5,
                label: "Glide Time".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("glide_time".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 6,
                label: "Mod Wheel".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("mod_wheel_modulation".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 7,
                label: "Arp Gate".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("arp_clock_gate".to_string()),
                led_feedback_cc: None,
            },
        ];

        let bank2_controls = vec![
            SurfaceControlBinding {
                hardware_cc: 16,
                label: "Filter Cutoff".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("filter_cutoff_frequency".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 17,
                label: "Filter Resonance".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("filter_resonance".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 18,
                label: "Filter Overdrive".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("filter_overdrive".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 19,
                label: "Filter Post Drive".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("filter_post_drive".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 20,
                label: "LFO 1 Rate".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("lfo_1_rate".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 21,
                label: "LFO 1 -> Filter".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("lfo_1_filter_depth".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 22,
                label: "LFO 2 Rate".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("lfo_2_rate".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 23,
                label: "Mod 1 -> Filter".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("mod_1_filter".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 0,
                label: "Amp Attack".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("amp_envelope_attack".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 1,
                label: "Amp Decay".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("amp_envelope_decay".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 2,
                label: "Amp Sustain".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("amp_envelope_sustain".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 3,
                label: "Amp Release".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("amp_envelope_release".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 4,
                label: "Mod 1 Attack".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("mod_envelope_1_attack".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 5,
                label: "Mod 1 Decay".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("mod_envelope_1_decay".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 6,
                label: "Mod 1 Sustain".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("mod_envelope_1_sustain".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 7,
                label: "Mod 1 Release".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("mod_envelope_1_release".to_string()),
                led_feedback_cc: None,
            },
        ];

        let bank3_controls = vec![
            SurfaceControlBinding {
                hardware_cc: 16,
                label: "Distortion Level".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("distortion_level".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 17,
                label: "Chorus Level".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("chorus_level".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 18,
                label: "Chorus Rate".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("chorus_rate".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 19,
                label: "Chorus Feedback".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("chorus_feedback".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 20,
                label: "Delay Level".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("delay_level".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 21,
                label: "Delay Time".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("delay_time".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 22,
                label: "Delay Feedback".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("delay_feedback".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 23,
                label: "Reverb Level".to_string(),
                action: "set_param".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("reverb_level".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 32,
                label: "Animate 1 Hold".to_string(),
                action: "animate_1_toggle".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("animate_1_hold".to_string()),
                led_feedback_cc: Some(32),
            },
            SurfaceControlBinding {
                hardware_cc: 33,
                label: "Animate 2 Hold".to_string(),
                action: "animate_2_toggle".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("animate_2_hold".to_string()),
                led_feedback_cc: Some(33),
            },
            SurfaceControlBinding {
                hardware_cc: 0,
                label: "Konduktiva Bass Rate".to_string(),
                action: "konduktiva_player_rate".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("player_bass".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 1,
                label: "Konduktiva Chords Rate".to_string(),
                action: "konduktiva_player_rate".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("player_chords".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 2,
                label: "Konduktiva Lead Rate".to_string(),
                action: "konduktiva_player_rate".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("player_lead".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 3,
                label: "Konduktiva Arp Rate".to_string(),
                action: "konduktiva_player_rate".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("player_arp".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 4,
                label: "Konduktiva ccCutoff Rate".to_string(),
                action: "konduktiva_player_rate".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("player_ccCutoff".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 5,
                label: "Konduktiva ccReso Rate".to_string(),
                action: "konduktiva_player_rate".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("player_ccReso".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 6,
                label: "Konduktiva ccMod Rate".to_string(),
                action: "konduktiva_player_rate".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("player_ccMod".to_string()),
                led_feedback_cc: None,
            },
            SurfaceControlBinding {
                hardware_cc: 7,
                label: "Konduktiva ccFx Rate".to_string(),
                action: "konduktiva_player_rate".to_string(),
                target_device: Some("novation_peak".to_string()),
                target_param: Some("player_ccFx".to_string()),
                led_feedback_cc: None,
            },
        ];

        Self {
            surface_name: "Korg nanoKONTROL2".to_string(),
            version: "1.0.0".to_string(),
            active_bank: 1,
            banks: vec![
                SurfaceBankMapping {
                    bank_index: 1,
                    name: "Novation Peak Oscillators & Mixer".to_string(),
                    target_mode: "OscillatorsAndMixer".to_string(),
                    controls: bank1_controls,
                },
                SurfaceBankMapping {
                    bank_index: 2,
                    name: "Novation Peak Filter, Envelopes & LFOs".to_string(),
                    target_mode: "FilterEnvelopesLfos".to_string(),
                    controls: bank2_controls,
                },
                SurfaceBankMapping {
                    bank_index: 3,
                    name: "Novation Peak FX, Animate & Konduktiva Live".to_string(),
                    target_mode: "FxAnimateKonduktiva".to_string(),
                    controls: bank3_controls,
                },
            ],
        }
    }

    pub fn to_json(&self) -> Result<String, SynthError> {
        serde_json::to_string_pretty(self).map_err(|e| SynthError::SerializationError(e.to_string()))
    }

    pub fn from_json(json_str: &str) -> Result<Self, SynthError> {
        serde_json::from_str(json_str).map_err(|e| SynthError::SerializationError(e.to_string()))
    }
}

/// Generates the standard JSON Schema for controller surface bindings and mapping configurations.
pub fn controller_mapping_json_schema() -> serde_json::Value {
    serde_json::json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "ControllerSurfaceMapping",
        "description": "Tactile surface hardware binding configuration across switchable banks",
        "type": "object",
        "required": ["surface_name", "version", "banks"],
        "properties": {
            "surface_name": { "type": "string" },
            "version": { "type": "string" },
            "active_bank": { "type": "integer", "minimum": 1, "maximum": 3 },
            "banks": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["bank_index", "name", "target_mode", "controls"],
                    "properties": {
                        "bank_index": { "type": "integer" },
                        "name": { "type": "string" },
                        "target_mode": { "type": "string" },
                        "controls": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "required": ["hardware_cc", "label", "action"],
                                "properties": {
                                    "hardware_cc": { "type": "integer" },
                                    "label": { "type": "string" },
                                    "action": { "type": "string" },
                                    "target_device": { "type": "string" },
                                    "target_param": { "type": "string" },
                                    "led_feedback_cc": { "type": ["integer", "null"] }
                                }
                            }
                        }
                    }
                }
            }
        }
    })
}

/// Validates and parses a JSON preset string into a `SynthPatch`.
pub fn validate_preset_json(json_str: &str) -> Result<SynthPatch, SynthError> {
    serde_json::from_str(json_str).map_err(|e| SynthError::SerializationError(e.to_string()))
}

/// Validates and parses a JSON profile string into a `DeviceSynthesizerProfile`.
pub fn validate_profile_json(json_str: &str) -> Result<DeviceSynthesizerProfile, SynthError> {
    serde_json::from_str(json_str).map_err(|e| SynthError::SerializationError(e.to_string()))
}

/// Validates and parses a JSON controller surface mapping string into a `ControllerSurfaceMapping`.
pub fn validate_controller_mapping_json(json_str: &str) -> Result<ControllerSurfaceMapping, SynthError> {
    serde_json::from_str(json_str).map_err(|e| SynthError::SerializationError(e.to_string()))
}

/// Exports all portable JSON schemas and canonical device profiles to a directory.
pub fn export_all_schemas(dir: &std::path::Path) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(dir)?;
    if let Ok(json) = serde_json::to_string_pretty(&preset_json_schema()) {
        let _ = std::fs::write(dir.join("synth-patch.schema.json"), json);
    }
    if let Ok(json) = serde_json::to_string_pretty(&device_profile_json_schema()) {
        let _ = std::fs::write(dir.join("device-profile.schema.json"), json);
    }
    if let Ok(json) = serde_json::to_string_pretty(&controller_mapping_json_schema()) {
        let _ = std::fs::write(dir.join("controller-mapping.schema.json"), json);
    }
    if let Ok(json) = DeviceSynthesizerProfile::asm_hydrasynth().to_json() {
        let _ = std::fs::write(dir.join("asm-hydrasynth.profile.json"), json);
    }
    if let Ok(json) = DeviceSynthesizerProfile::korg_nanokontrol2().to_json() {
        let _ = std::fs::write(dir.join("korg-nanokontrol2.profile.json"), json);
    }
    if let Ok(json) = DeviceSynthesizerProfile::akai_midimix().to_json() {
        let _ = std::fs::write(dir.join("akai-midimix.profile.json"), json);
    }
    if let Ok(json) = DeviceSynthesizerProfile::novation_bass_station_2().to_json() {
        let _ = std::fs::write(dir.join("novation-bass-station-2.profile.json"), json);
    }
    if let Ok(json) = DeviceSynthesizerProfile::behringer_td3_md().to_json() {
        let _ = std::fs::write(dir.join("behringer-td3-md.profile.json"), json);
    }
    if let Ok(json) = DeviceSynthesizerProfile::clavia_nord_drum_2().to_json() {
        let _ = std::fs::write(dir.join("clavia-nord-drum-2.profile.json"), json);
    }
    if let Ok(json) = serde_json::to_string_pretty(&ControllerSurfaceMapping::nanokontrol2_nord_drum_2()) {
        let _ = std::fs::write(dir.join("korg-nanokontrol2-nd2.mapping.json"), json);
    }
    if let Ok(json) = DeviceSynthesizerProfile::novation_peak().to_json() {
        let _ = std::fs::write(dir.join("novation-peak.profile.json"), json);
    }
    if let Ok(json) = serde_json::to_string_pretty(&ControllerSurfaceMapping::nanokontrol2_novation_peak()) {
        let _ = std::fs::write(dir.join("korg-nanokontrol2-peak.mapping.json"), json);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asm_hydrasynth_profile_completeness() {
        let profile = DeviceSynthesizerProfile::asm_hydrasynth();
        assert_eq!(profile.id, "asm_hydrasynth");
        assert_eq!(profile.category, DeviceCategory::Synthesizer);
        assert!(profile.parameters.len() >= 65);

        // Verify Oscillators
        assert!(profile.get_param("osc1_wavscan").is_some());
        assert!(profile.get_param("osc2_wavscan").is_some());
        assert!(profile.get_param("osc3_cent").is_some());

        // Verify Mutators
        assert!(profile.get_param("mutator1_ratio").is_some());
        assert!(profile.get_param("mutator2_depth").is_some());
        assert!(profile.get_param("mutator3_dry_wet").is_some());
        assert!(profile.get_param("mutator4_ratio").is_some());

        // Verify Filters
        let f1_cutoff = profile.get_param("filter1_cutoff").expect("Filter 1 Cutoff missing");
        assert_eq!(f1_cutoff.address, ParamAddress::cc7(74));
        assert_eq!(f1_cutoff.curve, ResponseCurve::Exponential);
        let f2_cutoff = profile.get_param("filter2_cutoff").expect("Filter 2 Cutoff missing");
        assert_eq!(f2_cutoff.address, ParamAddress::cc7(55));

        // Verify Envelopes 1..5
        assert!(profile.get_param("env1_attack").is_some());
        assert!(profile.get_param("env2_decay").is_some());
        assert!(profile.get_param("env3_sustain").is_some());
        assert!(profile.get_param("env4_release").is_some());
        assert!(profile.get_param("env5_attack").is_some());

        // Verify LFOs 1..5
        assert!(profile.get_param("lfo1_rate").is_some());
        assert!(profile.get_param("lfo2_gain").is_some());
        assert!(profile.get_param("lfo3_rate").is_some());
        assert!(profile.get_param("lfo4_rate").is_some());
        assert!(profile.get_param("lfo5_gain").is_some());

        // Verify Macros 1..8
        for i in 1..=8 {
            let key = format!("macro_{}", i);
            let desc = profile.get_param(&key).unwrap_or_else(|| panic!("Missing {}", key));
            assert_eq!(desc.address, ParamAddress::cc7(15 + i as u8));
        }

        // Verify Ribbon & FX
        assert!(profile.get_param("ribbon_mod").is_some());
        assert!(profile.get_param("pre_fx_mix").is_some());
        assert!(profile.get_param("delay_time").is_some());
        assert!(profile.get_param("reverb_time").is_some());
        assert!(profile.get_param("post_fx_mix").is_some());

        // Test default patch creation
        let patch = profile.create_default_patch("Hydra Init");
        assert_eq!(patch.meta.name, "Hydra Init");
        assert_eq!(patch.get_param("filter1_cutoff"), Some(127.0));
        assert_eq!(patch.get_param("filter2_cutoff"), Some(127.0));
    }

    #[test]
    fn test_korg_nanokontrol2_profile_controls() {
        let profile = DeviceSynthesizerProfile::korg_nanokontrol2();
        assert_eq!(profile.id, "korg_nanokontrol2");
        assert_eq!(profile.category, DeviceCategory::ControllerSurface);

        // 8 Sliders + 8 Knobs + 8 Solo + 8 Mute + 8 Rec + 11 Transport = 51 controls
        assert_eq!(profile.controller_controls.len(), 51);

        // Verify Knobs CC 16..23 and Sliders CC 0..7
        for i in 0..8 {
            let slider = profile
                .controller_controls
                .iter()
                .find(|c| c.id == format!("slider_{}", i + 1))
                .expect("Slider missing");
            assert_eq!(slider.cc, i as u8);
            assert_eq!(slider.kind, ControlElementKind::LinearSlider);

            let knob = profile
                .controller_controls
                .iter()
                .find(|c| c.id == format!("knob_{}", i + 1))
                .expect("Knob missing");
            assert_eq!(knob.cc, 16 + i as u8);
            assert_eq!(knob.kind, ControlElementKind::RotaryKnob);

            let solo = profile
                .controller_controls
                .iter()
                .find(|c| c.id == format!("solo_{}", i + 1))
                .expect("Solo missing");
            assert!(solo.has_led);
            assert_eq!(solo.led_cc, Some(32 + i as u8));
        }

        // Verify Transport
        let play = profile
            .controller_controls
            .iter()
            .find(|c| c.id == "play")
            .expect("Play missing");
        assert_eq!(play.cc, 41);
        assert!(play.has_led);
    }

    #[test]
    fn test_akai_midimix_profile_controls() {
        let profile = DeviceSynthesizerProfile::akai_midimix();
        assert_eq!(profile.id, "akai_midimix");
        // 24 Knobs + 8 Faders + 8 Mute + 8 Rec + 1 Master + 2 Bank = 51 controls
        assert_eq!(profile.controller_controls.len(), 51);

        let master = profile
            .controller_controls
            .iter()
            .find(|c| c.id == "master_fader")
            .expect("Master fader missing");
        assert_eq!(master.cc, 62);
    }

    #[test]
    fn test_novation_bass_station_2_profile() {
        let profile = DeviceSynthesizerProfile::novation_bass_station_2();
        assert_eq!(profile.id, "novation_bass_station_2");
        assert!(profile.parameters.len() >= 45);

        let cutoff = profile.get_param("filter_cutoff").expect("Cutoff missing");
        assert_eq!(cutoff.address, ParamAddress::cc14(16, 48));
        assert_eq!(cutoff.curve, ResponseCurve::Exponential);
    }

    #[test]
    fn test_behringer_td3_md_profile() {
        let profile = DeviceSynthesizerProfile::behringer_td3_md();
        assert_eq!(profile.id, "behringer_td3_md");
        assert_eq!(profile.parameters.len(), 19);

        assert!(profile.get_param("filter_cutoff").is_some());
        assert!(profile.get_param("sub_osc_level").is_some());
        assert!(profile.get_param("overdrive_drive").is_some());
        assert!(profile.get_param("filter_fm_depth").is_some());
        assert!(profile.get_param("muffler_mode").is_some());
    }

    #[test]
    fn test_clavia_nord_drum_2_profile() {
        let profile = DeviceSynthesizerProfile::clavia_nord_drum_2();
        assert_eq!(profile.id, "clavia_nord_drum_2");
        assert_eq!(profile.parameters.len(), 202);

        // Verify Voice 1 parameters (31 parameters across Tone, Noise, Click, Mix, FX, Choke)
        assert_eq!(profile.params_for_voice(1).len(), 31);
        assert!(profile.get_param("voice_1_tone_wave").is_some());
        assert!(profile.get_param("voice_1_tone_decay").is_some());
        assert!(profile.get_param("voice_1_noise_filter_freq").is_some());
        assert!(profile.get_param("voice_1_click_type").is_some());
        assert!(profile.get_param("voice_1_mix_balance").is_some());
        assert!(profile.get_param("voice_1_distortion_amount").is_some());
        assert!(profile.get_param("voice_1_eq_frequency").is_some());
        assert!(profile.get_param("voice_1_echo_time_msb").is_some());
        assert!(profile.get_param("voice_1_choke_group").is_some());

        // Verify Voice 6 parameters (31 parameters across Tone, Noise, Click, Mix, FX, Choke)
        assert_eq!(profile.params_for_voice(6).len(), 31);
        assert!(profile.get_param("voice_6_tone_wave").is_some());
        assert!(profile.get_param("voice_6_tone_decay").is_some());
        assert!(profile.get_param("voice_6_noise_filter_freq").is_some());
        assert!(profile.get_param("voice_6_click_type").is_some());
        assert!(profile.get_param("voice_6_mix_balance").is_some());
        assert!(profile.get_param("voice_6_distortion_amount").is_some());
        assert!(profile.get_param("voice_6_eq_frequency").is_some());
        assert!(profile.get_param("voice_6_echo_time_msb").is_some());
        assert!(profile.get_param("voice_6_choke_group").is_some());

        // Verify Global Kit parameters (16 parameters)
        assert_eq!(profile.global_params().len(), 16);
        assert!(profile.get_param("global_channel").is_some());
        assert!(profile.get_param("global_focus_cc").is_some());
        assert!(profile.get_param("kit_bank").is_some());
        assert!(profile.get_param("kit_program").is_some());
        assert!(profile.get_param("voice_1_channel").is_some());
        assert!(profile.get_param("voice_1_trigger_note").is_some());
        assert!(profile.get_param("voice_6_channel").is_some());
        assert!(profile.get_param("voice_6_trigger_note").is_some());

        // Verify Physical Controls (6 channel buttons, main dial, trig button)
        assert_eq!(profile.controller_controls.len(), 8);
    }

    #[test]
    fn test_novation_peak_profile_and_nanokontrol2_mapping() {
        let profile = DeviceSynthesizerProfile::novation_peak();
        assert_eq!(profile.id, "novation_peak");
        assert_eq!(profile.category, DeviceCategory::Synthesizer);
        assert_eq!(profile.parameters.len(), 74);
        assert_eq!(profile.controller_controls.len(), 8);

        let cutoff = profile
            .get_param("filter_cutoff_frequency")
            .expect("Cutoff missing");
        assert_eq!(cutoff.address, ParamAddress::cc7(29));
        assert_eq!(cutoff.curve, ResponseCurve::Exponential);

        let reso = profile.get_param("filter_resonance").expect("Resonance missing");
        assert_eq!(reso.address, ParamAddress::cc7(79));
        assert_eq!(reso.safe_sweep_bounds().hi, 62);

        let dist = profile.get_param("distortion_level").expect("Distortion missing");
        assert_eq!(dist.address, ParamAddress::cc7(104));
        assert_eq!(dist.safe_sweep_bounds().hi, 48);

        let anim1 = profile.get_param("animate_1_hold").expect("Animate 1 missing");
        assert_eq!(anim1.address, ParamAddress::cc7(114));
        let anim2 = profile.get_param("animate_2_hold").expect("Animate 2 missing");
        assert_eq!(anim2.address, ParamAddress::cc7(115));

        let mapping = ControllerSurfaceMapping::nanokontrol2_novation_peak();
        assert_eq!(mapping.surface_name, "Korg nanoKONTROL2");
        assert_eq!(mapping.banks.len(), 3);
        assert_eq!(mapping.banks[0].controls.len(), 16);
        assert_eq!(mapping.banks[1].controls.len(), 16);
        assert_eq!(mapping.banks[2].controls.len(), 18);
    }

    #[test]
    fn test_json_schemas_and_validation() {
        let preset_schema = preset_json_schema();
        assert_eq!(preset_schema["title"], "SynthPatch");

        let profile_schema = device_profile_json_schema();
        assert_eq!(profile_schema["title"], "DeviceSynthesizerProfile");

        let mapping_schema = controller_mapping_json_schema();
        assert_eq!(mapping_schema["title"], "ControllerSurfaceMapping");

        // Round-trip profile JSON
        let bs2 = DeviceSynthesizerProfile::novation_bass_station_2();
        let json_str = bs2.to_json().expect("Serialize profile failed");
        let restored = validate_profile_json(&json_str).expect("Deserialize profile failed");
        assert_eq!(restored.id, bs2.id);
        assert_eq!(restored.parameters.len(), bs2.parameters.len());

        // Round-trip preset JSON
        let patch = bs2.create_default_patch("Test BS2 Patch");
        let patch_json = serde_json::to_string(&patch).expect("Serialize patch failed");
        let restored_patch = validate_preset_json(&patch_json).expect("Deserialize patch failed");
        assert_eq!(restored_patch.meta.name, "Test BS2 Patch");

        // Controller Surface Mapping Round-Trip
        let mapping = ControllerSurfaceMapping::nanokontrol2_nord_drum_2();
        let map_json = mapping.to_json().expect("Serialize mapping failed");
        let restored_map = validate_controller_mapping_json(&map_json).expect("Deserialize mapping failed");
        assert_eq!(restored_map.surface_name, "Korg nanoKONTROL2");
        assert_eq!(restored_map.banks.len(), 1);
        assert_eq!(restored_map.banks[0].controls.len(), 4);
    }

    #[test]
    fn test_all_four_synth_projects_profiles_and_schema_export() {
        // 1. ASM Hydrasynth (Polyphonic Wavetable)
        let hydra = DeviceSynthesizerProfile::asm_hydrasynth();
        let hydra_json = hydra.to_json().expect("Hydrasynth JSON serialize failed");
        let hydra_restored = validate_profile_json(&hydra_json).expect("Hydrasynth JSON parse failed");
        assert_eq!(hydra_restored.id, "asm_hydrasynth");
        let hydra_patch = hydra.create_default_patch("Hydra Default");
        assert_eq!(hydra_patch.meta.name, "Hydra Default");

        // 2. nkEditor Surface Profiles (Korg nanoKONTROL2 + Akai MIDImix)
        let nk2 = DeviceSynthesizerProfile::korg_nanokontrol2();
        let nk2_json = nk2.to_json().expect("nanoKONTROL2 JSON serialize failed");
        let nk2_restored = validate_profile_json(&nk2_json).expect("nanoKONTROL2 JSON parse failed");
        assert_eq!(nk2_restored.id, "korg_nanokontrol2");

        let midimix = DeviceSynthesizerProfile::akai_midimix();
        let midimix_json = midimix.to_json().expect("MIDImix JSON serialize failed");
        let midimix_restored = validate_profile_json(&midimix_json).expect("MIDImix JSON parse failed");
        assert_eq!(midimix_restored.id, "akai_midimix");

        // 3. BS2 + TD-3 Profiles (Novation Bass Station II + Behringer TD-3)
        let bs2 = DeviceSynthesizerProfile::novation_bass_station_2();
        let bs2_json = bs2.to_json().expect("BS2 JSON serialize failed");
        let bs2_restored = validate_profile_json(&bs2_json).expect("BS2 JSON parse failed");
        assert_eq!(bs2_restored.id, "novation_bass_station_2");

        let td3 = DeviceSynthesizerProfile::behringer_td3_md();
        let td3_json = td3.to_json().expect("TD-3 JSON serialize failed");
        let td3_restored = validate_profile_json(&td3_json).expect("TD-3 JSON parse failed");
        assert_eq!(td3_restored.id, "behringer_td3_md");

        // 4. Clavia Nord Drum 2 Profile (6-Voice Percussion Workstation)
        let nd2 = DeviceSynthesizerProfile::clavia_nord_drum_2();
        let nd2_json = nd2.to_json().expect("ND2 JSON serialize failed");
        let nd2_restored = validate_profile_json(&nd2_json).expect("ND2 JSON parse failed");
        assert_eq!(nd2_restored.id, "clavia_nord_drum_2");
        assert_eq!(nd2_restored.parameters.len(), 202);

        // 5. Novation Peak Profile (8-Voice Hybrid Polyphonic Synthesizer)
        let peak = DeviceSynthesizerProfile::novation_peak();
        let peak_json = peak.to_json().expect("Peak JSON serialize failed");
        let peak_restored = validate_profile_json(&peak_json).expect("Peak JSON parse failed");
        assert_eq!(peak_restored.id, "novation_peak");
        assert_eq!(peak_restored.parameters.len(), 74);

        // Verify Schema Exporter writes all 12 files across all 5 projects
        let temp_dir = std::env::temp_dir().join(format!("synth_core_schema_test_{}", uuid::Uuid::new_v4()));
        export_all_schemas(&temp_dir).expect("export_all_schemas failed");

        assert!(temp_dir.join("synth-patch.schema.json").exists());
        assert!(temp_dir.join("device-profile.schema.json").exists());
        assert!(temp_dir.join("controller-mapping.schema.json").exists());
        assert!(temp_dir.join("asm-hydrasynth.profile.json").exists());
        assert!(temp_dir.join("korg-nanokontrol2.profile.json").exists());
        assert!(temp_dir.join("akai-midimix.profile.json").exists());
        assert!(temp_dir.join("novation-bass-station-2.profile.json").exists());
        assert!(temp_dir.join("behringer-td3-md.profile.json").exists());
        assert!(temp_dir.join("clavia-nord-drum-2.profile.json").exists());
        assert!(temp_dir.join("korg-nanokontrol2-nd2.mapping.json").exists());
        assert!(temp_dir.join("novation-peak.profile.json").exists());
        assert!(temp_dir.join("korg-nanokontrol2-peak.mapping.json").exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
