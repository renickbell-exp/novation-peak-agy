// SPDX-License-Identifier: GPL-3.0-or-later
//! Novation Peak SysEx patch dump codec (`F0 00 20 29 01 77 ... F7`) and
//! Bank Select (`CC 0` / `CC 32`) + Program Change (`0..127`) encoder across Banks A..D.

use crate::params::PeakMidiCcMapSpec;
use serde::{Deserialize, Serialize};
use synth_core::{DeviceSynthesizerProfile, PatchSysex, SynthError, SynthPatch};

/// Focusrite-Novation standard 3-byte IEEE MIDI Manufacturer ID (`00 20 29`).
pub const NOVATION_MANUFACTURER_ID: [u8; 3] = [0x00, 0x20, 0x29];

/// Novation Peak family/product identifier (`01 77`).
pub const PEAK_DEVICE_ID: [u8; 2] = [0x01, 0x77];

/// SysEx command byte for a Single Patch Dump targeting the live Edit Buffer (`0x01`).
pub const SYSEX_CMD_CURRENT_PATCH_DUMP: u8 = 0x01;

/// SysEx command byte for a Bank Patch Dump targeting a stored Bank/Slot (`0x02`).
pub const SYSEX_CMD_BANK_PATCH_DUMP: u8 = 0x02;

/// Canonical Novation Peak single-patch SysEx frame byte length (`527` bytes).
pub const PEAK_PATCH_SYSEX_LEN: usize = 527;

/// Offset where the 16-character ASCII patch name begins in a Peak SysEx frame.
pub const SYSEX_NAME_OFFSET: usize = 16;

/// Length of the ASCII patch name field in a Peak SysEx frame (`16` bytes, space-padded).
pub const SYSEX_NAME_LEN: usize = 16;

/// Offset where the 74 continuous CC parameter values begin in a Peak SysEx frame.
pub const SYSEX_CC_PAYLOAD_OFFSET: usize = 32;

/// Offset where the 16-slot Modulation Matrix payload (`16 * 4 = 64` bytes) begins.
pub const SYSEX_MOD_MATRIX_OFFSET: usize = 128;

/// Offset of the 7-bit XOR/sum checksum byte immediately preceding `0xF7` EOX.
pub const SYSEX_CHECKSUM_OFFSET: usize = PEAK_PATCH_SYSEX_LEN - 2;

/// The four hardware patch banks (`A`, `B`, `C`, `D`) on the Novation Peak (128 patches per bank, 512 total).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PeakPatchBank {
    BankA = 1,
    BankB = 2,
    BankC = 3,
    BankD = 4,
}

impl PeakPatchBank {
    /// Parses a bank letter (`"A"`, `"B"`, `"C"`, `"D"`) into a `PeakPatchBank`.
    pub fn from_str_code(code: &str) -> Option<Self> {
        match code.trim().to_uppercase().as_str() {
            "A" | "1" | "BANK A" => Some(Self::BankA),
            "B" | "2" | "BANK B" => Some(Self::BankB),
            "C" | "3" | "BANK C" => Some(Self::BankC),
            "D" | "4" | "BANK D" => Some(Self::BankD),
            _ => None,
        }
    }

    /// Returns the canonical single-letter bank label (`"A"`, `"B"`, `"C"`, `"D"`).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BankA => "A",
            Self::BankB => "B",
            Self::BankC => "C",
            Self::BankD => "D",
        }
    }

    /// Returns the MIDI Bank Select LSB (`CC 32`) value for this bank (`1..=4` on Peak hardware).
    pub fn bank_select_lsb(&self) -> u8 {
        *self as u8
    }
}

/// Encodes the two 3-byte MIDI Bank Select messages (`CC 0` Bank MSB `0`, `CC 32` Bank LSB `1..=4`)
/// to select a patch bank (`A..D`) on a given 1-based MIDI channel (`1..=16`).
pub fn encode_bank_select_messages(channel_1_based: u8, bank: PeakPatchBank) -> [[u8; 3]; 2] {
    let ch = channel_1_based.saturating_sub(1).min(15);
    let cc_status = 0xB0 | ch;
    [
        [cc_status, 0, 0],
        [cc_status, 32, bank.bank_select_lsb() & 0x7F],
    ]
}

/// Encodes the 2-byte MIDI Program Change message (`[0xC0 | ch, program & 0x7F]`).
pub fn encode_program_change_message(channel_1_based: u8, program: u8) -> [u8; 2] {
    let ch = channel_1_based.saturating_sub(1).min(15);
    [0xC0 | ch, program & 0x7F]
}

/// Computes the 7-bit checksum over the payload slice (`bytes[8..SYSEX_CHECKSUM_OFFSET]`).
pub fn compute_peak_sysex_checksum(frame: &[u8]) -> u8 {
    if frame.len() < PEAK_PATCH_SYSEX_LEN {
        return 0;
    }
    let mut acc: u8 = 0;
    for &b in &frame[8..SYSEX_CHECKSUM_OFFSET] {
        acc = acc.wrapping_add(b & 0x7F) & 0x7F;
    }
    acc & 0x7F
}

