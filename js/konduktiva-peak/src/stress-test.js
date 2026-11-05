'use strict';
// ---------------------------------------------------------------------------
// stress-test.js -- konduktiva-aware output wiring + the movement timeline.
//
// openOutput, runStressTest, and buildAndRun all need the host env, so they are
// produced by makeStress(K, parts) and exposed through the kit. The timeline
// (intro -> add lead/mod -> full -> wind down) and the capture PASS criteria are
// preserved from the working reference demo.
// ---------------------------------------------------------------------------

const { easymidi, resolvePeakPort, makeCaptureStub, panic, PEAK_DEFAULTS } = require('./output');
const { buildCcSet } = require('./cc');
const { PEAK_PLAYER_NAMES } = require('./players');

/**
 * Bind the output/run helpers to a host konduktiva module.
 * @param {object} K host konduktiva (closeMidiOutputs, setUpMusicalEnvironmentExamples).
 * @param {object} players the result of makePlayers(K, sweeps).
 * @returns {{openOutput:Function, runStressTest:Function, buildAndRun:Function}}
 */
function makeStress(K, players) {
    /**
     * Make konduktiva-peak robust against concurrent sessions on this machine.
     * `setUpMusicalEnvironmentExamples()` opens two shared ports that this
     * MIDI-only module never uses: the superdirt OSC port (57121) and the
     * default websocket server (8080). If another session already holds either,
     * the bind emits an 'error' that, left unhandled, crashes the process. We
     * attach one-time handlers that swallow EADDRINUSE and continue MIDI-only.
     * Idempotent and best-effort. Call AFTER env creation, in the same tick, so
     * the handler is in place before the async bind error fires.
     */
    function guardSuperdirtPort() {
        // 1. superdirt OSC port (57121)
        try {
            const up = K.udpPort;
            if (up && typeof up.on === 'function' && !up.__peakGuarded) {
                up.__peakGuarded = true;
                up.on('error', (err) => {
                    if (err && err.code === 'EADDRINUSE') {
                        console.warn('[konduktiva-peak] superdirt OSC port 57121 is busy ' +
                            '(another session); continuing MIDI-only.');
                    } else {
                        console.warn('[konduktiva-peak] superdirt OSC port error: ' +
                            (err && err.message ? err.message : err));
                    }
                });
            }
        } catch (_) { /* udpPort not present on this konduktiva: nothing to guard */ }

        // 2. default websocket server (8080)
        try {
            const ws = K.websocketSession;
            const wss = ws && typeof ws.connectVariableToCurrentWss === 'function'
                ? ws.connectVariableToCurrentWss() : false;
            if (wss && typeof wss.on === 'function' && !wss.__peakGuarded) {
                wss.__peakGuarded = true;
                wss.on('error', (err) => {
                    if (err && err.code === 'EADDRINUSE') {
                        console.warn('[konduktiva-peak] websocket port 8080 is busy ' +
                            '(another session); continuing MIDI-only.');
                    } else {
                        console.warn('[konduktiva-peak] websocket server error: ' +
                            (err && err.message ? err.message : err));
                    }
                });
            }
        } catch (_) { /* websocket server not present on this konduktiva: nothing to guard */ }
    }

    /**
     * Wire env.midiOutputs to a single output. Closes any existing outputs first.
     * @param {object} env a MusicalEnvironment.
     * @param {object} [opts]
     * @param {boolean} [opts.live=false] open the real Peak port.
     * @param {string} [opts.portName] exact output name (overrides pattern).
     * @param {RegExp} [opts.pattern] port match pattern (default /peak/i).
     * @param {boolean} [opts.captureStats=false] install a counting capture stub.
     * @param {string} [opts.virtualName] virtual port name for non-live, non-capture.
     * @returns {{index:number, name:string, output:object, stats?:object}}
     */
    function openOutput(env, opts) {
        const o = opts || {};
        K.closeMidiOutputs(env);

        if (o.captureStats) {
            const output = makeCaptureStub();
            env.midiOutputs = [output];
            return { index: 0, name: '(capture)', output, stats: output.stats };
        }

        if (o.live) {
            const name = resolvePeakPort({ pattern: o.pattern, override: o.portName });
            if (!name) {
                const pat = o.pattern || PEAK_DEFAULTS.portPattern;
                throw new Error('konduktiva-peak: no MIDI output matching ' + pat +
                    '. Available: ' + JSON.stringify(easymidi.getOutputs()));
            }
            const output = new easymidi.Output(name);
            env.midiOutputs = [output];
            return { index: 0, name, output };
        }

        // Non-live, non-capture: a virtual port so routing can be exercised
        // without the hardware present.
        const vname = o.virtualName || 'konduktiva-peak';
        const output = new easymidi.Output(vname, true);
        env.midiOutputs = [output];
        return { index: 0, name: vname, output };
    }

    /**
     * Run the stress-test movement timeline against players already created on
     * env. Sets tempo, starts the scheduler, stages the players, prints the
     * dispatch backend, then on completion sends a panic and resolves.
     * @param {object} env a MusicalEnvironment with the Peak players created.
     * @param {object} [opts]
     * @param {number} [opts.durationS=120] run length in seconds.
     * @param {number} [opts.bpm=116] tempo.
     * @param {boolean} [opts.live=false] staged (live) vs all-at-once (capture) timeline.
     * @param {number} [opts.channel=1] 1-based MIDI channel (for panic).
     * @param {boolean} [opts.captureStats] default !live; resolve with { stats, pass }.
     * @param {string[]} [opts.players] player names to manage (default all 8).
     * @param {object} [opts.ccMap] parsed Peak CC map (for the PASS controller check).
     * @returns {Promise<void|{stats:object, pass:boolean}>}
     */
    function runStressTest(env, opts) {
        const o = opts || {};
        const durationS = o.durationS == null ? 120 : o.durationS;
        const bpm = o.bpm == null ? 116 : o.bpm;
        const live = !!o.live;
        const channel = o.channel == null ? 1 : o.channel;
        const captureStats = o.captureStats == null ? !live : o.captureStats;
        const managed = o.players || PEAK_PLAYER_NAMES.slice();
        const CC = buildCcSet(o.ccMap);

        // playN/stopN only the names that actually exist on env.
        const present = (names) => names.filter((n) => env.players[n]);
        const playN = (names) => env.playN(present(names));
        const stopN = (names) => env.stopN(present(names));

        return new Promise((resolve) => {
            env.currentTempo = bpm;
            env.startScheduler();

            const at = (sec, fn) => setTimeout(fn, sec * 1000);
            const log = (s) => console.log('[peak-stress t+' + Number(s).toFixed(0) + 's]');
            const T = durationS;

            if (captureStats) {
                // Activate everything quickly so every player type is captured.
                at(0.3, () => { playN(managed); console.log('[peak-stress] capture: all players active'); });
            } else {
                at(0.5, () => { playN(['bass', 'chords', 'ccCutoff', 'ccFx']); log(0.5); });
                at(T * 0.22, () => { playN(['lead', 'ccMod']); log(T * 0.22); });
                at(T * 0.42, () => { playN(['arp', 'ccReso']); log(T * 0.42); });
                at(T * 0.86, () => { stopN(['arp']); log(T * 0.86); });
                at(T * 0.94, () => { stopN(['lead', 'ccReso']); log(T * 0.94); });
            }

            // Report which backend actually drove, shortly after first events fire.
            at(1.2, () => {
                try {
                    const core = env.getDispatchCore();
                    console.log('[peak-stress] dispatch backend: ' + core.capabilities().backend);
                } catch (_) { /* core may be unavailable */ }
            });

            at(T, () => {
                const output = env.midiOutputs && env.midiOutputs[0];
                panic(output, channel);
                if (captureStats) {
                    const stats = (output && output.stats) ||
                        { noteon: 0, noteoff: 0, cc: 0, byChannel: {}, byController: {} };
                    const wire = channel - 1;
                    const okChan = Object.keys(stats.byChannel).every((c) => c === String(wire));
                    const expected = [CC.cutoff, CC.reso, CC.mod, CC.reverb, CC.delay];
                    const okCtrls = expected.filter((c) => stats.byController[c]);
                    const pass = stats.noteon > 0 && stats.noteoff > 0 && stats.cc > 0 &&
                        okChan && okCtrls.length >= 4;
                    console.log('[peak-stress] CAPTURE RESULTS over ' + T + 's:');
                    console.log('  note-on : ' + stats.noteon);
                    console.log('  note-off: ' + stats.noteoff);
                    console.log('  cc      : ' + stats.cc);
                    console.log('  by channel (0-indexed): ' + JSON.stringify(stats.byChannel));
                    console.log('  by controller        : ' + JSON.stringify(stats.byController));
                    console.log('  expected CCs present : ' + okCtrls.length + '/5  (' + okCtrls.join(',') + ')');
                    console.log('  channel-' + channel + ' only      : ' + okChan);
                    console.log(pass ? '[peak-stress] CAPTURE PASS' : '[peak-stress] CAPTURE FAIL');
                    resolve({ stats, pass });
                } else {
                    console.log('[peak-stress] done -- ' + T + ' s complete');
                    resolve();
                }
            });
        });
    }

    /**
     * Convenience end-to-end runner: build an env, open the output, create the
     * players, run the stress test, then tear down. Exits the process when run
     * as a script (opts.exitOnDone defaults to true).
     * @param {object} [opts] merged options for openOutput / createPeakPlayers / runStressTest.
     * @param {string} [opts.backend] scheduler backend (default env KONDUKTIVA_BACKEND or 'auto').
     * @param {boolean} [opts.exitOnDone=true] call process.exit at the end.
     * @returns {Promise<void|{stats:object, pass:boolean}>}
     */
    async function buildAndRun(opts) {
        const o = opts || {};
        const live = !!o.live;
        const channel = o.channel == null ? 1 : o.channel;
        const captureStats = o.captureStats == null ? !live : o.captureStats;
        const exitOnDone = o.exitOnDone == null ? true : o.exitOnDone;

        const env = K.setUpMusicalEnvironmentExamples();
        // Guard the shared OSC (57121) and websocket (8080) ports AFTER setup:
        // setup creates the websocket server, and both binds emit their
        // EADDRINUSE on a later tick, so attaching here (same tick) is in time.
        guardSuperdirtPort();
        env.schedulerBackend = o.backend || process.env.KONDUKTIVA_BACKEND || 'auto';

        openOutput(env, {
            live,
            captureStats,
            portName: o.portName,
            pattern: o.pattern,
            virtualName: o.virtualName,
        });

        const created = players.createPeakPlayers(env, {
            channel,
            outputIndex: o.outputIndex,
            ccMap: o.ccMap,
            scalePool: o.scalePool,
        });

        let result;
        try {
            result = await runStressTest(env, {
                durationS: o.durationS,
                bpm: o.bpm,
                live,
                channel,
                captureStats,
                players: created.all,
                ccMap: o.ccMap,
            });
        } finally {
            // Teardown: stop players, panic, stop scheduler, close outputs.
            try { env.stopN(created.all); } catch (_) {}
            try { panic(env.midiOutputs && env.midiOutputs[0], channel); } catch (_) {}
            try { env.stopScheduler(); } catch (_) {}
            await new Promise((r) => setTimeout(r, 200));
            try { K.closeMidiOutputs(env); } catch (_) {}
        }

        if (exitOnDone) {
            const code = (result && result.pass === false) ? 2 : 0;
            process.exit(code);
        }
        return result;
    }

    return { openOutput, runStressTest, buildAndRun, guardSuperdirtPort };
}

module.exports = { makeStress };
