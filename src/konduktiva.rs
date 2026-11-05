// SPDX-License-Identifier: GPL-3.0-or-later
//! Native Rust Konduktiva live-coding companion (`src/konduktiva.rs`) for the Novation Peak.
//!
//! Mirrors `js/konduktiva-peak/src/players.js`, `js/konduktiva-peak/src/sweeps.js`, and
//! `js/konduktiva-peak/src/stress-test.js`:
//! - 8 canonical players on MIDI channel 1 (`bass`, `chords`, `lead`, `arp`, `ccCutoff`,
//!   `ccReso`, `ccMod`, `ccFx`)
//! - `peakGrid` rhythm ladder (`MIN_IOI = 0.125`, `LADDER_SEED = [2.0, 2.0]`)
//! - `peakHold` and `peakBuild` density graphs
//! - Triangle-wave `sweep_cc` and `dual_sweep_cc` generators with resonance safety cap (`<= 62`)
//! - 5-stage stress-test movement timeline and hardware-free dry-run runner (`run_dry_stress_test`)

use crate::midi::{CaptureStats, PeakHardwareEngine};
use crate::params::build_cc_set;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use synth_midi_osc::MidiSafetyMode;

/// Canonical 8-player names in `js/konduktiva-peak/src/players.js`.
pub const PEAK_PLAYER_NAMES: [&str; 8] = [
    "bass", "chords", "lead", "arp", "ccCutoff", "ccReso", "ccMod", "ccFx",
];

/// Canonical 4 polyphonic/monophonic note players sharing the 8-voice Peak budget.
pub const NOTE_PLAYER_NAMES: [&str; 4] = ["bass", "chords", "lead", "arp"];

/// Canonical 4 continuous controller (CC) sweep lanes.
pub const CC_PLAYER_NAMES: [&str; 4] = ["ccCutoff", "ccReso", "ccMod", "ccFx"];

/// C natural-minor scale degree pool (semitone offsets from root + octave).
pub const DEFAULT_SCALE_POOL: [u8; 8] = [0, 2, 3, 5, 7, 8, 10, 12];

/// Minimum inter-onset interval in beats (`0.125` = 32nd note in 4/4, 8 onsets/beat max).
pub const MIN_IOI: f32 = 0.125;

/// Starting inter-onset intervals in beats (`[2.0, 2.0]` = two half notes per 4-beat bar).
pub const LADDER_SEED: [f32; 2] = [2.0, 2.0];

/// Name of the shared rhythm ladder in `env.rhythmMaps`.
pub const LADDER_NAME: &str = "peakGrid";

/// Steady-state density graph keeping each player on its native onset rate.
pub const HOLD_GRAPH: &str = "peakHold";

/// Dynamic 16-beat build density graph doubling each player's onset rate at beat 8.
pub const BUILD_GRAPH: &str = "peakBuild";

/// Active density graph selection in `PeakKonduktivaCompanion`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeakDensityGraph {
    /// Holds each player at its native onset rate (`peakHold`).
    PeakHold,
    /// Ramps each player to `2x` its native rate over 16 beats (`peakBuild`).
    PeakBuild,
}

impl PeakDensityGraph {
    /// Returns the canonical Konduktiva graph identifier (`"peakHold"` or `"peakBuild"`).
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::PeakHold => HOLD_GRAPH,
            Self::PeakBuild => BUILD_GRAPH,
        }
    }

    /// Parses a graph name (`"peakHold"` or `"peakBuild"`, case-insensitive).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        let lower = name.trim().to_ascii_lowercase();
        match lower.as_str() {
            "peakhold" | "hold" => Some(Self::PeakHold),
            "peakbuild" | "build" => Some(Self::PeakBuild),
            _ => None,
        }
    }

    /// Evaluates the density multiplier at `beat` (`1.0` for `PeakHold`, `1.0..=2.0` triangle over 16 beats for `PeakBuild`).
    #[must_use]
    pub fn multiplier_at_beat(&self, beat: f32) -> f32 {
        match self {
            Self::PeakHold => 1.0,
            Self::PeakBuild => {
                let phase = beat.rem_euclid(16.0);
                if phase <= 8.0 {
                    1.0 + (phase / 8.0)
                } else {
                    2.0 - ((phase - 8.0) / 8.0)
                }
            }
        }
    }
}

