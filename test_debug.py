import socket
import struct
import os
import sys

def main():
    path = "/run/user/1000/wayland-winway"
    if not os.path.exists(path):
        print(f"Socket not found at {path}")
        return

    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    try:
        s.connect(path)
        print("Connected!")
    except Exception as e:
        print(f"Connect failed: {e}")
        return

    # 1. get_registry (Display object 1, opcode 1)
    # Header: ID(4) + Op/Size(4). Op=1, Size=8+4=12.
    # Args: new_id(4)
    reg_id = 2
    msg = struct.pack('<II I', 1, (12 << 16) | 1, reg_id)
    s.sendall(msg)

    # 2. sync (Display object 1, opcode 0)
    # Args: new_id(4)
    cb_id = 3
    msg = struct.pack('<II I', 1, (12 << 16) | 0, cb_id)
    s.sendall(msg)
    
    # Read loop
    buf = b""
    output_id = 0
    
    while True:
        try:
            d = s.recv(4096)
            if not d: break
            buf += d
            
            while len(buf) >= 8:
                oid, op_sz = struct.unpack('<II', buf[:8])
                size = op_sz >> 16
                op = op_sz & 0xFFFF
                
                if len(buf) < size: break
                
                payload = buf[8:size]
                buf = buf[size:]
                
                print(f"Event: ID={oid} Op={op} Size={size}")
                
                # Check for registry global event (ID=2, Op=0)
                if oid == reg_id and op == 0:
                    # Args: name(4), interface(str), version(4)
                    name = struct.unpack('<I', payload[:4])[0]
                    # Read string
                    slen = struct.unpack('<I', payload[4:8])[0]
                    sval = payload[8:8+slen-1].decode('utf-8')
                    padded = (slen + 3) & ~3
                    ver = struct.unpack('<I', payload[8+padded:8+padded+4])[0]
                    
                    print(f" Global: {name} {sval} v{ver}")
                    
                    if sval == "wl_output":
                        output_id = name
                        # Bind it!
                        # Registry.bind(name, interface, ver, new_id)
                        # Opcode 0.
                        # Args: name(4), interface(s), version(4), new_id(4)
                        # Let's bind version 3
                        
                        bind_id = 10
                        iface_bytes = b"wl_output\x00\x00\x00" # len 10 -> padded 12
                        ib_len = 10
                         # manual pack for string
                        bind_payload = struct.pack('<I I', name, ib_len) + b"wl_output\x00\x00" + struct.pack('<II', 3, bind_id)
                        
                        b_size = 8 + len(bind_payload)
                        s.sendall(struct.pack('<II', reg_id, (b_size << 16) | 0) + bind_payload)
                        print(f" Bound wl_output {name} to ID {bind_id} (v3)")
                        
                # Check for sync done (ID=3, Op=0)
                if oid == cb_id and op == 0:
                    print("Sync done. Exiting.")
                    return

                if oid == 10:
                    print(f"  wl_output event op={op} size={size}")
                    if op == 0: print("  wl_output.geometry")
                    elif op == 1: print("  wl_output.mode")
                    elif op == 2: print("  wl_output.done")
                    elif op == 3: 
                        scale = struct.unpack('<i', payload[:4])[0]
                        print(f"  wl_output.scale: {scale}")
                    
        except Exception as e:
            print(e)
            break

if __name__ == "__main__":
    main()
