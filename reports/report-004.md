---
title: "Epoch 4 of 6: Visual Theme (PeakOxfordBlue), Custom Peak Vector Widgets, nanoKONTROL2 3-Bank Mapper & Native egui GUI"
date: "2026-11-06 02:44:00"
project: "novation-peak-agy"
project_dir: "/home/renick/antigravity/novation-peak-agy"
report_number: 4
subject: "Epoch 4 of 6: PeakOxfordBlue Theme, 4 Vector Widgets, 3-Bank nanoKONTROL2 Soft-Takeover Controller & 6-Tab egui Workstation"
operator: "Antigravity Agent"
follows: ["report-001", "report-002", "report-003"]
tldr: "Extended `crates/synth-ui-theme` with `SynthAccent::PeakOxfordBlue` (`#1A73E8` primary, `#FF8C00` warm amber OLED readout), 4 custom headless-testable Novation Peak vector widgets (`WavetableOscillatorModel`, `StateVariableFilterModel`, `AnimateHoldButtonModel`, `VoiceAllocationMeterModel`), and 5-synthesizer theme schema exports. Created `src/controller.rs` (`NanoKontrol2PeakController` with 3 banks and soft takeover) and `src/gui.rs` (`PeakWorkstationApp` with 6 interactive tabs verified across 12 headless `egui` frames). 94 Rust tests and 16 Node.js assertions pass with 0 compiler warnings."
qa_verdict: "PASS"
tests_total: 110
---

# Prompt

We want to build a complete Rust/egui native desktop synthesizer manager, patch editor, librarian, Korg nanoKONTROL2 controller, and Konduktiva live-coding companion for the Novation Peak 8-voice polyphonic hybrid synthesizer (similar to our Bass Station II and Nord Drum 2 apps), integrating the existing js/konduktiva-peak library (from ~/claude/novation-peak/) and extending the shared crates (crates/synth-core, crates/synth-midi-osc, crates/synth-ui-theme, js/shared-synth-sdk) across all 5 synthesizers. Plan the development yourself, including choosing the optimal number of epochs N, write your development roadmap to PLAN.md, and execute all N epochs sequentially from Epoch 1 of N through Epoch N of N. For every epoch k (1..N), ensure all Rust and Node.js tests pass (CARGO_BUILD_JOBS=2 cargo test --workspace and npm test), generate and compile a standard HTML/PDF report in reports/report-00k.md using /home/renick/claude/html-report-generator/build-report.sh --pdf, and create a git commit for that epoch. Keep all physical MIDI hardware output (Peak MIDI 1 hw:3,0,0 and UMC1820 MIDI 1 hw:0,0,0) strictly unarmed/silent during automated tests.

# Content

## 1. Plain-English Overview of Epoch 4

A physical synthesizer like the **Novation Peak** has a distinctive visual look: an anodized Oxford Blue aluminum chassis paired with a crisp, warm amber **OLED (Organic Light-Emitting Diode)** screen and illuminated performance buttons. Moreover, when a musician uses a compact 8-knob / 8-slider MIDI controller like the **Korg nanoKONTROL2** to control a 74-parameter synthesizer, the controller needs **switchable banks** (so the same 8 knobs can control Oscillators in Bank 1, Filter and Envelopes in Bank 2, and Effects in Bank 3) and **Soft Takeover** (so turning a knob after switching banks does not cause the sound to jump abruptly until the physical knob crosses the saved parameter value).

In **Epoch 4 of 6**, we built the complete visual theme, custom vector widgets, physical controller manager, and 6-tab desktop graphical user interface (`egui`):

1. **Novation Peak Visual Theme (`crates/synth-ui-theme/src/lib.rs`)**: Added `SynthAccent::PeakOxfordBlue` (`#1A73E8` primary accent with `#FF8C00` warm OLED amber numeric readouts) to our shared theme crate, bringing `SynthAccent::all()` to 8 accents across all 5 synthesizer projects, and updated `export_all_theme_schemas(dir)` to export `novation-peak.theme.json`.
2. **Four Custom Peak Vector Widgets (`crates/synth-ui-theme/src/widgets.rs`)**: Implemented headless-testable vector models and `egui` painters for the Peak's New Oxford Wavetable Oscillators (`WavetableOscillatorModel`), Analog OTA State-Variable Filter (`StateVariableFilterModel`), Animate 1 & 2 Performance Hold buttons (`AnimateHoldButtonModel`), and 8-Voice Polyphonic Allocation Meter (`VoiceAllocationMeterModel`).
3. **Three-Bank nanoKONTROL2 Controller (`src/controller.rs`) & Six-Tab Desktop GUI (`src/gui.rs`)**: Built `NanoKontrol2PeakController` with bank switching (`CC 58` / `CC 59`) and per-control `SoftTakeoverState` catch logic, and assembled `PeakWorkstationApp` with 6 workstation tabs verified via headless `egui::Context` frame rendering.

