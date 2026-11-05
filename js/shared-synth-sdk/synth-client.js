// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * @file SharedSynthClient: Universal zero-dependency Node.js client for UDP Open Sound Control
 * communication with synthesizer managers, hardware hosts, and live-coding environments.
 * Usable across nkEditor, ASM Hydrasynth Editor, and bs2-td3-manager.
 */

import dgram from 'node:dgram';
import { EventEmitter } from 'node:events';
import {
  encodeMessage,
  decodePacket,
  oscFloat,
  oscInt,
  oscString,
  oscBool,
} from './osc-codec.js';

/**
 * Creates default 16-step TD-3 sequencer pattern.
 * @returns {Array<{pitch: number, octave: string, accent: boolean, slide: boolean, gate: string}>}
 */
function createDefaultTd3Steps() {
  return Array.from({ length: 16 }, (_, i) => ({
    step: i,
    pitch: 0,
    octave: 'Normal',
    accent: false,
    slide: false,
    gate: 'Trigger',
  }));
}

/**
 * Universal Shared Synth Client.
 */
export class SharedSynthClient extends EventEmitter {
  /**
   * @param {object} [options={}]
   * @param {string} [options.targetHost='127.0.0.1'] - IP address of the synth manager hub.
   * @param {number} [options.targetPort=9100] - Inbound OSC port of the synth manager hub.
   * @param {number} [options.localPort=0] - Local UDP listen port (0 for ephemeral, 9101 for dedicated broadcast listener).
   * @param {boolean} [options.autoSubscribe=true] - Whether to send /subscribe upon opening socket.
   * @param {number} [options.subscribeIntervalMs=0] - Periodic re-subscription interval in ms (0 = disabled).
   */
  constructor(options = {}) {
    super();

    this.targetHost = options.targetHost || '127.0.0.1';
    this.targetPort = options.targetPort ?? 9100;
    this.localPort = options.localPort ?? 0;
    this.autoSubscribe = options.autoSubscribe !== false;
    this.subscribeIntervalMs = options.subscribeIntervalMs || 0;

    /** @type {dgram.Socket|null} */
    this.socket = null;
    this.isListening = false;
    this._subscribeTimer = null;

    /**
     * Cached live state of the connected synthesizer manager.
     */
    this.state = {
      activeTab: 'bs2',
      theme: 'Default',
      background: 'Dark Grey',
      bs2Patch: 'Init Patch',
      td3Pattern: 'GroupI/SectionA/Pat1',
      isPlaying: false,
      bpm: 120.0,
      bar: 1,
      beat: 1,
      step: 0,
      devices: [],
      bs2Params: {},
      td3Params: {},
      peakParams: {},
      td3Steps: createDefaultTd3Steps(),
    };
  }

  /**
   * Binds local UDP socket and starts listening for OSC broadcasts and direct replies.
   * @returns {Promise<number>} Resolves with the bound local port number.
   */
  async open() {
    if (this.socket) {
      return this.localPort;
    }

    return new Promise((resolve, reject) => {
      const socket = dgram.createSocket({ type: 'udp4', reuseAddr: true });
      this.socket = socket;

      socket.on('error', (err) => {
        this.emit('error', err);
      });

      socket.on('message', (msg, rinfo) => {
        this._onUdpMessage(msg, rinfo);
      });

      socket.on('listening', () => {
        const addr = socket.address();
        this.localPort = addr.port;
        this.isListening = true;

        if (this.autoSubscribe) {
          this.subscribe(this.localPort).catch((err) => this.emit('error', err));
          if (this.subscribeIntervalMs > 0) {
            this._subscribeTimer = setInterval(() => {
              if (this.isListening) {
                this.subscribe(this.localPort).catch(() => {});
              }
            }, this.subscribeIntervalMs);
          }
        }

        this.emit('open', { port: this.localPort, address: addr.address });
        resolve(this.localPort);
      });

      socket.on('close', () => {
        this.isListening = false;
        this.socket = null;
        if (this._subscribeTimer) {
          clearInterval(this._subscribeTimer);
          this._subscribeTimer = null;
        }
        this.emit('close');
      });

      socket.bind(this.localPort);
    });
  }

