//! Wayland client handler
//!
//! Processes messages from a connected Wayland client

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use bytes::BytesMut;
use log::{debug, info, warn, error};

use super::wire::{Message, MessageDecoder, read_uint, read_int, read_string};
use super::object::{ObjectMap, Object, Interface};
use super::display::{WlDisplay, DisplayRequest};
use super::registry::{WlRegistry, standard_globals, Global};
use super::compositor::{Surface, callback_done};
use super::shm::{ShmPool, Buffer, supported_formats, shm_format, buffer_release};
use super::xdg_shell::{XdgSurface, XdgToplevel, toplevel_configure, xdg_surface_configure, ToplevelState};

static SERIAL: AtomicU32 = AtomicU32::new(1);

pub fn next_serial() -> u32 {
    SERIAL.fetch_add(1, Ordering::SeqCst)
}

/// Events to send to the renderer
#[derive(Debug)]
pub enum RenderEvent {
    /// New surface created
    SurfaceCreated { id: u32 },
    /// Surface buffer attached and committed
    SurfaceCommit { 
        surface_id: u32, 
        width: i32, 
        height: i32,
        data: Vec<u8>,
    },
    /// Surface destroyed
    SurfaceDestroyed { id: u32 },
    /// Toplevel title changed
    TitleChanged { surface_id: u32, title: String },
    /// PIXL data
    PixelData {
        surface_id: u32,
        width: u32,
        height: u32,
        format: u32,
        data: Vec<u8>,
    },
}

/// Wayland client state
pub struct WaylandClient {
    pub id: u32,
    pub decoder: MessageDecoder,
    objects: ObjectMap,
    surfaces: HashMap<u32, Surface>,
    pools: HashMap<u32, ShmPool>,
    buffers: HashMap<u32, Buffer>,
    xdg_surfaces: HashMap<u32, XdgSurface>,
    toplevels: HashMap<u32, XdgToplevel>,
    registry_id: Option<u32>,
    shm_id: Option<u32>,
    compositor_id: Option<u32>,
    wm_base_id: Option<u32>,
    output_id: Option<u32>,
    seat_id: Option<u32>,
    keyboard_id: Option<u32>,
    pointer_id: Option<u32>,
    /// Outgoing messages
    outgoing: Vec<Message>,
    /// Render events
    render_events: Vec<RenderEvent>,
    /// Shared memory data received over the wire
    shm_data: HashMap<u32, Vec<u8>>,
}

