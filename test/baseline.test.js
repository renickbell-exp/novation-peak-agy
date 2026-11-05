const test = require("node:test");
const assert = require("node:assert/strict");
const peak = require("../js/konduktiva-peak/src/index.js");
const sdk = require("../js/shared-synth-sdk/index.js");

test("konduktiva-peak and shared-synth-sdk load cleanly in workspace", () => {
  const ccMap = peak.loadCcMap();
  assert.ok(Array.isArray(ccMap.cc_map));
  assert.equal(ccMap.cc_map.length, 74);
  assert.equal(peak.resolveCc("cutoff"), 29);
  assert.equal(peak.resolveCc("resonance"), 79);
  assert.ok(sdk.SynthClient);
});
