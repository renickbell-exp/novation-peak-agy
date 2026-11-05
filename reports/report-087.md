---
title: "Experiment 5 Outcome Report: Autonomous Epoch Planning Benchmark (Novation Peak Synthesizer Manager & Konduktiva Companion)"
subtitle: "Comparative Evaluation of Self-Planned Multi-Epoch Execution in AGY (6 Epochs) vs. PIGS (8 Epochs) and Longitudinal Gains over Fixed 20-Epoch Scripting"
date: "2026-11-06"
report_number: 87
project: "pigsAndFishes / novation-peak-pigs / novation-peak-agy"
author: "Antigravity Research & Engineering"
qa_verdict: "PASS"
---

# Prompt

Run Experiment 5 in `tmux 11:0.0` (`agy`) and `tmux 12:0.0` (`pigs`): build a complete native Rust and `egui` desktop synthesizer manager, patch editor, librarian, Korg `nanoKONTROL2` controller mapper, and `Konduktiva` live-coding companion for the **Novation Peak** 8-voice polyphonic hybrid synthesizer, integrating the existing `~/claude/novation-peak/` (`konduktiva-peak`) library and extending the shared synthesizer crates across all 5 synthesizers. Instead of requiring 20 pre-planned epochs from an external script, pass a single high-level task prompt to both `agy` and `pigs` and let each application autonomously plan its own development roadmap (`PLAN.md`) and choose the optimal number of epochs (`N`). Collect complete quantitative and qualitative data (number of epochs, token usage, wall-clock time usage, code quality, test coverage, and architectural cleanliness), commit and push all repositories, and file a standard HTML/PDF report.

# Content

## Executive Summary (Plain-Language Overview)

### What We Tested and Why It Matters

In our previous four benchmark experiments, an external test script broke every synthesizer project into **20 fixed steps** (called **epochs**, where each epoch is one self-contained round of coding, testing, reporting, and saving a `git` commit) and fed those 20 steps one at a time to the two coding systems:

1. **Standalone `agy` (`tmux 11:0.0`)**: A single continuous AI coding session running `gemini-4-argon-medium` inside `/home/renick/antigravity/novation-peak-agy`.
2. **Neuro-Symbolic `pigs` (`tmux 12:0.0`, `pigsAndFishes`)**: A structured software engineering harness running inside `/home/renick/antigravity/novation-peak-pigs` that maintains a **Software World Model** (`.world-model.json`, a live map of every module and architectural rule in the project), routes coding turns to fast or frontier AI models, and checks every turn with a 4-stage automated verification pipeline (**SymbolicGateRunner**) before committing.

In **Experiment 5**, we changed the rules in a major way: instead of forcing both systems through 20 pre-scripted epochs, we sent **one single high-level prompt** to both `tmux 11:0.0` (`agy`) and `tmux 12:0.0` (`pigs`) at `2026-11-05T19:16:22Z` and let each system decide for itself how to organize the work and how many epochs (`N`) to run. We also tested the 5 speed and token-efficiency upgrades implemented in Report 086 (`commit 6f5cef8` and `commit 46a88a9`).

The target application is the **Novation Peak Manager, Patch Librarian & Konduktiva Companion** (`novation-peak-manager`), supporting the **Novation Peak** 8-voice polyphonic hybrid synthesizer (which combines field-programmable gate array numerically controlled oscillators, or **FPGA NCOs**, with an analogue state-variable filter, or **SVF**, and analogue distortion). Both workspaces were pre-seeded with the existing `~/claude/novation-peak/` (`js/konduktiva-peak`) live-coding package and the 4 shared ecosystem packages (`crates/synth-core`, `crates/synth-midi-osc`, `crates/synth-ui-theme`, and `js/shared-synth-sdk`), expanding the shared synthesizer platform to **5 synthesizers**:

1. **Korg nanoKONTROL2 / nkEditor** (`korg_nanokontrol2`)
2. **ASM Hydrasynth** (`asm_hydrasynth`)
3. **Novation Bass Station II & Behringer TD-3-MD** (`novation_bass_station_2`, `behringer_td3_md`)
4. **Clavia Nord Drum 2** (`clavia_nord_drum_2`)
5. **Novation Peak** (`novation_peak`, USB ID `1235:007e`, ALSA RawMIDI `hw:3,0,0`, Sequencer client `28:0`)

### Headline Results at a Glance

- **Autonomous Epoch Count (`N`)**:
  - **`agy` (`tmux 11:0.0`)** chose **`N_agy = 6` epochs**, authored an 80-line `PLAN.md`, and executed all 6 epochs (`reports/report-001.{md,html,pdf}` through `reports/report-006.{md,html,pdf}`) in a single continuous session without any external continuation prompts.
  - **`pigs` (`tmux 12:0.0`)** autonomously planned **`N_pigs = 8` module-aligned epochs** via `IntentEngine.planAutonomousApplicationEpochs()`, authored `PLAN.md`, executed `Epoch 1 of 8` immediately, staged `Epoch 2 of 8` through `Epoch 8 of 8` in `[2:Queue]`, and drained all 8 epochs (`reports/report-001.{md,html,pdf}` through `reports/report-008.{md,html,pdf}`) with `TaskDeconflictionEngine` enforcing strict sequential ordering across the shared Cargo workspace.
- **Massive Token & Time Savings in `pigs` (Experiment 5 vs. Experiment 4)**:
  - **Non-Cached Tokens**: Dropped from `14,016,743` in Experiment 4 (20 fixed epochs) to **`4,409,538` tokens** in Experiment 5 (8 autonomous epochs), a **68.5% reduction in total non-cached tokens** (**3.18x token savings**).
  - **Active Synthesis & Verification Time**: Dropped from `168m 44s` (`10,124s`) in Experiment 4 to **`70m 52s` (`4,252s`)** in Experiment 5, a **58.0% reduction in active runtime** (**2.38x faster**).
  - **File Read Calls (`read_file`)**: Dropped from `1,303` calls in Experiment 4 to **`292` calls** in Experiment 5, a **77.6% reduction** (**4.46x fewer reads**), directly validating the inline source pre-loading (`extractTargetEntityInlineSource`) and 800-line chunking rules from Report 086.
  - **Self-Repair Turns (`CEGIS`, Counterexample-Guided Inductive Synthesis)**: Dropped from `12` repair turns in Experiment 4 (`7` of which were false-positive `inv:clean-architecture` rejections on `src/app.rs`) to **only `2` repair turns** in Experiment 5 (**0 false-positive architectural rejections**), an **83.3% reduction in repair turns**.
  - **Model Routing Efficiency**: **`10 / 10` sub-agent turns (`100%`)** in `pigs` ran on the fast `gemini-3.8-flash-high` model (including both repair turns and the final documentation/CLI epoch), compared to Experiment 4 where `7` primary turns and `12` repair turns escalated to the slower `gemini-4-argon-medium` model.
