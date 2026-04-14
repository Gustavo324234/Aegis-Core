#!/usr/bin/env bash
# ==============================================================================
# AEGIS OS — UNIFIED INSTALLER (SRE GRADE)
# ==============================================================================
# OS: Ubuntu / Debian
# Tickets: CORE-040
# ==============================================================================

set -euo pipefail

# --- Configuration ---
CONFIG_DIR="/etc/aegis"
BIN_DIR="/usr/local/bin"
DATA_DIR="/var/lib/aegis"
UI_DIST_PATH="/usr/share/aegis/ui"
ENV_FILE="$CONFIG_DIR/aegis.env"
LOG_FILE="/root/aegis_install.log"

GITHUB_ORG="Gustavo324234"
GITHUB_REPO="Aegis-Core"
RELEASE_TAG="nightly"

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
NC='\033[0m'

INSTALL_MODE="1"
ARCH="x86_64"

log()     { echo "[INFO] $(date '+%H:%M:%S') - $1" >> "$LOG_FILE"; echo -e "${CYAN}  ->${NC} $1"; }
success() { echo "[OK]   $(date '+%H:%M:%S') - $1" >> "$LOG_FILE"; echo -e "${GREEN}  [OK]${NC} $1"; }
warn()    { echo "[WARN] $(date '+%H:%M:%S') - $1" >> "$LOG_FILE"; echo -e "${YELLOW}  [!]${NC} $1"; }
error()   { echo "[ERROR] $(date '+%H:%M:%S') - $1" >> "$LOG_FILE"; echo -e "${RED}[ERROR]${NC} $1" >&2; exit 1; }

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
    echo -e "      Aegis OS Installer"
    echo -e "------------------------------------------------------------"
}

check_root() {
    if [[ "$EUID" -ne 0 ]]; then
        echo -e "${RED}[ERROR]${NC} Run as root: sudo bash install.sh" >&2
        exit 1
    fi
    touch "$LOG_FILE"
    chmod 600 "$LOG_FILE"
}

detect_arch() {
    case "$(uname -m)" in
        x86_64)  ARCH="x86_64" ;;
        aarch64) ARCH="arm64" ;;
        *)       error "Unsupported architecture: $(uname -m)" ;;
    esac
    log "Architecture: ${ARCH}"
}

show_menu() {
    # Non-interactive (curl | bash) → native by default, no prompts
    if [[ ! -t 0 ]]; then
        log "Non-interactive mode — defaulting to Native."
        INSTALL_MODE="1"
        return
    fi

    print_banner
    echo "┌─────────────────────────────────────────────────┐"
    echo "│           AEGIS OS — INSTALLATION MODE          │"
    echo "├─────────────────────────────────────────────────┤"
    echo "│  [1] Native (recommended)  — systemd binary     │"
    echo "│  [2] Docker                — containerized       │"
    echo "└─────────────────────────────────────────────────┘"
    echo ""
    read -rp "Selection [1-2, default 1]: " choice
    case "${choice:-1}" in
        2) INSTALL_MODE="2" ;;
        *) INSTALL_MODE="1" ;;
    esac
}

install_dependencies() {
    log "Updating package lists..."
    apt-get update -qq >> "$LOG_FILE" 2>&1

    local deps=("curl" "openssl" "ca-certificates" "tar")
    if [[ "$INSTALL_MODE" == "2" ]]; then
        deps+=("docker.io" "docker-compose-v2")
    fi

    for dep in "${deps[@]}"; do
        if ! command -v "$dep" &>/dev/null; then
            log "Installing ${dep}..."
            apt-get install -y "$dep" -qq >> "$LOG_FILE" 2>&1 || warn "Could not install $dep"
        fi
    done
}

