# 🪟 win-way: Native Smithay on Windows (WGL)

> **Part of the Universal Wayland Research Project**

This project demonstrates running a Smithay-based compositor natively on Windows using the WGL backend, without any Linux virtualization or WSL.

## Architecture
`win-way` uses the `smithay-universal` fork which implements:
- **Glutin WGL**: Native OpenGL context creation on Windows.
- **Winit Loop**: Native Windows message pump integration.

## Usage
### Option A: Cloud Build (Recommended)
1. Go to the [Actions](../../actions) tab.
2. Click on the latest workflow run.
3. Scroll down to **Artifacts** and download `win-way-exe`.
4. Run `win-way.exe` on your Windows machine.

### Option B: Local Build
1. Install Rust on Windows (`rustup-init.exe` -> default-msvc).
2. Clone this repo:
   ```powershell
   git clone https://github.com/J-x-Z/win-way.git
   cd win-way
   ```
3. Run:
   ```powershell
   cargo run
   ```
