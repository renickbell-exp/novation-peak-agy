---
title: "Epoch 5 of 6: Bidirectional UDP OSC Bridge, Konduktiva Live-Coding Companion, and Shared SDK Integration"
date: "2026-11-06"
project: "Novation Peak Manager & Konduktiva Companion (novation-peak-agy)"
project_dir: "/home/renick/antigravity/novation-peak-agy"
report_number: 5
subject: "Epoch 5 of 6: PeakKonduktivaCompanion, PeakOscCoordinator, js/shared-synth-sdk 5-Synth Extension, and js/konduktiva-peak OSC Bridge"
operator: "Antigravity Agent"
follows: ["report-001", "report-002", "report-003", "report-004"]
tldr: "Completed Epoch 5 of 6 by implementing the native Rust 8-player Konduktiva live-coding companion (src/konduktiva.rs) with peakGrid rhythm ladder, peakHold/peakBuild density graphs, triangle-wave CC sweeps, and 5-stage dry-run stress testing; the bidirectional UDP OSC coordinator (src/osc_bridge.rs); and cross-language Novation Peak extensions in js/shared-synth-sdk and js/konduktiva-peak. All 97 Rust tests and 16 Node.js assertions pass with zero compiler warnings while keeping hardware MIDI outputs strictly unarmed."
qa_verdict: "PASS"
tests_total: 113
---

# Prompt

```text
We want to build a complete Rust/egui native desktop synthesizer manager, patch editor, librarian, Korg nanoKONTROL2 controller, and Konduktiva live-coding companion for the Novation Peak 8-voice polyphonic hybrid synthesizer (similar to our Bass Station II and Nord Drum 2 apps), integrating the existing js/konduktiva-peak library (from ~/claude/novation-peak/) and extending the shared crates (crates/synth-core, crates/synth-midi-osc, crates/synth-ui-theme, js/shared-synth-sdk) across all 5 synthesizers. Plan the development yourself, including choosing the optimal number of epochs N, write your development roadmap to PLAN.md, and execute all N epochs sequentially from Epoch 1 of N through Epoch N of N. For every epoch k (1..N), ensure all Rust and Node.js tests pass (CARGO_BUILD_JOBS=2 cargo test --workspace and npm test), generate and compile a standard HTML/PDF report in reports/report-00k.md using /home/renick/claude/html-report-generator/build-report.sh --pdf, and create a git commit for that epoch. Keep all physical MIDI hardware output (Peak MIDI 1 hw:3,0,0 and UMC1820 MIDI 1 hw:0,0,0) strictly unarmed/silent during automated tests.
```

# Content

## 1. Plain-Language Overview of Epoch 5

In everyday terms, a live-coding musician using the Node.js **Konduktiva** algorithmic rhythm environment wants to play polyphonic chords, basslines, arpeggios, and smooth filter sweeps on the Novation Peak while seeing every parameter change, active voice, and rhythm density graph update in real time inside the Rust desktop workstation. Conversely, when the musician turns a knob on the Korg nanoKONTROL2 or clicks a preset in the desktop GUI, the Node.js live-coding session should immediately receive that update over the local network.

In **Epoch 5 of 6**, we connected the Rust desktop workstation and the Node.js live-coding ecosystem over **Open Sound Control (OSC)**, a fast binary message format sent over local User Datagram Protocol (UDP) network sockets (`127.0.0.1:9100` for commands and `127.0.0.1:9101` for subscriber broadcasts):

