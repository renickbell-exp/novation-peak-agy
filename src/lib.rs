//! Novation Peak 8-Voice Hybrid Synthesizer Manager, Librarian & Konduktiva Companion.
pub use synth_core;
pub use synth_midi_osc;
pub use synth_ui_theme;

/// Returns the canonical synthesizer identifier for this application.
pub fn synth_id() -> &'static str {
    "novation-peak"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synth_id_baseline() {
        assert_eq!(synth_id(), "novation-peak");
    }
}
