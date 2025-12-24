//! Wayland compositor protocol implementation
//!
//! This module implements the Wayland protocol over TCP for Windows.

pub mod wire;
pub mod object;
pub mod display;
pub mod registry;
pub mod compositor;
pub mod shm;
pub mod xdg_shell;
pub mod client;

pub use client::WaylandClient;
pub use display::WlDisplay;
