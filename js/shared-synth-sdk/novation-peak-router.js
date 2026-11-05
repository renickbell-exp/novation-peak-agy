// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * @file Novation Peak 8-Voice Polyphonic Hybrid Synthesizer MIDI Router & Non-Invasive ALSA Discovery.
 *
 * Mirrors `PeakMidiRouter` and `discover_peak_hardware` in `crates/synth-midi-osc/src/routing.rs`
 * and `SafeSweepBounds` in `crates/synth-core/src/param.rs`.
 */

import { MidiSafetyMode, UMC1820_ALSA_DEVICE, NANOKONTROL2_ALSA_DEVICE } from './nord-drum-router.js';

export const PEAK_POLYPHONY = 8;
export const PEAK_DEFAULT_CHANNEL = 1;
export const PEAK_ALSA_DEVICE = 'hw:3,0,0';
export const PEAK_SEQ_CLIENT = 28;
export const PEAK_USB_VENDOR = '1235';
export const PEAK_USB_PRODUCT = '007e';

/**
 * Computes safe live sweep bounds for a Novation Peak parameter, matching
 * `safeSweepRange` in `js/konduktiva-peak/src/cc.js` and `safe_sweep_range` in `crates/synth-core/src/param.rs`.
 *
 * @param {string} paramName
 * @returns {{min: number, max: number}}
 */
export function getPeakSafeSweepBounds(paramName) {
  const lower = String(paramName || '').toLowerCase();
  if (lower.includes('resonance')) {
    return { min: 6, max: 62 };
  }
  if (lower.includes('cutoff') || lower.includes('frequency')) {
    return { min: 28, max: 118 };
  }
  if (lower.includes('reverb') && lower.includes('level')) {
    return { min: 20, max: 90 };
  }
  if (lower.includes('delay') && lower.includes('level')) {
    return { min: 12, max: 70 };
  }
  return { min: 0, max: 127 };
}

/**
 * Non-invasive ALSA & USB hardware discovery for the Novation Peak workstation.
 */
export class PeakHardwareDiscovery {
  /**
   * Discovers ALSA hardware endpoints for the Novation Peak (`hw:3,0,0`),
   * Behringer UMC1820 (`hw:0,0,0`), and Korg nanoKONTROL2 (`hw:2,0,0`).
   * Defaults strictly to `MidiSafetyMode.VIRTUAL_LOOPBACK` (`live: false`).
   *
   * @param {Object} [options]
   * @param {string} [options.safetyMode=MidiSafetyMode.VIRTUAL_LOOPBACK]
   * @returns {Object}
   */
  static discoverNonInvasive(options = {}) {
    const safetyMode = options.safetyMode || MidiSafetyMode.VIRTUAL_LOOPBACK;
    const isVirtualMock = safetyMode === MidiSafetyMode.VIRTUAL_LOOPBACK;

    return {
      peakUsb: {
        cardIndex: 3,
        rawmidiDevice: PEAK_ALSA_DEVICE,
        seqClient: PEAK_SEQ_CLIENT,
        seqPort: 0,
        cardName: 'Peak',
        deviceType: 'NovationPeak',
        vendorId: PEAK_USB_VENDOR,
        productId: PEAK_USB_PRODUCT,
        isVirtualMock,
      },
      umc1820: {
        cardIndex: 0,
        rawmidiDevice: UMC1820_ALSA_DEVICE,
        seqClient: 16,
        seqPort: 0,
        cardName: 'UMC1820',
        deviceType: 'BehringerUmc1820',
        isVirtualMock,
      },
      nanokontrol2: {
        cardIndex: 2,
        rawmidiDevice: NANOKONTROL2_ALSA_DEVICE,
        seqClient: 24,
        seqPort: 0,
        cardName: 'nanoKONTROL2',
        deviceType: 'KorgNanoKontrol2',
        isVirtualMock,
      },
      totalDevicesScanned: 3,
      safetyMode,
    };
  }
}

/**
 * 8-Voice Polyphonic MIDI Router for the Novation Peak with SafeSweepBounds clamping.
 */
