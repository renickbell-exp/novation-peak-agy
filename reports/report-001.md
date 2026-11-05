---
title: "Epoch 1 of 6: Shared synth-core 5-Synthesizer Profile Extension, Peak 74-CC Parameter Map, Safe Sweep Bounds & SysEx Codec"
date: "2026-11-06 02:30:00"
project: "novation-peak-agy"
project_dir: "/home/renick/antigravity/novation-peak-agy"
report_number: 1
subject: "Epoch 1 of 6: Shared synth-core 5-Synth Profile Extension, 74-CC Map, Safe Sweep Bounds & SysEx Codec"
operator: "Antigravity Agent"
tldr: "Planned the 6-epoch development roadmap in `PLAN.md` and implemented the foundational Novation Peak parameter, profile, and SysEx architecture across `crates/synth-core` and `novation-peak-manager`. Added `SafeSweepBounds` matching `js/konduktiva-peak`, extended `synth-core` to all 5 synthesizer projects, created the 3-bank Korg nanoKONTROL2 surface mapping, verified 100% parity with all 74 MIDI CCs in `midi-cc-map.json`, and built the 527-byte Peak SysEx patch codec. 83 Rust unit tests and 16 Node.js test assertions pass with zero compiler warnings."
qa_verdict: "PASS"
tests_total: 99
---

# Prompt

We want to build a complete Rust/egui native desktop synthesizer manager, patch editor, librarian, Korg nanoKONTROL2 controller, and Konduktiva live-coding companion for the Novation Peak 8-voice polyphonic hybrid synthesizer (similar to our Bass Station II and Nord Drum 2 apps), integrating the existing js/konduktiva-peak library (from ~/claude/novation-peak/) and extending the shared crates (crates/synth-core, crates/synth-midi-osc, crates/synth-ui-theme, js/shared-synth-sdk) across all 5 synthesizers. Plan the development yourself, including choosing the optimal number of epochs N, write your development roadmap to PLAN.md, and execute all N epochs sequentially from Epoch 1 of N through Epoch N of N. For every epoch k (1..N), ensure all Rust and Node.js tests pass (CARGO_BUILD_JOBS=2 cargo test --workspace and npm test), generate and compile a standard HTML/PDF report in reports/report-00k.md using /home/renick/claude/html-report-generator/build-report.sh --pdf, and create a git commit for that epoch. Keep all physical MIDI hardware output (Peak MIDI 1 hw:3,0,0 and UMC1820 MIDI 1 hw:0,0,0) strictly unarmed/silent during automated tests.

# Content

## 1. Plain-English Overview & Six-Epoch Development Roadmap

A hardware music synthesizer like the **Novation Peak** has dozens of physical knobs, sliders, and buttons that shape its sound. When a computer program wants to talk to a synthesizer, it sends numbered digital control messages called **MIDI Continuous Controllers (MIDI CCs)**, where each parameter (such as Filter Cutoff or Reverb Level) is assigned a controller number from 0 to 127 and a value from 0 to 127. For saving and loading entire sounds at once, the synthesizer uses **System Exclusive (SysEx)** messages, which bundle a sound's name, knob positions, and modulation connections into a single binary packet.

To build the complete desktop manager, patch librarian, Korg nanoKONTROL2 controller mapper, and Konduktiva live-coding companion cleanly, we structured the work into `N = 6` sequential milestones (called **epochs**) in `PLAN.md`. Every epoch is verified by automated Rust and Node.js tests while keeping all physical MIDI hardware (`Peak MIDI 1` at `hw:3,0,0` and `UMC1820 MIDI 1` at `hw:0,0,0`) strictly silent and unarmed.

### Table: Six-Epoch Architecture Roadmap (`PLAN.md`)

