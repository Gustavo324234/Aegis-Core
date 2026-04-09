#!/usr/bin/env bash
# ==============================================================================
# AEGIS OS — UNIFIED INSTALLER (NATIVE + DOCKER)
# ==============================================================================
# OS: Ubuntu / Debian / Linux
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
RELEASE_TAG="nightly"

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
  |  _  |  __|| | __  | |  `--. \ | | | |`--. \
  | | | | |___| |_\ \_| |_/\__/ / \ \_/ /\__/ /
  \_| |_\____/ \____/\___/\____/   \___/\____/
EOF
    echo -e "${NC}"
    echo -e "      Aegis OS Unified Installer"
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
    if [[ "${INSTALL_MODE}" == "2" ]]; then
        deps+=("docker.io" "docker-compose-v2")
    fi

    for dep in "${deps[@]}"; do
        if ! command -v "$dep" &>/dev/null; then
            log "Installing $dep..."
            apt-get install -y "$dep" -qq >> "$LOG_FILE" 2>&1 || warn "Could not install $dep"
        fi
    done
}

# --- Installation Flows ---

install_native() {
    log "Starting native installation..."

    # 1. Create system user
    if ! id -u aegis >/dev/null 2>&1; then
        log "Creating 'aegis' system user..."
        useradd --system --no-create-home --shell /sbin/nologin aegis >> "$LOG_FILE" 2>&1
    fi

    # 2. Create directories
    mkdir -p "$CONFIG_DIR" "$DATA_DIR" "$UI_DIST_PATH"
    chown -R aegis:aegis "$DATA_DIR"

    # 3. Download ank-server binary
    # El CI publica: ank-server-linux-x86_64.tar.gz conteniendo ank-server-linux-x86_64
    local release_url="https://github.com/${GITHUB_ORG}/${GITHUB_REPO}/releases/download/${RELEASE_TAG}"
    local artifact_name="ank-server-linux-${ARCH}"
    local tar_file="${artifact_name}.tar.gz"

    log "Downloading ank-server (${RELEASE_TAG} / ${artifact_name})..."
    curl -L --fail --progress-bar \
        "${release_url}/${tar_file}" \
        -o "/tmp/${tar_file}" \
        || error "Failed to download ank-server. Verify that release '${RELEASE_TAG}' exists at github.com/${GITHUB_ORG}/${GITHUB_REPO}/releases"

    # Extraer — el tar contiene un archivo llamado ank-server-linux-{arch}
    tar -xzf "/tmp/${tar_file}" -C "/tmp/" >> "$LOG_FILE" 2>&1
    mv "/tmp/${artifact_name}" "${BIN_DIR}/ank-server"
    chmod +x "${BIN_DIR}/ank-server"
    rm "/tmp/${tar_file}"
    success "ank-server installed → ${BIN_DIR}/ank-server"

    # 4. Download UI assets
    log "Downloading UI assets..."
    curl -L --fail --progress-bar \
        "${release_url}/ui-dist.tar.gz" \
        -o "/tmp/ui-dist.tar.gz" \
        || error "Failed to download UI assets."

    tar -xzf "/tmp/ui-dist.tar.gz" -C "$UI_DIST_PATH" >> "$LOG_FILE" 2>&1
    rm "/tmp/ui-dist.tar.gz"
    success "UI assets installed → ${UI_DIST_PATH}"

    # 5. Install aegis CLI
    log "Installing aegis CLI..."
    local script_dir
    script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    if [[ -f "${script_dir}/aegis" ]]; then
        cp "${script_dir}/aegis" "${BIN_DIR}/aegis"
    else
        curl -L --fail --silent \
            "https://raw.githubusercontent.com/${GITHUB_ORG}/${GITHUB_REPO}/main/installer/aegis" \
            -o "${BIN_DIR}/aegis" \
            || warn "Could not download aegis CLI"
    fi
    chmod +x "${BIN_DIR}/aegis"
    success "aegis CLI installed → ${BIN_DIR}/aegis"

    # 6. Generate environment file (solo si no existe — preserva instalaciones previas)
    if [[ ! -f "$ENV_FILE" ]]; then
        log "Generating AEGIS_ROOT_KEY..."
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
        success "Environment file created → ${ENV_FILE}"
    else
        warn "Environment file already exists at ${ENV_FILE} — preserving existing keys."
        # Asegurar que UI_DIST_PATH esté en el env (puede faltar en instalaciones antiguas)
        if ! grep -q "UI_DIST_PATH" "$ENV_FILE"; then
            echo "UI_DIST_PATH=${UI_DIST_PATH}" >> "$ENV_FILE"
        fi
    fi

    # 7. Write mode file
    echo "native" > /etc/aegis/mode

    # 8. Install systemd service
    log "Installing systemd service..."
    cat > /etc/systemd/system/aegis.service <<EOF
[Unit]
Description=Aegis OS — Cognitive Operating System
Documentation=https://github.com/${GITHUB_ORG}/${GITHUB_REPO}
After=network.target
Wants=network.target

[Service]
Type=simple
User=aegis
Group=aegis
EnvironmentFile=${ENV_FILE}
ExecStart=${BIN_DIR}/ank-server
Restart=on-failure
RestartSec=5s
TimeoutStopSec=10s
NoNewPrivileges=true
ProtectSystem=full
ProtectHome=true
ReadWritePaths=${DATA_DIR} ${CONFIG_DIR}
StandardOutput=journal
StandardError=journal
SyslogIdentifier=aegis

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload >> "$LOG_FILE" 2>&1
    systemctl enable aegis.service >> "$LOG_FILE" 2>&1
    systemctl start aegis.service >> "$LOG_FILE" 2>&1
    success "systemd service installed and started."
}

