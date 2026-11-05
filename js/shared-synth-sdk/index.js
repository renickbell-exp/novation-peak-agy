// SPDX-License-Identifier: GPL-3.0-or-later
/**
 * @file Reusable Zero-Dependency Node.js Shared Synth SDK.
 * Usable across nkEditor, ASM Hydrasynth Editor, and bs2-td3-manager.
 */

export * from './osc-codec.js';
export * from './synth-client.js';
import { SharedSynthClient } from './synth-client.js';
export const SynthClient = SharedSynthClient;
export * from './nord-drum-router.js';
export * from './nord-drum-sysex.js';
export * from './schema-validator.js';

/**
 * Standard default UDP ports matching docs/BS2-TD3-MANAGER-SPEC.md.
 */
export const DEFAULT_PORTS = {
  HUB_LISTEN: 9100,
  HUB_BROADCAST: 9101,
};

/**
 * All 15 yaae / twoBowls theme names supported across companion apps.
 */
export const THEMES = [
  'Default',
  'Light',
  'Dark',
  'SolarizedDark',
  'Monokai',
  'Dracula',
  'Nord',
  'GruvboxDark',
  'LightGrayscale',
  'DarkGrayscale',
  'HighContrastBw',
  'HighContrastColor',
  'HighContrastLight',
  'CreamLowContrast',
  'MediumContrast',
];

/**
 * 4 TwoBowls physical backdrop presets.
 */
export const TWO_BOWLS_BACKGROUNDS = [
  'White',
  'Cream',
  'Black',
  'Dark Grey',
];

/**
 * 7 Synth and editor accent profiles.
 */
export const ACCENT_PROFILES = [
  'Midnight',
  'Cyberpunk',
  'Arctic',
  'Volcanic',
  'BassStationBlue',
  'AcidYellowTD3',
  'NordCrimson',
];

/**
 * Clavia Nord Drum 2 hardware chassis crimson red accent hex (#E52521).
 */
export const NORD_CRIMSON_HEX = '#E52521';

/**
 * Clavia Nord Drum 2 hardware high-contrast amber LED readout hex (#FFB300).
 */
export const NORD_AMBER_LED_HEX = '#FFB300';

/**
 * 5 TD-3 Acid pattern styles.
 */
export const ACID_STYLES = [
  'ClassicPhuture303',
  'SquelchAcidTd3Md',
  'Polymetric7_9_11',
  'DetroitSubElectro',
  'HypnoticMinimal',
];