install_native() {
    log "Starting native installation..."

    if ! id -u aegis >/dev/null 2>&1; then
        log "Creating 'aegis' system user..."
        useradd --system --no-create-home --shell /sbin/nologin aegis >> "$LOG_FILE" 2>&1
    fi

    mkdir -p "$CONFIG_DIR" "$DATA_DIR" "$UI_DIST_PATH"
    mkdir -p "$DATA_DIR/logs" "$DATA_DIR/plugins" "$DATA_DIR/users"
    chown -R aegis:aegis "$DATA_DIR"
    chmod -R 750 "$DATA_DIR"

    local release_url="https://github.com/${GITHUB_ORG}/${GITHUB_REPO}/releases/download/${RELEASE_TAG}"
    local bin_file="ank-server-linux-${ARCH}.tar.gz"

    log "Downloading ank-server (${RELEASE_TAG})..."
    curl -L --fail --progress-bar \
        "${release_url}/${bin_file}" -o "/tmp/${bin_file}" \
        || error "Failed to download binary. Check: github.com/${GITHUB_ORG}/${GITHUB_REPO}/releases"

    tar -xzf "/tmp/${bin_file}" -C "/tmp/"
    mv "/tmp/ank-server-linux-${ARCH}" "${BIN_DIR}/ank-server"
    chmod +x "${BIN_DIR}/ank-server"
    rm "/tmp/${bin_file}"
    success "ank-server → ${BIN_DIR}/ank-server"

    log "Downloading UI assets..."
    curl -L --fail --progress-bar \
        "${release_url}/ui-dist.tar.gz" -o "/tmp/ui-dist.tar.gz" \
        || error "Failed to download UI assets."
    tar -xzf "/tmp/ui-dist.tar.gz" -C "$UI_DIST_PATH"
    rm "/tmp/ui-dist.tar.gz"
    chown -R aegis:aegis "$UI_DIST_PATH"
    success "UI assets → ${UI_DIST_PATH}"

    # Install aegis CLI
    local script_dir
    script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    if [[ -f "${script_dir}/aegis" ]]; then
        cp "${script_dir}/aegis" "${BIN_DIR}/aegis"
    else
        curl -L --fail --silent \
            "https://raw.githubusercontent.com/${GITHUB_ORG}/${GITHUB_REPO}/main/installer/aegis" \
            -o "${BIN_DIR}/aegis" >> "$LOG_FILE" 2>&1 || warn "Could not download aegis CLI"
    fi
    chmod +x "${BIN_DIR}/aegis"
    success "aegis CLI → ${BIN_DIR}/aegis"

    # Generate env file
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
        chmod 640 "$ENV_FILE"
        chown aegis:aegis "$ENV_FILE"
        success "Environment file → ${ENV_FILE}"
    else
        warn "Existing ${ENV_FILE} preserved."
        # Ensure required vars are present
        grep -q "UI_DIST_PATH"  "$ENV_FILE" || echo "UI_DIST_PATH=${UI_DIST_PATH}"  >> "$ENV_FILE"
        grep -q "AEGIS_DATA_DIR" "$ENV_FILE" || echo "AEGIS_DATA_DIR=${DATA_DIR}"   >> "$ENV_FILE"
    fi

    echo "native" > "$CONFIG_DIR/mode"

    # Install systemd unit
    cat > /etc/systemd/system/aegis.service <<EOF
[Unit]
Description=Aegis OS — Cognitive Operating System
Documentation=https://github.com/${GITHUB_ORG}/${GITHUB_REPO}
After=network.target

[Service]
Type=simple
User=aegis
Group=aegis
EnvironmentFile=${ENV_FILE}
Environment=RUST_LOG=info
ExecStart=${BIN_DIR}/ank-server
Restart=on-failure
RestartSec=5s
TimeoutStopSec=10s
ReadWritePaths=${DATA_DIR} ${CONFIG_DIR}
StandardOutput=journal+console
StandardError=journal+console
NoNewPrivileges=true
ProtectSystem=full
ProtectHome=true

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload
    systemctl enable aegis.service >> "$LOG_FILE" 2>&1
    systemctl start  aegis.service >> "$LOG_FILE" 2>&1
    success "systemd service enabled and started."
}

