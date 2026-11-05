// SPDX-License-Identifier: GPL-3.0-or-later
//! Native Rust/`egui` Desktop Synthesizer Workstation GUI for the Novation Peak (`src/gui.rs`).
//!
//! Renders 6 workstation tabs styled with `SynthAccent::PeakOxfordBlue` (`#1A73E8`) and
//! warm OLED amber readouts (`#FF8C00`), integrating custom Peak vector widgets,
//! the 16-slot Modulation Matrix editor, A/B Patch Diff & Librarian, 3-bank Korg
//! nanoKONTROL2 surface visualizer, and Konduktiva live-coding companion panel.

use crate::controller::NanoKontrol2PeakController;
use crate::librarian::{
    apply_safe_randomization_with_history, build_peak_factory_library, build_peak_randomizer,
};
use crate::midi::PeakHardwareEngine;
use crate::mod_matrix::{
    apply_slot_with_history, read_all_slots_from_patch, ModDestination, ModSlot, ModSource,
};
use crate::params::PeakMidiCcMapSpec;
use crate::sysex::PeakPatchBank;
use std::time::Instant;
use synth_core::{
    selective_merge_section, CommandHistory, DeviceSynthesizerProfile, LockShieldedRandomizer,
    PatchDiff, PresetLibrary, SetParamCommand, SoftTakeoverMode, SynthPatch,
};
use synth_ui_theme::{
    animate_hold_button_widget, state_variable_filter_widget, voice_allocation_meter_widget,
    wavetable_oscillator_widget, AnimateHoldButtonModel, CentralPanel, ComboBox, Context, Grid,
    RawInput, RichText, ScrollArea, Slider, StateVariableFilterModel, SynthAccent, ThemeConfig,
    TopBottomPanel, TwoBowlsBackground, Ui, VoiceAllocationMeterModel, WavetableOscillatorModel,
};

/// The 6 primary workstation tabs in the Novation Peak Manager GUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PeakTab {
    OscillatorsAndMixer,
    FilterEnvelopesLfos,
    FxAndModMatrix,
    LibrarianAndAbDiff,
    NanoKontrol2Surface,
    KonduktivaCompanion,
}

impl PeakTab {
    /// Returns all 6 workstation tabs in display order.
    pub const fn all() -> [Self; 6] {
        [
            Self::OscillatorsAndMixer,
            Self::FilterEnvelopesLfos,
            Self::FxAndModMatrix,
            Self::LibrarianAndAbDiff,
            Self::NanoKontrol2Surface,
            Self::KonduktivaCompanion,
        ]
    }

    /// Human-readable tab title.
    pub const fn title(&self) -> &'static str {
        match self {
            Self::OscillatorsAndMixer => "1. Oscillators & Mixer",
            Self::FilterEnvelopesLfos => "2. Filter, Envs & LFOs",
            Self::FxAndModMatrix => "3. FX & 16-Slot Mod Matrix",
            Self::LibrarianAndAbDiff => "4. Patch Librarian & A/B Diff",
            Self::NanoKontrol2Surface => "5. nanoKONTROL2 3-Bank",
            Self::KonduktivaCompanion => "6. Konduktiva Live Companion",
        }
    }
}

/// Complete native `egui` desktop workstation state for the Novation Peak Manager.
pub struct PeakWorkstationApp {
    pub profile: DeviceSynthesizerProfile,
    pub cc_spec: PeakMidiCcMapSpec,
    pub library: PresetLibrary,
    pub active_patch: SynthPatch,
    pub compare_patch_b: SynthPatch,
    pub history: CommandHistory,
    pub randomizer: LockShieldedRandomizer,
    pub engine: PeakHardwareEngine,
    pub controller: NanoKontrol2PeakController,
    pub theme: ThemeConfig,
    pub active_tab: PeakTab,
    pub search_query: String,
    pub selected_category: String,
    pub randomize_amount_pct: f64,
    pub animate_1_btn: AnimateHoldButtonModel,
    pub animate_2_btn: AnimateHoldButtonModel,
    pub voice_meter: VoiceAllocationMeterModel,
    pub active_density_graph: String,
    pub tempo_bpm: f64,
    pub status_banner: String,
}

