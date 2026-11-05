// SPDX-License-Identifier: GPL-3.0-or-later
//! Linux USB/ALSA hardware identity fingerprinting, multi-device management,
//! and MIDI packet codec (CC, 14-bit CC, NRPN, SysEx, rate limiting, panic).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

/// Standard USB and ALSA hardware fingerprint.
///
/// Discovered via `/sys/class/sound/card*`, `/dev/snd/by-id/`,
/// `/dev/snd/by-path/`, and `/proc/asound/seq/clients`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceFingerprint {
    pub vendor_id: String,
    pub product_id: String,
    pub serial: Option<String>,
    pub card_name: String,
    pub alsa_card_index: u32,
    pub alsa_seq_client: Option<u32>,
    pub alsa_seq_port: Option<u32>,
    pub usb_path: Option<String>,
    pub by_id_path: Option<String>,
    pub device_type: SupportedDeviceType,
}

/// Port-independent hardware fingerprint alias.
pub type HardwareFingerprint = DeviceFingerprint;

impl DeviceFingerprint {
    /// Creates a new device fingerprint with default seq and path options.
    pub fn new(
        vendor_id: impl Into<String>,
        product_id: impl Into<String>,
        serial: Option<String>,
        card_name: impl Into<String>,
        alsa_card_index: u32,
    ) -> Self {
        let v = vendor_id.into();
        let p = product_id.into();
        let c = card_name.into();
        let dev_type = SupportedDeviceType::from_vendor_product(&v, &p, &c);
        Self {
            vendor_id: v,
            product_id: p,
            serial,
            card_name: c,
            alsa_card_index,
            alsa_seq_client: None,
            alsa_seq_port: None,
            usb_path: None,
            by_id_path: None,
            device_type: dev_type,
        }
    }

    /// Builder method attaching ALSA sequencer client and port address.
    pub fn with_seq_address(mut self, client: u32, port: u32) -> Self {
        self.alsa_seq_client = Some(client);
        self.alsa_seq_port = Some(port);
        self
    }

    /// Builder method attaching physical USB topology path.
    pub fn with_usb_path(mut self, path: impl Into<String>) -> Self {
        self.usb_path = Some(path.into());
        self
    }

    /// Builder method attaching stable udev `/dev/snd/by-id/` path.
    pub fn with_by_id_path(mut self, path: impl Into<String>) -> Self {
        self.by_id_path = Some(path.into());
        self
    }

    /// Builder method explicitly overriding detected device type.
    pub fn with_device_type(mut self, device_type: SupportedDeviceType) -> Self {
        self.device_type = device_type;
        self
    }

    /// Formats the port-independent identity key `vendor_id:product_id:serial:card_name`.
    pub fn identity_key(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.vendor_id,
            self.product_id,
            self.serial.as_deref().unwrap_or("none"),
            self.card_name
        )
    }

    /// Returns the detected or assigned supported device type.
    pub fn device_type(&self) -> SupportedDeviceType {
        self.device_type
    }

    /// Checks if this fingerprint matches another device based on hardware identity.
    pub fn matches_hardware(&self, other: &DeviceFingerprint) -> bool {
        if self.vendor_id != other.vendor_id || self.product_id != other.product_id {
            return false;
        }
        match (&self.serial, &other.serial) {
            (Some(s1), Some(s2)) => s1 == s2,
            _ => self.card_name == other.card_name,
        }
    }
}

/// Recognized synthesizer and controller hardware models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SupportedDeviceType {
    NovationBassStation2,
    BehringerTd3,
    BehringerTd3Mo,
    BehringerTd3Md,
    AsmHydrasynth,
    ClaviaNordDrum2,
    KorgNanoKontrol2,
    AkaiMidiMix,
    BehringerUmc1820,
    GenericMidi,
}

impl SupportedDeviceType {
    /// Identifies the device type from USB vendor, product hex strings, and card name.
    pub fn from_vendor_product(vendor: &str, product: &str, card_name: &str) -> Self {
        let v = vendor.trim().trim_start_matches("0x").to_ascii_lowercase();
        let p = product.trim().trim_start_matches("0x").to_ascii_lowercase();
        let card_lower = card_name.to_ascii_lowercase();

        // Check vendor and product ID signatures first
        if v == "1235" && (p == "0018" || p == "18") {
            return Self::NovationBassStation2;
        }
        if v == "1397" && (p == "00b2" || p == "b2") {
            if card_lower.contains("td-3-md") || card_lower.contains("td3md") {
                return Self::BehringerTd3Md;
            }
            return Self::BehringerTd3;
        }
        if v == "1397" && (p == "00c1" || p == "c1") {
            if card_lower.contains("td-3-md") || card_lower.contains("td3md") {
                return Self::BehringerTd3Md;
            }
            return Self::BehringerTd3Mo;
        }
        if v == "2997" {
            return Self::AsmHydrasynth;
        }
        if v == "0944" && (p == "0117" || p == "117") {
            return Self::KorgNanoKontrol2;
        }
        if v == "09e8" && (p == "0031" || p == "31") {
            return Self::AkaiMidiMix;
        }
        if v == "1397" && (p == "0503" || p == "503") {
            return Self::BehringerUmc1820;
        }

        // Secondary fallback checking card and client name strings
        if card_lower.contains("nord drum") || card_lower.contains("norddrum") {
            Self::ClaviaNordDrum2
        } else if card_lower.contains("bass station") || card_lower.contains("bassstation") {
            Self::NovationBassStation2
        } else if card_lower.contains("td-3-mo") || card_lower.contains("td3mo") {
            Self::BehringerTd3Mo
        } else if card_lower.contains("td-3-md") || card_lower.contains("td3md") {
            Self::BehringerTd3Md
        } else if card_lower.contains("td-3") || card_lower.contains("td3") {
            Self::BehringerTd3
        } else if card_lower.contains("hydrasynth") {
            Self::AsmHydrasynth
        } else if card_lower.contains("nanokontrol2") {
            Self::KorgNanoKontrol2
        } else if card_lower.contains("midimix") {
            Self::AkaiMidiMix
        } else if card_lower.contains("umc1820") {
            Self::BehringerUmc1820
        } else {
            Self::GenericMidi
        }
    }

