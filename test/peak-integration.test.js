const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const peak = require("../js/konduktiva-peak/src/index.js");
const sdk = require("../js/shared-synth-sdk/index.js");

test("shared-synth-sdk exports all 5 synthesizer profiles, 8 accents, and Novation Peak schemas", () => {
  assert.equal(sdk.ACCENT_PROFILES.length, 8);
  assert.ok(sdk.ACCENT_PROFILES.includes("PeakOxfordBlue"));
  assert.equal(sdk.PEAK_OXFORD_BLUE_HEX, "#1A73E8");
  assert.equal(sdk.PEAK_AMBER_OLED_HEX, "#FF8C00");

  assert.ok(sdk.validateDeviceProfile(sdk.NOVATION_PEAK_PROFILE));
  assert.ok(sdk.validateThemeConfig(sdk.NOVATION_PEAK_THEME));

  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "peak-sdk-schemas-"));
  try {
    sdk.exportAllSchemas(tmpDir);
    assert.ok(fs.existsSync(path.join(tmpDir, "novation-peak.profile.json")));
    assert.ok(fs.existsSync(path.join(tmpDir, "novation-peak.theme.json")));
    assert.ok(fs.existsSync(path.join(tmpDir, "clavia-nord-drum-2.profile.json")));
    assert.ok(fs.existsSync(path.join(tmpDir, "novation-bass-station-2.profile.json")));
    assert.ok(fs.existsSync(path.join(tmpDir, "asm-hydrasynth.profile.json")));
    assert.ok(fs.existsSync(path.join(tmpDir, "korg-nanokontrol2.profile.json")));
  } finally {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
});

test("PeakMidiRouter and PeakHardwareDiscovery enforce 8-voice polyphony, safe resonance clamp, and virtual loopback", () => {
  const disc = sdk.PeakHardwareDiscovery.discoverNonInvasive();
  assert.equal(disc.safetyMode, sdk.MidiSafetyMode.VIRTUAL_LOOPBACK);
  assert.equal(disc.peakUsb.rawmidiDevice, "hw:3,0,0");
  assert.equal(disc.peakUsb.vendorId, "1235");
  assert.equal(disc.peakUsb.productId, "007e");
  assert.equal(disc.peakUsb.isVirtualMock, true);

  const router = new sdk.PeakMidiRouter();
  for (let i = 0; i < 8; i++) {
    const pkts = router.routeNoteOn(60 + i, 100);
    assert.equal(pkts.length, 1);
    assert.equal(pkts[0].type, "note_on");
  }
  assert.equal(router.activeVoices.length, 8);

  // 9th note steals oldest voice (60)
  const stealPkts = router.routeNoteOn(72, 110);
  assert.equal(stealPkts.length, 2);
  assert.equal(stealPkts[0].type, "note_off");
  assert.equal(stealPkts[0].note, 60);
  assert.equal(stealPkts[1].type, "note_on");
  assert.equal(stealPkts[1].note, 72);

  // Resonance CC 79 is clamped to <= 62
  const resoPkts = router.routeCc(79, "Filter resonance", 125);
  assert.equal(resoPkts[0].value, 62);

  // Animate 1 hold (CC 114) and 130-packet panic
  const animPkt = router.routeAnimateHold(1, true);
  assert.equal(animPkt.cc, 114);
  assert.equal(animPkt.value, 127);

  const panicPkts = router.routePanic();
  assert.equal(panicPkts.length, 130);
  assert.equal(router.activeVoices.length, 0);
});

test("createPeakOscBridge forwards Note, CC, and Density Graph events to SharedSynthClient over UDP OSC", async () => {
  const receiver = new sdk.SharedSynthClient({ localPort: 0, autoSubscribe: false });
  const listenPort = await receiver.open();
  const sender = new sdk.SharedSynthClient({
    targetHost: "127.0.0.1",
    targetPort: listenPort,
    localPort: 0,
    autoSubscribe: false,
  });
  await sender.open();

  const received = [];
  receiver.on("message", (msg) => received.push(msg));

  try {
    const bridge = peak.createPeakOscBridge({ client: sender });
    await bridge.sendNoteOn(60, 100, 1);
    const clampedReso = await bridge.sendCc(79, 120, "Filter resonance", 1);
    assert.equal(clampedReso, 62);
    await bridge.setDensityGraph("peakBuild");
    await bridge.sendNoteOff(60, 0, 1);

    await new Promise((r) => setTimeout(r, 30));
    assert.equal(bridge.stats.noteon, 1);
    assert.equal(bridge.stats.noteoff, 1);
    assert.equal(bridge.stats.cc, 1);
    assert.equal(bridge.stats.densityChanges, 1);
    assert.ok(received.some((m) => m.address === "/synth/peak/cc" && m.args[1] === 62));
    assert.ok(received.some((m) => m.address === "/synth/konduktiva/density_graph" && m.args[0] === "peakBuild"));
  } finally {
    await sender.close();
    await receiver.close();
  }
});