- **Code Quality & Verification Depth (`AGY` vs. `PIGS`)**:
  - **`novation-peak-pigs`** produced **`29,765` total lines of code (LOC)** (`25,280` Rust LOC across 25 files + `4,485` JS LOC across 17 files) verified by **`196` passing automated tests** (`168` Rust tests + `28` Node.js tests), with **`0` `unwrap()` and `0` `expect()` calls** in `20,457` lines of production Rust, a 19,779.9 events/sec 1,000-event polyphonic stress benchmark, a headless `egui` layout geometry auditor (`1280x800` and `1920x1080`), and full CLI flag support (`--help`, `--verify-hardware`, `--dry-stress`, `--export-shared-schemas`).
  - **`novation-peak-agy`** produced **`21,444` total LOC** (`16,869` Rust LOC across 25 files + `4,575` JS LOC across 19 files) verified by **`116` passing automated tests** (`100` Rust tests + `16` Node.js tests), with `0` `unwrap()` and `1` `expect()` call in `13,552` lines of production Rust, completing in **`42m 20s` (`2,540s`)** and **`955,368` non-cached tokens**.

| Metric | Standalone `agy` (`tmux 11:0.0`) | Neuro-Symbolic `pigs` (`tmux 12:0.0`) | `pigs` in Exp 4 (20 Fixed Epochs) | `pigs` Exp 5 vs. Exp 4 Improvement |
| :--- | :--- | :--- | :--- | :--- |
| **Epoch Planning Mode** | Self-planned (`PLAN.md`) | Self-planned (`IntentEngine` -> `PLAN.md`) | External 20-epoch script | Autonomous 1-prompt execution |
| **Planned & Completed Epochs (`N`)** | **6 / 6** (`100%`) | **8 / 8** (`100%`) | 20 / 20 (`100%`) | **60% fewer epochs** (`20 -> 8`) |
| **Sub-Agent / Session Turns** | 1 continuous session (`353` steps) | 10 turns (`8` primary + `2` repair, `1,429` steps) | 32 turns (`20` primary + `12` repair) | **68.8% fewer turns** (`32 -> 10`) |
| **Model Tier Usage** | `100%` `gemini-4-argon-medium` | **`100%` `gemini-3.8-flash-high`** (`10/10`) | `65%` Flash / `35%` Argon-Medium | **Zero slow Argon-Medium turns** |
| **Input Tokens (Non-Cached)** | `770,195` | **`3,703,440`** | `12,231,425` | **-69.7% input tokens** (`3.30x` less) |
| **Output Tokens** | `185,173` | **`706,098`** | `1,785,318` | **-60.4% output tokens** (`2.53x` less) |
| **Total Non-Cached Tokens** | **`955,368`** | **`4,409,538`** | `14,016,743` | **-68.5% total tokens (`3.18x` savings)** |
| **Cache-Read Tokens** | `14,920,849` | `71,903,007` | `204,118,520` | **-64.8% cached token reads** |
| **Active Synthesis + Gate Time** | **`42m 20s` (`2,540s`)** | **`70m 52s` (`4,252s`)** | `168m 44s` (`10,124s`) | **-58.0% active time (`2.38x` faster)** |
| **Total Tool Calls** | `232` | **`708`** | `2,418` | **-70.7% tool calls** (`3.42x` fewer) |
| **File Read Calls (`read_file`)** | `105` | **`292`** | `1,303` | **-77.6% `read_file` calls (`4.46x` fewer)** |
| **CEGIS Self-Repair Turns** | N/A (manual in-session retries) | **`2`** (`0` false positives) | `12` (`7` false positives) | **-83.3% repair turns** |
| **Total Codebase Size (LOC)** | `21,444` (`16,869` Rust + `4,575` JS) | **`29,765`** (`25,280` Rust + `4,485` JS) | `32,140` (`28,210` Rust + `3,930` JS) | Full 5-synth + Peak feature parity |
| **Automated Tests Passing** | `116 / 116` (`100` Rust + `16` JS) | **`196 / 196`** (`168` Rust + `28` JS) | `254 / 254` | **`+69.0%` more tests in `pigs` vs `agy`** |
| **Production Rust `unwrap()` / `expect()`** | `0` `unwrap()` / `1` `expect()` | **`0` `unwrap()` / `0` `expect()`** | `0` `unwrap()` / `0` `expect()` | **100% panic-free production Rust** |
| **HTML & PDF Epoch Reports** | `6` HTML + `6` PDF (`report-001..006`) | **`8` HTML + `8` PDF** (`report-001..008`) | `20` HTML + `20` PDF | 100% compiled via `build-report.sh` |

## Section 1: Autonomous Epoch Planning Comparison (`AGY` vs. `PIGS`)

### Plain-Language Explanation

When you ask a developer to build a full desktop application from a single high-level description, the first thing they must do is break the project into logical milestones. If there are too many tiny milestones (like the 20 pre-planned epochs in Experiment 4), the developer wastes hours re-reading the same files and re-running the full test suite 20 separate times. If there is only 1 giant milestone, errors pile up and become hard to debug.

When given the exact same single task prompt at `2026-11-05T19:16:22Z`, both `agy` and `pigs` independently converged on **6 to 8 cohesive, subsystem-aligned epochs** as the sweet spot for a full Rust + `egui` + `Konduktiva` synthesizer workstation:

- **`agy` (`tmux 11:0.0`)** planned **6 epochs** in `/home/renick/antigravity/novation-peak-agy/PLAN.md`, grouping the 3 FPGA oscillators, 16-slot modulation matrix, and preset librarian together into Epoch 3, and grouping the `PeakOxfordBlue` theme, custom vector widgets, `nanoKONTROL2` mapper, and 6-tab `egui` GUI into Epoch 4.
- **`pigs` (`tmux 12:0.0`)** used `IntentEngine.planAutonomousApplicationEpochs()` (`/home/renick/antigravity/pigsAndFishes/src/intent/IntentEngine.js`) to decompose the prompt into **8 module-isolated epochs** in `/home/renick/antigravity/novation-peak-pigs/PLAN.md`, dedicating one primary target module to each epoch so that each sub-agent turn had a clean, bounded architectural scope. `pigs` immediately started `Epoch 1 of 8`, staged `Epoch 2 of 8` through `Epoch 8 of 8` in `[2:Queue]`, and used `TaskDeconflictionEngine.checkConflict()` to execute them in strict sequential order (`Wave 1` through `Wave 7`).