    /// User-friendly display name of the device.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::NovationBassStation2 => "Novation Bass Station II",
            Self::BehringerTd3 => "Behringer TD-3",
            Self::BehringerTd3Mo => "Behringer TD-3-MO",
            Self::BehringerTd3Md => "Behringer TD-3-MD",
            Self::AsmHydrasynth => "ASM Hydrasynth",
            Self::ClaviaNordDrum2 => "Clavia Nord Drum 2",
            Self::KorgNanoKontrol2 => "Korg nanoKONTROL2",
            Self::AkaiMidiMix => "Akai MIDImix",
            Self::BehringerUmc1820 => "Behringer UMC1820",
            Self::GenericMidi => "Generic MIDI Device",
        }
    }

    /// Returns true if this device is an active sound generator / synthesizer.
    pub fn is_synthesizer(&self) -> bool {
        matches!(
            self,
            Self::NovationBassStation2
                | Self::BehringerTd3
                | Self::BehringerTd3Mo
                | Self::BehringerTd3Md
                | Self::AsmHydrasynth
                | Self::ClaviaNordDrum2
        )
    }

    /// Returns true if this device is a hardware control surface.
    pub fn is_controller(&self) -> bool {
        matches!(self, Self::KorgNanoKontrol2 | Self::AkaiMidiMix)
    }

    /// Returns true if this device is an audio and MIDI interface.
    pub fn is_audio_interface(&self) -> bool {
        matches!(self, Self::BehringerUmc1820)
    }
}

/// Stored persistent device profile remembered across sessions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredDeviceProfile {
    pub identity_key: String,
    pub vendor_id: String,
    pub product_id: String,
    pub serial: Option<String>,
    pub card_name: String,
    pub device_type: SupportedDeviceType,
    pub custom_alias: Option<String>,
    pub last_known_card_index: Option<u32>,
    pub last_known_seq_address: Option<(u32, u32)>,
    pub last_known_usb_path: Option<String>,
    pub reconnect_count: u32,
    pub is_active: bool,
}

/// Device connection and hot-plug lifecycle events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceLifecycleEvent {
    Connected {
        identity_key: String,
        device_type: SupportedDeviceType,
        card_index: u32,
        seq_address: Option<(u32, u32)>,
        usb_path: Option<String>,
    },
    Reconnected {
        identity_key: String,
        device_type: SupportedDeviceType,
        old_card_index: Option<u32>,
        new_card_index: u32,
        old_usb_path: Option<String>,
        new_usb_path: Option<String>,
        reconnect_count: u32,
    },
    Disconnected {
        identity_key: String,
        device_type: SupportedDeviceType,
        last_card_index: Option<u32>,
    },
}

/// Multi-device manager supporting simultaneous active devices, persistent
/// identity memory, and automatic hot-plug reconnection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MultiDeviceManager {
    profiles: HashMap<String, StoredDeviceProfile>,
    active_devices: HashMap<String, DeviceFingerprint>,
    #[serde(skip)]
    rate_limiters: HashMap<String, PortRateLimiter>,
}

impl MultiDeviceManager {
    /// Creates a new empty multi-device manager.
    pub fn new() -> Self {
        Self {
            profiles: HashMap::new(),
            active_devices: HashMap::new(),
            rate_limiters: HashMap::new(),
        }
    }

    /// Returns a list of all currently connected active device fingerprints.
    pub fn active_devices(&self) -> Vec<&DeviceFingerprint> {
        self.active_devices.values().collect()
    }

    /// Returns a reference to an active device fingerprint by its identity key.
    pub fn get_active_device(&self, identity_key: &str) -> Option<&DeviceFingerprint> {
        self.active_devices.get(identity_key)
    }

    /// Returns a reference to a stored profile by its identity key.
    pub fn get_profile(&self, identity_key: &str) -> Option<&StoredDeviceProfile> {
        self.profiles.get(identity_key)
    }

    /// Returns all stored profiles in persistent identity memory.
    pub fn all_profiles(&self) -> Vec<&StoredDeviceProfile> {
        self.profiles.values().collect()
    }

    /// Sets a user-defined alias for a device in persistent memory.
    pub fn set_alias(&mut self, identity_key: &str, alias: impl Into<String>) {
        if let Some(profile) = self.profiles.get_mut(identity_key) {
            profile.custom_alias = Some(alias.into());
        }
    }

    /// Returns all currently active devices matching a given device type.
    pub fn find_by_type(&self, dev_type: SupportedDeviceType) -> Vec<&DeviceFingerprint> {
        self.active_devices
            .values()
            .filter(|d| d.device_type == dev_type)
            .collect()
    }

