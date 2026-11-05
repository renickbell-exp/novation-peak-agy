# konduktiva-peak

`konduktiva-peak` packages the Novation Peak specific MIDI code as a reusable
module for any Konduktiva project. It bundles the Peak Control Change (CC) map,
output routing, note voices, CC sweep lanes, and a 120-second stress test. You
can drop it into an existing Konduktiva project to drive a Peak over MIDI, or
adapt its helpers to other synthesizers.

The module follows a dependency-injection design. Pure helpers (CC lookup, port
resolution, panic) need no Konduktiva. The Konduktiva-bound helpers come from a
kit factory, `createPeakKit(K)`, where `K` is the host project's Konduktiva
module. This keeps the package decoupled from any single Konduktiva checkout: the
host supplies its own copy.

---

## Install and prerequisites

Install the package and its runtime dependency:

```bash
npm install konduktiva-peak easymidi
```

You also need the following:

- **Host Konduktiva.** `konduktiva` is a peer dependency. The host project
  provides it; the kit receives it through `createPeakKit(K)`.
- **easymidi.** Used to enumerate MIDI ports and send messages.
- **A Novation Peak on a MIDI port.** By default the module selects the first
  output whose name matches the pattern `/peak/i`. Universal Serial Bus (USB)
  MIDI is preferred over the 5-pin DIN port for dense CC traffic.
- **MIDI channel 1.** The Peak's factory-default receive channel is 1. The
  module uses channel 1 by default.

The live stress test produces sound. The dry run does not touch hardware and
makes no sound.

---

## Quick start

The following example builds the full eight-player stress test and runs it
offline (no hardware, no sound):

```js
const K = require('konduktiva');
const { createPeakKit } = require('konduktiva-peak');

const peak = createPeakKit(K);
peak.buildAndRun({ live: false });
```

Set `live: true` to route to a real Peak and produce sound.

The package also defines npm scripts:

```bash
npm run stress:peak       # live run on real hardware (audible)
npm run stress:peak:dry   # dry run, no hardware, no sound
npm test                  # run the test suite
```

---

## Usage in a Konduktiva project

This section shows how to use the kit inside your own project. The steps are: get
the kit, open the Peak output, create the players on an existing
`MusicalEnvironment`, build CC sweeps, then run the bundled timeline or compose
your own.

### Create the kit

Pass your project's Konduktiva module to the factory:

```js
const K = require('konduktiva');
const { createPeakKit } = require('konduktiva-peak');
const peak = createPeakKit(K);
```

### Open the Peak output

Open an output on an existing `MusicalEnvironment` named `env`. The kit selects
the Peak port, attaches the output to the environment, and optionally captures
send statistics:

```js
const env = K.setUpMusicalEnvironmentExamples();

peak.openOutput(env, {
  live: true,          // false routes to a capture stub instead of hardware
  pattern: /peak/i,    // port-name pattern; omit to use the default
  captureStats: false, // true records counts of every message sent
});
```

### Create the players

Create the eight players (four note voices and four CC lanes) on the
environment. The function returns the player groups so you can start and stop
them by name:

```js
const { notePlayers, ccPlayers, all } = peak.createPeakPlayers(env, {
  channel: 1,      // Peak receive channel
  outputIndex: 0,  // index into env.midiOutputs
  // ccMap: a custom CC map; omit to load the bundled midi-cc-map.json
});
```

The four note voices are `bass`, `chords`, `lead`, and `arp`. Together they fit
inside the Peak's eight-voice budget. The four CC lanes sweep filter cutoff,
filter resonance, the modulation wheel, and the effects sends.

### Build CC sweeps

Build a control-change map that sweeps one controller between two values across
a number of beats. The shape is a triangle: the value rises to the maximum, then
falls back to the minimum, across one cycle.

```js
// Sweep CC 29 (filter cutoff) from 28 to 118 over 8 beats, in 32 steps.
const cutoffSweep = peak.sweepCC(29, 28, 118, 8, 32);
```

Think of `sweepCC` like a faucet handle that an automatic hand turns up and then
down at a fixed rhythm. `dualSweepCC` turns two handles at once with offset
timing, which exercises bursts of two CCs per onset.

### Run or compose

To run the bundled 120-second movement timeline:

```js
peak.runStressTest(env, {
  durationS: 120,
  bpm: 116,
  live: true,
  channel: 1,
  captureStats: false,
  players: all,
});
```

To compose your own arrangement, start and stop players directly on the
environment using the names returned by `createPeakPlayers`:

```js
env.startScheduler();
env.playN(['bass', 'chords', 'ccCutoff']);
// later
env.playN(['lead', 'arp']);
env.stopN(['arp']);
```

### Use the pure helpers

