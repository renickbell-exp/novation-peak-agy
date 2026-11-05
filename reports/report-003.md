---
title: "Epoch 3 of 6: 16-Slot Modulation Matrix, Factory Preset Library, A/B Diffing, Safe-Sweep Randomizer & Undo/Redo"
date: "2026-11-06 02:39:00"
project: "novation-peak-agy"
project_dir: "/home/renick/antigravity/novation-peak-agy"
report_number: 3
subject: "Epoch 3 of 6: 16-Slot Dual-Source Modulation Matrix, 16 Factory Presets, Safe-Sweep Randomizer & CommandHistory"
operator: "Antigravity Agent"
follows: ["report-001", "report-002"]
tldr: "Implemented the Novation Peak 16-Slot Dual-Source Modulation Matrix (`src/mod_matrix.rs` with 17 `ModSource`s and 37 `ModDestination`s) and the Factory Preset Librarian (`src/librarian.rs` with 16 factory patches across Banks A..D, each pre-encoded with a 527-byte SysEx dump). Directly reused `synth_core::PresetLibrary`, `synth_core::SynthPatch`, `synth_core::CommandHistory`, `synth_core::PatchDiff`, and `synth_core::LockShieldedRandomizer` without duplicate wrapper structs, adding `randomize_patch_safe` to clamp randomized patches within `SafeSweepBounds`. 91 Rust tests and 16 Node.js assertions pass with 0 warnings."
qa_verdict: "PASS"
tests_total: 107
---

# Prompt

We want to build a complete Rust/egui native desktop synthesizer manager, patch editor, librarian, Korg nanoKONTROL2 controller, and Konduktiva live-coding companion for the Novation Peak 8-voice polyphonic hybrid synthesizer (similar to our Bass Station II and Nord Drum 2 apps), integrating the existing js/konduktiva-peak library (from ~/claude/novation-peak/) and extending the shared crates (crates/synth-core, crates/synth-midi-osc, crates/synth-ui-theme, js/shared-synth-sdk) across all 5 synthesizers. Plan the development yourself, including choosing the optimal number of epochs N, write your development roadmap to PLAN.md, and execute all N epochs sequentially from Epoch 1 of N through Epoch N of N. For every epoch k (1..N), ensure all Rust and Node.js tests pass (CARGO_BUILD_JOBS=2 cargo test --workspace and npm test), generate and compile a standard HTML/PDF report in reports/report-00k.md using /home/renick/claude/html-report-generator/build-report.sh --pdf, and create a git commit for that epoch. Keep all physical MIDI hardware output (Peak MIDI 1 hw:3,0,0 and UMC1820 MIDI 1 hw:0,0,0) strictly unarmed/silent during automated tests.

# Content

## 1. Plain-English Overview of Epoch 3

A synthesizer's knobs set static starting values for a sound, but what makes a sound feel alive is **modulation**: automatically moving one or more knobs over time using performance controls (such as the Modulation Wheel, keyboard velocity, or the Peak's two **Animate** buttons) or internal wave generators called **Low-Frequency Oscillators (LFOs)** and **Envelopes**.

On the **Novation Peak**, this routing happens inside a **16-Slot Modulation Matrix**. Each of the 16 slots lets the musician pick **two modulation sources** (`source_1` and `source_2`) that are multiplied together (so, for example, LFO 2 only vibrates the pitch when the Modulation Wheel is pushed up), a **destination** parameter (such as Filter Cutoff or Oscillator 1 Wave Shape), and a bipolar **depth** from `-64` to `+63`.

In **Epoch 3 of 6**, we built two major modules while strictly reusing our shared `synth_core` library types (`SynthPatch`, `PresetLibrary`, `CommandHistory`, `PatchDiff`, and `LockShieldedRandomizer`) with zero duplicate undo stacks or wrapper structs:

1. **16-Slot Dual-Source Modulation Matrix (`src/mod_matrix.rs`)**: Models all 17 Peak `ModSource`s, all 37 `ModDestination`s, and `ModSlot` (`1..=16`), reading and writing directly into `synth_core::SynthPatch` parameters (`mod_slot_{1..16}_{src1,src2,depth,dst}`) and recording multi-field slot edits atomically onto `synth_core::CommandHistory` via `BatchSetParamsCommand`.
2. **Factory Preset Library & Safe-Sweep Randomizer (`src/librarian.rs`)**: Populates a `synth_core::PresetLibrary` with 16 curated factory patches across Banks A, B, C, and D (each complete with modulation matrix routings and a 527-byte SysEx dump), and provides `randomize_patch_safe` so randomized sounds never exceed acoustic `SafeSweepBounds`.