    /// Updates active device states based on a hardware scan vector.
    ///
    /// Evaluates connect, disconnect, and hot-plug reconnection transitions.
    pub fn update_devices(&mut self, scanned: Vec<DeviceFingerprint>) -> Vec<DeviceLifecycleEvent> {
        let mut events = Vec::new();
        let mut current_keys = HashMap::new();

        for dev in scanned {
            let key = dev.identity_key();
            current_keys.insert(key.clone(), dev);
        }

        // Process newly detected or reconnected devices
        for (key, dev) in &current_keys {
            let seq_addr = match (dev.alsa_seq_client, dev.alsa_seq_port) {
                (Some(c), Some(p)) => Some((c, p)),
                _ => None,
            };

            if let Some(existing_active) = self.active_devices.get(key) {
                // Device was already active: check if physical USB path or card index changed
                if existing_active.alsa_card_index != dev.alsa_card_index
                    || existing_active.usb_path != dev.usb_path
                {
                    let old_card = Some(existing_active.alsa_card_index);
                    let old_path = existing_active.usb_path.clone();

                    if let Some(profile) = self.profiles.get_mut(key) {
                        profile.reconnect_count = profile.reconnect_count.saturating_add(1);
                        profile.last_known_card_index = Some(dev.alsa_card_index);
                        profile.last_known_seq_address = seq_addr;
                        profile.last_known_usb_path = dev.usb_path.clone();
                        profile.is_active = true;

                        events.push(DeviceLifecycleEvent::Reconnected {
                            identity_key: key.clone(),
                            device_type: dev.device_type,
                            old_card_index: old_card,
                            new_card_index: dev.alsa_card_index,
                            old_usb_path: old_path,
                            new_usb_path: dev.usb_path.clone(),
                            reconnect_count: profile.reconnect_count,
                        });
                    }

                    self.active_devices.insert(key.clone(), dev.clone());
                }
            } else if let Some(profile) = self.profiles.get_mut(key) {
                // Device was previously known but inactive: hot-plug reconnection!
                profile.reconnect_count = profile.reconnect_count.saturating_add(1);
                let old_card = profile.last_known_card_index;
                let old_path = profile.last_known_usb_path.clone();

                profile.last_known_card_index = Some(dev.alsa_card_index);
                profile.last_known_seq_address = seq_addr;
                profile.last_known_usb_path = dev.usb_path.clone();
                profile.is_active = true;

                events.push(DeviceLifecycleEvent::Reconnected {
                    identity_key: key.clone(),
                    device_type: dev.device_type,
                    old_card_index: old_card,
                    new_card_index: dev.alsa_card_index,
                    old_usb_path: old_path,
                    new_usb_path: dev.usb_path.clone(),
                    reconnect_count: profile.reconnect_count,
                });

                self.active_devices.insert(key.clone(), dev.clone());
                if !self.rate_limiters.contains_key(key) {
                    self.rate_limiters
                        .insert(key.clone(), PortRateLimiter::default_usb());
                }
            } else {
                // First-time device connection
                let profile = StoredDeviceProfile {
                    identity_key: key.clone(),
                    vendor_id: dev.vendor_id.clone(),
                    product_id: dev.product_id.clone(),
                    serial: dev.serial.clone(),
                    card_name: dev.card_name.clone(),
                    device_type: dev.device_type,
                    custom_alias: None,
                    last_known_card_index: Some(dev.alsa_card_index),
                    last_known_seq_address: seq_addr,
                    last_known_usb_path: dev.usb_path.clone(),
                    reconnect_count: 0,
                    is_active: true,
                };

                self.profiles.insert(key.clone(), profile);
                self.active_devices.insert(key.clone(), dev.clone());
                self.rate_limiters
                    .insert(key.clone(), PortRateLimiter::default_usb());

                events.push(DeviceLifecycleEvent::Connected {
                    identity_key: key.clone(),
                    device_type: dev.device_type,
                    card_index: dev.alsa_card_index,
                    seq_address: seq_addr,
                    usb_path: dev.usb_path.clone(),
                });
            }
        }

        // Process disconnected devices
        let disconnected_keys: Vec<String> = self
            .active_devices
            .keys()
            .filter(|k| !current_keys.contains_key(*k))
            .cloned()
            .collect();

        for key in disconnected_keys {
            if let Some(removed) = self.active_devices.remove(&key) {
                if let Some(profile) = self.profiles.get_mut(&key) {
                    profile.is_active = false;
                }
                events.push(DeviceLifecycleEvent::Disconnected {
                    identity_key: key,
                    device_type: removed.device_type,
                    last_card_index: Some(removed.alsa_card_index),
                });
            }
        }

        events
    }

    /// Scans live Linux USB/ALSA hardware and applies updates.
    pub fn poll_system_hardware(&mut self) -> Vec<DeviceLifecycleEvent> {
        let scanned = scan_hardware_devices();
        self.update_devices(scanned)
    }

    /// Evaluates if the rate limiter permits sending `byte_count` bytes to `identity_key`.
    pub fn can_send_midi(&mut self, identity_key: &str, byte_count: usize) -> bool {
        self.rate_limiters
            .entry(identity_key.to_string())
            .or_insert_with(PortRateLimiter::default_usb)
            .can_send(byte_count)
    }

    /// Records bytes sent to update the rate limiter budget.
    pub fn record_sent_midi(&mut self, identity_key: &str, byte_count: usize) {
        self.rate_limiters
            .entry(identity_key.to_string())
            .or_insert_with(PortRateLimiter::default_usb)
            .record_sent(byte_count);
    }

    /// Serializes persistent identity memory to a JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserializes persistent identity memory from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Generates the standard JSON Schema (Draft-07) for `StoredDeviceProfile`.
pub fn stored_device_profile_json_schema() -> serde_json::Value {
    serde_json::json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "StoredDeviceProfile",
        "description": "Hardware identity fingerprint and connection persistence profile",
        "type": "object",
        "required": ["identity_key", "vendor_id", "product_id", "card_name", "device_type", "reconnect_count", "is_active"],
        "properties": {
            "identity_key": { "type": "string" },
            "vendor_id": { "type": "string" },
            "product_id": { "type": "string" },
            "serial": { "type": ["string", "null"] },
            "card_name": { "type": "string" },
            "device_type": { "type": "string" },
            "custom_alias": { "type": ["string", "null"] },
            "last_known_card_index": { "type": ["integer", "null"] },
            "last_known_seq_address": { "type": ["array", "null"], "items": { "type": "integer" } },
            "last_known_usb_path": { "type": ["string", "null"] },
            "reconnect_count": { "type": "integer", "minimum": 0 },
            "is_active": { "type": "boolean" }
        }
    })
}

/// Validates and parses a JSON profile string into a `StoredDeviceProfile`.
pub fn validate_stored_device_profile_json(
    json_str: &str,
) -> Result<StoredDeviceProfile, synth_core::SynthError> {
    serde_json::from_str(json_str)
        .map_err(|e| synth_core::SynthError::SerializationError(e.to_string()))
}

/// Token bucket per-port rate limiter protecting MIDI endpoints against buffer overruns.
#[derive(Debug, Clone)]
pub struct PortRateLimiter {
    pub max_bytes_per_sec: u32,
    pub burst_capacity: usize,
    available_tokens: f64,
    last_update: Instant,
    total_bytes_sent: u64,
    throttled_count: u64,
}

impl Default for PortRateLimiter {
    fn default() -> Self {
        Self::default_usb()
    }
}

impl PortRateLimiter {
    /// Creates a rate limiter with custom speed and burst capacity.
    pub fn new(max_bytes_per_sec: u32, burst_capacity: usize) -> Self {
        Self {
            max_bytes_per_sec,
            burst_capacity,
            available_tokens: burst_capacity as f64,
            last_update: Instant::now(),
            total_bytes_sent: 0,
            throttled_count: 0,
        }
    }

