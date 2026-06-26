#!/usr/bin/env python3
"""
Dump every OSC endpoint exposed by a running VRChat instance via OSCQuery.

VRChat advertises an OSCQuery HTTP server over mDNS (service type
"_oscjson._tcp", instance name "VRChat-Client-XXXXXX"). The server's root
("/") returns a JSON tree describing every OSC address, its OSC type tag,
its access mode, and its current value. This script discovers that server,
walks the tree, and emits a structured dump (addresses + types + values).

Note: OSCQuery exposes *current* values (the VALUE field); the protocol has
no separate "default" field, so current == default at avatar-load time.

Dependency: zeroconf  (pip install zeroconf) -- only needed for discovery.
            Manual mode (--host/--port) uses the stdlib alone.
"""

from __future__ import annotations

import argparse
import json
import sys
import time
import urllib.request
from urllib.error import URLError

OSCQUERY_SERVICE = "_oscjson._tcp.local."

# OSC type tag -> human readable (OSC 1.0 + 1.1, covers VRChat usage).
TYPE_TAGS = {
    "i": "int32", "f": "float32", "s": "string", "b": "blob",
    "h": "int64", "t": "timetag", "d": "float64", "S": "symbol",
    "c": "char", "r": "rgba", "m": "midi",
    "T": "true", "F": "false", "N": "nil", "I": "infinitum",
}

# OSCQuery ACCESS values.
ACCESS = {0: "none", 1: "read", 2: "write", 3: "read/write"}


def format_type(type_tag: str | None) -> str:
    """OSC type tag shown as it appears; multi-value tags become an array, e.g. 'fff' -> '[fff]'."""
    if not type_tag:
        return ""
    return f"[{type_tag}]" if len(type_tag) > 1 else type_tag


def discover(timeout=4.0):
    """Browse mDNS for OSCQuery services. Returns {service_name: (host, port)}."""
    try:
        from zeroconf import Zeroconf, ServiceBrowser, ServiceListener
    except ImportError:
        sys.exit("mDNS discovery needs zeroconf: pip install zeroconf "
                 "(or pass --host/--port to skip discovery).")

    class _Listener(ServiceListener):
        def __init__(self):
            self.found = {}

        def add_service(self, zc: Zeroconf, type_: str, name: str) -> None:
            info = zc.get_service_info(type_, name, timeout=2000)
            if info is None or info.port is None:
                return
            addrs = info.parsed_addresses()
            if addrs:
                self.found[name] = (addrs[0], info.port)

        def update_service(self, zc, type_, name):
            self.add_service(zc, type_, name)

        def remove_service(self, zc, type_, name):
            self.found.pop(name, None)

    zc = Zeroconf()
    listener = _Listener()
    ServiceBrowser(zc, OSCQUERY_SERVICE, listener)
    try:
        time.sleep(timeout)
    finally:
        zc.close()
    return dict(listener.found)


def pick_vrchat(services: dict[str, tuple[str, int]]) -> tuple[str, str, int] | None:
    """Prefer a VRChat-Client-* service; fall back to the first one found."""
    for name, (host, port) in services.items():
        if name.startswith("VRChat-Client"):
            return name, host, port
    for name, (host, port) in services.items():
        return name, host, port
    return None


def http_get_json(host, port, path="/"):
    url = f"http://{host}:{port}{path}"
    req = urllib.request.Request(url, headers={"User-Agent": "vrc-osc-dump"})
    with urllib.request.urlopen(req, timeout=5) as resp:
        return json.loads(resp.read().decode("utf-8"))


def walk(node, endpoints):
    """Recursively collect every node that declares a TYPE (an addressable method)."""
    if not isinstance(node, dict):
        return
    if "TYPE" in node:
        endpoints.append({
            "address": node.get("FULL_PATH", ""),
            "type": node.get("TYPE", ""),
            "type_readable": format_type(node.get("TYPE")),
            "access": ACCESS.get(node.get("ACCESS", 0), str(node.get("ACCESS"))),
            "value": node.get("VALUE"),
            "range": node.get("RANGE"),
            "description": node.get("DESCRIPTION", ""),
        })
    for child in (node.get("CONTENTS") or {}).values():
        walk(child, endpoints)


def format_table(result):
    lines = []
    svc = result["service"]
    lines.append(f"# {svc['name']}  ({svc['http_host']}:{svc['http_port']})")
    hi = result.get("host_info") or {}
    if hi:
        lines.append(f"# OSC: {hi.get('OSC_IP')}:{hi.get('OSC_PORT')} "
                     f"({hi.get('OSC_TRANSPORT', 'UDP')})")
    lines.append(f"# {result['endpoint_count']} endpoints\n")
    eps = result["endpoints"]
    addr_w = max((len(e["address"]) for e in eps), default=20)
    type_w = max((len(e["type_readable"]) for e in eps), default=8)
    header = f"{'ADDRESS':<{addr_w}}  {'TYPE':<{type_w}}  {'ACCESS':<10}  VALUE"
    lines.append(header)
    lines.append("-" * len(header))
    for e in eps:
        val = "" if e["value"] is None else json.dumps(e["value"], ensure_ascii=False)
        lines.append(f"{e['address']:<{addr_w}}  {e['type_readable']:<{type_w}}  "
                     f"{e['access']:<10}  {val}")
    return "\n".join(lines)


def main():
    ap = argparse.ArgumentParser(description="Dump VRChat OSC endpoints via OSCQuery.")
    ap.add_argument("--host", help="OSCQuery HTTP host (skip mDNS discovery).")
    ap.add_argument("--port", type=int, help="OSCQuery HTTP port (skip mDNS discovery).")
    ap.add_argument("--timeout", type=float, default=4.0, help="mDNS discovery timeout in seconds.")
    ap.add_argument("--format", choices=["json", "table"], default="json", help="Output format.")
    ap.add_argument("-o", "--output", help="Write output to this file instead of stdout.")
    args = ap.parse_args()

    if args.host or args.port:
        if not (args.host and args.port):
            ap.error("--host and --port must be used together")
        name, host, port = "(manual)", str(args.host), int(args.port)
    else:
        picked = pick_vrchat(discover(args.timeout))
        if picked is None:
            sys.exit("No OSCQuery service found. Is VRChat running with OSC enabled? "
                     "On Windows the server is localhost-only -- find the TCP port in the "
                     "VRChat log (line: 'Advertising Service ... of type OSCQuery on <port>') "
                     "and pass --host 127.0.0.1 --port <port>.")
        name, host, port = picked

    try:
        root = http_get_json(host, port, "/")
    except (URLError, OSError) as e:
        sys.exit(f"Failed to fetch OSCQuery tree from {host}:{port}: {e}")

    try:
        host_info = http_get_json(host, port, "/?HOST_INFO")
    except (URLError, OSError, ValueError):
        host_info = None

    endpoints = []
    walk(root, endpoints)
    endpoints.sort(key=lambda e: e["address"])

    result = {
        "service": {"name": name, "http_host": host, "http_port": port},
        "host_info": host_info,
        "endpoint_count": len(endpoints),
        "endpoints": endpoints,
    }

    out = (json.dumps(result, indent=2, ensure_ascii=False)
           if args.format == "json" else format_table(result))

    if args.output:
        with open(args.output, "w", encoding="utf-8") as f:
            f.write(out + "\n")
        print(f"Wrote {len(endpoints)} endpoints to {args.output}", file=sys.stderr)
    else:
        print(out)


if __name__ == "__main__":
    main()