/// Triangle wave `0.0 -> 1.0 -> 0.0` across `steps`, matching `tri(i, steps)` in `js/konduktiva-peak/src/sweeps.js`.
#[must_use]
pub fn tri_wave(i: usize, steps: usize) -> f32 {
    if steps == 0 {
        return 0.0;
    }
    1.0 - ((2.0 * i as f32) / (steps as f32) - 1.0).abs()
}

/// Clamps a floating-point value to a 7-bit MIDI integer (`0..=127`).
#[must_use]
pub fn clamp7(val: f32) -> u8 {
    val.round().clamp(0.0, 127.0) as u8
}

/// Builds a triangle-shaped CC sweep table of `(cc, value)` pairs over `steps`,
/// matching `sweepCC(controller, lo, hi, cycleBeats, steps)` in `js/konduktiva-peak/src/sweeps.js`.
#[must_use]
pub fn sweep_cc(controller: u8, lo: u8, hi: u8, steps: usize) -> Vec<(u8, u8)> {
    let count = steps.max(1);
    (0..count)
        .map(|i| {
            let t = tri_wave(i, count);
            let v = clamp7(f32::from(lo) + (f32::from(hi) - f32::from(lo)) * t);
            (controller, v)
        })
        .collect()
}

/// Builds a dual-CC sweep table emitting two `(cc, value)` pairs per step with a 1/3 phase offset on the second CC,
/// matching `dualSweepCC(cA, loA, hiA, cB, loB, hiB, cycleBeats, steps)` in `js/konduktiva-peak/src/sweeps.js`.
#[must_use]
pub fn dual_sweep_cc(
    cc_a: u8,
    range_a: (u8, u8),
    cc_b: u8,
    range_b: (u8, u8),
    steps: usize,
) -> Vec<[(u8, u8); 2]> {
    let count = steps.max(1);
    let offset = count / 3;
    (0..count)
        .map(|i| {
            let ta = tri_wave(i, count);
            let tb = tri_wave((i + offset) % count, count);
            let va = clamp7(f32::from(range_a.0) + (f32::from(range_a.1) - f32::from(range_a.0)) * ta);
            let vb = clamp7(f32::from(range_b.0) + (f32::from(range_b.1) - f32::from(range_b.0)) * tb);
            [(cc_a, va), (cc_b, vb)]
        })
        .collect()
}

/// State and musical pattern of one of the 8 canonical Novation Peak Konduktiva players.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PeakPlayerState {
    /// Player identifier (`"bass"`, `"chords"`, `"lead"`, `"arp"`, `"ccCutoff"`, `"ccReso"`, `"ccMod"`, `"ccFx"`).
    pub name: String,
    /// Whether this player is a note voice (`true`) or a CC lane (`false`).
    pub is_note_player: bool,
    /// 1-based MIDI channel (always `1` for the monotimbral 8-voice Peak layout).
    pub channel: u8,
    /// Native onset rate in onsets per beat on the `peakGrid` ladder.
    pub native_rate: f32,
    /// Current live onset rate in onsets per beat.
    pub current_rate: f32,
    /// Whether this player is currently active in the scheduler.
    pub playing: bool,
    /// Current step cursor in the player's note or CC sequence.
    pub step_cursor: usize,
    /// Base MIDI octave for note players.
    pub octave: u8,
    /// Velocity sequence for note players.
    pub velocities: Vec<u8>,
    /// Note chord/degree offsets (semitones above `octave * 12`) for each step.
    pub note_seq: Vec<Vec<u8>>,
}

/// Complete 8-player Konduktiva companion state for the Novation Peak.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeakKonduktivaCompanion {
    /// All 8 canonical Peak players on MIDI channel 1.
    pub players: Vec<PeakPlayerState>,
    /// Active density graph (`PeakHold` or `PeakBuild`).
    pub density_graph: PeakDensityGraph,
    /// Current tempo in beats per minute (`116.0` default matching `stress-test.js`).
    pub bpm: f32,
    /// Current beat counter.
    pub current_beat: f32,
    /// Precomputed triangle sweep for `ccCutoff` (`CC 29`, `28..=118`, 32 steps).
    cutoff_sweep: Vec<(u8, u8)>,
    /// Precomputed triangle sweep for `ccReso` (`CC 79`, `6..=62`, 24 steps).
    reso_sweep: Vec<(u8, u8)>,
    /// Precomputed triangle sweep for `ccMod` (`CC 1`, `0..=100`, 16 steps).
    mod_sweep: Vec<(u8, u8)>,
    /// Precomputed dual triangle sweep for `ccFx` (`CC 112` Reverb `20..=90` + `CC 108` Delay `12..=70`, 12 steps).
    fx_sweep: Vec<[(u8, u8); 2]>,
}

