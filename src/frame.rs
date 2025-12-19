//! Protocol for receiving render frames from winpipe
//!
//! Frame format from winpipe:
//! - Magic (4 bytes): "WPRD"
//! - Width (4 bytes, LE)
//! - Height (4 bytes, LE)
//! - Format (4 bytes, LE)
//! - Data size (4 bytes, LE)
//! - Data (N bytes)

/// Magic bytes for render frame
pub const FRAME_MAGIC: &[u8; 4] = b"WPRD";

/// Frame header size
pub const HEADER_SIZE: usize = 20;

/// A render frame received from winpipe
#[derive(Debug)]
pub struct RenderFrame {
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub data: Vec<u8>,
}

impl RenderFrame {
    /// Decode from wire format
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < HEADER_SIZE {
            return None;
        }

        // Check magic
        if &data[0..4] != FRAME_MAGIC {
            return None;
        }

        let width = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let height = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
        let format = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
        let data_size = u32::from_le_bytes([data[16], data[17], data[18], data[19]]) as usize;

        if data.len() < HEADER_SIZE + data_size {
            return None;
        }

        Some(Self {
            width,
            height,
            format,
            data: data[HEADER_SIZE..HEADER_SIZE + data_size].to_vec(),
        })
    }
}

/// Frame decoder for streaming data
pub struct FrameDecoder {
    buffer: Vec<u8>,
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(1024 * 1024),
        }
    }

    /// Add data to buffer
    pub fn push(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    /// Try to decode next frame
    pub fn decode(&mut self) -> Option<RenderFrame> {
        if self.buffer.len() < HEADER_SIZE {
            return None;
        }

        // Check magic
        if &self.buffer[0..4] != FRAME_MAGIC {
            // Skip to find next magic
            if let Some(pos) = self.find_magic() {
                self.buffer.drain(..pos);
            } else {
                self.buffer.clear();
            }
            return None;
        }

        // Get data size
        let data_size = u32::from_le_bytes([
            self.buffer[16], self.buffer[17], self.buffer[18], self.buffer[19]
        ]) as usize;

        let total_size = HEADER_SIZE + data_size;
        if self.buffer.len() < total_size {
            return None;
        }

        // Decode frame
        match RenderFrame::decode(&self.buffer[..total_size]) {
            Some(frame) => {
                self.buffer.drain(..total_size);
                Some(frame)
            }
            None => {
                self.buffer.drain(..4);
                None
            }
        }
    }

    fn find_magic(&self) -> Option<usize> {
        self.buffer.windows(4).position(|w| w == FRAME_MAGIC)
    }

    /// Clear buffer
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

impl Default for FrameDecoder {
    fn default() -> Self {
        Self::new()
    }
}
