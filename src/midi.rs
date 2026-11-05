// SPDX-License-Identifier: GPL-3.0-or-later
//! Novation Peak hardware MIDI engine (`src/midi.rs`) with `CaptureStats` matching
//! `js/konduktiva-peak/src/midi.js` (`makeCaptureStub()`), anti-choke CC thinning,
//! Note-On priority scheduling, 14-bit NRPN encoding, and strict `VirtualLoopback` safety.

use crate::params::PeakMidiCcMapSpec;
use crate::sysex::{encode_patch_to_sysex, PeakPatchBank};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::time::{Duration, Instant};
use synth_core::SynthPatch;
use synth_midi_osc::{
    discover_peak_hardware, MidiCodec, MidiPacket, MidiSafetyMode, PeakDiscoveryResult,
    PeakMidiRouter, DEFAULT_CC_THROTTLE_MS,
};

/// Capture telemetry statistics matching `makeCaptureStub()` in `js/konduktiva-peak/src/midi.js`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureStats {
    /// Total Note-On messages captured (`noteon`).
    pub noteon: u64,
    /// Total Note-Off messages captured (`noteoff`).
    pub noteoff: u64,
    /// Total Control Change messages captured (`cc`).
    pub cc: u64,
    /// Total Program Change messages captured.
    pub program_change: u64,
    /// Total SysEx frames captured.
    pub sysex: u64,
    /// Total intermediate CC messages thinned by the anti-choke scheduler.
    pub ccs_thinned: u64,
    /// Packet count indexed by 0-based wire MIDI channel (`0..=15`).
    pub by_channel: BTreeMap<u8, u64>,
    /// CC packet count indexed by MIDI CC controller number (`0..=127`).
    pub by_controller: BTreeMap<u8, u64>,
}

impl CaptureStats {
    /// Records a dispatched `MidiPacket` into the capture telemetry counters.
    pub fn record_packet(&mut self, pkt: &MidiPacket) {
        match pkt {
            MidiPacket::NoteOn { channel, .. } => {
                self.noteon = self.noteon.saturating_add(1);
                *self.by_channel.entry(*channel).or_insert(0) += 1;
            }
            MidiPacket::NoteOff { channel, .. } => {
                self.noteoff = self.noteoff.saturating_add(1);
                *self.by_channel.entry(*channel).or_insert(0) += 1;
            }
            MidiPacket::ControlChange { channel, cc, .. } => {
                self.cc = self.cc.saturating_add(1);
                *self.by_channel.entry(*channel).or_insert(0) += 1;
                *self.by_controller.entry(*cc).or_insert(0) += 1;
            }
            MidiPacket::ProgramChange { channel, .. } => {
                self.program_change = self.program_change.saturating_add(1);
                *self.by_channel.entry(*channel).or_insert(0) += 1;
            }
            MidiPacket::SysEx(_) => {
                self.sysex = self.sysex.saturating_add(1);
            }
        }
    }

    /// Returns `true` if all captured channel messages occurred exclusively on the expected 0-based wire channel.
    pub fn is_single_channel_only(&self, expected_wire_channel: u8) -> bool {
        self.by_channel
            .keys()
            .all(|&ch| ch == expected_wire_channel)
    }

    /// Counts how many of the 5 core stress-test CCs (`29`, `79`, `1`, `112`, `108`) were observed.
    pub fn stress_expected_ccs_present(&self) -> usize {
        [29u8, 79, 1, 112, 108]
            .iter()
            .filter(|cc| self.by_controller.get(cc).copied().unwrap_or(0) > 0)
            .count()
    }
}

/// High-priority and rate-thinned MIDI hardware engine for the Novation Peak.
///
/// Defaults strictly to `MidiSafetyMode::VirtualLoopback` (`live: false`) so automated
/// tests and CLI verification commands never transmit unprompted MIDI to `hw:3,0,0` or `hw:0,0,0`.
#[derive(Debug, Clone)]
pub struct PeakHardwareEngine {
    pub router: PeakMidiRouter,
    pub discovery: PeakDiscoveryResult,
    pub throttle_duration: Duration,
    pub stats: CaptureStats,
    cc_spec: PeakMidiCcMapSpec,
    high_priority_queue: VecDeque<MidiPacket>,
    pending_cc: HashMap<u8, (String, u8)>,
    last_sent_cc: HashMap<u8, Instant>,
    captured_packets: Vec<MidiPacket>,
}

impl Default for PeakHardwareEngine {
    fn default() -> Self {
        Self::new(MidiSafetyMode::VirtualLoopback)
    }
}