  /**
   * Closes local UDP socket and deregisters subscription from hub.
   * @returns {Promise<void>}
   */
  async close() {
    if (!this.socket) {
      return;
    }

    if (this._subscribeTimer) {
      clearInterval(this._subscribeTimer);
      this._subscribeTimer = null;
    }

    try {
      await this.unsubscribe();
    } catch {
      // Ignore unsubscribe transmission failure on teardown
    }

    return new Promise((resolve) => {
      if (!this.socket) {
        resolve();
        return;
      }
      this.socket.close(() => {
        this.socket = null;
        this.isListening = false;
        resolve();
      });
    });
  }

  /**
   * Sends raw OSC Message to target manager hub.
   * @param {string} address - OSC address pattern.
   * @param {Array<*>} [args=[]] - OSC arguments.
   * @returns {Promise<number>} Number of bytes transmitted.
   */
  async send(address, args = []) {
    if (!this.socket) {
      throw new Error('SharedSynthClient socket is not open. Call open() first.');
    }

    const buffer = encodeMessage(address, args);
    return new Promise((resolve, reject) => {
      this.socket.send(buffer, 0, buffer.length, this.targetPort, this.targetHost, (err, bytes) => {
        if (err) {
          reject(err);
        } else {
          resolve(bytes);
        }
      });
    });
  }

  // ---------------------------------------------------------------------------
  // Subscription Lifecycle
  // ---------------------------------------------------------------------------

  /**
   * Subscribes to live GUI telemetry broadcasts from the synth manager.
   * @param {number} [port=this.localPort] - Port where broadcaster should send updates.
   * @returns {Promise<number>}
   */
  async subscribe(port = this.localPort) {
    return this.send('/subscribe', port > 0 ? [oscInt(port)] : []);
  }

  /**
   * Unsubscribes from live GUI telemetry broadcasts.
   * @returns {Promise<number>}
   */
  async unsubscribe() {
    return this.send('/unsubscribe', []);
  }

  // ---------------------------------------------------------------------------
  // Global Manager & Surface Control
  // ---------------------------------------------------------------------------

  /**
   * Requests full status dump from synthesizer manager.
   * Hub replies with /synth/status to sender and broadcaster.
   * @returns {Promise<number>}
   */
  async queryStatus() {
    return this.send('/synth/status', []);
  }

  /**
   * Requests list of connected physical hardware MIDI devices and surfaces.
   * @returns {Promise<number>}
   */
  async listDevices() {
    return this.send('/synth/device/list', []);
  }

  /**
   * Selects active synthesizer visual tab and nanoKONTROL2 surface bank.
   * @param {string|number} device - 'bs2', 'td3', 'settings', or index (0, 1).
   * @returns {Promise<number>}
   */
  async selectDevice(device) {
    if (typeof device === 'number') {
      return this.send('/synth/device/select', [oscInt(device)]);
    }
    return this.send('/synth/device/select', [oscString(String(device))]);
  }

  /**
   * Switches visual theme and background palette in the GUI.
   * @param {string} theme - Theme preference name (e.g. 'Nord', 'Dracula', 'GruvboxDark', 'Default').
   * @param {string} [background] - Optional TwoBowls background (e.g. 'Dark Grey', 'Black', 'Cream', 'White').
   * @returns {Promise<number>}
   */
  async setTheme(theme, background) {
    const args = [oscString(theme)];
    if (background) {
      args.push(oscString(background));
    }
    return this.send('/synth/theme/set', args);
  }

  /**
   * Sends emergency All-Notes-Off panic command to all synthesizers and stops transport.
   * @returns {Promise<number>}
   */
  async panic() {
    return this.send('/synth/panic', []);
  }

  // ---------------------------------------------------------------------------
  // Novation Bass Station II (BS2) Control
  // ---------------------------------------------------------------------------

