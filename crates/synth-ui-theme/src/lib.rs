// SPDX-License-Identifier: GPL-3.0-or-later
//! `synth-ui-theme`: Visual theme engine integrating yaae-theme palettes,
//! twoBowls backgrounds, synthesizer accent profiles, custom hex color overrides,
//! zero-rounding crisp borders, UI scaling, and reusable vector widgets.

pub mod widgets;

pub use egui::{
    Align, Align2, Button, CentralPanel, CollapsingHeader, Color32, ComboBox, Context, FontId,
    Frame, Grid, Id, Key, Layout, Pos2, ProgressBar, RawInput, Rect, Response, RichText, Rounding,
    ScrollArea, Sense, Slider, Stroke, TextEdit, TopBottomPanel, Ui, Vec2, Visuals, Window,
};
use serde::{Deserialize, Serialize};
use std::fmt;

pub use widgets::{
    animate_hold_button_widget, drum_pad_velocity_widget, resonant_spectrum_bar_widget,
    state_variable_filter_widget, tone_noise_balance_slider, voice_allocation_meter_widget,
    voice_focus_badge, wavetable_oscillator_widget, AnimateHoldButtonModel, DrumPadVelocityModel,
    EnvelopeCurveModel, KeyOverlayBadgeModel, ResonantSpectrumBarModel, RotaryKnobModel,
    SoftTakeoverOutcome, StateVariableFilterModel, StepLedButtonModel, StepLedState,
    ToneNoiseBalanceModel, ToneNoiseBalanceSliderModel, VoiceAllocationMeterModel,
    VoiceFocusBadgeModel, WavetableOscillatorModel,
};
pub use yaae_theme::{
    theme_palette::{self, AccentPalette, FocusRingConfig},
    ui_visuals, ColorConfig, ThemePreference,
};

/// Clavia Nord Drum 2 hardware chassis crimson red accent hex (#E52521).
pub const NORD_CRIMSON_HEX: &str = "#E52521";
/// Clavia Nord Drum 2 hardware chassis crimson red accent Color32.
pub const NORD_CRIMSON_PRIMARY: Color32 = Color32::from_rgb(0xE5, 0x25, 0x21);
/// Clavia Nord Drum 2 hardware high-contrast amber LED readout hex (#FFB300).
pub const NORD_AMBER_LED_HEX: &str = "#FFB300";
/// Clavia Nord Drum 2 hardware high-contrast amber LED readout Color32.
pub const NORD_AMBER_LED: Color32 = Color32::from_rgb(0xFF, 0xB3, 0x00);

/// Novation Peak Oxford Blue anodized chassis accent hex (#1A73E8).
pub const PEAK_OXFORD_BLUE_HEX: &str = "#1A73E8";
/// Novation Peak Oxford Blue anodized chassis accent Color32.
pub const PEAK_OXFORD_BLUE_PRIMARY: Color32 = Color32::from_rgb(0x1A, 0x73, 0xE8);
/// Novation Peak warm OLED amber readout hex (#FF8C00).
pub const PEAK_AMBER_OLED_HEX: &str = "#FF8C00";
/// Novation Peak warm OLED amber readout Color32.
pub const PEAK_AMBER_OLED: Color32 = Color32::from_rgb(0xFF, 0x8C, 0x00);

/// The four canonical twoBowls background presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TwoBowlsBackground {
    /// Pure white background (#FFFFFF).
    White,
    /// Warm cream background (#F8F2E4).
    #[default]
    Cream,
    /// Deep pitch black background (#000000).
    Black,
    /// Professional dark grey background (#181818).
    DarkGrey,
}

impl TwoBowlsBackground {
    /// Returns all available twoBowls background presets.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[Self::White, Self::Cream, Self::Black, Self::DarkGrey]
    }

    /// Returns the human-readable display name.
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::White => "White",
            Self::Cream => "Cream",
            Self::Black => "Black",
            Self::DarkGrey => "Dark Grey",
        }
    }

    /// Returns the CSS hex color string.
    #[must_use]
    pub const fn hex(&self) -> &'static str {
        match self {
            Self::White => "#FFFFFF",
            Self::Cream => "#F8F2E4",
            Self::Black => "#000000",
            Self::DarkGrey => "#181818",
        }
    }

    /// Returns the RGB tuple [r, g, b].
    #[must_use]
    pub const fn rgb(&self) -> [u8; 3] {
        match self {
            Self::White => [255, 255, 255],
            Self::Cream => [248, 242, 228],
            Self::Black => [0, 0, 0],
            Self::DarkGrey => [24, 24, 24],
        }
    }

    /// Returns the egui Color32 representation.
    #[must_use]
    pub const fn color32(&self) -> Color32 {
        let [r, g, b] = self.rgb();
        Color32::from_rgb(r, g, b)
    }

    /// Returns whether this background preset is considered light.
    #[must_use]
    pub const fn is_light(&self) -> bool {
        matches!(self, Self::White | Self::Cream)
    }

    /// Parses from a string identifier.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_lowercase().as_str() {
            "white" => Some(Self::White),
            "cream" => Some(Self::Cream),
            "black" => Some(Self::Black),
            "dark grey" | "darkgrey" | "dark_grey" | "dark-grey" => Some(Self::DarkGrey),
            _ => None,
        }
    }
}

impl fmt::Display for TwoBowlsBackground {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.display_name())
    }
}

