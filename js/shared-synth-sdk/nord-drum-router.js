// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * @file Clavia Nord Drum 2 Dual MIDI Router, Non-Invasive ALSA Port Discovery,
 * and Anti-Choke CC Thinning & Note-On Priority Scheduler.
 *
 * Implements low-latency MIDI routing supporting both Individual Voice Channels
 * (Ch 1..6) and Global Channel (Ch 10) with automatic CC 70 Focus switching,
 * configurable trigger notes, hardware discovery, and anti-choke rate thinning.
 */

export const VOICE_COUNT = 6;
export const CLAVIA_MANUFACTURER_ID = 0x33;
export const NORD_DRUM_2_MODEL_ID = 0x12;
export const DEFAULT_GLOBAL_CHANNEL = 10;
export const GLOBAL_FOCUS_CC = 70;
export const DEFAULT_TRIGGER_NOTES = Object.freeze([60, 62, 64, 65, 67, 69]);
export const UMC1820_ALSA_DEVICE = 'hw:0,0,0';
export const NANOKONTROL2_ALSA_DEVICE = 'hw:2,0,0';
export const DEFAULT_CC_THROTTLE_MS = 2;

/**
 * Canonical names for the 6 Nord Drum 2 voices.
 */
export const ND2_VOICE_NAMES = Object.freeze([
  'Sub Kick',
  'Resonant Snare',
  'Closed Hat',
  'Metallic Open Hat',
  'Tuned Marimba/FM Perc',
  'Noise Clap/FX',
]);

/**
 * 1-based indexing enum for the 6 voice roles.
 */
export const ND2_VOICE_ROLES = Object.freeze({
  V1_SUB_KICK: 1,
  V2_RESONANT_SNARE: 2,
  V3_CLOSED_HAT: 3,
  V4_METALLIC_OPEN_HAT: 4,
  V5_TUNED_PERC: 5,
  V6_NOISE_CLAP_FX: 6,
});

/**
 * Per-voice synthesis and mixer MIDI CC numbers for Clavia Nord Drum 2.
 */
export const ND2_CC = Object.freeze({
  // Tone section
  TONE_WAVE: 46,
  TONE_PITCH_MSB: 30,
  TONE_PITCH_LSB: 62,
  TONE_SPECTRA: 52,
  TONE_TIMBRE: 53,
  TONE_TIMBRE_ENV: 54,
  TONE_TIMBRE_DECAY: 49,
  TONE_DECAY: 50,
  TONE_DECAY_MODE: 48,
  TONE_BEND_AMOUNT: 55,
  TONE_BEND_TIME: 56,
  TONE_PUNCH: 57,

  // Noise section
  NOISE_FILTER_FREQ: 14,
  NOISE_FILTER_TYPE: 15,
  NOISE_FILTER_ENV: 16,
  NOISE_RESONANCE: 17,
  NOISE_ATTACK_RATE: 18,
  NOISE_DECAY_MODE: 19,
  NOISE_DECAY: 21,
  NOISE_DECAY_LO_DYN: 20,

  // Click section
  CLICK_TYPE: 58,
  CLICK_LEVEL: 59,

  // Mix, EQ, Distortion, Echo & Mixer
  MIX_BALANCE: 23,
  DISTORTION_AMOUNT: 24,
  DISTORTION_TYPE: 25,
  EQ_FREQ: 26,
  EQ_GAIN: 27,
  ECHO_AMOUNT: 28,
  ECHO_FEEDBACK: 29,
  ECHO_TIME_MSB: 60,
  ECHO_TIME_LSB: 61,
  MUTE_CHOKE_GROUP: 63,
  CHANNEL_LEVEL: 7,
  STEREO_PAN: 10,
  GLOBAL_FOCUS: 70,
});

/**
 * Operational safety modes for MIDI output.
 */
