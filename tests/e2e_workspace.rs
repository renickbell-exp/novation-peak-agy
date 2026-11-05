// SPDX-License-Identifier: GPL-3.0-or-later
//! End-to-End Workspace Integration Test Suite (`tests/e2e_workspace.rs`)
//! verifying all 6 epochs of the Novation Peak Manager & Konduktiva Companion
//! and 5-synthesizer ecosystem schema exports under strict `VirtualLoopback` safety.

use novation_peak_manager::{
    controller::NanoKontrol2PeakController,
    gui::PeakWorkstationApp,
    konduktiva::{run_dry_stress_test, PeakDensityGraph, PeakKonduktivaCompanion, PEAK_PLAYER_NAMES},
    librarian::{apply_safe_randomization_with_history, build_peak_factory_library},
    mod_matrix::{apply_slot_with_history, read_slot_from_patch, ModDestination, ModSlot, ModSource},
    osc_bridge::PeakOscCoordinator,
    params::verify_profile_parity_with_cc_map,
    sysex::{decode_patch_from_sysex, encode_patch_to_sysex},
    synth_core::{export_all_schemas, PatchDiff},
    synth_midi_osc::{export_midi_osc_schemas, MidiSafetyMode},
    synth_ui_theme::{export_all_theme_schemas, SynthAccent},
};
use rosc::{encoder, OscMessage, OscPacket, OscType};
use std::net::UdpSocket;
use std::time::{Duration, Instant};

#[test]
fn test_five_synthesizer_ecosystem_schema_exports() {
    let temp_dir = std::env::temp_dir().join(format!("peak_e2e_schemas_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);

    export_all_schemas(&temp_dir).expect("Export synth-core schemas");
    export_midi_osc_schemas(&temp_dir).expect("Export synth-midi-osc schemas");
    export_all_theme_schemas(&temp_dir).expect("Export synth-ui-theme schemas");

    let expected_files = [
        // synth-core schemas & profiles across all 5 projects
        "synth-patch.schema.json",
        "device-profile.schema.json",
        "controller-mapping.schema.json",
        "asm-hydrasynth.profile.json",
        "korg-nanokontrol2.profile.json",
        "akai-midimix.profile.json",
        "novation-bass-station-2.profile.json",
        "behringer-td3-md.profile.json",
        "clavia-nord-drum-2.profile.json",
        "novation-peak.profile.json",
        "korg-nanokontrol2-nd2.mapping.json",
        "korg-nanokontrol2-peak.mapping.json",
        // synth-midi-osc schemas & hardware configs across all 5 projects
        "stored-device-profile.schema.json",
        "dual-midi-router.schema.json",
        "peak-midi-router.schema.json",
        "asm-hydrasynth.hardware.json",
        "korg-nanokontrol2.hardware.json",
        "novation-bs2.hardware.json",
        "clavia-nord-drum-2.hardware.json",
        "novation-peak.hardware.json",
        // synth-ui-theme schema & themes across all 5 projects
        "theme-config.schema.json",
        "asm-hydrasynth.theme.json",
        "nkeditor.theme.json",
        "bs2-td3.theme.json",
        "clavia-nord-drum-2.theme.json",
        "novation-peak.theme.json",
    ];

    for file in &expected_files {
        assert!(
            temp_dir.join(file).exists(),
            "Expected exported schema/profile file missing: {file}"
        );
    }

    assert_eq!(SynthAccent::all().len(), 8);
    assert_eq!(verify_profile_parity_with_cc_map().unwrap(), 74);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_full_workstation_workflow_gui_controller_mod_matrix_sysex_and_osc() {
    let mut app = PeakWorkstationApp::new();
    assert_eq!(
        app.engine.router.safety_mode,
        MidiSafetyMode::VirtualLoopback
    );

    // 1. Render 12 headless GUI frames across all 6 workstation tabs
    assert_eq!(app.run_headless_frames(12), 12);

    // 2. Apply a 16-slot Modulation Matrix edit and verify CommandHistory undo/redo
    let slot3 = ModSlot::new(
        3,
        ModSource::Lfo1Bipolar,
        ModSource::ModWheel,
        48,
        ModDestination::Osc1VSync,
    );
    apply_slot_with_history(&mut app.history, &mut app.active_patch, &slot3).unwrap();
    assert_eq!(read_slot_from_patch(&app.active_patch, 3), slot3);
    app.history.undo(&mut app.active_patch).unwrap();
    assert!(!read_slot_from_patch(&app.active_patch, 3).is_active());
    app.history.redo(&mut app.active_patch).unwrap();
    assert_eq!(read_slot_from_patch(&app.active_patch, 3), slot3);

    // 3. Verify 527-byte SysEx round-trip on the modified patch
    let sysex_bytes = encode_patch_to_sysex(&app.active_patch, true);
    assert_eq!(sysex_bytes.len(), 527);
    let restored = decode_patch_from_sysex(&sysex_bytes).unwrap();
    assert_eq!(restored.meta.name, app.active_patch.meta.name);

    // 4. Verify Korg nanoKONTROL2 Bank 1..3 switching and Animate Hold toggles
    let mut nk2 = NanoKontrol2PeakController::new(&app.active_patch, synth_core::SoftTakeoverMode::Catch);
    nk2.switch_bank(3, &app.active_patch);
    let now = Instant::now();
    // Toggle Animate 1 Hold via Solo 1 (CC 32) in Bank 3
    nk2.handle_surface_cc(
        32,
        127,
        &mut app.active_patch,
        &mut app.history,
        &mut app.engine,
        now,
    );
    assert!(app.engine.router.animate_1_held);

    // 5. Verify Safe-Sweep Randomizer + PatchDiff + Undo
    let before_rand = app.active_patch.clone();
    apply_safe_randomization_with_history(
        &mut app.history,
        &app.randomizer,
        &mut app.active_patch,
        85.0,
        100.0,
        Some(777),
    )
    .unwrap();
    let diff = PatchDiff::compare(&before_rand, &app.active_patch, 1e-6);
    assert!(!diff.modified.is_empty());
    assert!(app.active_patch.get_param("filter_resonance").unwrap() <= 62.0);
    app.history.undo(&mut app.active_patch).unwrap();
    assert!(PatchDiff::compare(&before_rand, &app.active_patch, 1e-6).modified.is_empty());

    // 6. Verify Bidirectional OSC Bridge + Konduktiva dry stress test
    let mut osc = PeakOscCoordinator::new_ephemeral().unwrap();
    let mut konduktiva = PeakKonduktivaCompanion::new();
    let osc_addr = osc.bridge.server().local_addr().unwrap();
    let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
    let pkt = OscPacket::Message(OscMessage {
        addr: "/synth/konduktiva/density_graph".to_string(),
        args: vec![OscType::String("peakBuild".to_string())],
    });
    sender
        .send_to(&encoder::encode(&pkt).unwrap(), osc_addr)
        .unwrap();
    std::thread::sleep(Duration::from_millis(10));
    osc.poll_and_dispatch(
        &mut app.active_patch,
        &mut app.history,
        &mut app.engine,
        &mut konduktiva,
        now,
    )
    .unwrap();
    assert_eq!(konduktiva.density_graph, PeakDensityGraph::PeakBuild);
    assert_eq!(konduktiva.players.len(), PEAK_PLAYER_NAMES.len());

    let stress = run_dry_stress_test(1);
    assert!(stress.pass);
    assert_eq!(build_peak_factory_library().len(), 16);
}
