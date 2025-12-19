//! Waypipe Protocol Parser
//!
//! This module handles parsing and serialization of Waypipe messages.
//! Waypipe uses a simple framing protocol over the socket.

use bytes::{Buf, BufMut, BytesMut};
use log::debug;

use crate::error::{Result, WinWayError};

/// Waypipe message types
#[derive(Debug, Clone)]
pub enum MessageType {
    /// Initial handshake
    Hello,
    /// Frame buffer update
    Buffer { width: u32, height: u32, data: Vec<u8> },
    /// Input event (keyboard/mouse)
    Input { event_type: u8, data: Vec<u8> },
    /// Window management
    Window { action: WindowAction },
    /// Raw passthrough data
    Raw(Vec<u8>),
}

#[derive(Debug, Clone)]
pub enum WindowAction {
    Create { id: u32, width: u32, height: u32 },
    Destroy { id: u32 },
    Resize { id: u32, width: u32, height: u32 },
    Move { id: u32, x: i32, y: i32 },
}

/// A parsed message from the wire
#[derive(Debug, Clone)]
pub struct Message {
    pub msg_type: MessageType,
}

/// Protocol decoder for streaming data
pub struct Decoder {
    buffer: BytesMut,
}

impl Decoder {
    pub fn new() -> Self {
        Self {
            buffer: BytesMut::with_capacity(65536),
        }
    }

    /// Add data to the buffer
    pub fn push(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    /// Try to decode the next message
    /// 
    /// For now, we use a simple pass-through mode since we're bridging
    /// raw Waypipe data without deep parsing.
    pub fn decode(&mut self) -> Option<Message> {
        if self.buffer.is_empty() {
            return None;
        }

        // For initial implementation: pass through all data as raw
        // This allows waypipe to handle its own protocol
        let data = self.buffer.split().to_vec();
        debug!("Decoded {} bytes of raw data", data.len());
        
        Some(Message {
            msg_type: MessageType::Raw(data),
        })
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

/// Protocol encoder for outgoing data
pub struct Encoder;

impl Encoder {
    pub fn new() -> Self {
        Self
    }

    /// Encode a message for sending
    pub fn encode(&self, msg: &Message) -> Vec<u8> {
        match &msg.msg_type {
            MessageType::Raw(data) => data.clone(),
            // Future: implement proper encoding for other message types
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoder_raw_passthrough() {
        let mut decoder = Decoder::new();
        decoder.push(b"hello world");
        
        let msg = decoder.decode();
        assert!(msg.is_some());
        
        if let Some(Message { msg_type: MessageType::Raw(data) }) = msg {
            assert_eq!(data, b"hello world");
        } else {
            panic!("Expected Raw message");
        }
    }
}