    /// Default profile for 5-pin DIN MIDI (3,125 bytes/second, 256-byte burst).
    pub fn default_din() -> Self {
        Self::new(3125, 256)
    }

    /// Default profile for High-Speed USB MIDI (12,000 bytes/second, 2,048-byte burst).
    pub fn default_usb() -> Self {
        Self::new(12000, 2048)
    }

    /// Replenishes the token bucket based on elapsed time.
    fn replenish(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update).as_secs_f64();
        self.last_update = now;

        let added = elapsed * (self.max_bytes_per_sec as f64);
        self.available_tokens = (self.available_tokens + added).min(self.burst_capacity as f64);
    }

    /// Checks if `byte_count` bytes can be transmitted without delay.
    pub fn can_send(&mut self, byte_count: usize) -> bool {
        self.replenish();
        self.available_tokens >= byte_count as f64
    }

    /// Records transmitted bytes and deducts from token budget.
    pub fn record_sent(&mut self, byte_count: usize) {
        self.replenish();
        if self.available_tokens >= byte_count as f64 {
            self.available_tokens -= byte_count as f64;
        } else {
            self.available_tokens = 0.0;
            self.throttled_count = self.throttled_count.saturating_add(1);
        }
        self.total_bytes_sent = self.total_bytes_sent.saturating_add(byte_count as u64);
    }

    /// Calculates delay duration needed before `byte_count` bytes can be sent.
    pub fn calculate_delay_needed(&mut self, byte_count: usize) -> Option<Duration> {
        self.replenish();
        let needed = byte_count as f64;
        if self.available_tokens >= needed {
            None
        } else {
            let deficit = needed - self.available_tokens;
            let secs = deficit / (self.max_bytes_per_sec as f64);
            Some(Duration::from_secs_f64(secs))
        }
    }

    /// Returns transmission statistics `(total_bytes_sent, throttled_count)`.
    pub fn metrics(&self) -> (u64, u64) {
        (self.total_bytes_sent, self.throttled_count)
    }

    /// Resets the token bucket to full capacity.
    pub fn reset(&mut self) {
        self.available_tokens = self.burst_capacity as f64;
        self.last_update = Instant::now();
    }
}

/// Errors returned by MIDI packet encoding and decoding routines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MidiCodecError {
    EmptyPacket,
    InvalidStatusByte(u8),
    TruncatedPacket { expected: usize, actual: usize },
    InvalidDataByte(u8),
    InvalidChannel(u8),
    SysExMissingStart,
    SysExMissingEnd,
    SysExInvalidData(u8),
}

impl fmt::Display for MidiCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPacket => write!(f, "MIDI packet buffer is empty"),
            Self::InvalidStatusByte(b) => write!(f, "Invalid MIDI status byte: 0x{:02X}", b),
            Self::TruncatedPacket { expected, actual } => {
                write!(
                    f,
                    "MIDI packet truncated: expected {} bytes, got {}",
                    expected, actual
                )
            }
            Self::InvalidDataByte(b) => write!(f, "Invalid MIDI 7-bit data byte: 0x{:02X}", b),
            Self::InvalidChannel(c) => write!(f, "Invalid MIDI channel {} (must be 0..=15)", c),
            Self::SysExMissingStart => write!(f, "SysEx packet missing 0xF0 start delimiter"),
            Self::SysExMissingEnd => write!(f, "SysEx packet missing 0xF7 end delimiter"),
            Self::SysExInvalidData(b) => write!(f, "SysEx payload byte has MSB set: 0x{:02X}", b),
        }
    }
}

impl std::error::Error for MidiCodecError {}

/// Decoded Non-Registered Parameter Number (NRPN) message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NrpnMessage {
    pub channel: u8,
    pub param: u16,
    pub value: u16,
    pub is_14bit: bool,
}

/// Streaming NRPN decoder tracking CC 99, 98, 6, and 38 state.
#[derive(Debug, Clone, Default)]
pub struct NrpnParser {
    channels: [ChannelNrpnState; 16],
}

#[derive(Debug, Clone, Copy, Default)]
struct ChannelNrpnState {
    param_msb: Option<u8>,
    param_lsb: Option<u8>,
    data_msb: Option<u8>,
}

impl NrpnParser {
    /// Creates a new streaming NRPN parser.
    pub fn new() -> Self {
        Self {
            channels: [ChannelNrpnState::default(); 16],
        }
    }

    /// Feeds a single Control Change event into the parser state machine.
    ///
    /// Returns `Some(NrpnMessage)` when an NRPN parameter value sequence is completed.
    pub fn parse_cc(&mut self, channel: u8, cc: u8, value: u8) -> Option<NrpnMessage> {
        if channel > 15 {
            return None;
        }
        let ch_idx = channel as usize;
        let state = &mut self.channels[ch_idx];
        let val7 = value & 0x7F;

        match cc {
            99 => {
                // NRPN Parameter MSB
                state.param_msb = Some(val7);
                state.data_msb = None;
                None
            }
            98 => {
                // NRPN Parameter LSB
                state.param_lsb = Some(val7);
                state.data_msb = None;
                None
            }
            6 => {
                // Data Entry MSB
                state.data_msb = Some(val7);
                if let (Some(p_msb), Some(p_lsb)) = (state.param_msb, state.param_lsb) {
                    let param = ((p_msb as u16) << 7) | (p_lsb as u16);
                    Some(NrpnMessage {
                        channel,
                        param,
                        value: val7 as u16,
                        is_14bit: false,
                    })
                } else {
                    None
                }
            }
            38 => {
                // Data Entry LSB (completes 14-bit NRPN)
                if let (Some(p_msb), Some(p_lsb), Some(d_msb)) =
                    (state.param_msb, state.param_lsb, state.data_msb)
                {
                    let param = ((p_msb as u16) << 7) | (p_lsb as u16);
                    let full_value = ((d_msb as u16) << 7) | (val7 as u16);
                    Some(NrpnMessage {
                        channel,
                        param,
                        value: full_value,
                        is_14bit: true,
                    })
                } else {
                    None
                }
            }
            100 | 101 => {
                // RPN Null / Reset
                if val7 == 127 {
                    state.param_msb = None;
                    state.param_lsb = None;
                    state.data_msb = None;
                }
                None
            }
            _ => None,
        }
    }

