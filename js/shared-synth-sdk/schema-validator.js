// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * @file Unified JSON Schema Specifications, Canonical Profiles, and Validators
 * across all 4 Synthesizer Projects (nkEditor, ASM Hydrasynth, BS2 + TD-3, and Nord Drum 2).
 */

import fs from 'node:fs';
import path from 'node:path';

/**
 * Standard Draft-07 JSON Schema for SynthPatch.
 */
export const SYNTH_PATCH_SCHEMA = Object.freeze({
  $schema: 'http://json-schema.org/draft-07/schema#',
  title: 'SynthPatch',
  description: 'Standard cross-project synthesizer patch document',
  type: 'object',
  required: ['meta', 'parameters'],
  properties: {
    meta: {
      type: 'object',
      required: ['id', 'name'],
      properties: {
        id: { type: 'string' },
        name: { type: 'string' },
        category: { type: 'string', default: 'Init' },
        color: { type: 'string', pattern: '^#[0-9a-fA-F]{6}$', default: '#888888' },
        favorite: { type: 'boolean', default: false },
        bank: { type: 'string', default: 'A' },
        slot: { type: 'integer', minimum: 0, default: 0 },
        createdAt: { type: 'string', format: 'date-time' },
        modifiedAt: { type: 'string', format: 'date-time' },
        tags: { type: 'array', items: { type: 'string' }, default: [] },
        author: { type: ['string', 'null'] },
        notes: { type: ['string', 'null'] },
      },
    },
    parameters: {
      type: 'object',
      description: 'Key-value map of parameter identifiers to numbers',
      additionalProperties: { type: 'number' },
    },
    sysex: {
      type: 'object',
      properties: {
        rawDump: {
          type: ['array', 'null'],
          items: { type: 'integer', minimum: 0, maximum: 255 },
        },
      },
    },
  },
});

/**
 * Standard Draft-07 JSON Schema for DeviceSynthesizerProfile.
 */
export const DEVICE_PROFILE_SCHEMA = Object.freeze({
  $schema: 'http://json-schema.org/draft-07/schema#',
  title: 'DeviceSynthesizerProfile',
  description: 'Device synthesis engine and controller layout specification',
  type: 'object',
  required: ['id', 'name', 'manufacturer', 'version', 'category', 'default_midi_channel', 'parameters'],
  properties: {
    id: { type: 'string' },
    name: { type: 'string' },
    manufacturer: { type: 'string' },
    version: { type: 'string' },
    category: {
      type: 'string',
      enum: ['synthesizer', 'controller_surface', 'hybrid'],
    },
    default_midi_channel: { type: 'integer', minimum: 0, maximum: 15 },
    parameters: {
      type: 'array',
      items: {
        type: 'object',
        required: ['id', 'name', 'module', 'address', 'range'],
        properties: {
          id: { type: 'string' },
          name: { type: 'string' },
          module: { type: 'string' },
          address: { type: 'object' },
          range: { type: 'object' },
          curve: { type: 'string' },
          is_locked: { type: 'boolean' },
        },
      },
    },
    modules: {
      type: 'array',
      items: { type: 'string' },
    },
    controller_controls: {
      type: 'array',
      items: {
        type: 'object',
        required: ['id', 'name', 'kind', 'cc', 'has_led', 'group'],
        properties: {
          id: { type: 'string' },
          name: { type: 'string' },
          kind: { type: 'string' },
          cc: { type: 'integer', minimum: 0, maximum: 127 },
          channel: { type: ['integer', 'null'] },
          has_led: { type: 'boolean' },
          led_cc: { type: ['integer', 'null'] },
          track_index: { type: ['integer', 'null'] },
          group: { type: 'string' },
        },
      },
    },
    tags: {
      type: 'array',
      items: { type: 'string' },
    },
  },
});

/**
 * Standard Draft-07 JSON Schema for ControllerSurfaceMapping.
 */