/// The eight synthesizer and editor accent profiles shared across ASM Hydrasynth Editor,
/// nkEditor, BS2/TD-3 Manager, Nord Drum 2 Manager, and Novation Peak Manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SynthAccent {
    /// Midnight: Royal blue accent profile (#4169E1).
    Midnight,
    /// Cyberpunk: Neon pink and magenta accent profile (#FF007F).
    Cyberpunk,
    /// Arctic: Electric ice blue and cyan accent profile (#00D2FF).
    Arctic,
    /// Volcanic: High-energy orange-red accent profile (#FF4500).
    Volcanic,
    /// BassStationBlue: Classic Novation navy and cobalt blue accent profile (#0055A5).
    #[default]
    BassStationBlue,
    /// AcidYellowTD3: Roland and Behringer 303 acid yellow accent profile (#E6DB22).
    AcidYellowTD3,
    /// NordCrimson: Classic Clavia Nord drum crimson red chassis accent profile (#E52521)
    /// with high-contrast amber LED readout (#FFB300).
    NordCrimson,
    /// PeakOxfordBlue: Novation Peak Oxford Blue anodized chassis accent (#1A73E8)
    /// with warm amber OLED readout (#FF8C00).
    PeakOxfordBlue,
}

impl SynthAccent {
    /// Returns all eight synthesizer accent profiles.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Midnight,
            Self::Cyberpunk,
            Self::Arctic,
            Self::Volcanic,
            Self::BassStationBlue,
            Self::AcidYellowTD3,
            Self::NordCrimson,
            Self::PeakOxfordBlue,
        ]
    }

    /// Returns the human-readable display name.
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Midnight => "Midnight",
            Self::Cyberpunk => "Cyberpunk",
            Self::Arctic => "Arctic",
            Self::Volcanic => "Volcanic",
            Self::BassStationBlue => "Bass Station Blue",
            Self::AcidYellowTD3 => "Acid Yellow TD-3",
            Self::NordCrimson => "Nord Crimson",
            Self::PeakOxfordBlue => "Peak Oxford Blue",
        }
    }

    /// Returns the CSS hex color string for the primary accent.
    #[must_use]
    pub const fn hex(&self) -> &'static str {
        match self {
            Self::Midnight => "#4169E1",
            Self::Cyberpunk => "#FF007F",
            Self::Arctic => "#00D2FF",
            Self::Volcanic => "#FF4500",
            Self::BassStationBlue => "#0055A5",
            Self::AcidYellowTD3 => "#E6DB22",
            Self::NordCrimson => "#E52521",
            Self::PeakOxfordBlue => "#1A73E8",
        }
    }

    /// Returns the primary RGB tuple [r, g, b].
    #[must_use]
    pub const fn rgb(&self) -> [u8; 3] {
        match self {
            Self::Midnight => [0x41, 0x69, 0xE1],
            Self::Cyberpunk => [0xFF, 0x00, 0x7F],
            Self::Arctic => [0x00, 0xD2, 0xFF],
            Self::Volcanic => [0xFF, 0x45, 0x00],
            Self::BassStationBlue => [0x00, 0x55, 0xA5],
            Self::AcidYellowTD3 => [0xE6, 0xDB, 0x22],
            Self::NordCrimson => [0xE5, 0x25, 0x21],
            Self::PeakOxfordBlue => [0x1A, 0x73, 0xE8],
        }
    }

    /// Returns the primary accent color as Color32.
    #[must_use]
    pub fn primary_color(&self) -> Color32 {
        let [r, g, b] = self.rgb();
        Color32::from_rgb(r, g, b)
    }

    /// Returns the complementary secondary accent color as Color32.
    #[must_use]
    pub fn secondary_color(&self) -> Color32 {
        match self {
            Self::Midnight => Color32::from_rgb(0x1E, 0x90, 0xFF),
            Self::Cyberpunk => Color32::from_rgb(0x00, 0xF0, 0xFF),
            Self::Arctic => Color32::from_rgb(0x70, 0xE0, 0xFF),
            Self::Volcanic => Color32::from_rgb(0xFF, 0x8C, 0x00),
            Self::BassStationBlue => Color32::from_rgb(0x00, 0x82, 0xD6),
            Self::AcidYellowTD3 => Color32::from_rgb(0xFF, 0x66, 0x00),
            Self::NordCrimson => Color32::from_rgb(0xFF, 0x5C, 0x5A),
            Self::PeakOxfordBlue => Color32::from_rgb(0x4A, 0x90, 0xE2),
        }
    }

    /// Returns the CSS hex color string for the LED readout and numeric display.
    #[must_use]
    pub const fn readout_hex(&self) -> &'static str {
        match self {
            Self::Midnight => "#00D2FF",
            Self::Cyberpunk => "#00F0FF",
            Self::Arctic => "#00E5FF",
            Self::Volcanic => "#FFCC00",
            Self::BassStationBlue => "#00E5FF",
            Self::AcidYellowTD3 => "#FF3030",
            Self::NordCrimson => "#FFB300",
            Self::PeakOxfordBlue => "#FF8C00",
        }
    }

    /// Returns the RGB tuple [r, g, b] for the LED readout.
    #[must_use]
    pub const fn readout_rgb(&self) -> [u8; 3] {
        match self {
            Self::Midnight => [0x00, 0xD2, 0xFF],
            Self::Cyberpunk => [0x00, 0xF0, 0xFF],
            Self::Arctic => [0x00, 0xE5, 0xFF],
            Self::Volcanic => [0xFF, 0xCC, 0x00],
            Self::BassStationBlue => [0x00, 0xE5, 0xFF],
            Self::AcidYellowTD3 => [0xFF, 0x30, 0x30],
            Self::NordCrimson => [0xFF, 0xB3, 0x00],
            Self::PeakOxfordBlue => [0xFF, 0x8C, 0x00],
        }
    }

    /// Returns the LED readout color as Color32.
    #[must_use]
    pub fn readout_color(&self) -> Color32 {
        let [r, g, b] = self.readout_rgb();
        Color32::from_rgb(r, g, b)
    }

    /// Parses an accent profile from a string identifier.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_lowercase().as_str() {
            "midnight" => Some(Self::Midnight),
            "cyberpunk" => Some(Self::Cyberpunk),
            "arctic" => Some(Self::Arctic),
            "volcanic" => Some(Self::Volcanic),
            "bass station blue" | "bassstationblue" | "bass_station_blue" | "bs2" => {
                Some(Self::BassStationBlue)
            }
            "acid yellow td-3" | "acidyellowtd3" | "acid_yellow_td3" | "acid" | "td3" => {
                Some(Self::AcidYellowTD3)
            }
            "nord crimson" | "nordcrimson" | "nord_crimson" | "nord" | "nd2" | "crimson" => {
                Some(Self::NordCrimson)
            }
            "peak oxford blue" | "peakoxfordblue" | "peak_oxford_blue" | "peak" | "oxford" => {
                Some(Self::PeakOxfordBlue)
            }
            _ => None,
        }
    }
}