| Epoch | Scope & Goal | Key Files & Modules | Primary Deliverables |
| --- | --- | --- | --- |
| **Epoch 1** | Shared `synth-core` 5-Synth Profile Extension, Peak 74-CC Map, Safe Sweep Bounds & SysEx Codec | `crates/synth-core`, `src/params.rs`, `src/sysex.rs` | `DeviceSynthesizerProfile::novation_peak()`, `SafeSweepBounds`, 3-bank nanoKONTROL2 mapping, 527-byte SysEx codec [DONE] |
| **Epoch 2** | Hardware Fingerprinting, Peak USB/ALSA Routing & Anti-Choke Scheduler | `crates/synth-midi-osc`, `src/midi.rs` | `SupportedDeviceType::NovationPeak` (`1235:007e`), `PeakMidiRouter`, `VirtualLoopback` safety guard, `CaptureStats` [PENDING] |
| **Epoch 3** | 16-Slot Modulation Matrix, Factory Preset Library, A/B Diffing, Safe-Sweep Randomizer & Undo/Redo | `src/mod_matrix.rs`, `src/librarian.rs` | 17 Mod Sources, 37 Mod Destinations, 16 factory presets across Banks A..D, direct reuse of `synth_core::CommandHistory` [PENDING] |
| **Epoch 4** | Visual Theme (`PeakOxfordBlue`), Custom Peak Vector Widgets, nanoKONTROL2 3-Bank Mapper & `egui` GUI | `crates/synth-ui-theme`, `src/controller.rs`, `src/gui.rs` | `SynthAccent::PeakOxfordBlue`, 4 Peak vector widgets, 6-tab native `egui` workstation with headless frame verification [PENDING] |
| **Epoch 5** | Bidirectional UDP OSC Bridge, Konduktiva Live-Coding Companion & `js/shared-synth-sdk` Integration | `src/konduktiva.rs`, `src/osc_bridge.rs`, `js/shared-synth-sdk`, `js/konduktiva-peak` | 8 Konduktiva player lanes (`bass`, `chords`, `lead`, `arp`, `ccCutoff`, `ccReso`, `ccMod`, `ccFx`), `peakHold`/`peakBuild` graphs, UDP OSC bridge [PENDING] |
| **Epoch 6** | Full CLI Workstation Entrypoint (`src/main.rs`), End-to-End Cross-Language Verification & Ecosystem Audit | `src/main.rs`, `tests/e2e_workspace.rs`, `test/peak-integration.test.js` | `--verify-hardware`, `--dry-stress`, `--export-schemas`, `--headless-frames`, full 5-synth ecosystem parity verification [PENDING] |

## 2. Safe Sweep Bounds & 5-Synthesizer Profile Extension in `crates/synth-core`

### Phase: Acoustic Protection with `SafeSweepBounds` (`crates/synth-core/src/param.rs`)

The Novation Peak combines high-resolution digital oscillators with genuine analog filter, overdrive, and distortion circuits. If an automated live-coding script or patch randomizer sweeps **Filter Resonance** (`CC 79`), **Filter Overdrive** (`CC 80`), **Filter Post-Drive** (`CC 36`), or **Analog Distortion Level** (`CC 104`) all the way to the maximum value of `127`, the analog circuitry can self-oscillate or jump sharply in volume.

In `js/konduktiva-peak/src/cc.js`, the function `safeSweepRange(name)` protects the listener's ears and studio monitors by capping resonance at `62`, distortion at `48`, drive/overdrive at `50`, feedback at `70`, and reverb/delay/chorus effects sends at `90`. In Epoch 1, we implemented `SafeSweepBounds` and `safe_sweep_range(name: &str)` in `crates/synth-core/src/param.rs` and exposed `ParamDescriptor::safe_sweep_bounds(&self)` so both Rust and JavaScript enforce identical safety limits.

### Table: Safe Sweep Bounds Mapping (`synth_core::safe_sweep_range` vs `konduktiva-peak`)

| Parameter Category / Keyword | Peak MIDI CC Numbers | Hardware Range | Safe Sweep Bounds (`lo..=hi`) | Rust Function / Method |
| --- | --- | --- | --- | --- |
| **Filter Resonance** (`resonance`, `reso`) | `CC 79` (`filter_resonance`) | `0..=127` | `0..=62` | `safe_sweep_range("Filter resonance")` [VERIFIED] |
| **Analog Distortion** (`distortion`) | `CC 104` (`distortion_level`) | `0..=127` | `0..=48` | `safe_sweep_range("Distortion level")` [VERIFIED] |
| **Filter Drive** (`overdrive`, `drive`) | `CC 36` (`filter_post_drive`), `CC 80` (`filter_overdrive`) | `0..=127` | `0..=50` | `safe_sweep_range("Filter overdrive")` [VERIFIED] |
| **FX Feedback** (`feedback`) | `CC 107` (`chorus_feedback`), `CC 110` (`delay_feedback`) | `0..=127` | `0..=70` | `safe_sweep_range("Delay feedback")` [VERIFIED] |
| **FX Send Levels** (`reverb`, `delay`, `chorus`, `fx`) | `CC 105`, `CC 108`, `CC 112` | `0..=127` | `0..=90` | `safe_sweep_range("Reverb level")` [VERIFIED] |
| **Standard Continuous Parameters** | e.g., `CC 29` (`filter_cutoff_frequency`), `CC 1` (`mod_wheel_modulation`) | `0..=127` | `0..=127` | `safe_sweep_range("Filter cutoff")` [VERIFIED] |