1. **Native Rust 8-Player Konduktiva Companion (`src/konduktiva.rs`)**: Mirrors the exact musical architecture of `js/konduktiva-peak/src/players.js`, `js/konduktiva-peak/src/sweeps.js`, and `js/konduktiva-peak/src/stress-test.js`. Because the Novation Peak is an 8-voice monotimbral synthesizer (meaning all 8 voices share one sound patch on MIDI Channel 1), the companion coordinates 4 note players (`bass`, `chords`, `lead`, `arp`) inside the 8-voice polyphony budget alongside 4 continuous controller sweep lanes (`ccCutoff`, `ccReso`, `ccMod`, `ccFx`). It includes the `peakGrid` rhythm ladder (`MIN_IOI = 0.125`), the `peakHold` and `peakBuild` density graphs, triangle-wave CC sweeps (`sweep_cc` and `dual_sweep_cc`), and the 5-stage stress-test movement timeline (`run_dry_stress_test`).
2. **Bidirectional UDP OSC Coordinator (`src/osc_bridge.rs`)**: Wraps `synth_midi_osc::OscBridge` into `PeakOscCoordinator` so incoming OSC messages from Node.js (`SharedSynthClient` or `konduktiva-peak`) update the `SynthPatch` with full `CommandHistory` undo/redo support, queue rate-thinned MIDI CCs, 14-bit NRPNs, polyphonic notes, and Animate 1/2 holds on `PeakHardwareEngine`, and switch `PeakKonduktivaCompanion` density graphs (`peakHold` vs `peakBuild`) and player onset rates.
3. **5-Synthesizer Extension of `js/shared-synth-sdk` & `js/konduktiva-peak`**: Added `js/shared-synth-sdk/novation-peak-router.js` (`PeakMidiRouter`, `PeakHardwareDiscovery`, `getPeakSafeSweepBounds`), extended `js/shared-synth-sdk/schema-validator.js` with `NOVATION_PEAK_PROFILE` and `NOVATION_PEAK_THEME` (`PeakOxfordBlue`), added 10 Novation Peak remote-control methods to `SharedSynthClient` in `js/shared-synth-sdk/synth-client.js`, and added `js/konduktiva-peak/src/osc-bridge.js` (`createPeakOscBridge`).

### Table: Plain-English Concept to Code Component Mapping

| Everyday Concept | What It Does for the Musician | Exact Code Symbol & File |
| --- | --- | --- |
| **8-Player Peak Ensemble** | Runs 4 note players (`bass`, `chords`, `lead`, `arp`) and 4 CC sweep lanes (`ccCutoff`, `ccReso`, `ccMod`, `ccFx`) on MIDI Channel 1 | `PeakKonduktivaCompanion`, `PEAK_PLAYER_NAMES` in `src/konduktiva.rs` |
| **Rhythm Density Graphs** | Switches all 8 voices between steady groove (`peakHold`) and a 16-beat 2x rhythmic build (`peakBuild`) | `PeakDensityGraph::PeakHold`, `PeakDensityGraph::PeakBuild` in `src/konduktiva.rs` |
| **Triangle CC Sweeps** | Generates smooth triangle-wave filter, resonance (`<= 62`), mod wheel, and dual reverb/delay sweeps | `sweep_cc()`, `dual_sweep_cc()`, `tri_wave()` in `src/konduktiva.rs` |
| **5-Stage Dry Stress Test** | Exercises all 8 players across 5 musical movement stages in `VirtualLoopback` mode and verifies capture stats | `run_dry_stress_test()`, `DryStressTestReport` in `src/konduktiva.rs` |
| **Bidirectional OSC Bridge** | Translates UDP OSC packets (`/synth/peak/*`, `/synth/konduktiva/*`) into patch edits, MIDI commands, and GUI broadcasts | `PeakOscCoordinator::poll_and_dispatch()` in `src/osc_bridge.rs` |
| **SDK Peak Router & Profile** | Brings Novation Peak 8-voice routing, ALSA discovery, profile, and `PeakOxfordBlue` theme to Node.js | `PeakMidiRouter`, `NOVATION_PEAK_PROFILE` in `js/shared-synth-sdk/` |
| **Konduktiva OSC Bridge** | Forwards live-coding Note/CC/Density events from `konduktiva-peak` to the Rust manager over UDP OSC | `createPeakOscBridge()` in `js/konduktiva-peak/src/osc-bridge.js` |

## 2. Native Rust 8-Player Konduktiva Companion (`src/konduktiva.rs`)

### Phase: Rhythm Ladder, Density Graphs, and 5-Stage Dry Stress Runner

In `src/konduktiva.rs`, we implemented `PeakKonduktivaCompanion` with exact structural parity against `js/konduktiva-peak/src/players.js`:

- **Note Voice 1 (`bass`)**: Octave 2, velocities `[98, 90]`, native rate `0.5` onsets/beat (half notes), root-and-fifth bass pattern.
- **Note Voice 2 (`chords`)**: Octave 4, velocities `[66, 60]`, native rate `0.5` onsets/beat, 3- and 4-note C-minor triads and seventh chords.
- **Note Voice 3 (`lead`)**: Octave 5, velocities `[104, 88, 96, 84]`, native rate `2.0` onsets/beat (eighth notes), C natural-minor melody (`DEFAULT_SCALE_POOL = [0, 2, 3, 5, 7, 8, 10, 12]`).
- **Note Voice 4 (`arp`)**: Octave 5, velocities `[80, 64, 72, 60]`, native rate `4.0` onsets/beat (sixteenth notes), 12-step minor arpeggio.
- **CC Sweep Lane 1 (`ccCutoff`)**: Triangle sweep on `CC 29` (`28..=118` over 32 steps, rate `4.0`).
- **CC Sweep Lane 2 (`ccReso`)**: Triangle sweep on `CC 79` (`6..=62` over 24 steps, rate `2.0`, strictly respecting the resonance self-oscillation safety cap).
- **CC Sweep Lane 3 (`ccMod`)**: Triangle sweep on `CC 1` (`0..=100` over 16 steps, rate `2.0`).
- **CC Sweep Lane 4 (`ccFx`)**: Dual triangle sweep on `CC 112` (Reverb Level `20..=90`) and `CC 108` (Delay Level `12..=70`) with a 1/3 cycle phase offset over 12 steps (`dual_sweep_cc`, rate `1.0`).
- **5-Stage Dry Stress Test (`run_dry_stress_test`)**: Stages `bass + chords + ccCutoff + ccFx` (Stage 1), adds `lead + ccMod` and switches to `PeakBuild` (Stage 2), adds `arp + ccReso` for the full 8-player ensemble (Stage 3), winds down `arp`, `lead`, and `ccReso` back to `PeakHold` (Stage 4), and executes a 130-packet `PeakHardwareEngine::panic` teardown (Stage 5) in `MidiSafetyMode::VirtualLoopback`.

## 3. Bidirectional UDP OSC Coordinator (`src/osc_bridge.rs`) & JS SDK Extensions

### Phase: `PeakOscCoordinator` and 5-Synth `js/shared-synth-sdk` Parity

`PeakOscCoordinator` in `src/osc_bridge.rs` binds `synth_midi_osc::OscBridge` and dispatches 10 OSC address families:

### Table: Supported Novation Peak & Konduktiva OSC Address Space

| OSC Address Pattern | Arguments | Action in `PeakOscCoordinator` |
| --- | --- | --- |
| `/synth/peak/param/<name>` | `<val:f32>` | Updates `SynthPatch` via `CommandHistory::push_and_apply`, schedules CC on `PeakHardwareEngine`, and broadcasts to subscribers |
| `/synth/peak/cc` | `<cc:i32> <val:i32>` | Schedules 7-bit CC on `PeakHardwareEngine` (with `SafeSweepBounds` clamping) and broadcasts `/synth/peak/cc` |
| `/synth/peak/nrpn` | `<param:i32> <val:i32>` | Queues 4-message 14-bit NRPN (`CC 99`, `CC 98`, `CC 6`, `CC 38`) and broadcasts `/synth/peak/nrpn` |
| `/synth/peak/note/on` | `<note:i32> <vel:i32>` | Routes polyphonic Note-On with 8-voice allocation on `PeakHardwareEngine` |
| `/synth/peak/note/off` | `<note:i32> <vel:i32>` | Routes polyphonic Note-Off release on `PeakHardwareEngine` |
| `/synth/peak/animate` | `<idx:1..2> <held:0..1>` | Toggles Animate 1 (`CC 114`) or Animate 2 (`CC 115`) hold on `PeakHardwareEngine` |
| `/synth/peak/patch/load` | `<bank:str> <slot:i32>` | Sends Bank Select (`CC 0 = 0`, `CC 32 = 1..4`) + Program Change (`0..127`) |
| `/synth/peak/panic` | `(none)` | Flushes 130-packet All-Sound-Off / All-Notes-Off panic and broadcasts `/synth/peak/panic` |
| `/synth/konduktiva/density_graph` | `<graph:str>` | Switches `PeakKonduktivaCompanion` between `peakHold` and `peakBuild` |
| `/synth/konduktiva/player_rate` | `<player:str> <rate:f32>` | Adjusts individual player onset rate (`0.125..=8.0` onsets/beat) on the `peakGrid` ladder |

In `js/shared-synth-sdk`, we added `novation-peak-router.js`, updated `schema-validator.js` to export `novation-peak.profile.json` and `novation-peak.theme.json` alongside the existing 4 synthesizers, updated `ACCENT_PROFILES` in `js/shared-synth-sdk/index.js` to all 8 accents (`PeakOxfordBlue`, `#1A73E8` primary, `#FF8C00` OLED amber), and added matching `SharedSynthClient` methods (`setPeakParam`, `sendPeakCc`, `sendPeakNrpn`, `sendPeakNoteOn`, `sendPeakNoteOff`, `setPeakAnimate`, `loadPeakPatch`, `setPeakDensityGraph`, `setPeakPlayerRate`, `setPeakPlayerState`).