export const CONTROLLER_MAPPING_SCHEMA = Object.freeze({
  $schema: 'http://json-schema.org/draft-07/schema#',
  title: 'ControllerSurfaceMapping',
  description: 'Tactile surface hardware binding configuration across switchable banks',
  type: 'object',
  required: ['surface_name', 'version', 'banks'],
  properties: {
    surface_name: { type: 'string' },
    version: { type: 'string' },
    active_bank: { type: 'integer', minimum: 1, maximum: 3 },
    banks: {
      type: 'array',
      items: {
        type: 'object',
        required: ['bank_index', 'name', 'target_mode', 'controls'],
        properties: {
          bank_index: { type: 'integer' },
          name: { type: 'string' },
          target_mode: { type: 'string' },
          controls: {
            type: 'array',
            items: {
              type: 'object',
              required: ['hardware_cc', 'label', 'action'],
              properties: {
                hardware_cc: { type: 'integer', minimum: 0, maximum: 127 },
                label: { type: 'string' },
                action: { type: 'string' },
                target_device: { type: 'string' },
                target_param: { type: 'string' },
                led_feedback_cc: { type: ['integer', 'null'] },
              },
            },
          },
        },
      },
    },
  },
});

/**
 * Standard Draft-07 JSON Schema for ThemeConfig.
 */
export const THEME_CONFIG_SCHEMA = Object.freeze({
  $schema: 'http://json-schema.org/draft-07/schema#',
  title: 'ThemeConfig',
  description: 'Unified visualizer and editor theme palette configuration across all 4 synth projects',
  type: 'object',
  required: ['preference', 'accent', 'overrides', 'scale', 'true_black_and_white'],
  properties: {
    preference: { type: 'string' },
    two_bowls_bg: { type: ['string', 'null'] },
    accent: { type: 'string' },
    overrides: { type: 'object' },
    scale: { type: 'number', minimum: 0.5, maximum: 2.0 },
    true_black_and_white: { type: 'boolean' },
  },
});

// =============================================================================
// Canonical Device Profiles across 4 Synthesizer Projects
// =============================================================================

export const ASM_HYDRASYNTH_PROFILE = Object.freeze({
  id: 'asm_hydrasynth',
  name: 'ASM Hydrasynth Keyboard/Desktop',
  manufacturer: 'Ashun Sound Machines',
  version: '2.0.0',
  category: 'synthesizer',
  default_midi_channel: 0,
  modules: ['Osc1', 'Osc2', 'Osc3', 'Mutator1', 'Mutator2', 'Filter1', 'Filter2', 'Amp', 'Lfo1', 'Env1'],
  parameters: [
    {
      id: 'filter_1_cutoff',
      name: 'Filter 1 Cutoff',
      module: 'Filter1',
      address: { kind: 'Cc', cc: 74 },
      range: { min: 0.0, max: 127.0, step: 1.0, default_value: 64.0 },
      curve: 'Logarithmic',
      is_locked: false,
    },
    {
      id: 'filter_1_resonance',
      name: 'Filter 1 Resonance',
      module: 'Filter1',
      address: { kind: 'Cc', cc: 71 },
      range: { min: 0.0, max: 127.0, step: 1.0, default_value: 0.0 },
      curve: 'Linear',
      is_locked: false,
    },
  ],
  controller_controls: [],
  tags: ['wavetable', 'polyphonic', 'hydrasynth'],
});

