// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * @file Clavia Nord Drum 2 SysEx Frame Codec & Checksum Engine.
 *
 * Implements byte-for-byte serialization and parsing for Clavia 0x33
 * SysEx Program dump frames (231 bytes) and Multi-Program / Bank dumps,
 * with 7-bit XOR checksum calculation and error detection.
 */

import {
  CLAVIA_MANUFACTURER_ID,
  NORD_DRUM_2_MODEL_ID,
  VOICE_COUNT,
} from './nord-drum-router.js';

export const SYSEX_MSG_TYPE_PROGRAM = 0x01;
export const SYSEX_MSG_TYPE_BANK = 0x02;

export const SYSEX_VOICE_PARAM_COUNT = 32;
export const SYSEX_VOICE_PAYLOAD_BYTES = VOICE_COUNT * SYSEX_VOICE_PARAM_COUNT; // 192 bytes
export const SYSEX_KIT_GLOBALS_BYTES = 30;
export const SYSEX_KIT_TOTAL_PAYLOAD_BYTES = SYSEX_VOICE_PAYLOAD_BYTES + SYSEX_KIT_GLOBALS_BYTES; // 222 bytes
export const SYSEX_FRAME_LENGTH = 231;
export const TOTAL_PROGRAMS_PER_BANK = 50;
export const TOTAL_BANKS = 8;

/**
 * Exact specification of all 31 per-voice Continuous Controller parameters,
 * matching crates/synth-midi-osc and src/lib.rs PER_VOICE_PARAMS.
 */
export const PER_VOICE_PARAMS_SPEC = Object.freeze([
  // Tone section (12 parameters)
  { key: 'tone_wave', cc: 46, module: 'Tone', defaultVal: 0 },
  { key: 'tone_pitch_msb', cc: 30, module: 'Tone', defaultVal: 60 },
  { key: 'tone_pitch_lsb', cc: 62, module: 'Tone', defaultVal: 64 },
  { key: 'tone_spectra', cc: 52, module: 'Tone', defaultVal: 64 },
  { key: 'tone_timbre', cc: 53, module: 'Tone', defaultVal: 64 },
  { key: 'tone_timbre_env', cc: 54, module: 'Tone', defaultVal: 64 },
  { key: 'tone_timbre_decay', cc: 49, module: 'Tone', defaultVal: 64 },
  { key: 'tone_decay', cc: 50, module: 'Tone', defaultVal: 64 },
  { key: 'tone_decay_mode', cc: 48, module: 'Tone', defaultVal: 0 },
  { key: 'tone_bend_amount', cc: 55, module: 'Tone', defaultVal: 64 },
  { key: 'tone_bend_time', cc: 56, module: 'Tone', defaultVal: 32 },
  { key: 'tone_punch', cc: 57, module: 'Tone', defaultVal: 0 },
  // Noise section (8 parameters)
  { key: 'noise_filter_freq', cc: 14, module: 'Noise', defaultVal: 64 },
  { key: 'noise_filter_type', cc: 15, module: 'Noise', defaultVal: 0 },
  { key: 'noise_filter_env', cc: 16, module: 'Noise', defaultVal: 64 },
  { key: 'noise_resonance', cc: 17, module: 'Noise', defaultVal: 0 },
  { key: 'noise_attack_rate', cc: 18, module: 'Noise', defaultVal: 0 },
  { key: 'noise_decay_mode', cc: 19, module: 'Noise', defaultVal: 0 },
  { key: 'noise_decay', cc: 21, module: 'Noise', defaultVal: 64 },
  { key: 'noise_decay_lo_dyn', cc: 20, module: 'Noise', defaultVal: 64 },
  // Click section (2 parameters)
  { key: 'click_type', cc: 58, module: 'Click', defaultVal: 0 },
  { key: 'click_level', cc: 59, module: 'Click', defaultVal: 64 },
  // Mix, EQ, Distortion, Echo, Choke Group (9 parameters)
  { key: 'mix_balance', cc: 23, module: 'Mix', defaultVal: 64 },
  { key: 'distortion_amount', cc: 24, module: 'Distortion', defaultVal: 0 },
  { key: 'distortion_type', cc: 25, module: 'Distortion', defaultVal: 0 },
  { key: 'eq_frequency', cc: 26, module: 'EQ', defaultVal: 64 },
  { key: 'eq_gain', cc: 27, module: 'EQ', defaultVal: 64 },
  { key: 'echo_amount', cc: 28, module: 'Echo', defaultVal: 0 },
  { key: 'echo_feedback', cc: 29, module: 'Echo', defaultVal: 0 },
  { key: 'echo_time_msb', cc: 60, module: 'Echo', defaultVal: 64 },
  { key: 'choke_group', cc: 63, module: 'Choke', defaultVal: 0 },
]);

