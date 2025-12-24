#!/usr/bin/env python3
import socket
import struct
import time
import math

HOST = 'localhost' # Or Windows IP if needed
PORT = 9999
WIDTH = 400
HEIGHT = 300

def connect():
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    try:
        s.connect((HOST, PORT))
        print(f"✅ Connected to {HOST}:{PORT}")
        return s
    except Exception as e:
        # Try finding Windows IP
        import os
        try:
            with os.popen("ip route | grep default | awk '{print $3}'") as f:
                ip = f.read().strip()
                if ip:
                    print(f"Trying Windows IP: {ip}")
                    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
                    s.connect((ip, PORT))
                    print(f"✅ Connected to {ip}:{PORT}")
                    return s
        except:
            pass
        print(f"❌ Connection failed: {e}")
        return None

def create_frame(t):
    # Create a nice animated pattern
    # format is BGRA (little endian ARGB) or RGBA? Let's try to generate visible colors
    
    pixels = bytearray(WIDTH * HEIGHT * 4)
    
    for y in range(HEIGHT):
        for x in range(WIDTH):
            idx = (y * WIDTH + x) * 4
            
            # Moving stripes
            r = int((math.sin(x * 0.05 + t) + 1) * 127)
            g = int((math.sin(y * 0.05 + t) + 1) * 127)
            b = int((math.sin((x+y) * 0.05 - t) + 1) * 127)
            
            # Draw a white box in middle
            if WIDTH//2 - 50 < x < WIDTH//2 + 50 and HEIGHT//2 - 50 < y < HEIGHT//2 + 50:
                r, g, b = 255, 255, 255
            
            # B G R A (Assuming little endian writing to int) 
            # If win-way treats as bytes:
            pixels[idx] = r     # R? or B?
            pixels[idx+1] = g   # G
            pixels[idx+2] = b   # B? or R?
            pixels[idx+3] = 255 # A
            
    return pixels

def main():
    s = connect()
    if not s:
        # Try to use arguments
        import sys
        if len(sys.argv) > 1:
            global HOST
            HOST = sys.argv[1]
            s = connect()
            
    if not s:
        print("Could not connect. Usage: python3 pixel-test.py [HOST_IP]")
        return

    print("🚀 Sending frames... Press Ctrl+C to stop")
    t = 0.0
    surface_id = 999
    
    try:
        while True:
            data = create_frame(t)
            data_len = len(data)
            
            # Header: PIXL [surface:4] [w:4] [h:4] [fmt:4] [len:4]
            # format 0 = ARGB8888, 1 = XRGB8888
            header = struct.pack('<4sIIIII', b'PIXL', surface_id, WIDTH, HEIGHT, 1, data_len)
            
            s.sendall(header + data)
            print(f"Sent frame {t:.1f}", end='\r')
            
            t += 0.1
            time.sleep(0.033) # 30 FPS
            
    except KeyboardInterrupt:
        print("\nStopped.")
    except Exception as e:
        print(f"\nError: {e}")
    finally:
        s.close()

if __name__ == "__main__":
    main()