impl PeakHardwareEngine {
    /// Creates a new Novation Peak hardware engine in the specified `MidiSafetyMode`.
    #[must_use]
    pub fn new(safety_mode: MidiSafetyMode) -> Self {
        let router = PeakMidiRouter::new().with_safety_mode(safety_mode);
        let discovery = discover_peak_hardware(safety_mode);
        Self {
            router,
            discovery,
            throttle_duration: Duration::from_millis(DEFAULT_CC_THROTTLE_MS),
            stats: CaptureStats::default(),
            cc_spec: PeakMidiCcMapSpec::load_embedded(),
            high_priority_queue: VecDeque::new(),
            pending_cc: HashMap::new(),
            last_sent_cc: HashMap::new(),
            captured_packets: Vec::new(),
        }
    }

    /// Builder method configuring the minimum anti-choke interval between identical CC numbers.
    #[must_use]
    pub fn with_throttle_duration(mut self, duration: Duration) -> Self {
        self.throttle_duration = duration;
        self
    }

    /// Returns `true` if the engine is in safe `VirtualLoopback` or `DryRun` mode (`live: false`).
    #[must_use]
    pub const fn is_safe(&self) -> bool {
        self.router.safety_mode.is_safe()
    }

    /// Queues a high-priority Note-On trigger event (never thinned, always dispatched ahead of CCs).
    pub fn send_note_on(&mut self, note: u8, velocity: u8) -> Vec<MidiPacket> {
        let pkts = self.router.route_note_on(note, velocity);
        for pkt in &pkts {
            self.high_priority_queue.push_back(pkt.clone());
        }
        pkts
    }

    /// Queues a high-priority Note-Off release event.
    pub fn send_note_off(&mut self, note: u8, velocity: u8) -> MidiPacket {
        let pkt = self.router.route_note_off(note, velocity);
        self.high_priority_queue.push_back(pkt.clone());
        pkt
    }

    /// Schedules a 7-bit MIDI CC update by CC number (`0..=127`), automatically looking up its
    /// parameter name in `midi-cc-map.json` so `SafeSweepBounds` clamping is applied accurately.
    pub fn schedule_cc(&mut self, cc: u8, value: u8, _now: Instant) {
        let param_name = self
            .cc_spec
            .resolve_cc_by_number(cc)
            .map(|e| e.param.clone())
            .unwrap_or_else(|| format!("cc_{cc}"));
        self.schedule_named_cc(cc, &param_name, value);
    }

    /// Schedules a named 7-bit MIDI CC update with anti-choke burst deduplication.
    pub fn schedule_named_cc(&mut self, cc: u8, param_name: &str, value: u8) {
        let val7 = value & 0x7F;
        if let Some((existing_name, existing_val)) = self.pending_cc.get_mut(&cc) {
            if *existing_val == val7 {
                self.stats.ccs_thinned = self.stats.ccs_thinned.saturating_add(1);
                return;
            }
            *existing_name = param_name.to_string();
            *existing_val = val7;
            self.stats.ccs_thinned = self.stats.ccs_thinned.saturating_add(1);
        } else {
            self.pending_cc
                .insert(cc, (param_name.to_string(), val7));
        }
    }

    /// Immediately encodes and queues a 4-message 14-bit NRPN parameter update (`CC 99`, `CC 98`, `CC 6`, `CC 38`).
    pub fn send_nrpn_14bit(&mut self, param_14bit: u16, value_14bit: u16) -> Vec<MidiPacket> {
        let ch = self.router.wire_channel();
        let raw = MidiCodec::encode_nrpn(ch, param_14bit, value_14bit, true);
        let mut pkts = Vec::with_capacity(4);
        for chunk in raw.chunks_exact(3) {
            let pkt = MidiPacket::ControlChange {
                channel: chunk[0] & 0x0F,
                cc: chunk[1] & 0x7F,
                value: chunk[2] & 0x7F,
            };
            self.high_priority_queue.push_back(pkt.clone());
            pkts.push(pkt);
        }
        pkts
    }

    /// Queues an Animate 1 (`index = 1`) or Animate 2 (`index = 2`) performance hold toggle.
    pub fn send_animate_hold(&mut self, index: u8, held: bool) -> Option<MidiPacket> {
        let pkt = self.router.route_animate_hold(index, held)?;
        self.high_priority_queue.push_back(pkt.clone());
        Some(pkt)
    }

    /// Queues a Bank Select (`A..D`) + Program Change (`0..=127`) sequence.
    pub fn send_bank_program(&mut self, bank: PeakPatchBank, program: u8) -> Vec<MidiPacket> {
        let pkts = self
            .router
            .route_bank_program(bank.bank_select_lsb(), program);
        for pkt in &pkts {
            self.high_priority_queue.push_back(pkt.clone());
        }
        pkts
    }

    /// Encodes a `SynthPatch` into a 527-byte Novation Peak SysEx dump and queues it for transmission.
    pub fn send_patch_sysex(&mut self, patch: &SynthPatch, edit_buffer: bool) -> MidiPacket {
        let frame = encode_patch_to_sysex(patch, edit_buffer);
        let pkt = MidiPacket::SysEx(frame);
        self.high_priority_queue.push_back(pkt.clone());
        pkt
    }

