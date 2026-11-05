// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthesizer parameter descriptors, addressing, scaling, response curves,
//! and hardware controller soft-takeover state handling.

use crate::error::SynthError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;

/// Strongly-typed synthesizer parameter identifier.
///
/// Converts human-readable names (e.g., "Filter 1 Cutoff") to normalized JSON-safe keys
/// (e.g., "filter_1_cutoff") matching the reference preset manager schema.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ParamId(String);

impl ParamId {
    /// Creates a new `ParamId` directly from a key.
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    /// Converts an arbitrary parameter name to a normalized key.
    ///
    /// Matches the transformation from `asm-preset-manager.js`:
    /// lowercases, replaces non-alphanumeric character sequences with underscores,
    /// and trims leading and trailing underscores.
    pub fn from_name(name: &str) -> Self {
        let mut out = String::with_capacity(name.len());
        let mut prev_underscore = false;

        for ch in name.chars() {
            if ch.is_ascii_alphanumeric() {
                out.push(ch.to_ascii_lowercase());
                prev_underscore = false;
            } else if !prev_underscore && !out.is_empty() {
                out.push('_');
                prev_underscore = true;
            }
        }

        if out.ends_with('_') {
            out.pop();
        }

        Self(out)
    }

    /// Returns the string slice representation of the identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the identifier and returns the inner string.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl Deref for ParamId {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for ParamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for ParamId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for ParamId {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// Hardware MIDI or SysEx address for a synthesizer parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ParamAddress {
    /// 7-bit standard MIDI Continuous Controller (0..127).
    Cc7(u8),
    /// 14-bit high-resolution Continuous Controller pair (0..16383).
    Cc14Bit { msb: u8, lsb: u8 },
    /// Non-Registered Parameter Number (NRPN) address.
    Nrpn { msb: u8, lsb: u8 },
    /// Byte or bit offset and size within a synthesizer SysEx dump payload.
    SysExOffset { offset: usize, size: usize },
    /// Parameter has no direct MIDI CC or NRPN address (e.g. kit or patch setting).
    None,
}

impl ParamAddress {
    /// Helper constructor for standard 7-bit Continuous Controller.
    pub fn cc7(cc: u8) -> Self {
        Self::Cc7(cc)
    }

    /// Helper constructor for unaddressed parameter (e.g. kit or patch setting).
    pub fn none() -> Self {
        Self::None
    }

    /// Helper constructor for 14-bit high-resolution Continuous Controller.
    pub fn cc14(msb: u8, lsb: u8) -> Self {
        Self::Cc14Bit { msb, lsb }
    }

    /// Helper constructor for Non-Registered Parameter Number.
    pub fn nrpn(msb: u8, lsb: u8) -> Self {
        Self::Nrpn { msb, lsb }
    }

    /// Helper constructor for SysEx offset.
    pub fn sysex(offset: usize, size: usize) -> Self {
        Self::SysExOffset { offset, size }
    }

    /// Returns whether this address is a standard 7-bit CC.
    pub fn is_cc7(&self) -> bool {
        matches!(self, Self::Cc7(_))
    }

    /// Returns whether this address is a 14-bit CC.
    pub fn is_cc14(&self) -> bool {
        matches!(self, Self::Cc14Bit { .. })
    }

    /// Returns whether this address is an NRPN.
    pub fn is_nrpn(&self) -> bool {
        matches!(self, Self::Nrpn { .. })
    }

    /// Returns whether this address is a SysEx payload offset.
    pub fn is_sysex(&self) -> bool {
        matches!(self, Self::SysExOffset { .. })
    }
}

/// Value range specification, step quantization, and display formatting for a parameter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValueRange {
    pub min: f64,
    pub max: f64,
    pub default_val: f64,
    pub step: f64,
    pub unit: String,
}