### Phase: Five-Synthesizer Profile & Three-Bank nanoKONTROL2 Mapping (`crates/synth-core/src/profiles.rs`)

We extended `crates/synth-core/src/profiles.rs` with `DeviceSynthesizerProfile::novation_peak()` and `ControllerSurfaceMapping::nanokontrol2_novation_peak()`. This unifies all five synthesizer families in our shared workspace (`asm_hydrasynth`, `korg_nanokontrol2`, `novation_bass_station_2` + `behringer_td3_md`, `clavia_nord_drum_2`, and `novation_peak`) and updates `export_all_schemas(dir)` to export `novation-peak.profile.json` and `korg-nanokontrol2-peak.mapping.json` alongside the existing 10 schema files.

- **Bank 1 (`Novation Peak Oscillators & Mixer`)**: Maps the 8 rotary knobs (`CC 16..23`) to Osc 1/2/3 Coarse Pitch, Wave Shape, and Virtual Sync (`VSync`), and maps the 8 linear sliders (`CC 0..7`) to Osc 1/2/3 Mix, Ring Mod Mix, Noise Mix, Glide Time, Mod Wheel, and Arp Clock Gate (16 bindings).
- **Bank 2 (`Novation Peak Filter, Envelopes & LFOs`)**: Maps the 8 rotary knobs (`CC 16..23`) to Filter Cutoff, Filter Resonance, Filter Overdrive, Filter Post-Drive, LFO 1 Rate, LFO 1 to Filter Depth, LFO 2 Rate, and Mod Env 1 to Filter Depth, and maps the 8 linear sliders (`CC 0..7`) to the four Amp Envelope ADSR stages and four Mod Envelope 1 ADSR stages (16 bindings).
- **Bank 3 (`Novation Peak FX, Animate & Konduktiva Live`)**: Maps the 8 rotary knobs (`CC 16..23`) to Distortion Level, Chorus Level/Rate/Feedback, Delay Level/Time/Feedback, and Reverb Level, maps the 8 linear sliders (`CC 0..7`) to the 8 Konduktiva live-coding player lanes (`bass`, `chords`, `lead`, `arp`, `ccCutoff`, `ccReso`, `ccMod`, `ccFx`), and binds Solo buttons 1 and 2 (`CC 32`, `CC 33`) to `Animate 1 Hold` (`CC 114`) and `Animate 2 Hold` (`CC 115`) with LED feedback (18 bindings).

## 3. Peak 74-CC Parameter Registry (`src/params.rs`) & SysEx Codec (`src/sysex.rs`)

### Phase: Compile-Time `midi-cc-map.json` Parity (`src/params.rs`)

To ensure that the Rust workstation and the Node.js `js/konduktiva-peak` package never disagree on a single parameter name or MIDI CC number, `src/params.rs` embeds `js/konduktiva-peak/midi-cc-map.json` at compile time via `include_str!("../js/konduktiva-peak/midi-cc-map.json")`.

- `PeakMidiCcMapSpec::load_embedded()` parses the `_meta` block (`device = "Novation Peak"`, `usb_id = "1235:007e"`, and the 9 `recommended_for_stress_test` CCs) together with all 74 entries in `cc_map`.
- `verify_profile_parity_with_cc_map()` iterates through all 74 entries in `midi-cc-map.json` and verifies that `DeviceSynthesizerProfile::novation_peak()` contains the exact matching `ParamId`, `ParamAddress::cc7(cc)`, and `[min, max]` value range.
- `dual_cc_pair(cc: u8)` resolves the six +32 dual-CC pairs documented in the Peak implementation notes (`23/55`, `24/56`, `25/57`, `29/61`, `30/62`, and `31/63`).

### Phase: 527-Byte Single Patch Dump SysEx Codec & Bank Selection (`src/sysex.rs`)

In `src/sysex.rs`, we implemented `encode_patch_to_sysex` and `decode_patch_from_sysex` for the 527-byte Novation Peak Single Patch Dump frame (`NOVATION_MANUFACTURER_ID = [0x00, 0x20, 0x29]`, `PEAK_DEVICE_ID = [0x01, 0x77]`), as well as `encode_bank_select_messages` (`CC 0` MSB `0`, `CC 32` LSB `1..=4` for `PeakPatchBank::BankA` through `PeakPatchBank::BankD`) and `encode_program_change_message` (`0..=127`).