export const KORG_NANOKONTROL2_PROFILE = Object.freeze({
  id: 'korg_nanokontrol2',
  name: 'Korg nanoKONTROL2',
  manufacturer: 'Korg',
  version: '1.0.0',
  category: 'controller_surface',
  default_midi_channel: 0,
  modules: ['Faders', 'Knobs', 'Solo', 'Mute', 'Record', 'Transport'],
  parameters: [],
  controller_controls: [
    { id: 'fader_1', name: 'Fader 1', kind: 'Fader', cc: 0, channel: 0, has_led: false, led_cc: null, track_index: 0, group: 'track_1' },
    { id: 'knob_1', name: 'Knob 1', kind: 'Knob', cc: 16, channel: 0, has_led: false, led_cc: null, track_index: 0, group: 'track_1' },
    { id: 'solo_1', name: 'Solo 1', kind: 'Button', cc: 32, channel: 0, has_led: true, led_cc: 32, track_index: 0, group: 'track_1' },
    { id: 'mute_1', name: 'Mute 1', kind: 'Button', cc: 48, channel: 0, has_led: true, led_cc: 48, track_index: 0, group: 'track_1' },
  ],
  tags: ['surface', 'tactile', 'nkeditor'],
});

export const NOVATION_BS2_PROFILE = Object.freeze({
  id: 'novation_bass_station_2',
  name: 'Novation Bass Station II',
  manufacturer: 'Novation',
  version: '2.5.0',
  category: 'synthesizer',
  default_midi_channel: 0,
  modules: ['Osc1', 'Osc2', 'SubOsc', 'Filter', 'EnvMod', 'EnvAmp', 'Lfo1', 'Lfo2', 'Effects', 'Arp'],
  parameters: [
    {
      id: 'filter_frequency',
      name: 'Filter Frequency',
      module: 'Filter',
      address: { kind: 'Cc', cc: 16 },
      range: { min: 0.0, max: 255.0, step: 1.0, default_value: 128.0 },
      curve: 'Logarithmic',
      is_locked: false,
    },
    {
      id: 'filter_resonance',
      name: 'Filter Resonance',
      module: 'Filter',
      address: { kind: 'Cc', cc: 82 },
      range: { min: 0.0, max: 127.0, step: 1.0, default_value: 0.0 },
      curve: 'Linear',
      is_locked: false,
    },
  ],
  controller_controls: [],
  tags: ['analog', 'monosynth', 'bass-station'],
});

export const BEHRINGER_TD3_PROFILE = Object.freeze({
  id: 'behringer_td3_md',
  name: 'Behringer TD-3-MO / TD-3',
  manufacturer: 'Behringer',
  version: '1.0.0',
  category: 'synthesizer',
  default_midi_channel: 1,
  modules: ['VCO', 'VCF', 'Envelope', 'Overdrive', 'SubOsc', 'Muffler'],
  parameters: [
    {
      id: 'filter_cutoff',
      name: 'Filter Cutoff',
      module: 'VCF',
      address: { kind: 'Cc', cc: 74 },
      range: { min: 0.0, max: 127.0, step: 1.0, default_value: 64.0 },
      curve: 'Logarithmic',
      is_locked: false,
    },
    {
      id: 'filter_resonance',
      name: 'Filter Resonance',
      module: 'VCF',
      address: { kind: 'Cc', cc: 71 },
      range: { min: 0.0, max: 127.0, step: 1.0, default_value: 0.0 },
      curve: 'Linear',
      is_locked: false,
    },
  ],
  controller_controls: [],
  tags: ['acid', 'bassline', 'td-3'],
});