    /// Feeds a raw byte slice containing serialized CC messages into the parser.
    pub fn parse_stream(&mut self, bytes: &[u8]) -> Vec<NrpnMessage> {
        let mut results = Vec::new();
        let mut idx = 0;
        while idx + 2 < bytes.len() {
            let status = bytes[idx];
            if (0xB0..=0xBF).contains(&status) {
                let ch = status & 0x0F;
                let cc = bytes[idx + 1] & 0x7F;
                let val = bytes[idx + 2] & 0x7F;
                if let Some(msg) = self.parse_cc(ch, cc, val) {
                    results.push(msg);
                }
                idx += 3;
            } else {
                idx += 1;
            }
        }
        results
    }
}

/// Decoded System Exclusive (SysEx) packet with manufacturer and payload separation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SysExPacket {
    pub manufacturer_id: Vec<u8>,
    pub payload: Vec<u8>,
    pub raw: Vec<u8>,
}

/// Reusable MIDI packet encoding and decoding helpers.
pub struct MidiCodec;

impl MidiCodec {
    /// Encodes a standard 7-bit Control Change message (3 bytes).
    pub fn encode_cc(channel: u8, cc: u8, value: u8) -> [u8; 3] {
        [0xB0 | (channel & 0x0F), cc & 0x7F, value & 0x7F]
    }

    /// Decodes a 3-byte slice into `(channel, cc, value)`.
    pub fn decode_cc(bytes: &[u8]) -> Result<(u8, u8, u8), MidiCodecError> {
        if bytes.is_empty() {
            return Err(MidiCodecError::EmptyPacket);
        }
        if bytes.len() < 3 {
            return Err(MidiCodecError::TruncatedPacket {
                expected: 3,
                actual: bytes.len(),
            });
        }
        let status = bytes[0];
        if (status & 0xF0) != 0xB0 {
            return Err(MidiCodecError::InvalidStatusByte(status));
        }
        let channel = status & 0x0F;
        let cc = bytes[1];
        let val = bytes[2];
        if cc > 127 {
            return Err(MidiCodecError::InvalidDataByte(cc));
        }
        if val > 127 {
            return Err(MidiCodecError::InvalidDataByte(val));
        }
        Ok((channel, cc, val))
    }

    /// Encodes a 14-bit high-resolution CC pair (6 bytes: MSB cc, LSB cc + 32).
    pub fn encode_cc14(channel: u8, msb_cc: u8, value14: u16) -> [u8; 6] {
        let ch = channel & 0x0F;
        let clamped = value14.min(16383);
        let msb_val = ((clamped >> 7) & 0x7F) as u8;
        let lsb_val = (clamped & 0x7F) as u8;
        let lsb_cc = if msb_cc < 32 { msb_cc + 32 } else { msb_cc };

        [
            0xB0 | ch,
            msb_cc & 0x7F,
            msb_val,
            0xB0 | ch,
            lsb_cc & 0x7F,
            lsb_val,
        ]
    }

    /// Combines MSB and LSB 7-bit values into a 14-bit integer [0..16383].
    pub fn decode_cc14(msb: u8, lsb: u8) -> u16 {
        ((msb as u16 & 0x7F) << 7) | (lsb as u16 & 0x7F)
    }

    /// Encodes an NRPN parameter and value packet (CC 99, 98, 6, and optionally 38).
    pub fn encode_nrpn(channel: u8, param: u16, value: u16, is_14bit: bool) -> Vec<u8> {
        let ch = 0xB0 | (channel & 0x0F);
        let p_msb = ((param >> 7) & 0x7F) as u8;
        let p_lsb = (param & 0x7F) as u8;

        if is_14bit {
            let v_msb = ((value >> 7) & 0x7F) as u8;
            let v_lsb = (value & 0x7F) as u8;
            vec![ch, 99, p_msb, ch, 98, p_lsb, ch, 6, v_msb, ch, 38, v_lsb]
        } else {
            let v_7bit = (value & 0x7F) as u8;
            vec![ch, 99, p_msb, ch, 98, p_lsb, ch, 6, v_7bit]
        }
    }

    /// Encodes an NRPN Null reset sequence (CC 101: 127, CC 100: 127).
    pub fn encode_nrpn_null(channel: u8) -> [u8; 6] {
        let ch = 0xB0 | (channel & 0x0F);
        [ch, 101, 127, ch, 100, 127]
    }

    /// Encodes a framed SysEx packet `[0xF0, ...manufacturer, ...payload, 0xF7]`.
    pub fn encode_sysex(
        manufacturer: &[u8],
        payload: &[u8],
    ) -> Result<Vec<u8>, MidiCodecError> {
        for &b in manufacturer {
            if b > 0x7F {
                return Err(MidiCodecError::SysExInvalidData(b));
            }
        }
        for &b in payload {
            if b > 0x7F {
                return Err(MidiCodecError::SysExInvalidData(b));
            }
        }
        let mut packet = Vec::with_capacity(manufacturer.len() + payload.len() + 2);
        packet.push(0xF0);
        packet.extend_from_slice(manufacturer);
        packet.extend_from_slice(payload);
        packet.push(0xF7);
        Ok(packet)
    }

    /// Validates and decodes a raw SysEx byte sequence.
    pub fn decode_sysex(bytes: &[u8]) -> Result<SysExPacket, MidiCodecError> {
        if bytes.is_empty() {
            return Err(MidiCodecError::EmptyPacket);
        }
        if bytes[0] != 0xF0 {
            return Err(MidiCodecError::SysExMissingStart);
        }
        if *bytes.last().unwrap_or(&0) != 0xF7 {
            return Err(MidiCodecError::SysExMissingEnd);
        }
        if bytes.len() < 3 {
            return Err(MidiCodecError::TruncatedPacket {
                expected: 3,
                actual: bytes.len(),
            });
        }

        let inner = &bytes[1..bytes.len() - 1];
        for &b in inner {
            if b > 0x7F {
                return Err(MidiCodecError::SysExInvalidData(b));
            }
        }

        let (mfr, payload) = if inner[0] == 0x00 && inner.len() >= 3 {
            // Extended 3-byte manufacturer identifier
            (inner[0..3].to_vec(), inner[3..].to_vec())
        } else {
            // Standard 1-byte manufacturer identifier
            (inner[0..1].to_vec(), inner[1..].to_vec())
        };

        Ok(SysExPacket {
            manufacturer_id: mfr,
            payload,
            raw: bytes.to_vec(),
        })
    }
}

