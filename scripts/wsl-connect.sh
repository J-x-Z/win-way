#!/bin/bash
# WSL to Win-Way Connection Script
# Run this in WSL to bridge Wayland apps to win-way on Windows

set -e

# Get Windows host IP (using grep + cut instead of awk)
WIN_IP=$(ip route | grep default | cut -d' ' -f3)
echo "🔗 Windows IP: $WIN_IP"

# Port that win-way listens on
WIN_PORT=${1:-9999}
echo "📡 Connecting to win-way at $WIN_IP:$WIN_PORT"

# Check if socat is installed
if ! command -v socat &> /dev/null; then
    echo "❌ socat not installed. Run: sudo apt install socat"
    exit 1
fi

# Create Wayland socket path
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/tmp}"
WAYLAND_SOCKET="$XDG_RUNTIME_DIR/wayland-winway"

# Clean up old socket
rm -f "$WAYLAND_SOCKET" "$WAYLAND_SOCKET.lock"

# Start socat bridge in background
echo "🌉 Starting socat bridge..."
socat UNIX-LISTEN:"$WAYLAND_SOCKET",fork TCP:"$WIN_IP":"$WIN_PORT" &
SOCAT_PID=$!
echo "   PID: $SOCAT_PID"

# Wait for socket
sleep 1

if [ ! -S "$WAYLAND_SOCKET" ]; then
    echo "❌ Failed to create socket"
    kill $SOCAT_PID 2>/dev/null
    exit 1
fi

echo "✅ Wayland socket ready: $WAYLAND_SOCKET"
echo ""
echo "🚀 To run Wayland apps in another terminal:"
echo "   export WAYLAND_DISPLAY=$WAYLAND_SOCKET"
echo "   niri --session"
echo ""
echo "Press Ctrl+C to stop the bridge"

# Cleanup on exit
cleanup() {
    echo ""
    echo "🧹 Cleaning up..."
    kill $SOCAT_PID 2>/dev/null || true
    rm -f "$WAYLAND_SOCKET" "$WAYLAND_SOCKET.lock"
}
trap cleanup EXIT

# Wait for socat
wait $SOCAT_PID
