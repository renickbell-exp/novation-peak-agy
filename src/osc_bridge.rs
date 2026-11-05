// SPDX-License-Identifier: GPL-3.0-or-later
//! Bidirectional UDP Open Sound Control (OSC) coordinator (`src/osc_bridge.rs`)
//! connecting the Novation Peak workstation, `SynthPatch` / `CommandHistory`,
//! `PeakHardwareEngine`, and `PeakKonduktivaCompanion` with `js/shared-synth-sdk`
//! and `js/konduktiva-peak`.

use crate::konduktiva::{PeakDensityGraph, PeakKonduktivaCompanion};
use crate::midi::PeakHardwareEngine;
use crate::params::resolve_cc;
use crate::sysex::PeakPatchBank;
use rosc::{OscMessage, OscType};
use std::time::Instant;
use synth_core::{CommandHistory, ParamId, SetParamCommand, SynthPatch};
use synth_midi_osc::{OscBridge, OscConfig, OscError, OscIncomingMessage};

/// Bidirectional OSC bridge coordinator for the Novation Peak manager and Konduktiva companion.
#[derive(Debug)]
pub struct PeakOscCoordinator {
    pub bridge: OscBridge,
    pub messages_processed: u64,
}

impl PeakOscCoordinator {
    /// Binds a new `PeakOscCoordinator` using the given `OscConfig` (`9100` listen, `9101` broadcast by default).
    pub fn new(config: &OscConfig) -> Result<Self, OscError> {
        let bridge = OscBridge::new(config)?;
        Ok(Self {
            bridge,
            messages_processed: 0,
        })
    }

    /// Binds an ephemeral `PeakOscCoordinator` on `127.0.0.1:0` for automated tests.
    pub fn new_ephemeral() -> Result<Self, OscError> {
        let bridge = OscBridge::new_ephemeral()?;
        Ok(Self {
            bridge,
            messages_processed: 0,
        })
    }

    /// Polls all pending UDP OSC packets and dispatches them against the Peak patch,
    /// undo/redo history, hardware MIDI engine, and Konduktiva companion.
    pub fn poll_and_dispatch(
        &mut self,
        patch: &mut SynthPatch,
        history: &mut CommandHistory,
        engine: &mut PeakHardwareEngine,
        konduktiva: &mut PeakKonduktivaCompanion,
        now: Instant,
    ) -> Result<Vec<OscIncomingMessage>, OscError> {
        let incoming = self.bridge.poll()?;
        for msg in &incoming {
            self.handle_message(msg, patch, history, engine, konduktiva, now)?;
        }
        Ok(incoming)
    }

