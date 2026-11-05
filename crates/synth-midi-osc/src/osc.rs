// SPDX-License-Identifier: GPL-3.0-or-later
//! Bidirectional UDP Open Sound Control (OSC) server and subscriber broadcaster
//! using `rosc = "0.10"`.

use rosc::{decoder, encoder, OscMessage, OscPacket, OscType};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, RwLock};

/// Configuration for the OSC server and subscriber broadcaster.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OscConfig {
    pub listen_port: u16,
    pub broadcast_port: u16,
    pub broadcast_host: String,
    pub bind_host: String,
}

impl Default for OscConfig {
    fn default() -> Self {
        Self {
            listen_port: 9100,
            broadcast_port: 9101,
            broadcast_host: "127.0.0.1".to_string(),
            bind_host: "127.0.0.1".to_string(),
        }
    }
}

/// Errors occurring during OSC packet processing, encoding, or network I/O.
#[derive(Debug)]
pub enum OscError {
    Io(io::Error),
    Rosc(rosc::OscError),
    Serialization(String),
    InvalidAddress(String),
}

impl fmt::Display for OscError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "OSC network I/O error: {}", e),
            Self::Rosc(e) => write!(f, "OSC codec error: {:?}", e),
            Self::Serialization(msg) => write!(f, "OSC serialization error: {}", msg),
            Self::InvalidAddress(addr) => write!(f, "Invalid OSC address pattern: {}", addr),
        }
    }
}

impl std::error::Error for OscError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for OscError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<rosc::OscError> for OscError {
    fn from(e: rosc::OscError) -> Self {
        Self::Rosc(e)
    }
}

/// Inbound OSC message received from a remote client or Konduktiva engine.
#[derive(Debug, Clone, PartialEq)]
pub struct OscIncomingMessage {
    pub sender: SocketAddr,
    pub address: String,
    pub args: Vec<OscType>,
}

impl OscIncomingMessage {
    /// Extracts a parameter value as a 32-bit float if present.
    pub fn param_float(&self) -> Option<f32> {
        self.args.first().and_then(|arg| match arg {
            OscType::Float(f) => Some(*f),
            OscType::Double(d) => Some(*d as f32),
            OscType::Int(i) => Some(*i as f32),
            _ => None,
        })
    }

    /// Returns true if this message is an All-Notes-Off panic request.
    pub fn is_panic(&self) -> bool {
        self.address.ends_with("/panic")
    }

    /// Returns true if this message is a subscriber registration command.
    pub fn is_subscribe(&self) -> bool {
        self.address == "/subscribe" || self.address == "/synth/subscribe"
    }

    /// Returns true if this message is a subscriber deregistration command.
    pub fn is_unsubscribe(&self) -> bool {
        self.address == "/unsubscribe" || self.address == "/synth/unsubscribe"
    }

    /// Parses `/synth/{device}/param/{param_id}` into `(device, param_id)`.
    pub fn parse_synth_param(&self) -> Option<(&str, &str)> {
        let parts: Vec<&str> = self.address.split('/').filter(|s| !s.is_empty()).collect();
        if parts.len() >= 4 && parts[0] == "synth" && parts[2] == "param" {
            Some((parts[1], parts[3]))
        } else if parts.len() >= 2 && parts[0] == "param" {
            Some(("all", parts[1]))
        } else {
            None
        }
    }
}

/// OSC subscriber broadcaster transmitting synthesizer state to registered clients.
#[derive(Debug, Clone)]
pub struct OscBroadcaster {
    socket: Arc<UdpSocket>,
    subscribers: Arc<RwLock<HashSet<SocketAddr>>>,
}