impl Default for PeakWorkstationApp {
    fn default() -> Self {
        Self::new()
    }
}

impl PeakWorkstationApp {
    /// Creates a new `PeakWorkstationApp` in safe `VirtualLoopback` mode with the 16 factory presets loaded.
    #[must_use]
    pub fn new() -> Self {
        let profile = DeviceSynthesizerProfile::novation_peak();
        let cc_spec = PeakMidiCcMapSpec::load_embedded();
        let library = build_peak_factory_library();
        let all = library.all_patches();
        let active_patch = all
            .first()
            .cloned()
            .unwrap_or_else(|| profile.create_default_patch("Init Patch"));
        let compare_patch_b = all
            .get(1)
            .cloned()
            .unwrap_or_else(|| active_patch.clone());

        let history = CommandHistory::new(128);
        let randomizer = build_peak_randomizer();
        let engine = PeakHardwareEngine::default();
        let controller = NanoKontrol2PeakController::new(&active_patch, SoftTakeoverMode::Catch);
        let theme = ThemeConfig::new()
            .with_accent(SynthAccent::PeakOxfordBlue)
            .with_background(TwoBowlsBackground::DarkGrey);

        Self {
            profile,
            cc_spec,
            library,
            active_patch,
            compare_patch_b,
            history,
            randomizer,
            engine,
            controller,
            theme,
            active_tab: PeakTab::OscillatorsAndMixer,
            search_query: String::new(),
            selected_category: "All".to_string(),
            randomize_amount_pct: 35.0,
            animate_1_btn: AnimateHoldButtonModel::new(1, 114, false),
            animate_2_btn: AnimateHoldButtonModel::new(2, 115, false),
            voice_meter: VoiceAllocationMeterModel::new(),
            active_density_graph: "peakBuild".to_string(),
            tempo_bpm: 120.0,
            status_banner: "Ready | Peak MIDI 1 (hw:3,0,0) in VirtualLoopback Safe Mode".to_string(),
        }
    }

    /// Sets a parameter on `active_patch` through `CommandHistory` and schedules its MIDI CC on `engine`.
    pub fn set_param_with_history(&mut self, param_key: &str, new_val: f64) {
        let old_val = self.active_patch.get_param(param_key).unwrap_or(0.0);
        if (old_val - new_val).abs() > 1e-4 {
            let _ = self.history.push_and_apply(
                Box::new(SetParamCommand::new(param_key, old_val, new_val)),
                &mut self.active_patch,
            );
            if let Some(entry) = self.cc_spec.resolve_cc_by_name(param_key) {
                self.engine
                    .schedule_named_cc(entry.cc, &entry.param, new_val.round() as u8);
            }
        }
    }

