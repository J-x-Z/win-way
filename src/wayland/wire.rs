//! Wayland wire protocol encoding and decoding
//!
//! The Wayland wire protocol format:
//! - Header: object_id (4 bytes) + opcode_and_size (4 bytes)
//! - Payload: arguments

use std::io::{self, Read, Write};
use bytes::{Buf, BufMut, BytesMut};

/// Wayland message header size
pub const HEADER_SIZE: usize = 8;

/// Wayland argument types
#[derive(Debug, Clone)]
pub enum Argument {
    Int(i32),
    Uint(u32),
    Fixed(i32), // 24.8 fixed point
    String(String),
    Object(u32),
    NewId(u32),
    Array(Vec<u8>),
    Fd, // File descriptor (handled separately)
}

/// A decoded Wayland message
#[derive(Debug, Clone)]
pub struct Message {
    /// Object ID this message is for
    pub object_id: u32,
    /// Opcode (method index)
    pub opcode: u16,
    /// Message arguments
    pub args: Vec<Argument>,
}

impl Message {
    /// Create a new message
    pub fn new(object_id: u32, opcode: u16) -> Self {
        Self {
            object_id,
            opcode,
            args: Vec::new(),
        }
    }

    /// Add an argument
    pub fn arg(mut self, arg: Argument) -> Self {
        self.args.push(arg);
        self
    }

    /// Add uint argument
    pub fn uint(self, value: u32) -> Self {
        self.arg(Argument::Uint(value))
    }

    /// Add int argument
    pub fn int(self, value: i32) -> Self {
        self.arg(Argument::Int(value))
    }

    /// Add string argument
    pub fn string(self, value: impl Into<String>) -> Self {
        self.arg(Argument::String(value.into()))
    }

    /// Add object argument
    pub fn object(self, id: u32) -> Self {
        self.arg(Argument::Object(id))
    }

    /// Add new_id argument
    pub fn new_id(self, id: u32) -> Self {
        self.arg(Argument::NewId(id))
    }

    /// Encode this message to bytes
    pub fn encode(&self) -> BytesMut {
        let mut payload = BytesMut::new();
        
        for arg in &self.args {
            match arg {
                Argument::Int(v) => payload.put_i32_le(*v),
                Argument::Uint(v) => payload.put_u32_le(*v),
                Argument::Fixed(v) => payload.put_i32_le(*v),
                Argument::Object(v) => payload.put_u32_le(*v),
                Argument::NewId(v) => payload.put_u32_le(*v),
                Argument::String(s) => {
                    let bytes = s.as_bytes();
                    let len = bytes.len() as u32 + 1; // Include null terminator
                    payload.put_u32_le(len);
                    payload.put_slice(bytes);
                    payload.put_u8(0); // Null terminator
                    // Pad to 4-byte boundary
                    let padding = (4 - (len as usize % 4)) % 4;
                    for _ in 0..padding {
                        payload.put_u8(0);
                    }
                }
                Argument::Array(data) => {
                    payload.put_u32_le(data.len() as u32);
                    payload.put_slice(data);
                    // Pad to 4-byte boundary
                    let padding = (4 - (data.len() % 4)) % 4;
                    for _ in 0..padding {
                        payload.put_u8(0);
                    }
                }
                Argument::Fd => {
                    // FDs are passed out-of-band, nothing in payload
                }
            }
        }

        let size = (HEADER_SIZE + payload.len()) as u32;
        let opcode_and_size = ((size << 16) | self.opcode as u32);

        let mut result = BytesMut::with_capacity(size as usize);
        result.put_u32_le(self.object_id);
        result.put_u32_le(opcode_and_size);
        result.put_slice(&payload);

        result
    }
}

/// Wayland message decoder
pub struct MessageDecoder {
    pub buffer: BytesMut,
}

impl MessageDecoder {
    pub fn new() -> Self {
        Self {
            buffer: BytesMut::with_capacity(65536),
        }
    }

    /// Push data into the decoder buffer
    pub fn push(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    /// Try to decode the next message
    pub fn decode(&mut self) -> Option<(u32, u16, BytesMut)> {
        if self.buffer.len() < HEADER_SIZE {
            return None;
        }

        // Peek at header
        let object_id = u32::from_le_bytes([
            self.buffer[0], self.buffer[1], self.buffer[2], self.buffer[3]
        ]);
        let opcode_and_size = u32::from_le_bytes([
            self.buffer[4], self.buffer[5], self.buffer[6], self.buffer[7]
        ]);

        let size = (opcode_and_size >> 16) as usize;
        let opcode = (opcode_and_size & 0xFFFF) as u16;

        if size < HEADER_SIZE || self.buffer.len() < size {
            return None;
        }

        // Consume the message
        self.buffer.advance(HEADER_SIZE);
        let payload = self.buffer.split_to(size - HEADER_SIZE);

        Some((object_id, opcode, payload))
    }
}

impl Default for MessageDecoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Read a string from payload
pub fn read_string(payload: &mut BytesMut) -> Option<String> {
    if payload.len() < 4 {
        return None;
    }
    let len = payload.get_u32_le() as usize;
    if payload.len() < len {
        return None;
    }
    let bytes = payload.split_to(len);
    // Remove null terminator and trailing padding
    let s = String::from_utf8_lossy(&bytes[..len.saturating_sub(1)]).into_owned();
    // Skip padding
    let padded = (len + 3) & !3;
    if padded > len {
        payload.advance(padded - len);
    }
    Some(s)
}

/// Read a uint from payload
pub fn read_uint(payload: &mut BytesMut) -> Option<u32> {
    if payload.len() < 4 {
        return None;
    }
    Some(payload.get_u32_le())
}

/// Read an int from payload
pub fn read_int(payload: &mut BytesMut) -> Option<i32> {
    if payload.len() < 4 {
        return None;
    }
    Some(payload.get_i32_le())
}