impl WaylandClient {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            decoder: MessageDecoder::new(),
            objects: ObjectMap::new(),
            surfaces: HashMap::new(),
            pools: HashMap::new(),
            buffers: HashMap::new(),
            xdg_surfaces: HashMap::new(),
            toplevels: HashMap::new(),
            registry_id: None,
            shm_id: None,
            compositor_id: None,
            wm_base_id: None,
            output_id: None,
            seat_id: None,
            keyboard_id: None,
            pointer_id: None,
            outgoing: Vec::new(),
            render_events: Vec::new(),
            shm_data: HashMap::new(),
        }
    }

    /// Process incoming data
    pub fn process(&mut self, data: &[u8]) {
        self.decoder.push(data);
        
        loop {
            // Check for PIXL header first (4 bytes 'PIXL' + 5*4 bytes ints = 24 bytes)
            if self.decoder.buffer.len() >= 24 {
                let magic = &self.decoder.buffer[0..4];
                if magic == b"PIXL" {
                    use bytes::Buf;
                    
                    // Decode header without consuming yet
                    let sid = u32::from_le_bytes(self.decoder.buffer[4..8].try_into().unwrap());
                    let w = u32::from_le_bytes(self.decoder.buffer[8..12].try_into().unwrap());
                    let h = u32::from_le_bytes(self.decoder.buffer[12..16].try_into().unwrap());
                    let fmt = u32::from_le_bytes(self.decoder.buffer[16..20].try_into().unwrap());
                    let len = u32::from_le_bytes(self.decoder.buffer[20..24].try_into().unwrap()) as usize;
                    
                    if self.decoder.buffer.len() >= 24 + len {
                        // Consume header
                        self.decoder.buffer.advance(24);
                        // Consume payload
                        let payload = self.decoder.buffer.split_to(len).to_vec();
                        
                        self.render_events.push(RenderEvent::PixelData {
                            surface_id: sid,
                            width: w,
                            height: h,
                            format: fmt,
                            data: payload,
                        });
                        continue;
                    } else {
                        // Wait for more data
                        break;
                    }
                }
            }
        
            if let Some((object_id, opcode, mut payload)) = self.decoder.decode() {
                self.handle_message(object_id, opcode, &mut payload);
            } else {
                break;
            }
        }
    }

    /// Get outgoing messages to send
    pub fn take_outgoing(&mut self) -> Vec<Message> {
        std::mem::take(&mut self.outgoing)
    }

    /// Get render events
    pub fn take_render_events(&mut self) -> Vec<RenderEvent> {
        std::mem::take(&mut self.render_events)
    }

    /// Handle a decoded message
    fn handle_message(&mut self, object_id: u32, opcode: u16, payload: &mut BytesMut) {
        let interface = self.objects.get(object_id).map(|o| o.interface);
        
        match interface {
            Some(Interface::Display) => self.handle_display(opcode, payload),
            Some(Interface::Registry) => self.handle_registry(opcode, payload),
            Some(Interface::Compositor) => self.handle_compositor(opcode, payload),
            Some(Interface::Surface) => self.handle_surface(object_id, opcode, payload),
            Some(Interface::Seat) => self.handle_seat(opcode, payload),
            Some(Interface::Shm) => self.handle_shm(opcode, payload),
            Some(Interface::ShmPool) => self.handle_shm_pool(object_id, opcode, payload),
            Some(Interface::Buffer) => self.handle_buffer(object_id, opcode, payload),
            Some(Interface::XdgWmBase) => self.handle_xdg_wm_base(opcode, payload),
            Some(Interface::XdgSurface) => self.handle_xdg_surface(object_id, opcode, payload),
            Some(Interface::XdgToplevel) => self.handle_xdg_toplevel(object_id, opcode, payload),
            Some(Interface::Callback) => { /* Callbacks are server->client only */ }
            Some(other) => {
                debug!("Unhandled interface {:?} object {} opcode {}", other, object_id, opcode);
            }
            None => {
                warn!("Unknown object {} opcode {}", object_id, opcode);
            }
        }
    }

    fn handle_display(&mut self, opcode: u16, payload: &mut BytesMut) {
        match opcode {
            0 => { // sync
                let callback_id = read_uint(payload).unwrap_or(0);
                debug!("wl_display.sync -> callback {}", callback_id);
                
                self.objects.insert(Object {
                    id: callback_id,
                    interface: Interface::Callback,
                    version: 1,
                });
                
                // Send callback done immediately
                let time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u32)
                    .unwrap_or(0);
                self.outgoing.push(callback_done(callback_id, time));
            }
            1 => { // get_registry
                let registry_id = read_uint(payload).unwrap_or(0);
                debug!("wl_display.get_registry -> {}", registry_id);
                
                self.registry_id = Some(registry_id);
                self.objects.insert(Object {
                    id: registry_id,
                    interface: Interface::Registry,
                    version: 1,
                });
                
                // Send all globals
                for global in standard_globals() {
                    self.outgoing.push(WlRegistry::global(
                        registry_id,
                        global.name,
                        &global.interface,
                        global.version,
                    ));
                }
            }
            _ => warn!("Unknown display opcode {}", opcode),
        }
    }

    fn handle_registry(&mut self, opcode: u16, payload: &mut BytesMut) {
        match opcode {
            0 => { // bind
                let name = read_uint(payload).unwrap_or(0);
                let interface = read_string(payload).unwrap_or_default();
                let version = read_uint(payload).unwrap_or(1);
                let id = read_uint(payload).unwrap_or(0);
                
                debug!("wl_registry.bind: name={} interface={} version={} -> id={}", 
                    name, interface, version, id);
                
                match interface.as_str() {
                    "wl_compositor" => {
                        self.compositor_id = Some(id);
                        self.objects.insert(Object { id, interface: Interface::Compositor, version });
                    }
                    "wl_subcompositor" => {
                        self.objects.insert(Object { id, interface: Interface::Subcompositor, version });
                    }
                    "wl_shm" => {
                        self.shm_id = Some(id);
                        self.objects.insert(Object { id, interface: Interface::Shm, version });
                        // Send supported formats
                        for format in supported_formats() {
                            self.outgoing.push(shm_format(id, format));
                        }
                    }
                    "xdg_wm_base" => {
                        self.wm_base_id = Some(id);
                        self.objects.insert(Object { id, interface: Interface::XdgWmBase, version });
                    }
                    "wl_seat" => {
                        self.seat_id = Some(id);
                        self.objects.insert(Object { id, interface: Interface::Seat, version });
                        
                        // Send capabilities (Pointer = 1, Keyboard = 2 -> Both = 3)
                        self.outgoing.push(Message::new(id, 0).uint(3));
                        
                        // Send name
                        self.outgoing.push(Message::new(id, 1).string("win-way-seat"));
                    }
                    "wl_output" => {
                        self.output_id = Some(id);
                        self.objects.insert(Object { id, interface: Interface::Output, version });
                    }
                    "wl_data_device_manager" => {
                        self.objects.insert(Object { id, interface: Interface::DataDeviceManager, version });
                    }
                    _ => {
                        warn!("Unknown interface to bind: {}", interface);
                    }
                }
            }
            _ => warn!("Unknown registry opcode {}", opcode),
        }
    }

    fn handle_compositor(&mut self, opcode: u16, payload: &mut BytesMut) {
        match opcode {
            0 => { // create_surface
                let surface_id = read_uint(payload).unwrap_or(0);
                debug!("wl_compositor.create_surface -> {}", surface_id);
                
                self.objects.insert(Object {
                    id: surface_id,
                    interface: Interface::Surface,
                    version: 5,
                });
                self.surfaces.insert(surface_id, Surface::new(surface_id));
                self.render_events.push(RenderEvent::SurfaceCreated { id: surface_id });
            }
            1 => { // create_region
                let region_id = read_uint(payload).unwrap_or(0);
                self.objects.insert(Object {
                    id: region_id,
                    interface: Interface::Region,
                    version: 1,
                });
            }
            _ => warn!("Unknown compositor opcode {}", opcode),
        }
    }

    fn handle_surface(&mut self, surface_id: u32, opcode: u16, payload: &mut BytesMut) {
        match opcode {
            0 => { // destroy
                self.surfaces.remove(&surface_id);
                self.objects.remove(surface_id);
                self.render_events.push(RenderEvent::SurfaceDestroyed { id: surface_id });
            }
            1 => { // attach
                let buffer_id = read_uint(payload).unwrap_or(0);
                let x = read_int(payload).unwrap_or(0);
                let y = read_int(payload).unwrap_or(0);
                
                if let Some(surface) = self.surfaces.get_mut(&surface_id) {
                    surface.buffer_id = if buffer_id == 0 { None } else { Some(buffer_id) };
                    surface.buffer_x = x;
                    surface.buffer_y = y;
                }
            }
            3 => { // frame
                let callback_id = read_uint(payload).unwrap_or(0);
                self.objects.insert(Object {
                    id: callback_id,
                    interface: Interface::Callback,
                    version: 1,
                });
                if let Some(surface) = self.surfaces.get_mut(&surface_id) {
                    surface.frame_callback = Some(callback_id);
                }
            }
            6 => { // commit
                if let Some(surface) = self.surfaces.get_mut(&surface_id) {
                    surface.committed = true;
                    
                    // If there's a buffer, send to renderer
                    if let Some(buffer_id) = surface.buffer_id {
                        if let Some(buffer) = self.buffers.get(&buffer_id) {
                            self.render_events.push(RenderEvent::SurfaceCommit {
                                surface_id,
                                width: buffer.width,
                                height: buffer.height,
                                data: buffer.data.clone(),
                            });
                            
                            // Release buffer
                            self.outgoing.push(buffer_release(buffer_id));
                        }
                    }
                    
                    // Send frame callback
                    if let Some(callback_id) = surface.frame_callback.take() {
                        let time = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_millis() as u32)
                            .unwrap_or(0);
                        self.outgoing.push(callback_done(callback_id, time));
                        // Destroy callback object
                        self.outgoing.push(WlDisplay::delete_id(callback_id));
                    }
                }
            }
            _ => debug!("Surface opcode {} not handled", opcode),
        }
    }

    fn handle_shm(&mut self, opcode: u16, payload: &mut BytesMut) {
        match opcode {
            0 => { // create_pool (fd, size)
                let pool_id = read_uint(payload).unwrap_or(0);
                let size = read_uint(payload).unwrap_or(0);
                
                debug!("wl_shm.create_pool -> id={} size={}", pool_id, size);
                
                self.objects.insert(Object {
                    id: pool_id,
                    interface: Interface::ShmPool,
                    version: 1,
                });
                self.pools.insert(pool_id, ShmPool { id: pool_id, size });
            }
            _ => warn!("Unknown shm opcode {}", opcode),
        }
    }

    fn handle_shm_pool(&mut self, pool_id: u32, opcode: u16, payload: &mut BytesMut) {
        match opcode {
            0 => { // create_buffer
                let buffer_id = read_uint(payload).unwrap_or(0);
                let offset = read_int(payload).unwrap_or(0);
                let width = read_int(payload).unwrap_or(0);
                let height = read_int(payload).unwrap_or(0);
                let stride = read_int(payload).unwrap_or(0);
                let format = read_uint(payload).unwrap_or(0);
                
                debug!("wl_shm_pool.create_buffer: id={} {}x{} stride={}", 
                    buffer_id, width, height, stride);
                
                self.objects.insert(Object {
                    id: buffer_id,
                    interface: Interface::Buffer,
                    version: 1,
                });
                self.buffers.insert(buffer_id, Buffer::new(
                    buffer_id, pool_id, offset, width, height, stride, format
                ));
            }
            1 => { // destroy
                self.pools.remove(&pool_id);
                self.objects.remove(pool_id);
            }
            2 => { // resize
                let size = read_int(payload).unwrap_or(0);
                if let Some(pool) = self.pools.get_mut(&pool_id) {
                    pool.size = size as u32;
                }
            }
            _ => warn!("Unknown shm_pool opcode {}", opcode),
        }
    }

    fn handle_buffer(&mut self, buffer_id: u32, opcode: u16, _payload: &mut BytesMut) {
        match opcode {
            0 => { // destroy
                self.buffers.remove(&buffer_id);
                self.objects.remove(buffer_id);
            }
            _ => {}
        }
    }

    fn handle_xdg_wm_base(&mut self, opcode: u16, payload: &mut BytesMut) {
        match opcode {
            0 => { // destroy
                self.wm_base_id = None;
            }
            1 => { // create_positioner
                let positioner_id = read_uint(payload).unwrap_or(0);
                self.objects.insert(Object {
                    id: positioner_id,
                    interface: Interface::XdgPositioner,
                    version: 3,
                });
            }
            2 => { // get_xdg_surface
                let xdg_surface_id = read_uint(payload).unwrap_or(0);
                let surface_id = read_uint(payload).unwrap_or(0);
                
                debug!("xdg_wm_base.get_xdg_surface: xdg_id={} surface_id={}", 
                    xdg_surface_id, surface_id);
                
                self.objects.insert(Object {
                    id: xdg_surface_id,
                    interface: Interface::XdgSurface,
                    version: 3,
                });
                self.xdg_surfaces.insert(xdg_surface_id, XdgSurface {
                    id: xdg_surface_id,
                    surface_id,
                    toplevel_id: None,
                });
            }
            3 => { // pong
                let serial = read_uint(payload).unwrap_or(0);
                debug!("xdg_wm_base.pong serial={}", serial);
            }
            _ => warn!("Unknown xdg_wm_base opcode {}", opcode),
        }
    }

    fn handle_xdg_surface(&mut self, xdg_surface_id: u32, opcode: u16, payload: &mut BytesMut) {
        match opcode {
            0 => { // destroy
                self.xdg_surfaces.remove(&xdg_surface_id);
                self.objects.remove(xdg_surface_id);
            }
            1 => { // get_toplevel
                let toplevel_id = read_uint(payload).unwrap_or(0);
                
                debug!("xdg_surface.get_toplevel -> {}", toplevel_id);
                
                self.objects.insert(Object {
                    id: toplevel_id,
                    interface: Interface::XdgToplevel,
                    version: 3,
                });
                
                if let Some(xdg_surface) = self.xdg_surfaces.get_mut(&xdg_surface_id) {
                    xdg_surface.toplevel_id = Some(toplevel_id);
                }
                
                self.toplevels.insert(toplevel_id, XdgToplevel {
                    id: toplevel_id,
                    xdg_surface_id,
                    ..Default::default()
                });
                
                // Send initial configure
                self.outgoing.push(toplevel_configure(toplevel_id, 800, 600, &[ToplevelState::Activated]));
                self.outgoing.push(xdg_surface_configure(xdg_surface_id, next_serial()));
            }
            4 => { // ack_configure
                let serial = read_uint(payload).unwrap_or(0);
                debug!("xdg_surface.ack_configure serial={}", serial);
            }
            _ => debug!("xdg_surface opcode {} not handled", opcode),
        }
    }

    fn handle_xdg_toplevel(&mut self, toplevel_id: u32, opcode: u16, payload: &mut BytesMut) {
        match opcode {
            0 => { // destroy
                self.toplevels.remove(&toplevel_id);
                self.objects.remove(toplevel_id);
            }
            2 => { // set_title
                let title = read_string(payload).unwrap_or_default();
                info!("Toplevel {} title: {}", toplevel_id, title);
                
                if let Some(toplevel) = self.toplevels.get_mut(&toplevel_id) {
                    toplevel.title = title.clone();
                    
                    // Find surface ID
                    if let Some(xdg) = self.xdg_surfaces.get(&toplevel.xdg_surface_id) {
                        self.render_events.push(RenderEvent::TitleChanged {
                            surface_id: xdg.surface_id,
                            title,
                        });
                    }
                }
            }
            3 => { // set_app_id
                let app_id = read_string(payload).unwrap_or_default();
                if let Some(toplevel) = self.toplevels.get_mut(&toplevel_id) {
                    toplevel.app_id = app_id;
                }
            }
            _ => debug!("xdg_toplevel opcode {} not handled", opcode),
        }
    }

    /// Set buffer data (received from shared memory transfer)
    pub fn set_buffer_data(&mut self, buffer_id: u32, data: Vec<u8>) {
        if let Some(buffer) = self.buffers.get_mut(&buffer_id) {
            buffer.data = data;
        }
    }

    fn handle_seat(&mut self, opcode: u16, payload: &mut BytesMut) {
        match opcode {
            0 => { // get_pointer
                let id = read_uint(payload).unwrap_or(0);
                self.objects.insert(Object { id, interface: Interface::Pointer, version: 1 });
                self.pointer_id = Some(id);
            }
            1 => { // get_keyboard
                let id = read_uint(payload).unwrap_or(0);
                self.objects.insert(Object { id, interface: Interface::Keyboard, version: 1 });
                self.keyboard_id = Some(id);
            }

             _ => {}
        }
    }

    pub fn send_key(&mut self, serial: u32, time: u32, key: u32, state: u32) {
        if let Some(id) = self.keyboard_id {
            // wl_keyboard.key(serial, time, key, state)
            self.outgoing.push(
                Message::new(id, 3)
                    .uint(serial)
                    .uint(time)
                    .uint(key)
                    .uint(state)
            );
        }
    }

    pub fn send_motion(&mut self, time: u32, x: f64, y: f64) {
         if let Some(id) = self.pointer_id {
             // wl_pointer.motion(time, x, y)
             // x, y are wl_fixed_t (24.8 fixed point)
             let x_fixed = (x * 256.0) as i32;
             let y_fixed = (y * 256.0) as i32;
             
             // Message helper doesn't have fixed(), use arg
             self.outgoing.push(
                 Message::new(id, 2)
                    .uint(time)
                    .arg(super::wire::Argument::Fixed(x_fixed))
                    .arg(super::wire::Argument::Fixed(y_fixed))
             );
         }
    }

    pub fn send_button(&mut self, serial: u32, time: u32, button: u32, state: u32) {
        if let Some(id) = self.pointer_id {
            // wl_pointer.button(serial, time, button, state)
             self.outgoing.push(
                Message::new(id, 3)
                    .uint(serial)
                    .uint(time)
                    .uint(button)
                    .uint(state)
            );
        }
    }
}
