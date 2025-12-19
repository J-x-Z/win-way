//! TCP Server for Waypipe connections from WSL
//!
//! This module handles incoming TCP connections from WSL and forwards
//! Waypipe protocol messages to the compositor.

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use log::{info, warn, error, debug};

use crate::error::{Result, WinWayError};
use crate::protocol::Message;

/// Events sent from the server to the main compositor
#[derive(Debug)]
pub enum ServerEvent {
    /// New client connected
    ClientConnected { id: u32 },
    /// Client disconnected
    ClientDisconnected { id: u32 },
    /// Received data from client
    Data { id: u32, data: Vec<u8> },
}

/// Handle to control the server
pub struct ServerHandle {
    /// Channel to receive events
    pub events: mpsc::Receiver<ServerEvent>,
    /// Channel to send data back to clients
    pub sender: mpsc::Sender<(u32, Vec<u8>)>,
}

/// TCP Server configuration
pub struct ServerConfig {
    pub bind_addr: SocketAddr,
    pub max_clients: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:9999".parse().unwrap(),
            max_clients: 16,
        }
    }
}

/// Start the TCP server in a background task
pub async fn start_server(config: ServerConfig) -> Result<ServerHandle> {
    let listener = TcpListener::bind(config.bind_addr).await?;
    info!("🚀 Win-Way server listening on {}", config.bind_addr);

    let (event_tx, event_rx) = mpsc::channel(256);
    let (send_tx, mut send_rx) = mpsc::channel::<(u32, Vec<u8>)>(256);

    // Spawn the accept loop
    tokio::spawn(async move {
        let mut client_id: u32 = 0;
        
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    client_id = client_id.wrapping_add(1);
                    info!("📥 Client {} connected from {}", client_id, addr);
                    
                    let event_tx = event_tx.clone();
                    let id = client_id;
                    
                    // Notify compositor of new connection
                    let _ = event_tx.send(ServerEvent::ClientConnected { id }).await;
                    
                    // Spawn handler for this client
                    tokio::spawn(handle_client(stream, id, event_tx));
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    });

    Ok(ServerHandle {
        events: event_rx,
        sender: send_tx,
    })
}

async fn handle_client(
    mut stream: TcpStream,
    client_id: u32,
    event_tx: mpsc::Sender<ServerEvent>,
) {
    let mut buffer = vec![0u8; 65536]; // 64KB buffer
    
    loop {
        match stream.read(&mut buffer).await {
            Ok(0) => {
                // Connection closed
                info!("📤 Client {} disconnected", client_id);
                let _ = event_tx.send(ServerEvent::ClientDisconnected { id: client_id }).await;
                break;
            }
            Ok(n) => {
                debug!("Received {} bytes from client {}", n, client_id);
                let data = buffer[..n].to_vec();
                if event_tx.send(ServerEvent::Data { id: client_id, data }).await.is_err() {
                    break;
                }
            }
            Err(e) => {
                warn!("Error reading from client {}: {}", client_id, e);
                let _ = event_tx.send(ServerEvent::ClientDisconnected { id: client_id }).await;
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_starts() {
        let config = ServerConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(), // Random port
            max_clients: 4,
        };
        let handle = start_server(config).await;
        assert!(handle.is_ok());
    }
}
