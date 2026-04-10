#!/usr/bin/env bash
# ==============================================================================
# AEGIS OS - UNINSTALLER
# ==============================================================================
# Description: Full removal script for Aegis OS (Native/Docker)
# ==============================================================================

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# Check root
if [[ "$EUID" -ne 0 ]]; then
    echo -e "${RED}[ERROR]${NC} This script must be run as root (use sudo)." >&2
    exit 1
fi

echo -e "${RED}"
echo "╔════════════════════════════════════════════════════════════╗"
echo "║          AEGIS OS - UNINSTALLATION UTILITY                 ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo -e "${NC}"
echo -e "${YELLOW}WARNING: This will permanently delete ALL data, including:${NC}"
echo "  - Master Admin & Tenant Users"
echo "  - Identity databases (admin.db)"
echo "  - Chat histories and workspace files"
echo "  - System configurations (/etc/aegis)"
echo ""
read -rp "Are you absolutely sure you want to proceed? (y/N): " confirm

if [[ ! "$confirm" =~ ^[yY]$ ]]; then
    echo "Uninstallation aborted."
    exit 0
fi

# Detect Mode
MODE="native"
if [[ -f "/etc/aegis/mode" ]]; then
    MODE=$(cat "/etc/aegis/mode")
fi

echo -e "${CYAN}Uninstalling Aegis OS (Mode: $MODE)...${NC}"

if [[ "$MODE" == "docker" ]]; then
    if [[ -d "/opt/aegis" ]]; then
        echo "→ Stopping Docker containers and removing volumes..."
        (cd /opt/aegis && docker compose down -v) || true
        rm -rf "/opt/aegis"
    fi
fi

# Generic cleanup (Native & Shared)
echo "→ Stopping and disabling systemd service..."
systemctl stop aegis 2>/dev/null || true
systemctl disable aegis 2>/dev/null || true

echo "→ Removing systemd unit files..."
rm -f /etc/systemd/system/aegis.service
systemctl daemon-reload

echo "→ Removing binaries..."
rm -f /usr/local/bin/ank-server
rm -f /usr/local/bin/aegis

echo "→ Removing configuration and data directories..."
rm -rf /etc/aegis
rm -rf /var/lib/aegis
rm -rf /usr/share/aegis

# Remove internal installer log
rm -f /root/aegis_install.log

echo ""
echo -e "${GREEN}################################################################${NC}"
echo -e "${GREEN}#          AEGIS OS HAS BEEN FULLY UNINSTALLED                 #${NC}"
echo -e "${GREEN}################################################################${NC}"
echo ""