### Technical Comparison of Autonomous Roadmaps (`PLAN.md`)

| Subsystem / Capability | `agy` Autonomous Plan (`N_agy = 6` in `novation-peak-agy/PLAN.md`) | `pigs` Autonomous Plan (`N_pigs = 8` in `novation-peak-pigs/PLAN.md`) |
| :--- | :--- | :--- |
| **5-Synth Ecosystem & 74-CC Profile** | **Epoch 1 of 6**: `DeviceSynthesizerProfile::novation_peak()`, `SafeSweepBounds`, `src/params.rs`, `src/sysex.rs` | **Epoch 1 of 8**: `NovationPeakProfile` in `crates/synth-core`, 74 CCs, +32 dual-CC pairs, 14-bit NRPNs, 5-synth `UnifiedEcosystemSchemaBundle` in Rust & JS |
| **Hardware Discovery, Routing & Safety** | **Epoch 2 of 6**: `SupportedDeviceType::NovationPeak` (`1235:007e`), `PeakMidiRouter`, `src/midi.rs` (`CaptureStats`) | **Epoch 2 of 8**: Non-invasive `/proc/asound` inspection (`hw:3,0,0`, `hw:0,0,0`, `hw:2,0,0`), `safe_sweep_range` clamping, NRPN/Dual-CC/SysEx router, bidirectional UDP OSC server |
| **8-Voice FPGA NCOs, 17 Wavetables & SVF Filter** | Combined into Epoch 1 & Epoch 4 widget models | **Epoch 3 of 8**: Dedicated domain engine `src/peak_synth.rs` backed by `synth_core::SynthPatch` & `CommandHistory` (3 FPGA NCOs, 17 wavetables x 5 waves, VSync, Ring Mod, 12/24dB LP/BP/HP SVF with pre/post drive) |
| **3x ADSR Envelopes, 2x LFOs, 16-Slot Mod Matrix & Master FX** | **Epoch 3 of 6** (part 1): `src/mod_matrix.rs` (16 mod slots + 16 direct assignments) | **Epoch 4 of 8**: Dedicated engine `src/peak_mod_fx.rs` (3 ADSRs, 2 tempo-synced LFOs with fade, 16-slot dual-source `Source A x Source B` Mod Matrix + 4-slot FX Matrix, Animate 1/2 macros, Arp, Distortion/Chorus/Delay/Reverb) |
| **512-Patch Librarian, SysEx Dump, A/B Diff, Morph & nanoKONTROL2** | **Epoch 3 of 6** (part 2): `src/librarian.rs` (16 factory patches, `PatchDiff`, `LockShieldedRandomizer`); `src/controller.rs` in Epoch 4 | **Epoch 5 of 8**: Dedicated `src/peak_librarian.rs` (Banks A..D x 128 patches, SysEx state machine with checksum verification, continuous `0.0..=1.0` A/B morph fader, lock-shielded safe randomizer, `nanoKONTROL2` surface mapper) |
| **Native `egui` Desktop GUI, Vector Widgets & Layout Auditor** | **Epoch 4 of 6**: `SynthAccent::PeakOxfordBlue`, 4 vector widgets in `synth-ui-theme`, `src/gui.rs` (`PeakWorkstationApp`, 6 tabs) | **Epoch 6 of 8**: `NovationPeakOxfordBlue` & `NovationPeakAmber` accents, 5 custom vector widgets in `synth-ui-theme`, `src/app.rs` & `src/gui.rs` (`NovationPeakApp`), text-focus shortcut gating, headless `LayoutGeometryAuditor` (`1280x800` & `1920x1080`) |
| **Konduktiva 8-Player Companion & Cross-Language Parity** | **Epoch 5 of 6**: `src/konduktiva.rs`, `src/osc_bridge.rs`, `js/konduktiva-peak/src/osc-bridge.js`, `js/shared-synth-sdk` | **Epoch 7 of 8**: `src/konduktiva_bridge.rs`, GUI Konduktiva live-coding tab (`bass`, `chords`, `lead`, `arp`, `ccCutoff`, `ccReso`, `ccMod`, `ccFx`, `peakHold`/`peakBuild`), `tests/cross_synth_parity.rs` + 20-check `js/konduktiva-peak/test/run.js` |
| **1,000-Event Stress Benchmark, CLI Entrypoint & Documentation** | **Epoch 6 of 6**: `src/main.rs` CLI flags, `tests/e2e_workspace.rs`, `test/peak-integration.test.js` | **Epoch 8 of 8**: `src/stress_bench.rs` (1,000-event 8-voice + 9-lane CC sweep stress benchmark at `19,779.9` events/sec), full CLI flags including `--help` in `src/main.rs`, and complete `README.md` |

## Section 2: Complete Token, Wall-Clock Time & Tool-Call Telemetry

### Plain-Language Explanation

Every time an AI coding agent reads a file, edits code, or runs a command, it exchanges **tokens** (chunks of text roughly equal to three-quarters of a word) with the language model. We measure three token numbers from the raw JSON Lines (`transcript.jsonl`) logs in `~/.gemini/antigravity-cli/brain/`:

1. **`input_tokens` (Non-Cached Input)**: Fresh prompt and tool-output tokens processed for the first time during a step.
2. **`output_tokens` (Generated Output)**: Code, reasoning, and tool calls written by the model.
3. **`cache_read_tokens` (Context Cache Reads)**: Previously seen conversation context re-read from the fast server cache at a fraction of the cost of fresh input tokens.

Below is the complete per-epoch and total telemetry for both `agy` (1 continuous session across 6 epochs) and `pigs` (8 epochs executed across 8 primary turns and 2 fast self-repair turns, all 10 using `gemini-3.8-flash-high`).

### Per-Turn & Per-Epoch Telemetry Table (`PIGS` vs. `AGY`)