/**
 * Computes 7-bit XOR checksum over a sequence of bytes.
 * Clavia SysEx uses 7-bit values (0..127) XORed together.
 *
 * @param {Uint8Array|Array<number>} bytes
 * @param {number} [start=0]
 * @param {number} [end=bytes.length]
 * @returns {number}
 */
export function compute7BitXorChecksum(bytes, start = 0, end = bytes.length) {
  let xor = 0;
  for (let i = start; i < end; i++) {
    xor ^= (bytes[i] & 0x7F);
  }
  return xor & 0x7F;
}

/**
 * Validates Clavia 0x33 SysEx program dump headers.
 * Header: `0xF0 0x33 <DeviceID: 0x00..0x0F> 0x12 ... 0xF7`
 *
 * @param {Uint8Array|Array<number>} bytes
 * @returns {boolean}
 */
export function validateClaviaSysexHeader(bytes) {
  if (!bytes || bytes.length < 7) {
    return false;
  }
  return (
    bytes[0] === 0xF0 &&
    bytes[1] === CLAVIA_MANUFACTURER_ID &&
    (bytes[2] & 0x7F) <= 0x0F &&
    bytes[3] === NORD_DRUM_2_MODEL_ID &&
    bytes[bytes.length - 1] === 0xF7
  );
}

/**
 * Packs a 6-voice kit into an official Clavia 0x33 SysEx Single Program frame (231 bytes).
 *
 * Frame structure:
 * `F0 33 <DeviceID> 12 01 <Bank: 0..7> <Program: 0..49> <222 Payload Bytes> <Checksum> F7`
 *
 * @param {Object} kit
 * @param {number} [deviceId=0]
 * @param {number} [msgType=SYSEX_MSG_TYPE_PROGRAM]
 * @returns {Uint8Array}
 */
export function packSingleProgramSysex(kit, deviceId = 0, msgType = SYSEX_MSG_TYPE_PROGRAM) {
  const frame = new Uint8Array(SYSEX_FRAME_LENGTH);

  // 1. SysEx Header (7 bytes)
  frame[0] = 0xF0;
  frame[1] = CLAVIA_MANUFACTURER_ID; // 0x33
  frame[2] = deviceId & 0x0F;
  frame[3] = NORD_DRUM_2_MODEL_ID; // 0x12
  frame[4] = msgType & 0x7F;
  frame[5] = Math.max(0, Math.min(7, Math.floor(kit.bank || 0))) & 0x7F;
  frame[6] = Math.max(0, Math.min(49, Math.floor(kit.program || 0))) & 0x7F;

  // 2. Voice Payloads (6 voices x 32 bytes = 192 bytes, offsets 7..198)
  const voices = kit.voices || [];
  for (let v = 0; v < VOICE_COUNT; v++) {
    const vOffset = 7 + (v * SYSEX_VOICE_PARAM_COUNT);
    const voiceData = voices[v] || {};
    const params = voiceData.params || {};

    for (let p = 0; p < PER_VOICE_PARAMS_SPEC.length; p++) {
      const spec = PER_VOICE_PARAMS_SPEC[p];
      const val = params[spec.key] !== undefined ? params[spec.key] : spec.defaultVal;
      frame[vOffset + p] = Math.max(0, Math.min(127, Math.floor(val))) & 0x7F;
    }
    const echoLsb = voiceData.echo_time_lsb !== undefined ? voiceData.echo_time_lsb : 64;
    frame[vOffset + 31] = Math.max(0, Math.min(127, Math.floor(echoLsb))) & 0x7F;
  }

  // 3. Kit Globals (30 bytes, offsets 199..228)
  const gOffset = 199;
  // Byte 0: Global Channel (1..16 -> 7-bit)
  frame[gOffset + 0] = Math.max(1, Math.min(16, Math.floor(kit.globalChannel || 10))) & 0x7F;

  // Bytes 1..6: Per-Voice Individual Channels (0 = Off, 1..16)
  const voiceChannels = kit.voiceChannels || [1, 2, 3, 4, 5, 6];
  for (let i = 0; i < VOICE_COUNT; i++) {
    const ch = voiceChannels[i];
    frame[gOffset + 1 + i] = (ch === null || ch === undefined || ch === 0)
      ? 0
      : Math.max(1, Math.min(16, Math.floor(ch))) & 0x7F;
  }

  // Bytes 7..12: Per-Voice Trigger Notes (0..127)
  const triggerNotes = kit.triggerNotes || [60, 62, 64, 65, 67, 69];
  for (let i = 0; i < VOICE_COUNT; i++) {
    const note = triggerNotes[i] !== undefined ? triggerNotes[i] : (60 + i * 2);
    frame[gOffset + 7 + i] = Math.max(0, Math.min(127, Math.floor(note))) & 0x7F;
  }

  // Bytes 13..28: Kit Name (16 ASCII chars space-padded)
  const nameStr = String(kit.name || 'Default Kit');
  for (let i = 0; i < 16; i++) {
    if (i < nameStr.length) {
      const code = nameStr.charCodeAt(i);
      frame[gOffset + 13 + i] = (code >= 32 && code <= 126) ? (code & 0x7F) : 0x20;
    } else {
      frame[gOffset + 13 + i] = 0x20; // ASCII space
    }
  }

  // Byte 29: Reserved flags
  frame[gOffset + 29] = 0x00;

  // 4. Checksum computation over bytes 4..228 inclusive (length 225)
  frame[229] = compute7BitXorChecksum(frame, 4, 229);

  // 5. SysEx EOX terminator
  frame[230] = 0xF7;

  return frame;
}

