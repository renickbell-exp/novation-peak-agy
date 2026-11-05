# Sources consulted — Novation Peak

Accessed 2026-06-26.

## MIDI / CC (primary, for the CC map)

- **midi.guide — "Novation Summit and Peak MIDI CCs and NRPNs"**
  https://midi.guide/d/novation/summit-and-peak/
  Primary source for the full CC table. Community-maintained MIDI database, well
  regarded and internally consistent. Lists 70+ device-specific CCs with
  parameter names and ranges, and states extra resolution is available via
  14-bit NRPN. Reliability: high for the 7-bit CC list; the NRPN claim is
  stated but the specific NRPN numbers were not enumerated here.

- **Squarp community — "Novation Peak CC Definition"**
  https://squarp.community/t/novation-peak-cc-definition/952
  Second source, used to cross-check the core musical CCs (cutoff 29,
  resonance 79, osc mix 23/24/25, amp env 86-89, LFO rates 30/31, FX
  104/105/108/112, glide 5) — all agree with midi.guide. Also describes a
  dual-CC scheme (a second CC offset by +32 for bipolar/extended range) and
  notes the definition file's receive channel is 1. Reliability: medium-high
  (user-contributed controller-definition file); the +32 pairing is the
  author's mapping and is not independently confirmed.

## Architecture and specifications

- **Sound on Sound — Novation Peak review**
  https://www.soundonsound.com/reviews/novation-peak
  Used to confirm: 8-voice, monotimbral; three NCOs per voice via 24 MHz FPGA;
  17 wavetables (5 waves each); ring mod and filtered noise; analogue
  state-variable filter with LP/BP/HP and 12/24 dB slopes; three ADSR
  envelopes; two LFOs with sync; chorus/delay/reverb plus distortion; poly /
  mono / Mono2 play modes; glide, Vsync, divergence/drift; stereo outs,
  headphones, MIDI In/Out/Thru, USB MIDI, two pedal inputs, CV mod input.
  Reliability: high (established, detailed editorial review).

- **Novation product page — Peak**
  https://novationmusic.com/products/peak
  Official marketing/spec page. Reliability: high for headline specs, light on
  MIDI detail.

- **Amazon / B&H / Perfect Circuit listings**
  https://www.amazon.com/Novation-8-Voice-Desktop-Polyphonic-Synthesizer/dp/B06Y4FYPNY
  https://www.bhphotovideo.com/c/product/1333497-REG/nova_peak_8_voice_polyphonic_synthesizer.html
  https://www.perfectcircuit.com/novation-peak-eight-voice-polyphonic-synthesiser.html
  Used to corroborate the 16-slot modulation matrix, 2 LFOs per voice, and
  "3 analogue distortion points." Reliability: medium (retailer copy, usually
  derived from Novation's own spec sheet).

## Official user guide (MIDI settings behaviour)

- **Novation Peak User Guide (PDF mirror, Kraft Music)**
  https://files.kraftmusic.com/media/ownersmanual/Novation_Peak_User_Guide.pdf
  The official user guide. It contains a MIDI implementation chart, but the
  bundled PDF is image-based and did not yield machine-extractable CC text in
  this session, so it could not be used to certify each CC label verbatim.
  Reliability: high (official) but not machine-verified here.

- **Novation official User Guides portal — "Novation Peak in detail" /
  "Getting started with Novation Peak"**
  https://userguides.novationmusic.com/hc/en-gb/articles/25494693870354-Novation-Peak-in-detail
  https://userguides.novationmusic.com/hc/en-gb/articles/25494651214098-Getting-started-with-Novation-Peak
  Official online documentation. Returned HTTP 403 to automated fetch in this
  session, so they were not read directly; listed for completeness/manual
  follow-up. Reliability: high (official) but not retrieved here.

- **Spectrasonics Omnisphere Hardware Library — "Setting up the Peak"**
  https://support.spectrasonics.net/manual/OmnisphereHW/2.5/en/topic/novation-peak
  Used to confirm MIDI-settings behaviour: factory default is all MIDI
  transmit/receive ON and MIDI Channel 1 active; Settings menu exposes MIDI
  channel, Aftertouch, CC/NRPN, and Program/Bank Change toggles; pitch-bend
  range adjustable up to +/-2 octaves (default +/-1 octave); aftertouch
  (channel and polyphonic) handling. Reliability: high (third-party but
  precise integration documentation derived from the Peak manual).
