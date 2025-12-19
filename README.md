# Win-way

Windows-native GPU-accelerated display server for Wayland applications from WSL.

## Features

- 🖥️ **GPU Rendering** - OpenGL 3.3+ with VSync
- 🔌 **TCP Server** - Accepts connections from WSL via socat
- 🎨 **Hardware Acceleration** - Uses WGL on Windows
- 📺 **WPRD Protocol** - Custom frame format for efficient transfer

## Architecture

```
WSL Wayland App → socat → TCP:9999 → win-way (GPU render) → Windows Display
```

## Installation

```powershell
git clone https://github.com/J-x-Z/win-way.git
cd win-way
cargo build --release
```

## Usage

### Windows Side
```powershell
cargo run --release
# or
./target/release/win-way.exe
```

### WSL Side
```bash
WIN_IP=$(ip route | grep default | cut -d' ' -f3)
socat UNIX-LISTEN:/tmp/wayland-winway,fork TCP:$WIN_IP:9999 &
export WAYLAND_DISPLAY=/tmp/wayland-winway
your-wayland-app
```

## CLI Options

```
win-way [OPTIONS]
  -p, --port <PORT>    TCP port to listen on (default: 9999)
  -d, --debug          Enable debug logging
```

## Requirements

- Windows 10+ with OpenGL 3.3 support
- Rust 1.70+
- WSL2 with socat installed

## WPRD Frame Format

```
┌──────────┬───────────┬───────────┬──────────┬───────────┬──────────┐
│ Magic    │ Width     │ Height    │ Format   │ Data Size │ Data     │
│ "WPRD"   │ (u32)     │ (u32)     │ (u32)    │ (u32)     │ (bytes)  │
└──────────┴───────────┴───────────┴──────────┴───────────┴──────────┘
```

## License

MIT