  /**
   * Sets a BS2 parameter by CC name or number.
   * @param {string|number} paramOrCc - Parameter name or MIDI CC number.
   * @param {number} value - Value (normalized 0.0..1.0 or raw 0..127 / 0..16383 for cutoff).
   * @returns {Promise<number>}
   */
  async setBs2Param(paramOrCc, value) {
    return this.send(`/synth/bs2/param/${paramOrCc}`, [oscFloat(value)]);
  }

  /**
   * Sends an NRPN parameter change directly to the Bass Station II.
   * @param {number} msb - NRPN MSB (0..127).
   * @param {number} lsb - NRPN LSB (0..127).
   * @param {number} value - Parameter value (0..16383).
   * @returns {Promise<number>}
   */
  async setBs2Nrpn(msb, lsb, value) {
    return this.send('/synth/bs2/nrpn', [oscInt(msb), oscInt(lsb), oscInt(value)]);
  }

  /**
   * Loads a factory or user patch slot into the BS2 engine.
   * @param {number} slot - Patch slot index (0..127).
   * @returns {Promise<number>}
   */
  async loadBs2Patch(slot) {
    return this.send('/synth/bs2/patch/load', [oscInt(slot)]);
  }

  /**
   * Saves current BS2 patch to a memory slot with a custom name.
   * @param {number} slot - Memory slot index (0..127).
   * @param {string} name - Preset display name.
   * @returns {Promise<number>}
   */
  async saveBs2Patch(slot, name) {
    return this.send('/synth/bs2/patch/save', [oscInt(slot), oscString(name)]);
  }

  /**
   * Selects an AFX Mode key overlay bank.
   * @param {number} bankIndex - Overlay bank index (0..7, mapped to banks 1..8).
   * @returns {Promise<number>}
   */
  async setBs2AfxBank(bankIndex) {
    return this.send('/synth/bs2/afx/bank', [oscInt(bankIndex)]);
  }

  /**
   * Overrides an AFX Mode parameter on an individual key.
   * @param {number} note - MIDI key note (36..60).
   * @param {string} param - Parameter key name.
   * @param {number} value - Value.
   * @returns {Promise<number>}
   */
  async setBs2AfxKey(note, param, value) {
    return this.send(`/synth/bs2/afx/key/${note}`, [oscString(param), oscFloat(value)]);
  }

  /**
   * Triggers a musical note on the Bass Station II.
   * If note is within AFX range (36..60), automatically activates the AFX overlay.
   * @param {number} note - MIDI pitch (0..127).
   * @param {number} [velocity=100] - Note velocity (0 = note off, 1..127 = note on).
   * @param {number} [durationMs=200] - Note duration in milliseconds.
   * @returns {Promise<number>}
   */
  async sendBs2Note(note, velocity = 100, durationMs = 200) {
    return this.send('/synth/bs2/note', [
      oscInt(note),
      oscInt(velocity),
      oscInt(durationMs),
    ]);
  }

  // ---------------------------------------------------------------------------
  // Behringer TD-3-MO / TD-3-MD Control
  // ---------------------------------------------------------------------------

  /**
   * Sets a TD-3 parameter by name or CC.
   * @param {string|number} paramOrCc - Parameter name or MIDI CC number.
   * @param {number} value - Value (normalized 0.0..1.0 or raw 0..127).
   * @returns {Promise<number>}
   */
  async setTd3Param(paramOrCc, value) {
    return this.send(`/synth/td3/param/${paramOrCc}`, [oscFloat(value)]);
  }

  /**
   * Loads a pattern slot into the TD-3 sequencer engine.
   * @param {string|number} group - Pattern Group ('GroupI'..'GroupIV' or 0..3).
   * @param {string|number} section - Pattern Section ('A' / 'B' or 0 / 1).
   * @param {number} index - Pattern index (0..7).
   * @returns {Promise<number>}
   */
  async loadTd3Pattern(group, section, index) {
    const g = typeof group === 'number' ? oscInt(group) : oscString(String(group));
    const s = typeof section === 'number' ? oscInt(section) : oscString(String(section));
    return this.send('/synth/td3/pattern/load', [g, s, oscInt(index)]);
  }