export const NORD_DRUM_2_PROFILE = Object.freeze({
  id: 'clavia_nord_drum_2',
  name: 'Clavia Nord Drum 2',
  manufacturer: 'Clavia DMI AB',
  version: '3.0.0',
  category: 'synthesizer',
  default_midi_channel: 9, // Channel 10
  modules: ['Voice1', 'Voice2', 'Voice3', 'Voice4', 'Voice5', 'Voice6', 'Global'],
  parameters: [
    {
      id: 'voice_1_tone_wave',
      name: 'Voice 1 Tone Wave',
      module: 'Voice1',
      address: { kind: 'Cc', cc: 46 },
      range: { min: 0.0, max: 127.0, step: 1.0, default_value: 0.0 },
      curve: 'Linear',
      is_locked: false,
    },
    {
      id: 'voice_1_tone_decay',
      name: 'Voice 1 Tone Decay',
      module: 'Voice1',
      address: { kind: 'Cc', cc: 50 },
      range: { min: 0.0, max: 127.0, step: 1.0, default_value: 64.0 },
      curve: 'Linear',
      is_locked: false,
    },
    {
      id: 'voice_1_noise_filter_freq',
      name: 'Voice 1 Noise Filter Freq',
      module: 'Voice1',
      address: { kind: 'Cc', cc: 14 },
      range: { min: 0.0, max: 127.0, step: 1.0, default_value: 64.0 },
      curve: 'Linear',
      is_locked: false,
    },
    {
      id: 'global_focus_cc',
      name: 'Global Focus Voice Selection',
      module: 'Global',
      address: { kind: 'Cc', cc: 70 },
      range: { min: 0.0, max: 5.0, step: 1.0, default_value: 0.0 },
      curve: 'Linear',
      is_locked: false,
    },
  ],
  controller_controls: [
    { id: 'ch_1_button', name: 'Channel 1 Select', kind: 'Button', cc: 70, channel: 9, has_led: true, led_cc: null, track_index: 0, group: 'channel_selection' },
    { id: 'dial_main', name: 'Main Value Dial', kind: 'Encoder', cc: 15, channel: 9, has_led: false, led_cc: null, track_index: null, group: 'global_controls' },
  ],
  tags: ['percussion', 'drum-synthesizer', 'nord-drum-2'],
});

export const NOVATION_PEAK_PROFILE = Object.freeze({
  id: 'novation_peak',
  name: 'Novation Peak',
  manufacturer: 'Focusrite / Novation',
  version: '2.1.0',
  category: 'synthesizer',
  default_midi_channel: 0,
  modules: [
    'Osc1',
    'Osc2',
    'Osc3',
    'Mixer',
    'Filter',
    'EnvAmp',
    'EnvMod',
    'Lfo1',
    'Lfo2',
    'ArpVoice',
    'Effects',
    'ModMatrix',
  ],
  parameters: [
    {
      id: 'filter_cutoff_frequency',
      name: 'Filter cutoff (Frequency)',
      module: 'Filter',
      address: { kind: 'Cc', cc: 29 },
      range: { min: 0.0, max: 127.0, step: 1.0, default_value: 96.0 },
      curve: 'Logarithmic',
      is_locked: false,
    },
    {
      id: 'filter_resonance',
      name: 'Filter resonance',
      module: 'Filter',
      address: { kind: 'Cc', cc: 79 },
      range: { min: 0.0, max: 127.0, step: 1.0, default_value: 20.0 },
      curve: 'Linear',
      is_locked: false,
    },
    {
      id: 'mod_wheel_modulation',
      name: 'Mod wheel (modulation)',
      module: 'ArpVoice',
      address: { kind: 'Cc', cc: 1 },
      range: { min: 0.0, max: 127.0, step: 1.0, default_value: 0.0 },
      curve: 'Linear',
      is_locked: false,
    },
    {
      id: 'reverb_level',
      name: 'Reverb level',
      module: 'Effects',
      address: { kind: 'Cc', cc: 112 },
      range: { min: 0.0, max: 127.0, step: 1.0, default_value: 32.0 },
      curve: 'Linear',
      is_locked: false,
    },
    {
      id: 'delay_level',
      name: 'Delay level',
      module: 'Effects',
      address: { kind: 'Cc', cc: 108 },
      range: { min: 0.0, max: 127.0, step: 1.0, default_value: 24.0 },
      curve: 'Linear',
      is_locked: false,
    },
  ],
  controller_controls: [
    { id: 'animate_1', name: 'Animate 1 Hold', kind: 'Button', cc: 114, channel: 0, has_led: true, led_cc: 114, track_index: 0, group: 'performance' },
    { id: 'animate_2', name: 'Animate 2 Hold', kind: 'Button', cc: 115, channel: 0, has_led: true, led_cc: 115, track_index: 1, group: 'performance' },
  ],
  tags: ['hybrid', 'polyphonic', 'fpga-oxford-oscillators', 'novation-peak'],
});