impl OscBroadcaster {
    /// Creates a broadcaster bound to an ephemeral or specified host and port.
    pub fn new(bind_host: &str, bind_port: u16) -> Result<Self, OscError> {
        let addr = format!("{}:{}", bind_host, bind_port);
        let socket = UdpSocket::bind(&addr)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket: Arc::new(socket),
            subscribers: Arc::new(RwLock::new(HashSet::new())),
        })
    }

    /// Adds a subscriber address to receive broadcast updates.
    pub fn add_subscriber(&self, addr: SocketAddr) {
        if let Ok(mut subs) = self.subscribers.write() {
            subs.insert(addr);
        }
    }

    /// Removes a subscriber address from broadcast updates.
    pub fn remove_subscriber(&self, addr: &SocketAddr) {
        if let Ok(mut subs) = self.subscribers.write() {
            subs.remove(addr);
        }
    }

    /// Returns the number of currently active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.subscribers
            .read()
            .map(|s| s.len())
            .unwrap_or(0)
    }

    /// Returns a list of all active subscriber socket addresses.
    pub fn subscribers(&self) -> Vec<SocketAddr> {
        self.subscribers
            .read()
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Broadcasts an OSC message to all registered subscribers.
    ///
    /// Returns the number of subscribers the message was successfully dispatched to.
    pub fn broadcast_message(&self, msg: OscMessage) -> Result<usize, OscError> {
        let packet = OscPacket::Message(msg);
        let bytes = encoder::encode(&packet)?;
        let subs = self.subscribers();

        let mut sent_count = 0;
        for sub in subs {
            if self.socket.send_to(&bytes, sub).is_ok() {
                sent_count += 1;
            }
        }
        Ok(sent_count)
    }

    /// Broadcasts a synthesizer parameter update `/synth/{device}/param/{param_name} <val>`.
    pub fn broadcast_param(
        &self,
        device: &str,
        param_name: &str,
        value: f32,
    ) -> Result<usize, OscError> {
        let addr = format!("/synth/{}/param/{}", device, param_name);
        let msg = OscMessage {
            addr,
            args: vec![OscType::Float(value)],
        };
        self.broadcast_message(msg)
    }

    /// Broadcasts device connection status `/synth/{device}/status <connected> <port_info>`.
    pub fn broadcast_device_status(
        &self,
        device: &str,
        connected: bool,
        port_info: &str,
    ) -> Result<usize, OscError> {
        let addr = format!("/synth/{}/status", device);
        let msg = OscMessage {
            addr,
            args: vec![
                OscType::Bool(connected),
                OscType::String(port_info.to_string()),
            ],
        };
        self.broadcast_message(msg)
    }

    /// Broadcasts an emergency All-Notes-Off panic event `/synth/{device}/panic`.
    pub fn broadcast_panic(&self, device: Option<&str>) -> Result<usize, OscError> {
        let addr = match device {
            Some(dev) => format!("/synth/{}/panic", dev),
            None => "/synth/panic".to_string(),
        };
        let msg = OscMessage {
            addr,
            args: Vec::new(),
        };
        self.broadcast_message(msg)
    }

    /// Returns the local socket address this broadcaster is bound to.
    pub fn local_addr(&self) -> Result<SocketAddr, OscError> {
        Ok(self.socket.local_addr()?)
    }
}

/// Inbound OSC server decoding UDP datagrams into structured messages.
#[derive(Debug)]
pub struct OscServer {
    socket: UdpSocket,
}

impl OscServer {
    /// Binds the server UDP socket to the given host and port.
    pub fn bind(host: &str, port: u16) -> Result<Self, OscError> {
        let addr = format!("{}:{}", host, port);
        let socket = UdpSocket::bind(&addr)?;
        socket.set_nonblocking(true)?;
        Ok(Self { socket })
    }

    /// Returns the local socket address.
    pub fn local_addr(&self) -> Result<SocketAddr, OscError> {
        Ok(self.socket.local_addr()?)
    }

    /// Non-blocking polling pass receiving and decoding all pending UDP datagrams.
    pub fn poll_incoming(&self) -> Result<Vec<OscIncomingMessage>, OscError> {
        let mut messages = Vec::new();
        let mut buf = [0u8; 4096];

        loop {
            match self.socket.recv_from(&mut buf) {
                Ok((len, sender)) => {
                    if let Ok((_, packet)) = decoder::decode_udp(&buf[..len]) {
                        Self::extract_messages(packet, sender, &mut messages);
                    }
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    break;
                }
                Err(e) => return Err(OscError::Io(e)),
            }
        }

        Ok(messages)
    }

    /// Sends a direct OSC reply message back to a specific client address.
    pub fn send_to(&self, addr: SocketAddr, msg: OscMessage) -> Result<(), OscError> {
        let packet = OscPacket::Message(msg);
        let bytes = encoder::encode(&packet)?;
        self.socket.send_to(&bytes, addr)?;
        Ok(())
    }

    /// Recursively flattens `OscPacket::Message` and `OscPacket::Bundle` into message list.
    fn extract_messages(
        packet: OscPacket,
        sender: SocketAddr,
        out: &mut Vec<OscIncomingMessage>,
    ) {
        match packet {
            OscPacket::Message(msg) => {
                out.push(OscIncomingMessage {
                    sender,
                    address: msg.addr,
                    args: msg.args,
                });
            }
            OscPacket::Bundle(bundle) => {
                for inner in bundle.content {
                    Self::extract_messages(inner, sender, out);
                }
            }
        }
    }
}

/// Bidirectional OSC coordinator integrating `OscServer` and `OscBroadcaster`.
#[derive(Debug)]
pub struct OscBridge {
    server: OscServer,
    broadcaster: OscBroadcaster,
}

impl OscBridge {
    /// Binds an OSC bridge using the supplied configuration.
    pub fn new(config: &OscConfig) -> Result<Self, OscError> {
        let server = OscServer::bind(&config.bind_host, config.listen_port)?;
        let broadcaster = OscBroadcaster::new(&config.bind_host, config.broadcast_port)?;
        Ok(Self {
            server,
            broadcaster,
        })
    }

    /// Binds an OSC bridge on ephemeral ports `(0, 0)` for testing.
    pub fn new_ephemeral() -> Result<Self, OscError> {
        let server = OscServer::bind("127.0.0.1", 0)?;
        let broadcaster = OscBroadcaster::new("127.0.0.1", 0)?;
        Ok(Self {
            server,
            broadcaster,
        })
    }