## 2. 16-Slot Dual-Source Modulation Matrix (`src/mod_matrix.rs`)

### Phase: Dual-Source Multiplication & `CommandHistory` Integration

When only one modulation source is needed in a slot, the Peak sets `source_2` to `ModSource::Direct` (which acts as `1.0` unity gain). When two sources are selected (such as `ModSource::ModWheel` and `ModSource::Lfo2Bipolar`), the hardware multiplies them together before scaling by `depth` (`-64..=+63`) and adding the result to `destination`.

Instead of maintaining a separate modulation struct outside the patch, `read_slot_from_patch` and `write_slot_to_patch` store all 16 slots directly in `SynthPatch.parameters` under `mod_slot_{1..16}_{src1,src2,depth,dst}`. This means `PatchDiff::compare`, JSON serialization, `encode_patch_to_sysex`, and `CommandHistory` undo/redo work seamlessly across both continuous knobs and modulation matrix slots.

### Table: Modulation Matrix Architecture in `src/mod_matrix.rs`

| Component | Rust Symbol | Count / Range | Storage & Undo Mechanism |
| --- | --- | --- | --- |
| **Modulation Sources** | `ModSource` | `17` sources (`0..=16`, `Off`, `Direct`, `ModWheel`, `Aftertouch`, `Animate1/2`, `Lfo1/2`, `Env1..3`, etc.) | Stored as `mod_slot_{i}_src1` and `mod_slot_{i}_src2` in `SynthPatch` [VERIFIED] |
| **Modulation Destinations** | `ModDestination` | `37` destinations (`0..=36`, Pitch, VSync, Shape, Levels, Filter, Drive, LFOs, Envs, FX) | Stored as `mod_slot_{i}_dst` in `SynthPatch` [VERIFIED] |
| **Bipolar Modulation Depth** | `ModSlot::depth` | `-64..=+63` (`i8`, encoded as `0..=127` with `64` center in SysEx) | Stored as `mod_slot_{i}_depth` in `SynthPatch` [VERIFIED] |
| **Atomic Slot Undo / Redo** | `apply_slot_with_history` | `16` slots (`MOD_MATRIX_SLOT_COUNT = 16`) | Pushes `synth_core::BatchSetParamsCommand` to `synth_core::CommandHistory` [VERIFIED] |

## 3. 16 Curated Factory Patches & Safe-Sweep Randomizer (`src/librarian.rs`)

### Phase: Factory Library Across Banks A, B, C, and D (`build_peak_factory_library`)

We created 16 factory presets in `src/librarian.rs` spanning four banks (`Bank A` through `Bank D`, 4 patches per bank), covering the Peak's core synthesis strengths: New Oxford NCO wavetables, analog OTA multimode filter overdrive, virtual sync, ring modulation, lush ensemble chorus, and a dedicated `Konduktiva Stress Init` patch on Bank D Slot 3. Every factory patch is pre-encoded with a verified 527-byte SysEx frame (`patch.sysex.raw_dump`).

### Table: 16 Curated Novation Peak Factory Presets (`build_peak_factory_library`)

| Bank & Slot | Patch Name | Category | Key Synthesis Features & Mod Matrix Routing |
| --- | --- | --- | --- |
| **A0..A3** | `Oxford Warm Pad`, `State Variable Acid`, `Triple NCO Detune Lead`, `FPGA Wavetable Pluck` | Pad, Bass, Lead, Pluck | Wavetable shape LFO modulation, high-resonance OTA acid filter with `Velocity -> Cutoff`, 3-oscillator detune stack [VERIFIED] |
| **B0..B3** | `Ring Mod Metallic Bell`, `Subterranean Drone`, `Animate Morph Brass`, `Lush Chorus Strings` | Bell, Drone, Brass, Strings | `Osc 1 x Osc 2` ring mod bell, sub-octave stereo pan drone, `Animate 1 -> Cutoff` & `Animate 2 -> VSync` brass [VERIFIED] |
| **C0..C3** | `Cybernetic Arp`, `Analog Overdrive Bass`, `Crystalline Keys`, `Drift Poly Synth` | Arp, Bass, Keys, Poly | Synced arpeggiator gate, dual pre/post filter drive bass, FM electric piano keys, aftertouch poly synth [VERIFIED] |
| **D0..D3** | `Sync Lead 24MHz`, `Dub Delay Chord`, `Dark Velvet Pad`, `Konduktiva Stress Init` | Lead, Chord, Pad, LiveCoding | 24 MHz NCO virtual sync lead, dub techno filtered delay chord, and the baseline live-coding stress test patch [VERIFIED] |