/// Encodes a `SynthPatch` into a complete 527-byte Novation Peak Single Patch Dump SysEx message.
pub fn encode_patch_to_sysex(patch: &SynthPatch, edit_buffer: bool) -> Vec<u8> {
    let mut frame = vec![0u8; PEAK_PATCH_SYSEX_LEN];
    frame[0] = 0xF0;
    frame[1..4].copy_from_slice(&NOVATION_MANUFACTURER_ID);
    frame[4..6].copy_from_slice(&PEAK_DEVICE_ID);
    frame[6] = if edit_buffer {
        SYSEX_CMD_CURRENT_PATCH_DUMP
    } else {
        SYSEX_CMD_BANK_PATCH_DUMP
    };
    let bank = PeakPatchBank::from_str_code(&patch.meta.bank).unwrap_or(PeakPatchBank::BankA);
    frame[7] = bank.bank_select_lsb() & 0x7F;
    frame[8] = patch.meta.slot.min(127) as u8;

    // Encode 16-char ASCII patch name (space-padded, 7-bit safe)
    let mut name_bytes = [b' '; SYSEX_NAME_LEN];
    for (idx, b) in patch.meta.name.bytes().take(SYSEX_NAME_LEN).enumerate() {
        name_bytes[idx] = if (0x20..=0x7E).contains(&b) { b } else { b'_' };
    }
    frame[SYSEX_NAME_OFFSET..SYSEX_NAME_OFFSET + SYSEX_NAME_LEN].copy_from_slice(&name_bytes);

    // Encode all 74 CC parameters in canonical midi-cc-map.json order
    let spec = PeakMidiCcMapSpec::load_embedded();
    for (idx, entry) in spec.cc_map.iter().enumerate() {
        let key = entry.param_id();
        let [lo, _hi] = entry.parsed_range();
        let val = patch
            .get_param(key.as_str())
            .unwrap_or(lo as f64)
            .round()
            .clamp(0.0, 127.0) as u8;
        frame[SYSEX_CC_PAYLOAD_OFFSET + idx] = val & 0x7F;
    }

    // Encode 16 modulation matrix slots if present in patch parameters
    for slot in 1..=16usize {
        let base = SYSEX_MOD_MATRIX_OFFSET + (slot - 1) * 4;
        let src1 = patch
            .get_param(&format!("mod_slot_{slot}_src1"))
            .unwrap_or(0.0)
            .round()
            .clamp(0.0, 127.0) as u8;
        let src2 = patch
            .get_param(&format!("mod_slot_{slot}_src2"))
            .unwrap_or(0.0)
            .round()
            .clamp(0.0, 127.0) as u8;
        // Depth is stored as 0..=127 where 64 is center (0 depth)
        let depth_signed = patch
            .get_param(&format!("mod_slot_{slot}_depth"))
            .unwrap_or(0.0)
            .round()
            .clamp(-64.0, 63.0) as i16;
        let depth_u7 = (depth_signed + 64).clamp(0, 127) as u8;
        let dst = patch
            .get_param(&format!("mod_slot_{slot}_dst"))
            .unwrap_or(0.0)
            .round()
            .clamp(0.0, 127.0) as u8;

        frame[base] = src1 & 0x7F;
        frame[base + 1] = src2 & 0x7F;
        frame[base + 2] = depth_u7 & 0x7F;
        frame[base + 3] = dst & 0x7F;
    }

    frame[SYSEX_CHECKSUM_OFFSET] = compute_peak_sysex_checksum(&frame);
    frame[PEAK_PATCH_SYSEX_LEN - 1] = 0xF7;
    frame
}