install_docker() {
    log "Starting Docker installation..."

    mkdir -p "$INSTALL_ROOT"

    # Download docker-compose.yml
    local raw_url="https://raw.githubusercontent.com/${GITHUB_ORG}/${GITHUB_REPO}/main/installer/docker-compose.yml"
    log "Fetching docker-compose.yml..."
    curl -L --fail --silent \
        "$raw_url" \
        -o "${INSTALL_ROOT}/docker-compose.yml" \
        || error "Failed to download docker-compose.yml"

    # Generate .env
    if [[ ! -f "${INSTALL_ROOT}/.env" ]]; then
        local root_key
        root_key=$(openssl rand -hex 32)
        cat > "${INSTALL_ROOT}/.env" <<EOF
AEGIS_ROOT_KEY=${root_key}
AEGIS_MTLS_STRICT=false
EOF
        chmod 600 "${INSTALL_ROOT}/.env"
        success "Docker .env created → ${INSTALL_ROOT}/.env"
    else
        warn ".env already exists — preserving existing keys."
    fi

    # Write mode file
    mkdir -p /etc/aegis
    echo "docker" > /etc/aegis/mode

    # Install aegis CLI
    local script_dir
    script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    if [[ -f "${script_dir}/aegis" ]]; then
        cp "${script_dir}/aegis" "${BIN_DIR}/aegis"
        chmod +x "${BIN_DIR}/aegis"
    fi

    # Pull and start
    log "Pulling image and starting containers..."
    (cd "$INSTALL_ROOT" && docker compose up -d >> "$LOG_FILE" 2>&1) \
        || error "Docker Compose failed. Check ${LOG_FILE} for details."

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
    echo "│      Single binary, systemd managed             │"
    echo "│  [2] Docker                                     │"
    echo "│      Containerized deployment                   │"
    echo "└─────────────────────────────────────────────────┘"
    echo ""
    read -rp "Selection [1-2]: " choice

    case "${choice:-1}" in
        1) INSTALL_MODE="1" ;;
        2) INSTALL_MODE="2" ;;
        *) warn "Invalid selection, defaulting to Native."; INSTALL_MODE="1" ;;
    esac
}

# --- Health Check + Token Display ---
wait_and_show() {
    log "Waiting for Aegis to initialize (max 60s)..."
    local attempts=0
    while [[ $attempts -lt 30 ]]; do
        if curl -s "http://localhost:8000/health" 2>/dev/null | grep -q "Online"; then
            break
        fi
        sleep 2
        attempts=$((attempts + 1))
    done

    if curl -s "http://localhost:8000/health" 2>/dev/null | grep -q "Online"; then
        success "Aegis is UP at http://localhost:8000"

        local token=""
        if [[ "${INSTALL_MODE}" == "1" ]]; then
            token=$(journalctl -u aegis --since "5 min ago" --no-pager 2>/dev/null \
                | grep -oP '(?<=setup_token=)\S+' | tail -n 1 || true)
        else
            token=$(docker compose -f "${INSTALL_ROOT}/docker-compose.yml" logs ank-server 2>&1 \
                | grep -oP '(?<=setup_token=)\S+' | tail -n 1 || true)
        fi

        echo ""
        echo -e "${GREEN}################################################################${NC}"
        echo -e "${GREEN}#          AEGIS OS — INSTALLATION COMPLETE                    #${NC}"
        echo -e "${GREEN}################################################################${NC}"
        echo ""
        local ip
        ip=$(hostname -I 2>/dev/null | awk '{print $1}') || ip="localhost"

        if [[ -n "$token" ]]; then
            echo -e "${CYAN}  First-time setup URL:${NC}"
            echo -e "  ${GREEN}http://${ip}:8000?setup_token=${token}${NC}"
            echo ""
            echo -e "  Token expires in 30 minutes."
            echo -e "  To regenerate: ${CYAN}sudo aegis token${NC}"
        else
            echo -e "  Access URL: ${CYAN}http://${ip}:8000${NC}"
            echo -e "  Run ${CYAN}sudo aegis token${NC} to get the setup URL."
        fi
        echo -e "${GREEN}################################################################${NC}"
    else
        warn "Health check timed out after 60s."
        warn "Check logs with: sudo journalctl -u aegis -n 50"
    fi
}

# --- Main ---
check_root
detect_arch
show_menu
install_dependencies

if [[ "${INSTALL_MODE}" == "1" ]]; then
    install_native
else
    install_docker
fi

wait_and_show