# Files Touched

- `src/konduktiva.rs` (Created: `PeakKonduktivaCompanion`, `PeakPlayerState`, `PeakDensityGraph`, `sweep_cc`, `dual_sweep_cc`, `run_dry_stress_test`, `DryStressTestReport`, and 2 unit tests)
- `src/osc_bridge.rs` (Created: `PeakOscCoordinator` wrapping `synth_midi_osc::OscBridge` with 10 OSC command handlers and end-to-end UDP loopback test)
- `src/params.rs` (Modified: Added `PeakCoreCcSet`, `resolve_cc`, and `build_cc_set` convenience functions)
- `src/lib.rs` (Modified: Registered `pub mod konduktiva;` and `pub mod osc_bridge;`)
- `js/shared-synth-sdk/novation-peak-router.js` (Created: `PeakMidiRouter`, `PeakHardwareDiscovery`, and `getPeakSafeSweepBounds`)
- `js/shared-synth-sdk/schema-validator.js` (Modified: Added `NOVATION_PEAK_PROFILE`, `NOVATION_PEAK_THEME`, and 5-synthesizer exports in `exportAllSchemas`)
- `js/shared-synth-sdk/synth-client.js` (Modified: Added `peakParams` state and 10 Novation Peak & Konduktiva OSC methods on `SharedSynthClient`)
- `js/shared-synth-sdk/index.js` (Modified: Exported `novation-peak-router.js`, `PEAK_OXFORD_BLUE_HEX`, `PEAK_AMBER_OLED_HEX`, and added `PeakOxfordBlue` to `ACCENT_PROFILES`)
- `js/konduktiva-peak/src/osc-bridge.js` (Created: `createPeakOscBridge` with `safeSweepRange` resonance clamping and `SharedSynthClient` forwarding)
- `js/konduktiva-peak/src/index.js` (Modified: Exported `createPeakOscBridge`)

# Files Not Touched

- `crates/synth-core/src/lib.rs` (Untouched in Epoch 5; `CommandHistory` and `SynthPatch` reused directly)
- `crates/synth-ui-theme/src/lib.rs` (Untouched in Epoch 5)
- `js/konduktiva-peak/midi-cc-map.json` (Read-only hardware CC specification)

# Tests

- **`novation-peak-manager` (`cargo test -p novation-peak-manager`)**: 18 passed, 0 failed (`test_canonical_8_players_and_sweeps`, `test_dry_stress_test_5_stage_movement_and_capture_pass`, `test_peak_osc_coordinator_end_to_end_udp_loopback`, plus 15 tests from Epochs 1..4).
- **`synth-core` (`cargo test -p synth-core`)**: 29 passed, 0 failed.
- **`synth-midi-osc` (`cargo test -p synth-midi-osc`)**: 25 passed, 0 failed.
- **`synth-ui-theme` (`cargo test -p synth-ui-theme`)**: 25 passed, 0 failed.
- **Node.js `konduktiva-peak` & `shared-synth-sdk` (`npm test`)**: 16 checks passed (`15` in `js/konduktiva-peak/test/run.js` + `1` in `test/baseline.test.js`).

# QA

- **Zero Compiler Warnings**: Verified across all 4 Rust crates (`synth-core`, `synth-midi-osc`, `synth-ui-theme`, `novation-peak-manager`) under `CARGO_BUILD_JOBS=2 cargo test --workspace`.
- **Hardware MIDI Safety**: All UDP OSC loopback tests and the 5-stage Konduktiva dry stress test (`run_dry_stress_test`) execute strictly in `MidiSafetyMode::VirtualLoopback` (`live: false`). Zero packets were transmitted to `Peak MIDI 1` (`hw:3,0,0`) or `UMC1820 MIDI 1` (`hw:0,0,0`).

# Lesson

Routing incoming OSC parameter messages (`/synth/peak/param/<name>`) through both `synth_core::CommandHistory::push_and_apply` and `PeakHardwareEngine::schedule_cc` ensures that algorithmic live-coding parameter changes from Node.js participate in the same undo/redo history and `SafeSweepBounds` resonance clamping (`CC 79 <= 62`) as manual GUI edits.