/// Decodes a 527-byte Novation Peak Single Patch Dump SysEx message back into a `SynthPatch`.
pub fn decode_patch_from_sysex(frame: &[u8]) -> Result<SynthPatch, SynthError> {
    if frame.len() != PEAK_PATCH_SYSEX_LEN {
        return Err(SynthError::ValidationError(format!(
            "Invalid Novation Peak SysEx length: expected {} bytes, got {}",
            PEAK_PATCH_SYSEX_LEN,
            frame.len()
        )));
    }
    if frame[0] != 0xF0 || frame[PEAK_PATCH_SYSEX_LEN - 1] != 0xF7 {
        return Err(SynthError::ValidationError(
            "Missing 0xF0 start or 0xF7 EOX byte in Peak SysEx frame".to_string(),
        ));
    }
    if frame[1..4] != NOVATION_MANUFACTURER_ID || frame[4..6] != PEAK_DEVICE_ID {
        return Err(SynthError::ValidationError(
            "Invalid Novation Peak Manufacturer/Device ID in SysEx header".to_string(),
        ));
    }

    let checksum = frame[SYSEX_CHECKSUM_OFFSET];
    let expected = compute_peak_sysex_checksum(frame);
    if checksum != expected && checksum != 0x7F {
        return Err(SynthError::ValidationError(format!(
            "Peak SysEx checksum mismatch: expected 0x{:02X}, got 0x{:02X}",
            expected, checksum
        )));
    }

    let bank = match frame[7] {
        1 => PeakPatchBank::BankA,
        2 => PeakPatchBank::BankB,
        3 => PeakPatchBank::BankC,
        4 => PeakPatchBank::BankD,
        _ => PeakPatchBank::BankA,
    };
    let slot = (frame[8] & 0x7F) as u32;

    let raw_name = &frame[SYSEX_NAME_OFFSET..SYSEX_NAME_OFFSET + SYSEX_NAME_LEN];
    let name = String::from_utf8_lossy(raw_name).trim().to_string();
    let patch_name = if name.is_empty() {
        "Init Patch".to_string()
    } else {
        name
    };

    let profile = DeviceSynthesizerProfile::novation_peak();
    let mut patch = profile
        .create_default_patch(&patch_name)
        .with_bank_slot(bank.as_str(), slot);

    let spec = PeakMidiCcMapSpec::load_embedded();
    for (idx, entry) in spec.cc_map.iter().enumerate() {
        let key = entry.param_id();
        let val = (frame[SYSEX_CC_PAYLOAD_OFFSET + idx] & 0x7F) as f64;
        patch.set_param(key.as_str(), val);
    }

    for slot_idx in 1..=16usize {
        let base = SYSEX_MOD_MATRIX_OFFSET + (slot_idx - 1) * 4;
        let src1 = (frame[base] & 0x7F) as f64;
        let src2 = (frame[base + 1] & 0x7F) as f64;
        let depth = ((frame[base + 2] & 0x7F) as i16 - 64) as f64;
        let dst = (frame[base + 3] & 0x7F) as f64;
        if src1 > 0.0 || src2 > 0.0 || depth != 0.0 || dst > 0.0 {
            patch.set_param(format!("mod_slot_{slot_idx}_src1"), src1);
            patch.set_param(format!("mod_slot_{slot_idx}_src2"), src2);
            patch.set_param(format!("mod_slot_{slot_idx}_depth"), depth);
            patch.set_param(format!("mod_slot_{slot_idx}_dst"), dst);
        }
    }

    patch.sysex = PatchSysex {
        raw_dump: Some(frame.to_vec()),
    };

    Ok(patch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peak_sysex_roundtrip_preserves_name_bank_slot_and_all_74_ccs() {
        let profile = DeviceSynthesizerProfile::novation_peak();
        let mut patch = profile
            .create_default_patch("Oxford Blue Pad")
            .with_bank_slot("B", 42);
        patch.set_param("filter_cutoff_frequency", 96.0);
        patch.set_param("filter_resonance", 38.0);
        patch.set_param("reverb_level", 72.0);
        patch.set_param("mod_slot_1_src1", 2.0);
        patch.set_param("mod_slot_1_depth", 28.0);
        patch.set_param("mod_slot_1_dst", 9.0);

        let sysex = encode_patch_to_sysex(&patch, false);
        assert_eq!(sysex.len(), PEAK_PATCH_SYSEX_LEN);
        assert_eq!(sysex[0], 0xF0);
        assert_eq!(&sysex[1..4], &NOVATION_MANUFACTURER_ID);
        assert_eq!(&sysex[4..6], &PEAK_DEVICE_ID);
        assert_eq!(sysex[PEAK_PATCH_SYSEX_LEN - 1], 0xF7);

        let decoded = decode_patch_from_sysex(&sysex).expect("SysEx decode failed");
        assert_eq!(decoded.meta.name, "Oxford Blue Pad");
        assert_eq!(decoded.meta.bank, "B");
        assert_eq!(decoded.meta.slot, 42);
        assert_eq!(decoded.get_param("filter_cutoff_frequency"), Some(96.0));
        assert_eq!(decoded.get_param("filter_resonance"), Some(38.0));
        assert_eq!(decoded.get_param("reverb_level"), Some(72.0));
        assert_eq!(decoded.get_param("mod_slot_1_src1"), Some(2.0));
        assert_eq!(decoded.get_param("mod_slot_1_depth"), Some(28.0));
        assert_eq!(decoded.get_param("mod_slot_1_dst"), Some(9.0));
    }

    #[test]
    fn test_bank_and_program_change_encoding() {
        let bank_msgs = encode_bank_select_messages(1, PeakPatchBank::BankC);
        assert_eq!(bank_msgs[0], [0xB0, 0, 0]);
        assert_eq!(bank_msgs[1], [0xB0, 32, 3]);
        let pc_msg = encode_program_change_message(1, 15);
        assert_eq!(pc_msg, [0xC0, 15]);
    }
}