    /// Handles a single decoded `OscIncomingMessage`.
    pub fn handle_message(
        &mut self,
        msg: &OscIncomingMessage,
        patch: &mut SynthPatch,
        history: &mut CommandHistory,
        engine: &mut PeakHardwareEngine,
        konduktiva: &mut PeakKonduktivaCompanion,
        now: Instant,
    ) -> Result<(), OscError> {
        self.messages_processed = self.messages_processed.saturating_add(1);
        let addr = msg.address.as_str();

        // 1. Emergency panic: /synth/peak/panic or /synth/panic
        if msg.is_panic() {
            engine.panic(now);
            let _ = self.bridge.broadcaster().broadcast_panic(Some("peak"));
            return Ok(());
        }

        // 2. Named parameter update: /synth/peak/param/<param_id> <float>
        if let Some(param_name) = addr.strip_prefix("/synth/peak/param/") {
            if let Some(val) = msg.param_float() {
                let param_id = ParamId::from_name(param_name);
                let key_str = param_id.as_str().to_string();
                let old_val = patch.get_param(&key_str).unwrap_or(0.0);
                let cmd = SetParamCommand::new(key_str, old_val, f64::from(val));
                let _ = history.push_and_apply(Box::new(cmd), patch);

                if let Some(cc) = resolve_cc(param_name) {
                    let cc_val = val.round().clamp(0.0, 127.0) as u8;
                    engine.schedule_cc(cc, cc_val, now);
                    engine.flush(now);
                }
                let _ = self
                    .bridge
                    .broadcaster()
                    .broadcast_param("peak", param_name, val);
            }
            return Ok(());
        }

        // 3. Direct CC message: /synth/peak/cc <cc:int> <value:int>
        if addr == "/synth/peak/cc" {
            if let (Some(cc), Some(val)) = (extract_i32(&msg.args, 0), extract_i32(&msg.args, 1)) {
                let cc_u8 = cc.clamp(0, 127) as u8;
                let val_u8 = val.clamp(0, 127) as u8;
                engine.schedule_cc(cc_u8, val_u8, now);
                engine.flush(now);
                let _ = self.bridge.broadcaster().broadcast_message(OscMessage {
                    addr: "/synth/peak/cc".to_string(),
                    args: vec![OscType::Int(i32::from(cc_u8)), OscType::Int(i32::from(val_u8))],
                });
            }
            return Ok(());
        }

        // 4. 14-bit NRPN message: /synth/peak/nrpn <param_14bit:int> <value_14bit:int>
        if addr == "/synth/peak/nrpn" {
            if let (Some(param_nrpn), Some(val_nrpn)) =
                (extract_i32(&msg.args, 0), extract_i32(&msg.args, 1))
            {
                let p14 = param_nrpn.clamp(0, 16383) as u16;
                let v14 = val_nrpn.clamp(0, 16383) as u16;
                engine.send_nrpn_14bit(p14, v14);
                engine.flush(now);
                let _ = self.bridge.broadcaster().broadcast_message(OscMessage {
                    addr: "/synth/peak/nrpn".to_string(),
                    args: vec![OscType::Int(i32::from(p14)), OscType::Int(i32::from(v14))],
                });
            }
            return Ok(());
        }

        // 5. Note-On & Note-Off: /synth/peak/note/on and /synth/peak/note/off
        if addr == "/synth/peak/note/on" {
            if let (Some(note), Some(vel)) = (extract_i32(&msg.args, 0), extract_i32(&msg.args, 1))
            {
                engine.send_note_on(note.clamp(0, 127) as u8, vel.clamp(1, 127) as u8);
                engine.flush(now);
            }
            return Ok(());
        }
        if addr == "/synth/peak/note/off" {
            if let Some(note) = extract_i32(&msg.args, 0) {
                let vel = extract_i32(&msg.args, 1).unwrap_or(0);
                engine.send_note_off(note.clamp(0, 127) as u8, vel.clamp(0, 127) as u8);
                engine.flush(now);
            }
            return Ok(());
        }

        // 6. Animate 1 / Animate 2 hold: /synth/peak/animate <index:1|2> <held:0|1>
        if addr == "/synth/peak/animate" {
            if let (Some(idx), Some(held)) = (extract_i32(&msg.args, 0), extract_i32(&msg.args, 1))
            {
                engine.send_animate_hold(idx as u8, held != 0);
                engine.flush(now);
            }
            return Ok(());
        }

        // 7. Bank + Program Change load: /synth/peak/patch/load <bank:string> <slot:int>
        if addr == "/synth/peak/patch/load" {
            let bank_str = extract_string(&msg.args, 0).unwrap_or_else(|| "A".to_string());
            let slot = extract_i32(&msg.args, 1).unwrap_or(0).clamp(0, 127) as u8;
            let bank = match bank_str.trim().to_ascii_uppercase().as_str() {
                "B" => PeakPatchBank::BankB,
                "C" => PeakPatchBank::BankC,
                "D" => PeakPatchBank::BankD,
                _ => PeakPatchBank::BankA,
            };
            engine.send_bank_program(bank, slot);
            engine.flush(now);
            return Ok(());
        }

        // 8. Konduktiva density graph switch: /synth/konduktiva/density_graph <name:string>
        if addr == "/synth/konduktiva/density_graph" {
            if let Some(graph_name) = extract_string(&msg.args, 0) {
                if let Some(graph) = PeakDensityGraph::from_name(&graph_name) {
                    konduktiva.set_density_graph(graph);
                    let _ = self.bridge.broadcaster().broadcast_message(OscMessage {
                        addr: "/synth/konduktiva/density_graph".to_string(),
                        args: vec![OscType::String(graph.as_str().to_string())],
                    });
                }
            }
            return Ok(());
        }

        // 9. Konduktiva player rate: /synth/konduktiva/player_rate <player:string> <rate:float>
        if addr == "/synth/konduktiva/player_rate" {
            if let (Some(player), Some(rate)) =
                (extract_string(&msg.args, 0), extract_f32(&msg.args, 1))
            {
                if konduktiva.set_player_rate(&player, rate) {
                    let _ = self.bridge.broadcaster().broadcast_message(OscMessage {
                        addr: "/synth/konduktiva/player_rate".to_string(),
                        args: vec![OscType::String(player), OscType::Float(rate)],
                    });
                }
            }
            return Ok(());
        }

        // 10. Konduktiva player state: /synth/konduktiva/player_state <player:string> <playing:int|bool>
        if addr == "/synth/konduktiva/player_state" {
            if let Some(player) = extract_string(&msg.args, 0) {
                let playing = match msg.args.get(1) {
                    Some(OscType::Int(v)) => *v != 0,
                    Some(OscType::Bool(b)) => *b,
                    Some(OscType::Float(f)) => *f != 0.0,
                    _ => false,
                };
                konduktiva.set_players_playing(&[player.as_str()], playing);
            }
            return Ok(());
        }

        Ok(())
    }
}