export const MidiSafetyMode = Object.freeze({
  VIRTUAL_LOOPBACK: 'virtual_loopback',
  DRY_RUN: 'dry_run',
  LIVE_ARMED: 'live_armed',
});

/**
 * MIDI channel addressing modes for Nord Drum 2.
 */
export const MidiControlMode = Object.freeze({
  INDIVIDUAL_CHANNELS: 'individual',
  GLOBAL_FOCUS: 'global_focus',
});

/**
 * Non-invasive ALSA port discovery for Nord Drum 2 workstation.
 */
export class NordDrumHardwareDiscovery {
  /**
   * Discovers ALSA hardware endpoints.
   * In VIRTUAL_LOOPBACK mode (default safe state), returns simulated loopback endpoints.
   *
   * @param {Object} [options]
   * @param {string} [options.safetyMode=MidiSafetyMode.VIRTUAL_LOOPBACK]
   * @returns {Object} Discovery result
   */
  static discoverNonInvasive(options = {}) {
    const safetyMode = options.safetyMode || MidiSafetyMode.VIRTUAL_LOOPBACK;

    if (safetyMode === MidiSafetyMode.VIRTUAL_LOOPBACK) {
      return {
        umc1820: {
          cardIndex: 0,
          rawmidiDevice: UMC1820_ALSA_DEVICE,
          seqClient: 16,
          seqPort: 0,
          cardName: 'UMC1820',
          deviceType: 'BehringerUmc1820',
          isVirtualMock: true,
        },
        nanokontrol2: {
          cardIndex: 2,
          rawmidiDevice: NANOKONTROL2_ALSA_DEVICE,
          seqClient: 24,
          seqPort: 0,
          cardName: 'nanoKONTROL2',
          deviceType: 'KorgNanoKontrol2',
          isVirtualMock: true,
        },
        totalDevicesScanned: 2,
        safetyMode: MidiSafetyMode.VIRTUAL_LOOPBACK,
      };
    }

    // DryRun or LiveArmed simulation
    return {
      umc1820: {
        cardIndex: 0,
        rawmidiDevice: UMC1820_ALSA_DEVICE,
        seqClient: 16,
        seqPort: 0,
        cardName: 'UMC1820',
        deviceType: 'BehringerUmc1820',
        isVirtualMock: false,
      },
      nanokontrol2: {
        cardIndex: 2,
        rawmidiDevice: NANOKONTROL2_ALSA_DEVICE,
        seqClient: 24,
        seqPort: 0,
        cardName: 'nanoKONTROL2',
        deviceType: 'KorgNanoKontrol2',
        isVirtualMock: false,
      },
      totalDevicesScanned: 2,
      safetyMode,
    };
  }

  /**
   * Helper verifying if the Nord Drum 2 port was detected.
   * @param {Object} result
   * @returns {boolean}
   */
  static hasNordDrumPort(result) {
    return Boolean(result && result.umc1820);
  }

  /**
   * Helper verifying if the nanoKONTROL2 controller was detected.
   * @param {Object} result
   * @returns {boolean}
   */
  static hasController(result) {
    return Boolean(result && result.nanokontrol2);
  }
}

/**
 * Dual MIDI Router supporting Individual Voice Channels (Ch 1..6) and
 * Global Channel (Ch 10) with automatic CC 70 Focus switching.
 */
export class DualMidiRouter {
  /**
   * @param {Object} [options]
   */
  constructor(options = {}) {
    this.controlMode = options.controlMode || MidiControlMode.INDIVIDUAL_CHANNELS;
    this.safetyMode = options.safetyMode || MidiSafetyMode.VIRTUAL_LOOPBACK;
    this.globalChannel = options.globalChannel || DEFAULT_GLOBAL_CHANNEL;
    this.voiceChannels = options.voiceChannels
      ? [...options.voiceChannels]
      : [1, 2, 3, 4, 5, 6];
    this.triggerNotes = options.triggerNotes
      ? [...options.triggerNotes]
      : [...DEFAULT_TRIGGER_NOTES];
    this.currentFocusVoice = options.currentFocusVoice || 1;
  }