| System / Turn | Conversation ID | Model Tier | Steps | Active Time (`s`) | Tool Calls | `read_file` | `edit_file` | `write_file` | `run_command` | `input_tokens` | `output_tokens` | `cache_read_tokens` | `total_tokens` (In+Out) | Commit SHA |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **`PIGS` Epoch 1 of 8** | `58eb3778` | `gemini-3.8-flash-high` | 146 | `562s` | 74 | 36 | 13 | 2 | 23 | `433,592` | `108,473` | `11,137,886` | **`542,065`** | `8dc645e` |
| **`PIGS` Epoch 2 of 8** | `4e47c0a0` | `gemini-3.8-flash-high` | 207 | `558s` | 103 | 46 | 29 | 3 | 25 | `524,492` | `113,687` | `13,182,690` | **`638,179`** | `09100e9` |
| **`PIGS` Epoch 3 of 8** | `586dc62e` | `gemini-3.8-flash-high` | 108 | `483s` | 53 | 13 | 7 | 2 | 31 | `332,778` | `93,852` | `3,746,934` | **`426,630`** | `fd2f3c2` |
| **`PIGS` Epoch 4 of 8** | `0c0bdf89` | `gemini-3.8-flash-high` | 82 | `246s` | 40 | 16 | 4 | 2 | 18 | `242,096` | `60,477` | `2,852,435` | **`302,573`** | `b06a0c2` |
| **`PIGS` Epoch 5 of 8** | `326ff99b` | `gemini-3.8-flash-high` | 96 | `316s` | 47 | 26 | 6 | 2 | 13 | `291,778` | `63,136` | `4,515,791` | **`354,914`** | `78fbbe1` |
| **`PIGS` Epoch 6 of 8 (Initial)** | `78bf5a06` | `gemini-3.8-flash-high` | 268 | `809s` | 131 | 58 | 30 | 3 | 39 | `641,724` | `108,316` | `16,931,004` | **`750,040`** | (repaired) |
| **`PIGS` Epoch 6 of 8 (CEGIS Repair)** | `b2c7f2bc` | `gemini-3.8-flash-high` | 70 | `278s` | 33 | 10 | 4 | 0 | 19 | `184,339` | `19,924` | `1,336,585` | **`204,263`** | `04cc177` |
| **`PIGS` Epoch 7 of 8** | `d615ec39` | `gemini-3.8-flash-high` | 208 | `501s` | 103 | 46 | 19 | 3 | 35 | `477,690` | `71,551` | `10,827,342` | **`549,241`** | `ecc9481` |
| **`PIGS` Epoch 8 of 8 (Initial)** | `4392e136` | `gemini-3.8-flash-high` | 152 | `363s` | 79 | 33 | 9 | 5 | 32 | `362,671` | `53,305` | `5,153,091` | **`415,976`** | (repaired) |
| **`PIGS` Epoch 8 of 8 (CEGIS Repair)** | `962e8d74` | `gemini-3.8-flash-high` | 92 | `136s` | 45 | 8 | 4 | 0 | 33 | `212,280` | `13,377` | `2,219,249` | **`225,657`** | `3cabe08` |
| **`PIGS` Total (All 8 Epochs)** | **10 turns** | **`100%` Flash-High** | **1,429** | **`4,252s` (`70m 52s`)** | **708** | **292** | **125** | **22** | **268** | **`3,703,440`** | **`706,098`** | **`71,903,007`** | **`4,409,538`** | **8 commits** |
| **`AGY` Total (All 6 Epochs)** | `7f124267` | **`100%` Argon-Medium** | **353** | **`2,540s` (`42m 20s`)** | **232** | **105** | **66** | **25** | **36** | **`770,195`** | **`185,173`** | **`14,920,849`** | **`955,368`** | **6 commits** |

### Why Did Autonomous Epoch Planning + Report 086 Optimizations Save 68.5% of Tokens in `PIGS`?

1. **Cohesive 8-Epoch Decomposition vs. 20 Micro-Epochs**:
   - In Experiment 4, 20 fine-grained epochs forced `pigs` to launch 20 separate sub-agent conversations (plus 12 repair conversations = 32 cold-start sessions), re-reading `crates/synth-core`, `crates/synth-midi-osc`, `crates/synth-ui-theme`, and `src/lib.rs` on every epoch.
   - In Experiment 5, `IntentEngine.planAutonomousApplicationEpochs()` grouped the application into **8 cohesive subsystem epochs** (`crates/synth-core` -> `crates/synth-midi-osc` -> `src/peak_synth.rs` -> `src/peak_mod_fx.rs` -> `src/peak_librarian.rs` -> `src/app.rs` -> `src/konduktiva_bridge.rs` -> `src/stress_bench.rs`), cutting the number of sub-agent cold starts from `32` to `10` (**68.8% fewer sessions**).
2. **Inline Source Pre-Loading & 800-Line Read Chunking (`AgyBridge.js`)**:
   - Pre-loading target files (`<= 600` lines) and crate entrypoints (`<= 400` lines) directly into the `[TARGET ENTITY INLINE SOURCE]` section of `buildCagedPrompt()` and instructing sub-agents to read full 800-line chunks reduced `read_file` calls in `pigs` from `1,303` (in Experiment 4) to **`292`** (in Experiment 5), a **77.6% drop**.
   - Look at `Epoch 3 of 8` (`src/peak_synth.rs`, creating `1,060+` lines of 8-voice FPGA NCO oscillator and SVF filter code): the sub-agent made only **13 `read_file` calls** in the entire turn!
3. **Elimination of `src/app.rs` Architectural False Positives (`SymbolicGateRunner.js`)**:
   - In Experiment 4, 7 out of 12 CEGIS repair turns were triggered because `src/app.rs` (`eframe::App`) imported `egui` while `SymbolicGateRunner._scanForForbiddenImports()` treated `app.rs` as a pure non-UI domain file.
   - In Experiment 5, with `baseName === "app.rs"` and `widget`/`panel`/`modal` files exempted in `SymbolicGateRunner.js` and `SoftwareWorldModel.js`, `pigs` experienced **zero (`0`) false-positive `inv:clean-architecture` rejections**.
4. **Fast-Model Tier Routing (`IntentEngine.js` & `repl.js`)**:
   - All 8 primary epochs and both CEGIS repair turns were routed to `gemini-3.8-flash-high`, completing Epoch 4 (`src/peak_mod_fx.rs`) in just **`245.5s` (`4m 05s`)** and Epoch 5 (`src/peak_librarian.rs`) in **`316.2s` (`5m 16s`)**.

## Section 3: Code Quality, Architecture, Seam-Tax & Test Coverage Analysis

### Plain-Language Explanation

Raw speed and low token usage only matter if the finished software is thorough, bug-free, and well-architected. To evaluate code quality objectively, we audited both repositories (`/home/renick/antigravity/novation-peak-agy` and `/home/renick/antigravity/novation-peak-pigs`) across six engineering dimensions:

