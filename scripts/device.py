#!/usr/bin/env python3
"""
Simulated WebSocket haptic device.

Connects to the haptic-core websocket device server, registers the nodes loaded
from one or more config files, and renders the inbound feedback buffer live,
in place (no scrolling), even at ~100Hz.

Wire protocol (must match src/devices/websocket.rs):
  client -> server (text JSON):
    {"type":"hello","id":..,"name":..,"nodes":[..]}
    {"type":"updateInfo","nodes":[..]}
  server -> client (binary): tightly packed little-endian f32, one per node.

Usage:
  pip install websockets
  python ws_device.py map1.json [map2.json ...] \
      --url ws://127.0.0.1:8431 --id sim-1 --name "Sim Device"
"""

import argparse
import asyncio
import json
import struct
import sys
import time

try:
    import websockets
except ImportError:
    sys.exit("Missing dependency: pip install websockets")


# ---------- config loading ----------

# Map interactionTag spellings to NodeGroup flag names (must match groups.rs).
_GROUP_NAMES = {
    "Head", "UpperArmRight", "UpperArmLeft", "LowerArmRight", "LowerArmLeft",
    "TorsoRight", "TorsoLeft", "TorsoFront", "TorsoBack",
    "UpperLegRight", "UpperLegLeft", "LowerLegRight", "LowerLegLeft",
    "FootRight", "FootLeft", "All",
}


def to_haptic_node(raw, path, idx):
    """Translate a map-config node into the server's HapticNode shape:
    {"loc": [x,y,z], "groups": ["Head", ...]}."""
    loc = raw.get("location")
    if not (isinstance(loc, list) and len(loc) == 3):
        sys.exit(f"{path} node {idx}: missing/invalid 'location'")

    tags = raw.get("interactionTags", [])
    if not isinstance(tags, list):
        sys.exit(f"{path} node {idx}: 'interactionTags' must be an array")
    unknown = [t for t in tags if t not in _GROUP_NAMES]
    if unknown:
        sys.exit(f"{path} node {idx}: unknown group(s) {unknown}; "
                 f"must match NodeGroup flags")

    return {"loc": [float(c) for c in loc], "groups": list(tags)}


def load_nodes(paths):
    """Load each config's `nodes` array and convert to HapticNode shape.

    Returns (haptic_nodes, raw_nodes) — raw kept only for display labels.
    """
    haptic, raw_nodes = [], []
    for p in paths:
        with open(p, "r", encoding="utf-8") as f:
            data = json.load(f)
        file_nodes = data.get("nodes")
        if not isinstance(file_nodes, list):
            sys.exit(f"{p}: no top-level 'nodes' array")
        for i, n in enumerate(file_nodes):
            haptic.append(to_haptic_node(n, p, i))
            raw_nodes.append(n)
    if not haptic:
        sys.exit("No nodes loaded from any config file")
    return haptic, raw_nodes


def node_label(node, idx):
    """Best-effort human label: first input address, else location, else index."""
    inputs = node.get("inputs")
    if isinstance(inputs, list) and inputs:
        addr = inputs[0].get("address")
        if addr:
            return addr
    loc = node.get("location")
    if isinstance(loc, list) and len(loc) == 3:
        return f"({loc[0]:.2f},{loc[1]:.2f},{loc[2]:.2f})"
    return f"node {idx}"


# ---------- shared state ----------

class Shared:
    __slots__ = ("values", "frames", "last_frame_ns", "connected")

    def __init__(self, n):
        self.values = [0.0] * n
        self.frames = 0            # total feedback frames received
        self.last_frame_ns = 0     # monotonic time of last frame
        self.connected = False


# ---------- networking ----------

async def receiver(ws, shared):
    """Overwrite the latest buffer on every binary frame. Never blocks rendering."""
    async for msg in ws:
        if isinstance(msg, bytes):
            count = len(msg) // 4
            if count == 0:
                continue
            vals = struct.unpack(f"<{count}f", msg[: count * 4])
            # write in place; render loop reads independently
            shared.values[:count] = vals
            shared.frames += 1
            shared.last_frame_ns = time.monotonic_ns()
        # text frames (if any) ignored; server sends binary feedback only


