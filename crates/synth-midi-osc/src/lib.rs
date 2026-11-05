// SPDX-License-Identifier: GPL-3.0-or-later
//! `synth-midi-osc`: Shared Linux USB/ALSA hardware identity fingerprinting,
//! simultaneous multi-device manager, MIDI packet codec, and bidirectional OSC router.
//!
//! Provides the shared hardware and network abstraction layer used across nkEditor,
//! ASM Hydrasynth Editor, and the Novation BS2 / Behringer TD-3 Manager.

pub mod hardware;
pub mod osc;
pub mod routing;

// Re-export hardware types and functions
pub use hardware::{
    build_deep_panic_packet, build_panic_packet, scan_hardware_devices,
    scan_hardware_devices_with_roots, stored_device_profile_json_schema,
    validate_stored_device_profile_json, DeviceFingerprint, DeviceLifecycleEvent,
    HardwareFingerprint, MidiCodec, MidiCodecError, MultiDeviceManager, NrpnMessage,
    NrpnParser, PortRateLimiter, StoredDeviceProfile, SupportedDeviceType, SysExPacket,
};

// Re-export OSC types and functions
pub use osc::{
    OscBridge, OscBroadcaster, OscConfig, OscError, OscIncomingMessage, OscServer,
};

// Re-export Nord Drum 2 and Novation Peak routing, ALSA discovery, and Anti-Choke scheduling
pub use routing::{
    discover_nord_drum_hardware, discover_nord_drum_hardware_with_roots,
    discover_peak_hardware, discover_peak_hardware_with_roots,
    dual_midi_router_json_schema, export_midi_osc_schemas, peak_midi_router_json_schema,
    validate_dual_midi_router_json, validate_peak_midi_router_json, AlsaPortEndpoint,
    AntiChokeScheduler, DualMidiRouter, MidiControlMode, MidiPacket, MidiSafetyMode,
    NordDrumDiscoveryResult, PeakDiscoveryResult, PeakMidiRouter, SchedulerMetrics,
    DEFAULT_CC_THROTTLE_MS, DEFAULT_GLOBAL_CHANNEL, DEFAULT_TRIGGER_NOTES,
    GLOBAL_FOCUS_CC, NANOKONTROL2_ALSA_DEVICE, NORD_DRUM_2_MODEL_ID,
    PEAK_ALSA_DEVICE, PEAK_DEFAULT_CHANNEL, PEAK_POLYPHONY, PEAK_SEQ_CLIENT,
    PEAK_USB_PRODUCT, PEAK_USB_VENDOR, UMC1820_ALSA_DEVICE, VOICE_COUNT,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reexported_types_accessible_at_crate_root() {
        let fp = DeviceFingerprint::new("1235", "0018", None, "Bass Station II", 0);
        assert_eq!(fp.identity_key(), "1235:0018:none:Bass Station II");
        assert_eq!(fp.device_type(), SupportedDeviceType::NovationBassStation2);

        let osc_cfg = OscConfig::default();
        assert_eq!(osc_cfg.listen_port, 9100);
        assert_eq!(osc_cfg.broadcast_port, 9101);

        let panic_buf = build_panic_packet(Some(0));
        assert!(!panic_buf.is_empty());
    }

    #[test]
    fn live_linux_hardware_fingerprint_scan() {
        // Run against live system to verify scanning of /sys/class/sound,
        // /dev/snd/by-id, /dev/snd/by-path, and /proc/asound/seq/clients.
        let devices = scan_hardware_devices();
        // The host has at least card0 (UMC1820) and card2 (nanoKONTROL2) connected.
        println!("Scanned {} live hardware devices", devices.len());
        for dev in &devices {
            println!(
                "Found: {} -> {} ({:?}) [card: {}, seq: {:?}:{:?}, usb: {:?}]",
                dev.identity_key(),
                dev.card_name,
                dev.device_type(),
                dev.alsa_card_index,
                dev.alsa_seq_client,
                dev.alsa_seq_port,
                dev.usb_path
            );
        }

        // Verify that nanoKONTROL2 (0944:0117) or UMC1820 (1397:0503) is recognized if present
        if let Some(nk2) = devices
            .iter()
            .find(|d| d.device_type == SupportedDeviceType::KorgNanoKontrol2)
        {
            assert_eq!(nk2.vendor_id, "0944");
            assert_eq!(nk2.product_id, "0117");
            assert_eq!(nk2.alsa_card_index, 2);
            assert_eq!(nk2.alsa_seq_client, Some(24));
        }

        if let Some(umc) = devices
            .iter()
            .find(|d| d.device_type == SupportedDeviceType::BehringerUmc1820)
        {
            assert_eq!(umc.vendor_id, "1397");
            assert_eq!(umc.product_id, "0503");
            assert_eq!(umc.alsa_card_index, 0);
            assert_eq!(umc.alsa_seq_client, Some(16));
        }
    }

    #[test]
    fn routing_and_discovery_reexported_and_operational() {
        let disc = discover_nord_drum_hardware(MidiSafetyMode::VirtualLoopback);
        assert!(disc.has_nord_drum_port());
        assert!(disc.has_controller());
        assert_eq!(disc.safety_mode, MidiSafetyMode::VirtualLoopback);

        let router = DualMidiRouter::new()
            .with_control_mode(MidiControlMode::GlobalFocus)
            .with_safety_mode(MidiSafetyMode::VirtualLoopback);
        let mut scheduler = AntiChokeScheduler::new(router);

        let now = std::time::Instant::now();
        scheduler.schedule_cc(1, 46, 50, now);
        scheduler.schedule_note_on(1, 100);

        let pkts = scheduler.drain_ready(now);
        assert_eq!(pkts.len(), 2);
        assert!(pkts[0].is_note());
        assert_eq!(scheduler.metrics().notes_dispatched, 1);
        assert_eq!(scheduler.metrics().ccs_dispatched, 1);
    }

    #[test]
    fn midi_osc_schemas_and_export_roundtrip() {
        let hw_schema = stored_device_profile_json_schema();
        assert_eq!(hw_schema["title"], "StoredDeviceProfile");

        let router_schema = dual_midi_router_json_schema();
        assert_eq!(router_schema["title"], "DualMidiRouter");

        let router = DualMidiRouter::new()
            .with_control_mode(MidiControlMode::GlobalFocus)
            .with_global_channel(10);
        let router_json = serde_json::to_string(&router).expect("Router JSON failed");
        let restored_router = validate_dual_midi_router_json(&router_json).expect("Validate router JSON failed");
        assert_eq!(restored_router.global_channel, 10);
        assert_eq!(restored_router.control_mode, MidiControlMode::GlobalFocus);

        let temp_dir = std::env::temp_dir().join(format!("synth_midi_osc_schema_test_{}", rosc::encoder::encode(&rosc::OscPacket::Message(rosc::OscMessage { addr: "/test".into(), args: vec![] })).unwrap().len()));
        export_midi_osc_schemas(&temp_dir).expect("Export midi osc schemas failed");

        assert!(temp_dir.join("stored-device-profile.schema.json").exists());
        assert!(temp_dir.join("dual-midi-router.schema.json").exists());
        assert!(temp_dir.join("peak-midi-router.schema.json").exists());
        assert!(temp_dir.join("asm-hydrasynth.hardware.json").exists());
        assert!(temp_dir.join("korg-nanokontrol2.hardware.json").exists());
        assert!(temp_dir.join("novation-bs2.hardware.json").exists());
        assert!(temp_dir.join("clavia-nord-drum-2.hardware.json").exists());
        assert!(temp_dir.join("novation-peak.hardware.json").exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