### Phase: Lock-Shielded Safe-Sweep Randomizer (`randomize_patch_safe`)

When generating new patch ideas with `synth_core::LockShieldedRandomizer`, an unconstrained randomizer could set `filter_resonance` (`CC 79`) or `distortion_level` (`CC 104`) to `127`. Our function `randomize_patch_safe` delegates randomization to `LockShieldedRandomizer::randomize_global` (respecting all locked parameters and sections) and then clamps every unlocked CC parameter to its `SafeSweepBounds` (`Resonance <= 62`, `Distortion <= 48`, `Drive <= 50`, `Feedback <= 70`, `FX <= 90`). Applying randomization in the UI calls `apply_safe_randomization_with_history`, which pushes a `synth_core::ReplacePatchCommand` directly onto `synth_core::CommandHistory`.

# Files Touched

- `src/mod_matrix.rs` -- Implemented 16-slot modulation matrix (`ModSource`, `ModDestination`, `ModSlot`, `read_slot_from_patch`, `write_slot_to_patch`, `apply_slot_with_history`)
- `src/librarian.rs` -- Implemented `build_peak_factory_library` (16 factory presets across Banks A..D with 527-byte SysEx dumps), `build_peak_randomizer`, `randomize_patch_safe`, and `apply_safe_randomization_with_history`
- `src/lib.rs` -- Registered `pub mod librarian;` and `pub mod mod_matrix;`

# Files Not Touched

- `crates/synth-core/src/history.rs` -- Reused `CommandHistory`, `SetParamCommand`, `BatchSetParamsCommand`, and `ReplacePatchCommand` directly without modification
- `crates/synth-core/src/preset.rs` -- Reused `SynthPatch` and `PresetLibrary` directly without wrapper structs

# Tests

- `29` tests passing in `crates/synth-core`
- `25` tests passing in `crates/synth-midi-osc`
- `24` tests passing in `crates/synth-ui-theme`
- `13` tests passing in `novation-peak-manager` (including `test_mod_matrix_slot_read_write_and_command_history_undo_redo`, `test_factory_library_has_16_patches_across_4_banks_with_sysex`, `test_safe_sweep_randomizer_clamps_resonance_distortion_and_drive_with_undo`, and `test_ab_patch_diff_and_section_merge_between_factory_patches`)
- `16` Node.js test assertions passing across `js/konduktiva-peak/test/run.js` and `test/baseline.test.js`

# QA

- `91 Rust Tests PASS`
- `16 Node.js Assertions PASS`
- `0 Compiler Warnings`
- `10-Step Safe Randomization + 10-Step Undo Verified Identical via PatchDiff`

In `test_safe_sweep_randomizer_clamps_resonance_distortion_and_drive_with_undo`, we ran 10 consecutive 100%-intensity randomizations with `filter_cutoff_frequency` locked, verified that `filter_cutoff_frequency` never moved while `filter_resonance <= 62`, `distortion_level <= 48`, `filter_overdrive <= 50`, `delay_feedback <= 70`, and `reverb_level <= 90` held across all 10 seeds, and then executed 10 `CommandHistory::undo` steps to confirm `PatchDiff::compare` reported `identical == true` with the original factory preset.

# Lesson

Storing the 16 modulation matrix slots directly as normalized parameters (`mod_slot_{1..16}_{src1,src2,depth,dst}`) inside `synth_core::SynthPatch` avoids creating any custom wrapper around `SynthPatch` and allows `PatchDiff`, `CommandHistory`, JSON preset files, and the 527-byte SysEx codec to handle modulation routings automatically.