fn extract_i32(args: &[OscType], index: usize) -> Option<i32> {
    match args.get(index) {
        Some(OscType::Int(i)) => Some(*i),
        Some(OscType::Long(l)) => Some(*l as i32),
        Some(OscType::Float(f)) => Some(f.round() as i32),
        Some(OscType::Bool(b)) => Some(i32::from(*b)),
        _ => None,
    }
}

fn extract_f32(args: &[OscType], index: usize) -> Option<f32> {
    match args.get(index) {
        Some(OscType::Float(f)) => Some(*f),
        Some(OscType::Double(d)) => Some(*d as f32),
        Some(OscType::Int(i)) => Some(*i as f32),
        _ => None,
    }
}

fn extract_string(args: &[OscType], index: usize) -> Option<String> {
    match args.get(index) {
        Some(OscType::String(s)) => Some(s.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rosc::{decoder, encoder, OscPacket};
    use std::net::UdpSocket;
    use std::time::Duration;
    use synth_core::DeviceSynthesizerProfile;

    #[test]
    fn test_peak_osc_coordinator_end_to_end_udp_loopback() {
        let mut coord = PeakOscCoordinator::new_ephemeral().expect("Bind ephemeral OSC bridge");
        let server_addr = coord.bridge.server().local_addr().unwrap();

        let client_sock = UdpSocket::bind("127.0.0.1:0").unwrap();
        client_sock.set_nonblocking(true).unwrap();
        let client_port = client_sock.local_addr().unwrap().port();

        let profile = DeviceSynthesizerProfile::novation_peak();
        let mut patch = profile.create_default_patch("OSC Test Patch");
        let mut history = CommandHistory::new(32);
        let mut engine = PeakHardwareEngine::default().with_throttle_duration(Duration::ZERO);
        let mut konduktiva = PeakKonduktivaCompanion::new();
        let now = Instant::now();

        let send_osc = |addr: &str, args: Vec<OscType>| {
            let pkt = OscPacket::Message(OscMessage {
                addr: addr.to_string(),
                args,
            });
            let bytes = encoder::encode(&pkt).unwrap();
            client_sock.send_to(&bytes, server_addr).unwrap();
        };

        // 1. Subscribe client, set param, send CC (resonance 120 -> clamped to 62), send NRPN, switch density graph
        send_osc("/subscribe", vec![OscType::Int(i32::from(client_port))]);
        send_osc(
            "/synth/peak/param/filter_cutoff_frequency",
            vec![OscType::Float(92.0)],
        );
        send_osc("/synth/peak/cc", vec![OscType::Int(79), OscType::Int(120)]);
        send_osc(
            "/synth/peak/nrpn",
            vec![OscType::Int(258), OscType::Int(8192)],
        );
        send_osc(
            "/synth/peak/note/on",
            vec![OscType::Int(60), OscType::Int(100)],
        );
        send_osc(
            "/synth/peak/animate",
            vec![OscType::Int(1), OscType::Int(1)],
        );
        send_osc(
            "/synth/konduktiva/density_graph",
            vec![OscType::String("peakBuild".to_string())],
        );
        send_osc(
            "/synth/konduktiva/player_rate",
            vec![OscType::String("arp".to_string()), OscType::Float(6.0)],
        );

        std::thread::sleep(Duration::from_millis(10));
        let received = coord
            .poll_and_dispatch(&mut patch, &mut history, &mut engine, &mut konduktiva, now)
            .unwrap();
        assert_eq!(received.len(), 8);
        assert_eq!(coord.bridge.broadcaster().subscriber_count(), 1);
        assert_eq!(
            patch.get_param("filter_cutoff_frequency"),
            Some(92.0)
        );
        assert!(history.can_undo());
        assert_eq!(konduktiva.density_graph, PeakDensityGraph::PeakBuild);
        assert_eq!(konduktiva.player("arp").unwrap().current_rate, 6.0);
        assert!(engine.router.animate_1_held);
        assert_eq!(engine.stats.noteon, 1);

        // Verify broadcast datagram arrived at client socket
        let mut buf = [0u8; 2048];
        let (len, _) = client_sock.recv_from(&mut buf).expect("Receive broadcast");
        let (_, pkt) = decoder::decode_udp(&buf[..len]).unwrap();
        assert!(matches!(pkt, OscPacket::Message(_)));
    }
}