```
+-------------------------------------------------------------------------+
| Novation Peak Single Patch Dump SysEx Frame (527 Bytes Total)           |
+------------------------+------------------------+-----------------------+
| Bytes 0..8: Header     | Bytes 16..31: Name     | Bytes 32..105: CCs    |
| F0 00 20 29 01 77      | 16-Byte ASCII String   | All 74 CC Parameters  |
| Cmd (01=Edit, 02=Bank) | Space-padded (0x20..7E)| Ordered by CC Map     |
| Bank (1..4), Slot(0..127)|                      |                       |
+------------------------+------------------------+-----------------------+
| Bytes 128..191: Mod Matrix (16 Slots x 4 Bytes) | Bytes 525..526: Tail  |
| Each Slot: [src1, src2, depth+64, destination]  | Byte 525: 7-Bit Cksum |
| Supports Bipolar Depth (-64..+63 -> 0..127)     | Byte 526: 0xF7 (EOX)  |
+-------------------------------------------------+-----------------------+
```

# Files Touched

- `PLAN.md` -- Created the 6-epoch architectural roadmap for `novation-peak-agy`
- `crates/synth-core/src/param.rs` -- Added `SafeSweepBounds`, `safe_sweep_range(name)`, and `ParamDescriptor::safe_sweep_bounds(&self)`
- `crates/synth-core/src/profiles.rs` -- Added `DeviceSynthesizerProfile::novation_peak()`, `ControllerSurfaceMapping::nanokontrol2_novation_peak()`, updated `export_all_schemas(dir)` to 12 files across all 5 synthesizers, and added unit tests
- `crates/synth-core/src/lib.rs` -- Re-exported `SafeSweepBounds` and `safe_sweep_range` and added 5-synthesizer and safe-sweep parity tests
- `src/params.rs` -- Created compile-time embedded `midi-cc-map.json` parser, `resolve_cc`, `build_cc_set`, `dual_cc_pair`, and `verify_profile_parity_with_cc_map`
- `src/sysex.rs` -- Created 527-byte Novation Peak SysEx patch encoder/decoder and Bank Select (`CC 0`/`CC 32`) + Program Change (`0..127`) encoder across Banks A..D
- `src/lib.rs` -- Wired `params` and `sysex` modules and exposed `synth_id()` and `app_title()`

# Files Not Touched

- `js/konduktiva-peak/midi-cc-map.json` -- Preserved existing 74-CC research table verbatim and embedded via `include_str!`
- `js/konduktiva-peak/src/cc.js` -- Preserved existing Node.js CC resolver and `safeSweepRange` implementation

# Tests

- `29` tests passing in `crates/synth-core` (all 5 synthesizer profiles, 12 schema exports, `SafeSweepBounds`, `CommandHistory`, `PatchDiff`, `LockShieldedRandomizer`)
- `23` tests passing in `crates/synth-midi-osc` (ALSA virtual loopback discovery, anti-choke scheduler, UDP OSC loopback)
- `24` tests passing in `crates/synth-ui-theme` (15 `yaae` themes, 4 `two_bowls` backgrounds, vector widget geometry)
- `7` tests passing in `novation-peak-manager` (`src/params.rs`, `src/sysex.rs`, `src/lib.rs`)
- `16` Node.js test assertions passing across `js/konduktiva-peak/test/run.js` and `test/baseline.test.js`

# QA

- `83 Rust Tests PASS`
- `16 Node.js Assertions PASS`
- `0 Compiler Warnings`
- `Live MIDI Output Unarmed (VirtualLoopback / Capture Stub)`

All automated tests executed with `CARGO_BUILD_JOBS=2 cargo test --workspace` and `npm test`. Zero MIDI packets were sent to physical ALSA ports `hw:3,0,0` (`Peak MIDI 1`) or `hw:0,0,0` (`UMC1820 MIDI 1`).

# Lesson

Embedding `js/konduktiva-peak/midi-cc-map.json` directly into the Rust crate with `include_str!` and running an automated parity check (`verify_profile_parity_with_cc_map()`) against `synth_core::DeviceSynthesizerProfile::novation_peak()` guarantees that our Rust GUI/librarian and our JavaScript Konduktiva live-coding kit share the exact same 74 MIDI CC numbers, normalized parameter IDs, and acoustic safe-sweep limits.