The pure helpers work without Konduktiva. Use them for port discovery, CC
lookup, and safety:

```js
const {
  resolveCc,
  listMidiOutputs,
  panic,
  resolvePeakPort,
} = require('konduktiva-peak');

const outputs = listMidiOutputs();        // array of MIDI output names
const portName = resolvePeakPort({});      // first name matching /peak/i
const cutoffCc = resolveCc('cutoff');      // 29

// Stop all sound on channel 1 (panic). `output` is an easymidi Output.
panic(output, 1);
```

---

## API reference

This section lists every export with its signature and a one-line description.

### Pure exports

These functions and constants require no Konduktiva.

- `loadCcMap(jsonPath?)`: load a CC map from a JSON file; defaults to the bundled
  `midi-cc-map.json`.
- `resolveCc(name, ccMap?)`: return the CC number for a parameter name (for
  example `"cutoff"`); searches the given or default map.
- `RECOMMENDED_SWEEP_CCS`: array of the CCs that are musically expressive and
  generally safe to sweep continuously.
- `safeSweepRange(name)`: return a clamped value range for a named parameter, to
  prevent loud or self-oscillating output.
- `PEAK_DEFAULTS`: defaults object, `{ channel: 1, portPattern: /peak/i }`.
- `listMidiOutputs()`: return an array of the available MIDI output port names.
- `resolvePeakPort({ pattern?, override? })`: return the Peak port name; an
  `override` wins, otherwise the first name matching `pattern`.
- `panic(output, channel)`: send all-sound-off, all-notes-off, and a full
  note-off sweep on the given channel.

### Kit exports

`createPeakKit(K)` returns an object with the following members. `K` is the
host's Konduktiva module.

- `sweepCC(controller, lo, hi, cycleBeats, steps)`: build a single-CC triangle
  sweep map.
- `dualSweepCC(cA, loA, hiA, cB, loB, hiB, cycleBeats, steps)`: build a map that
  emits two CCs per onset, each with its own offset sweep.
- `rhythmPresets()`: return the standard inter-onset-interval rhythm maps (half,
  quarter, eighth, sixteenth).
- `noteVoice(env, name, spec)`: create and register a note player on `env`;
  returns the player name.
- `ccLane(env, name, spec)`: create and register a CC player on `env`; returns
  the player name.
- `openOutput(env, { live, portName, pattern, captureStats, virtualName })`:
  open and attach a MIDI output to `env`.
- `createPeakPlayers(env, { channel, outputIndex, ccMap, scalePool })`: create
  the full set of players; returns `{ notePlayers, ccPlayers, all }`.
- `runStressTest(env, { durationS, bpm, live, channel, captureStats, players })`:
  run the movement timeline on an existing environment.
- `buildAndRun({ durationS, bpm, live, ... })`: convenience function that builds
  the environment, players, and output, then runs the stress test.

---

## Adapting to other synthesizers or projects

The module is built around the Peak, but its parts generalize. This section
explains how to retarget each part.

### Change the port

Two methods select a different port:

- Pass an explicit name or pattern: `resolvePeakPort({ override: 'My Synth' })`
  or `resolvePeakPort({ pattern: /minilogue/i })`.
- Change the default pattern globally: set `PEAK_DEFAULTS.portPattern` to a new
  regular expression.

### Change the channel

The Peak is monotimbral and listens on one channel. To target a different
channel, pass `channel` to `createPeakPlayers` or `runStressTest`. To change the
default, set `PEAK_DEFAULTS.channel`.

### Supply a different CC map

Load a CC map for another synthesizer with `loadCcMap(path)`. The expected JSON
shape is:

```json
{
  "_meta": {
    "recommended_for_stress_test": [
      { "cc": 29, "param": "Filter cutoff", "why": "expressive sweep" }
    ]
  },
  "cc_map": [
    { "cc": 29, "param": "Filter cutoff", "range": "0-127", "notes": "" }
  ]
}
```

The two arrays serve different roles:

- `cc_map`: the full parameter table. `resolveCc(name)` searches it.
- `_meta.recommended_for_stress_test`: a curated short list of safe, expressive
  CCs. The sweep lanes prefer this list when present.

### Clamp dangerous sweeps

Some parameters produce loud or self-oscillating output near the top of their
range (for example filter resonance and distortion level). Call
`safeSweepRange(name)` to obtain a clamped low-high range before building a
sweep, then pass that range to `sweepCC`.

---

## Novation Peak hardware reference

This section retains the Novation Peak hardware and MIDI reference material. The
USB identifier is `1235:007e` ("Focusrite-Novation Peak"). The machine-readable
CC table lives in `midi-cc-map.json`; the list of sources lives in `sources.md`.
The Peak's factory-default receive channel is **MIDI channel 1**.