impl ValueRange {
    /// Creates a new value range with mathematical validation.
    pub fn new(
        min: f64,
        max: f64,
        default_val: f64,
        step: f64,
        unit: impl Into<String>,
    ) -> Result<Self, SynthError> {
        if min > max {
            return Err(SynthError::InvalidRange {
                min: min.to_string(),
                max: max.to_string(),
                reason: "minimum cannot exceed maximum".to_string(),
            });
        }
        if step < 0.0 {
            return Err(SynthError::InvalidRange {
                min: min.to_string(),
                max: max.to_string(),
                reason: "step cannot be negative".to_string(),
            });
        }
        let clamped_default = default_val.clamp(min, max);
        Ok(Self {
            min,
            max,
            default_val: clamped_default,
            step,
            unit: unit.into(),
        })
    }

    /// Standard 7-bit MIDI Continuous Controller range (0..127).
    pub fn midi_cc7(default_val: f64) -> Self {
        Self {
            min: 0.0,
            max: 127.0,
            default_val: default_val.clamp(0.0, 127.0),
            step: 1.0,
            unit: String::new(),
        }
    }

    /// Standard 14-bit MIDI Continuous Controller range (0..16383).
    pub fn midi_cc14(default_val: f64) -> Self {
        Self {
            min: 0.0,
            max: 16383.0,
            default_val: default_val.clamp(0.0, 16383.0),
            step: 1.0,
            unit: String::new(),
        }
    }

    /// Bipolar symmetric range with custom units (e.g. -100..+100 cents).
    pub fn bipolar(min: f64, max: f64, default_val: f64, unit: impl Into<String>) -> Self {
        Self {
            min,
            max,
            default_val: default_val.clamp(min, max),
            step: 1.0,
            unit: unit.into(),
        }
    }

    /// Normalized unit interval range (0.0..1.0).
    pub fn normalized_0_1() -> Self {
        Self {
            min: 0.0,
            max: 1.0,
            default_val: 0.0,
            step: 0.001,
            unit: String::new(),
        }
    }

    /// Normalizes a raw parameter value into the range [0.0, 1.0].
    pub fn normalize(&self, raw: f64) -> f64 {
        let clamped = raw.clamp(self.min, self.max);
        let span = self.max - self.min;
        if span <= f64::EPSILON {
            0.0
        } else {
            ((clamped - self.min) / span).clamp(0.0, 1.0)
        }
    }

    /// Denormalizes a [0.0, 1.0] interval value back into raw parameter units.
    pub fn denormalize(&self, norm: f64) -> f64 {
        let clamped_norm = norm.clamp(0.0, 1.0);
        let raw = self.min + clamped_norm * (self.max - self.min);
        self.quantize(raw)
    }

    /// Clamps a raw value to the allowed range [min, max].
    pub fn clamp(&self, raw: f64) -> f64 {
        raw.clamp(self.min, self.max)
    }

    /// Snaps a raw value to the nearest step increment and clamps to [min, max].
    pub fn quantize(&self, raw: f64) -> f64 {
        let clamped = self.clamp(raw);
        if self.step <= f64::EPSILON {
            clamped
        } else {
            let offset = clamped - self.min;
            let steps = (offset / self.step).round();
            (self.min + steps * self.step).clamp(self.min, self.max)
        }
    }

    /// Formats the raw value into a string with optional units.
    pub fn format_value(&self, raw: f64) -> String {
        let quantized = self.quantize(raw);
        if self.step >= 1.0 {
            if self.unit.is_empty() {
                format!("{:.0}", quantized)
            } else {
                format!("{:.0} {}", quantized, self.unit)
            }
        } else if self.step >= 0.01 {
            if self.unit.is_empty() {
                format!("{:.2}", quantized)
            } else {
                format!("{:.2} {}", quantized, self.unit)
            }
        } else if self.unit.is_empty() {
            format!("{:.3}", quantized)
        } else {
            format!("{:.3} {}", quantized, self.unit)
        }
    }
}

