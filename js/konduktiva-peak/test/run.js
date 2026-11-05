'use strict';
// ---------------------------------------------------------------------------
// test/run.js -- no-hardware test runner for konduktiva-peak.
//
// Plain node, no test framework. Exits non-zero on any failure. Covers:
//   * loadCcMap parses the bundled map
//   * resolveCc('cutoff') === 29, resolveCc('resonance') === 79
//   * sweepCC builds a QuantizedMap whose wrapLookup returns {controller,value}
//     inside 0..127
//   * createPeakPlayers builds 8 players, all on channel 1
//   * a SHORT capture run emits note-on, note-off, and cc, all on wire channel 0,
//     with the expected controllers present
// ---------------------------------------------------------------------------

const peak = require('../src/index');
const { resolveKonduktiva } = require('../src/_resolve');

const K = resolveKonduktiva();
const kit = peak.createPeakKit(K);

let failures = 0;
function check(label, cond, detail) {
    const ok = !!cond;
    if (!ok) failures++;
    console.log((ok ? 'PASS  ' : 'FAIL  ') + label + (detail != null ? '  -> ' + detail : ''));
}

async function main() {
    // 1. loadCcMap parses.
    const ccMap = peak.loadCcMap();
    check('loadCcMap parses bundled JSON',
        ccMap && Array.isArray(ccMap.cc_map) && ccMap._meta != null,
        'cc_map entries: ' + (ccMap && ccMap.cc_map ? ccMap.cc_map.length : 0));

    // 2. resolveCc keyword matching.
    const cutoff = peak.resolveCc('cutoff');
    const reso = peak.resolveCc('resonance');
    check('resolveCc("cutoff") === 29', cutoff === 29, String(cutoff));
    check('resolveCc("resonance") === 79', reso === 79, String(reso));

    // 3. sweepCC -> QuantizedMap with in-range {controller,value}.
    const sweep = kit.sweepCC(29, 28, 118, 8, 32);
    let sweepOk = typeof sweep.wrapLookup === 'function';
    let sample = null;
    for (let beat = 0; beat <= 16 && sweepOk; beat += 0.5) {
        const v = sweep.wrapLookup(beat);
        if (!v || v.controller !== 29 || typeof v.value !== 'number' ||
            v.value < 0 || v.value > 127) {
            sweepOk = false;
            sample = v;
        } else {
            sample = v;
        }
    }
    check('sweepCC wrapLookup returns in-range {controller,value}',
        sweepOk, JSON.stringify(sample));

    // One env for the remaining checks. setUpMusicalEnvironmentExamples binds the
    // shared OSC UDP port (57121) and the websocket server (8080); guard AFTER it
    // so an EADDRINUSE from a concurrent live session (or 8080 in use) is swallowed
    // rather than crashing the process. The bind errors fire on a later tick, so
    // attaching the handlers right after setup is in time.
    const env = K.setUpMusicalEnvironmentExamples();
    kit.guardSuperdirtPort();
    env.schedulerBackend = process.env.KONDUKTIVA_BACKEND || 'auto';

    // 4. createPeakPlayers -> 8 players, all channel 1.
    const created = kit.createPeakPlayers(env, { channel: 1 });
    const eight = created.all.length === 8 &&
        created.all.every((n) => env.players[n] != null);
    check('createPeakPlayers builds 8 players', eight, created.all.join(','));
    const allCh1 = created.all.every((n) => {
        const p = env.players[n];
        const cm = env.channelMaps[p.channelMap];
        return cm && cm.wrapLookup(0) === 1;
    });
    check('all 8 players on channel 1', allCh1);

    // 5. SHORT capture run: note-on, note-off, cc, wire channel 0, expected CCs.
    kit.openOutput(env, { captureStats: true });
    const players = created;
    const durationS = Number(process.env.STRESS_SECONDS || 6);
    const result = await kit.runStressTest(env, {
        durationS,
        bpm: 140,
        captureStats: true,
        players: players.all,
    });

    // Teardown (no process.exit; this is a library-level run).
    try { env.stopN(players.all); } catch (_) {}
    try { kit.panic(env.midiOutputs[0], 1); } catch (_) {}
    try { env.stopScheduler(); } catch (_) {}
    await new Promise((r) => setTimeout(r, 200));
    try { K.closeMidiOutputs(env); } catch (_) {}

    const s = result.stats;
    check('capture: note-on emitted', s.noteon > 0, String(s.noteon));
    check('capture: note-off emitted', s.noteoff > 0, String(s.noteoff));
    check('capture: cc emitted', s.cc > 0, String(s.cc));
    const chans = Object.keys(s.byChannel);
    check('capture: all messages on wire channel 0',
        chans.length > 0 && chans.every((c) => c === '0'), JSON.stringify(s.byChannel));
    const expected = [29, 79, 1, 112, 108]; // cutoff, reso, mod, reverb, delay
    const present = expected.filter((c) => s.byController[c]);
    check('capture: expected controllers present (>=4)',
        present.length >= 4, present.join(','));
    check('capture: runStressTest reported PASS', result.pass === true, String(result.pass));

    console.log('');
    if (failures > 0) {
        console.log('konduktiva-peak tests: ' + failures + ' FAILURE(S)');
        process.exit(1);
    }
    console.log('konduktiva-peak tests: ALL PASS');
    process.exit(0);
}

main().catch((err) => {
    console.error('test runner crashed:', err && err.stack ? err.stack : err);
    process.exit(1);
});