    /// Returns a reference to the inbound server component.
    pub fn server(&self) -> &OscServer {
        &self.server
    }

    /// Returns a reference to the subscriber broadcaster component.
    pub fn broadcaster(&self) -> &OscBroadcaster {
        &self.broadcaster
    }

    /// Polls incoming messages and automatically processes `/subscribe` and `/unsubscribe`.
    pub fn poll(&self) -> Result<Vec<OscIncomingMessage>, OscError> {
        let incoming = self.server.poll_incoming()?;
        for msg in &incoming {
            if msg.is_subscribe() {
                // If client specified an explicit port in args, use client IP with that port
                let target_addr = if let Some(OscType::Int(port)) = msg.args.first() {
                    SocketAddr::new(msg.sender.ip(), *port as u16)
                } else {
                    msg.sender
                };
                self.broadcaster.add_subscriber(target_addr);
            } else if msg.is_unsubscribe() {
                self.broadcaster.remove_subscriber(&msg.sender);
            }
        }
        Ok(incoming)
    }

    /// Executes an in-memory loopback verification test over local UDP sockets.
    pub fn run_loopback_test() -> Result<(), OscError> {
        let bridge = Self::new_ephemeral()?;
        let bridge_server_addr = bridge.server().local_addr()?;

        // Client socket
        let client_socket = UdpSocket::bind("127.0.0.1:0")?;
        client_socket.set_nonblocking(true)?;
        let client_addr = client_socket.local_addr()?;

        // 1. Client sends /subscribe to bridge
        let sub_msg = OscPacket::Message(OscMessage {
            addr: "/subscribe".to_string(),
            args: vec![OscType::Int(client_addr.port() as i32)],
        });
        let sub_bytes = encoder::encode(&sub_msg)?;
        client_socket.send_to(&sub_bytes, bridge_server_addr)?;

        // Bridge polls and processes subscription
        let msgs = bridge.poll()?;
        if msgs.is_empty() {
            return Err(OscError::Serialization("Expected subscribe message".to_string()));
        }
        if bridge.broadcaster().subscriber_count() == 0 {
            return Err(OscError::Serialization("Subscriber count should be > 0".to_string()));
        }

        // 2. Bridge broadcasts a parameter update to subscribers
        let sent = bridge
            .broadcaster()
            .broadcast_param("bs2", "filter_cutoff", 0.75)?;
        if sent == 0 {
            return Err(OscError::Serialization("Broadcast sent count was 0".to_string()));
        }

        // 3. Client receives broadcast datagram
        let mut buf = [0u8; 1024];
        let mut received = false;
        for _ in 0..50 {
            match client_socket.recv_from(&mut buf) {
                Ok((len, _)) => {
                    let (_, packet) = decoder::decode_udp(&buf[..len])?;
                    if let OscPacket::Message(msg) = packet {
                        if msg.addr == "/synth/bs2/param/filter_cutoff" {
                            received = true;
                            break;
                        }
                    }
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                Err(e) => return Err(OscError::Io(e)),
            }
        }

        if !received {
            return Err(OscError::Serialization(
                "Client failed to receive parameter broadcast".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osc_incoming_message_parsing_and_helpers() {
        let dummy_addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();

        let param_msg = OscIncomingMessage {
            sender: dummy_addr,
            address: "/synth/bs2/param/cutoff".to_string(),
            args: vec![OscType::Float(0.85)],
        };
        assert_eq!(param_msg.param_float(), Some(0.85));
        assert_eq!(param_msg.parse_synth_param(), Some(("bs2", "cutoff")));
        assert!(!param_msg.is_panic());
        assert!(!param_msg.is_subscribe());

        let panic_msg = OscIncomingMessage {
            sender: dummy_addr,
            address: "/synth/td3/panic".to_string(),
            args: vec![],
        };
        assert!(panic_msg.is_panic());

        let sub_msg = OscIncomingMessage {
            sender: dummy_addr,
            address: "/subscribe".to_string(),
            args: vec![],
        };
        assert!(sub_msg.is_subscribe());
    }

    #[test]
    fn osc_udp_bidirectional_loopback_verification() {
        let result = OscBridge::run_loopback_test();
        assert!(result.is_ok(), "Loopback test failed: {:?}", result.err());
    }

    #[test]
    fn osc_broadcaster_subscriber_lifecycle() {
        let broadcaster = OscBroadcaster::new("127.0.0.1", 0).unwrap();
        let addr1: SocketAddr = "127.0.0.1:8001".parse().unwrap();
        let addr2: SocketAddr = "127.0.0.1:8002".parse().unwrap();

        assert_eq!(broadcaster.subscriber_count(), 0);

        broadcaster.add_subscriber(addr1);
        broadcaster.add_subscriber(addr2);
        assert_eq!(broadcaster.subscriber_count(), 2);

        broadcaster.remove_subscriber(&addr1);
        assert_eq!(broadcaster.subscriber_count(), 1);
        assert_eq!(broadcaster.subscribers(), vec![addr2]);
    }
}
