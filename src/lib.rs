//! Win-Way: Native Wayland Compositor for Windows
//! 
//! This crate provides a bridge between WSL Wayland applications and
//! native Windows rendering via OpenGL/WGL.

pub mod server;
pub mod protocol;
pub mod renderer;
pub mod frame;
pub mod error;
pub mod wayland;