install_docker() {
    log "Starting Docker installation..."
    mkdir -p /opt/aegis

    curl -L --fail --silent \
        "https://raw.githubusercontent.com/${GITHUB_ORG}/${GITHUB_REPO}/main/installer/docker-compose.yml" \
        -o "/opt/aegis/docker-compose.yml" >> "$LOG_FILE" 2>&1 \
        || error "Failed to download docker-compose.yml"

    if [[ ! -f "/opt/aegis/.env" ]]; then
        local root_key
        root_key=$(openssl rand -hex 32)
        cat > "/opt/aegis/.env" <<EOF
AEGIS_ROOT_KEY=${root_key}
AEGIS_MTLS_STRICT=false
EOF
        chmod 600 "/opt/aegis/.env"
        success "Docker .env → /opt/aegis/.env"
    else
        warn "Existing /opt/aegis/.env preserved."
    fi

    echo "docker" > "$CONFIG_DIR/mode"
    mkdir -p "$CONFIG_DIR"

    # Install aegis CLI
    local script_dir
    script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    if [[ -f "${script_dir}/aegis" ]]; then
        cp "${script_dir}/aegis" "${BIN_DIR}/aegis"
        chmod +x "${BIN_DIR}/aegis"
    fi

    log "Pulling image and starting containers..."
    cd /opt/aegis && docker compose up -d >> "$LOG_FILE" 2>&1 \
        || error "Docker Compose failed. Check ${LOG_FILE}"

    success "Docker installation complete."
}

wait_and_show() {
    log "Waiting for Aegis to start (max 60s)..."
    local attempts=0
    while [[ $attempts -lt 30 ]]; do
        if curl -s "http://localhost:8000/health" 2>/dev/null | grep -q "Online"; then
            break
        fi
        sleep 2
        attempts=$((attempts + 1))
    done

    local ip
    ip=$(hostname -I 2>/dev/null | awk '{print $1}') || ip="localhost"

    echo ""
    echo -e "${GREEN}################################################################${NC}"
    echo -e "${GREEN}#          AEGIS OS — INSTALLATION COMPLETE                    #${NC}"
    echo -e "${GREEN}################################################################${NC}"
    echo ""

    if curl -s "http://localhost:8000/health" 2>/dev/null | grep -q "Online"; then
        success "Aegis is UP at http://${ip}:8000"

        # FIX: extraer token directamente del log del proceso, sin journalctl
        # El proceso escribe el token en stdout → journal+console → visible en el log
        local token=""
        local attempts_token=0
        while [[ -z "$token" && $attempts_token -lt 10 ]]; do
            sleep 1
            token=$(journalctl -u aegis -n 200 --no-pager 2>/dev/null \
                | grep -oP '(?<=setup_token=)\S+' | tail -1 || true)
            # Fallback: leer desde el log file si journalctl no tiene permisos
            if [[ -z "$token" ]]; then
                token=$(find "$DATA_DIR/logs" -name "ank.log*" 2>/dev/null \
                    | xargs grep -h "setup_token=" 2>/dev/null \
                    | grep -oP '(?<=setup_token=)\S+' | tail -1 || true)
            fi
            attempts_token=$((attempts_token + 1))
        done

        if [[ -n "$token" ]]; then
            echo -e "  ${CYAN}First-time setup URL:${NC}"
            echo -e "  ${GREEN}http://${ip}:8000?setup_token=${token}${NC}"
            echo ""
            echo -e "  Token expires in 30 minutes."
            echo -e "  To regenerate: ${CYAN}sudo aegis token${NC}"
        else
            echo -e "  Access URL: ${CYAN}http://${ip}:8000${NC}"
            echo -e "  Run ${CYAN}sudo aegis token${NC} to get the first-time setup URL."
        fi
    else
        warn "Service started but health check timed out."
        echo -e "  Check logs: ${CYAN}sudo journalctl -u aegis -n 50${NC}"
    fi

    echo -e "${GREEN}################################################################${NC}"
    echo ""
    echo -e "  Log file: ${LOG_FILE}"
}

# ── Main ──────────────────────────────────────────────────────────────────────
check_root
detect_arch
show_menu
install_dependencies

if [[ "$INSTALL_MODE" == "1" ]]; then
    install_native
else
    install_docker
fi

wait_and_show
