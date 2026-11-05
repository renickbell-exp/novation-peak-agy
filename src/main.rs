// SPDX-License-Identifier: GPL-3.0-or-later
//! CLI and headless GUI workstation entrypoint (`src/main.rs`) for the
//! Novation Peak Manager & Konduktiva Companion (`novation-peak-manager`).
//!
//! Supported flags (all strictly defaulting to `MidiSafetyMode::VirtualLoopback`):
//! - `--verify-hardware`: Non-invasive USB/ALSA hardware discovery (`hw:3,0,0`, `hw:0,0,0`, `hw:2,0,0`)
//! - `--dry-stress [seconds]`: Runs the 5-stage 8-player Konduktiva dry-run stress test
//! - `--export-schemas <dir>`: Exports all JSON schemas, profiles, hardware configs, and themes for all 5 synths
//! - `--list-patches`: Lists all 16 factory presets across Banks A..D
//! - `--inspect-patch <name>`: Inspects a factory patch, its active mod matrix slots, and its 527-byte SysEx frame
//! - `--headless-frames <N>`: Renders `N` headless `egui` frames cycling through all 6 workstation tabs

use novation_peak_manager::{
    app_title,
    gui::PeakWorkstationApp,
    konduktiva::run_dry_stress_test,
    librarian::build_peak_factory_library,
    mod_matrix::read_all_slots_from_patch,
    params::verify_profile_parity_with_cc_map,
    sysex::encode_patch_to_sysex,
    synth_core::export_all_schemas,
    synth_id,
    synth_midi_osc::{
        discover_peak_hardware, export_midi_osc_schemas, scan_hardware_devices, MidiSafetyMode,
    },
    synth_ui_theme::export_all_theme_schemas,
};
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run_cli(&args) {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("Error: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Executes the CLI command specified in `args` and returns the formatted report string.
pub fn run_cli(args: &[String]) -> Result<String, String> {
    if args.is_empty() {
        let mut app = PeakWorkstationApp::new();
        let frames = app.run_headless_frames(6);
        return Ok(format!(
            "{} ({})\nSafety Mode: {:?} (Hardware Strictly Unarmed)\nVerified {} headless egui workstation frames across all 6 tabs.\n",
            app_title(),
            synth_id(),
            app.engine.router.safety_mode,
            frames
        ));
    }

    let mut out = String::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--verify-hardware" => {
                let cc_count = verify_profile_parity_with_cc_map()?;
                let virtual_disc = discover_peak_hardware(MidiSafetyMode::VirtualLoopback);
                let live_scan = scan_hardware_devices();
                out.push_str(&format!(
                    "[verify-hardware] {} ({})\n",
                    app_title(),
                    synth_id()
                ));
                out.push_str(&format!(
                    "  Safety Mode          : {:?} (live: false, physical outputs unarmed)\n",
                    virtual_disc.safety_mode
                ));
                out.push_str(&format!("  CC Map Parity        : {cc_count}/74 CCs verified\n"));
                if let Some(peak) = &virtual_disc.peak {
                    out.push_str(&format!(
                        "  Peak Endpoint        : {} ({}) seq={:?}:{:?}\n",
                        peak.card_name, peak.rawmidi_device, peak.seq_client, peak.seq_port
                    ));
                }
                if let Some(umc) = &virtual_disc.umc1820 {
                    out.push_str(&format!(
                        "  UMC1820 Endpoint     : {} ({}) seq={:?}:{:?}\n",
                        umc.card_name, umc.rawmidi_device, umc.seq_client, umc.seq_port
                    ));
                }
                if let Some(nk2) = &virtual_disc.nanokontrol2 {
                    out.push_str(&format!(
                        "  nanoKONTROL2 Surface : {} ({}) seq={:?}:{:?}\n",
                        nk2.card_name, nk2.rawmidi_device, nk2.seq_client, nk2.seq_port
                    ));
                }
                out.push_str(&format!(
                    "  Host ALSA Scan Count : {} connected sound/MIDI device(s)\n",
                    live_scan.len()
                ));
            }
            "--dry-stress" => {
                let seconds = if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                    i += 1;
                    args[i].parse::<u64>().unwrap_or(2)
                } else {
                    2
                };
                let report = run_dry_stress_test(seconds);
                out.push_str(&format!(
                    "[dry-stress] Duration={}s | noteon={} | noteoff={} | cc={} | expected_ccs={}/5 | ch1_only={} | verdict={}\n",
                    report.duration_seconds,
                    report.stats.noteon,
                    report.stats.noteoff,
                    report.stats.cc,
                    report.expected_ccs_present,
                    report.channel_1_only,
                    if report.pass { "PASS" } else { "FAIL" }
                ));
                if !report.pass {
                    return Err("Dry stress test failed PASS criteria".to_string());
                }
            }
            "--export-schemas" => {
                if i + 1 >= args.len() {
                    return Err("--export-schemas requires a target directory path".to_string());
                }
                i += 1;
                let dir = PathBuf::from(&args[i]);
                export_all_schemas(&dir).map_err(|e| format!("synth-core schema export: {e}"))?;
                export_midi_osc_schemas(&dir)
                    .map_err(|e| format!("synth-midi-osc schema export: {e}"))?;
                export_all_theme_schemas(&dir)
                    .map_err(|e| format!("synth-ui-theme schema export: {e}"))?;
                out.push_str(&format!(
                    "[export-schemas] Exported all 5-synthesizer profiles, mappings, hardware configs, and themes to {}\n",
                    dir.display()
                ));
            }
            "--list-patches" => {
                let lib = build_peak_factory_library();
                let patches = lib.all_patches();
                out.push_str(&format!(
                    "[list-patches] {} Factory Presets across Banks A..D:\n",
                    patches.len()
                ));
                for p in patches {
                    let sysex_len = p
                        .sysex
                        .raw_dump
                        .as_ref()
                        .map_or_else(|| encode_patch_to_sysex(p, false).len(), Vec::len);
                    out.push_str(&format!(
                        "  - Bank {} Slot {:03} | {:<22} | Category: {:<8} | SysEx: {} bytes\n",
                        p.meta.bank, p.meta.slot, p.meta.name, p.meta.category, sysex_len
                    ));
                }
            }
            "--inspect-patch" => {
                if i + 1 >= args.len() {
                    return Err("--inspect-patch requires a patch name".to_string());
                }
                i += 1;
                let query = args[i].to_lowercase();
                let lib = build_peak_factory_library();
                let patch = lib
                    .all_patches()
                    .into_iter()
                    .find(|p| p.meta.name.to_lowercase().contains(&query))
                    .ok_or_else(|| format!("No factory patch matching '{}'", args[i]))?;
                let sysex = encode_patch_to_sysex(patch, true);
                let active_slots: Vec<_> = read_all_slots_from_patch(patch)
                    .into_iter()
                    .filter(|s| s.is_active())
                    .collect();
                out.push_str(&format!(
                    "[inspect-patch] {} (Bank {} Slot {}, Category: {})\n  Parameters: {} | Active Mod Slots: {} | SysEx Frame: {} bytes\n",
                    patch.meta.name,
                    patch.meta.bank,
                    patch.meta.slot,
                    patch.meta.category,
                    patch.parameters.len(),
                    active_slots.len(),
                    sysex.len()
                ));
            }
            "--headless-frames" => {
                let count = if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                    i += 1;
                    args[i].parse::<usize>().unwrap_or(6)
                } else {
                    6
                };
                let mut app = PeakWorkstationApp::new();
                let rendered = app.run_headless_frames(count);
                out.push_str(&format!(
                    "[headless-frames] Rendered {rendered} headless egui frames across all 6 tabs.\n"
                ));
            }
            other => {
                return Err(format!("Unrecognized CLI argument: {other}"));
            }
        }
        i += 1;
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_flags_verify_hardware_dry_stress_and_list_patches() {
        let out = run_cli(&[
            "--verify-hardware".to_string(),
            "--dry-stress".to_string(),
            "1".to_string(),
            "--list-patches".to_string(),
            "--inspect-patch".to_string(),
            "Oxford Warm Pad".to_string(),
            "--headless-frames".to_string(),
            "6".to_string(),
        ])
        .expect("CLI execution");

        assert!(out.contains("74/74 CCs verified"));
        assert!(out.contains("verdict=PASS"));
        assert!(out.contains("16 Factory Presets"));
        assert!(out.contains("SysEx Frame: 527 bytes"));
        assert!(out.contains("Rendered 6 headless egui frames"));
    }
}
