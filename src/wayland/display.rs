//! wl_display implementation

use bytes::BytesMut;
use super::wire::Message;

/// wl_display opcodes (client -> server)
pub mod request {
    pub const SYNC: u16 = 0;
    pub const GET_REGISTRY: u16 = 1;
}

/// wl_display opcodes (server -> client)
pub mod event {
    pub const ERROR: u16 = 0;
    pub const DELETE_ID: u16 = 1;
}

/// wl_display implementation
pub struct WlDisplay;

impl WlDisplay {
    /// Handle incoming request
    pub fn handle_request(opcode: u16, payload: &mut BytesMut) -> Option<DisplayRequest> {
        match opcode {
            request::SYNC => {
                let callback_id = super::wire::read_uint(payload)?;
                Some(DisplayRequest::Sync { callback_id })
            }
            request::GET_REGISTRY => {
                let registry_id = super::wire::read_uint(payload)?;
                Some(DisplayRequest::GetRegistry { registry_id })
            }
            _ => None,
        }
    }

    /// Create error event
    pub fn error(object_id: u32, code: u32, message: &str) -> Message {
        Message::new(1, event::ERROR)
            .uint(object_id)
            .uint(code)
            .string(message)
    }

    /// Create delete_id event
    pub fn delete_id(id: u32) -> Message {
        Message::new(1, event::DELETE_ID)
            .uint(id)
    }
}

/// Requests for wl_display
#[derive(Debug)]
pub enum DisplayRequest {
    Sync { callback_id: u32 },
    GetRegistry { registry_id: u32 },
}
