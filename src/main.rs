//! Win-Way: Native Waypipe Server for Windows
//! 
//! This is the Windows-side receiver for Wayland applications running in WSL.
//! Connect from WSL using: socat TCP:<win_ip>:9999 UNIX-LISTEN:/tmp/wayland-0
//!
//! Usage: win-way [OPTIONS]
//!   --port <PORT>    TCP port to listen on (default: 9999)
//!   --debug          Enable debug logging

use std::sync::mpsc as std_mpsc;
use std::thread;
use std::time::Instant;

use clap::Parser;
use log::{info, debug, error};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
    keyboard::{Key, NamedKey, PhysicalKey, KeyCode},
    event::{ElementState, MouseButton},
};

use win_way::server::{ServerConfig, ServerEvent, ServerCommand, InputEvent, start_server};
use win_way::protocol::Decoder;
use win_way::renderer::Renderer;
use win_way::frame::FrameDecoder;
use win_way::wayland::WaylandClient;

/// Win-Way: Native Waypipe Server for Windows
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// TCP port to listen on
    #[arg(short, long, default_value_t = 9999)]
    port: u16,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,
}

/// Application state
struct App {
    window: Option<Window>,
    renderer: Renderer,
    decoder: Decoder,
    frame_decoder: FrameDecoder,
    server_rx: Option<std_mpsc::Receiver<ServerEvent>>,
    server_command_tx: Option<tokio::sync::broadcast::Sender<ServerCommand>>,
    wayland_clients: std::collections::HashMap<u32, WaylandClient>,
    client_count: u32,
    start_time: Instant,
    last_size: (u32, u32),
}

impl App {
    fn new(server_rx: std_mpsc::Receiver<ServerEvent>, server_command_tx: tokio::sync::broadcast::Sender<ServerCommand>) -> Self {
        Self {
            window: None,
            renderer: Renderer::new(),
            decoder: Decoder::new(),
            frame_decoder: FrameDecoder::new(),
            server_rx: Some(server_rx),
            server_command_tx: Some(server_command_tx),
            wayland_clients: std::collections::HashMap::new(),
            client_count: 0,
            start_time: Instant::now(),
            last_size: (800, 600),
        }
    }


    fn process_server_events(&mut self) {
        if let Some(rx) = &self.server_rx {
            // Non-blocking receive of all pending events (limit to 100 per frame to prevent starvation)
            let mut count = 0;
            while let Ok(event) = rx.try_recv() {
                if count > 100 {
                    break;
                }
                count += 1;
                match event {
                    ServerEvent::ClientConnected { id } => {
                        info!("🔗 Client {} connected", id);
                        self.client_count += 1;
                        self.update_title();
                    }
                    ServerEvent::ClientDisconnected { id } => {
                        info!("🔌 Client {} disconnected", id);
                        self.client_count = self.client_count.saturating_sub(1);
                        self.update_title();
                    }
                    ServerEvent::Data { id: _, data: _ } => {
                        // Handled by server now
                    }
                    ServerEvent::Render(client_id, render_event) => {
                        match render_event {
                            win_way::wayland::client::RenderEvent::SurfaceCreated { id: sid } => {
                                info!("🖼️ Surface {} created", sid);
                            }
                            win_way::wayland::client::RenderEvent::SurfaceCommit { surface_id, width, height, data } => {
                                info!("🎨 Surface {} commit: {}x{} ({} bytes)", surface_id, width, height, data.len());
                                self.renderer.update_surface(client_id, &data, width as u32, height as u32);
                            }
                            win_way::wayland::client::RenderEvent::SurfaceDestroyed { id: sid } => {
                                info!("💥 Surface {} destroyed", sid);
                            }
                            win_way::wayland::client::RenderEvent::TitleChanged { surface_id, title } => {
                                info!("📝 Surface {} title: {}", surface_id, title);
                                if let Some(window) = &self.window {
                                    window.set_title(&format!("Win-Way | {}", title));
                                }
                            }
                            win_way::wayland::client::RenderEvent::PixelData { .. } => {
                                // Handled via ServerEvent::PixelData
                            }
                        }
                    }
                    ServerEvent::PixelData { client_id, surface_id, width, height, format: _, data } => {
                        info!("🎨 PIXL: Surface {} from client {} - {}x{} ({} bytes)", 
                            surface_id, client_id, width, height, data.len());
                        self.renderer.update_surface(client_id, &data, width, height);
                    }
                }
            }
        }
    }

