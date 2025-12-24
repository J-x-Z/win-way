//! xdg_shell implementation (xdg_wm_base, xdg_surface, xdg_toplevel)

use super::wire::Message;

/// xdg_wm_base opcodes (client -> server)
pub mod wm_base_request {
    pub const DESTROY: u16 = 0;
    pub const CREATE_POSITIONER: u16 = 1;
    pub const GET_XDG_SURFACE: u16 = 2;
    pub const PONG: u16 = 3;
}

/// xdg_wm_base opcodes (server -> client)
pub mod wm_base_event {
    pub const PING: u16 = 0;
}

/// xdg_surface opcodes (client -> server)
pub mod xdg_surface_request {
    pub const DESTROY: u16 = 0;
    pub const GET_TOPLEVEL: u16 = 1;
    pub const GET_POPUP: u16 = 2;
    pub const SET_WINDOW_GEOMETRY: u16 = 3;
    pub const ACK_CONFIGURE: u16 = 4;
}

/// xdg_surface opcodes (server -> client)
pub mod xdg_surface_event {
    pub const CONFIGURE: u16 = 0;
}

/// xdg_toplevel opcodes (client -> server)
pub mod toplevel_request {
    pub const DESTROY: u16 = 0;
    pub const SET_PARENT: u16 = 1;
    pub const SET_TITLE: u16 = 2;
    pub const SET_APP_ID: u16 = 3;
    pub const SHOW_WINDOW_MENU: u16 = 4;
    pub const MOVE: u16 = 5;
    pub const RESIZE: u16 = 6;
    pub const SET_MAX_SIZE: u16 = 7;
    pub const SET_MIN_SIZE: u16 = 8;
    pub const SET_MAXIMIZED: u16 = 9;
    pub const UNSET_MAXIMIZED: u16 = 10;
    pub const SET_FULLSCREEN: u16 = 11;
    pub const UNSET_FULLSCREEN: u16 = 12;
    pub const SET_MINIMIZED: u16 = 13;
}

/// xdg_toplevel opcodes (server -> client)
pub mod toplevel_event {
    pub const CONFIGURE: u16 = 0;
    pub const CLOSE: u16 = 1;
    pub const CONFIGURE_BOUNDS: u16 = 2;
}

/// XDG toplevel state flags
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum ToplevelState {
    Maximized = 1,
    Fullscreen = 2,
    Resizing = 3,
    Activated = 4,
    TiledLeft = 5,
    TiledRight = 6,
    TiledTop = 7,
    TiledBottom = 8,
}

/// XDG surface state
#[derive(Debug, Default)]
pub struct XdgSurface {
    pub id: u32,
    pub surface_id: u32,
    pub toplevel_id: Option<u32>,
}

/// XDG toplevel state
#[derive(Debug, Default)]
pub struct XdgToplevel {
    pub id: u32,
    pub xdg_surface_id: u32,
    pub title: String,
    pub app_id: String,
    pub min_size: (i32, i32),
    pub max_size: (i32, i32),
}

/// Create xdg_wm_base ping event
pub fn wm_base_ping(wm_base_id: u32, serial: u32) -> Message {
    Message::new(wm_base_id, wm_base_event::PING)
        .uint(serial)
}

/// Create xdg_surface configure event
pub fn xdg_surface_configure(surface_id: u32, serial: u32) -> Message {
    Message::new(surface_id, xdg_surface_event::CONFIGURE)
        .uint(serial)
}

/// Create xdg_toplevel configure event
pub fn toplevel_configure(toplevel_id: u32, width: i32, height: i32, states: &[ToplevelState]) -> Message {
    let states_bytes: Vec<u8> = states.iter()
        .flat_map(|s| (*s as u32).to_le_bytes())
        .collect();
    
    Message::new(toplevel_id, toplevel_event::CONFIGURE)
        .int(width)
        .int(height)
        .arg(super::wire::Argument::Array(states_bytes))
}

/// Create xdg_toplevel close event
pub fn toplevel_close(toplevel_id: u32) -> Message {
    Message::new(toplevel_id, toplevel_event::CLOSE)
}