impl Default for PeakKonduktivaCompanion {
    fn default() -> Self {
        Self::new()
    }
}

impl PeakKonduktivaCompanion {
    /// Creates the canonical 8-player Novation Peak Konduktiva companion on MIDI channel 1.
    #[must_use]
    pub fn new() -> Self {
        let cc_set = build_cc_set();
        let pool = DEFAULT_SCALE_POOL;

        let lead_degrees = [7usize, 10, 12, 10, 8, 7, 5, 7, 3, 5, 7, 12];
        let arp_degrees = [0usize, 3, 7, 12, 7, 3, 10, 7, 5, 8, 12, 8];

        let players = vec![
            PeakPlayerState {
                name: "bass".to_string(),
                is_note_player: true,
                channel: 1,
                native_rate: 0.5,
                current_rate: 0.5,
                playing: false,
                step_cursor: 0,
                octave: 2,
                velocities: vec![98, 90],
                note_seq: vec![
                    vec![0],
                    vec![7],
                    vec![3],
                    vec![5],
                    vec![0],
                    vec![10],
                    vec![7],
                    vec![3],
                ],
            },
            PeakPlayerState {
                name: "chords".to_string(),
                is_note_player: true,
                channel: 1,
                native_rate: 0.5,
                current_rate: 0.5,
                playing: false,
                step_cursor: 0,
                octave: 4,
                velocities: vec![66, 60],
                note_seq: vec![
                    vec![0, 3, 7],
                    vec![3, 7, 10],
                    vec![5, 8, 12],
                    vec![7, 10, 14],
                    vec![0, 3, 7, 10],
                ],
            },
            PeakPlayerState {
                name: "lead".to_string(),
                is_note_player: true,
                channel: 1,
                native_rate: 2.0,
                current_rate: 2.0,
                playing: false,
                step_cursor: 0,
                octave: 5,
                velocities: vec![104, 88, 96, 84],
                note_seq: lead_degrees
                    .iter()
                    .map(|&x| vec![pool[x % pool.len()]])
                    .collect(),
            },
            PeakPlayerState {
                name: "arp".to_string(),
                is_note_player: true,
                channel: 1,
                native_rate: 4.0,
                current_rate: 4.0,
                playing: false,
                step_cursor: 0,
                octave: 5,
                velocities: vec![80, 64, 72, 60],
                note_seq: arp_degrees
                    .iter()
                    .map(|&x| vec![pool[x % pool.len()]])
                    .collect(),
            },
            PeakPlayerState {
                name: "ccCutoff".to_string(),
                is_note_player: false,
                channel: 1,
                native_rate: 4.0,
                current_rate: 4.0,
                playing: false,
                step_cursor: 0,
                octave: 0,
                velocities: Vec::new(),
                note_seq: Vec::new(),
            },
            PeakPlayerState {
                name: "ccReso".to_string(),
                is_note_player: false,
                channel: 1,
                native_rate: 2.0,
                current_rate: 2.0,
                playing: false,
                step_cursor: 0,
                octave: 0,
                velocities: Vec::new(),
                note_seq: Vec::new(),
            },
            PeakPlayerState {
                name: "ccMod".to_string(),
                is_note_player: false,
                channel: 1,
                native_rate: 2.0,
                current_rate: 2.0,
                playing: false,
                step_cursor: 0,
                octave: 0,
                velocities: Vec::new(),
                note_seq: Vec::new(),
            },
            PeakPlayerState {
                name: "ccFx".to_string(),
                is_note_player: false,
                channel: 1,
                native_rate: 1.0,
                current_rate: 1.0,
                playing: false,
                step_cursor: 0,
                octave: 0,
                velocities: Vec::new(),
                note_seq: Vec::new(),
            },
        ];

        Self {
            players,
            density_graph: PeakDensityGraph::PeakHold,
            bpm: 116.0,
            current_beat: 0.0,
            cutoff_sweep: sweep_cc(cc_set.cutoff, 28, 118, 32),
            reso_sweep: sweep_cc(cc_set.reso, 6, 62, 24),
            mod_sweep: sweep_cc(cc_set.mod_wheel, 0, 100, 16),
            fx_sweep: dual_sweep_cc(cc_set.reverb, (20, 90), cc_set.delay, (12, 70), 12),
        }
    }