/// Response curve applied between physical input or UI and target parameter values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ResponseCurve {
    /// Straight linear 1:1 mapping.
    #[default]
    Linear,
    /// Exponential response curve tailored for audio frequencies and resonance.
    Exponential,
    /// Logarithmic response curve tailored for volume attenuation and decay times.
    Logarithmic,
    /// Smooth S-curve easing at lower and upper boundaries.
    SCurve,
}

impl ResponseCurve {
    /// Maps a normalized input value [0.0, 1.0] through the selected response curve.
    pub fn transform(&self, norm_in: f64) -> f64 {
        let x = norm_in.clamp(0.0, 1.0);
        match self {
            Self::Linear => x,
            Self::Exponential => {
                // Audio taper mapping 0.0 -> 0.0 and 1.0 -> 1.0 smoothly:
                // (10^x - 1) / 9
                (10.0_f64.powf(x) - 1.0) / 9.0
            }
            Self::Logarithmic => {
                // Inverse of audio taper mapping 0.0 -> 0.0 and 1.0 -> 1.0:
                // log10(9 * x + 1)
                (9.0 * x + 1.0).log10().clamp(0.0, 1.0)
            }
            Self::SCurve => {
                // Cubic Hermite smoothstep: 3*x^2 - 2*x^3
                x * x * (3.0 - 2.0 * x)
            }
        }
    }

    /// Inverts the response curve, mapping curved values back to linear normalized space.
    pub fn inverse(&self, curved_val: f64) -> f64 {
        let y = curved_val.clamp(0.0, 1.0);
        match self {
            Self::Linear => y,
            Self::Exponential => (9.0 * y + 1.0).log10().clamp(0.0, 1.0),
            Self::Logarithmic => ((10.0_f64.powf(y) - 1.0) / 9.0).clamp(0.0, 1.0),
            Self::SCurve => {
                // Numerical inversion of cubic smoothstep using Newton-Raphson iterations.
                let mut x = y;
                for _ in 0..6 {
                    let f = x * x * (3.0 - 2.0 * x) - y;
                    let df = 6.0 * x * (1.0 - x);
                    if df.abs() < 1e-6 {
                        break;
                    }
                    x = (x - f / df).clamp(0.0, 1.0);
                }
                x
            }
        }
    }
}

/// Operating mode for physical MIDI controller soft-takeover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SoftTakeoverMode {
    /// Parameter does not move until physical control position crosses stored preset value.
    #[default]
    Catch,
    /// Parameter immediately jumps to incoming physical controller value.
    Jump,
    /// Proportional smooth pickup converging toward physical control without jumps.
    Pickup,
}

/// Result returned after processing an incoming physical controller movement.
#[derive(Debug, Clone, PartialEq)]
pub struct SoftTakeoverResult {
    /// Current effective parameter value after evaluation.
    pub value: f64,
    /// Whether the effective parameter value changed.
    pub updated: bool,
    /// Whether the physical control has caught the parameter.
    pub is_caught: bool,
    /// Signed difference (physical_value - target_value).
    pub delta: f64,
}

/// State tracker for soft-takeover between physical hardware controllers and target parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoftTakeoverState {
    pub mode: SoftTakeoverMode,
    pub target_value: f64,
    pub physical_value: Option<f64>,
    pub is_caught: bool,
    pub threshold: f64,
}

impl SoftTakeoverState {
    /// Default capture threshold in parameter units.
    pub const DEFAULT_THRESHOLD: f64 = 1.5;

    /// Creates a new soft-takeover state tracker.
    pub fn new(mode: SoftTakeoverMode, target_value: f64) -> Self {
        Self {
            mode,
            target_value,
            physical_value: None,
            is_caught: matches!(mode, SoftTakeoverMode::Jump),
            threshold: Self::DEFAULT_THRESHOLD,
        }
    }

