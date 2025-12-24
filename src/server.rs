//! Stdio Server for Wayland connections from WSL (Waypipe style)
//!
//! This module spawns the WSL proxy process and communicates via Stdio pipes.

use std::net::SocketAddr;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command; // Async process
use std::process::Stdio;
use tokio::sync::{mpsc, Mutex, broadcast};
use log::{info, warn, error, debug};

use crate::wayland::WaylandClient;
use crate::wayland::client::RenderEvent;
use crate::wayland::wire::Message;

/// Input events to forward to WSL
#[derive(Debug, Clone)]
pub enum InputEvent {
    Key { state: u32, code: u32 },
    Motion { x: f64, y: f64 },
    Button { state: u32, button: u32 },
}

/// Commands sent from main thread to server
#[derive(Debug, Clone)]
pub enum ServerCommand {
    SendInput(InputEvent),
    Shutdown,
}

/// Events sent from the server to the main compositor
#[derive(Debug)]
pub enum ServerEvent {
    /// New client connected (Simulated for single Stdio stream)
    ClientConnected { id: u32 },
    /// Client disconnected
    ClientDisconnected { id: u32 },
    /// Received data from client (raw bytes)
    Data { id: u32, data: Vec<u8> },
    /// Render event from Wayland protocol
    Render(u32, RenderEvent),
    /// Pixel data from WSL proxy (PIXL protocol)
    PixelData {
        client_id: u32,
        surface_id: u32,
        width: u32,
        height: u32,
        format: u32,
        data: Vec<u8>,
    },
}

/// Handle to control the server
pub struct ServerHandle {
    /// Channel to receive events
    pub events: mpsc::Receiver<ServerEvent>,
    /// Channel to send commands
    pub command_tx: broadcast::Sender<ServerCommand>,
}

/// Server configuration (kept for API compatibility, mostly unused in Stdio)
pub struct ServerConfig {
    pub bind_addr: SocketAddr,
    pub max_clients: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:9999".parse().unwrap(),
            max_clients: 16,
        }
    }
}

/// Start the Stdio bridge to WSL proxy
pub async fn start_server(_config: ServerConfig) -> std::io::Result<ServerHandle> {
    let (event_tx, event_rx) = mpsc::channel(256);
    let (command_tx, _) = broadcast::channel(16);

    let command_tx_clone = command_tx.clone();
    
    // Spawn the Stdio loop
    tokio::spawn(async move {
        loop {
            info!("🚀 Spawning WSL Proxy via Stdio...");
            
            // Command: wsl -d FedoraLinux-43 bash -c "python3 ~/wsl-proxy.py --stdio"
            // Use --exec or bash -c to ensure environment? bash -c is safer for ~ expansion.
            let mut child = Command::new("wsl")
                .args(["-d", "FedoraLinux-43", "bash", "-c", "python3 ~/wsl-proxy.py --stdio"])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit()) // Let stderr flow to our console for logs
                .kill_on_drop(true)
                .spawn();

            match child {
                Ok(mut child) => {
                    info!("✅ WSL Proxy Process Started!");
                    
                    let stdin = child.stdin.take().expect("Failed to open stdin");
                    let stdout = child.stdout.take().expect("Failed to open stdout");
                    
                    let event_tx = event_tx.clone();
                    let command_rx = command_tx_clone.subscribe();
                    
                    // Client ID 1 for the single Stdio connection
                    let id = 1;
                    
                    let _ = event_tx.send(ServerEvent::ClientConnected { id }).await;
                    
                    // Run the IO loop
                    // This blocks until connection dies
                    handle_stdio_io(stdin, stdout, id, event_tx.clone(), command_rx).await;
                    
                    let _ = event_tx.send(ServerEvent::ClientDisconnected { id }).await;
                    
                    info!("⚠️ WSL Proxy Process exited, restarting in 3s...");
                    let _ = child.kill().await;
                }
                Err(e) => {
                    error!("❌ Failed to spawn WSL proxy: {}. Retrying in 3s...", e);
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
        }
    });

    Ok(ServerHandle {
        events: event_rx,
        command_tx,
    })
}

/// Handle Stdio IO logic
async fn handle_stdio_io(
    mut stdin: tokio::process::ChildStdin,
    mut stdout: tokio::process::ChildStdout,
    client_id: u32,
    event_tx: mpsc::Sender<ServerEvent>,
    mut command_rx: broadcast::Receiver<ServerCommand>,
) {
    let mut wayland = WaylandClient::new(client_id);
    let mut read_buf = [0u8; 65536];
    
    loop {
        tokio::select! {
            // Read from Child Stdout (Data from WSL)
            read_result = stdout.read(&mut read_buf) => {
                match read_result {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        let data = &read_buf[0..n];
                        // Process data (Wayland + PIXL)
                        wayland.process(data);
                        
                        // Handle outgoing Wayland messages (Windows -> WSL)
                        for msg in wayland.take_outgoing() {
                            let bytes = msg.encode();
                             if let Err(e) = stdin.write_all(&bytes).await {
                                 error!("❌ Stdin write error: {}", e);
                                 return; // Break loop
                             }
                        }
                        if let Err(e) = stdin.flush().await {
                             error!("❌ Stdin flush error: {}", e);
                             break;
                        }
                        
                        // Handle render events
                        for event in wayland.take_render_events() {
                            match event {
                                RenderEvent::PixelData { surface_id, width, height, format, data } => {
                                    let _ = event_tx.send(ServerEvent::PixelData {
                                        client_id,
                                        surface_id,
                                        width,
                                        height,
                                        format,
                                        data,
                                    }).await;
                                }
                                other => {
                                    let _ = event_tx.send(ServerEvent::Render(client_id, other)).await;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("❌ Stdout read error: {}", e);
                        break;
                    }
                }
            }
            
            // Write to Child Stdin (Input to WSL)
            Ok(cmd) = command_rx.recv() => {
                match cmd {
                    ServerCommand::Shutdown => break,
                    ServerCommand::SendInput(event) => {
                        let mut packet = Vec::with_capacity(20);
                        packet.extend_from_slice(b"INPT");
                        match event {
                            InputEvent::Key { state, code } => {
                                packet.extend_from_slice(&1u32.to_le_bytes()); 
                                packet.extend_from_slice(&state.to_le_bytes());
                                packet.extend_from_slice(&code.to_le_bytes());
                                packet.extend_from_slice(&0u32.to_le_bytes()); // padding
                            }
                            InputEvent::Motion { x, y } => {
                                packet.extend_from_slice(&2u32.to_le_bytes());
                                packet.extend_from_slice(&(x as u32).to_le_bytes()); 
                                packet.extend_from_slice(&(y as u32).to_le_bytes());
                                packet.extend_from_slice(&0u32.to_le_bytes());
                            }
                            InputEvent::Button { state, button } => {
                                packet.extend_from_slice(&3u32.to_le_bytes());
                                packet.extend_from_slice(&state.to_le_bytes());
                                packet.extend_from_slice(&button.to_le_bytes());
                                packet.extend_from_slice(&0u32.to_le_bytes());
                            }
                        }
                        
                        if let Err(e) = stdin.write_all(&packet).await {
                             error!("❌ Stdin write error: {}", e);
                             break;
                        }
                        let _ = stdin.flush().await; 
                    }
                }
            }
        }
    }
}
