# Novation Peak Manager & Konduktiva Companion: Development Plan (`PLAN.md`)

## Project Objective

Build a complete Rust and `egui` native desktop synthesizer manager, 74-CC and NRPN patch editor, 16-slot modulation matrix editor, preset librarian, Korg nanoKONTROL2 3-bank controller surface mapper, and Konduktiva live-coding companion for the **Novation Peak** 8-voice polyphonic hybrid synthesizer (`1235:007e`, `Peak MIDI 1` at `hw:3,0,0`). Simultaneously extend the shared workspace crates (`crates/synth-core`, `crates/synth-midi-osc`, `crates/synth-ui-theme`) and Node.js packages (`js/shared-synth-sdk`, `js/konduktiva-peak`) across all 5 synthesizers and controllers in the ecosystem:

1. **ASM Hydrasynth** (`asm_hydrasynth`)
2. **Korg nanoKONTROL2 / nkEditor** (`korg_nanokontrol2`)
3. **Novation Bass Station II & Behringer TD-3-MD** (`novation_bass_station_2`, `behringer_td3_md`)
4. **Clavia Nord Drum 2** (`clavia_nord_drum_2`)
5. **Novation Peak** (`novation_peak`)

## Hardware Safety & Build Invariants

- **Strict Hardware Silence During Automated Runs**: All unit tests, integration tests, stress tests, and CLI verification commands (`--verify-hardware`, `--dry-stress`) default to `MidiSafetyMode::VirtualLoopback` (`live: false`, `captureStats: true`). Physical ALSA MIDI outputs (`Peak MIDI 1` at `hw:3,0,0` and `UMC1820 MIDI 1` at `hw:0,0,0`) remain strictly unarmed and silent during all automated execution.
- **CPU & Compiler Discipline**: All Rust builds and test runs use `CARGO_BUILD_JOBS=2` (`-j 2`) with the external target directory configured in `.cargo/config.toml` and maintain zero compiler warnings across the workspace.
- **Epoch Reporting & Git Commits**: At the completion of each Epoch `k` (`1..6`), run `CARGO_BUILD_JOBS=2 cargo test --workspace` and `npm test`, author `reports/report-00k.md` in clear plain language paired with exact code symbols and metrics, compile `reports/report-00k.html` and `reports/report-00k.pdf` via `/home/renick/claude/html-report-generator/build-report.sh --pdf`, and create a clean git commit for Epoch `k`.

---

## Development Roadmap (`N = 6` Epochs)

| Epoch | Title | Key Deliverables | Target Report |
| :--- | :--- | :--- | :--- |
| **Epoch 1 of 6** | Shared `synth-core` Extension, Complete 74-CC Novation Peak Profile, Safe Sweep Bounds & Patch SysEx Codec | Extend `crates/synth-core` with `DeviceSynthesizerProfile::novation_peak()`, `SafeSweepBounds`, `ControllerSurfaceMapping::nanokontrol2_novation_peak()`, and 5-synth schema export; implement `src/params.rs` and `src/sysex.rs` | `reports/report-001.html` / `.pdf` |
| **Epoch 2 of 6** | Hardware Fingerprinting, Peak USB/ALSA Routing & Anti-Choke Scheduler (`synth-midi-osc` & `src/midi.rs`) | Extend `crates/synth-midi-osc` with `SupportedDeviceType::NovationPeak` (`1235:007e`), `discover_peak_hardware()`, and `PeakMidiRouter` with Note-On priority, CC deduplication, safe sweep clamping, and +32 dual-CC pairs; implement `src/midi.rs` | `reports/report-002.html` / `.pdf` |
| **Epoch 3 of 6** | 16-Slot Modulation Matrix, Factory Preset Library, A/B Diffing, Safe-Sweep Randomizer & Undo/Redo | Implement `src/mod_matrix.rs` (16 mod matrix slots + 16 direct assignments) and `src/librarian.rs` (16 curated factory patches across Banks A..D, `CommandHistory`, `PatchDiff`, and `LockShieldedRandomizer` with acoustic safety clamps) | `reports/report-003.html` / `.pdf` |
| **Epoch 4 of 6** | Visual Theme (`PeakOxfordBlue`), Custom Peak Vector Widgets, nanoKONTROL2 3-Bank Mapper & Native `egui` GUI | Extend `crates/synth-ui-theme` with `SynthAccent::PeakOxfordBlue` and 4 headless-testable Peak vector widgets (`WavetableOscillatorModel`, `StateVariableFilterModel`, `AnimateHoldButtonModel`, `VoiceAllocationMeterModel`); implement `src/controller.rs` and `src/gui.rs` | `reports/report-004.html` / `.pdf` |
| **Epoch 5 of 6** | Bidirectional UDP OSC Bridge, Konduktiva Live-Coding Companion & `js/shared-synth-sdk` + `js/konduktiva-peak` Integration | Implement `src/konduktiva.rs` and `src/osc_bridge.rs`; extend `js/shared-synth-sdk` (`novation-peak-router.js`, `SharedSynthClient`, 5-synth schemas) and `js/konduktiva-peak` (`src/osc-bridge.js`) | `reports/report-005.html` / `.pdf` |
| **Epoch 6 of 6** | Full CLI Workstation Entrypoint (`src/main.rs`), End-to-End Cross-Language Verification & Ecosystem Audit | Implement full CLI flags (`--verify-hardware`, `--dry-stress`, `--export-schemas`, `--list-patches`, `--headless-frames`), cross-process Rust + Node.js E2E test suites, and complete 6-epoch verification | `reports/report-006.html` / `.pdf` |

