'use strict';
// ---------------------------------------------------------------------------
// players.js -- konduktiva Player + per-player map builders for the Peak.
//
// The wiring (noteMap/octaveMap/rootMap/velocityMap/noteDurationMap/rhythmMap/
// channelMap/modeMap('peakPass')/legatoMap/polyphony for notes; controlChangeMap
// + channelMap for CC lanes) is preserved verbatim from the working reference
// demo. The Peak is an 8-voice MONOTIMBRAL synth, so the optimal layout is 8
// players on ONE channel: 4 note voices inside the 8-voice budget plus 4 CC lanes.
// ---------------------------------------------------------------------------

const { buildCcSet } = require('./cc');

// C natural-minor degree pool (semitone offsets from root+octave).
const DEFAULT_SCALE_POOL = [0, 2, 3, 5, 7, 8, 10, 12];

// === LADDER AND DENSITY: the parameters to change live ===

// The declared minimum inter-onset interval, in beats. 0.125 is a 32nd in 4/4,
// so the densest rung of the ladder is 8 onsets per beat.
const MIN_IOI = 0.125;

// The seed the ladder is split down and thinned up from: one bar of two half
// notes. Every row covers the same 4 beats, so choosing a row changes how many
// onsets land in the bar, never how long the bar is. This particular seed makes
// the rungs at 0.5, 1, 2 and 4 onsets per beat come out identical to the four
// fixed presets this layout used before, so the voices sound as they did.
const LADDER_SEED = [2, 2];

// Name of the shared ladder in env.rhythmMaps, and of the two published graphs.
const LADDER_NAME = 'peakGrid';
const HOLD_GRAPH = 'peakHold';
const BUILD_GRAPH = 'peakBuild';

// Where each voice sits on the ladder by default, in ONSETS PER BEAT. These are
// the rates the Peak layout was written for: half notes for bass and chords,
// eighths for the lead, sixteenths for the arp, and matching rates for the CC
// lanes. Change a number here to move that one voice up or down the ladder.
const PEAK_NATIVE_RATE = {
    bass: 0.5,
    chords: 0.5,
    lead: 2,
    arp: 4,
    ccCutoff: 4,
    ccReso: 2,
    ccMod: 2,
    ccFx: 1,
};

// Canonical player names, in the order the demo creates them.
const PEAK_PLAYER_NAMES = ['bass', 'chords', 'lead', 'arp', 'ccCutoff', 'ccReso', 'ccMod', 'ccFx'];
const NOTE_PLAYER_NAMES = ['bass', 'chords', 'lead', 'arp'];
const CC_PLAYER_NAMES = ['ccCutoff', 'ccReso', 'ccMod', 'ccFx'];

// All map collections + filter tables the actions read.
const MAP_COLLECTIONS = ['noteMaps', 'octaveMaps', 'rootMaps', 'velocityMaps',
    'noteDurationMaps', 'channelMaps', 'modeMaps', 'legatoMaps', 'maxPolyphonyMaps',
    'controlChangeMaps', 'rhythmMaps', 'maskMaps'];

/**
 * Bind the player builders to a host konduktiva module and sweep helpers.
 * @param {object} K host konduktiva (QuantizedMap, Player).
 * @param {object} sweeps result of makeSweeps(K) (sweepCC, dualSweepCC, rhythmPresets).
 */
