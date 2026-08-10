#!/bin/sh
# Run as root.
set -eu

PORT=6868

if [ "$(id -u)" -ne 0 ]; then
    echo "uninstall: must run as root to modify the firewall" >&2
    exit 1
fi

if systemctl is-active --quiet firewalld 2>/dev/null; then
    firewall-cmd --permanent --remove-port="${PORT}/udp" >/dev/null 2>&1 || true
    firewall-cmd --reload >/dev/null 2>&1 || true
    echo "Closed ${PORT}/udp via firewalld"
elif command -v ufw >/dev/null 2>&1; then
    # Delete matches on the rule spec; the comment is irrelevant to matching.
    ufw delete allow "${PORT}/udp" >/dev/null 2>&1 || true
    echo "Closed ${PORT}/udp via ufw"
fi

exit 0