## 2. `PeakOxfordBlue` Theme & Four Custom Vector Widgets (`crates/synth-ui-theme`)

### Phase: Eight Shared Synthesizer Accents & Four Peak Vector Widgets

Every vector widget in `crates/synth-ui-theme/src/widgets.rs` separates its mathematical geometry and state (`*Model`) from its `egui::Painter` rendering function. This allows automated tests (`cargo test`) to verify polyline coordinates, safe-sweep badges, and button latches without needing an open window or X11 display server.

### Table: Novation Peak Custom Vector Widgets in `crates/synth-ui-theme/src/widgets.rs`

| Widget Model & Function | Hardware Section Modeled | Key State Fields | Headless Geometry & Behavior Verified |
| --- | --- | --- | --- |
| `WavetableOscillatorModel` / `wavetable_oscillator_widget` | New Oxford FPGA Oscillators 1, 2, 3 | `osc_index` (`1..=3`), `shape` (`0.0..=1.0`), `vsync` (`0.0..=1.0`), `coarse_st` | `waveform_polyline(rect, 32)` generates 33-point sine/saw/pulse/vsync morphing curve [VERIFIED] |
| `StateVariableFilterModel` / `state_variable_filter_widget` | Analog OTA Multimode Filter | `cutoff`, `resonance`, `overdrive`, `post_drive` (`0.0..=1.0`) | `frequency_response_polyline(rect, 32)` & `is_within_safe_sweep()` (`Reso<=62`, `Drive<=50`) [VERIFIED] |
| `AnimateHoldButtonModel` / `animate_hold_button_widget` | Animate 1 (`CC 114`) & Animate 2 (`CC 115`) Buttons | `animate_index` (`1..=2`), `cc_number` (`114`/`115`), `held: bool` | `toggle()` & `midi_cc_value()` (`127` when held, `0` when released) [VERIFIED] |
| `VoiceAllocationMeterModel` / `voice_allocation_meter_widget` | 8-Voice Polyphonic Voice Allocator | `voices: [Option<u8>; 8]` | `set_active_notes`, `active_count()`, and `voice_slot_rects(rect, 4.0)` across 8 voice LEDs [VERIFIED] |

## 3. 3-Bank Korg nanoKONTROL2 Mapper (`src/controller.rs`) & 6-Tab `egui` GUI (`src/gui.rs`)

### Phase: `NanoKontrol2PeakController` Soft-Takeover Arbitration (`src/controller.rs`)

When the user presses **Track Right** (`CC 59`) or **Track Left** (`CC 58`) on the Korg nanoKONTROL2 (`0944:0117`, `hw:2,0,0`), `NanoKontrol2PeakController::switch_bank` switches between Bank 1 (`Oscillators & Mixer`), Bank 2 (`Filter, Envelopes & LFOs`), and Bank 3 (`FX, Animate & Konduktiva Live`) and calls `sync_bank_targets_from_patch(patch)` to reset each knob and slider's `SoftTakeoverState` target to the new bank's parameter value. Physical movements are ignored (`is_caught == false`) until the knob or fader crosses or enters the pickup threshold (`2.0` CC units) of the target value.

### Table: Six Workstation Tabs in `PeakWorkstationApp` (`src/gui.rs`)