/// Builds an emergency All-Notes-Off and Controller Reset PANIC packet.
///
/// If `channel` is `Some(ch)`, panics that specific MIDI channel (0..=15).
/// If `channel` is `None`, sweeps across all 16 MIDI channels.
pub fn build_panic_packet(channel: Option<u8>) -> Vec<u8> {
    let channels: Vec<u8> = match channel {
        Some(c) => vec![c.min(15)],
        None => (0..16).collect(),
    };

    let mut packet = Vec::with_capacity(channels.len() * 20);
    for ch in channels {
        let b = 0xB0 | (ch & 0x0F);
        // CC 120: All Sound Off (immediate oscillator mute) - 3 bytes
        packet.extend_from_slice(&[b, 120, 0]);
        // CC 121: Reset All Controllers (mod wheel, expression, aftertouch) - 3 bytes
        packet.extend_from_slice(&[b, 121, 0]);
        // CC 123: All Notes Off (send note release to all voices) - 3 bytes
        packet.extend_from_slice(&[b, 123, 0]);
        // CC 64: Damper / Sustain Pedal Off - 3 bytes
        packet.extend_from_slice(&[b, 64, 0]);
        // CC 1: Modulation Wheel Reset to zero - 3 bytes
        packet.extend_from_slice(&[b, 1, 0]);
        // Pitch Bend Reset to neutral center (0x2000 -> LSB 0x00, MSB 0x40) - 3 bytes
        packet.extend_from_slice(&[0xE0 | (ch & 0x0F), 0x00, 0x40]);
        // Channel Pressure / Aftertouch reset to zero - 2 bytes
        packet.extend_from_slice(&[0xD0 | (ch & 0x0F), 0]);
    }
    packet
}

/// Builds a deep PANIC packet including explicit Note Off commands for all 128 notes.
///
/// Used for hardware synths with firmware bugs that ignore CC 123.
pub fn build_deep_panic_packet(channel: Option<u8>) -> Vec<u8> {
    let mut packet = build_panic_packet(channel);
    let channels: Vec<u8> = match channel {
        Some(c) => vec![c.min(15)],
        None => (0..16).collect(),
    };

    for ch in channels {
        let note_off_cmd = 0x80 | (ch & 0x0F);
        for note in 0..128 {
            packet.extend_from_slice(&[note_off_cmd, note, 0]);
        }
    }
    packet
}

/// Linux hardware identity scanner using real Linux filesystem roots.
pub fn scan_hardware_devices() -> Vec<DeviceFingerprint> {
    scan_hardware_devices_with_roots(
        Path::new("/sys/class/sound"),
        Path::new("/dev/snd/by-id"),
        Path::new("/dev/snd/by-path"),
        Path::new("/proc/asound/seq/clients"),
    )
}

/// Scans hardware identity using configurable root paths for headless unit testing.
pub fn scan_hardware_devices_with_roots(
    sys_class_sound: &Path,
    dev_snd_by_id: &Path,
    dev_snd_by_path: &Path,
    proc_asound_seq_clients: &Path,
) -> Vec<DeviceFingerprint> {
    let mut results = Vec::new();

    // 1. Parse /proc/asound/seq/clients to extract ALSA sequencer client and port mappings
    let alsa_seq_map = parse_proc_seq_clients(proc_asound_seq_clients);

    // 2. Parse /dev/snd/by-id symlinks to extract card index to stable by-id path and serial
    let by_id_map = parse_dev_snd_symlinks(dev_snd_by_id);

    // 3. Parse /dev/snd/by-path symlinks to extract card index to USB topology path
    let by_path_map = parse_dev_snd_symlinks(dev_snd_by_path);

    // 4. Iterate over /sys/class/sound/card* directories
    let entries = match fs::read_dir(sys_class_sound) {
        Ok(e) => e,
        Err(_) => return results,
    };

    let mut card_dirs = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(rest) = name.strip_prefix("card") {
            if let Ok(idx) = rest.parse::<u32>() {
                card_dirs.push((idx, entry.path()));
            }
        }
    }
    card_dirs.sort_by_key(|(idx, _)| *idx);

    for (card_idx, card_path) in card_dirs {
        // Read card name from card/id
        let card_name = fs::read_to_string(card_path.join("id"))
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| format!("card{}", card_idx));

        // Read USB vendor_id and product_id
        let (vendor_id, product_id) = find_usb_ids(&card_path);

        // Read serial number: check sysfs first, fallback to by-id parse
        let serial = fs::read_to_string(card_path.join("device/../serial"))
            .or_else(|_| fs::read_to_string(card_path.join("device/serial")))
            .map(|s| s.trim().to_string())
            .ok()
            .filter(|s| !s.is_empty() && s != "none")
            .or_else(|| {
                by_id_map.get(&card_idx).and_then(|file_name| {
                    extract_serial_from_by_id(file_name)
                })
            });

        // Resolve USB topology path from by_path_map
        let usb_path = by_path_map.get(&card_idx).cloned();

        // Resolve by-id path from by_id_map
        let by_id_path = by_id_map.get(&card_idx).cloned();

        // Resolve ALSA seq client and port
        let (seq_client, seq_port) = match_seq_client(&card_name, &alsa_seq_map);

        let device_type =
            SupportedDeviceType::from_vendor_product(&vendor_id, &product_id, &card_name);

        let fingerprint = DeviceFingerprint {
            vendor_id,
            product_id,
            serial,
            card_name,
            alsa_card_index: card_idx,
            alsa_seq_client: seq_client,
            alsa_seq_port: seq_port,
            usb_path,
            by_id_path,
            device_type,
        };

        results.push(fingerprint);
    }

    results
}

fn parse_dev_snd_symlinks(dir: &Path) -> HashMap<u32, String> {
    let mut map = HashMap::new();
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return map,
    };

    for entry in entries.flatten() {
        let file_name = entry.file_name().to_string_lossy().to_string();
        if let Ok(target) = fs::read_link(entry.path()) {
            let target_str = target.to_string_lossy();
            if let Some(pos) = target_str.find("controlC") {
                let rest = &target_str[pos + 8..];
                let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(card_idx) = num_str.parse::<u32>() {
                    map.insert(card_idx, file_name);
                }
            }
        }
    }
    map
}