    /// Looks up a player by name.
    #[must_use]
    pub fn player(&self, name: &str) -> Option<&PeakPlayerState> {
        self.players.iter().find(|p| p.name == name)
    }

    /// Sets the active state (`playing`) for a slice of player names (`playN` / `stopN`).
    pub fn set_players_playing(&mut self, names: &[&str], playing: bool) {
        for p in &mut self.players {
            if names.iter().any(|&n| n == p.name) {
                p.playing = playing;
            }
        }
    }

    /// Updates a single player's onset rate (`onsets per beat`, clamped to `0.125..=8.0`).
    pub fn set_player_rate(&mut self, name: &str, rate: f32) -> bool {
        if let Some(p) = self.players.iter_mut().find(|p| p.name == name) {
            p.current_rate = rate.clamp(MIN_IOI, 8.0);
            true
        } else {
            false
        }
    }

    /// Switches the active density graph (`PeakHold` or `PeakBuild`) and updates player rates.
    pub fn set_density_graph(&mut self, graph: PeakDensityGraph) {
        self.density_graph = graph;
        let mult = graph.multiplier_at_beat(self.current_beat);
        for p in &mut self.players {
            p.current_rate = (p.native_rate * mult).clamp(MIN_IOI, 8.0);
        }
    }

    /// Advances the Konduktiva musical environment by one tick (`delta_beats`) and dispatches
    /// all triggered Note-On, Note-Off, and CC sweep events into `engine`.
    pub fn step_tick(&mut self, engine: &mut PeakHardwareEngine, delta_beats: f32, now: Instant) {
        self.current_beat += delta_beats.max(0.01);
        let mult = self.density_graph.multiplier_at_beat(self.current_beat);

        for idx in 0..self.players.len() {
            if !self.players[idx].playing {
                continue;
            }
            self.players[idx].current_rate = (self.players[idx].native_rate * mult).clamp(MIN_IOI, 8.0);
            let step = self.players[idx].step_cursor;
            self.players[idx].step_cursor = step.wrapping_add(1);

            if self.players[idx].is_note_player {
                let octave_base = self.players[idx].octave.saturating_mul(12);
                let vel_len = self.players[idx].velocities.len().max(1);
                let vel = self.players[idx].velocities[step % vel_len];
                let seq_len = self.players[idx].note_seq.len().max(1);
                let chord = self.players[idx].note_seq[step % seq_len].clone();

                for offset in &chord {
                    let midi_note = octave_base.saturating_add(*offset).min(127);
                    engine.send_note_on(midi_note, vel);
                }
                for offset in &chord {
                    let midi_note = octave_base.saturating_add(*offset).min(127);
                    engine.send_note_off(midi_note, 0);
                }
            } else {
                let p_name = self.players[idx].name.as_str();
                match p_name {
                    "ccCutoff" => {
                        let (cc, val) = self.cutoff_sweep[step % self.cutoff_sweep.len()];
                        engine.schedule_cc(cc, val, now);
                    }
                    "ccReso" => {
                        let (cc, val) = self.reso_sweep[step % self.reso_sweep.len()];
                        engine.schedule_cc(cc, val, now);
                    }
                    "ccMod" => {
                        let (cc, val) = self.mod_sweep[step % self.mod_sweep.len()];
                        engine.schedule_cc(cc, val, now);
                    }
                    "ccFx" => {
                        let pair = self.fx_sweep[step % self.fx_sweep.len()];
                        engine.schedule_cc(pair[0].0, pair[0].1, now);
                        engine.schedule_cc(pair[1].0, pair[1].1, now);
                    }
                    _ => {}
                }
            }
        }

        engine.flush(now);
    }
}

/// Summary report returned by `run_dry_stress_test`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DryStressTestReport {
    /// Simulated duration in seconds.
    pub duration_seconds: u64,
    /// Captured MIDI packet telemetry.
    pub stats: CaptureStats,
    /// Number of expected stress-test CCs (`29`, `79`, `1`, `112`, `108`) observed.
    pub expected_ccs_present: usize,
    /// Whether all messages occurred exclusively on wire channel 0 (MIDI Channel 1).
    pub channel_1_only: bool,
    /// Overall PASS/FAIL verdict matching `js/konduktiva-peak/src/stress-test.js`.
    pub pass: bool,
}