  /**
   * Sets the musical state of an individual step in the 16-step TD-3 acid pattern.
   * @param {number} step - Step index (0..15).
   * @param {number} pitch - Note pitch within octave (0..11, C..B).
   * @param {string|number} octave - Octave shift ('Down' / -1, 'Normal' / 0, 'Up' / 1).
   * @param {boolean} accent - Whether accent is active.
   * @param {boolean} slide - Whether slide portamento is active.
   * @param {string|number} gate - Gate state ('Trigger' / 1, 'Tie' / 2, 'Rest' / 0).
   * @returns {Promise<number>}
   */
  async setTd3Step(step, pitch, octave, accent, slide, gate) {
    const oct = typeof octave === 'number' ? (octave === -1 ? 'Down' : octave === 1 ? 'Up' : 'Normal') : String(octave);
    const gt = typeof gate === 'number' ? (gate === 0 ? 'Rest' : gate === 2 ? 'Tie' : 'Trigger') : String(gate);
    return this.send('/synth/td3/pattern/step', [
      oscInt(step),
      oscInt(pitch),
      oscString(oct),
      oscBool(accent),
      oscBool(slide),
      oscString(gt),
    ]);
  }

  /**
   * Generates an algorithmic 303 acid pattern on the TD-3.
   * @param {string} style - Pattern style ('ClassicPhuture303', 'SquelchAcidTd3Md', 'Polymetric7_9_11', 'DetroitSubElectro', 'HypnoticMinimal').
   * @param {number} [density=0.75] - Note density (0.1..1.0).
   * @param {number} [accentProb=0.35] - Accent probability (0.0..1.0).
   * @param {number} [slideProb=0.25] - Slide probability (0.0..1.0).
   * @returns {Promise<number>}
   */
  async generateTd3Pattern(style, density = 0.75, accentProb = 0.35, slideProb = 0.25) {
    return this.send('/synth/td3/pattern/generate', [
      oscString(style),
      oscFloat(density),
      oscFloat(accentProb),
      oscFloat(slideProb),
    ]);
  }

  /**
   * Controls sequencer transport on the TD-3.
   * @param {'play'|'stop'|'step'} command - Transport command.
   * @returns {Promise<number>}
   */
  async setTd3Transport(command) {
    return this.send('/synth/td3/transport', [oscString(command)]);
  }

  // ---------------------------------------------------------------------------
  // Konduktiva Live-Coding Synchronization
  // ---------------------------------------------------------------------------

  /**
   * Queries live Konduktiva clock and transport status from the manager.
   * @returns {Promise<number>}
   */
  async queryKonduktivaState() {
    return this.send('/synth/konduktiva/state', []);
  }

  /**
   * Transmits a beat clock pulse to the manager, advancing visual step indicators
   * and queuing hardware MIDI Timing Clock bytes (0xF8).
   * @param {number} bar - Measure count (e.g. 1..N).
   * @param {number} beat - Beat within bar (1..4).
   * @param {number} step - 16th-note step index (0..15).
   * @param {number} bpm - Current tempo in beats per minute.
   * @returns {Promise<number>}
   */
  async sendKonduktivaBeat(bar, beat, step, bpm) {
    return this.send('/synth/konduktiva/beat', [
      oscInt(bar),
      oscInt(beat),
      oscInt(step),
      oscFloat(bpm),
    ]);
  }

