#!/usr/bin/env python3
"""
VRChat OSC/OSCQuery drop-in test server.

Reads haptic config JSONs, advertises all parameters + prefab metadata via
OSCQuery (HTTP), and continuously sends triangle-wave ramps (0→1→0 over 1s)
at 10 Hz per parameter via OSC UDP, with updates spread evenly across each cycle.

python scripts/vrc.py ./src-tauri/map_configs/Average_nardo-generic-face_1.json ./src-tauri/map_configs/Average_Ear-Pull-Levi_1.json ./src-tauri/map_configs/Average_nardo-generic-vest_1.json ./src-tauri/map_configs/Average_Tail-Pull_1.json

Dependencies: python-osc
    pip install python-osc
"""

import argparse
import asyncio
import json
import threading
from http.server import HTTPServer, BaseHTTPRequestHandler
from pathlib import Path
from urllib.parse import urlparse
from pythonosc.udp_client import SimpleUDPClient
from zeroconf import Zeroconf, ServiceInfo
import socket


# ─── Config loading ───────────────────────────────────────────────────────────

def load_configs(paths: list[str]) -> list[tuple[dict, list[dict]]]:
    configs = []
    for p in paths:
        data = json.loads(Path(p).read_text())
        configs.append((data["meta"], data["nodes"]))
    return configs


# ─── OSCQuery tree ────────────────────────────────────────────────────────────

def build_tree(configs):
    """Return (tree_root, list_of_float_addresses)."""
    leaves: dict[str, dict] = {}

    # Fake avatar change parameter
    avatar_id = "avtr_" + "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
    leaves["/avatar/change"] = {"TYPE": "s", "VALUE": [avatar_id], "ACCESS": 1}

    for meta, nodes in configs:
        prefab = (
            f"/avatar/parameters/haptic/prefabs"
            f"/{meta['map_author']}/{meta['map_name']}/v{meta['map_version']}"
        )
        leaves[prefab] = {"TYPE": "i", "VALUE": [meta["map_version"]], "ACCESS": 1}

        for node in nodes:
            leaves[node["address"]] = {"TYPE": "f", "VALUE": [0.0], "ACCESS": 1}

    root = {"FULL_PATH": "/", "CONTENTS": {}}
    for full_path, leaf_data in leaves.items():
        parts = full_path.strip("/").split("/")
        cur = root
        for i, part in enumerate(parts):
            cur.setdefault("CONTENTS", {})
            if part not in cur["CONTENTS"]:
                cur["CONTENTS"][part] = {
                    "FULL_PATH": "/" + "/".join(parts[: i + 1])
                }
            cur = cur["CONTENTS"][part]
        cur.update(leaf_data)

    float_addrs = [a for a, v in leaves.items() if v["TYPE"] == "f"]
    return root, float_addrs, avatar_id


async def ramp_loop(addrs: list[str], avatar_id: str, target_ip: str, target_port: int):
    client = SimpleUDPClient(target_ip, target_port)

    # Send avatar change once at startup
    client.send_message("/avatar/change", avatar_id)
    print(f"[osc] sent /avatar/change → {avatar_id}")

    n = len(addrs)
    if n == 0:
        print("[osc] no float parameters found")
        return

    interval = 0.1
    stagger = interval / n
    print(f"[osc] modulating {n} params → {target_ip}:{target_port}  "
          f"(stagger {stagger * 1000:.2f} ms)")

    t0 = asyncio.get_event_loop().time()
    while True:
        cycle_start = asyncio.get_event_loop().time()
        for i, addr in enumerate(addrs):
            now = asyncio.get_event_loop().time()
            t = (now - t0) % 1.0
            value = 1.0 - abs(2.0 * t - 1.0)
            client.send_message(addr, float(value))
            if i < n - 1:
                await asyncio.sleep(stagger)

        elapsed = asyncio.get_event_loop().time() - cycle_start
        remaining = interval - elapsed
        if remaining > 0:
            await asyncio.sleep(remaining)


# ─── OSCQuery HTTP server ────────────────────────────────────────────────────

class _Handler(BaseHTTPRequestHandler):
    tree: dict = {}
    host_info: dict = {}

    def log_message(self, format, *args):
        pass    

    def do_GET(self):
        parsed = urlparse(self.path)
        if "HOST_INFO" in parsed.query:
            return self._json(self.host_info)

        node = self.tree
        if parsed.path not in ("", "/"):
            for part in parsed.path.strip("/").split("/"):
                node = node.get("CONTENTS", {}).get(part)
                if node is None:
                    self.send_error(404)
                    return
        self._json(node)

    def _json(self, obj):
        body = json.dumps(obj).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def start_http(tree, osc_ip, osc_port, http_port):
    _Handler.tree = tree
    _Handler.host_info = {
        "NAME": "VRChat-Client",
        "OSC_IP": osc_ip,
        "OSC_PORT": osc_port,
        "OSC_TRANSPORT": "UDP",
        "EXTENSIONS": {"ACCESS": True, "VALUE": True},
    }
    srv = HTTPServer(("0.0.0.0", http_port), _Handler)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    print(f"[oscquery] http://0.0.0.0:{http_port}")

    local_ip = socket.inet_aton(
        socket.gethostbyname(socket.gethostname())
    )

    zc = Zeroconf()

    # 1) Advertise the OSCQuery HTTP service (_oscjson._tcp)
    oscjson_info = ServiceInfo(
        "_oscjson._tcp.local.",
        "VRChat-Client._oscjson._tcp.local.",
        addresses=[local_ip],
        port=http_port,
        properties={},
    )
    zc.register_service(oscjson_info)
    print(f"[mdns] registered VRChat-Client._oscjson._tcp.local. on port {http_port}")

    # 2) Advertise the OSC UDP service (_osc._udp)
    osc_info = ServiceInfo(
        "_osc._udp.local.",
        "VRChat-Client._osc._udp.local.",
        addresses=[local_ip],
        port=osc_port,
        properties={},
    )
    zc.register_service(osc_info)
    print(f"[mdns] registered VRChat-Client._osc._udp.local. on port {osc_port}")

    return srv, zc

# ─── Main ─────────────────────────────────────────────────────────────────────

def main():
    ap = argparse.ArgumentParser(description="Fake VRChat OSC/OSCQuery server")
    ap.add_argument("configs", nargs="+", help="Haptic config JSON file paths")
    ap.add_argument("--osc-ip",      default="127.0.0.1")
    ap.add_argument("--target-port", default=9001, type=int,
                    help="UDP port the application listens on (default 9001)")
    ap.add_argument("--http-port",   default=8080, type=int,
                    help="OSCQuery HTTP port (default 8080)")
    args = ap.parse_args()

    configs = load_configs(args.configs)
    tree, float_addrs, avatar_id = build_tree(configs)

    print(f"[init] {len(configs)} config(s), {len(float_addrs)} float param(s)")
    for a in sorted(float_addrs):
        print(f"       {a}")

    srv, zc = start_http(tree, args.osc_ip, args.target_port, args.http_port)

    try:
        asyncio.run(ramp_loop(float_addrs, avatar_id, args.osc_ip, args.target_port))
    except KeyboardInterrupt:
        zc.unregister_all_services()
        zc.close()
        print("\nstopped.")

if __name__ == "__main__":
    main()