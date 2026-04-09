#!/usr/bin/env bash
# ==============================================================================
# AEGIS OS — UNIFIED INSTALLER (NATIVE + DOCKER)
# ==============================================================================
# OS: Ubuntu / Debian / Linux
# Author: Antigravity SRE Team
# Reference: CORE-040
# ==============================================================================

set -euo pipefail

# --- Configuration ---
INSTALL_ROOT="/opt/aegis"
CONFIG_DIR="/etc/aegis"
BIN_DIR="/usr/local/bin"
DATA_DIR="/var/lib/aegis"
UI_DIST_PATH="/usr/share/aegis/ui"
ENV_FILE="$CONFIG_DIR/aegis.env"
LOG_FILE="/tmp/aegis_install.log"

GITHUB_ORG="Gustavo324234"
GITHUB_REPO="Aegis-Core"
RELEASE_TAG="latest"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Helpers
log()     { echo -e "[INFO] $(date '+%H:%M:%S') - $1" >> "$LOG_FILE" 2>/dev/null || true; echo -e "${CYAN}  ->${NC} $1"; }
success() { echo -e "[OK]   $(date '+%H:%M:%S') - $1" >> "$LOG_FILE" 2>/dev/null || true; echo -e "${GREEN}  [OK]${NC} $1"; }
warn()    { echo -e "[WARN] $(date '+%H:%M:%S') - $1" >> "$LOG_FILE" 2>/dev/null || true; echo -e "${YELLOW}  [!]${NC} $1"; }
error()   { echo -e "[ERROR] $(date '+%H:%M:%S') - $1" >> "$LOG_FILE" 2>/dev/null || true; echo -e "${RED}[ERROR]${NC} $1" >&2; exit 1; }

# --- Banner ---
print_banner() {
    echo -e "${CYAN}"
    cat << "EOF"
    ___  _____ _____ _____ _____   _____ _____
   / _ \|  ___|  __ \_   _/  ___| |  _  /  ___|
  / /_\ \ |__ | |  \/ | | \ `--.  | | | \ `--. 
  |  _  |  __|| | __  | |  `--. \ | | | `--. \
  | | | | |___| |_\ \_| |_/\__/ / \ \_/ /\__/ /
  \_| |_\____/ \____/\___/\____/   \___/\____/
EOF
    echo -e "${NC}"
    echo -e "      Aegis OS Unified Installer - v2.0.0 (Core)"
    echo -e "------------------------------------------------------------"
}

# --- System Audit ---
check_root() {
    if [[ "$EUID" -ne 0 ]]; then
        error "This script must be run as root (use sudo)."
    fi
}

detect_arch() {
    local arch
    arch=$(uname -m)
    case "$arch" in
        x86_64)  ARCH="x86_64" ;;
        aarch64) ARCH="arm64" ;;
        *)       error "Unsupported architecture: $arch" ;;
    esac
    log "Architecture detected: $ARCH"
}

# --- Dependencies ---
install_dependencies() {
    log "Updating package lists..."
    apt-get update -qq >> "$LOG_FILE" 2>&1
    
    local deps=("curl" "openssl" "ca-certificates" "tar")
    if [[ "$INSTALL_MODE" == "2" ]]; then
        deps+=("docker.io" "docker-compose-v2")
    fi

    for dep in "${deps[@]}"; do
        if ! command -v "$dep" &> /dev/null && ! dpkg -l "$dep" &> /dev/null; then
            log "Installing $dep..."
            apt-get install -y "$dep" -qq >> "$LOG_FILE" 2>&1 || warn "Failed to install $dep, it might already be present."
        fi
    done
}

# --- Installation Flows ---

install_native() {
    log "Starting Native installation..."
    
    # 1. Create user
    if ! id -u aegis >/dev/null 2>&1; then
        log "Creating 'aegis' system user..."
        useradd --system --no-create-home --shell /sbin/nologin aegis >> "$LOG_FILE" 2>&1
    fi

    # 2. Create directories
    mkdir -p "$CONFIG_DIR" "$DATA_DIR" "$UI_DIST_PATH"
    chown -R aegis:aegis "$DATA_DIR"

    # 3. Download binaries (ank-server and aegis-supervisor)
    local release_url="https://github.com/${GITHUB_ORG}/${GITHUB_REPO}/releases/download/${RELEASE_TAG}"
    
    log "Downloading ank-server bios from ${release_url}/ank-server-linux-${ARCH}..."
    curl -L --fail --silent --show-error "${release_url}/ank-server-linux-${ARCH}" -o "${BIN_DIR}/ank-server" >> "$LOG_FILE" 2>&1 || error "Failed to download ank-server"
    chmod +x "${BIN_DIR}/ank-server"

    log "Downloading aegis-supervisor from ${release_url}/aegis-supervisor-linux-${ARCH}..."
    curl -L --fail --silent --show-error "${release_url}/aegis-supervisor-linux-${ARCH}" -o "${BIN_DIR}/aegis-supervisor" >> "$LOG_FILE" 2>&1 || error "Failed to download aegis-supervisor"
    chmod +x "${BIN_DIR}/aegis-supervisor"

    # 4. Download UI assets
    log "Downloading UI assets from ${release_url}/ui-dist.tar.gz..."
    curl -L --fail --silent --show-error "${release_url}/ui-dist.tar.gz" -o "/tmp/ui-dist.tar.gz" >> "$LOG_FILE" 2>&1 || error "Failed to download UI assets"
    tar -xzf "/tmp/ui-dist.tar.gz" -C "$UI_DIST_PATH" --strip-components=1 >> "$LOG_FILE" 2>&1 || error "Failed to extract UI assets"
    rm "/tmp/ui-dist.tar.gz"

    # 5. Generate Environment
    if [[ ! -f "$ENV_FILE" ]]; then
        log "Generating root key..."
        local root_key
        root_key=$(openssl rand -hex 32)
        cat > "$ENV_FILE" <<EOF
AEGIS_ROOT_KEY=${root_key}
AEGIS_DATA_DIR=${DATA_DIR}
AEGIS_MTLS_STRICT=false
UI_DIST_PATH=${UI_DIST_PATH}
EOF
        chmod 600 "$ENV_FILE"
        chown aegis:aegis "$ENV_FILE"
        success "Environment file created with root key."
    fi

    # 6. Install systemd service
    log "Installing systemd service (aegis-supervisor)..."
    cat > /etc/systemd/system/aegis.service <<EOF
[Unit]
Description=Aegis OS — Cognitive Operating System
After=network.target

[Service]
Type=simple
User=aegis
Group=aegis
EnvironmentFile=${ENV_FILE}
ExecStart=${BIN_DIR}/aegis-supervisor --service
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload
    systemctl enable aegis.service >> "$LOG_FILE" 2>&1
    systemctl start aegis.service >> "$LOG_FILE" 2>&1
    
    success "Native installation complete."
}