    /// Sets a custom capture threshold.
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold.max(0.0);
        self
    }

    /// Resets physical tracking when loading a new preset.
    pub fn reset_target(&mut self, new_target: f64) {
        self.target_value = new_target;
        self.physical_value = None;
        self.is_caught = matches!(self.mode, SoftTakeoverMode::Jump);
    }

    /// Signed physical-vs-target delta calculation: `physical - target`.
    pub fn delta(&self) -> Option<f64> {
        self.physical_value.map(|p| p - self.target_value)
    }

    /// Absolute physical-vs-target delta calculation: `|physical - target|`.
    pub fn abs_delta(&self) -> Option<f64> {
        self.physical_value.map(|p| (p - self.target_value).abs())
    }

    /// Updates state with an incoming physical controller value.
    pub fn update(&mut self, incoming_physical: f64) -> SoftTakeoverResult {
        let prev_physical = self.physical_value;
        self.physical_value = Some(incoming_physical);

        let delta = incoming_physical - self.target_value;
        let abs_delta = delta.abs();

        match self.mode {
            SoftTakeoverMode::Jump => {
                let changed = (self.target_value - incoming_physical).abs() > f64::EPSILON
                    || prev_physical.is_none();
                self.target_value = incoming_physical;
                self.is_caught = true;
                SoftTakeoverResult {
                    value: self.target_value,
                    updated: changed,
                    is_caught: true,
                    delta: 0.0,
                }
            }
            SoftTakeoverMode::Catch => {
                if self.is_caught {
                    let changed = (self.target_value - incoming_physical).abs() > f64::EPSILON;
                    self.target_value = incoming_physical;
                    SoftTakeoverResult {
                        value: self.target_value,
                        updated: changed,
                        is_caught: true,
                        delta: 0.0,
                    }
                } else {
                    // Check if physical crossed the target value or is within threshold
                    let crossed = match prev_physical {
                        Some(prev) => {
                            (prev <= self.target_value && incoming_physical >= self.target_value)
                                || (prev >= self.target_value
                                    && incoming_physical <= self.target_value)
                        }
                        None => false,
                    };

                    if crossed || abs_delta <= self.threshold {
                        self.is_caught = true;
                        self.target_value = incoming_physical;
                        SoftTakeoverResult {
                            value: self.target_value,
                            updated: true,
                            is_caught: true,
                            delta: 0.0,
                        }
                    } else {
                        // Stays uncaught, target value does not move
                        SoftTakeoverResult {
                            value: self.target_value,
                            updated: false,
                            is_caught: false,
                            delta,
                        }
                    }
                }
            }
            SoftTakeoverMode::Pickup => {
                if self.is_caught {
                    let changed = (self.target_value - incoming_physical).abs() > f64::EPSILON;
                    self.target_value = incoming_physical;
                    SoftTakeoverResult {
                        value: self.target_value,
                        updated: changed,
                        is_caught: true,
                        delta: 0.0,
                    }
                } else if abs_delta <= self.threshold {
                    self.is_caught = true;
                    self.target_value = incoming_physical;
                    SoftTakeoverResult {
                        value: self.target_value,
                        updated: true,
                        is_caught: true,
                        delta: 0.0,
                    }
                } else {
                    // Proportional convergence: move target toward incoming physical
                    let step = match prev_physical {
                        Some(prev) => incoming_physical - prev,
                        None => 0.0,
                    };

                    let was_updated = step.abs() > f64::EPSILON;
                    if was_updated {
                        let pull = (incoming_physical - self.target_value) * 0.25;
                        self.target_value += pull;
                    }

                    // Check if convergence caught up or crossed
                    let new_delta = incoming_physical - self.target_value;
                    let crossed = match prev_physical {
                        Some(_) => {
                            (delta < 0.0 && new_delta >= 0.0) || (delta > 0.0 && new_delta <= 0.0)
                        }
                        None => false,
                    };

                    if new_delta.abs() <= self.threshold || crossed {
                        self.is_caught = true;
                        self.target_value = incoming_physical;
                    }

                    SoftTakeoverResult {
                        value: self.target_value,
                        updated: was_updated,
                        is_caught: self.is_caught,
                        delta: incoming_physical - self.target_value,
                    }
                }
            }
        }
    }
}

