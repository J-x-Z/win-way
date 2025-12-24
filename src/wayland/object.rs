//! Wayland object ID management

use std::collections::HashMap;

/// Object interface types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interface {
    Display,
    Registry,
    Compositor,
    Subcompositor,
    Surface,
    Subsurface,
    Region,
    Shm,
    ShmPool,
    Buffer,
    XdgWmBase,
    XdgSurface,
    XdgToplevel,
    XdgPositioner,
    Seat,
    Keyboard,
    Pointer,
    Output,
    Callback,
    DataDeviceManager,
    DataDevice,
}

/// A Wayland object
#[derive(Debug, Clone)]
pub struct Object {
    pub id: u32,
    pub interface: Interface,
    pub version: u32,
}

/// Object map for tracking all objects
#[derive(Debug)]
pub struct ObjectMap {
    objects: HashMap<u32, Object>,
    next_server_id: u32, // Server IDs start from 0xFF000000
}

impl ObjectMap {
    pub fn new() -> Self {
        let mut map = Self {
            objects: HashMap::new(),
            next_server_id: 0xFF000000,
        };
        // wl_display is always object 1
        map.insert(Object {
            id: 1,
            interface: Interface::Display,
            version: 1,
        });
        map
    }

    /// Insert an object
    pub fn insert(&mut self, obj: Object) {
        self.objects.insert(obj.id, obj);
    }

    /// Get an object by ID
    pub fn get(&self, id: u32) -> Option<&Object> {
        self.objects.get(&id)
    }

    /// Remove an object
    pub fn remove(&mut self, id: u32) -> Option<Object> {
        self.objects.remove(&id)
    }

    /// Allocate a new server-side ID
    pub fn alloc_server_id(&mut self, interface: Interface, version: u32) -> u32 {
        let id = self.next_server_id;
        self.next_server_id += 1;
        self.insert(Object { id, interface, version });
        id
    }
}

impl Default for ObjectMap {
    fn default() -> Self {
        Self::new()
    }
}