---

## Detailed Epoch Breakdown

### Epoch 1 of 6: Shared `synth-core` Extension, Complete 74-CC Novation Peak Profile, Safe Sweep Bounds & Patch SysEx Codec
- **Goal**: Establish the data model for the Novation Peak inside `crates/synth-core` alongside the other 4 synthesizers, and build the parameter and SysEx codecs in the main crate.
- **Tasks**:
  1. Add `DeviceSynthesizerProfile::novation_peak()` in `crates/synth-core/src/profiles.rs` containing all 74 CC parameters from `js/konduktiva-peak/midi-cc-map.json` organized across modules (`System`, `Voice`, `Osc 1`, `Osc 2`, `Osc 3`, `Mixer`, `Filter`, `LFO 1`, `LFO 2`, `Amp Env`, `Mod Env 1`, `Mod Env 2`, `FX Distortion`, `FX Chorus`, `FX Delay`, `FX Reverb`, `Animate`, `Arp`).
  2. Add `SafeSweepBounds` (`safe_sweep_range(name)`) in `crates/synth-core/src/param.rs` matching `js/konduktiva-peak/src/cc.js` (`resonance: 0..62`, `distortion: 0..48`, `overdrive/drive: 0..50`, `feedback: 0..70`, `reverb/delay/chorus/fx: 0..90`, default `0..127`).
  3. Add `ControllerSurfaceMapping::nanokontrol2_novation_peak()` in `crates/synth-core/src/profiles.rs` and update `export_all_schemas()` to export `novation-peak.profile.json` and `korg-nanokontrol2-peak.mapping.json`.
  4. Implement `src/params.rs` (canonical CC lookup, `resolve_cc()`, `build_cc_set()`, +32 bipolar dual-CC pair resolver, and compile-time parity check with `js/konduktiva-peak/midi-cc-map.json`) and `src/sysex.rs` (Novation Peak SysEx frame encoder/decoder `0x00 0x20 0x29`, Bank Select + Program Change packet builder, and `SynthPatch` round-trip).

### Epoch 2 of 6: Hardware Fingerprinting, Peak USB/ALSA Routing & Anti-Choke Scheduler (`synth-midi-osc` & `src/midi.rs`)
- **Goal**: Extend `crates/synth-midi-osc` to identify and safely route MIDI traffic for the Novation Peak (`1235:007e`, `hw:3,0,0`) with anti-choke rate thinning and virtual loopback safety.
- **Tasks**:
  1. Add `SupportedDeviceType::NovationPeak` to `crates/synth-midi-osc/src/hardware.rs` matching USB ID `1235:007e` and card/client names containing `"peak"`.
  2. Add `PeakDiscoveryResult`, `discover_peak_hardware()`, and `PeakMidiRouter` to `crates/synth-midi-osc/src/routing.rs` supporting 8-voice polyphonic voice allocation, safe sweep clamping, +32 dual-CC high-resolution output, Animate 1/2 hold messages, and Anti-Choke CC thinning with Note-On priority.
  3. Update `export_midi_osc_schemas()` in `crates/synth-midi-osc/src/routing.rs` to include `novation-peak.hardware.json` and `peak-midi-router.schema.json`.
  4. Implement `src/midi.rs` (`PeakHardwareEngine`) with virtual loopback capture statistics (`CaptureStats`) compatible with `js/konduktiva-peak`.