impl fmt::Display for SynthAccent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.display_name())
    }
}

/// Convenience function returning the primary accent color for a SynthAccent profile.
#[must_use]
pub fn primary_color(accent: SynthAccent) -> Color32 {
    accent.primary_color()
}

/// Convenience function returning the LED readout color for a SynthAccent profile.
#[must_use]
pub fn readout_color(accent: SynthAccent) -> Color32 {
    accent.readout_color()
}

/// Errors that can occur when parsing a hex color string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HexColorError {
    /// Input string was empty or contained only whitespace.
    Empty,
    /// Invalid length for hex representation (expected 3, 4, 6, or 8 hex digits).
    InvalidLength(usize),
    /// Encountered a character that is not a valid hexadecimal digit.
    InvalidDigit(char),
}

impl fmt::Display for HexColorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "hex color string is empty"),
            Self::InvalidLength(len) => write!(
                f,
                "invalid hex length {len}, expected 3, 4, 6, or 8 digits"
            ),
            Self::InvalidDigit(ch) => write!(f, "invalid hex digit '{ch}'"),
        }
    }
}

impl std::error::Error for HexColorError {}

/// Parses a CSS-style hex color string (`#RGB`, `#RGBA`, `#RRGGBB`, or `#RRGGBBAA`) into `Color32`.
pub fn parse_hex_color(input: &str) -> Result<Color32, HexColorError> {
    let s = input.trim().trim_start_matches('#');
    if s.is_empty() {
        return Err(HexColorError::Empty);
    }

    let decode_nibble = |c: char| -> Result<u8, HexColorError> {
        match c {
            '0'..='9' => Ok(c as u8 - b'0'),
            'a'..='f' => Ok(c as u8 - b'a' + 10),
            'A'..='F' => Ok(c as u8 - b'A' + 10),
            _ => Err(HexColorError::InvalidDigit(c)),
        }
    };

    match s.len() {
        3 => {
            let mut chars = s.chars();
            let r = decode_nibble(chars.next().ok_or(HexColorError::Empty)?)?;
            let g = decode_nibble(chars.next().ok_or(HexColorError::Empty)?)?;
            let b = decode_nibble(chars.next().ok_or(HexColorError::Empty)?)?;
            Ok(Color32::from_rgb(r * 17, g * 17, b * 17))
        }
        4 => {
            let mut chars = s.chars();
            let r = decode_nibble(chars.next().ok_or(HexColorError::Empty)?)?;
            let g = decode_nibble(chars.next().ok_or(HexColorError::Empty)?)?;
            let b = decode_nibble(chars.next().ok_or(HexColorError::Empty)?)?;
            let a = decode_nibble(chars.next().ok_or(HexColorError::Empty)?)?;
            Ok(Color32::from_rgba_premultiplied(
                r * 17,
                g * 17,
                b * 17,
                a * 17,
            ))
        }
        6 => {
            let bytes = s.as_bytes();
            let mut rgb = [0u8; 3];
            for i in 0..3 {
                let high = decode_nibble(bytes[i * 2] as char)?;
                let low = decode_nibble(bytes[i * 2 + 1] as char)?;
                rgb[i] = (high << 4) | low;
            }
            Ok(Color32::from_rgb(rgb[0], rgb[1], rgb[2]))
        }
        8 => {
            let bytes = s.as_bytes();
            let mut rgba = [0u8; 4];
            for i in 0..4 {
                let high = decode_nibble(bytes[i * 2] as char)?;
                let low = decode_nibble(bytes[i * 2 + 1] as char)?;
                rgba[i] = (high << 4) | low;
            }
            Ok(Color32::from_rgba_premultiplied(
                rgba[0], rgba[1], rgba[2], rgba[3],
            ))
        }
        other => Err(HexColorError::InvalidLength(other)),
    }
}

/// Formats a `Color32` into a CSS-style hex string.
#[must_use]
pub fn format_hex_color(color: Color32, include_alpha: bool) -> String {
    if include_alpha && color.a() != 255 {
        format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            color.r(),
            color.g(),
            color.b(),
            color.a()
        )
    } else {
        format!("#{:02X}{:02X}{:02X}", color.r(), color.g(), color.b())
    }
}

