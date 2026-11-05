'use strict';
// ---------------------------------------------------------------------------
// osc-bridge.js -- Optional OSC telemetry and remote-control bridge for
// konduktiva-peak, forwarding note/CC/density events to the Rust PeakOscCoordinator
// or capturing them locally in dry-run mode without opening physical MIDI ports.
// ---------------------------------------------------------------------------

const { safeSweepRange } = require('./cc');

/**
 * Creates a lightweight OSC / capture bridge for `konduktiva-peak` sessions.
 * When `client` (an instance of `SharedSynthClient` from `js/shared-synth-sdk`)
 * is provided and open, forwards all Note-On, Note-Off, CC, Animate, and Density
 * Graph transitions over UDP OSC (`127.0.0.1:9100`). Always records dry-run
 * telemetry into `stats`.
 *
 * @param {object} [opts]
 * @param {object} [opts.client] optional SharedSynthClient instance
 * @param {boolean} [opts.enforceSafeSweeps=true] clamp Resonance (CC 79 <= 62) etc.
 * @returns {object}
 */
function createPeakOscBridge(opts) {
    const o = opts || {};
    const client = o.client || null;
    const enforceSafeSweeps = o.enforceSafeSweeps !== false;
    const stats = {
        noteon: 0,
        noteoff: 0,
        cc: 0,
        densityChanges: 0,
        byChannel: {},
        byController: {},
    };

    return {
        stats,
        async sendNoteOn(note, velocity = 100, channel = 1) {
            const wire = Math.max(0, channel - 1);
            stats.noteon++;
            stats.byChannel[wire] = (stats.byChannel[wire] || 0) + 1;
            if (client && client.isListening) {
                await client.sendPeakNoteOn(note, velocity);
            }
        },
        async sendNoteOff(note, velocity = 0, channel = 1) {
            const wire = Math.max(0, channel - 1);
            stats.noteoff++;
            stats.byChannel[wire] = (stats.byChannel[wire] || 0) + 1;
            if (client && client.isListening) {
                await client.sendPeakNoteOff(note, velocity);
            }
        },
        async sendCc(cc, value, paramName = '', channel = 1) {
            const wire = Math.max(0, channel - 1);
            let clamped = Math.max(0, Math.min(127, Math.round(value)));
            if (enforceSafeSweeps && (cc === 79 || String(paramName).toLowerCase().includes('resonance'))) {
                const [lo, hi] = safeSweepRange('Filter resonance');
                clamped = Math.max(lo, Math.min(hi, clamped));
            }
            stats.cc++;
            stats.byChannel[wire] = (stats.byChannel[wire] || 0) + 1;
            stats.byController[cc] = (stats.byController[cc] || 0) + 1;
            if (client && client.isListening) {
                await client.sendPeakCc(cc, clamped);
            }
            return clamped;
        },
        async setDensityGraph(graphName) {
            stats.densityChanges++;
            if (client && client.isListening) {
                await client.setPeakDensityGraph(graphName);
            }
        },
    };
}

module.exports = { createPeakOscBridge };