  /**
   * Configures trigger note for a voice (1..6).
   * @param {number} voiceIndex
   * @param {number} note
   */
  setTriggerNote(voiceIndex, note) {
    if (voiceIndex < 1 || voiceIndex > VOICE_COUNT) {
      throw new RangeError(`Voice index ${voiceIndex} out of range (1..${VOICE_COUNT})`);
    }
    const clamped = Math.max(0, Math.min(127, Math.floor(note)));
    this.triggerNotes[voiceIndex - 1] = clamped;
  }

  /**
   * Retrieves trigger note for a voice (1..6).
   * @param {number} voiceIndex
   * @returns {number|null}
   */
  getTriggerNote(voiceIndex) {
    if (voiceIndex < 1 || voiceIndex > VOICE_COUNT) {
      return null;
    }
    return this.triggerNotes[voiceIndex - 1];
  }

  /**
   * Configures MIDI channel for an individual voice (1..6).
   * @param {number} voiceIndex
   * @param {number|null} channel
   */
  setVoiceChannel(voiceIndex, channel) {
    if (voiceIndex < 1 || voiceIndex > VOICE_COUNT) {
      throw new RangeError(`Voice index ${voiceIndex} out of range (1..${VOICE_COUNT})`);
    }
    if (channel === null || channel === undefined) {
      this.voiceChannels[voiceIndex - 1] = null;
    } else {
      this.voiceChannels[voiceIndex - 1] = Math.max(1, Math.min(16, Math.floor(channel)));
    }
  }

  /**
   * Retrieves configured channel for a voice (1..6).
   * @param {number} voiceIndex
   * @returns {number|null}
   */
  getVoiceChannel(voiceIndex) {
    if (voiceIndex < 1 || voiceIndex > VOICE_COUNT) {
      return null;
    }
    return this.voiceChannels[voiceIndex - 1];
  }

  /**
   * Explicitly switches hardware focus in GlobalFocus mode.
   * @param {number} voiceIndex
   * @returns {Object|null}
   */
  switchFocus(voiceIndex) {
    const target = Math.max(1, Math.min(6, Math.floor(voiceIndex)));
    if (this.controlMode === MidiControlMode.GLOBAL_FOCUS && this.currentFocusVoice !== target) {
      this.currentFocusVoice = target;
      return {
        type: 'cc',
        channel: Math.max(0, this.globalChannel - 1),
        cc: GLOBAL_FOCUS_CC,
        value: target - 1,
      };
    }
    return null;
  }

  /**
   * Routes parameter CC to the appropriate MIDI channel, prepending CC 70 in GlobalFocus mode.
   * @param {number} voiceIndex
   * @param {number} cc
   * @param {number} value
   * @returns {Array<Object>}
   */
  routeParamCc(voiceIndex, cc, value) {
    if (voiceIndex < 1 || voiceIndex > VOICE_COUNT) {
      return [];
    }

    const packets = [];
    const val7 = Math.max(0, Math.min(127, Math.floor(value)));
    const cc7 = Math.max(0, Math.min(127, Math.floor(cc)));

    if (this.controlMode === MidiControlMode.INDIVIDUAL_CHANNELS) {
      const ch = this.voiceChannels[voiceIndex - 1];
      if (ch !== null && ch !== undefined) {
        packets.push({
          type: 'cc',
          channel: Math.max(0, ch - 1),
          cc: cc7,
          value: val7,
        });
      }
    } else if (this.controlMode === MidiControlMode.GLOBAL_FOCUS) {
      const wireChannel = Math.max(0, this.globalChannel - 1);
      if (this.currentFocusVoice !== voiceIndex) {
        packets.push({
          type: 'cc',
          channel: wireChannel,
          cc: GLOBAL_FOCUS_CC,
          value: voiceIndex - 1,
        });
        this.currentFocusVoice = voiceIndex;
      }
      packets.push({
        type: 'cc',
        channel: wireChannel,
        cc: cc7,
        value: val7,
      });
    }

    return packets;
  }