// =============================================================================
// Canonical Themes across 4 Synthesizer Projects
// =============================================================================

export const ASM_HYDRASYNTH_THEME = Object.freeze({
  preference: 'Slate',
  two_bowls_bg: 'SlateStudio',
  accent: 'HydraOrange',
  overrides: { accent: '#FF7700', panel: '#1E2228', led_readout: '#FF8800' },
  scale: 1.0,
  true_black_and_white: false,
});

export const NKEDITOR_THEME = Object.freeze({
  preference: 'Default',
  two_bowls_bg: 'ZenMonochrome',
  accent: 'MinimalWhite',
  overrides: { accent: '#F0F0F0', panel: '#141414', led_readout: '#FFFFFF' },
  scale: 1.0,
  true_black_and_white: false,
});

export const BS2_TD3_THEME = Object.freeze({
  preference: 'Dark',
  two_bowls_bg: 'DeepNight',
  accent: 'AcidGreen',
  overrides: { accent: '#39FF14', panel: '#0F1210', led_readout: '#39FF14' },
  scale: 1.0,
  true_black_and_white: false,
});

export const NORD_DRUM_2_THEME = Object.freeze({
  preference: 'Nord',
  two_bowls_bg: 'WarmPaper',
  accent: 'NordCrimson',
  overrides: { accent: '#E52521', panel: '#1C1616', led_readout: '#FFB300' },
  scale: 1.0,
  true_black_and_white: false,
});

export const NOVATION_PEAK_THEME = Object.freeze({
  preference: 'SolarizedDark',
  two_bowls_bg: 'SlateStudio',
  accent: 'PeakOxfordBlue',
  overrides: { accent: '#1A73E8', panel: '#121822', led_readout: '#FF8C00' },
  scale: 1.0,
  true_black_and_white: false,
});

// =============================================================================
// Validator Functions
// =============================================================================

/**
 * Validates a SynthPatch object against the patch schema.
 * @param {Object} patch
 * @returns {boolean}
 */
export function validateSynthPatch(patch) {
  if (!patch || typeof patch !== 'object') {
    throw new TypeError('SynthPatch must be a valid non-null object');
  }
  if (!patch.meta || typeof patch.meta !== 'object') {
    throw new TypeError('SynthPatch missing required meta object');
  }
  if (!patch.meta.id || typeof patch.meta.id !== 'string') {
    throw new TypeError('SynthPatch meta.id must be a string');
  }
  if (!patch.meta.name || typeof patch.meta.name !== 'string') {
    throw new TypeError('SynthPatch meta.name must be a string');
  }
  if (!patch.parameters || typeof patch.parameters !== 'object') {
    throw new TypeError('SynthPatch missing required parameters object');
  }
  return true;
}

/**
 * Validates a DeviceSynthesizerProfile object against the profile schema.
 * @param {Object} profile
 * @returns {boolean}
 */
export function validateDeviceProfile(profile) {
  if (!profile || typeof profile !== 'object') {
    throw new TypeError('DeviceProfile must be a valid non-null object');
  }
  const required = ['id', 'name', 'manufacturer', 'version', 'category', 'default_midi_channel', 'parameters'];
  for (const field of required) {
    if (profile[field] === undefined) {
      throw new TypeError(`DeviceProfile missing required field: ${field}`);
    }
  }
  if (!Array.isArray(profile.parameters)) {
    throw new TypeError('DeviceProfile parameters must be an array');
  }
  return true;
}

/**
 * Validates a ControllerSurfaceMapping object against the mapping schema.
 * @param {Object} mapping
 * @returns {boolean}
 */
