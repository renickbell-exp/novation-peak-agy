'use strict';
// ---------------------------------------------------------------------------
// output.js -- PURE MIDI-output helpers (no konduktiva required).
//
// Port discovery, the Peak default port pattern, a capture stub for hardware-
// free runs, and an all-notes-off panic. The konduktiva-aware openOutput lives
// in the kit (createPeakKit) because it must touch the host's env/closeMidiOutputs.
// ---------------------------------------------------------------------------

const { resolveEasymidi } = require('./_resolve');
const easymidi = resolveEasymidi();

// Defaults: the Peak's factory receive channel is 1, and its USB port name
// matches /peak/i ("Focusrite-Novation Peak").
const PEAK_DEFAULTS = { channel: 1, portPattern: /peak/i };

/**
 * List the names of all available easymidi outputs.
 * @returns {string[]}
 */
function listMidiOutputs() {
    return easymidi.getOutputs();
}

/**
 * Resolve a Peak output port name. An explicit override wins; otherwise the
 * first output matching the pattern is returned.
 * @param {object} [opts]
 * @param {RegExp} [opts.pattern] match pattern (default /peak/i).
 * @param {string} [opts.override] exact output name; bypasses pattern matching.
 * @returns {string|null} the resolved port name, or null when none matches.
 */
function resolvePeakPort(opts) {
    const o = opts || {};
    if (o.override) return o.override;
    const pattern = o.pattern || PEAK_DEFAULTS.portPattern;
    const outs = easymidi.getOutputs();
    return outs.find((name) => pattern.test(name)) || null;
}

/**
 * Build a capture stub that mimics an easymidi.Output but only counts the
 * messages the action layer emits. It carries its own `.stats` object so a
 * runner can read results without a virtual MIDI loopback.
 * @returns {{send:Function, close:Function, stats:object}}
 */
function makeCaptureStub() {
    const stats = { noteon: 0, noteoff: 0, cc: 0, byChannel: {}, byController: {} };
    const stub = {
        stats,
        send(type, msg) {
            if (stats[type] !== undefined) stats[type]++;
            stats.byChannel[msg.channel] = (stats.byChannel[msg.channel] || 0) + 1;
            if (type === 'cc') {
                stats.byController[msg.controller] = (stats.byController[msg.controller] || 0) + 1;
            }
        },
        close() {},
    };
    return stub;
}

/**
 * Send an all-sound-off, all-notes-off, and explicit note-offs across the whole
 * range on the given output and channel. Safe to call repeatedly.
 * @param {object} output an easymidi.Output (or the capture stub).
 * @param {number} [channel] 1-based MIDI channel (default 1); sent on wire
 *   channel `channel - 1`.
 */
function panic(output, channel) {
    if (!output) return;
    const wire = (channel == null ? PEAK_DEFAULTS.channel : channel) - 1;
    try {
        output.send('cc', { controller: 120, value: 0, channel: wire }); // all sound off
        output.send('cc', { controller: 123, value: 0, channel: wire }); // all notes off
        for (let n = 0; n < 128; n++) {
            output.send('noteoff', { note: n, velocity: 0, channel: wire });
        }
    } catch (_) { /* output may already be closed */ }
}

module.exports = {
    easymidi,
    PEAK_DEFAULTS,
    listMidiOutputs,
    resolvePeakPort,
    makeCaptureStub,
    panic,
};