/// User-configurable custom color overrides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CustomColorOverrides {
    /// Window and background color override (CSS hex string).
    pub background: Option<String>,
    /// Primary accent highlight color override (CSS hex string).
    pub accent: Option<String>,
    /// Panel fill color override (CSS hex string).
    pub panel: Option<String>,
    /// Text color override (CSS hex string).
    pub text: Option<String>,
    /// Hardware Solo button LED color (CSS hex string, default amber #FFB000).
    pub solo_led: Option<String>,
    /// Hardware Mute button LED color (CSS hex string, default red #FF2020).
    pub mute_led: Option<String>,
    /// Hardware Record button LED color (CSS hex string, default red #FF0040).
    pub rec_led: Option<String>,
    /// Sequencer step button LED color (CSS hex string, default red #FF3030).
    pub step_led: Option<String>,
    /// Soft-takeover ghost ring indicator color (CSS hex string, default amber #FFA500).
    pub ghost_ring: Option<String>,
    /// Hardware LED 7-segment and readout numeric display color (CSS hex string, default amber #FFB300).
    pub led_readout: Option<String>,
}

impl CustomColorOverrides {
    /// Creates an empty set of color overrides.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolves parsed background color, or `None` if unset or invalid hex.
    #[must_use]
    pub fn resolved_background(&self) -> Option<Color32> {
        self.background.as_deref().and_then(|h| parse_hex_color(h).ok())
    }

    /// Resolves parsed accent color, or `None` if unset or invalid hex.
    #[must_use]
    pub fn resolved_accent(&self) -> Option<Color32> {
        self.accent.as_deref().and_then(|h| parse_hex_color(h).ok())
    }

    /// Resolves parsed panel color, or `None` if unset or invalid hex.
    #[must_use]
    pub fn resolved_panel(&self) -> Option<Color32> {
        self.panel.as_deref().and_then(|h| parse_hex_color(h).ok())
    }

    /// Resolves parsed text color, or `None` if unset or invalid hex.
    #[must_use]
    pub fn resolved_text(&self) -> Option<Color32> {
        self.text.as_deref().and_then(|h| parse_hex_color(h).ok())
    }

    /// Resolves parsed solo LED color, or defaults to amber (#FFB000).
    #[must_use]
    pub fn resolved_solo_led(&self) -> Color32 {
        self.solo_led
            .as_deref()
            .and_then(|h| parse_hex_color(h).ok())
            .unwrap_or_else(|| Color32::from_rgb(255, 176, 0))
    }

    /// Resolves parsed mute LED color, or defaults to red (#FF2020).
    #[must_use]
    pub fn resolved_mute_led(&self) -> Color32 {
        self.mute_led
            .as_deref()
            .and_then(|h| parse_hex_color(h).ok())
            .unwrap_or_else(|| Color32::from_rgb(255, 32, 32))
    }

    /// Resolves parsed record arm LED color, or defaults to deep red (#FF0040).
    #[must_use]
    pub fn resolved_rec_led(&self) -> Color32 {
        self.rec_led
            .as_deref()
            .and_then(|h| parse_hex_color(h).ok())
            .unwrap_or_else(|| Color32::from_rgb(255, 0, 64))
    }

    /// Resolves parsed step LED color, or defaults to red (#FF3030).
    #[must_use]
    pub fn resolved_step_led(&self) -> Color32 {
        self.step_led
            .as_deref()
            .and_then(|h| parse_hex_color(h).ok())
            .unwrap_or_else(|| Color32::from_rgb(255, 48, 48))
    }

    /// Resolves parsed ghost ring indicator color, or defaults to amber (#FFA500).
    #[must_use]
    pub fn resolved_ghost_ring(&self) -> Color32 {
        self.ghost_ring
            .as_deref()
            .and_then(|h| parse_hex_color(h).ok())
            .unwrap_or_else(|| Color32::from_rgb(255, 165, 0))
    }

    /// Resolves parsed LED readout color, or defaults to high-contrast amber (#FFB300).
    #[must_use]
    pub fn resolved_led_readout(&self) -> Color32 {
        self.led_readout
            .as_deref()
            .and_then(|h| parse_hex_color(h).ok())
            .unwrap_or(NORD_AMBER_LED)
    }
}

/// Clamped UI scaling between 50% and 200%.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UiScale(f32);

impl Default for UiScale {
    fn default() -> Self {
        Self(1.0)
    }
}

impl UiScale {
    /// Minimum allowed scaling (50%).
    pub const MIN_FACTOR: f32 = 0.50;
    /// Maximum allowed scaling (200%).
    pub const MAX_FACTOR: f32 = 2.00;

    /// Creates a new `UiScale` clamped within the 0.50 to 2.00 range.
    #[must_use]
    pub fn new(factor: f32) -> Self {
        Self(factor.clamp(Self::MIN_FACTOR, Self::MAX_FACTOR))
    }

    /// Creates a new `UiScale` from an integer percentage (50 to 200).
    #[must_use]
    pub fn from_percent(percent: u32) -> Self {
        let p = percent.clamp(50, 200) as f32;
        Self(p / 100.0)
    }

    /// Returns the zoom scale factor as f32.
    #[must_use]
    pub const fn factor(&self) -> f32 {
        self.0
    }

    /// Returns the zoom scale as integer percentage (50 to 200).
    #[must_use]
    pub fn percent(&self) -> u32 {
        (self.0 * 100.0).round() as u32
    }

    /// Applies this zoom factor to the egui Context.
    pub fn apply(&self, ctx: &Context) {
        ctx.set_zoom_factor(self.0);
    }
}

/// Applies zero-rounding crisp borders across all widget visuals.
pub fn apply_crisp_zero_rounding(visuals: &mut Visuals) {
    visuals.widgets.noninteractive.rounding = Rounding::ZERO;
    visuals.widgets.inactive.rounding = Rounding::ZERO;
    visuals.widgets.hovered.rounding = Rounding::ZERO;
    visuals.widgets.active.rounding = Rounding::ZERO;
    visuals.widgets.open.rounding = Rounding::ZERO;
    visuals.window_rounding = Rounding::ZERO;
    visuals.menu_rounding = Rounding::ZERO;
}