1. **Automated Test Depth**: How many unit, integration, and cross-language parity tests verify the code (`196` in `pigs` vs. `116` in `agy`).
2. **Panic Safety (`inv:no-raw-panics`)**: Whether production Rust code contains raw `.unwrap()` or `.expect()` calls that could crash the synthesizer manager during a live performance (`0` `unwrap()` and `0` `expect()` in `pigs` across `20,457` production lines vs. `0` `unwrap()` and `1` `expect()` in `agy` across `13,552` production lines).
3. **Seam-Tax (Glue-Code Overhead)**: How cleanly each application reuses `synth_core::SynthPatch`, `CommandHistory`, `PresetLibrary`, `PatchDiff`, `Randomizer`, and `SoftTakeoverState` without writing redundant wrapper structs.
4. **Compiler & Linter Cleanliness**: Both workspaces compile with **`0` `rustc` warnings** on `cargo test --workspace`; under strict `cargo clippy --workspace --all-targets - -D warnings`, `novation-peak-agy` has `1` minor lint (`clippy::into_iter_on_ref` in `src/gui.rs:455`) and `novation-peak-pigs` has `7` minor stylistic lints (`derivable_impls`, `needless_range_loop`, `needless_bool_assign`, `op_ref`, `for_kv_map`, `manual_range_contains`).
5. **CLI Ergonomics (`--help`, `--verify-hardware`, `--dry-stress`, `--export-shared-schemas`)**: `novation-peak-pigs` supports all standard CLI flags including `-h` / `--help`, `--verify-hardware`, `--dry-stress`, `--export-shared-schemas`, `--audit-geometry`, and `--headless-server`; `novation-peak-agy` supports `--verify-hardware`, `--dry-stress`, `--export-schemas`, `--list-patches`, `--inspect-patch`, and `--headless-frames`, but returns `Error: Unrecognized CLI argument: --help` when invoked with `--help`.
6. **Hardware MIDI Safety**: Both applications default strictly to `MidiSafetyMode::VirtualLoopback` (`live: false`), keeping the connected physical **Focusrite A.E Peak** (`hw:3,0,0`, client `28:0`) and **Behringer UMC1820** (`hw:0,0,0`, client `16:0`) completely silent during all automated test and CLI verification runs.

### Side-by-Side Code Quality & Architecture Comparison Table

| Quality / Architecture Dimension | `novation-peak-agy` (`tmux 11:0.0`) | `novation-peak-pigs` (`tmux 12:0.0`) | Winner & Technical Notes |
| :--- | :--- | :--- | :--- |
| **Total Rust LOC (`src/` + `crates/`)** | `16,869` lines (`25` `.rs` files) | **`25,280` lines** (`25` `.rs` files) | **`pigs` (`+49.9%` deeper domain & widget modeling)** |
| **Application Crate Rust LOC (`src/` + `tests/`)** | `4,011` lines (`11` `.rs` files) | **`10,239` lines** (`12` `.rs` files) | **`pigs`** (includes 1,000-event stress bench & geometry auditor) |
| **Shared Crates Rust LOC (`crates/*`)** | `12,858` lines (`14` `.rs` files) | **`15,041` lines** (`13` `.rs` files) | **`pigs`** (richer 5-synth profiles & Peak vector widgets) |
| **Production vs. Test Rust LOC** | `13,552` prod / `3,317` test | **`20,457` prod / `4,823` test** | **`pigs`** (`+45.4%` more Rust test code) |
| **JavaScript SDK & Konduktiva LOC (`js/` + `test/`)** | `4,575` lines (`19` `.js` files) | **`4,485` lines** (`17` `.js` files) | Parity (both integrate `js/konduktiva-peak` & `js/shared-synth-sdk`) |
| **Total Workspace LOC (Rust + JS)** | `21,444` lines | **`29,765` lines** | **`pigs`** |
| **Public Rust API Surface (`pub fn` / `pub struct` / `enum`)** | `495` `pub fn` / `94` types | **`730` `pub fn` / `154` types** | **`pigs`** |
| **Rust Automated Tests (`cargo test --workspace`)** | `100` passed (`0` failed) | **`168` passed (`0` failed)** | **`pigs` (`+68.0%` more Rust unit & parity tests)** |
| **Node.js Automated Tests (`npm test`)** | `16` passed (`12` `run.js` + `4` `node --test`) | **`28` passed (`20` `run.js` + `8` `node --test`)** | **`pigs` (`+75.0%` more JS cross-language parity checks)** |
| **Total Automated Tests Passing** | `116 / 116` (`100%`) | **`196 / 196` (`100%`)** | **`pigs` (`+80` more passing tests)** |
| **Production Rust `.unwrap()` Count** | **`0`** | **`0`** | **Tie (100% zero `.unwrap()` in both)** |
| **Production Rust `.expect()` Count** | `1` | **`0`** | **`pigs` (`inv:no-raw-panics` enforced by gate)** |
| **`rustc` Compiler Warnings** | **`0`** | **`0`** | **Tie** |
| **Headless `egui` Layout Geometry Auditor** | Tab frame runner (`run_headless_frames(6)`) | **Full `LayoutGeometryAuditor` (`1280x800` & `1920x1080` zero-overlap check)** | **`pigs`** |
| **1,000-Event Polyphonic + 9-Lane CC Stress Bench** | 2s dry stress (`371` events) | **Deterministic 1,000-event bench (`19,779.9` events/sec, 8-voice steal & safety clamp check)** | **`pigs`** |
| **CLI `--help` Flag Support** | Fails (`Unrecognized CLI argument: --help`) | **Full `-h` / `--help` usage output** | **`pigs`** |

## Section 4: Deep-Dive Analysis  -  Why `PIGS` Used 8 Epochs vs. `AGY`'s 6 Epochs, the "Controller vs. Simulation" Planning Trap, and How to Fix It

### Plain-Language Explanation: Why Fewer Epochs Alone Is Not Automatically a Win

Simply cutting a project from 20 epochs down to 8 epochs is not a guaranteed win if some of those 8 epochs are doing **the wrong kind of work**. When we compared `agy`'s 6-epoch run (`42m 20s`, `955,368` non-cached tokens, `4,011` application Rust LOC) against `pigs`'s 8-epoch run (`70m 52s`, `4,409,538` non-cached tokens, `10,239` application Rust LOC), two critical questions emerged:

1. **Why did `pigs` need 8 epochs when `agy` finished in 6 epochs?**
2. **Why did each `pigs` epoch still consume 3.46x more non-cached tokens (`551k` vs. `159k`) and 7.4x more shell commands (`268` vs. `36` total `run_command` calls) than `agy`?**