  /**
   * Schedules a live-coding musical event targeting the Novation Bass Station II.
   * @param {'note'|'param'} eventType - Type of event.
   * @param {...*} eventArgs - Arguments for event (e.g. note, vel, dur OR param, val).
   * @returns {Promise<number>}
   */
  async sendKonduktivaBs2Event(eventType, ...eventArgs) {
    if (eventType === 'note') {
      const [note, vel = 100, dur = 200] = eventArgs;
      return this.send('/synth/konduktiva/bs2/event', [
        oscString('note'),
        oscInt(note),
        oscInt(vel),
        oscInt(dur),
      ]);
    }
    if (eventType === 'param') {
      const [param, val] = eventArgs;
      return this.send('/synth/konduktiva/bs2/event', [
        oscString('param'),
        oscString(param),
        oscFloat(val),
      ]);
    }
    throw new Error(`Unsupported BS2 event type: ${eventType}`);
  }

  /**
   * Schedules a live-coding musical event targeting the Behringer TD-3-MO.
   * @param {'note'|'param'|'step'} eventType - Type of event.
   * @param {...*} eventArgs - Arguments for event.
   * @returns {Promise<number>}
   */
  async sendKonduktivaTd3Event(eventType, ...eventArgs) {
    if (eventType === 'note') {
      const [note, vel = 100] = eventArgs;
      return this.send('/synth/konduktiva/td3/event', [
        oscString('note'),
        oscInt(note),
        oscInt(vel),
      ]);
    }
    if (eventType === 'param') {
      const [param, val] = eventArgs;
      return this.send('/synth/konduktiva/td3/event', [
        oscString('param'),
        oscString(param),
        oscFloat(val),
      ]);
    }
    if (eventType === 'step') {
      const [step, pitch, octave, accent, slide, gate] = eventArgs;
      return this.send('/synth/konduktiva/td3/event', [
        oscString('step'),
        oscInt(step),
        oscInt(pitch),
        oscString(octave),
        oscBool(accent),
        oscBool(slide),
        oscString(gate),
      ]);
    }
    throw new Error(`Unsupported TD-3 event type: ${eventType}`);
  }

  // ---------------------------------------------------------------------------
  // Konduktiva Cockpit Remote Control Methods (Epoch 14)
  // ---------------------------------------------------------------------------

  /**
   * Dispatches a scene launch command to switch active performance scene.
   * @param {number|string} scene - Scene index (0..7) or string identifier.
   * @returns {Promise<number>}
   */
  async sendKonduktivaScene(scene) {
    const arg = typeof scene === 'number' ? oscInt(scene) : oscString(String(scene));
    return this.send('/synth/konduktiva/scene', [arg]);
  }

  /**
   * Dispatches a tempo nudge command (+/-1 or +/-5 BPM).
   * @param {number} deltaBpm - Delta in BPM.
   * @returns {Promise<number>}
   */
  async sendKonduktivaTempoNudge(deltaBpm) {
    return this.send('/synth/konduktiva/tempo/nudge', [oscFloat(deltaBpm)]);
  }

  /**
   * Sets Konduktiva tempo to an absolute BPM value.
   * @param {number} bpm - Target tempo.
   * @returns {Promise<number>}
   */
  async sendKonduktivaTempo(bpm) {
    return this.send('/synth/konduktiva/tempo', [oscFloat(bpm)]);
  }

  /**
   * Sets voice lane mute state.
   * @param {'bs2'|'td3'} lane - Voice lane.
   * @param {boolean} muted - Whether lane is muted.
   * @returns {Promise<number>}
   */
  async sendKonduktivaLaneMute(lane, muted) {
    return this.send('/synth/konduktiva/lane/mute', [oscString(lane), oscBool(muted)]);
  }

  /**
   * Sets voice lane solo state.
   * @param {'bs2'|'td3'} lane - Voice lane.
   * @param {boolean} solo - Whether lane is soloed.
   * @returns {Promise<number>}
   */
  async sendKonduktivaLaneSolo(lane, solo) {
    return this.send('/synth/konduktiva/lane/solo', [oscString(lane), oscBool(solo)]);
  }

  /**
   * Toggles pattern lock on a voice lane.
   * @param {'bs2'|'td3'} lane - Voice lane.
   * @param {boolean} locked - Whether lane pattern is locked.
   * @returns {Promise<number>}
   */
  async sendKonduktivaPatternLock(lane, locked) {
    return this.send('/synth/konduktiva/pattern/lock', [oscString(lane), oscBool(locked)]);
  }