    /// Emits a full Peak panic sequence (`CC 120`, `CC 123`, and 128 Note-Offs) and flushes immediately.
    pub fn panic(&mut self, now: Instant) -> Vec<MidiPacket> {
        self.pending_cc.clear();
        let pkts = self.router.route_panic();
        for pkt in pkts {
            self.high_priority_queue.push_back(pkt);
        }
        self.flush(now)
    }

    /// Flushes all ready MIDI packets at timestamp `now`, always dispatching Note/SysEx/PC events
    /// ahead of rate-thinned continuous CC sweeps.
    pub fn flush(&mut self, now: Instant) -> Vec<MidiPacket> {
        let mut out = Vec::new();

        // 1. Drain high-priority Note, NRPN, Bank/PC, and SysEx events first
        while let Some(pkt) = self.high_priority_queue.pop_front() {
            self.stats.record_packet(&pkt);
            out.push(pkt);
        }

        // 2. Drain eligible CC updates that have satisfied the anti-choke throttle window
        let mut eligible_ccs: Vec<u8> = self
            .pending_cc
            .keys()
            .copied()
            .filter(|cc| match self.last_sent_cc.get(cc) {
                Some(&last) => now.duration_since(last) >= self.throttle_duration,
                None => true,
            })
            .collect();
        eligible_ccs.sort_unstable();

        for cc in eligible_ccs {
            if let Some((param_name, val)) = self.pending_cc.remove(&cc) {
                let pkts = self.router.route_cc(cc, &param_name, val);
                for pkt in pkts {
                    self.stats.record_packet(&pkt);
                    out.push(pkt);
                }
                self.last_sent_cc.insert(cc, now);
            }
        }

        if self.is_safe() {
            self.captured_packets.extend(out.clone());
        }

        out
    }

    /// Returns a slice of all packets captured in the virtual loopback buffer.
    #[must_use]
    pub fn captured_packets(&self) -> &[MidiPacket] {
        &self.captured_packets
    }

    /// Clears captured packets and resets `CaptureStats`.
    pub fn reset_capture(&mut self) {
        self.captured_packets.clear();
        self.stats = CaptureStats::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use synth_core::DeviceSynthesizerProfile;

    #[test]
    fn test_peak_hardware_engine_note_priority_and_cc_thinning() {
        let mut engine = PeakHardwareEngine::default()
            .with_throttle_duration(Duration::from_millis(5));
        assert!(engine.is_safe());
        assert!(engine.discovery.has_peak_usb());

        let t0 = Instant::now();
        // Burst 10 updates to Filter Cutoff (CC 29) and Filter Resonance (CC 79)
        for v in 10..20 {
            engine.schedule_cc(29, v * 5, t0);
            engine.schedule_cc(79, v * 6, t0); // v*6 reaches 114, should be clamped to 62!
        }
        // Queue Note-On AFTER CCs
        engine.send_note_on(60, 100);

        let flushed = engine.flush(t0);
        // Note-On must be first, followed by CC 29 (val 95) and CC 79 (clamped to 62)
        assert_eq!(flushed.len(), 3);
        assert_eq!(
            flushed[0],
            MidiPacket::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100
            }
        );
        assert_eq!(
            flushed[1],
            MidiPacket::ControlChange {
                channel: 0,
                cc: 29,
                value: 95
            }
        );
        assert_eq!(
            flushed[2],
            MidiPacket::ControlChange {
                channel: 0,
                cc: 79,
                value: 62
            }
        );
        assert_eq!(engine.stats.noteon, 1);
        assert_eq!(engine.stats.cc, 2);
        assert_eq!(engine.stats.ccs_thinned, 18);
        assert!(engine.stats.is_single_channel_only(0));
    }

    #[test]
    fn test_peak_hardware_engine_nrpn_bank_sysex_and_panic() {
        let mut engine = PeakHardwareEngine::default();
        let t0 = Instant::now();

        let nrpn_pkts = engine.send_nrpn_14bit(258, 8192);
        assert_eq!(nrpn_pkts.len(), 4);

        let bank_pkts = engine.send_bank_program(PeakPatchBank::BankB, 12);
        assert_eq!(bank_pkts.len(), 3);

        let profile = DeviceSynthesizerProfile::novation_peak();
        let patch = profile.create_default_patch("Loopback Test");
        let sysex_pkt = engine.send_patch_sysex(&patch, true);
        assert_eq!(sysex_pkt.to_bytes().len(), 527);

        let flushed = engine.flush(t0);
        assert_eq!(flushed.len(), 8);
        assert_eq!(engine.stats.cc, 6); // 4 NRPN CCs + 2 Bank Select CCs
        assert_eq!(engine.stats.program_change, 1);
        assert_eq!(engine.stats.sysex, 1);

        let panic_flushed = engine.panic(t0);
        assert_eq!(panic_flushed.len(), 130);
        assert_eq!(engine.stats.noteoff, 128);
    }
}