| Tab Enum (`PeakTab`) | UI Title | Rendered Controls & Interactive Capabilities |
| --- | --- | --- |
| `OscillatorsAndMixer` | `1. Oscillators & Mixer` | Three side-by-side `wavetable_oscillator_widget` scopes + Range, Coarse, Fine, Shape, VSync, LFO/Env mod depths, and Mixer levels (`CC 23..27`, `CC 5`, `CC 35`) [VERIFIED] |
| `FilterEnvelopesLfos` | `2. Filter, Envs & LFOs` | Interactive `state_variable_filter_widget` curve with safe-sweep badge + Cutoff (`CC 29`), Resonance (`CC 79`), Overdrive (`CC 80`), Post-Drive (`CC 36`), LFO 1/2, and Amp/Mod Envelopes 1 & 2 [VERIFIED] |
| `FxAndModMatrix` | `3. FX & 16-Slot Mod Matrix` | Analog Distortion (`CC 104`), Chorus, Delay, Reverb sliders + full 16-row Modulation Matrix grid (`Source 1`, `Source 2`, `Depth -64..+63`, `Destination`) backed by `CommandHistory` [VERIFIED] |
| `LibrarianAndAbDiff` | `4. Patch Librarian & A/B Diff` | Searchable 16-patch Factory Librarian (Banks A..D), Safe-Sweep Randomizer slider/button, 527-byte SysEx Edit Buffer sender, and A/B Patch Diff with selective section merge [VERIFIED] |
| `NanoKontrol2Surface` | `5. nanoKONTROL2 3-Bank` | Interactive Bank 1/2/3 selector and live table of all 16-18 hardware CC bindings showing real-time `CAUGHT` vs `PENDING` soft-takeover status [VERIFIED] |
| `KonduktivaCompanion` | `6. Konduktiva Live Companion` | Density graph selector (`peakBuild` vs `peakHold`), BPM slider, and per-lane rate sliders for all 8 Konduktiva players (`bass`, `chords`, `lead`, `arp`, `ccCutoff`, `ccReso`, `ccMod`, `ccFx`) [VERIFIED] |

# Files Touched

- `crates/synth-ui-theme/src/lib.rs` -- Added `SynthAccent::PeakOxfordBlue` (`#1A73E8` / `#FF8C00`), `PEAK_OXFORD_BLUE_HEX`, `PEAK_AMBER_OLED_HEX`, and exported `novation-peak.theme.json` in `export_all_theme_schemas`
- `crates/synth-ui-theme/src/widgets.rs` -- Added `WavetableOscillatorModel`, `StateVariableFilterModel`, `AnimateHoldButtonModel`, `VoiceAllocationMeterModel`, their `egui` widget functions, and unit tests
- `src/controller.rs` -- Implemented `NanoKontrol2PeakController` with 3-bank switching (`CC 58`/`CC 59`), transport controls (`CC 41`/`CC 42`), Animate 1/2 toggles (`CC 32`/`CC 33`), and `SoftTakeoverState` catch arbitration
- `src/gui.rs` -- Implemented `PeakWorkstationApp` and `PeakTab` with 6 tabs and `run_headless_frames`
- `src/lib.rs` -- Registered `pub mod controller;` and `pub mod gui;`

# Files Not Touched

- `crates/synth-core/src/param.rs` -- Reused `SoftTakeoverState` and `SoftTakeoverMode` directly inside `src/controller.rs`

# Tests

- `29` tests passing in `crates/synth-core`
- `25` tests passing in `crates/synth-midi-osc`
- `25` tests passing in `crates/synth-ui-theme` (including `peak_vector_widgets_geometry_and_state` and 8-accent `all_seven_synth_accent_profiles`)
- `15` tests passing in `novation-peak-manager` (including `test_nanokontrol2_three_banks_and_soft_takeover_catch` and `test_peak_workstation_headless_gui_renders_all_6_tabs`)
- `16` Node.js test assertions passing across `js/konduktiva-peak/test/run.js` and `test/baseline.test.js`

# QA

- `94 Rust Tests PASS`
- `16 Node.js Assertions PASS`
- `0 Compiler Warnings`
- `12 Headless egui Frames Rendered Across All 6 Tabs`

In `test_peak_workstation_headless_gui_renders_all_6_tabs`, we instantiated `PeakWorkstationApp::new()`, verified that `SynthAccent::PeakOxfordBlue` is active, executed 12 full headless `egui::Context::run` layout and paint passes cycling through all 6 tabs, and confirmed that editing `filter_cutoff_frequency` updates the patch, records an undoable command on `CommandHistory`, and flushes safely to the virtual loopback buffer.

# Lesson

Calling `SoftTakeoverState::reset_target(target_val)` whenever the user switches banks on the Korg nanoKONTROL2 ensures that a physical knob left at `100` in Bank 1 cannot accidentally slam a Bank 2 parameter whose saved value is `20` until the musician intentionally sweeps the physical knob across `20`.
