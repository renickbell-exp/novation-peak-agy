'use strict';
// ---------------------------------------------------------------------------
// konduktiva-peak -- reusable Novation Peak MIDI driver for Konduktiva.
//
// Two layers:
//   1. PURE helpers (CC data, port resolution, panic) that need no konduktiva.
//   2. A KIT bound to a host konduktiva via createPeakKit(K). Different projects
//      bring their own konduktiva build; this module never hard-requires one.
//
// Quick start (host project):
//   const peak = require('konduktiva-peak');
//   const K = require('konduktiva');               // YOUR konduktiva
//   const kit = peak.createPeakKit(K);
//   await kit.buildAndRun({ live: true, durationS: 120 });
//
// Hardware-free check:
//   await kit.buildAndRun({ captureStats: true, durationS: 8, exitOnDone: false });
// ---------------------------------------------------------------------------

const cc = require('./cc');
const output = require('./output');
const { makeSweeps } = require('./sweeps');
const playersFactory = require('./players');
const { makeStress } = require('./stress-test');

/**
 * Build a kit bound to a host konduktiva module. Everything that constructs
 * konduktiva maps or players lives here so the package itself stays konduktiva-
 * agnostic.
 * @param {object} K host konduktiva, exporting at least QuantizedMap, Player,
 *   closeMidiOutputs, and setUpMusicalEnvironmentExamples.
 * @returns {object} kit: sweepCC, dualSweepCC, rhythmPresets, noteVoice, ccLane,
 *   openOutput, createPeakPlayers, runStressTest, buildAndRun (plus PURE helpers).
 */
function createPeakKit(K) {
    if (!K || typeof K.QuantizedMap !== 'function' || typeof K.Player !== 'function') {
        throw new Error('konduktiva-peak: createPeakKit(K) needs a konduktiva module exporting QuantizedMap and Player');
    }
    const sweeps = makeSweeps(K);
    const players = playersFactory.makePlayers(K, sweeps);
    const stress = makeStress(K, players);

    return {
        // sweeps / rhythm
        sweepCC: sweeps.sweepCC,
        dualSweepCC: sweeps.dualSweepCC,
        rhythmPresets: sweeps.rhythmPresets,
        // players / maps
        ensureSharedMaps: players.ensureSharedMaps,
        ensureChannelMap: players.ensureChannelMap,
        noteVoice: players.noteVoice,
        ccLane: players.ccLane,
        createPeakPlayers: players.createPeakPlayers,
        // output / run
        openOutput: stress.openOutput,
        runStressTest: stress.runStressTest,
        buildAndRun: stress.buildAndRun,
        guardSuperdirtPort: stress.guardSuperdirtPort,
        // re-export pure helpers for convenience
        loadCcMap: cc.loadCcMap,
        resolveCc: cc.resolveCc,
        safeSweepRange: cc.safeSweepRange,
        RECOMMENDED_SWEEP_CCS: cc.RECOMMENDED_SWEEP_CCS,
        listMidiOutputs: output.listMidiOutputs,
        resolvePeakPort: output.resolvePeakPort,
        panic: output.panic,
        PEAK_DEFAULTS: output.PEAK_DEFAULTS,
        // constants
        PEAK_PLAYER_NAMES: playersFactory.PEAK_PLAYER_NAMES,
        NOTE_PLAYER_NAMES: playersFactory.NOTE_PLAYER_NAMES,
        CC_PLAYER_NAMES: playersFactory.CC_PLAYER_NAMES,
        DEFAULT_SCALE_POOL: playersFactory.DEFAULT_SCALE_POOL,
    };
}

module.exports = {
    // factory
    createPeakKit,
    // PURE helpers (no konduktiva needed)
    loadCcMap: cc.loadCcMap,
    resolveCc: cc.resolveCc,
    buildCcSet: cc.buildCcSet,
    RECOMMENDED_SWEEP_CCS: cc.RECOMMENDED_SWEEP_CCS,
    safeSweepRange: cc.safeSweepRange,
    STANDARD_FALLBACK_CC: cc.STANDARD_FALLBACK_CC,
    DEFAULT_CC_MAP_PATH: cc.DEFAULT_CC_MAP_PATH,
    PEAK_DEFAULTS: output.PEAK_DEFAULTS,
    listMidiOutputs: output.listMidiOutputs,
    resolvePeakPort: output.resolvePeakPort,
    panic: output.panic,
    // constants
    PEAK_PLAYER_NAMES: playersFactory.PEAK_PLAYER_NAMES,
    NOTE_PLAYER_NAMES: playersFactory.NOTE_PLAYER_NAMES,
    CC_PLAYER_NAMES: playersFactory.CC_PLAYER_NAMES,
    DEFAULT_SCALE_POOL: playersFactory.DEFAULT_SCALE_POOL,
};