/// Metadata descriptor for a synthesizer parameter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParamDescriptor {
    pub id: ParamId,
    pub name: String,
    pub module: String,
    pub address: ParamAddress,
    pub range: ValueRange,
    pub curve: ResponseCurve,
    pub is_locked: bool,
}

impl ParamDescriptor {
    /// Creates a new parameter descriptor.
    pub fn new(
        name: impl Into<String>,
        module: impl Into<String>,
        address: ParamAddress,
        range: ValueRange,
    ) -> Self {
        let name_str = name.into();
        let id = ParamId::from_name(&name_str);
        Self {
            id,
            name: name_str,
            module: module.into(),
            address,
            range,
            curve: ResponseCurve::Linear,
            is_locked: false,
        }
    }

    /// Sets the response curve for this parameter.
    pub fn with_curve(mut self, curve: ResponseCurve) -> Self {
        self.curve = curve;
        self
    }

    /// Marks whether this parameter is shielded from randomization.
    pub fn with_locked(mut self, locked: bool) -> Self {
        self.is_locked = locked;
        self
    }

    /// Returns the acoustic safe sweep bounds for this parameter based on its name.
    pub fn safe_sweep_bounds(&self) -> SafeSweepBounds {
        safe_sweep_range(&self.name)
    }
}

/// Conservative acoustic sweep bounds for a 7-bit MIDI CC parameter (`0..=127`).
///
/// Matches `safeSweepRange(name)` in `js/konduktiva-peak/src/cc.js` so that
/// resonance, analog distortion, filter overdrive/post-drive, delay feedback,
/// and FX sends do not self-oscillate or blast at full volume during automated
/// sweeps or randomization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SafeSweepBounds {
    pub lo: u8,
    pub hi: u8,
}

impl SafeSweepBounds {
    /// Creates a new `SafeSweepBounds` pair (`lo..=hi`).
    pub const fn new(lo: u8, hi: u8) -> Self {
        Self { lo, hi }
    }

    /// Clamps a 7-bit MIDI CC value into `[lo, hi]`.
    pub fn clamp(&self, value: u8) -> u8 {
        value.clamp(self.lo, self.hi)
    }

    /// Clamps a floating-point parameter value into `[lo as f64, hi as f64]`.
    pub fn clamp_f64(&self, value: f64) -> f64 {
        value.clamp(self.lo as f64, self.hi as f64)
    }
}

/// Computes the conservative safe sweep range for a parameter keyword or name.
///
/// Identical to `safeSweepRange(name)` in `js/konduktiva-peak/src/cc.js`:
/// - `resonance` / `reso` -> `0..=62`
/// - `distortion` -> `0..=48`
/// - `overdrive` / `drive` -> `0..=50`
/// - `feedback` -> `0..=70`
/// - `reverb` / `delay` / `chorus` / `fx` -> `0..=90`
/// - Everything else -> `0..=127`
pub fn safe_sweep_range(name: &str) -> SafeSweepBounds {
    let n = name.to_ascii_lowercase();
    if n.contains("resonance") || n.contains("reso") {
        SafeSweepBounds::new(0, 62)
    } else if n.contains("distortion") {
        SafeSweepBounds::new(0, 48)
    } else if n.contains("overdrive") || n.contains("drive") {
        SafeSweepBounds::new(0, 50)
    } else if n.contains("feedback") {
        SafeSweepBounds::new(0, 70)
    } else if n.contains("reverb")
        || n.contains("delay")
        || n.contains("chorus")
        || n.contains("fx")
    {
        SafeSweepBounds::new(0, 90)
    } else {
        SafeSweepBounds::new(0, 127)
    }
}
