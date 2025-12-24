# Win-way

Windows-native display server for Wayland applications (Experimental)

## Current Status

⚠️ **This is an experimental project demonstrating architectural feasibility:**

- ✅ Can create a GPU-accelerated Windows window
- ✅ Can accept TCP connections  
- ✅ Can display frames sent via WPRD protocol
- ⚠️ Simple clients render correctly
- ❌ Complex apps (browsers) crash during buffer allocation

## 📚 Research

Part of the **"Turbo-Charged Protocol Virtualization"** research. See [../paper/](../paper/) for manuscript and benchmarks.

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
