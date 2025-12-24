//! wl_shm and wl_shm_pool implementation

use std::collections::HashMap;
use super::wire::Message;

/// wl_shm opcodes (client -> server)  
pub mod shm_request {
    pub const CREATE_POOL: u16 = 0;
}

/// wl_shm opcodes (server -> client)
pub mod shm_event {
    pub const FORMAT: u16 = 0;
}

/// wl_shm_pool opcodes (client -> server)
pub mod pool_request {
    pub const CREATE_BUFFER: u16 = 0;
    pub const DESTROY: u16 = 1;
    pub const RESIZE: u16 = 2;
}

/// wl_buffer opcodes (client -> server)
pub mod buffer_request {
    pub const DESTROY: u16 = 0;
}

/// wl_buffer opcodes (server -> client)
pub mod buffer_event {
    pub const RELEASE: u16 = 0;
}

/// SHM formats we support
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum ShmFormat {
    Argb8888 = 0,
    Xrgb8888 = 1,
    Rgb888 = 20,
    Bgr888 = 21,
    Rgba8888 = 0x34324152,
}

/// Get supported SHM formats
pub fn supported_formats() -> Vec<u32> {
    vec![
        ShmFormat::Argb8888 as u32,
        ShmFormat::Xrgb8888 as u32,
    ]
}

/// Create shm format event
pub fn shm_format(shm_id: u32, format: u32) -> Message {
    Message::new(shm_id, shm_event::FORMAT)
        .uint(format)
}

/// Create buffer release event
pub fn buffer_release(buffer_id: u32) -> Message {
    Message::new(buffer_id, buffer_event::RELEASE)
}

/// SHM pool info
#[derive(Debug)]
pub struct ShmPool {
    pub id: u32,
    pub size: u32,
    // Note: On Windows, we'd need to map shared memory differently
    // For now, we store buffer data directly
}

/// Buffer info
#[derive(Debug)]
pub struct Buffer {
    pub id: u32,
    pub pool_id: u32,
    pub offset: i32,
    pub width: i32,
    pub height: i32,
    pub stride: i32,
    pub format: u32,
    /// Pixel data (copied from shared memory)
    pub data: Vec<u8>,
}

impl Buffer {
    pub fn new(id: u32, pool_id: u32, offset: i32, width: i32, height: i32, stride: i32, format: u32) -> Self {
        Self {
            id,
            pool_id,
            offset,
            width,
            height,
            stride,
            format,
            data: Vec::new(),
        }
    }
}