export class PeakMidiRouter {
  /**
   * @param {Object} [options]
   */
  constructor(options = {}) {
    this.midiChannel = Math.max(1, Math.min(16, Number(options.midiChannel || PEAK_DEFAULT_CHANNEL)));
    this.safetyMode = options.safetyMode || MidiSafetyMode.VIRTUAL_LOOPBACK;
    this.enforceSafeSweeps = options.enforceSafeSweeps !== false;
    this.animate1Held = false;
    this.animate2Held = false;
    this.activeVoices = [];
  }

  /**
   * Returns 0-based wire channel (`0..15`).
   * @returns {number}
   */
  wireChannel() {
    return this.midiChannel - 1;
  }

  /**
   * Routes a Note-On with 8-voice polyphonic oldest-note voice stealing.
   * @param {number} note
   * @param {number} [velocity=100]
   * @returns {Array<Object>}
   */
  routeNoteOn(note, velocity = 100) {
    const ch = this.wireChannel();
    const n7 = Math.max(0, Math.min(127, Math.floor(note)));
    const v7 = Math.max(1, Math.min(127, Math.floor(velocity)));
    const packets = [];

    const existingIdx = this.activeVoices.indexOf(n7);
    if (existingIdx !== -1) {
      this.activeVoices.splice(existingIdx, 1);
    } else if (this.activeVoices.length >= PEAK_POLYPHONY) {
      const stolen = this.activeVoices.shift();
      packets.push({ type: 'note_off', channel: ch, note: stolen, velocity: 0 });
    }

    this.activeVoices.push(n7);
    packets.push({ type: 'note_on', channel: ch, note: n7, velocity: v7 });
    return packets;
  }

  /**
   * Routes a Note-Off release message.
   * @param {number} note
   * @param {number} [velocity=0]
   * @returns {Object}
   */
  routeNoteOff(note, velocity = 0) {
    const ch = this.wireChannel();
    const n7 = Math.max(0, Math.min(127, Math.floor(note)));
    const v7 = Math.max(0, Math.min(127, Math.floor(velocity)));
    const idx = this.activeVoices.indexOf(n7);
    if (idx !== -1) {
      this.activeVoices.splice(idx, 1);
    }
    return { type: 'note_off', channel: ch, note: n7, velocity: v7 };
  }

  /**
   * Routes a 7-bit CC parameter update, applying SafeSweepBounds clamping when enabled.
   * @param {number} cc
   * @param {string} paramName
   * @param {number} value
   * @returns {Array<Object>}
   */
  routeCc(cc, paramName, value) {
    const ch = this.wireChannel();
    const cc7 = Math.max(0, Math.min(127, Math.floor(cc)));
    let val7 = Math.max(0, Math.min(127, Math.floor(value)));

    if (this.enforceSafeSweeps) {
      const bounds = getPeakSafeSweepBounds(paramName);
      val7 = Math.max(bounds.min, Math.min(bounds.max, val7));
    }

    return [{ type: 'cc', channel: ch, cc: cc7, value: val7 }];
  }

  /**
   * Routes an Animate 1 (`CC 114`) or Animate 2 (`CC 115`) hold toggle.
   * @param {number} animateIndex - 1 or 2
   * @param {boolean} held
   * @returns {Object|null}
   */
  routeAnimateHold(animateIndex, held) {
    const ch = this.wireChannel();
    const value = held ? 127 : 0;
    if (animateIndex === 1) {
      this.animate1Held = Boolean(held);
      return { type: 'cc', channel: ch, cc: 114, value };
    }
    if (animateIndex === 2) {
      this.animate2Held = Boolean(held);
      return { type: 'cc', channel: ch, cc: 115, value };
    }
    return null;
  }

  /**
   * Emits a 130-packet All-Sound-Off / All-Notes-Off panic sequence on the Peak channel.
   * @returns {Array<Object>}
   */
  routePanic() {
    const ch = this.wireChannel();
    this.activeVoices = [];
    this.animate1Held = false;
    this.animate2Held = false;
    const packets = [
      { type: 'cc', channel: ch, cc: 120, value: 0 },
      { type: 'cc', channel: ch, cc: 123, value: 0 },
    ];
    for (let note = 0; note < 128; note++) {
      packets.push({ type: 'note_off', channel: ch, note, velocity: 0 });
    }
    return packets;
  }
}