    fn update_title(&self) {
        if let Some(window) = &self.window {
            let title = format!(
                "Win-Way | {} client{} | GPU Accelerated",
                self.client_count,
                if self.client_count == 1 { "" } else { "s" }
            );
            window.set_title(&title);
        }
    }

    fn elapsed_time(&self) -> f32 {
        self.start_time.elapsed().as_secs_f32()
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window_attributes = Window::default_attributes()
                .with_title("Win-Way | 0 clients | GPU Accelerated")
                .with_inner_size(winit::dpi::LogicalSize::new(800, 600));
            
            match event_loop.create_window(window_attributes) {
                Ok(window) => {
                    info!("🪟 Window created successfully!");
                    
                    // Initialize the GPU renderer
                    if let Err(e) = self.renderer.init(&window) {
                        error!("❌ Failed to initialize GPU renderer: {}", e);
                        error!("   The application will exit.");
                        event_loop.exit();
                        return;
                    }
                    
                    let size = window.inner_size();
                    self.last_size = (size.width, size.height);
                    self.window = Some(window);
                }
                Err(e) => {
                    error!("Failed to create window: {}", e);
                    event_loop.exit();
                }
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // Process any pending server events
        self.process_server_events();

        match event {
            WindowEvent::CloseRequested => {
                info!("👋 Close requested, exiting...");
                // Force shutdown server?
                if let Some(tx) = &self.server_command_tx {
                    let _ = tx.send(ServerCommand::Shutdown);
                }
                self.renderer.cleanup();
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if size.width > 0 && size.height > 0 {
                    self.last_size = (size.width, size.height);
                    self.renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::Focused(focused) => {
                info!("👁️ Window Focused: {}", focused);
            }
            WindowEvent::RedrawRequested => {
                // info!("🎨 RedrawRequested"); // Too noisy, maybe just once in a while?
                let time = self.elapsed_time();
                self.renderer.render(self.last_size.0, self.last_size.1, time);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                info!("🔴 RAW INPUT: physical_key={:?} state={:?}", event.physical_key, event.state);
                if let PhysicalKey::Code(keycode) = event.physical_key {
                    let state = match event.state {
                        ElementState::Pressed => 1,
                        ElementState::Released => 0,
                    };
                    
                    // Simple manual mapping for testing
                    // TODO: Complete mapping or use crate
                    let linux_code = match keycode {
                        KeyCode::Escape => 1,
                        KeyCode::Digit1 => 2, KeyCode::Digit2 => 3, KeyCode::Digit3 => 4,
                        KeyCode::Backspace => 14,
                        KeyCode::Tab => 15,
                        KeyCode::KeyQ => 16, KeyCode::KeyW => 17, KeyCode::KeyE => 18, KeyCode::KeyR => 19,
                        KeyCode::KeyT => 20, KeyCode::KeyY => 21, KeyCode::KeyU => 22, KeyCode::KeyI => 23,
                        KeyCode::KeyO => 24, KeyCode::KeyP => 25,
                        KeyCode::Enter => 28,
                        KeyCode::ControlLeft => 29,
                        KeyCode::KeyA => 30, KeyCode::KeyS => 31, KeyCode::KeyD => 32, KeyCode::KeyF => 33,
                        KeyCode::KeyG => 34, KeyCode::KeyH => 35, KeyCode::KeyJ => 36, KeyCode::KeyK => 37,
                        KeyCode::KeyL => 38,
                        KeyCode::ShiftLeft => 42,
                        KeyCode::KeyZ => 44, KeyCode::KeyX => 45, KeyCode::KeyC => 46, KeyCode::KeyV => 47,
                        KeyCode::KeyB => 48, KeyCode::KeyN => 49, KeyCode::KeyM => 50,
                        KeyCode::Space => 57,
                        KeyCode::Minus => 12,
                        KeyCode::Equal => 13,
                        KeyCode::BracketLeft => 26,
                        KeyCode::BracketRight => 27,
                        KeyCode::Backslash => 43,
                        KeyCode::Semicolon => 39,
                        KeyCode::Quote => 40,
                        KeyCode::Comma => 51,
                        KeyCode::Period => 52,
                        KeyCode::Slash => 53,
                        KeyCode::AltLeft => 56,
                        KeyCode::ArrowUp => 103,
                        KeyCode::ArrowLeft => 105,
                        KeyCode::ArrowRight => 106,
                        KeyCode::ArrowDown => 108,
                        _ => 0,
                    };

                    if linux_code != 0 {
                         // info!("⌨️ Key Event: code={}, state={}", linux_code, state);
                         if let Some(tx) = &self.server_command_tx {
                             let _ = tx.send(ServerCommand::SendInput(InputEvent::Key { state, code: linux_code }));
                         }
                    } else {
                        // info!("⌨️ Unknown Key: {:?}", event.physical_key);
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(tx) = &self.server_command_tx {
                     let _ = tx.send(ServerCommand::SendInput(InputEvent::Motion { x: position.x, y: position.y }));
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                // Map MouseButton
                let btn = match button {
                    MouseButton::Left => 0x110, // BTN_LEFT
                    MouseButton::Right => 0x111, // BTN_RIGHT
                    MouseButton::Middle => 0x112, // BTN_MIDDLE
                    _ => 0,
                };
                let s = match state {
                    ElementState::Pressed => 1,
                    ElementState::Released => 0,
                };
                if btn != 0 {
                    if let Some(tx) = &self.server_command_tx {
                         let _ = tx.send(ServerCommand::SendInput(InputEvent::Button { state: s, button: btn }));
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Process server events periodically
        self.process_server_events();
        
        // Request a redraw for animation
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Initialize logging
    if args.debug {
        env_logger::Builder::from_env(
            env_logger::Env::default().default_filter_or("debug")
        ).init();
        info!("🚀 STARTING WIN-WAY V2 (YELLOW DEBUG VERSION) 🚀");
    } else {
        env_logger::Builder::from_env(
            env_logger::Env::default().default_filter_or("info")
        ).init();
    }

    println!();
    println!("  ╔═══════════════════════════════════════════════════╗");
    println!("  ║       🪟 Win-Way: Native Waypipe Server           ║");
    println!("  ║       Windows-side receiver for WSL Wayland       ║");
    println!("  ╚═══════════════════════════════════════════════════╝");
    println!();

    info!("🚀 Starting Win-Way (Waypipe Windows Server)");
    info!("📡 TCP listening on 0.0.0.0:{}", args.port);
    info!("💡 Connect from WSL: socat TCP:$(cat /etc/resolv.conf | grep nameserver | awk '{{print $2}}'):{} UNIX-LISTEN:/tmp/wayland-0", args.port);

    // Create channel for server -> main thread communication (Events)
    let (event_tx, event_rx) = std_mpsc::channel();
    
    // Create channel to extract command_tx from the server thread
    let (startup_tx, startup_rx) = std_mpsc::channel();

    // Start tokio runtime in a separate thread for the TCP server
    let port = args.port;
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let config = ServerConfig {
                bind_addr: format!("0.0.0.0:{}", port).parse().unwrap(),
                max_clients: 16,
            };

            match start_server(config).await {
                Ok(mut handle) => {
                    info!("✅ TCP server started");
                    
                    // Send the command channel back to main thread
                    let _ = startup_tx.send(handle.command_tx.clone());

                    // Forward events to the main thread
                    while let Some(event) = handle.events.recv().await {
                        if event_tx.send(event).is_err() {
                            break;
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Failed to start server: {}", e);
                }
            }
        });
    });

    // Wait for server to start and give us the command channel
    info!("⏳ Waiting for server to initialize...");
    let server_command_tx = match startup_rx.recv() {
        Ok(tx) => tx,
        Err(_) => {
            error!("Failed to receive command channel from server thread");
            return Ok(());
        }
    };
    info!("✅ Server control channel received");

    // Start winit event loop
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll); // Poll for continuous animation

    let mut app = App::new(event_rx, server_command_tx);
    event_loop.run_app(&mut app)?;

    Ok(())
}
