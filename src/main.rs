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
};

use win_way::server::{ServerConfig, ServerEvent, start_server};
use win_way::protocol::Decoder;
use win_way::renderer::Renderer;
use win_way::frame::FrameDecoder;

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
    client_count: u32,
    start_time: Instant,
    last_size: (u32, u32),
}

impl App {
    fn new(server_rx: std_mpsc::Receiver<ServerEvent>) -> Self {
        Self {
            window: None,
            renderer: Renderer::new(),
            decoder: Decoder::new(),
            frame_decoder: FrameDecoder::new(),
            server_rx: Some(server_rx),
            client_count: 0,
            start_time: Instant::now(),
            last_size: (800, 600),
        }
    }

    fn process_server_events(&mut self) {
        if let Some(rx) = &self.server_rx {
            // Non-blocking receive of all pending events
            while let Ok(event) = rx.try_recv() {
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
                    ServerEvent::Data { id, data } => {
                        debug!("📦 Received {} bytes from client {}", data.len(), id);
                        
                        // Try to decode as render frames from winpipe
                        self.frame_decoder.push(&data);
                        while let Some(frame) = self.frame_decoder.decode() {
                            info!("🎨 Received frame: {}x{}", frame.width, frame.height);
                            self.renderer.update_surface(
                                id,
                                &frame.data,
                                frame.width,
                                frame.height
                            );
                        }
                        
                        // Also try the protocol decoder for waypipe messages
                        self.decoder.push(&data);
                        while let Some(msg) = self.decoder.decode() {
                            debug!("Decoded message: {:?}", msg.msg_type);
                        }
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
                self.renderer.cleanup();
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if size.width > 0 && size.height > 0 {
                    self.last_size = (size.width, size.height);
                    self.renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                let time = self.elapsed_time();
                self.renderer.render(self.last_size.0, self.last_size.1, time);
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

    // Create channel for server -> main thread communication
    let (tx, rx) = std_mpsc::channel();

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
                    
                    // Forward events to the main thread
                    while let Some(event) = handle.events.recv().await {
                        if tx.send(event).is_err() {
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

    // Start winit event loop
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll); // Poll for continuous animation

    let mut app = App::new(rx);
    event_loop.run_app(&mut app)?;

    Ok(())
}