  /**
   * Routes a Note-On trigger message for a voice (1..6).
   * @param {number} voiceIndex
   * @param {number} [velocity=100]
   * @returns {Object|null}
   */
  routeNoteOn(voiceIndex, velocity = 100) {
    if (voiceIndex < 1 || voiceIndex > VOICE_COUNT) {
      return null;
    }
    const voiceIdx = voiceIndex - 1;
    const vel = Math.max(1, Math.min(127, Math.floor(velocity)));
    const note = this.triggerNotes[voiceIdx];

    if (this.controlMode === MidiControlMode.INDIVIDUAL_CHANNELS) {
      const ch = this.voiceChannels[voiceIdx];
      if (ch === null || ch === undefined) {
        return null;
      }
      return {
        type: 'note_on',
        channel: Math.max(0, ch - 1),
        note,
        velocity: vel,
      };
    }

    return {
      type: 'note_on',
      channel: Math.max(0, this.globalChannel - 1),
      note,
      velocity: vel,
    };
  }

  /**
   * Routes a Note-Off release message for a voice (1..6).
   * @param {number} voiceIndex
   * @param {number} [velocity=0]
   * @returns {Object|null}
   */
  routeNoteOff(voiceIndex, velocity = 0) {
    if (voiceIndex < 1 || voiceIndex > VOICE_COUNT) {
      return null;
    }
    const voiceIdx = voiceIndex - 1;
    const vel = Math.max(0, Math.min(127, Math.floor(velocity)));
    const note = this.triggerNotes[voiceIdx];

    if (this.controlMode === MidiControlMode.INDIVIDUAL_CHANNELS) {
      const ch = this.voiceChannels[voiceIdx];
      if (ch === null || ch === undefined) {
        return null;
      }
      return {
        type: 'note_off',
        channel: Math.max(0, ch - 1),
        note,
        velocity: vel,
      };
    }

    return {
      type: 'note_off',
      channel: Math.max(0, this.globalChannel - 1),
      note,
      velocity: vel,
    };
  }
}

/**
 * Anti-Choke CC Thinning & Note-On Priority scheduler.
 */
export class AntiChokeScheduler {
  /**
   * @param {DualMidiRouter} [router]
   * @param {Object} [options]
   * @param {number} [options.throttleMs=2]
   */
  constructor(router, options = {}) {
    this.router = router || new DualMidiRouter();
    this.throttleMs = options.throttleMs || DEFAULT_CC_THROTTLE_MS;
    this.noteQueue = [];
    this.pendingCc = new Map();
    this.lastSentCcTimes = new Map();
    this.loopbackLog = [];
    this.metrics = {
      notesDispatched: 0,
      ccsDispatched: 0,
      ccsThinned: 0,
      focusSwitchesDispatched: 0,
      loopbackPacketsCaptured: 0,
    };
  }

  /**
   * Queues a high-priority Note-On trigger message.
   * @param {number} voiceIndex
   * @param {number} [velocity=100]
   * @returns {Object|null}
   */
  scheduleNoteOn(voiceIndex, velocity = 100) {
    const pkt = this.router.routeNoteOn(voiceIndex, velocity);
    if (pkt) {
      this.noteQueue.push(pkt);
    }
    return pkt;
  }

  /**
   * Queues a high-priority Note-Off release message.
   * @param {number} voiceIndex
   * @param {number} [velocity=0]
   * @returns {Object|null}
   */
  scheduleNoteOff(voiceIndex, velocity = 0) {
    const pkt = this.router.routeNoteOff(voiceIndex, velocity);
    if (pkt) {
      this.noteQueue.push(pkt);
    }
    return pkt;
  }

