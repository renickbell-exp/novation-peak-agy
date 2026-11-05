'use strict';
// ---------------------------------------------------------------------------
// sweeps.js -- konduktiva map builders for CC sweeps and rhythm presets.
//
// These need the host's QuantizedMap, so they are produced by makeSweeps(K)
// and exposed through the kit. Behaviour is identical to the reference demo.
// ---------------------------------------------------------------------------

// triangle 0..1..0 across `steps`.
const tri = (i, steps) => 1 - Math.abs((2 * i) / steps - 1);

const clamp7 = (v) => Math.max(0, Math.min(127, Math.round(v)));

/**
 * Bind the sweep/rhythm builders to a host konduktiva module.
 * @param {object} K host konduktiva (must export QuantizedMap).
 * @returns {{sweepCC:Function, dualSweepCC:Function, rhythmPresets:Function, tri:Function}}
 */
function makeSweeps(K) {
    const QuantizedMap = K.QuantizedMap;

    /**
     * controlChangeMap that sweeps one controller between lo..hi over
     * cycleBeats, in `steps` triangle-shaped values. wrapLookup(beat) returns
     * { controller, value }.
     */
    function sweepCC(controller, lo, hi, cycleBeats, steps) {
        const keys = Array.from({ length: steps }, (_, i) => (i * cycleBeats) / steps);
        const vals = keys.map((_, i) => ({
            controller,
            value: clamp7(lo + (hi - lo) * tri(i, steps)),
        }));
        return new QuantizedMap(cycleBeats, keys, vals);
    }

    /**
     * controlChangeMap that emits TWO CCs per onset (an array), each on its own
     * slow sweep, to exercise multi-CC bursts. wrapLookup(beat) returns an array
     * of { controller, value }.
     */
    function dualSweepCC(cA, loA, hiA, cB, loB, hiB, cycleBeats, steps) {
        const keys = Array.from({ length: steps }, (_, i) => (i * cycleBeats) / steps);
        const vals = keys.map((_, i) => ([
            { controller: cA, value: clamp7(loA + (hiA - loA) * tri(i, steps)) },
            { controller: cB, value: clamp7(loB + (hiB - loB) * tri((i + Math.floor(steps / 3)) % steps, steps)) },
        ]));
        return new QuantizedMap(cycleBeats, keys, vals);
    }

    // IOI rhythm presets (inter-onset intervals in beats), nested like the stock
    // maps: an outer QuantizedMap wraps the inner per-bar IOI map.
    const wrapRhythm = (inner) => new QuantizedMap(1, [1], [inner]);

    /**
     * @returns {{half:object, quarter:object, eighth:object, sixteenth:object}}
     *   four nested QuantizedMap rhythm presets.
     */
    function rhythmPresets() {
        return {
            half: wrapRhythm(new QuantizedMap(4, [0, 2], [2, 2])),
            quarter: wrapRhythm(new QuantizedMap(4, [0, 1, 2, 3], [1, 1, 1, 1])),
            eighth: wrapRhythm(new QuantizedMap(4,
                [0, 0.5, 1, 1.5, 2, 2.5, 3, 3.5],
                [0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5])),
            sixteenth: wrapRhythm(new QuantizedMap(4,
                Array.from({ length: 16 }, (_, i) => i * 0.25),
                Array.from({ length: 16 }, () => 0.25))),
        };
    }

    return { sweepCC, dualSweepCC, rhythmPresets, tri };
}

module.exports = { makeSweeps, tri };