    /// Renders a single frame of the workstation UI onto an `egui::Context`.
    pub fn render_ui(&mut self, ctx: &Context) {
        self.theme.apply_to_context(ctx);
        let accent = self.theme.resolved_accent_color();
        let readout = self.theme.resolved_readout_color();

        self.voice_meter
            .set_active_notes(&self.engine.router.active_voices);

        TopBottomPanel::top("peak_header_bar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading(
                    RichText::new("NOVATION PEAK MANAGER")
                        .color(accent)
                        .strong(),
                );
                ui.separator();
                ui.label(
                    RichText::new(format!(
                        "Patch: {} [{}{:03}]",
                        self.active_patch.meta.name,
                        self.active_patch.meta.bank,
                        self.active_patch.meta.slot
                    ))
                    .color(readout)
                    .monospace(),
                );
                ui.separator();

                let r1 = animate_hold_button_widget(ui, &mut self.animate_1_btn, accent, readout);
                if r1.clicked() {
                    let held = self.animate_1_btn.held;
                    self.active_patch
                        .set_param("animate_1_hold", if held { 127.0 } else { 0.0 });
                    self.engine.send_animate_hold(1, held);
                }
                let r2 = animate_hold_button_widget(ui, &mut self.animate_2_btn, accent, readout);
                if r2.clicked() {
                    let held = self.animate_2_btn.held;
                    self.active_patch
                        .set_param("animate_2_hold", if held { 127.0 } else { 0.0 });
                    self.engine.send_animate_hold(2, held);
                }

                voice_allocation_meter_widget(ui, &self.voice_meter, accent, readout);

                if ui
                    .add_enabled(self.history.can_undo(), synth_ui_theme::Button::new("Undo"))
                    .clicked()
                {
                    let _ = self.history.undo(&mut self.active_patch);
                }
                if ui
                    .add_enabled(self.history.can_redo(), synth_ui_theme::Button::new("Redo"))
                    .clicked()
                {
                    let _ = self.history.redo(&mut self.active_patch);
                }
                if ui.button("PANIC (All Notes Off)").clicked() {
                    self.engine.panic(Instant::now());
                    self.status_banner = "Panic dispatched (CC 120 + CC 123 + 128 NoteOffs)".to_string();
                }
            });

            ui.separator();
            ui.horizontal_wrapped(|ui| {
                for tab in PeakTab::all() {
                    let selected = self.active_tab == tab;
                    if ui.selectable_label(selected, tab.title()).clicked() {
                        self.active_tab = tab;
                    }
                }
            });
        });

        TopBottomPanel::bottom("peak_status_footer").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.status_banner).monospace().color(readout));
                ui.separator();
                ui.label(
                    RichText::new(format!(
                        "Captured: NoteOn={} NoteOff={} CC={} Thinned={}",
                        self.engine.stats.noteon,
                        self.engine.stats.noteoff,
                        self.engine.stats.cc,
                        self.engine.stats.ccs_thinned
                    ))
                    .monospace(),
                );
            });
        });

        CentralPanel::default().show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| match self.active_tab {
                PeakTab::OscillatorsAndMixer => self.render_oscillators_and_mixer_tab(ui, accent, readout),
                PeakTab::FilterEnvelopesLfos => self.render_filter_envelopes_lfos_tab(ui, accent, readout),
                PeakTab::FxAndModMatrix => self.render_fx_and_mod_matrix_tab(ui, accent, readout),
                PeakTab::LibrarianAndAbDiff => self.render_librarian_and_diff_tab(ui, accent, readout),
                PeakTab::NanoKontrol2Surface => self.render_nanokontrol2_tab(ui, accent, readout),
                PeakTab::KonduktivaCompanion => self.render_konduktiva_tab(ui, accent, readout),
            });
        });

        let _ = self.engine.flush(Instant::now());
    }

    fn render_param_slider(&mut self, ui: &mut Ui, label: &str, param_key: &str) {
        let mut val = self.active_patch.get_param(param_key).unwrap_or(64.0);
        ui.horizontal(|ui| {
            ui.label(label);
            if ui
                .add(Slider::new(&mut val, 0.0..=127.0).step_by(1.0))
                .changed()
            {
                self.set_param_with_history(param_key, val);
            }
        });
    }

    fn render_oscillators_and_mixer_tab(&mut self, ui: &mut Ui, accent: synth_ui_theme::Color32, readout: synth_ui_theme::Color32) {
        ui.heading("New Oxford FPGA Oscillators (Osc 1, Osc 2, Osc 3) & Analog Mixer");
        ui.columns(3, |cols| {
            let osc_configs = [
                (1u8, "osc_1_range", "osc_1_coarse", "osc_1_fine", "osc_1_shape", "osc_1_vsync", "osc_1_lfo_2_pitch", "osc_1_lfo_1_shape", "osc_1_mod_1_shape"),
                (2u8, "osc_2_range", "osc_2_coarse", "osc_2_fine", "osc_2_shape", "osc_2_vsync", "osc_2_lfo_2_pitch", "osc_2_lfo_1_shape", "osc_2_mod_1_shape"),
                (3u8, "osc_3_range", "osc_3_coarse", "osc_3_fine", "osc_3_shape", "osc_3_vsync", "osc_3_lfo_2_pitch", "osc_3_lfo_1_shape", "osc_3_mod_1_pitch"),
            ];
            for (idx, (osc_num, range_k, coarse_k, fine_k, shape_k, vsync_k, lfo2_k, lfo1_k, mod1_k)) in osc_configs.iter().enumerate() {
                let col = &mut cols[idx];
                let shape_norm = (self.active_patch.get_param(shape_k).unwrap_or(64.0) / 127.0) as f32;
                let vsync_norm = (self.active_patch.get_param(vsync_k).unwrap_or(0.0) / 127.0) as f32;
                let coarse_st = self.active_patch.get_param(coarse_k).unwrap_or(64.0) as i32 - 64;
                let model = WavetableOscillatorModel::new(*osc_num, shape_norm, vsync_norm, coarse_st);
                wavetable_oscillator_widget(col, &model, accent, readout);
                self.render_param_slider(col, "Range (Oct)", range_k);
                self.render_param_slider(col, "Coarse Tune", coarse_k);
                self.render_param_slider(col, "Fine Tune", fine_k);
                self.render_param_slider(col, "Wave Shape", shape_k);
                self.render_param_slider(col, "Virtual Sync", vsync_k);
                self.render_param_slider(col, "LFO 2 -> Pitch", lfo2_k);
                self.render_param_slider(col, "LFO 1 -> Shape", lfo1_k);
                self.render_param_slider(col, "Mod Env 1 -> Shape/Pitch", mod1_k);
            }
        });
        ui.separator();
        ui.heading("Mixer, Ring Mod, Noise & Glide");
        self.render_param_slider(ui, "Osc 1 Mix (CC 23)", "osc_1_mix");
        self.render_param_slider(ui, "Osc 2 Mix (CC 24)", "osc_2_mix");
        self.render_param_slider(ui, "Osc 3 Mix (CC 25)", "osc_3_mix");
        self.render_param_slider(ui, "Ring Mod 1*2 Mix (CC 26)", "ring_mod_mix");
        self.render_param_slider(ui, "Noise Mix (CC 27)", "noise_mix");
        self.render_param_slider(ui, "Glide Time (CC 5)", "glide_time");
        self.render_param_slider(ui, "Glide On (CC 35)", "glide_on");
    }

    fn render_filter_envelopes_lfos_tab(&mut self, ui: &mut Ui, accent: synth_ui_theme::Color32, readout: synth_ui_theme::Color32) {
        ui.heading("Analog OTA State-Variable Multimode Filter (12/24 dB LP/BP/HP)");
        let cutoff = (self.active_patch.get_param("filter_cutoff_frequency").unwrap_or(100.0) / 127.0) as f32;
        let reso = (self.active_patch.get_param("filter_resonance").unwrap_or(16.0) / 127.0) as f32;
        let overdrive = (self.active_patch.get_param("filter_overdrive").unwrap_or(0.0) / 127.0) as f32;
        let post_drive = (self.active_patch.get_param("filter_post_drive").unwrap_or(0.0) / 127.0) as f32;
        let svf = StateVariableFilterModel::new(cutoff, reso, overdrive, post_drive);
        state_variable_filter_widget(ui, &svf, accent, readout);

        self.render_param_slider(ui, "Filter Cutoff (CC 29)", "filter_cutoff_frequency");
        self.render_param_slider(ui, "Filter Resonance (CC 79, Safe<=62)", "filter_resonance");
        self.render_param_slider(ui, "Pre-Filter Overdrive (CC 80, Safe<=50)", "filter_overdrive");
        self.render_param_slider(ui, "Post-Filter Drive (CC 36, Safe<=50)", "filter_post_drive");
        self.render_param_slider(ui, "Key Tracking (CC 75)", "filter_track");
        self.render_param_slider(ui, "Mod Env 1 -> Filter (CC 78)", "mod_1_filter");
        self.render_param_slider(ui, "Osc 3 -> Filter (CC 76)", "osc_3_filter");
        self.render_param_slider(ui, "LFO 1 -> Filter Depth (CC 28)", "lfo_1_filter_depth");

        ui.separator();
        ui.columns(2, |cols| {
            cols[0].heading("Amp Envelope & Mod Envelopes 1/2");
            self.render_param_slider(&mut cols[0], "Amp Attack (CC 86)", "amp_envelope_attack");
            self.render_param_slider(&mut cols[0], "Amp Decay (CC 87)", "amp_envelope_decay");
            self.render_param_slider(&mut cols[0], "Amp Sustain (CC 88)", "amp_envelope_sustain");
            self.render_param_slider(&mut cols[0], "Amp Release (CC 89)", "amp_envelope_release");
            self.render_param_slider(&mut cols[0], "Mod 1 Attack (CC 90)", "mod_envelope_1_attack");
            self.render_param_slider(&mut cols[0], "Mod 1 Decay (CC 91)", "mod_envelope_1_decay");
            self.render_param_slider(&mut cols[0], "Mod 1 Sustain (CC 92)", "mod_envelope_1_sustain");
            self.render_param_slider(&mut cols[0], "Mod 1 Release (CC 93)", "mod_envelope_1_release");

            cols[1].heading("LFO 1, LFO 2 & Mod Envelope 2");
            self.render_param_slider(&mut cols[1], "LFO 1 Rate (CC 30)", "lfo_1_rate");
            self.render_param_slider(&mut cols[1], "LFO 1 Sync Rate (CC 81)", "lfo_1_sync_rate");
            self.render_param_slider(&mut cols[1], "LFO 1 Fade (CC 82)", "lfo_1_fade");
            self.render_param_slider(&mut cols[1], "LFO 2 Rate (CC 31)", "lfo_2_rate");
            self.render_param_slider(&mut cols[1], "LFO 2 Sync Rate (CC 84)", "lfo_2_sync_rate");
            self.render_param_slider(&mut cols[1], "LFO 2 Fade (CC 85)", "lfo_2_fade");
            self.render_param_slider(&mut cols[1], "Mod 2 Attack (CC 94)", "mod_envelope_2_attack");
            self.render_param_slider(&mut cols[1], "Mod 2 Decay (CC 95)", "mod_envelope_2_decay");
            self.render_param_slider(&mut cols[1], "Mod 2 Sustain (CC 117)", "mod_envelope_2_sustain");
            self.render_param_slider(&mut cols[1], "Mod 2 Release (CC 103)", "mod_envelope_2_release");
        });
    }

    fn render_fx_and_mod_matrix_tab(&mut self, ui: &mut Ui, _accent: synth_ui_theme::Color32, readout: synth_ui_theme::Color32) {
        ui.heading("Analog Distortion & Master Digital FX (Chorus, Delay, Reverb)");
        self.render_param_slider(ui, "Analog Distortion Level (CC 104, Safe<=48)", "distortion_level");
        self.render_param_slider(ui, "Chorus Level (CC 105, Safe<=90)", "chorus_level");
        self.render_param_slider(ui, "Chorus Rate (CC 118)", "chorus_rate");
        self.render_param_slider(ui, "Chorus Feedback (CC 107, Safe<=70)", "chorus_feedback");
        self.render_param_slider(ui, "Delay Level (CC 108, Safe<=90)", "delay_level");
        self.render_param_slider(ui, "Delay Time (CC 109)", "delay_time");
        self.render_param_slider(ui, "Delay Feedback (CC 110, Safe<=70)", "delay_feedback");
        self.render_param_slider(ui, "Reverb Level (CC 112, Safe<=90)", "reverb_level");
        self.render_param_slider(ui, "Reverb Time (CC 113)", "reverb_time");

        ui.separator();
        ui.heading("16-Slot Dual-Source Modulation Matrix (Source 1 x Source 2 -> Destination)");
        let slots = read_all_slots_from_patch(&self.active_patch);
        let mut pending_edit: Option<ModSlot> = None;

        Grid::new("peak_mod_matrix_grid").striped(true).show(ui, |ui| {
            ui.label(RichText::new("Slot").strong());
            ui.label(RichText::new("Source 1").strong());
            ui.label(RichText::new("Source 2").strong());
            ui.label(RichText::new("Depth (-64..+63)").strong());
            ui.label(RichText::new("Destination").strong());
            ui.end_row();

            for slot in slots {
                let mut edited = slot;
                ui.label(RichText::new(format!("#{:02}", slot.slot_index)).color(readout).monospace());
                ComboBox::from_id_salt(format!("src1_{}", slot.slot_index))
                    .selected_text(edited.source_1.display_name())
                    .show_ui(ui, |ui| {
                        for src in ModSource::all() {
                            ui.selectable_value(&mut edited.source_1, src, src.display_name());
                        }
                    });
                ComboBox::from_id_salt(format!("src2_{}", slot.slot_index))
                    .selected_text(edited.source_2.display_name())
                    .show_ui(ui, |ui| {
                        for src in ModSource::all() {
                            ui.selectable_value(&mut edited.source_2, src, src.display_name());
                        }
                    });
                let mut depth_i32 = edited.depth as i32;
                if ui.add(Slider::new(&mut depth_i32, -64..=63)).changed() {
                    edited.depth = depth_i32 as i8;
                }
                ComboBox::from_id_salt(format!("dst_{}", slot.slot_index))
                    .selected_text(edited.destination.display_name())
                    .show_ui(ui, |ui| {
                        for dst in ModDestination::all() {
                            ui.selectable_value(&mut edited.destination, dst, dst.display_name());
                        }
                    });
                ui.end_row();

                if edited != slot {
                    pending_edit = Some(edited);
                }
            }
        });

        if let Some(updated_slot) = pending_edit {
            let _ = apply_slot_with_history(&mut self.history, &mut self.active_patch, &updated_slot);
        }
    }

    fn render_librarian_and_diff_tab(&mut self, ui: &mut Ui, _accent: synth_ui_theme::Color32, readout: synth_ui_theme::Color32) {
        ui.heading("Factory Preset Librarian (Banks A..D), Safe-Sweep Randomizer & A/B Patch Diff");
        ui.horizontal(|ui| {
            ui.label("Search:");
            ui.text_edit_singleline(&mut self.search_query);
            ui.add(Slider::new(&mut self.randomize_amount_pct, 5.0..=100.0).text("Randomize %"));
            if ui.button("Safe-Sweep Randomize").clicked() {
                let _ = apply_safe_randomization_with_history(
                    &mut self.history,
                    &self.randomizer,
                    &mut self.active_patch,
                    self.randomize_amount_pct,
                    100.0,
                    None,
                );
                self.status_banner = format!(
                    "Randomized '{}' within SafeSweepBounds",
                    self.active_patch.meta.name
                );
            }
            if ui.button("Send 527B SysEx to Edit Buffer").clicked() {
                self.engine.send_patch_sysex(&self.active_patch, true);
                self.status_banner = "Queued 527-byte Single Patch Dump SysEx".to_string();
            }
        });

        ui.separator();
        let patches: Vec<SynthPatch> = if self.search_query.trim().is_empty() {
            self.library.all_patches().into_iter().cloned().collect()
        } else {
            self.library
                .search(&self.search_query)
                .into_iter()
                .cloned()
                .collect()
        };

        Grid::new("peak_preset_grid").striped(true).show(ui, |ui| {
            ui.label("Bank/Slot");
            ui.label("Patch Name");
            ui.label("Category");
            ui.label("Actions");
            ui.end_row();

            for p in &patches {
                ui.label(RichText::new(format!("{}{:03}", p.meta.bank, p.meta.slot)).monospace().color(readout));
                ui.label(&p.meta.name);
                ui.label(&p.meta.category);
                ui.horizontal(|ui| {
                    if ui.button("Load as A").clicked() {
                        self.active_patch = p.clone();
                        self.controller.sync_bank_targets_from_patch(&self.active_patch);
                        if let Some(bank) = PeakPatchBank::from_str_code(&p.meta.bank) {
                            self.engine.send_bank_program(bank, p.meta.slot as u8);
                        }
                    }
                    if ui.button("Compare as B").clicked() {
                        self.compare_patch_b = p.clone();
                    }
                });
                ui.end_row();
            }
        });

        ui.separator();
        let diff = PatchDiff::compare(&self.active_patch, &self.compare_patch_b, 0.001);
        ui.heading(format!(
            "A/B Patch Diff: '{}' (A) vs '{}' (B) - {} differences",
            self.active_patch.meta.name,
            self.compare_patch_b.meta.name,
            diff.total_differences()
        ));
        ui.horizontal(|ui| {
            if ui.button("Merge Filter Section (B -> A)").clicked() {
                self.active_patch = selective_merge_section(&self.active_patch, &self.compare_patch_b, "filter_");
            }
            if ui.button("Merge Oscillators (B -> A)").clicked() {
                self.active_patch = selective_merge_section(&self.active_patch, &self.compare_patch_b, "osc_");
            }
        });
    }

    fn render_nanokontrol2_tab(&mut self, ui: &mut Ui, _accent: synth_ui_theme::Color32, readout: synth_ui_theme::Color32) {
        ui.heading("Korg nanoKONTROL2 (0944:0117, hw:2,0,0) 3-Bank Tactile Surface");
        ui.horizontal(|ui| {
            for b in 1..=3u32 {
                let selected = self.controller.mapping.active_bank == b;
                if ui.selectable_label(selected, format!("Bank {}", b)).clicked() {
                    self.controller.switch_bank(b, &self.active_patch);
                }
            }
        });
        let bank = self.controller.active_bank_mapping().clone();
        ui.label(RichText::new(format!("Active Bank {}: {} ({})", bank.bank_index, bank.name, bank.target_mode)).color(readout));

        Grid::new("nk2_bindings_grid").striped(true).show(ui, |ui| {
            ui.label("Surface CC");
            ui.label("Control Label");
            ui.label("Target Parameter");
            ui.label("Soft Takeover Status");
            ui.end_row();

            for binding in &bank.controls {
                ui.label(RichText::new(format!("CC {:02}", binding.hardware_cc)).monospace());
                ui.label(&binding.label);
                ui.label(binding.target_param.as_deref().unwrap_or("-"));
                let status = self
                    .controller
                    .takeover_states
                    .get(&binding.hardware_cc)
                    .map(|s| {
                        if s.is_caught {
                            format!("CAUGHT ({:.0})", s.target_value)
                        } else {
                            format!("PENDING (Target {:.0})", s.target_value)
                        }
                    })
                    .unwrap_or_else(|| "Button / Trigger".to_string());
                ui.label(RichText::new(status).color(readout).monospace());
                ui.end_row();
            }
        });
    }

    fn render_konduktiva_tab(&mut self, ui: &mut Ui, _accent: synth_ui_theme::Color32, readout: synth_ui_theme::Color32) {
        ui.heading("Konduktiva Live-Coding Companion (8 Players on MIDI Channel 1)");
        ui.horizontal(|ui| {
            ui.label("Active Density Graph:");
            ui.selectable_value(&mut self.active_density_graph, "peakBuild".to_string(), "peakBuild (Rising Arc)");
            ui.selectable_value(&mut self.active_density_graph, "peakHold".to_string(), "peakHold (Steady Full)");
            ui.add(Slider::new(&mut self.tempo_bpm, 60.0..=180.0).text("BPM"));
        });
        ui.separator();
        let players = ["bass", "chords", "lead", "arp", "ccCutoff", "ccReso", "ccMod", "ccFx"];
        Grid::new("konduktiva_players_grid").striped(true).show(ui, |ui| {
            ui.label("Player Lane");
            ui.label("MIDI Channel");
            ui.label("Native Rate (beats)");
            ui.end_row();
            for (idx, name) in players.iter().enumerate() {
                ui.label(RichText::new(*name).color(readout).monospace());
                ui.label("Channel 1 (wire 0)");
                ui.add(Slider::new(&mut self.controller.konduktiva_player_rates[idx], 0.25..=8.0));
                ui.end_row();
            }
        });
    }

    /// Executes `frames` headless `egui` layout and paint passes across all 6 workstation tabs
    /// for automated CI verification without requiring an X11/Wayland display server.
    pub fn run_headless_frames(&mut self, frames: usize) -> usize {
        let ctx = Context::default();
        let all_tabs = PeakTab::all();
        for i in 0..frames {
            self.active_tab = all_tabs[i % all_tabs.len()];
            let _ = ctx.run(RawInput::default(), |ctx| {
                self.render_ui(ctx);
            });
        }
        frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peak_workstation_headless_gui_renders_all_6_tabs() {
        let mut app = PeakWorkstationApp::new();
        assert_eq!(app.theme.accent, SynthAccent::PeakOxfordBlue);
        let rendered = app.run_headless_frames(12);
        assert_eq!(rendered, 12);

        // Verify parameter edit through GUI helper records history and schedules CC
        app.set_param_with_history("filter_cutoff_frequency", 112.0);
        assert_eq!(app.active_patch.get_param("filter_cutoff_frequency"), Some(112.0));
        assert!(app.history.can_undo());
    }
}
