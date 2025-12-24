#!/usr/bin/env python3
"""
WSL Wayland Proxy v5.0 (Stdio Pipe Mode)
Redirects PIXL data to stdout and reads INPT from stdin.
Logs to stderr.
"""

import os
import sys
import socket
import struct
import mmap
import select
import argparse
import time

# Wayland wire protocol constants
HEADER_SIZE = 8

# Standard globals
GLOBALS = [
    (1, "wl_compositor", 4),
    (2, "wl_subcompositor", 1),
    (3, "wl_shm", 1),
    (4, "xdg_wm_base", 1),
    (5, "wl_seat", 5),
    (6, "wl_output", 3),
    (7, "wl_data_device_manager", 3),
]

# SHM formats
SHM_FORMAT_ARGB8888 = 0
SHM_FORMAT_XRGB8888 = 1

class WaylandClient:
    def __init__(self, sock, proxy):
        self.sock = sock
        self.proxy = proxy
        self.objects = {1: ("wl_display", 1)}
        self.surfaces = {} # object_id -> buffer_id
        self.buffers = {}  # buffer_id -> (pool_id, offset, w, h, stride, fmt)
        self.shm_pools = {} # pool_id -> (fd, mm, size)
        self.serial = 0
        self.connected_at = time.time()
        
    def next_serial(self):
        self.serial += 1
        return self.serial

    def close(self):
        # self.proxy.log(f"🧹 Cleaning up client {self.sock.fileno()}")
        # Cleanup SHM maps
        for pid, (fd, mm, size) in self.shm_pools.items():
            if mm: mm.close()
            if fd >= 0: os.close(fd)
        self.sock.close()

    def send(self, data):
        try:
            self.sock.sendall(data)
        except:
            pass
            
    def decode_header(self, data):
        if len(data) < HEADER_SIZE:
            return None, None, None, data
        object_id, opcode_and_size = struct.unpack('<II', data[:HEADER_SIZE])
        size = opcode_and_size >> 16
        opcode = opcode_and_size & 0xFFFF
        return object_id, opcode, size, data[HEADER_SIZE:]
    
    def read_uint(self, data, offset):
        return struct.unpack('<I', data[offset:offset+4])[0], offset + 4
    
    def read_int(self, data, offset):
        return struct.unpack('<i', data[offset:offset+4])[0], offset + 4
    
    def read_string(self, data, offset):
        length, offset = self.read_uint(data, offset)
        s = data[offset:offset+length-1].decode('utf-8').strip('\x00')
        padding = (4 - (length % 4)) % 4
        return s, offset + length + padding

    def encode_message(self, object_id, opcode, *args):
        payload = bytearray()
        for arg in args:
            if isinstance(arg, int):
                payload.extend(struct.pack('<I', arg & 0xFFFFFFFF))
            elif isinstance(arg, str):
                encoded = arg.encode('utf-8') + b'\0'
                padding = (4 - (len(encoded) % 4)) % 4
                payload.extend(struct.pack('<I', len(encoded)))
                payload.extend(encoded)
                payload.extend(b'\0' * padding)
            elif isinstance(arg, bytes):
                 payload.extend(arg)
                 
        size = HEADER_SIZE + len(payload)
        header = struct.pack('<II', object_id, (size << 16) | (opcode & 0xFFFF))
        return header + payload

    def handle_message(self, data, fds):
        offset = 0
        responses = []
        
        while offset < len(data):
            if len(data) - offset < HEADER_SIZE:
                break
                
            object_id, opcode, size, _ = self.decode_header(data[offset:])
            
            if size < 8 or len(data) - offset < size:
                break
                
            self.proxy.log(f"📥 ID={object_id} Op={opcode} Sz={size}") # LOG
            
            payload = data[offset + HEADER_SIZE : offset + size]
            offset += size
            
            obj_info = self.objects.get(object_id)
            if not obj_info:
                self.proxy.log(f"⚠️ Unknown object {object_id}")
                continue
                
            obj_type, obj_version = obj_info
            
            # Dispatch
            if obj_type == "wl_display":
                responses.extend(self.handle_display(opcode, payload))
            elif obj_type == "wl_registry":
                responses.extend(self.handle_registry(object_id, opcode, payload))
            elif obj_type == "wl_compositor":
                responses.extend(self.handle_compositor(object_id, opcode, payload))
            elif obj_type == "wl_shm":
                responses.extend(self.handle_shm(object_id, opcode, payload, fds))
            elif obj_type == "wl_shm_pool":
                responses.extend(self.handle_shm_pool(object_id, opcode, payload))
            elif obj_type == "wl_buffer":
                responses.extend(self.handle_buffer(object_id, opcode, payload))
            elif obj_type == "wl_surface":
                responses.extend(self.handle_surface(object_id, opcode, payload))
            elif obj_type == "xdg_wm_base":
                responses.extend(self.handle_xdg_wm_base(object_id, opcode, payload))
            elif obj_type == "xdg_surface":
                responses.extend(self.handle_xdg_surface(object_id, opcode, payload))
            elif obj_type == "xdg_toplevel":
                responses.extend(self.handle_xdg_toplevel(object_id, opcode, payload))
            elif obj_type == "wl_seat":
                responses.extend(self.handle_seat(object_id, opcode, payload))
            elif obj_type == "wl_data_device_manager":
                 responses.extend(self.handle_data_device_manager(object_id, opcode, payload))
            elif obj_type == "wl_region":
                 responses.extend(self.handle_region(object_id, opcode, payload))
            elif obj_type == "wl_subcompositor":
                 responses.extend(self.handle_subcompositor(object_id, opcode, payload))
            elif obj_type == "wl_callback":
                 pass
            else:
                self.proxy.log(f"📝 Unhandled {obj_type} {object_id} op {opcode}")

        for resp in responses:
            self.send(resp)
            
        return True

    def try_focus(self):
        sid = next(iter(self.surfaces), None)
        if not sid: return
        for obj_id, (iface, ver) in self.objects.items():
            if iface == "wl_keyboard":
                ser = self.next_serial()
                self.send(self.encode_message(obj_id, 4, ser, sid, b''))
                self.proxy.log(f"📤 Auto-Focus Keyboard {obj_id}")
            elif iface == "wl_pointer":
                 self.send(self.encode_message(obj_id, 4, self.next_serial(), sid, 0, 0))
                 self.proxy.log(f"📤 Auto-Focus Pointer {obj_id}")

    def handle_display(self, opcode, payload):
        res = []
        pos = 0
        if opcode == 0: # sync
            cb_id, pos = self.read_uint(payload, pos)
            self.objects[cb_id] = ("wl_callback", 1)
            res.append(self.encode_message(cb_id, 0, int(time.time()*1000)&0xFFFFFFFF))
            self.proxy.log(f"📤 sync -> {cb_id}")
            del self.objects[cb_id]
        elif opcode == 1: # get_registry
            reg_id, pos = self.read_uint(payload, pos)
            self.objects[reg_id] = ("wl_registry", 1)
            for name, interface, version in GLOBALS:
                res.append(self.encode_message(reg_id, 0, name, interface, version))
            self.proxy.log(f"📤 get_registry -> {reg_id}")
        return res

    def handle_registry(self, oid, op, pay):
        res = []
        pos = 0
        if op == 0: # bind
            name, pos = self.read_uint(pay, pos)
            iface, pos = self.read_string(pay, pos)
            ver, pos = self.read_uint(pay, pos)
            nid, pos = self.read_uint(pay, pos)
            self.proxy.log(f"📝 BIND request: name={name} iface='{iface}' v={ver} nid={nid}")
            self.objects[nid] = (iface, ver)
            
            if iface == "wl_shm":
                res.append(self.encode_message(nid, 0, SHM_FORMAT_ARGB8888))
                res.append(self.encode_message(nid, 0, SHM_FORMAT_XRGB8888))
            elif iface == "wl_seat":
                res.append(self.encode_message(nid, 0, 3)) # caps
                res.append(self.encode_message(nid, 1, "win-way-seat"))
            elif iface == "wl_output":
                # wl_output events:
                # 0: geometry (x, y, w, h, subpixel, make, model, transform)
                # 1: mode (flags, w, h, refresh)
                # 2: done (v2)
                # 3: scale (v2)
                
                # Send geometry
                res.append(self.encode_message(nid, 0, 0, 0, 1920, 1080, 0, "WinWay", "Monitor", 0))
                # Send mode (current | preferred = 0x3)
                res.append(self.encode_message(nid, 1, 3, 1920, 1080, 60000))
                
                if ver >= 2:
                    # Send scale (factor=1) -> Opcode 3
                    res.append(self.encode_message(nid, 3, 1))
                    # Send done -> Opcode 2
                    res.append(self.encode_message(nid, 2))
        return res

    def handle_compositor(self, oid, op, pay):
        pos = 0
        if op == 0: # create_surface
            sid, pos = self.read_uint(pay, pos)
            self.objects[sid] = ("wl_surface", 4)
            self.proxy.log(f"📤 create_surface -> {sid}")
            self.try_focus()
        elif op == 1: # create_region
            rid, pos = self.read_uint(pay, pos)
            self.objects[rid] = ("wl_region", 1)
        return []

    def handle_subcompositor(self, oid, op, pay):
        pos = 0
        if op == 0: # destroy
            if oid in self.objects: del self.objects[oid]
        elif op == 1: # get_subsurface
            sub_id, pos = self.read_uint(pay, pos)
            surf_id, pos = self.read_uint(pay, pos)
            parent_id, pos = self.read_uint(pay, pos)
            self.objects[sub_id] = ("wl_subsurface", 1)
        return []

    def handle_region(self, oid, op, pay):
        if op == 0: # destroy
            if oid in self.objects: del self.objects[oid]
        return []

    def handle_data_device_manager(self, oid, op, pay):
        pos = 0
        if op == 0: # get_data_device
            id, pos = self.read_uint(pay, pos)
            seat, pos = self.read_uint(pay, pos)
            self.objects[id] = ("wl_data_device", 3)
        elif op == 1: # create_data_source
            id, pos = self.read_uint(pay, pos)
            self.objects[id] = ("wl_data_source", 3)
        return []

    def handle_shm(self, oid, op, pay, fds):
        res = []
        pos = 0
        if op == 0: # create_pool
            pid, pos = self.read_uint(pay, pos)
            size, pos = self.read_uint(pay, pos)
            if fds:
                fd = fds.pop(0)
                try:
                    mm = mmap.mmap(fd, size, mmap.MAP_SHARED, mmap.PROT_READ)
                    self.shm_pools[pid] = (fd, mm, size)
                    self.proxy.log(f"📤 create_pool -> {pid} ({size})")
                except Exception as e:
                    self.proxy.log(f"⚠️ mmap fail: {e}")
                    self.shm_pools[pid] = (fd, None, size)
            self.objects[pid] = ("wl_shm_pool", 1)
        return res

    def handle_shm_pool(self, oid, op, pay):
        pos = 0
        if op == 0: # create_buffer
            bid, pos = self.read_uint(pay, pos)
            off, pos = self.read_int(pay, pos)
            w, pos = self.read_int(pay, pos)
            h, pos = self.read_int(pay, pos)
            stride, pos = self.read_int(pay, pos)
            fmt, pos = self.read_uint(pay, pos)
            self.objects[bid] = ("wl_buffer", 1)
            self.buffers[bid] = (oid, off, w, h, stride, fmt)
            self.proxy.log(f"📤 create_buffer -> {bid}")
        elif op == 1: # destroy
            if oid in self.shm_pools:
                fd, mm, sz = self.shm_pools.pop(oid)
                if mm: mm.close()
                if fd >= 0: os.close(fd)
        return []

    def handle_buffer(self, oid, op, pay):
        if op == 0: # destroy
            if oid in self.objects: del self.objects[oid]
            if oid in self.buffers: del self.buffers[oid]
        return []

    def handle_surface(self, oid, op, pay):
        res = []
        pos = 0
        try:
            if op == 0: # destroy
                if oid in self.objects: del self.objects[oid]
                if oid in self.surfaces: del self.surfaces[oid]
            elif op == 1: # attach
                if len(pay) < 12:
                    if len(pay) >= 4:
                        bid, pos = self.read_uint(pay, 0)
                        self.surfaces[oid] = bid if bid != 0 else None
                    return []
                    
                bid, pos = self.read_uint(pay, pos)
                x, pos = self.read_int(pay, pos)
                y, pos = self.read_int(pay, pos)
                self.surfaces[oid] = bid if bid != 0 else None
            elif op == 3: # frame
                if len(pay) < 4: return []
                cb_id, pos = self.read_uint(pay, pos)
                self.objects[cb_id] = ("wl_callback", 1)
                res.append(self.encode_message(cb_id, 0, int(time.time()*1000)&0xFFFFFFFF))
                del self.objects[cb_id]
            elif op == 6: # commit
                bid = self.surfaces.get(oid)
                if bid and bid in self.buffers:
                    self.proxy.send_pixl(oid, self.buffers[bid], self.shm_pools)
                    res.append(self.encode_message(bid, 0)) # release
        except Exception as e:
            self.proxy.log(f"⚠️ handle_surface error: {e}")
        return res

    def handle_xdg_wm_base(self, oid, op, pay):
        res = []
        pos = 0
        if op == 2: # get_xdg_surface
            xdg_id, pos = self.read_uint(pay, pos)
            sid, pos = self.read_uint(pay, pos)
            self.objects[xdg_id] = ("xdg_surface", 3)
            self.proxy.log(f"📤 get_xdg_surface -> {xdg_id}")
        return res

    def handle_xdg_surface(self, oid, op, pay):
        res = []
        pos = 0
        if op == 1: # get_toplevel
            tid, pos = self.read_uint(pay, pos)
            self.objects[tid] = ("xdg_toplevel", 3)
            res.append(self.encode_message(tid, 0, 800, 600, b''))
            res.append(self.encode_message(oid, 0, self.next_serial()))
            self.proxy.log(f"📤 get_toplevel -> {tid}")
            self.try_focus()
        return res

    def handle_xdg_toplevel(self, oid, op, pay):
        return []
        
    def handle_seat(self, oid, op, pay):
        pos = 0
        if op == 0: # get_pointer
            nid, pos = self.read_uint(pay, pos)
            self.objects[nid] = ("wl_pointer", 1)
            self.proxy.log(f"📤 get_pointer -> {nid}")
            self.try_focus()

        elif op == 1: # get_keyboard
            nid, pos = self.read_uint(pay, pos)
            self.objects[nid] = ("wl_keyboard", 1)
            self.proxy.log(f"📤 get_keyboard -> {nid}")
            self.try_focus()
        return []