fn extract_serial_from_by_id(name: &str) -> Option<String> {
    // Standard udev format: usb-BEHRINGER_UMC1820_B8AAF8D3-00
    let stripped = name.strip_prefix("usb-")?.strip_suffix("-00").unwrap_or(name);
    let parts: Vec<&str> = stripped.split('_').collect();
    if parts.len() >= 3 {
        let candidate = parts.last()?.trim();
        if !candidate.is_empty() && candidate != "none" {
            return Some(candidate.to_string());
        }
    }
    None
}

fn find_usb_ids(card_path: &Path) -> (String, String) {
    // Check idVendor and idProduct files
    let v_file = fs::read_to_string(card_path.join("device/../idVendor"))
        .or_else(|_| fs::read_to_string(card_path.join("device/idVendor")));
    let p_file = fs::read_to_string(card_path.join("device/../idProduct"))
        .or_else(|_| fs::read_to_string(card_path.join("device/idProduct")));

    if let (Ok(v), Ok(p)) = (v_file, p_file) {
        let v_trim = v.trim().to_ascii_lowercase();
        let p_trim = p.trim().to_ascii_lowercase();
        if !v_trim.is_empty() && !p_trim.is_empty() {
            return (format_hex4(&v_trim), format_hex4(&p_trim));
        }
    }

    // Check uevent file
    let uevent_file = fs::read_to_string(card_path.join("device/uevent"))
        .or_else(|_| fs::read_to_string(card_path.join("device/../uevent")));

    if let Ok(uevent) = uevent_file {
        for line in uevent.lines() {
            if let Some(rest) = line.strip_prefix("MODALIAS=usb:v") {
                if rest.len() >= 9 && rest.chars().nth(4) == Some('p') {
                    let v = &rest[0..4];
                    let p = &rest[5..9];
                    return (v.to_ascii_lowercase(), p.to_ascii_lowercase());
                }
            } else if let Some(rest) = line.strip_prefix("PRODUCT=") {
                let parts: Vec<&str> = rest.split('/').collect();
                if parts.len() >= 2 {
                    let v = format_hex4(parts[0]);
                    let p = format_hex4(parts[1]);
                    return (v, p);
                }
            }
        }
    }

    ("0000".to_string(), "0000".to_string())
}

fn format_hex4(s: &str) -> String {
    let clean = s.trim().trim_start_matches("0x");
    if let Ok(val) = u16::from_str_radix(clean, 16) {
        format!("{:04x}", val)
    } else {
        format!("{:0>4}", clean.to_ascii_lowercase())
    }
}

#[derive(Debug, Clone)]
struct SeqClientEntry {
    client_id: u32,
    name: String,
    ports: Vec<u32>,
}

fn parse_proc_seq_clients(path: &Path) -> Vec<SeqClientEntry> {
    let mut clients = Vec::new();
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return clients,
    };

    let mut current_client: Option<SeqClientEntry> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Client") {
            if let Some(entry) = current_client.take() {
                clients.push(entry);
            }
            if let Some(pos) = trimmed.find(':') {
                let left = &trimmed[6..pos].trim();
                let right = &trimmed[pos + 1..].trim();
                if let Ok(c_id) = left.parse::<u32>() {
                    let name = right
                        .split('"')
                        .nth(1)
                        .unwrap_or(right)
                        .to_string();
                    current_client = Some(SeqClientEntry {
                        client_id: c_id,
                        name,
                        ports: Vec::new(),
                    });
                }
            }
        } else if trimmed.starts_with("Port") {
            if let Some(ref mut client) = current_client {
                if let Some(pos) = trimmed.find(':') {
                    let left = &trimmed[4..pos].trim();
                    if let Ok(p_id) = left.parse::<u32>() {
                        client.ports.push(p_id);
                    }
                }
            }
        }
    }

    if let Some(entry) = current_client {
        clients.push(entry);
    }

    clients
}