  /**
   * Sets voice lane trigger density multiplier (0.0..1.0).
   * @param {'bs2'|'td3'} lane - Voice lane.
   * @param {number} density - Density multiplier.
   * @returns {Promise<number>}
   */
  async sendKonduktivaDensity(lane, density) {
    return this.send('/synth/konduktiva/lane/density', [oscString(lane), oscFloat(density)]);
  }

  // ---------------------------------------------------------------------------
  // Novation Peak 8-Voice Polyphonic Hybrid Synthesizer Control
  // ---------------------------------------------------------------------------

  /**
   * Sets a Novation Peak parameter by normalized ID or CC name.
   * @param {string} paramName
   * @param {number} value
   * @returns {Promise<number>}
   */
  async setPeakParam(paramName, value) {
    return this.send(`/synth/peak/param/${paramName}`, [oscFloat(value)]);
  }

  /**
   * Sends a 7-bit MIDI CC message to the Novation Peak.
   * @param {number} cc
   * @param {number} value
   * @returns {Promise<number>}
   */
  async sendPeakCc(cc, value) {
    return this.send('/synth/peak/cc', [oscInt(cc), oscInt(value)]);
  }

  /**
   * Sends a 14-bit NRPN message to the Novation Peak.
   * @param {number} param14Bit
   * @param {number} value14Bit
   * @returns {Promise<number>}
   */
  async sendPeakNrpn(param14Bit, value14Bit) {
    return this.send('/synth/peak/nrpn', [oscInt(param14Bit), oscInt(value14Bit)]);
  }

  /**
   * Triggers a polyphonic Note-On on the Novation Peak.
   * @param {number} note
   * @param {number} [velocity=100]
   * @returns {Promise<number>}
   */
  async sendPeakNoteOn(note, velocity = 100) {
    return this.send('/synth/peak/note/on', [oscInt(note), oscInt(velocity)]);
  }

  /**
   * Releases a polyphonic Note-Off on the Novation Peak.
   * @param {number} note
   * @param {number} [velocity=0]
   * @returns {Promise<number>}
   */
  async sendPeakNoteOff(note, velocity = 0) {
    return this.send('/synth/peak/note/off', [oscInt(note), oscInt(velocity)]);
  }

  /**
   * Toggles Animate 1 (index=1, CC 114) or Animate 2 (index=2, CC 115) hold on the Novation Peak.
   * @param {number} animateIndex
   * @param {boolean} held
   * @returns {Promise<number>}
   */
  async setPeakAnimate(animateIndex, held) {
    return this.send('/synth/peak/animate', [oscInt(animateIndex), oscInt(held ? 1 : 0)]);
  }

  /**
   * Loads a patch slot across Banks A..D on the Novation Peak.
   * @param {'A'|'B'|'C'|'D'} bank
   * @param {number} slot
   * @returns {Promise<number>}
   */
  async loadPeakPatch(bank, slot) {
    return this.send('/synth/peak/patch/load', [oscString(String(bank)), oscInt(slot)]);
  }

  /**
   * Switches the active Konduktiva density graph ('peakHold' or 'peakBuild').
   * @param {'peakHold'|'peakBuild'} graphName
   * @returns {Promise<number>}
   */
  async setPeakDensityGraph(graphName) {
    return this.send('/synth/konduktiva/density_graph', [oscString(String(graphName))]);
  }

  /**
   * Updates a Peak Konduktiva player's onset rate on the peakGrid ladder.
   * @param {string} playerName
   * @param {number} rate
   * @returns {Promise<number>}
   */
  async setPeakPlayerRate(playerName, rate) {
    return this.send('/synth/konduktiva/player_rate', [oscString(String(playerName)), oscFloat(rate)]);
  }

  /**
   * Starts or stops a Peak Konduktiva player.
   * @param {string} playerName
   * @param {boolean} playing
   * @returns {Promise<number>}
   */
  async setPeakPlayerState(playerName, playing) {
    return this.send('/synth/konduktiva/player_state', [oscString(String(playerName)), oscInt(playing ? 1 : 0)]);
  }

