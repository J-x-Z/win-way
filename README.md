# Win-way

Windows-native display server for Wayland applications (Experimental)

## Current Status

⚠️ **This is an experimental project with very limited functionality:**

- ✅ Can create a GPU-accelerated Windows window
- ✅ Can accept TCP connections
- ✅ Can display frames sent via WPRD protocol
- ❌ **Cannot display WSL Wayland apps** (Unix file descriptors cannot be passed over TCP)

## Installation

```powershell
git clone https://github.com/J-x-Z/win-way.git
cd win-way
cargo build --release
```

## Usage

```powershell
cargo run --release
```

Starts a window and listens on TCP port 9999.

## CLI Options

```
win-way [OPTIONS]
  -p, --port <PORT>    Listen port (default: 9999)
  -d, --debug          Enable debug logging
```

## Requirements

- Windows 10+
- GPU with OpenGL 3.3 support
- Rust 1.70+

## License

GPL-3.0