fn match_seq_client(card_name: &str, clients: &[SeqClientEntry]) -> (Option<u32>, Option<u32>) {
    let card_lower = card_name.to_ascii_lowercase();
    for client in clients {
        let cl_lower = client.name.to_ascii_lowercase();
        if cl_lower == card_lower
            || cl_lower.contains(&card_lower)
            || card_lower.contains(&cl_lower)
        {
            let port = client.ports.first().copied().unwrap_or(0);
            return (Some(client.client_id), Some(port));
        }
    }
    (None, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_fingerprint_identity_key_formatting() {
        let fp = DeviceFingerprint::new("1235", "0018", None, "Bass Station II", 1);
        assert_eq!(fp.identity_key(), "1235:0018:none:Bass Station II");
        assert_eq!(fp.device_type(), SupportedDeviceType::NovationBassStation2);

        let fp_serial = DeviceFingerprint::new(
            "1397",
            "0503",
            Some("B8AAF8D3".to_string()),
            "UMC1820",
            0,
        );
        assert_eq!(fp_serial.identity_key(), "1397:0503:B8AAF8D3:UMC1820");
        assert_eq!(fp_serial.device_type(), SupportedDeviceType::BehringerUmc1820);
    }

    #[test]
    fn supported_device_recognition_signatures() {
        assert_eq!(
            SupportedDeviceType::from_vendor_product("1235", "0018", "Bass Station II"),
            SupportedDeviceType::NovationBassStation2
        );
        assert_eq!(
            SupportedDeviceType::from_vendor_product("1397", "00b2", "TD-3"),
            SupportedDeviceType::BehringerTd3
        );
        assert_eq!(
            SupportedDeviceType::from_vendor_product("1397", "00b2", "TD-3-MD"),
            SupportedDeviceType::BehringerTd3Md
        );
        assert_eq!(
            SupportedDeviceType::from_vendor_product("1397", "00c1", "TD-3-MO"),
            SupportedDeviceType::BehringerTd3Mo
        );
        assert_eq!(
            SupportedDeviceType::from_vendor_product("2997", "0101", "Hydrasynth Desktop"),
            SupportedDeviceType::AsmHydrasynth
        );
        assert_eq!(
            SupportedDeviceType::from_vendor_product("0944", "0117", "nanoKONTROL2"),
            SupportedDeviceType::KorgNanoKontrol2
        );
        assert_eq!(
            SupportedDeviceType::from_vendor_product("09e8", "0031", "MIDImix"),
            SupportedDeviceType::AkaiMidiMix
        );
        assert_eq!(
            SupportedDeviceType::from_vendor_product("1397", "0503", "UMC1820"),
            SupportedDeviceType::BehringerUmc1820
        );
        assert_eq!(
            SupportedDeviceType::from_vendor_product("0000", "0000", "Clavia Nord Drum 2"),
            SupportedDeviceType::ClaviaNordDrum2
        );
    }

    #[test]
    fn multi_device_manager_hot_plug_reconnection() {
        let mut manager = MultiDeviceManager::new();

        // 1. Connect Bass Station II on USB port 1-5 (card 1)
        let bs2 = DeviceFingerprint::new("1235", "0018", None, "Bass Station II", 1)
            .with_usb_path("pci-0000:00:14.0-usb-0:5:1.0")
            .with_seq_address(20, 0);

        let events = manager.update_devices(vec![bs2.clone()]);
        assert_eq!(events.len(), 1);
        match &events[0] {
            DeviceLifecycleEvent::Connected { card_index, .. } => assert_eq!(*card_index, 1),
            _ => panic!("Expected Connected event"),
        }
        assert_eq!(manager.active_devices().len(), 1);

        // 2. Unplug Bass Station II
        let events = manager.update_devices(vec![]);
        assert_eq!(events.len(), 1);
        match &events[0] {
            DeviceLifecycleEvent::Disconnected { last_card_index, .. } => {
                assert_eq!(*last_card_index, Some(1))
            }
            _ => panic!("Expected Disconnected event"),
        }
        assert_eq!(manager.active_devices().len(), 0);

        // 3. Re-plug Bass Station II into a DIFFERENT USB port (card 3, new path)
        let bs2_moved = DeviceFingerprint::new("1235", "0018", None, "Bass Station II", 3)
            .with_usb_path("pci-0000:00:14.0-usb-0:6:1.0")
            .with_seq_address(28, 0);

        let events = manager.update_devices(vec![bs2_moved]);
        assert_eq!(events.len(), 1);
        match &events[0] {
            DeviceLifecycleEvent::Reconnected {
                old_card_index,
                new_card_index,
                reconnect_count,
                ..
            } => {
                assert_eq!(*old_card_index, Some(1));
                assert_eq!(*new_card_index, 3);
                assert_eq!(*reconnect_count, 1);
            }
            _ => panic!("Expected Reconnected event"),
        }
        assert_eq!(manager.active_devices().len(), 1);

        // Verify persistent profile state
        let profile = manager.get_profile(&bs2.identity_key()).unwrap();
        assert_eq!(profile.last_known_card_index, Some(3));
        assert!(profile.is_active);
    }

    #[test]
    fn midi_cc_and_14bit_codec() {
        // Standard 7-bit CC
        let cc_bytes = MidiCodec::encode_cc(2, 74, 100);
        assert_eq!(cc_bytes, [0xB2, 74, 100]);
        let (ch, cc, val) = MidiCodec::decode_cc(&cc_bytes).unwrap();
        assert_eq!((ch, cc, val), (2, 74, 100));

        // 14-bit CC
        let cc14_bytes = MidiCodec::encode_cc14(0, 16, 10000);
        // 10000 = (78 << 7) | 16
        assert_eq!(cc14_bytes, [0xB0, 16, 78, 0xB0, 48, 16]);
        let decoded14 = MidiCodec::decode_cc14(78, 16);
        assert_eq!(decoded14, 10000);
    }

    #[test]
    fn midi_nrpn_stream_parser() {
        let mut parser = NrpnParser::new();
        // Param 200, Value 12000 (14-bit)
        // 200 = (1 << 7) | 72
        // 12000 = (93 << 7) | 96
        let encoded = MidiCodec::encode_nrpn(1, 200, 12000, true);
        let parsed = parser.parse_stream(&encoded);

        // CC 6 emits first 7-bit value, CC 38 emits complete 14-bit value
        assert!(!parsed.is_empty());
        let last = parsed.last().unwrap();
        assert_eq!(last.channel, 1);
        assert_eq!(last.param, 200);
        assert_eq!(last.value, 12000);
        assert!(last.is_14bit);
    }

    #[test]
    fn midi_sysex_codec() {
        let mfr = [0x00, 0x20, 0x29]; // Novation manufacturer ID
        let payload = [0x01, 0x02, 0x03, 0x7F];
        let packet = MidiCodec::encode_sysex(&mfr, &payload).unwrap();

        assert_eq!(packet[0], 0xF0);
        assert_eq!(*packet.last().unwrap(), 0xF7);

        let decoded = MidiCodec::decode_sysex(&packet).unwrap();
        assert_eq!(decoded.manufacturer_id, mfr);
        assert_eq!(decoded.payload, payload);
    }

    #[test]
    fn emergency_panic_packet_generation() {
        let single_ch = build_panic_packet(Some(5));
        assert_eq!(single_ch.len(), 20);
        // Verify CC 123 (All Notes Off) is present for channel 5 (0xB5)
        assert!(single_ch.windows(3).any(|w| w == [0xB5, 123, 0]));

        let all_ch = build_panic_packet(None);
        assert_eq!(all_ch.len(), 16 * 20);
        for ch in 0..16 {
            assert!(all_ch.windows(3).any(|w| w == [0xB0 | ch, 123, 0]));
        }

        let deep = build_deep_panic_packet(Some(0));
        assert_eq!(deep.len(), 20 + 128 * 3);
    }

    #[test]
    fn port_rate_limiter_throttling() {
        let mut limiter = PortRateLimiter::new(1000, 200);
        assert!(limiter.can_send(100));
        limiter.record_sent(100);

        assert!(limiter.can_send(100));
        limiter.record_sent(100);

        // Budget is now exhausted (200 tokens consumed)
        assert!(!limiter.can_send(50));
        let delay = limiter.calculate_delay_needed(50);
        assert!(delay.is_some());
        assert!(delay.unwrap().as_secs_f64() > 0.0);
    }
}