function makePlayers(K, sweeps) {
    const QuantizedMap = K.QuantizedMap;
    const Player = K.Player;

    /**
     * Ensure every map collection, the modeFilters table, and the shared maps
     * (peakPass mode, peak legato, 8-voice polyphony, default mask) exist on env.
     * Idempotent. modeMap 'peakPass' returns a string absent from modeFilters so
     * filterMode passes pre-picked scale degrees through unchanged.
     * @param {object} env a MusicalEnvironment.
     */
    function ensureSharedMaps(env) {
        for (const k of MAP_COLLECTIONS) {
            if (env[k] === undefined) env[k] = {};
        }
        if (env.modeFilters === undefined) env.modeFilters = {};
        if (env.densityGraphs === undefined) env.densityGraphs = {};
        if (!Array.isArray(env.currentDensityGraphs)) env.currentDensityGraphs = [];
        if (!env.modeMaps.peakPass) env.modeMaps.peakPass = new QuantizedMap(1, [0], ['__pass__']);
        if (!env.modeFilters.__pass__) {
            env.modeFilters.__pass__ = new QuantizedMap(
                12,
                [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
                [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
            );
        }
        if (!env.legatoMaps.peak) env.legatoMaps.peak = new QuantizedMap(1, [0], [0.9]);
        if (!env.maxPolyphonyMaps.peak8) env.maxPolyphonyMaps.peak8 = new QuantizedMap(1, [0], [8]);
        if (!env.maskMaps.default) env.maskMaps.default = new QuantizedMap(4, [0], [false]);
    }

    /**
     * Ensure the shared rhythm LADDER exists in env.rhythmMaps and return its
     * name. The ladder is built by the core's makeRhythmMap, so it is a nested
     * QuantizedMap: outer keys are onsets per beat, values are the row patterns.
     * The project's rest pattern is handed to the builder so that the sparser
     * rows drop SILENT onsets first rather than sounding ones.
     * @param {object} env a MusicalEnvironment.
     * @param {object} [spec] optional overrides.
     * @param {string} [spec.rhythmMap] name of a ladder already on env to use instead.
     * @param {number} [spec.minIOI=MIN_IOI] declared minimum inter-onset interval, in beats.
     * @param {number[]} [spec.seed=LADDER_SEED] starting inter-onset intervals, in beats.
     * @returns {string} the rhythmMaps key.
     */
    function ensureRhythmLadder(env, spec) {
        const s = spec || {};
        if (s.rhythm) {
            // A caller-supplied map must itself be a ladder. The old flat
            // presets in sweeps.js are rejected here rather than silently
            // pinning the voice to one rate.
            const own = (s.name || 'peak') + '_rhy';
            assertLadder(own, s.rhythm);
            env.rhythmMaps[own] = s.rhythm;
            return own;
        }
        if (s.rhythmMap) {
            const existing = env.rhythmMaps[s.rhythmMap];
            if (existing === undefined) {
                throw new Error('konduktiva-peak: no rhythmMap named "' + s.rhythmMap
                    + '" on this environment.');
            }
            assertLadder(s.rhythmMap, existing);
            return s.rhythmMap;
        }
        const name = s.ladderName || LADDER_NAME;
        if (!env.rhythmMaps[name]) {
            env.rhythmMaps[name] = K.makeRhythmMap(
                s.minIOI == null ? MIN_IOI : s.minIOI,
                [0.5],
                s.seed || LADDER_SEED,
                { maskMap: env.maskMaps.default });
        }
        assertLadder(name, env.rhythmMaps[name]);
        return name;
    }

    /**
     * Fail loudly if a map is a single flat pattern rather than a ladder. A flat
     * pattern gives density nothing to select, so it silently pins the voice to
     * one rate forever.
     * @param {string} name the map name, for the error message.
     * @param {object} map the candidate rhythm map.
     */
    function assertLadder(name, map) {
        const report = K.validateRhythmMap(map);
        if (!report.ok || !(map.values && map.values.length > 1)) {
            throw new Error('konduktiva-peak: rhythmMap "' + name + '" is not a density '
                + 'ladder. A rhythmMap is a nested QuantizedMap of more than one row, '
                + 'keyed by onsets per beat. Build it with K.makeRhythmMap(minIOI, '
                + '[0.5], deltas). '
                + (report.ok ? 'It has ' + (map.values ? map.values.length : 0) + ' row(s).'
                    : report.errors.join('; ')));
        }
    }

    /**
     * Publish the two density graphs and make the holding one current. ONE graph
     * carries a curve for every player, because graphs compose by arithmetic
     * mean: naming a second graph in the array pulls every voice halfway toward
     * it rather than scaling anything.
     *
     * peakHold sits each voice on its own native rung, which is the rhythm the
     * Peak layout was written for. peakBuild is published but NOT made current:
     * it doubles every voice across 16 beats and back. To bring it in live:
     *     K.useDensityGraphs(env, 'peakBuild', { replace: true })
     * and to go back, the same call with 'peakHold'.
     * @param {object} env a MusicalEnvironment.
     * @param {object} [rates] player name to onsets per beat (default PEAK_NATIVE_RATE).
     * @returns {{hold:string, build:string}} the published graph names.
     */
    function ensureDensityGraphs(env, rates) {
        ensureSharedMaps(env);
        const r = rates || PEAK_NATIVE_RATE;
        const hold = {};
        const build = {};
        // A density must be finite and greater than zero. Silence is the mask's job.
        hold.default = [[0, 1], [4, 1]];
        build.default = [[0, 1], [8, 2], [16, 1]];
        for (const name of Object.keys(r)) {
            const rate = r[name];
            hold[name] = [[0, rate], [4, rate]];
            build[name] = [[0, rate], [8, rate * 2], [16, rate]];
        }
        env.densityGraphs[HOLD_GRAPH] = K.densityGraph(hold);
        env.densityGraphs[BUILD_GRAPH] = K.densityGraph(build);
        K.useDensityGraphs(env, HOLD_GRAPH, { replace: true });
        return { hold: HOLD_GRAPH, build: BUILD_GRAPH };
    }

    /**
     * Ensure a channelMap exists for a 1-based MIDI channel and return its name.
     * @param {object} env a MusicalEnvironment.
     * @param {number} channel 1-based MIDI channel.
     * @returns {string} the channelMap key.
     */
    function ensureChannelMap(env, channel) {
        const name = 'ch' + channel;
        if (!env.channelMaps[name]) env.channelMaps[name] = new QuantizedMap(1, [0], [channel]);
        return name;
    }

    /**
     * Create a note player wired to sendNotesMidiInfo, fully mapped.
     * @param {object} env a MusicalEnvironment.
     * @param {string} name player name.
     * @param {object} spec
     * @param {object} spec.rhythm a rhythm QuantizedMap (from rhythmPresets()).
     * @param {number} spec.octave octave number.
     * @param {number[]} spec.vel velocity sequence.
     * @param {number} spec.dur note duration in beats.
     * @param {Array} spec.noteSeq sequence of note-offset arrays.
     * @param {number} [spec.channel=1] 1-based MIDI channel.
     * @param {number} [spec.outputIndex=1] 1-based midiOutput index (-> midiOutputs[index-1]).
     * @param {string} [spec.root='C'] root note name.
     * @returns {string} the player name.
     */
    function noteVoice(env, name, spec) {
        ensureSharedMaps(env);
        const channel = spec.channel == null ? 1 : spec.channel;
        const outputIndex = spec.outputIndex == null ? 1 : spec.outputIndex;
        const root = spec.root || 'C';
        const p = new Player(name);
        p.action = 'sendNotesMidiInfo';
        p.midiOutput = outputIndex;
        p.rhythmMap = ensureRhythmLadder(env, Object.assign({ name }, spec));
        // The ladder is traversed by the graphs in env.currentDensityGraphs. The
        // Player constructor writes a singular densityGraph field, which shadows
        // that array wherever it is read, so it is removed.
        delete p.densityGraph;
        p.maskMap = 'default';
        p.modeMap = 'peakPass';
        p.channelMap = ensureChannelMap(env, channel);
        p.legatoMap = 'peak';
        p.polyphonyMap = 'peak8';
        p.noteMap = name + '_notes';
        env.noteMaps[p.noteMap] = new QuantizedMap(
            spec.noteSeq.length, spec.noteSeq.map((_, i) => i), spec.noteSeq);
        p.octaveMap = name + '_oct'; env.octaveMaps[p.octaveMap] = new QuantizedMap(1, [0], [spec.octave]);
        p.rootMap = name + '_root'; env.rootMaps[p.rootMap] = new QuantizedMap(1, [0], [root]);
        p.velocityMap = name + '_vel';
        env.velocityMaps[p.velocityMap] = new QuantizedMap(spec.vel.length, spec.vel.map((_, i) => i), spec.vel);
        p.noteDurationMap = name + '_dur'; env.noteDurationMaps[p.noteDurationMap] = new QuantizedMap(1, [0], [spec.dur]);
        env.players[name] = p;
        return name;
    }

    /**
     * Create a CC player wired to sendMidiCCMessages.
     * @param {object} env a MusicalEnvironment.
     * @param {string} name player name.
     * @param {object} spec
     * @param {object} spec.rhythm a rhythm QuantizedMap.
     * @param {object} spec.ccMap a controlChangeMap (from sweepCC/dualSweepCC).
     * @param {number} [spec.channel=1] 1-based MIDI channel.
     * @param {number} [spec.outputIndex=1] 1-based midiOutput index.
     * @returns {string} the player name.
     */
    function ccLane(env, name, spec) {
        ensureSharedMaps(env);
        const channel = spec.channel == null ? 1 : spec.channel;
        const outputIndex = spec.outputIndex == null ? 1 : spec.outputIndex;
        const p = new Player(name);
        p.action = 'sendMidiCCMessages';
        p.midiOutput = outputIndex;
        p.rhythmMap = ensureRhythmLadder(env, Object.assign({ name }, spec));
        delete p.densityGraph;
        p.maskMap = 'default';
        p.channelMap = ensureChannelMap(env, channel);
        p.controlChangeMap = name + '_cc'; env.controlChangeMaps[p.controlChangeMap] = spec.ccMap;
        env.players[name] = p;
        return name;
    }

    /**
     * Build the canonical 8-player Peak layout (4 note voices + 4 CC lanes), all
     * on one channel, using resolved Peak CC numbers with the resonance clamp.
     * @param {object} env a MusicalEnvironment.
     * @param {object} [opts]
     * @param {number} [opts.channel=1] 1-based MIDI channel.
     * @param {number} [opts.outputIndex=1] 1-based midiOutput index.
     * @param {object} [opts.ccMap] parsed Peak CC map (default bundled).
     * @param {number[]} [opts.scalePool] scale-degree pool for lead/arp (default C minor).
     * @param {object} [opts.rates] player name to onsets per beat, overriding PEAK_NATIVE_RATE.
     * @returns {{notePlayers:string[], ccPlayers:string[], all:string[]}}
     */
    function createPeakPlayers(env, opts) {
        const o = opts || {};
        const channel = o.channel == null ? 1 : o.channel;
        const outputIndex = o.outputIndex == null ? 1 : o.outputIndex;
        const pool = o.scalePool || DEFAULT_SCALE_POOL;
        const CC = buildCcSet(o.ccMap);
        const base = { channel, outputIndex };

        // Publish the density graphs first: every player below reads the same
        // ladder, and it is these curves that decide the rate of each voice.
        ensureSharedMaps(env);
        ensureDensityGraphs(env, o.rates);

        // 4 note voices that together sit inside the 8-voice budget.
        noteVoice(env, 'bass', Object.assign({}, base, {
            octave: 2, vel: [98, 90], dur: 1.7,
            noteSeq: [[0], [7], [3], [5], [0], [10], [7], [3]], // roots and fifths
        }));
        noteVoice(env, 'chords', Object.assign({}, base, {
            octave: 4, vel: [66, 60], dur: 1.7,
            noteSeq: [[0, 3, 7], [3, 7, 10], [5, 8, 12], [7, 10, 14], [0, 3, 7, 10]], // minor triads/7ths
        }));
        noteVoice(env, 'lead', Object.assign({}, base, {
            octave: 5, vel: [104, 88, 96, 84], dur: 0.45,
            noteSeq: [[7], [10], [12], [10], [8], [7], [5], [7], [3], [5], [7], [12]]
                .map(([x]) => [pool[x % pool.length]]),
        }));
        noteVoice(env, 'arp', Object.assign({}, base, {
            octave: 5, vel: [80, 64, 72, 60], dur: 0.2,
            noteSeq: [[0], [3], [7], [12], [7], [3], [10], [7], [5], [8], [12], [8]]
                .map(([x]) => [pool[x % pool.length]]),
        }));

        // 4 CC lanes. Resonance (CC79) self-oscillates / gets loud near the top,
        // so it is capped at 62 for a safe live run.
        ccLane(env, 'ccCutoff', Object.assign({}, base, {
            ccMap: sweeps.sweepCC(CC.cutoff, 28, 118, 8, 32),
        }));
        ccLane(env, 'ccReso', Object.assign({}, base, {
            ccMap: sweeps.sweepCC(CC.reso, 6, 62, 6, 24),
        }));
        ccLane(env, 'ccMod', Object.assign({}, base, {
            ccMap: sweeps.sweepCC(CC.mod, 0, 100, 4, 16),
        }));
        ccLane(env, 'ccFx', Object.assign({}, base, {
            ccMap: sweeps.dualSweepCC(CC.reverb, 20, 90, CC.delay, 12, 70, 12, 12),
        }));

        return {
            notePlayers: NOTE_PLAYER_NAMES.slice(),
            ccPlayers: CC_PLAYER_NAMES.slice(),
            all: PEAK_PLAYER_NAMES.slice(),
        };
    }

    return {
        ensureSharedMaps,
        ensureRhythmLadder,
        ensureDensityGraphs,
        ensureChannelMap,
        noteVoice,
        ccLane,
        createPeakPlayers,
    };
}

module.exports = {
    makePlayers,
    MIN_IOI,
    LADDER_SEED,
    LADDER_NAME,
    HOLD_GRAPH,
    BUILD_GRAPH,
    PEAK_NATIVE_RATE,
    DEFAULT_SCALE_POOL,
    PEAK_PLAYER_NAMES,
    NOTE_PLAYER_NAMES,
    CC_PLAYER_NAMES,
};
