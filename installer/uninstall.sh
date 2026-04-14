#!/usr/bin/env bash
# ==============================================================================
# AEGIS OS - UNINSTALLATION UTILITY (SRE GRADE)
# ==============================================================================
# Description: Full removal script for Aegis OS with safety checks
# ==============================================================================

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

check_root() {
    if [[ "$EUID" -ne 0 ]]; then
        echo -e "${RED}[ERROR]${NC} This script must be run as root (use sudo)." >&2
        exit 1
    fi
}

print_warning() {
    clear
    echo -e "${RED}"
    echo "╔════════════════════════════════════════════════════════════╗"
    echo "║          AEGIS OS - DESTRUCTIVE REMOVAL                    ║"
    echo "╚════════════════════════════════════════════════════════════╝"
    echo -e "${NC}"
    echo -e "${YELLOW}DANGER: This action is permanent. All data will be wiped:${NC}"
    echo "  - Identity databases (admin.db)"
    echo "  - Chat histories and cognitive logs"
    echo "  - System configurations (/etc/aegis)"
    echo "  - Custom workspace files"
    echo ""
}

confirm_removal() {
    # If --force is passed, skip confirmation
    if [[ "${1:-}" == "--force" ]]; then
        return 0
    fi

    if [ -t 0 ] && command -v dialog &> /dev/null; then
        if ! dialog --title "CONFIRM DESTRUCTION" --yesno "Are you ABSOLUTELY sure you want to uninstall Aegis OS and delete ALL associated data?\n\nThis cannot be undone." 10 60; then
            echo "Uninstallation aborted."
            exit 0
        fi
    else
        read -rp "Final confirmation: Type 'DELETE' to proceed: " confirm < /dev/tty
        if [[ "$confirm" != "DELETE" ]]; then
            echo "Uninstallation aborted."
            exit 0
        fi
    fi
}

# --- Main Execution ---
check_root
print_warning
confirm_removal "$@"

echo -e "${CYAN}→ Stopping all Aegis processes...${NC}"
systemctl stop aegis 2>/dev/null || true
systemctl disable aegis 2>/dev/null || true

echo -e "${CYAN}→ Removing systemd unit files...${NC}"
rm -f /etc/systemd/system/aegis.service
systemctl daemon-reload

echo -e "${CYAN}→ Purging binaries...${NC}"
rm -f /usr/local/bin/ank-server
rm -f /usr/local/bin/aegis

echo -e "${CYAN}→ Wiping configuration and data...${NC}"
rm -rf /etc/aegis
rm -rf /var/lib/aegis
rm -rf /usr/share/aegis

# Docker cleanup if mode exists
if [[ -d "/opt/aegis" ]]; then
    echo -e "${CYAN}→ Cleaning up Docker orchestrator...${NC}"
    (cd /opt/aegis && docker compose down -v) 2>/dev/null || true
    rm -rf "/opt/aegis"
fi

echo ""
echo -e "${GREEN}################################################################${NC}"
echo -e "${GREEN}#          AEGIS OS HAS BEEN COMPLETELY REMOVED                #${NC}"
echo -e "${GREEN}################################################################${NC}"
echo ""