/// Configured theme manager coordinating yaae-theme preferences, twoBowls backgrounds,
/// synthesizer accent profiles, custom color overrides, crisp borders, and UI scaling.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeConfig {
    /// Selected yaae-theme palette variant (15 variants).
    pub preference: ThemePreference,
    /// Optional twoBowls background preset.
    pub two_bowls_bg: Option<TwoBowlsBackground>,
    /// Synthesizer accent color profile.
    pub accent: SynthAccent,
    /// Custom hex color overrides.
    pub overrides: CustomColorOverrides,
    /// UI scaling factor (50% to 200%).
    pub scale: UiScale,
    /// True black-and-white mode for high-contrast accessibility.
    pub true_black_and_white: bool,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            preference: ThemePreference::Default,
            two_bowls_bg: None,
            accent: SynthAccent::BassStationBlue,
            overrides: CustomColorOverrides::default(),
            scale: UiScale::default(),
            true_black_and_white: false,
        }
    }
}

impl ThemeConfig {
    /// Creates a new `ThemeConfig` with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the theme preference variant.
    #[must_use]
    pub fn with_preference(mut self, preference: ThemePreference) -> Self {
        self.preference = preference;
        self
    }

    /// Sets the twoBowls background preset.
    #[must_use]
    pub fn with_background(mut self, bg: TwoBowlsBackground) -> Self {
        self.two_bowls_bg = Some(bg);
        self
    }

    /// Sets the synthesizer accent profile.
    #[must_use]
    pub fn with_accent(mut self, accent: SynthAccent) -> Self {
        self.accent = accent;
        self
    }

    /// Sets the UI zoom scale factor.
    #[must_use]
    pub fn with_scale(mut self, scale: UiScale) -> Self {
        self.scale = scale;
        self
    }

    /// Resolves the primary accent color, prioritizing user overrides.
    #[must_use]
    pub fn resolved_accent_color(&self) -> Color32 {
        self.overrides
            .resolved_accent()
            .unwrap_or_else(|| self.accent.primary_color())
    }

    /// Resolves the LED readout color, prioritizing user overrides.
    #[must_use]
    pub fn resolved_readout_color(&self) -> Color32 {
        self.overrides
            .led_readout
            .as_deref()
            .and_then(|h| parse_hex_color(h).ok())
            .unwrap_or_else(|| self.accent.readout_color())
    }

    /// Generates full egui `Visuals` for the current configuration.
    #[must_use]
    pub fn build_visuals(&self) -> Visuals {
        let is_light = self.preference.is_light()
            || self
                .two_bowls_bg
                .as_ref()
                .is_some_and(|bg| bg.is_light());

        let mut vis = if is_light {
            Visuals::light()
        } else {
            Visuals::dark()
        };

        // 1. Apply yaae-theme base visuals if this theme has an interface base
        let _ = ui_visuals::apply_ui_visuals(self.preference, &mut vis);

        // 2. High contrast specific hand-written visual stylings
        match self.preference {
            ThemePreference::HighContrastBw => {
                vis.panel_fill = Color32::BLACK;
                vis.window_fill = Color32::BLACK;
                vis.extreme_bg_color = Color32::BLACK;
                vis.override_text_color = Some(Color32::WHITE);
                vis.widgets.noninteractive.bg_fill = Color32::BLACK;
                vis.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::WHITE);
                vis.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Color32::WHITE);
                vis.widgets.inactive.bg_fill = Color32::BLACK;
                vis.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::WHITE);
                vis.widgets.inactive.fg_stroke = Stroke::new(1.0, Color32::WHITE);
                vis.widgets.hovered.bg_fill = Color32::from_gray(40);
                vis.widgets.hovered.bg_stroke = Stroke::new(1.5, Color32::WHITE);
                vis.widgets.hovered.fg_stroke = Stroke::new(1.5, Color32::WHITE);
                vis.widgets.active.bg_fill = Color32::WHITE;
                vis.widgets.active.bg_stroke = Stroke::new(1.5, Color32::WHITE);
                vis.widgets.active.fg_stroke = Stroke::new(1.5, Color32::BLACK);
            }
            ThemePreference::HighContrastLight => {
                vis.panel_fill = Color32::WHITE;
                vis.window_fill = Color32::WHITE;
                vis.extreme_bg_color = Color32::WHITE;
                vis.override_text_color = Some(Color32::BLACK);
                vis.widgets.noninteractive.bg_fill = Color32::WHITE;
                vis.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::BLACK);
                vis.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Color32::BLACK);
                vis.widgets.inactive.bg_fill = Color32::from_gray(245);
                vis.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::BLACK);
                vis.widgets.inactive.fg_stroke = Stroke::new(1.0, Color32::BLACK);
                vis.widgets.hovered.bg_fill = Color32::from_gray(225);
                vis.widgets.hovered.bg_stroke = Stroke::new(1.5, Color32::BLACK);
                vis.widgets.hovered.fg_stroke = Stroke::new(1.5, Color32::BLACK);
                vis.widgets.active.bg_fill = Color32::BLACK;
                vis.widgets.active.bg_stroke = Stroke::new(1.5, Color32::BLACK);
                vis.widgets.active.fg_stroke = Stroke::new(1.5, Color32::WHITE);
            }
            ThemePreference::HighContrastColor => {
                let dark_bg = Color32::from_rgb(5, 5, 10);
                let cyan = Color32::from_rgb(0, 255, 255);
                let yellow = Color32::from_rgb(255, 255, 0);
                vis.panel_fill = dark_bg;
                vis.window_fill = dark_bg;
                vis.extreme_bg_color = Color32::BLACK;
                vis.override_text_color = Some(yellow);
                vis.hyperlink_color = cyan;
                vis.widgets.noninteractive.bg_fill = dark_bg;
                vis.widgets.noninteractive.bg_stroke = Stroke::new(1.0, cyan);
                vis.widgets.noninteractive.fg_stroke = Stroke::new(1.0, yellow);
                vis.widgets.inactive.bg_fill = Color32::from_rgb(20, 20, 30);
                vis.widgets.inactive.bg_stroke = Stroke::new(1.0, cyan);
                vis.widgets.inactive.fg_stroke = Stroke::new(1.0, yellow);
                vis.widgets.hovered.bg_fill = Color32::from_rgb(40, 40, 60);
                vis.widgets.hovered.bg_stroke = Stroke::new(1.5, yellow);
                vis.widgets.hovered.fg_stroke = Stroke::new(1.5, Color32::WHITE);
                vis.widgets.active.bg_fill = cyan;
                vis.widgets.active.bg_stroke = Stroke::new(1.5, Color32::WHITE);
                vis.widgets.active.fg_stroke = Stroke::new(1.5, Color32::BLACK);
            }
            _ => {}
        }

        // 3. Apply twoBowls background preset if selected
        if let Some(bg) = self.two_bowls_bg {
            let bg_color = bg.color32();
            vis.panel_fill = bg_color;
            vis.window_fill = bg_color;
            if bg.is_light() {
                vis.override_text_color = Some(Color32::from_rgb(24, 24, 24));
            } else {
                vis.override_text_color = Some(Color32::from_rgb(230, 230, 230));
            }
        }

        // 4. Apply primary synth accent to selection highlight and hyperlinks
        let accent_color = self
            .overrides
            .resolved_accent()
            .unwrap_or_else(|| self.accent.primary_color());
        vis.selection.bg_fill = accent_color;
        vis.hyperlink_color = accent_color;

        // 5. Apply custom hex color overrides
        if let Some(bg) = self.overrides.resolved_background() {
            vis.panel_fill = bg;
            vis.window_fill = bg;
        }
        if let Some(panel) = self.overrides.resolved_panel() {
            vis.panel_fill = panel;
        }
        if let Some(text) = self.overrides.resolved_text() {
            vis.override_text_color = Some(text);
        }

        // 6. Enforce crisp zero-rounding borders everywhere
        apply_crisp_zero_rounding(&mut vis);

        vis
    }

    /// Installs the full visual styling, palette, and zoom scale onto an egui Context.
    pub fn apply_to_context(&self, ctx: &Context) {
        theme_palette::set_true_black_and_white(self.true_black_and_white);
        let palette = AccentPalette::for_theme(&self.preference, self.true_black_and_white);
        palette.install(ctx);

        let visuals = self.build_visuals();
        ctx.set_visuals(visuals);

        self.scale.apply(ctx);
    }
}

