'use strict';
// ---------------------------------------------------------------------------
// cc.js -- PURE CC-map helpers (no konduktiva required).
//
// The Novation Peak CC table lives in the bundled midi-cc-map.json (a research
// deliverable). These helpers parse it, resolve param names to CC numbers, and
// supply conservative sweep ranges so an unfamiliar or dangerous parameter does
// not blast at full range.
// ---------------------------------------------------------------------------

const fs = require('fs');
const path = require('path');

// Bundled CC map: package root is one level up from src/.
const DEFAULT_CC_MAP_PATH = path.join(__dirname, '..', 'midi-cc-map.json');

// Standard-MIDI fallback CCs, used only when the research JSON cannot resolve a
// musical parameter. brightness/cutoff 74, harmonic/resonance 71, mod-wheel 1,
// reverb send 91, chorus/delay send 93.
const STANDARD_FALLBACK_CC = { cutoff: 74, reso: 71, mod: 1, reverb: 91, delay: 93 };

/**
 * Read and parse a Peak CC map JSON file.
 * @param {string} [jsonPath] absolute path; defaults to the bundled map.
 * @returns {object} the parsed map: { _meta: {...}, cc_map: [...] }.
 */
function loadCcMap(jsonPath) {
    const p = jsonPath || DEFAULT_CC_MAP_PATH;
    return JSON.parse(fs.readFileSync(p, 'utf8'));
}

/**
 * Resolve a parameter NAME to a CC number, searching the curated
 * recommended_for_stress_test list first, then the full cc_map. Matching is a
 * case-insensitive substring test against each entry's `param`, the same logic
 * the reference stress demo uses.
 * @param {string} name keyword such as 'cutoff', 'resonance', 'mod wheel',
 *   'reverb', 'delay'.
 * @param {object} [ccMap] a parsed map; defaults to the bundled map.
 * @returns {number|undefined} the CC number, or undefined when not found.
 */
function resolveCc(name, ccMap) {
    const j = ccMap || loadCcMap();
    const needle = String(name || '').toLowerCase();
    const rec = (j._meta && j._meta.recommended_for_stress_test) || [];
    const full = j.cc_map || [];
    for (const list of [rec, full]) {
        for (const ent of list) {
            const nm = String(ent.param || '').toLowerCase();
            if (nm.includes(needle)) return ent.cc;
        }
    }
    return undefined;
}

/**
 * Resolve the five musical CCs used by the stress kit (cutoff, reso, mod,
 * reverb, delay), with the demo's synonym lists and standard-MIDI fallbacks.
 * @param {object} [ccMap] a parsed map; defaults to the bundled map.
 * @returns {{cutoff:number,reso:number,mod:number,reverb:number,delay:number}}
 */
function buildCcSet(ccMap) {
    const j = ccMap || safeLoad();
    const out = Object.assign({}, STANDARD_FALLBACK_CC);
    if (j) {
        const pick = (...names) => {
            for (const n of names) {
                const cc = resolveCc(n, j);
                if (cc != null) return cc;
            }
            return undefined;
        };
        const set = (key, v) => { if (v != null) out[key] = v; };
        set('cutoff', pick('cutoff', 'filter cutoff', 'frequency'));
        set('reso', pick('resonance'));
        set('mod', pick('mod wheel', 'modulation'));
        set('reverb', pick('reverb level', 'reverb'));
        set('delay', pick('delay level')); // delay LEVEL only, not time/feedback
    }
    return out;
}

/** Load the bundled map, tolerating its absence (returns null). */
function safeLoad() {
    try { return loadCcMap(); } catch (_) { return null; }
}

// RECOMMENDED_SWEEP_CCS: the curated list from the bundled map's _meta.
const RECOMMENDED_SWEEP_CCS = (function () {
    const j = safeLoad();
    return (j && j._meta && j._meta.recommended_for_stress_test) || [];
})();

/**
 * Conservative sweep range for a parameter so unfamiliar or dangerous CCs do
 * not blast. resonance and the drive/distortion stages are capped low; FX sends
 * are kept moderate; everything else defaults to the full 0..127.
 * @param {string} name parameter keyword (matched case-insensitively).
 * @returns {{lo:number, hi:number}}
 */
function safeSweepRange(name) {
    const n = String(name || '').toLowerCase();
    if (n.includes('resonance') || n.includes('reso')) return { lo: 0, hi: 62 };
    if (n.includes('distortion')) return { lo: 0, hi: 48 };
    if (n.includes('overdrive') || n.includes('drive')) return { lo: 0, hi: 50 };
    if (n.includes('feedback')) return { lo: 0, hi: 70 };
    if (n.includes('reverb') || n.includes('delay') || n.includes('chorus') || n.includes('fx')) {
        return { lo: 0, hi: 90 };
    }
    return { lo: 0, hi: 127 };
}

module.exports = {
    DEFAULT_CC_MAP_PATH,
    STANDARD_FALLBACK_CC,
    loadCcMap,
    resolveCc,
    buildCcSet,
    RECOMMENDED_SWEEP_CCS,
    safeSweepRange,
};