### Overview and specifications

The Novation Peak is an eight-voice polyphonic desktop synthesizer. It has no
keyboard; it is a tabletop module. Its sound engine is a hybrid. The three
oscillators per voice are digital "New Oxford Oscillators" (numerically
controlled oscillators, NCOs, generated by a Field Programmable Gate Array
clocked at 24 MHz). The filter and the distortion stages are genuine analogue
circuits per voice.

- **Polyphony:** 8 voices.
- **Timbrality:** monotimbral. The Peak plays one patch at a time. It is not
  multitimbral; it does not split into independent parts on different MIDI
  channels. This is confirmed by the Sound on Sound review.
- **Audio outputs:** stereo main outputs plus a headphone socket.
- **MIDI ports:** full-size 5-pin DIN MIDI In, Out, and Thru, plus USB MIDI
  (class-compliant; this is the `1235:007e` device). There are also two pedal
  inputs and a control-voltage (CV) modulation input (mini-jack).

The Summit is essentially a two-Peak, keyboard, bi-timbral sibling. Much of the
MIDI implementation is shared, which is why the best CC reference, midi.guide,
documents "Summit and Peak" together.

### Synth architecture

**Oscillators (3 per voice):** three digital NCOs. Each oscillator provides the
standard analogue-style waveforms plus access to 17 wavetables (5 waves each).
There is an FM/oscillator-modulation system; the "mod 1 / mod 2" sources seen in
the CC list feed oscillator pitch and shape. Additional sources in the mixer are
ring modulation (oscillator 1 times oscillator 2) and white noise (with an
optional low-pass on the noise). Per-oscillator controls include coarse and fine
tune, range, shape (wave/wavetable position), and virtual oscillator sync
(Vsync). Voice-spread controls (divergence, drift) detune voices for
analogue-style movement.

**Filter:** one resonant analogue multi-mode filter per voice, a state-variable
operational-transconductance-amplifier (OTA) design. Modes: low-pass, band-pass,
high-pass. Slopes: 12 dB/oct and 24 dB/oct. There are drive stages around the
filter (overdrive before it, post-drive after it) that contribute to the
analogue character.

**Envelopes:** three ADSR envelopes per voice. One is hard-wired to the
amplifier (the amp envelope); the other two are assignable "modulation"
envelopes (commonly used for the filter and for oscillator/FM modulation).

**LFOs:** two low-frequency oscillators (LFOs) per voice. Each LFO has
free-running and tempo-synced rates, a fade-in, range, and selectable shapes.
Both can be routed widely through the modulation system.

**Modulation matrix:** a 16-slot modulation matrix, plus 16 "direct" assignments
available straight from the front panel. Sources include the LFOs, the mod
envelopes, velocity, the mod wheel, aftertouch (the Peak generates polyphonic
aftertouch and can also use channel aftertouch as a source), and more.
Destinations span oscillator pitch/shape, filter, levels, and effects.

**Animate:** two momentary "Animate" performance buttons (Animate 1 and
Animate 2) that trigger pre-assigned modulation while held, for live
"hold-to-mangle" gestures. They are exposed over MIDI as "Animate 1 hold"
(CC114) and "Animate 2 hold" (CC115).

**Effects:** true analogue distortion with three separate distortion points per
voice, followed by a digital effects section: chorus, delay, and reverb. Each
effect has level plus its own parameters (chorus rate/feedback, delay
time/feedback, reverb time).

**Glide / portamento:** glide on/off and glide time, with the usual mono/legato
behaviour.

**Voice / play modes:** poly, mono, and "Mono 2" (a new voice is allocated per
note). Unison-style stacking and the divergence/drift spread give thick, detuned
sounds.

### MIDI implementation

#### Channel and global behaviour

- **Receive channel:** a single active MIDI channel; the factory default is
  **channel 1**. It is not Omni by default. Drive the stress test on channel 1.
- The Settings menu independently toggles: MIDI channel, Arp MIDI out,
  Aftertouch, CC transmit/receive, NRPN transmit/receive, and Program/Bank
  Change transmit/receive. **By default all MIDI transmit/receive options are
  ON.** Out of the box, the Peak receives CC, NRPN, program change, and bank
  change on channel 1.

#### Continuous controllers (CC)

- The listed CCs are **7-bit** (value 0 to 127). The Peak maps most front-panel
  parameters to a dedicated CC. See `midi-cc-map.json` for the full table; 73
  device-specific CCs are documented there, plus CC1 (mod wheel).