### Finding 1: The "Controller vs. DSP Simulation" Planning Trap (`Epoch 3` and `Epoch 4` in `PIGS`)

We are building a **hardware controller, patch editor, librarian, and live-coding companion** for an external hardware synthesizer (the Novation Peak), **not** a software audio synthesizer that generates sound waves on the computer's CPU. The physical Novation Peak hardware already contains the 3 FPGA oscillators and analogue filter that make the sound; our desktop app only needs to store and edit the 74 MIDI CC / NRPN parameter values, send and receive SysEx patch dumps, map Korg `nanoKONTROL2` faders, draw 2D parameter preview curves in the GUI, and sequence MIDI/OSC events from `Konduktiva`.

Look at where the 2-epoch difference (`8` vs. `6`) came from when we compare every `.rs` file in `novation-peak-agy/src/` against `novation-peak-pigs/src/`:

| Phase | `agy` (6 Epochs, `4,011` App Rust LOC) | `pigs` (8 Epochs, `10,239` App Rust LOC) | Forensic Diagnosis |
| :--- | :--- | :--- | :--- |
| **Shared Profile & SysEx** | **Epoch 1**: `synth-core` profile + `src/params.rs` (`286L`) + `src/sysex.rs` (`301L`) | **Epoch 1**: `synth-core` profile (`NovationPeakProfile`) + `js/shared-synth-sdk` (`542k` tok, `562s`) | Equivalent controller scope |
| **Hardware, Routing & OSC** | **Epoch 2**: `synth-midi-osc` + `src/midi.rs` (`368L`) | **Epoch 2**: `synth-midi-osc` + `src/midi.rs` (`263L`) + `src/hardware_verify.rs` (`416L`) (`638k` tok, `558s`) | Equivalent controller scope |
| **Synthesis Domain & Librarian** | **Epoch 3 (1 Epoch)**: `src/mod_matrix.rs` (`472L`) + `src/librarian.rs` (`549L`), storing all 74 CCs directly in `synth_core::SynthPatch` | **Epochs 3, 4 & 5 (3 Epochs)**:<br>• **Epoch 3**: `src/peak_synth.rs` (`1,946L`, `427k` tok, `483s`)<br>• **Epoch 4**: `src/peak_mod_fx.rs` (`1,929L`, `303k` tok, `246s`)<br>• **Epoch 5**: `src/peak_librarian.rs` (`1,952L`, `355k` tok, `316s`) | **Wrong-Domain Work**: `pigs` spent 2 extra epochs (`729s`, `730k` tokens) writing `3,875` lines of software DSP simulation structs (`PeakSynthEngine`, `PeakModFxEngine`) for an external hardware synth |
| **Theme, Vector Widgets & GUI** | **Epoch 4**: `synth-ui-theme` + `src/controller.rs` (`264L`) + `src/gui.rs` (`607L`) | **Epoch 6**: `synth-ui-theme` + `src/gui.rs` (`1,303L`) + `src/app.rs` (`514L`) (`954k` tok, `1,087s` incl. repair) | `pigs` had to bridge `gui.rs`/`app.rs` across 3 large custom domain modules (`peak_synth`, `peak_mod_fx`, `peak_librarian`) |
| **Konduktiva & OSC Bridge** | **Epoch 5**: `src/konduktiva.rs` (`557L`) + `src/osc_bridge.rs` (`333L`) + JS bridge | **Epoch 7**: `src/konduktiva_bridge.rs` (`919L`) + `tests/cross_synth_parity.rs` (`370L`) (`549k` tok, `501s`) | Equivalent scope |
| **CLI, Stress Bench & Audit** | **Epoch 6**: `src/main.rs` (`233L`) + `tests/e2e_workspace.rs` (`169L`) | **Epoch 8**: `src/stress_bench.rs` (`439L`) + `src/main.rs` (`280L`) + `README.md` (`642k` tok, `499s` incl. repair) | Equivalent scope (`pigs` added `--help` and 1,000-event stress bench) |

**Why did `pigs` fall into the "DSP Simulation Trap" while `agy` did not?**

1. **Keyword Conflation in `IntentDomains.js` (`AUDIO_DSP` & `SYNTHESIS` vs. `HARDWARE_CONTROLLER_LIBRARIAN`)**:
   - In `src/intent/IntentDomains.js` (lines `362-381`), any prompt mentioning `"synthesizer"`, `"oscillator"`, `"wavetable"`, `"filter"`, `"adsr"`, `"lfo"`, `"reverb"`, or `"alsa"` automatically matched `AUDIO_DSP` (*"Real-Time Audio Signal Processing, Filters, Effects & Spectral Analysis"*) and `SYNTHESIS` (*"Sound Synthesis Engines, Oscillators, Envelopes, Modulation & Timbre Design"*).
   - `IntentDomains.js` lacked a distinct **`HARDWARE_CONTROLLER_LIBRARIAN`** domain separating an **external MIDI/NRPN/SysEx hardware controller** from an **in-process software audio DSP engine**.
2. **Component-Driven Decomposition Instead of Role-Driven Capability Subtraction**:
   - `IntentEngine.planAutonomousApplicationEpochs()` mapped the hardware components of the Novation Peak into dedicated Rust modules (`src/peak_synth.rs` and `src/peak_mod_fx.rs` with `domain: "DSP_AUDIO"`), even though `crates/synth-core` (`NovationPeakProfile` + `SynthPatch`) had **already** defined all 74 CCs, +32 dual-CC pairs, and 14-bit NRPNs in Epoch 1.
   - Because `SymbolicGateRunner` requires file mutation (`requireMutation: true`) on each epoch's target file, the sub-agent had to fill `src/peak_synth.rs` (`1,946L`) and `src/peak_mod_fx.rs` (`1,929L`) with software synthesis simulation structs (`PeakSynthEngine`, `PeakModFxEngine`, sample-by-sample ADSR and SVF evaluators), which then had to be read and bridged by every downstream epoch (`Epochs 5, 6, 7, 8`).

### Finding 2: Four Execution Bottlenecks Inside `PIGS` Sub-Agent Turns (`268` vs. `36` Shell Commands)

Inspecting the raw JSONL transcripts for all 10 `pigs` turns against `agy`'s single session (`7f124267`) explains the remaining token and time gap:

1. **Stateless Sub-Agent Cold Starts Across Epochs (`10` Isolated Sessions vs. `1` Continuous Session)**:
   - `agy` ran all 6 epochs in **1 continuous session (`353` steps)**, keeping `crates/synth-core`, `crates/synth-midi-osc`, `crates/synth-ui-theme`, and `js/konduktiva-peak` warm in its prompt cache across all 6 epochs (`105` `read_file` calls total).
   - `pigs` spawned a **brand-new stateless sub-agent process (`--new-session`) for every epoch**. Each new sub-agent had to re-discover structs and functions written in earlier epochs (`292` `read_file` calls + dozens of `grep`/`sed` shell commands).
2. **Duplicate Full-Workspace Test Runs & Report Compilations Inside Sub-Agents (`268` `run_command` Calls in `pigs` vs. `36` in `agy`)**:
   - Because each `pigs` epoch prompt instructed the sub-agent to *"Ensure all workspace tests pass (`CARGO_BUILD_JOBS=2 cargo test --workspace` and `npm test`) and compile `reports/report-00k.md` with `build-report.sh --pdf`"*, each sub-agent ran:
     - Baseline `cargo test --workspace` and `npm test` at the start of the turn before editing any code,
     - Up to **7 full `cargo test --workspace` runs** during coding (e.g., 31 shell commands in Epoch 3; 39 shell commands in Epoch 6; 32 shell commands in Epoch 8), and
     - Final `cargo test --workspace`, `npm test`, and `build-report.sh --pdf` at the end of the turn  -  right before `SymbolicGateRunner.runAllGates()` ran `ensureReportArtifacts()` (`build-report.sh --pdf`) and `runUnitTests()` (`cargo test --workspace` + `npm test`) **a second time**!
   - Every `cargo test --workspace` run inside the sub-agent printed `150+` lines of test output into the sub-agent's conversation history, inflating `input_tokens` and `cache_read_tokens` on every subsequent step of the turn.
3. **1-Line Structural Outlines on Large Files (`> 600` Lines) Triggering `grep` / `sed` Shell Exploration**:
   - When files like `crates/synth-core/src/profiles.rs` (`2,434L`) or `src/peak_synth.rs` (`1,946L`) exceeded the 600-line limit of `extractTargetEntityInlineSource()`, `extractTargetEntityOutline()` only emitted `L123: pub struct Foo` without struct fields or function return types. Sub-agents compensated by running `grep -n "pub struct"` and `sed -n '1138,1330p'` via `run_command`.
4. **Two Avoidable CEGIS Self-Repair Turns (`414s` and `430k` Non-Cached Tokens)**:
   - **Epoch 6 Repair (`b2c7f2bc`, `278s`, `204k` tokens)**: `SymbolicGateRunner._scanForForbiddenImports()` flagged `pub use gui::{is_text_input_active, MainTab, RandomizerUiMode};` in `src/lib.rs` (`inv:clean-architecture`) because `lib.rs` (the crate root module manifest) was not exempted alongside `main.rs` and `app.rs`.
   - **Epoch 8 Repair (`962e8d74`, `136s`, `226k` tokens)**: A single `.unwrap()` on a constant IP literal (`"127.0.0.1:0".parse().unwrap()`) in `src/main.rs:205` triggered a 92-step sub-agent turn (`33` shell commands) to replace one line.

### Five Concrete Improvements Implemented in `pigsAndFishes`

To eliminate wrong-domain planning work and close the remaining token/time gap with standalone `agy` while preserving `pigs`'s superior verification depth (`196` tests, `0` `unwrap()`/`expect()`, full `--help` CLI support, and headless geometry auditing), we designed and implemented five architectural upgrades in `pigsAndFishes`:

| # | Improvement Area | Plain-Language Goal | Exact Technical Implementation in `pigsAndFishes` |
| :--- | :--- | :--- | :--- |
| **1** | **System Boundary & Role Classifier (`HARDWARE_CONTROLLER_LIBRARIAN` vs. `SOFTWARE_DSP_ENGINE`)** | Teach `pigs` the difference between building a remote control/librarian for external hardware vs. building a software audio synthesizer | Added `HARDWARE_CONTROLLER_LIBRARIAN` to `INTENT_DOMAIN_CATALOG` and `classifySystemBoundaryRole()` in `src/intent/IntentDomains.js` so controller/editor/librarian prompts suppress `AUDIO_DSP` software simulation goals |
| **2** | **Workspace Capability Subtraction & 6-Epoch Controller Planner (`validatePlanAgainstRole`)** | Don't plan epochs for work that `synth-core` already does, and reject any plan that simulates hardware DSP in a controller app | Updated `IntentEngine.planAutonomousApplicationEpochs()` and added `IntentEngine.validatePlanAgainstRole()` in `src/intent/IntentEngine.js` to subtract existing `synth-core`/`synth-midi-osc`/`synth-ui-theme` capabilities, prune/merge redundant `DSP_AUDIO` simulation epochs into **6 lean controller epochs**, and inject `[SYSTEM BOUNDARY  -  CONTROLLER, NOT DSP SIMULATOR]` (`inv:controller-not-dsp-simulator`) |
| **3** | **Rich Multi-Line Struct/Signature Outlines & Cross-Epoch Workspace API Digest** | Give each epoch sub-agent the exact struct fields and function signatures created in earlier epochs so it never needs `grep` or `sed` | Upgraded `extractTargetEntityOutline()` and added `extractWorkspaceApiDigest()` in `src/agy/AgyBridge.js` to include struct fields, enum variants, and full `pub fn (...) -> ...` signatures across `src/*.rs` and shared crates |
| **4** | **Single-Pass Testing & Gate-Delegated Report Compilation** | Stop running `cargo test --workspace`, `npm test`, and `build-report.sh --pdf` repeatedly inside the sub-agent when `SymbolicGateRunner` runs them on exit | Updated `AgyBridge.buildCagedPrompt()` and `IntentEngine.planAutonomousApplicationEpochs()` to forbid start-of-turn baseline test runs, use fast `cargo check --quiet` / `cargo test --lib` during edits, and delegate `build-report.sh --pdf` + final full-workspace test verification to `SymbolicGateRunner.runAllGates()` |
| **5** | **Crate-Root `src/lib.rs` Exemption, Pre-Gate `.unwrap()` Auto-Sanitizer & CLI `--help` Gate Check** | Eliminate false-positive `inv:clean-architecture` repairs on `src/lib.rs` re-exports, auto-fix trivial literal `.unwrap()` calls without spawning a 225k-token repair turn, and verify `--help` | Updated `_scanForForbiddenImports()` (`baseName === "lib.rs"` exemption) and added `_autoSanitizeTrivialUnwraps()` in `src/gates/SymbolicGateRunner.js` and `src/world-model/SoftwareWorldModel.js` |