install_docker() {
    log "Starting Docker installation..."
    
    mkdir -p "$INSTALL_ROOT"
    
    # 2. Download docker-compose.yml
    local raw_url="https://raw.githubusercontent.com/${GITHUB_ORG}/${GITHUB_REPO}/main/installer/docker-compose.yml"
    log "Fetching docker-compose.yml..."
    curl -L --fail --silent --show-error "$raw_url" -o "${INSTALL_ROOT}/docker-compose.yml" >> "$LOG_FILE" 2>&1 || error "Failed to download docker-compose.yml"

    # 3. Generate .env
    if [[ ! -f "${INSTALL_ROOT}/.env" ]]; then
        local root_key
        root_key=$(openssl rand -hex 32)
        cat > "${INSTALL_ROOT}/.env" <<EOF
AEGIS_ROOT_KEY=${root_key}
AEGIS_MTLS_STRICT=false
AEGIS_DATA_DIR=./data
EOF
        chmod 600 "${INSTALL_ROOT}/.env"
        success "Docker .env file created."
    fi

    # 4. Start compose
    log "Executing docker compose up -d..."
    (cd "$INSTALL_ROOT" && docker compose up -d >> "$LOG_FILE" 2>&1) || error "Docker Compose failed"
    
    success "Docker installation complete."
}

# --- TUI / Menu ---
show_menu() {
    clear
    print_banner
    echo "┌─────────────────────────────────────────────────┐"
    echo "│           AEGIS OS — INSTALLATION MODE          │"
    echo "├─────────────────────────────────────────────────┤"
    echo "│  [1] Native (recommended)                       │"
    echo "│      Single binary, no Docker required          │"
    echo "│  [2] Docker                                     │"
    echo "│      Containerized deployment                   │"
    echo "└─────────────────────────────────────────────────┘"
    echo ""
    read -rp "Selection [1-2]: " choice

    case "${choice:-1}" in
        1) INSTALL_MODE="1" ;;
        2) INSTALL_MODE="2" ;;
        *) warn "Invalid selection, defaulting to Native." ; INSTALL_MODE="1" ;;
    esac
}

# --- Main ---
check_root
detect_arch
show_menu
install_dependencies

if [[ "$INSTALL_MODE" == "1" ]]; then
    install_native
else
    install_docker
fi

# 8. Wait for Health + Token
log "Waiting for Aegis service to initialize (max 60s)..."
attempts=0
while ! curl -s "http://localhost:8000/health" | grep -q "Online" && [[ $attempts -lt 30 ]]; do
    sleep 2
    attempts=$((attempts + 1))
done

if curl -s "http://localhost:8000/health" | grep -q "Online"; then
    success "Aegis is UP and RUNNING at http://localhost:8000"
    
    log "Fetching setup token..."
    TOKEN=""
    if [[ "$INSTALL_MODE" == "1" ]]; then
        TOKEN=$(journalctl -u aegis --since "10 min ago" | grep "setup_token=" | tail -n 1 | sed 's/.*setup_token=\([^ ]*\).*/\1/' || echo "")
    else
        TOKEN=$(docker compose -f "${INSTALL_ROOT}/docker-compose.yml" logs ank-server 2>&1 | grep "setup_token=" | tail -n 1 | sed 's/.*setup_token=\([^ ]*\).*/\1/' || echo "")
    fi

    if [[ -n "$TOKEN" ]]; then
        echo -e "\n${GREEN}################################################################${NC}"
        echo -e "${GREEN}#          AEGIS OS - DEPLOYMENT COMPLETED                      #${NC}"
        echo -e "${GREEN}################################################################${NC}"
        echo ""
        echo -e "${CYAN}  FIRST TIME SETUP:${NC}"
        echo -e "  Open this URL in your browser:"
        echo ""
        echo -e "  ${GREEN}http://localhost:8000?setup_token=${TOKEN}${NC}"
        echo ""
        echo -e "  Token expires in 30 minutes."
        echo -e "${GREEN}################################################################${NC}\n"
    else
        echo -e "\n${GREEN}Installation complete.${NC}"
        echo -e "Access URL: ${CYAN}http://localhost:8000${NC}"
        echo -e "If this is a new installation, check logs for setup_token."
    fi
else
    warn "Aegis health check timed out. Please check logs manually."
fi