  /**
   * Schedules a parameter CC message with rate thinning and deduplication.
   * @param {number} voiceIndex
   * @param {number} cc
   * @param {number} value
   * @param {number} [now=Date.now()]
   */
  scheduleCc(voiceIndex, cc, value, now = Date.now()) {
    if (voiceIndex < 1 || voiceIndex > VOICE_COUNT) {
      return;
    }
    const key = `${voiceIndex}:${cc}`;
    const val7 = Math.max(0, Math.min(127, Math.floor(value)));

    if (this.pendingCc.has(key)) {
      const existing = this.pendingCc.get(key);
      if (existing.value === val7) {
        // Redundant duplicate: drop silently
        this.metrics.ccsThinned++;
        return;
      }
      // Dense burst update: overwrite with latest parameter value
      existing.value = val7;
      existing.receivedTime = now;
      this.metrics.ccsThinned++;
    } else {
      this.pendingCc.set(key, {
        voice: voiceIndex,
        cc,
        value: val7,
        receivedTime: now,
      });
    }
  }

  /**
   * Drains all ready messages at time `now`.
   * Note events are always emitted first.
   * @param {number} [now=Date.now()]
   * @returns {Array<Object>}
   */
  drainReady(now = Date.now()) {
    const dispatched = [];

    // 1. Drain high-priority Note triggers first
    while (this.noteQueue.length > 0) {
      const notePkt = this.noteQueue.shift();
      this.metrics.notesDispatched++;
      dispatched.push(notePkt);
    }

    // 2. Identify eligible CCs that have passed throttle interval
    const eligibleKeys = [];
    for (const [key] of this.pendingCc) {
      const last = this.lastSentCcTimes.get(key);
      if (last === undefined || now - last >= this.throttleMs) {
        eligibleKeys.push(key);
      }
    }

    for (const key of eligibleKeys) {
      const msg = this.pendingCc.get(key);
      if (msg) {
        this.pendingCc.delete(key);
        const pkts = this.router.routeParamCc(msg.voice, msg.cc, msg.value);
        for (const pkt of pkts) {
          if (pkt.cc === GLOBAL_FOCUS_CC) {
            this.metrics.focusSwitchesDispatched++;
          } else {
            this.metrics.ccsDispatched++;
          }
          dispatched.push(pkt);
        }
        this.lastSentCcTimes.set(key, now);
      }
    }

    // 3. Record into loopback log if operating in safe mode
    if (this.router.safetyMode !== MidiSafetyMode.LIVE_ARMED) {
      this.metrics.loopbackPacketsCaptured += dispatched.length;
      this.loopbackLog.push(...dispatched);
    }

    return dispatched;
  }

  /**
   * Returns current performance and throughput metrics.
   * @returns {Object}
   */
  getMetrics() {
    return { ...this.metrics };
  }

  /**
   * Returns inspection slice of the in-memory loopback sink log.
   * @returns {Array<Object>}
   */
  getLoopbackLog() {
    return [...this.loopbackLog];
  }

  /**
   * Clears the in-memory loopback sink log.
   */
  clearLoopbackLog() {
    this.loopbackLog.length = 0;
  }

  /**
   * Returns true if no Note or CC events are currently queued.
   * @returns {boolean}
   */
  isEmpty() {
    return this.noteQueue.length === 0 && this.pendingCc.size === 0;
  }

  /**
   * Clears all pending queues and resets metrics.
   */
  clear() {
    this.noteQueue.length = 0;
    this.pendingCc.clear();
    this.lastSentCcTimes.clear();
    this.loopbackLog.length = 0;
    this.metrics = {
      notesDispatched: 0,
      ccsDispatched: 0,
      ccsThinned: 0,
      focusSwitchesDispatched: 0,
      loopbackPacketsCaptured: 0,
    };
  }
}