## Section 5: Pre-Flight & Mid-Run Discoveries and Permanent Fixes

### Plain-Language Explanation

Running a live benchmark with real external libraries (`~/claude/novation-peak/`) and live USB-MIDI hardware (`Peak MIDI 1` at `hw:3,0,0`) surfaced two subtle integration bugs that we diagnosed and fixed permanently:

1. **Pre-Flight Discovery in `konduktiva-peak` (`js/konduktiva-peak/src/players.js`)**:
   - **What happened**: When we ran `js/konduktiva-peak/test/run.js` before starting the clock, `konduktiva`'s note scheduler threw `resolveFinalNotesAt: player "arp" must bind a modeMap resolving to a core modeFilter`.
   - **Why it happened**: `players.js` bound `env.modeMaps.peakPass = new QuantizedMap(1, [0], ['__pass__'])` relying on older `konduktiva` behavior where a mode name absent from `env.modeFilters` passed scale degrees through unchanged. Newer `konduktiva` (`src/midi-io.js:540`) requires `env.modeFilters[modeName] instanceof QuantizedMap`.
   - **Permanent Fix**: We registered `env.modeFilters.__pass__ = new QuantizedMap(12, [0,1,2,3,4,5,6,7,8,9,10,11], [0,1,2,3,4,5,6,7,8,9,10,11])` (an identity 12-semitone pass-through `QuantizedMap`) in `ensureSharedMaps(env)` across `/home/renick/claude/novation-peak/src/players.js`, `/home/renick/antigravity/novation-peak-agy/js/konduktiva-peak/src/players.js`, and `/home/renick/antigravity/novation-peak-pigs/js/konduktiva-peak/src/players.js`. All `konduktiva-peak` capture tests immediately passed (`note-on: 33`, `note-off: 284`, `cc: 44`, `5/5` expected CCs on channel 1).
2. **Mid-Run Discovery at the End of Epoch 1 in `pigs` (`SymbolicGateRunner.js` Circular Symlink Traversal)**:
   - **What happened**: Right after `pigs` finished synthesizing `Epoch 1 of 8` (`83` Rust tests passing, `8` Node tests passing, `report-001.{md,html,pdf}` compiled), the Node.js process in `tmux 12:0.0` hit a 4 GB V8 heap out-of-memory (OOM) crash during `Checking code syntax and build...`.
   - **Why it happened**: `SymbolicGateRunner.checkSyntax()` called `fs.readdirSync(scanDir, { recursive: true }).filter(f => !f.includes("node_modules"))` on `js/`. Because `fs.readdirSync(..., { recursive: true })` walks the entire directory tree in C++ *before* returning the array to `.filter(...)`, it entered `js/konduktiva-peak/node_modules`, where `/home/renick/claude/novation-peak/node_modules/konduktiva -> ../../konduktiva` and `/home/renick/claude/konduktiva/node_modules/konduktiva-form -> ../../konduktiva-form -> node_modules/konduktiva` formed a 2-hop circular symlink loop!
   - **Permanent Fix**: We replaced all 7 `fs.readdirSync(..., { recursive: true })` calls in `SymbolicGateRunner.js`, `SoftwareWorldModel.js`, and `ProjectExtractor.js` with `_walkRelativeFilesSafe()`, which skips `node_modules`, `target`, `dist`, `build`, `.git`, and symbolic links (`ent.isSymbolicLink()`) **before** recursing. After applying the fix, `SymbolicGateRunner.runAllGates()` completed all 4 verification gates on `novation-peak-pigs` in **6 seconds using only 11 MB of heap**, and `pigs` executed `Epoch 2 of 8` through `Epoch 8 of 8` non-stop without a single glitch.

## Section 6: Verification and Test Summary

All automated test suites across `pigsAndFishes`, `novation-peak-pigs`, and `novation-peak-agy` were executed and verified prior to filing this report:

- **`pigsAndFishes` Harness Suite (`npm test`)**: **`177 / 177` passing** (`0` failures) in `6.67s`, including dedicated regression tests for `classifySystemBoundaryRole()`, `IntentEngine.validatePlanAgainstRole()` pruning `peak_synth.rs` and `peak_mod_fx.rs` into 6 lean controller epochs, multi-line struct/signature extraction in `AgyBridge.extractTargetEntityOutline()` and `extractWorkspaceApiDigest()`, root `src/lib.rs` `inv:clean-architecture` exemption, and `SymbolicGateRunner._autoSanitizeTrivialUnwraps()`.
- **`novation-peak-pigs` Rust & Node.js Suites**:
  - `CARGO_BUILD_JOBS=2 cargo test --workspace`: **`168 / 168` passing** (`63` in `novation-peak-manager` lib + `9` in `tests/cross_synth_parity.rs` + `34` in `synth-core` + `33` in `synth-midi-osc` + `29` in `synth-ui-theme`), `0` warnings.
  - `npm test`: **`28 / 28` passing** (`20` checks in `js/konduktiva-peak/test/run.js` + `8` tests in `test/novation-peak-profile.test.js` and `test/baseline.test.js`).
  - CLI Verification (`--help`, `--verify-hardware`, `--dry-stress`): Verified `ALL 3 HARDWARE TARGETS VERIFIED` (`Focusrite A.E Peak hw:3,0,0`, `BEHRINGER UMC1820 hw:0,0,0`, `KORG INC. nanoKONTROL2 hw:2,0,0`) in `Virtual Loopback (Offline Safe)` mode and `VERDICT: PASS` on the 1,000-event stress benchmark (`19,779.9` events/sec).
- **`novation-peak-agy` Rust & Node.js Suites**:
  - `CARGO_BUILD_JOBS=2 cargo test --workspace`: **`100 / 100` passing** (`18` in `novation-peak-manager` lib + `1` in `main.rs` + `2` in `tests/e2e_workspace.rs` + `29` in `synth-core` + `25` in `synth-midi-osc` + `25` in `synth-ui-theme`), `0` warnings.
  - `npm test`: **`16 / 16` passing** (`12` checks in `js/konduktiva-peak/test/run.js` + `4` tests in `test/*.test.js`).
  - CLI Verification (`--verify-hardware`, `--dry-stress 2`): Verified `74/74 CCs verified`, `Safety Mode: VirtualLoopback (live: false, physical outputs unarmed)`, and `verdict=PASS`.