export function validateControllerMapping(mapping) {
  if (!mapping || typeof mapping !== 'object') {
    throw new TypeError('ControllerSurfaceMapping must be a valid non-null object');
  }
  if (!mapping.surface_name || typeof mapping.surface_name !== 'string') {
    throw new TypeError('ControllerSurfaceMapping missing surface_name');
  }
  if (!Array.isArray(mapping.banks)) {
    throw new TypeError('ControllerSurfaceMapping banks must be an array');
  }
  return true;
}

/**
 * Validates a ThemeConfig object against the theme schema.
 * @param {Object} theme
 * @returns {boolean}
 */
export function validateThemeConfig(theme) {
  if (!theme || typeof theme !== 'object') {
    throw new TypeError('ThemeConfig must be a valid non-null object');
  }
  const required = ['preference', 'accent', 'overrides', 'scale', 'true_black_and_white'];
  for (const field of required) {
    if (theme[field] === undefined) {
      throw new TypeError(`ThemeConfig missing required field: ${field}`);
    }
  }
  return true;
}

/**
 * Exports all schemas, profiles, and themes across the 4 synthesizer projects.
 * @param {string} outputDir
 */
export function exportAllSchemas(outputDir) {
  fs.mkdirSync(outputDir, { recursive: true });

  fs.writeFileSync(path.join(outputDir, 'synth-patch.schema.json'), JSON.stringify(SYNTH_PATCH_SCHEMA, null, 2));
  fs.writeFileSync(path.join(outputDir, 'device-profile.schema.json'), JSON.stringify(DEVICE_PROFILE_SCHEMA, null, 2));
  fs.writeFileSync(path.join(outputDir, 'controller-mapping.schema.json'), JSON.stringify(CONTROLLER_MAPPING_SCHEMA, null, 2));
  fs.writeFileSync(path.join(outputDir, 'theme-config.schema.json'), JSON.stringify(THEME_CONFIG_SCHEMA, null, 2));

  fs.writeFileSync(path.join(outputDir, 'asm-hydrasynth.profile.json'), JSON.stringify(ASM_HYDRASYNTH_PROFILE, null, 2));
  fs.writeFileSync(path.join(outputDir, 'korg-nanokontrol2.profile.json'), JSON.stringify(KORG_NANOKONTROL2_PROFILE, null, 2));
  fs.writeFileSync(path.join(outputDir, 'novation-bass-station-2.profile.json'), JSON.stringify(NOVATION_BS2_PROFILE, null, 2));
  fs.writeFileSync(path.join(outputDir, 'behringer-td3-md.profile.json'), JSON.stringify(BEHRINGER_TD3_PROFILE, null, 2));
  fs.writeFileSync(path.join(outputDir, 'clavia-nord-drum-2.profile.json'), JSON.stringify(NORD_DRUM_2_PROFILE, null, 2));
  fs.writeFileSync(path.join(outputDir, 'novation-peak.profile.json'), JSON.stringify(NOVATION_PEAK_PROFILE, null, 2));

  fs.writeFileSync(path.join(outputDir, 'asm-hydrasynth.theme.json'), JSON.stringify(ASM_HYDRASYNTH_THEME, null, 2));
  fs.writeFileSync(path.join(outputDir, 'nkeditor.theme.json'), JSON.stringify(NKEDITOR_THEME, null, 2));
  fs.writeFileSync(path.join(outputDir, 'bs2-td3.theme.json'), JSON.stringify(BS2_TD3_THEME, null, 2));
  fs.writeFileSync(path.join(outputDir, 'clavia-nord-drum-2.theme.json'), JSON.stringify(NORD_DRUM_2_THEME, null, 2));
  fs.writeFileSync(path.join(outputDir, 'novation-peak.theme.json'), JSON.stringify(NOVATION_PEAK_THEME, null, 2));
}