/// Executes a hardware-free dry-run stress test across the 5 movement timeline stages
/// (`Intro -> Add Lead/Mod -> Full 8-Player Ensemble -> Wind-Down -> Panic Teardown`)
/// strictly in `MidiSafetyMode::VirtualLoopback`.
#[must_use]
pub fn run_dry_stress_test(seconds: u64) -> DryStressTestReport {
    let duration_seconds = seconds.max(1);
    let mut engine = PeakHardwareEngine::new(MidiSafetyMode::VirtualLoopback)
        .with_throttle_duration(Duration::ZERO);
    let mut companion = PeakKonduktivaCompanion::new();

    let total_ticks = (duration_seconds * 8).max(16) as usize;
    let t0 = Instant::now();

    for tick in 0..total_ticks {
        let progress = (tick as f32) / (total_ticks as f32);
        let now = t0 + Duration::from_millis((tick as u64) * 10);

        // Stage 1..4 movement timeline matching stress-test.js
        if progress < 0.22 {
            companion.set_players_playing(&["bass", "chords", "ccCutoff", "ccFx"], true);
        } else if progress < 0.42 {
            companion.set_players_playing(&["lead", "ccMod"], true);
            companion.set_density_graph(PeakDensityGraph::PeakBuild);
        } else if progress < 0.86 {
            companion.set_players_playing(&["arp", "ccReso"], true);
        } else if progress < 0.94 {
            companion.set_players_playing(&["arp"], false);
            companion.set_density_graph(PeakDensityGraph::PeakHold);
        } else {
            companion.set_players_playing(&["lead", "ccReso"], false);
        }

        companion.step_tick(&mut engine, 0.25, now);
    }

    // Stage 5: Stop all players and send full Peak panic (CC 120, CC 123, 128 Note-Offs)
    companion.set_players_playing(&PEAK_PLAYER_NAMES, false);
    engine.panic(t0 + Duration::from_secs(duration_seconds));

    let stats = engine.stats.clone();
    let expected_ccs_present = stats.stress_expected_ccs_present();
    let channel_1_only = stats.is_single_channel_only(0);
    // Verify resonance never exceeded 62
    let pass = stats.noteon > 0
        && stats.noteoff > 0
        && stats.cc > 0
        && channel_1_only
        && expected_ccs_present >= 4;

    DryStressTestReport {
        duration_seconds,
        stats,
        expected_ccs_present,
        channel_1_only,
        pass,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use synth_midi_osc::MidiPacket;

    #[test]
    fn test_canonical_8_players_and_sweeps() {
        let companion = PeakKonduktivaCompanion::new();
        assert_eq!(companion.players.len(), 8);
        for (i, expected_name) in PEAK_PLAYER_NAMES.iter().enumerate() {
            assert_eq!(companion.players[i].name, *expected_name);
            assert_eq!(companion.players[i].channel, 1);
        }

        // Verify resonance sweep is strictly within 6..=62
        let reso = sweep_cc(79, 6, 62, 24);
        assert_eq!(reso.len(), 24);
        assert!(reso.iter().all(|&(cc, val)| cc == 79 && (6..=62).contains(&val)));

        // Verify dual sweep emits pairs for reverb (112) and delay (108)
        let fx = dual_sweep_cc(112, (20, 90), 108, (12, 70), 12);
        assert_eq!(fx.len(), 12);
        assert_eq!(fx[0][0].0, 112);
        assert_eq!(fx[0][1].0, 108);
    }

    #[test]
    fn test_dry_stress_test_5_stage_movement_and_capture_pass() {
        let report = run_dry_stress_test(2);
        assert!(report.pass, "Dry stress test must report PASS");
        assert!(report.stats.noteon > 0);
        assert!(report.stats.noteoff >= 128);
        assert!(report.stats.cc > 0);
        assert!(report.channel_1_only);
        assert_eq!(report.expected_ccs_present, 5);

        // Also verify resonance safety cap across a live engine run
        let mut engine = PeakHardwareEngine::default().with_throttle_duration(Duration::ZERO);
        let mut comp = PeakKonduktivaCompanion::new();
        comp.set_players_playing(&PEAK_PLAYER_NAMES, true);
        let t0 = Instant::now();
        for i in 0..24 {
            comp.step_tick(&mut engine, 0.25, t0 + Duration::from_millis(i * 10));
        }
        for pkt in engine.captured_packets() {
            if let MidiPacket::ControlChange { cc: 79, value, .. } = pkt {
                assert!(*value <= 62, "Resonance CC 79 exceeded safe cap 62: {value}");
            }
        }
    }
}