/// Generates the standard JSON Schema (Draft-07) for `ThemeConfig`.
pub fn theme_config_json_schema() -> serde_json::Value {
    serde_json::json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "ThemeConfig",
        "description": "Unified visualizer and editor theme palette configuration across all 4 synth projects",
        "type": "object",
        "required": ["preference", "accent", "overrides", "scale", "true_black_and_white"],
        "properties": {
            "preference": { "type": "string" },
            "two_bowls_bg": { "type": ["string", "null"] },
            "accent": { "type": "string" },
            "overrides": { "type": "object" },
            "scale": { "type": "number", "minimum": 0.5, "maximum": 2.0 },
            "true_black_and_white": { "type": "boolean" }
        }
    })
}

/// Validates and parses JSON into a `ThemeConfig`.
pub fn validate_theme_json(json_str: &str) -> Result<ThemeConfig, serde_json::Error> {
    serde_json::from_str(json_str)
}

/// Exports theme schemas and canonical themes across the 4 synthesizer projects.
pub fn export_all_theme_schemas(dir: &std::path::Path) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(dir)?;
    if let Ok(json) = serde_json::to_string_pretty(&theme_config_json_schema()) {
        let _ = std::fs::write(dir.join("theme-config.schema.json"), json);
    }
    // 1. ASM Hydrasynth Theme (Volcanic Orange + Dark Grey)
    let hydra_theme = ThemeConfig::new()
        .with_accent(SynthAccent::Volcanic)
        .with_background(TwoBowlsBackground::DarkGrey);
    if let Ok(json) = serde_json::to_string_pretty(&hydra_theme) {
        let _ = std::fs::write(dir.join("asm-hydrasynth.theme.json"), json);
    }
    // 2. nkEditor Theme (Arctic Ice Blue + White)
    let nk_theme = ThemeConfig::new()
        .with_accent(SynthAccent::Arctic)
        .with_background(TwoBowlsBackground::White);
    if let Ok(json) = serde_json::to_string_pretty(&nk_theme) {
        let _ = std::fs::write(dir.join("nkeditor.theme.json"), json);
    }
    // 3. BS2 + TD-3 Theme (Acid Yellow + Pitch Black)
    let acid_theme = ThemeConfig::new()
        .with_accent(SynthAccent::AcidYellowTD3)
        .with_background(TwoBowlsBackground::Black);
    if let Ok(json) = serde_json::to_string_pretty(&acid_theme) {
        let _ = std::fs::write(dir.join("bs2-td3.theme.json"), json);
    }
    // 4. Nord Drum 2 Theme (Nord Crimson + Warm Cream)
    let nd2_theme = ThemeConfig::new()
        .with_accent(SynthAccent::NordCrimson)
        .with_background(TwoBowlsBackground::Cream);
    if let Ok(json) = serde_json::to_string_pretty(&nd2_theme) {
        let _ = std::fs::write(dir.join("clavia-nord-drum-2.theme.json"), json);
    }
    // 5. Novation Peak Theme (Peak Oxford Blue + Dark Grey)
    let peak_theme = ThemeConfig::new()
        .with_accent(SynthAccent::PeakOxfordBlue)
        .with_background(TwoBowlsBackground::DarkGrey);
    if let Ok(json) = serde_json::to_string_pretty(&peak_theme) {
        let _ = std::fs::write(dir.join("novation-peak.theme.json"), json);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_15_yaae_theme_variants_are_present() {
        let all = ThemePreference::all();
        assert_eq!(all.len(), 15);
        let expected_names = [
            "Default",
            "Light",
            "Dark",
            "Solarized Dark",
            "Monokai",
            "Dracula",
            "Nord",
            "Gruvbox Dark",
            "Light Grayscale",
            "Dark Grayscale",
            "High Contrast B/W",
            "High Contrast Color",
            "High Contrast Light",
            "Cream Low Contrast",
            "Medium Contrast",
        ];
        for (i, theme) in all.iter().enumerate() {
            assert_eq!(theme.display_name(), expected_names[i]);
        }
    }

    #[test]
    fn four_two_bowls_background_presets() {
        let presets = TwoBowlsBackground::all();
        assert_eq!(presets.len(), 4);
        assert_eq!(TwoBowlsBackground::White.hex(), "#FFFFFF");
        assert_eq!(TwoBowlsBackground::Cream.hex(), "#F8F2E4");
        assert_eq!(TwoBowlsBackground::Black.hex(), "#000000");
        assert_eq!(TwoBowlsBackground::DarkGrey.hex(), "#181818");

        assert_eq!(
            TwoBowlsBackground::White.color32(),
            Color32::from_rgb(255, 255, 255)
        );
        assert_eq!(
            TwoBowlsBackground::Cream.color32(),
            Color32::from_rgb(248, 242, 228)
        );
        assert_eq!(
            TwoBowlsBackground::Black.color32(),
            Color32::from_rgb(0, 0, 0)
        );
        assert_eq!(
            TwoBowlsBackground::DarkGrey.color32(),
            Color32::from_rgb(24, 24, 24)
        );

        assert_eq!(
            TwoBowlsBackground::from_name("cream"),
            Some(TwoBowlsBackground::Cream)
        );
        assert_eq!(
            TwoBowlsBackground::from_name("dark-grey"),
            Some(TwoBowlsBackground::DarkGrey)
        );
    }

    #[test]
    fn all_seven_synth_accent_profiles() {
        let accents = SynthAccent::all();
        assert_eq!(accents.len(), 8);
        assert_eq!(SynthAccent::Midnight.hex(), "#4169E1");
        assert_eq!(SynthAccent::Cyberpunk.hex(), "#FF007F");
        assert_eq!(SynthAccent::Arctic.hex(), "#00D2FF");
        assert_eq!(SynthAccent::Volcanic.hex(), "#FF4500");
        assert_eq!(SynthAccent::BassStationBlue.hex(), "#0055A5");
        assert_eq!(SynthAccent::AcidYellowTD3.hex(), "#E6DB22");
        assert_eq!(SynthAccent::NordCrimson.hex(), "#E52521");
        assert_eq!(SynthAccent::PeakOxfordBlue.hex(), PEAK_OXFORD_BLUE_HEX);
        assert_eq!(SynthAccent::PeakOxfordBlue.readout_hex(), PEAK_AMBER_OLED_HEX);
        assert_eq!(
            SynthAccent::from_name("peak"),
            Some(SynthAccent::PeakOxfordBlue)
        );

        assert_eq!(
            primary_color(SynthAccent::BassStationBlue),
            Color32::from_rgb(0x00, 0x55, 0xA5)
        );
        assert_eq!(
            primary_color(SynthAccent::AcidYellowTD3),
            Color32::from_rgb(0xE6, 0xDB, 0x22)
        );
        assert_eq!(
            primary_color(SynthAccent::NordCrimson),
            Color32::from_rgb(0xE5, 0x25, 0x21)
        );

        assert_eq!(
            SynthAccent::from_name("bs2"),
            Some(SynthAccent::BassStationBlue)
        );
        assert_eq!(
            SynthAccent::from_name("acid"),
            Some(SynthAccent::AcidYellowTD3)
        );
        assert_eq!(
            SynthAccent::from_name("nd2"),
            Some(SynthAccent::NordCrimson)
        );
        assert_eq!(
            SynthAccent::from_name("nord crimson"),
            Some(SynthAccent::NordCrimson)
        );

        // Clavia Nord Drum 2 hardware accent and amber LED readout
        assert_eq!(SynthAccent::NordCrimson.readout_hex(), "#FFB300");
        assert_eq!(
            SynthAccent::NordCrimson.readout_color(),
            Color32::from_rgb(0xFF, 0xB3, 0x00)
        );
        assert_eq!(readout_color(SynthAccent::NordCrimson), NORD_AMBER_LED);
        assert_eq!(NORD_CRIMSON_HEX, "#E52521");
        assert_eq!(NORD_AMBER_LED_HEX, "#FFB300");
        assert_eq!(NORD_CRIMSON_PRIMARY, Color32::from_rgb(0xE5, 0x25, 0x21));
        assert_eq!(NORD_AMBER_LED, Color32::from_rgb(0xFF, 0xB3, 0x00));

        // ThemeConfig default resolution for NordCrimson
        let cfg = ThemeConfig::new().with_accent(SynthAccent::NordCrimson);
        assert_eq!(cfg.resolved_accent_color(), NORD_CRIMSON_PRIMARY);
        assert_eq!(cfg.resolved_readout_color(), NORD_AMBER_LED);
    }

    #[test]
    fn hex_color_parsing_and_formatting() {
        assert_eq!(
            parse_hex_color("#FFFFFF"),
            Ok(Color32::from_rgb(255, 255, 255))
        );
        assert_eq!(parse_hex_color("000000"), Ok(Color32::from_rgb(0, 0, 0)));
        assert_eq!(
            parse_hex_color("#F8F2E4"),
            Ok(Color32::from_rgb(248, 242, 228))
        );
        assert_eq!(parse_hex_color("#FFF"), Ok(Color32::from_rgb(255, 255, 255)));
        assert_eq!(
            parse_hex_color("#FF000080"),
            Ok(Color32::from_rgba_premultiplied(255, 0, 0, 128))
        );

        assert_eq!(
            format_hex_color(Color32::from_rgb(255, 255, 255), false),
            "#FFFFFF"
        );
        assert_eq!(
            format_hex_color(Color32::from_rgb(24, 24, 24), false),
            "#181818"
        );
        assert_eq!(
            format_hex_color(Color32::from_rgba_premultiplied(255, 0, 0, 128), true),
            "#FF000080"
        );

        assert!(matches!(parse_hex_color(""), Err(HexColorError::Empty)));
        assert!(matches!(
            parse_hex_color("#12"),
            Err(HexColorError::InvalidLength(2))
        ));
        assert!(matches!(
            parse_hex_color("#GGGGGG"),
            Err(HexColorError::InvalidDigit('G'))
        ));
    }

    #[test]
    fn custom_color_overrides_resolutions() {
        let mut overrides = CustomColorOverrides::new();
        assert_eq!(
            overrides.resolved_solo_led(),
            Color32::from_rgb(255, 176, 0)
        );
        assert_eq!(overrides.resolved_mute_led(), Color32::from_rgb(255, 32, 32));
        assert_eq!(overrides.resolved_rec_led(), Color32::from_rgb(255, 0, 64));
        assert_eq!(overrides.resolved_step_led(), Color32::from_rgb(255, 48, 48));
        assert_eq!(
            overrides.resolved_ghost_ring(),
            Color32::from_rgb(255, 165, 0)
        );
        assert_eq!(
            overrides.resolved_led_readout(),
            NORD_AMBER_LED
        );

        overrides.solo_led = Some("#00FF00".to_string());
        assert_eq!(overrides.resolved_solo_led(), Color32::from_rgb(0, 255, 0));

        overrides.led_readout = Some("#00FFFF".to_string());
        assert_eq!(overrides.resolved_led_readout(), Color32::from_rgb(0, 255, 255));
    }

    #[test]
    fn zero_rounding_crisp_borders_enforced() {
        let config = ThemeConfig::default();
        let visuals = config.build_visuals();
        assert_eq!(visuals.widgets.noninteractive.rounding, Rounding::ZERO);
        assert_eq!(visuals.widgets.inactive.rounding, Rounding::ZERO);
        assert_eq!(visuals.widgets.hovered.rounding, Rounding::ZERO);
        assert_eq!(visuals.widgets.active.rounding, Rounding::ZERO);
        assert_eq!(visuals.widgets.open.rounding, Rounding::ZERO);
        assert_eq!(visuals.window_rounding, Rounding::ZERO);
        assert_eq!(visuals.menu_rounding, Rounding::ZERO);
    }

    #[test]
    fn ui_scaling_clamped_bounds() {
        let s1 = UiScale::new(0.20);
        assert_eq!(s1.factor(), 0.50);
        assert_eq!(s1.percent(), 50);

        let s2 = UiScale::new(3.00);
        assert_eq!(s2.factor(), 2.00);
        assert_eq!(s2.percent(), 200);

        let s3 = UiScale::from_percent(150);
        assert_eq!(s3.factor(), 1.50);
        assert_eq!(s3.percent(), 150);

        let s4 = UiScale::from_percent(30);
        assert_eq!(s4.factor(), 0.50);
    }

    #[test]
    fn apply_to_context_works() {
        let ctx = Context::default();
        let config = ThemeConfig {
            preference: ThemePreference::Nord,
            two_bowls_bg: Some(TwoBowlsBackground::Cream),
            accent: SynthAccent::Arctic,
            overrides: CustomColorOverrides::default(),
            scale: UiScale::from_percent(125),
            true_black_and_white: false,
        };
        config.apply_to_context(&ctx);
        let _ = ctx.run(RawInput::default(), |_| {});
        assert_eq!(ctx.zoom_factor(), 1.25);
    }

    #[test]
    fn test_theme_schema_and_export_roundtrip() {
        let schema = theme_config_json_schema();
        assert_eq!(schema["title"], "ThemeConfig");

        let theme = ThemeConfig::new()
            .with_accent(SynthAccent::NordCrimson)
            .with_background(TwoBowlsBackground::Cream);
        let json_str = serde_json::to_string(&theme).expect("Theme to JSON failed");
        let restored = validate_theme_json(&json_str).expect("Validate theme JSON failed");
        assert_eq!(restored.accent, SynthAccent::NordCrimson);
        assert_eq!(restored.two_bowls_bg, Some(TwoBowlsBackground::Cream));

        let temp_dir = std::env::temp_dir().join(format!("synth_ui_theme_schema_test_{}", theme.scale.percent()));
        export_all_theme_schemas(&temp_dir).expect("Export theme schemas failed");

        assert!(temp_dir.join("theme-config.schema.json").exists());
        assert!(temp_dir.join("asm-hydrasynth.theme.json").exists());
        assert!(temp_dir.join("nkeditor.theme.json").exists());
        assert!(temp_dir.join("bs2-td3.theme.json").exists());
        assert!(temp_dir.join("clavia-nord-drum-2.theme.json").exists());
        assert!(temp_dir.join("novation-peak.theme.json").exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