# ---------- rendering ----------

ESC = "\x1b["
HOME = ESC + "H"          # cursor to row 1 col 1
CLR_EOL = ESC + "K"       # clear to end of line
CLR_BELOW = ESC + "J"     # clear from cursor down
HIDE_CUR = ESC + "?25l"
SHOW_CUR = ESC + "?25h"
BAR_W = 30


def bar(v):
    v = 0.0 if v < 0.0 else 1.0 if v > 1.0 else v
    filled = int(round(v * BAR_W))
    return "#" * filled + "-" * (BAR_W - filled)


async def render(shared, labels, url, fps):
    period = 1.0 / fps
    label_w = min(max((len(l) for l in labels), default=4), 24)
    out = sys.stdout
    out.write(HIDE_CUR)
    last_frames = 0
    last_rate_t = time.monotonic()
    rate = 0.0
    try:
        while True:
            now = time.monotonic()
            # measure inbound frame rate roughly once per second
            if now - last_rate_t >= 1.0:
                rate = (shared.frames - last_frames) / (now - last_rate_t)
                last_frames = shared.frames
                last_rate_t = now

            stale_ms = (time.monotonic_ns() - shared.last_frame_ns) / 1e6
            status = "connected" if shared.connected else "waiting..."
            lines = [
                f"{url}  [{status}]  frames={shared.frames}  "
                f"~{rate:5.1f} Hz  last={stale_ms:6.1f} ms ago",
                "",
            ]
            for i, (lbl, v) in enumerate(zip(labels, shared.values)):
                name = (lbl[: label_w - 1] + "\u2026") if len(lbl) > label_w else lbl
                lines.append(f"[{i:3}] {name:<{label_w}} {v:5.3f} |{bar(v)}|")

            # redraw from home, clearing each line, then clear any leftover below
            buf = HOME + ("\n".join(line + CLR_EOL for line in lines)) + "\n" + CLR_BELOW
            out.write(buf)
            out.flush()
            await asyncio.sleep(period)
    finally:
        out.write(SHOW_CUR)
        out.flush()


# ---------- main ----------

async def run(args):
    nodes, raw_nodes = load_nodes(args.configs)
    labels = [node_label(n, i) for i, n in enumerate(raw_nodes)]
    shared = Shared(len(nodes))

    hello = json.dumps(
        {"type": "hello", "id": args.id, "name": args.name, "nodes": nodes}
    )

    render_task = asyncio.create_task(render(shared, labels, args.url, args.fps))
    try:
        while True:
            try:
                async with websockets.connect(args.url, max_size=None) as ws:
                    await ws.send(hello)
                    shared.connected = True
                    await receiver(ws, shared)
            except Exception:
                # reconnect on any drop; render keeps running and shows "waiting..."
                pass
            shared.connected = False
            if not args.reconnect:
                break
            await asyncio.sleep(args.retry)
    finally:
        render_task.cancel()
        try:
            await render_task
        except asyncio.CancelledError:
            pass


def parse_args():
    ap = argparse.ArgumentParser(description="WebSocket haptic device simulator")
    ap.add_argument("configs", nargs="+", help="map/config JSON file(s) with a 'nodes' array")
    ap.add_argument("--url", default="ws://127.0.0.1:8431")
    ap.add_argument("--id", default="sim-device")
    ap.add_argument("--name", default="Python Sim")
    ap.add_argument("--fps", type=float, default=30.0, help="screen refresh rate")
    ap.add_argument("--retry", type=float, default=1.0, help="reconnect delay (s)")
    ap.add_argument("--no-reconnect", dest="reconnect", action="store_false")
    return ap.parse_args()


if __name__ == "__main__":
    try:
        asyncio.run(run(parse_args()))
    except KeyboardInterrupt:
        sys.stdout.write(SHOW_CUR + "\n")