/**
 * Unpacks a 231-byte Clavia 0x33 SysEx Single Program frame into a Kit object.
 *
 * @param {Uint8Array|Array<number>} frame
 * @returns {Object}
 */
export function unpackSingleProgramSysex(frame) {
  if (!frame || frame.length !== SYSEX_FRAME_LENGTH) {
    throw new Error(
      `Invalid SysEx frame length: expected ${SYSEX_FRAME_LENGTH} bytes, received ${frame ? frame.length : 0}`
    );
  }

  if (!validateClaviaSysexHeader(frame)) {
    throw new Error('Invalid Clavia Nord Drum 2 SysEx header (expected F0 33 <dev> 12 ... F7)');
  }

  const msgType = frame[4] & 0x7F;
  if (msgType !== SYSEX_MSG_TYPE_PROGRAM && msgType !== SYSEX_MSG_TYPE_BANK) {
    throw new Error(`Unexpected SysEx message type 0x${msgType.toString(16)} (expected 0x01 or 0x02)`);
  }

  const bank = frame[5] & 0x7F;
  if (bank > 7) {
    throw new Error(`Bank index out of range: ${bank} (expected 0..7 for Banks A..H)`);
  }

  const program = frame[6] & 0x7F;
  if (program > 49) {
    throw new Error(`Program slot out of range: ${program} (expected 0..49 for Programs 1..50)`);
  }

  const expectedChecksum = compute7BitXorChecksum(frame, 4, 229);
  const actualChecksum = frame[229] & 0x7F;
  if (expectedChecksum !== actualChecksum) {
    throw new Error(
      `SysEx checksum mismatch: expected 0x${expectedChecksum.toString(16)}, received 0x${actualChecksum.toString(16)}`
    );
  }

  // Extract voices
  const voices = [];
  for (let v = 0; v < VOICE_COUNT; v++) {
    const vOffset = 7 + (v * SYSEX_VOICE_PARAM_COUNT);
    const params = {};
    for (let p = 0; p < PER_VOICE_PARAMS_SPEC.length; p++) {
      const spec = PER_VOICE_PARAMS_SPEC[p];
      params[spec.key] = frame[vOffset + p] & 0x7F;
    }
    const echoLsb = frame[vOffset + 31] & 0x7F;
    voices.push({
      voiceIndex: v + 1,
      params,
      echo_time_lsb: echoLsb,
    });
  }

  // Extract kit globals
  const gOffset = 199;
  const globalChannel = (frame[gOffset + 0] & 0x7F) || 10;

  const voiceChannels = [];
  for (let i = 0; i < VOICE_COUNT; i++) {
    const ch = frame[gOffset + 1 + i] & 0x7F;
    voiceChannels.push(ch === 0 ? null : ch);
  }

  const triggerNotes = [];
  for (let i = 0; i < VOICE_COUNT; i++) {
    triggerNotes.push(frame[gOffset + 7 + i] & 0x7F);
  }

  const nameChars = [];
  for (let i = 0; i < 16; i++) {
    const code = frame[gOffset + 13 + i] & 0x7F;
    if (code >= 32 && code <= 126) {
      nameChars.push(String.fromCharCode(code));
    }
  }
  const name = nameChars.join('').trim() || `Program ${String.fromCharCode(65 + bank)}:${String(program + 1).padStart(2, '0')}`;

  return {
    deviceId: frame[2] & 0x0F,
    msgType,
    bank,
    program,
    name,
    globalChannel,
    voiceChannels,
    triggerNotes,
    voices,
  };
}