  // ---------------------------------------------------------------------------
  // Inbound Packet Parsing & Event Dispatch
  // ---------------------------------------------------------------------------

  /**
   * Handles incoming binary packet from UDP socket.
   * @param {Buffer} buffer
   * @param {dgram.RemoteInfo} rinfo
   * @private
   */
  _onUdpMessage(buffer, rinfo) {
    try {
      const packet = decodePacket(buffer);
      this._dispatchPacket(packet, rinfo);
    } catch (err) {
      this.emit('decodeError', { error: err, buffer, rinfo });
    }
  }

  /**
   * Dispatches decoded OSC packet (message or bundle).
   * @param {object} packet
   * @param {dgram.RemoteInfo} rinfo
   * @private
   */
  _dispatchPacket(packet, rinfo) {
    if (packet.isBundle) {
      for (const element of packet.elements) {
        this._dispatchPacket(element, rinfo);
      }
      return;
    }

    const { address, args } = packet;
    this.emit('message', { address, args, rinfo });

    // 1. /synth/status
    if (address === '/synth/status') {
      if (args.length >= 6) {
        this.state.activeTab = String(args[0]);
        this.state.theme = String(args[1]);
        this.state.bs2Patch = String(args[2]);
        this.state.td3Pattern = String(args[3]);
        this.state.isPlaying = Boolean(args[4]);
        this.state.bpm = Number(args[5]);
      }
      this.emit('status', { ...this.state });
      return;
    }

    // 2. /synth/device/list
    if (address === '/synth/device/list') {
      const count = Number(args[0] || 0);
      const devStr = String(args[1] || '');
      const devices = devStr.split(',').filter(Boolean);
      this.state.devices = devices;
      this.emit('deviceList', { count, devices });
      return;
    }

    // 3. /synth/device/select
    if (address === '/synth/device/select') {
      const activeTab = String(args[0] || 'bs2');
      this.state.activeTab = activeTab;
      this.emit('deviceSelect', activeTab);
      return;
    }

    // 4. /synth/theme/set
    if (address === '/synth/theme/set') {
      const theme = String(args[0] || this.state.theme);
      const background = String(args[1] || this.state.background);
      this.state.theme = theme;
      this.state.background = background;
      this.emit('themeChange', { theme, background });
      return;
    }

    // 5. /synth/bs2/param/<name> or /synth/td3/param/<name>
    if (address.startsWith('/synth/bs2/param/')) {
      const paramName = address.slice('/synth/bs2/param/'.length);
      const value = Number(args[0] || 0.0);
      this.state.bs2Params[paramName] = value;
      this.emit('param', { device: 'bs2', param: paramName, value });
      return;
    }

    if (address.startsWith('/synth/td3/param/')) {
      const paramName = address.slice('/synth/td3/param/'.length);
      const value = Number(args[0] || 0.0);
      this.state.td3Params[paramName] = value;
      this.emit('param', { device: 'td3', param: paramName, value });
      return;
    }

    // Fallback broadcast format /synth/<device>/param/<name>
    if (address.startsWith('/synth/') && address.includes('/param/')) {
      const parts = address.split('/').filter(Boolean);
      if (parts.length >= 4 && parts[0] === 'synth' && parts[2] === 'param') {
        const device = parts[1];
        const paramName = parts[3];
        const value = Number(args[0] || 0.0);
        if (device === 'bs2') {
          this.state.bs2Params[paramName] = value;
        } else if (device === 'td3') {
          this.state.td3Params[paramName] = value;
        } else if (device === 'peak') {
          this.state.peakParams[paramName] = value;
        }
        this.emit('param', { device, param: paramName, value });
        return;
      }
    }

    // 6. /synth/td3/pattern/step
    if (address === '/synth/td3/pattern/step') {
      const stepIdx = Number(args[0] || 0);
      const stepData = {
        step: stepIdx,
        pitch: Number(args[1] || 0),
        octave: String(args[2] || 'Normal'),
        accent: Boolean(args[3]),
        slide: Boolean(args[4]),
        gate: String(args[5] || 'Trigger'),
      };
      if (stepIdx >= 0 && stepIdx < 16) {
        this.state.td3Steps[stepIdx] = stepData;
      }
      this.emit('step', stepData);
      return;
    }

    // 7. /synth/td3/transport
    if (address === '/synth/td3/transport') {
      const command = String(args[0] || '');
      if (command === 'play') {
        this.state.isPlaying = true;
      } else if (command === 'stop') {
        this.state.isPlaying = false;
      } else if (command === 'step') {
        this.state.step = Number(args[1] ?? ((this.state.step + 1) % 16));
      }
      this.emit('transport', { command, isPlaying: this.state.isPlaying, step: this.state.step });
      return;
    }

    // 8. /synth/konduktiva/state
    if (address === '/synth/konduktiva/state') {
      const bar = Number(args[0] || 1);
      const beat = Number(args[1] || 1);
      const step = Number(args[2] || 0);
      const bpm = Number(args[3] || 120.0);
      const isPlaying = Boolean(args[4]);
      this.state.bar = bar;
      this.state.beat = beat;
      this.state.step = step;
      this.state.bpm = bpm;
      this.state.isPlaying = isPlaying;
      this.emit('konduktivaState', { bar, beat, step, bpm, isPlaying });
      return;
    }

    // 9. /synth/konduktiva/beat
    if (address === '/synth/konduktiva/beat') {
      const bar = Number(args[0] || 1);
      const beat = Number(args[1] || 1);
      const step = Number(args[2] || 0);
      const bpm = Number(args[3] || 120.0);
      this.state.bar = bar;
      this.state.beat = beat;
      this.state.step = step;
      this.state.bpm = bpm;
      this.emit('beat', { bar, beat, step, bpm });
      return;
    }

    // 10. /synth/konduktiva/scene
    if (address === '/synth/konduktiva/scene') {
      const scene = args[0] !== undefined ? args[0] : 0;
      const sceneId = args[1] !== undefined ? String(args[1]) : '';
      this.emit('sceneLaunch', { scene, sceneId });
      return;
    }

    // 11. /synth/konduktiva/tempo/nudge
    if (address === '/synth/konduktiva/tempo/nudge') {
      const delta = Number(args[0] || 0.0);
      this.emit('tempoNudge', { delta });
      return;
    }

    // 12. /synth/konduktiva/tempo
    if (address === '/synth/konduktiva/tempo') {
      const bpm = Number(args[0] || 125.0);
      this.state.bpm = bpm;
      this.emit('tempo', { bpm });
      return;
    }

    // 13. /synth/konduktiva/lane/mute
    if (address === '/synth/konduktiva/lane/mute') {
      const lane = String(args[0] || 'bs2');
      const muted = Boolean(args[1]);
      this.emit('laneMute', { lane, muted });
      return;
    }

    // 14. /synth/konduktiva/lane/solo
    if (address === '/synth/konduktiva/lane/solo') {
      const lane = String(args[0] || 'bs2');
      const solo = Boolean(args[1]);
      this.emit('laneSolo', { lane, solo });
      return;
    }

    // 15. /synth/konduktiva/pattern/lock
    if (address === '/synth/konduktiva/pattern/lock') {
      const lane = String(args[0] || 'td3');
      const locked = Boolean(args[1]);
      this.emit('patternLock', { lane, locked });
      return;
    }

    // 16. /synth/konduktiva/lane/density
    if (address === '/synth/konduktiva/lane/density') {
      const lane = String(args[0] || 'td3');
      const density = Number(args[1] || 0.75);
      this.emit('laneDensity', { lane, density });
      return;
    }

    // 17. /synth/panic
    if (address.endsWith('/panic')) {
      this.state.isPlaying = false;
      this.emit('panic');
      return;
    }
  }
}