- Key musical CCs (cross-checked across two sources): filter **cutoff = CC29**,
  filter **resonance = CC79**, oscillator mix levels **CC23/24/25**, ring-mod
  mix CC26, noise mix CC27, amp envelope **A/D/S/R = CC86/87/88/89**, mod
  envelope 1 A/D/S/R = CC90/91/92/93, mod envelope 2 = CC94/95/117/103
  (attack/decay/sustain/release; note the non-contiguous sustain CC117 and
  release CC103), **LFO 1 rate = CC30**, **LFO 2 rate = CC31**, glide time CC5,
  distortion level CC104, chorus level CC105, delay level CC108, reverb level
  CC112.
- **Finer-than-7-bit resolution.** Two methods are referenced and are not
  individually verified here, so treat them as lower confidence:
  - midi.guide states extra resolution is available via NRPN (14-bit).
  - the Squarp community definition describes a dual-CC scheme where a second CC
    offset by +32 from the base (for example cutoff CC29 paired with CC61, LFO 1
    rate CC30 with CC62, osc 1 mix CC23 with CC55) extends or biases bipolar
    parameters. For a stress test, plain 7-bit CCs are the safe, well-attested
    path; only use the +32 pairs or NRPN after confirming them on hardware.

#### Pitch bend, aftertouch, program/bank change

- **Pitch bend:** supported; range adjustable in semitone steps up to +/-2
  octaves, default +/-1 octave.
- **Aftertouch:** the Peak both generates polyphonic aftertouch and accepts
  aftertouch as a mod source (channel aftertouch is the common received form).
  Aftertouch receive is on by default.
- **Program change / bank select:** standard MIDI Program Change selects patches;
  Bank Select uses **CC0 (bank MSB)** and **CC32 (bank LSB)** in the normal MIDI
  fashion, sent before the Program Change. Program/Bank change receive is on by
  default. The exact bank MSB/LSB numbers per Peak patch bank were not confirmed
  from an official chart, so verify on hardware before relying on specific bank
  numbers.

#### Known quirks for dense CC streams

No official maximum CC rate is documented in the sources consulted. General
hardware-synth caution applies: a flood of high-rate CC (especially many
parameters at once, or NRPN bursts) can saturate the 5-pin DIN MIDI link
(about 31.25 kbaud, about 1 ms per 3-byte message) and cause lag or
dropped/stuck values. USB MIDI has far more headroom than DIN. For a stress
test, prefer USB, throttle or de-duplicate identical consecutive CC values, and
keep per-parameter update rates modest (for example coalesce to roughly 100 to
200 messages per second total rather than sending every parameter every tick).
This is general guidance, not a Peak-specific published limit; it is flagged as
such.

### Patch / bank / program-change structure

Patches are selected with standard MIDI **Program Change** messages. To reach a
specific bank, send **Bank Select MSB = CC0** and **Bank Select LSB = CC32**
before the Program Change, per the MIDI standard. The Peak has Program/Bank
Change receive enabled by default on its active channel (channel 1). The precise
bank-number-to-patch-bank mapping is a hardware-confirmable detail and is not
asserted here.

### Recommended CCs for continuous sweeps

These CCs are the most musically expressive and generally safe to sweep
continuously on channel 1 (see `midi-cc-map.json`,
`_meta.recommended_for_stress_test`):

| CC  | Parameter                | Caution |
|-----|--------------------------|---------|
| 29  | Filter cutoff            | safe, very expressive |
| 79  | Filter resonance         | can self-oscillate / get loud at high values |
| 1   | Mod wheel                | safe |
| 28  | LFO 1 -> filter depth    | safe |
| 30  | LFO 1 rate               | safe |
| 104 | Distortion level         | can get loud near the top |
| 105 | Chorus level             | safe |
| 108 | Delay level              | safe |
| 112 | Reverb level             | safe |

### Confidence note

The primary CC table comes from midi.guide, a community-maintained but
well-regarded reference. It was **cross-checked against a second source** (the
Squarp community Peak definition) for the core musical parameters (cutoff,
resonance, oscillator levels, envelopes, LFO rates, FX, glide), which agree.
Overall confidence in the 7-bit CC numbers is **high**. The bundled official
Novation Peak user-guide PDF contains a MIDI chart, but it is image-based and
could not be machine-extracted to verify each label verbatim, so individual
labels are high-but-not-certified rather than certified-from-official-chart. The
NRPN numbers and the +32 dual-CC pairing are **not** individually verified and
are flagged low confidence in the JSON. No CC number was invented; every entry
traces to a cited source.

---

## Files

- `src/`: the module source. The main entry point is `src/index.js`.
- `midi-cc-map.json`: the bundled Peak CC table, with `_meta` and `cc_map`.
- `examples/`: copy-pasteable usage examples.
- `test/`: the test suite, run with `npm test`.
- `sources.md`: source URLs and per-source reliability notes for the CC data.

See `sources.md` for the full list of references behind the hardware reference
and the CC map.