class WaylandProxy:
    def __init__(self, port=9999):
        self.port = port
        self.tcp_socket = None
        self.clients = {} # sock -> WaylandClient
        self.mode = 'tcp'

    def log(self, msg):
        sys.stderr.write(f"{msg}\n")
        sys.stderr.flush()

    def send_pixl(self, sid, buf, pools):
        # In Stdio mode, write to stdout
        pid, off, w, h, stride, fmt = buf
        if pid not in pools: return
        fd, mm, sz = pools[pid]
        if not mm: return
        
        try:
            row = w*4
            if off + stride * h > sz: return
            
            # Calculate size
            total_len = 0
            for y in range(h):
                off_y = off + y*stride
                if off_y + row > sz: break
                total_len += row
                
            hdr = struct.pack('<4sIIIII', b'PIXL', sid, w, h, fmt, total_len)
            
            if self.mode == 'stdio':
                sys.stdout.buffer.write(hdr)
                for y in range(h):
                    off_y = off + y*stride
                    if off_y + row > sz: break
                    mm.seek(off_y)
                    sys.stdout.buffer.write(mm.read(row))
                sys.stdout.buffer.flush()
            elif self.tcp_socket:
                self.tcp_socket.sendall(hdr)
                for y in range(h):
                    off_y = off + y*stride
                    if off_y + row > sz: break
                    mm.seek(off_y)
                    self.tcp_socket.sendall(mm.read(row))
                    
            # self.log(f"🖼️ Sent PIXL {w}x{h}")
        except Exception as e:
            self.log(f"⚠️ Send PIXL fail: {e}")

    def broadcast_input(self, data):
        if len(data) < 20: return
        if data[0:4] != b'INPT': return
        
        try:
            type_code = struct.unpack('<I', data[4:8])[0]
            p1 = struct.unpack('<I', data[8:12])[0]
            p2 = struct.unpack('<I', data[12:16])[0]
            
            self.log(f"📥 INPT {type_code}")
            
            for cli in self.clients.values():
                ser = cli.next_serial()
                now = int(time.time()*1000) & 0xFFFFFFFF
                
                keys = [k for k,v in cli.objects.items() if v[0] == "wl_keyboard"]
                ptrs = [k for k,v in cli.objects.items() if v[0] == "wl_pointer"]
                
                msgs = []
                if type_code == 1: # Key
                    for k in keys:
                        msgs.append(cli.encode_message(k, 3, ser, now, p2, p1))
                elif type_code == 2: # Motion
                    fx, fy = int(p1)*256, int(p2)*256
                    for p in ptrs:
                        msgs.append(cli.encode_message(p, 2, now, fx, fy))
                elif type_code == 3: # Button
                    for p in ptrs:
                        msgs.append(cli.encode_message(p, 3, ser, now, p2, p1))
                        
                for m in msgs:
                    cli.send(m)
        except:
            pass

    def run_stdio(self, socket_path):
        self.mode = 'stdio'
        self.log(f"🌟 WSL Proxy Stdio Mode v5.0 Loaded! 🌟")
        if os.path.exists(socket_path): os.remove(socket_path)
        
        srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        srv.bind(socket_path)
        srv.listen(5)
        self.log(f"🚀 Listening on {socket_path}")
        
        # Stdin is blocking, so use select
        stdin_fd = sys.stdin.fileno()
        try:
             os.set_blocking(stdin_fd, False)
        except: pass
        
        buf = bytearray()
        
        while True:
            rlist = [srv, stdin_fd] + list(self.clients.keys())
            
            ready, _, _ = select.select(rlist, [], [], 0.01)
            
            for s in ready:
                if s == srv:
                    cli, addr = srv.accept()
                    wc = WaylandClient(cli, self)
                    self.clients[cli] = wc
                    self.log("📥 Client +")
                elif s == stdin_fd:
                    # Read from Stdin (Data from Windows)
                    try:
                         # Using os.read for unbuffered raw read
                        d = os.read(stdin_fd, 65536)
                        if not d:
                            self.log("⚠️ Stdin EOF (Windows closed)")
                            return
                        buf.extend(d)
                        while len(buf) >= 8:
                            # Check INPT (Fixed 20 bytes)
                            if len(buf) >= 20 and buf[0:4] == b'INPT':
                                self.broadcast_input(buf[0:20])
                                del buf[0:20]
                                continue
                                
                            # Check Wayland Message
                            # Decode header (8 bytes)
                            oid, op_sz = struct.unpack('<II', buf[0:8])
                            size = op_sz >> 16
                            
                            if size < 8:
                                # Invalid size, maybe garbage? Discard 1 byte
                                del buf[0]
                                continue
                                
                            if len(buf) >= size:
                                packet = buf[0:size]
                                # Broadcast to all clients (Simple muxing)
                                for cli in self.clients.values():
                                    cli.send(packet)
                                del buf[0:size]
                            else:
                                # Wait for more data
                                break
                    except BlockingIOError: pass
                    except Exception as e:
                         self.log(f"❌ Stdin Read Err: {e}")

                elif s in self.clients:
                    wc = self.clients[s]
                    try:
                        d, anc, f, a = s.recvmsg(65536, socket.CMSG_LEN(1024))
                        if d:
                            fds = []
                            for l, t, cd in anc:
                                if l == socket.SOL_SOCKET and t == socket.SCM_RIGHTS:
                                    n = len(cd)//4
                                    fds.extend(struct.unpack(f'{n}i', cd[:n*4]))
                            wc.handle_message(d, fds)
                        else:
                            raise Exception("EOF")
                    except Exception as e:
                        wc.close()
                        del self.clients[s]
                        self.log("📤 Client -")

    def run(self, socket_path):
        # Legacy
        print("This mode is deprecated")

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--stdio", action="store_true", help="Use Stdio transport")
    args = parser.parse_args()

    wp = WaylandProxy()
    
    # Ensure directory exists
    p_dir = f"/run/user/{os.getuid()}"
    if not os.path.exists(p_dir):
        p = f"/tmp/wayland-winway"
    else:
        p = f"{p_dir}/wayland-winway"
        
    if args.stdio:
        wp.run_stdio(p)
    else:
        wp.run_stdio(p) # Default to stdio for now
