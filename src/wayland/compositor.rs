//! wl_compositor and wl_surface implementation

use super::wire::Message;
use super::object::Interface;

/// wl_compositor opcodes (client -> server)
pub mod compositor_request {
    pub const CREATE_SURFACE: u16 = 0;
    pub const CREATE_REGION: u16 = 1;
}

/// wl_surface opcodes (client -> server)
pub mod surface_request {
    pub const DESTROY: u16 = 0;
    pub const ATTACH: u16 = 1;
    pub const DAMAGE: u16 = 2;
    pub const FRAME: u16 = 3;
    pub const SET_OPAQUE_REGION: u16 = 4;
    pub const SET_INPUT_REGION: u16 = 5;
    pub const COMMIT: u16 = 6;
    pub const SET_BUFFER_TRANSFORM: u16 = 7;
    pub const SET_BUFFER_SCALE: u16 = 8;
    pub const DAMAGE_BUFFER: u16 = 9;
    pub const OFFSET: u16 = 10;
}

/// wl_surface opcodes (server -> client)
pub mod surface_event {
    pub const ENTER: u16 = 0;
    pub const LEAVE: u16 = 1;
    pub const PREFERRED_BUFFER_SCALE: u16 = 2;
    pub const PREFERRED_BUFFER_TRANSFORM: u16 = 3;
}

/// wl_callback opcodes (server -> client)
pub mod callback_event {
    pub const DONE: u16 = 0;
}

/// Surface state
#[derive(Debug, Default)]
pub struct Surface {
    pub id: u32,
    pub buffer_id: Option<u32>,
    pub buffer_x: i32,
    pub buffer_y: i32,
    pub committed: bool,
    pub frame_callback: Option<u32>,
}

impl Surface {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            ..Default::default()
        }
    }
}

/// Create callback done event
pub fn callback_done(callback_id: u32, time: u32) -> Message {
    Message::new(callback_id, callback_event::DONE)
        .uint(time)
}

/// Create surface enter event (surface entered output)
pub fn surface_enter(surface_id: u32, output_id: u32) -> Message {
    Message::new(surface_id, surface_event::ENTER)
        .uint(output_id)
}
