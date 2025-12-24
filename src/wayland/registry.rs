//! wl_registry implementation

use super::wire::Message;

/// wl_registry opcodes (client -> server)
pub mod request {
    pub const BIND: u16 = 0;
}

/// wl_registry opcodes (server -> client)
pub mod event {
    pub const GLOBAL: u16 = 0;
    pub const GLOBAL_REMOVE: u16 = 1;
}

/// Global object info
#[derive(Debug, Clone)]
pub struct Global {
    pub name: u32,
    pub interface: String,
    pub version: u32,
}

/// Standard globals that we advertise
pub fn standard_globals() -> Vec<Global> {
    vec![
        Global { name: 1, interface: "wl_compositor".into(), version: 5 },
        Global { name: 2, interface: "wl_subcompositor".into(), version: 1 },
        Global { name: 3, interface: "wl_shm".into(), version: 1 },
        Global { name: 4, interface: "xdg_wm_base".into(), version: 3 },
        Global { name: 5, interface: "wl_seat".into(), version: 7 },
        Global { name: 6, interface: "wl_output".into(), version: 4 },
        Global { name: 7, interface: "wl_data_device_manager".into(), version: 3 },
    ]
}

/// wl_registry implementation
pub struct WlRegistry;

impl WlRegistry {
    /// Create global event
    pub fn global(registry_id: u32, name: u32, interface: &str, version: u32) -> Message {
        Message::new(registry_id, event::GLOBAL)
            .uint(name)
            .string(interface)
            .uint(version)
    }

    /// Create global_remove event  
    pub fn global_remove(registry_id: u32, name: u32) -> Message {
        Message::new(registry_id, event::GLOBAL_REMOVE)
            .uint(name)
    }
}

/// Bind request data
#[derive(Debug)]
pub struct BindRequest {
    pub name: u32,
    pub interface: String,
    pub version: u32,
    pub id: u32,
}
