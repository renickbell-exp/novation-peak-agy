// SPDX-License-Identifier: GPL-3.0-or-later
//! `novation-peak-manager`: Native Rust/egui desktop synthesizer workstation,
//! patch librarian, Korg nanoKONTROL2 controller surface, and Konduktiva live-coding
//! companion for the Novation Peak 8-voice polyphonic hybrid synthesizer.

pub mod controller;
pub mod gui;
pub mod librarian;
pub mod midi;
pub mod mod_matrix;
pub mod params;
pub mod sysex;

pub use synth_core;
pub use synth_midi_osc;
pub use synth_ui_theme;

/// Returns the canonical synthesizer identifier (`"novation_peak"`).
pub fn synth_id() -> &'static str {
    "novation_peak"
}

/// Returns the application title for the Novation Peak Manager.
pub fn app_title() -> &'static str {
    "Novation Peak Manager & Konduktiva Companion"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_title_and_epoch1_modules() {
        assert!(app_title().contains("Novation Peak"));
        let count = params::verify_profile_parity_with_cc_map().expect("CC map parity");
        assert_eq!(count, 74);
    }
}
