#!/bin/sh
set -eu

PORT=6868
TAG="HapticGui"

if [ "$(id -u)" -ne 0 ]; then
    echo "install: must run as root to modify the firewall" >&2
    exit 1
fi

if systemctl is-active --quiet firewalld 2>/dev/null; then
    firewall-cmd --permanent --add-port="${PORT}/udp" >/dev/null
    firewall-cmd --reload >/dev/null
    echo "Opened ${PORT}/udp via firewalld"
elif command -v ufw >/dev/null 2>&1; then
    ufw allow "${PORT}/udp" comment "$TAG" >/dev/null
    echo "Opened ${PORT}/udp via ufw"
else
    echo "No supported firewall found; skipping. Open ${PORT}/udp manually if a firewall is added later." >&2
fi

exit 0