### Epoch 3 of 6: 16-Slot Modulation Matrix, Factory Preset Library, A/B Diffing, Safe-Sweep Randomizer & Undo/Redo
- **Goal**: Build the Peak's 16-slot modulation matrix, curated 16-patch factory bank, A/B patch diff/merge workflow, safe-sweep lock-shielded randomizer, and command history manager.
- **Tasks**:
  1. Implement `src/mod_matrix.rs` with `ModSource`, `ModDestination`, `ModSlot` (Slots 1..16 with `source_a`, `source_b`, `depth: -64..+63`, `destination`), and `DirectModAssignment` (1..16), stored directly inside `synth_core::SynthPatch` parameters.
  2. Implement `src/librarian.rs` wrapping `synth_core::PresetLibrary`, `synth_core::CommandHistory`, `synth_core::PatchDiff`, and `synth_core::LockShieldedRandomizer` (with `SafeSweepBounds` clamping so randomized patches never self-oscillate or clip).
  3. Populate 16 factory presets spanning Banks A, B, C, and D across 8 musical categories.

### Epoch 4 of 6: Visual Theme (`PeakOxfordBlue`), Custom Peak Vector Widgets, nanoKONTROL2 3-Bank Mapper & Native `egui` GUI
- **Goal**: Extend `crates/synth-ui-theme` with the Novation Peak color palette and 4 vector widgets, build the 3-bank Korg nanoKONTROL2 controller mapper with soft-takeover ghost rings, and assemble the complete `egui` desktop interface.
- **Tasks**:
  1. Add `SynthAccent::PeakOxfordBlue` (`#1A73E8` primary, `#FF8C00` OLED amber readout) to `crates/synth-ui-theme/src/lib.rs` and update `export_all_theme_schemas()`.
  2. Add `WavetableOscillatorModel`, `StateVariableFilterModel`, `AnimateHoldButtonModel`, and `VoiceAllocationMeterModel` to `crates/synth-ui-theme/src/widgets.rs`.
  3. Implement `src/controller.rs` (`NanoKontrol2PeakMapper`) with 3 banks (Oscillators & Mixer; Filter, Envelopes & LFOs; FX, Animate & Konduktiva Live), `SoftTakeoverState` tracking, and LED feedback.
  4. Implement `src/gui.rs` (`PeakManagerApp`) rendering all 6 workstation tabs and top bar controls in headless-verifiable `egui`.

### Epoch 5 of 6: Bidirectional UDP OSC Bridge, Konduktiva Live-Coding Companion & `js/shared-synth-sdk` + `js/konduktiva-peak` Integration
- **Goal**: Connect the Rust application and `js/konduktiva-peak` + `js/shared-synth-sdk` via bidirectional UDP Open Sound Control (OSC) and shared Konduktiva rhythm ladder / density graph state.
- **Tasks**:
  1. Implement `src/konduktiva.rs` modeling the 8 Peak players (`bass`, `chords`, `lead`, `arp`, `ccCutoff`, `ccReso`, `ccMod`, `ccFx`), `peakGrid` ladder (`minIOI = 0.125`), `peakHold` and `peakBuild` density graphs, triangle CC sweeps, and the 120-second movement timeline.
  2. Implement `src/osc_bridge.rs` handling `/synth/peak/*` and `/synth/konduktiva/*` OSC commands and telemetry broadcasts.
  3. Extend `js/shared-synth-sdk` with `novation-peak-router.js`, `NOVATION_PEAK_PROFILE`, `NOVATION_PEAK_THEME`, and Peak methods on `SharedSynthClient`.
  4. Extend `js/konduktiva-peak` with `src/osc-bridge.js` so `createPeakKit(K)` can mirror or route player note/CC events and density graph switches over UDP OSC.

### Epoch 6 of 6: Full CLI Workstation Entrypoint (`src/main.rs`), End-to-End Cross-Language Verification & Ecosystem Audit
- **Goal**: Complete the CLI entrypoint (`src/main.rs`), run end-to-end cross-process tests between Node.js and Rust, verify 5-synth schema exports, and deliver the final project verification report.
- **Tasks**:
  1. Implement `src/main.rs` supporting `--verify-hardware`, `--dry-stress`, `--export-schemas`, `--list-patches`, `--inspect-patch`, and `--headless-frames`.
  2. Add comprehensive Node.js and Rust E2E tests (`test/peak-integration.test.js` and `tests/e2e_workspace.rs`).
  3. Verify all Rust and Node.js tests pass with zero warnings and compile `reports/report-006.html` and `reports/report-006.pdf`.
