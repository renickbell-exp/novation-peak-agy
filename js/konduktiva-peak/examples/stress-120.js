'use strict';
// ---------------------------------------------------------------------------
// stress-120.js -- thin CLI around konduktiva-peak.
//
//   --live   drive the real Novation Peak (audible, full duration)
//   --dry    capture emitted messages with no hardware and no sound (default)
//
// Env overrides (mirror the reference demo):
//   KONDUKTIVA_BACKEND = auto|rust|js|echo   (default auto)
//   STRESS_SECONDS     = <n>                 (default 120 live / 14 dry)
//   STRESS_BPM         = <n>                 (default 116)
//   PEAK_PORT          = <exact easymidi output name> (default first /peak/i)
// ---------------------------------------------------------------------------

const peak = require('../src/index');
const { resolveKonduktiva } = require('../src/_resolve');

const LIVE = process.argv.includes('--live');
const DRY = !LIVE; // default to dry unless --live is explicitly passed
const BPM = Number(process.env.STRESS_BPM || 116);
const DURATION_S = Number(process.env.STRESS_SECONDS || (LIVE ? 120 : 14));
const BACKEND = process.env.KONDUKTIVA_BACKEND || 'auto';

const K = resolveKonduktiva();
const kit = peak.createPeakKit(K);

console.log('[peak-stress] mode=' + (LIVE ? 'LIVE' : 'DRY') +
    ' backend=' + BACKEND + ' bpm=' + BPM + ' seconds=' + DURATION_S);

kit.buildAndRun({
    live: LIVE,
    captureStats: DRY,
    backend: BACKEND,
    durationS: DURATION_S,
    bpm: BPM,
    channel: 1,
    portName: process.env.PEAK_PORT,
    exitOnDone: true,
}).catch((err) => {
    console.error('[peak-stress] FATAL:', err && err.message ? err.message : err);
    process.exit(1);
});

process.on('SIGINT', () => { console.log('\n[peak-stress] SIGINT'); process.exit(0); });
