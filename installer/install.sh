#!/usr/bin/env bash
# ==============================================================================
# AEGIS OS — UNIFIED INSTALLER (SRE GRADE)
# ==============================================================================
# OS: Ubuntu / Debian
# Description: Professional bootstrapper for Aegis OS Unified Binary
# Tickets: CORE-040, CORE-090 (UX Enhancement)
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

# --- Colors & Aesthetics ---
RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
NC='\033[0m'

USE_TUI=true
INSTALL_MODE="1" # 1: Native, 2: Docker
ARCH="x86_64"

# --- Helper Functions ---
log()     { echo -e "[INFO] $(date '+%H:%M:%S') - $1" >> "$LOG_FILE"; echo -e "${CYAN}  ->${NC} $1"; }
success() { echo -e "[OK]   $(date '+%H:%M:%S') - $1" >> "$LOG_FILE"; echo -e "${GREEN}  [OK]${NC} $1"; }
warn()    { echo -e "[WARN] $(date '+%H:%M:%S') - $1" >> "$LOG_FILE"; echo -e "${YELLOW}  [!]${NC} $1"; }
error()   { echo -e "[ERROR] $(date '+%H:%M:%S') - $1" >> "$LOG_FILE"; echo -e "${RED}[ERROR]${NC} $1" >&2; exit 1; }

print_banner() {
    clear
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
    echo -e "      Aegis OS Professional Bootstrapper — v2.0"
    echo -e "------------------------------------------------------------"
}

check_root() {
    if [[ "$EUID" -ne 0 ]]; then
        echo -e "${RED}[ERROR]${NC} This script must be run as root (use sudo)." >&2
        exit 1
    fi
    touch "$LOG_FILE"
    chmod 600 "$LOG_FILE"
}

detect_arch() {
    local arch_name
    arch_name=$(uname -m)
    case "$arch_name" in
        x86_64)  ARCH="x86_64" ;;
        aarch64) ARCH="arm64" ;;
        *)       error "Unsupported architecture: $arch_name" ;;
    esac
}

# --- System Audit ---
run_system_audit() {
    log "Performing System Audit..."
    
    local cpu_cores ram_gb docker_status nvidia_status="Not Detected"
    cpu_cores=$(nproc)
    ram_gb=$(free -g | awk '/^Mem:/{print $2}')
    docker_status=$(command -v docker &> /dev/null && echo "Installed" || echo "Missing")
    
    if command -v nvidia-smi &> /dev/null && nvidia-smi &> /dev/null; then
        nvidia_status=$(nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null | head -n 1)
    fi

    if [ "$USE_TUI" = true ]; then
        if ! command -v dialog &> /dev/null; then
            apt-get update -qq && apt-get install -y dialog -qq >> "$LOG_FILE" 2>&1
        fi
        
        local report="System Audit Results:\n\n"
        report+="CPU Cores:    $cpu_cores\n"
        report+="Total RAM:    ${ram_gb}GB\n"
        report+="Docker:       $docker_status\n"
        report+="GPU:          $nvidia_status\n\n"
        
        if [ "$ram_gb" -lt 2 ]; then
            report+="[WARNING] Low RAM detected. Installation might be unstable."
        fi

        dialog --title "Aegis System Audit" --msgbox "$report" 15 60
    else
        echo "CPU Cores: $cpu_cores | RAM: ${ram_gb}GB | Docker: $docker_status | GPU: $nvidia_status"
    fi

    # Port Conflict Check
    for port in 8000 50051; do
        if ss -tulpn 2>/dev/null | grep -q ":$port "; then
            warn "Port $port is already in use. This may conflict with Aegis."
        fi
    done
}

# --- UI Menus ---
show_main_menu() {
    if [ ! -t 0 ]; then
        USE_TUI=false
        log "Non-interactive shell detected. Defaulting to Native mode."
        return
    fi

    INSTALL_MODE=$(dialog --clear \
        --title "AEGIS OS — INSTALLATION MODE" \
        --menu "Select your preferred deployment method:" 15 60 2 \
        "1" "Native (Recommended) - Single binary via Systemd" \
        "2" "Docker - Isolated Container" \
        3>&1 1>&2 2>&3) || INSTALL_MODE="1"
}

# --- Installation Steps ---
install_dependencies() {
    log "Synchronizing dependencies..."
    apt-get update -qq >> "$LOG_FILE" 2>&1
    local deps=("curl" "openssl" "ca-certificates" "tar" "git")
    
    if [[ "$INSTALL_MODE" == "2" ]]; then
        deps+=("docker.io" "docker-compose-v2")
    fi

    for dep in "${deps[@]}"; do
        if ! command -v "$dep" &>/dev/null; then
            apt-get install -y "$dep" -qq >> "$LOG_FILE" 2>&1 || warn "Could not install $dep"
        fi
    done
}

install_native() {
    log "Starting native installation..."

    if ! id -u aegis >/dev/null 2>&1; then
        useradd --system --no-create-home --shell /sbin/nologin aegis >> "$LOG_FILE" 2>&1
    fi

    mkdir -p "$CONFIG_DIR" "$DATA_DIR" "$UI_DIST_PATH"
    mkdir -p "$DATA_DIR/logs" "$DATA_DIR/plugins" "$DATA_DIR/users"
    
    # Recursive chown to ensure all subdirs are writable by the aegis service user
    chown -R aegis:aegis "$DATA_DIR" "$CONFIG_DIR" "$UI_DIST_PATH"
    chmod -R 750 "$DATA_DIR"

    local release_url="https://github.com/${GITHUB_ORG}/${GITHUB_REPO}/releases/download/${RELEASE_TAG}"
    local bin_file="ank-server-linux-${ARCH}.tar.gz"
    
    log "Downloading ank-server binary (${RELEASE_TAG})..."
    curl -L --fail --progress-bar "${release_url}/${bin_file}" -o "/tmp/${bin_file}" || error "Download failed"
    
    tar -xzf "/tmp/${bin_file}" -C "/tmp/"
    mv "/tmp/ank-server-linux-${ARCH}" "${BIN_DIR}/ank-server"
    chmod +x "${BIN_DIR}/ank-server"
    rm "/tmp/${bin_file}"
    
    success "Binary installed to ${BIN_DIR}/ank-server"

    log "Downloading UI assets..."
    curl -L --fail --progress-bar "${release_url}/ui-dist.tar.gz" -o "/tmp/ui-dist.tar.gz" || error "UI download failed"
    tar -xzf "/tmp/ui-dist.tar.gz" -C "$UI_DIST_PATH"
    rm "/tmp/ui-dist.tar.gz"
    success "UI assets ready at ${UI_DIST_PATH}"

    # Generate environment and systemd
    if [[ ! -f "$ENV_FILE" ]]; then
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
    fi

    cat > /etc/systemd/system/aegis.service <<EOF
[Unit]
Description=Aegis OS — Cognitive Operating System
After=network.target

[Service]
Type=simple
User=aegis
Group=aegis
EnvironmentFile=${ENV_FILE}
Environment=RUST_LOG=info
ExecStart=${BIN_DIR}/ank-server
Restart=always
RestartSec=5s
ReadWritePaths=${DATA_DIR} ${CONFIG_DIR}
StandardOutput=journal+console
StandardError=journal+console
# Hardening
NoNewPrivileges=true
ProtectSystem=full
ProtectHome=true

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload
    systemctl enable aegis.service
    systemctl start aegis.service
    
    # CLI
    cp "$(dirname "$0")/aegis" "${BIN_DIR}/aegis" 2>/dev/null || \
    curl -L --fail --silent "https://raw.githubusercontent.com/${GITHUB_ORG}/${GITHUB_REPO}/main/installer/aegis" -o "${BIN_DIR}/aegis"
    chmod +x "${BIN_DIR}/aegis"
    
    success "Systemd service and CLI installed."
}

# --- Main Flow ---
check_root
detect_arch
print_banner
run_system_audit
show_main_menu
install_dependencies

if [[ "$INSTALL_MODE" == "1" ]]; then
    install_native
else
    # Docker mode omitted for brevity in this example but follows same logic
    error "Docker mode implementation pending refinement."
fi

# Success Feedback
clear
print_banner
ip=$(hostname -I | awk '{print $1}')
echo -e "${GREEN}################################################################${NC}"
echo -e "${GREEN}#          AEGIS OS — INSTALLATION COMPLETE                    #${NC}"
echo -e "${GREEN}################################################################${NC}"
echo -e ""
echo -e "  Access URL: ${CYAN}http://${ip}:8000${NC}"
echo -e "  To get setup token: ${YELLOW}sudo aegis token${NC}"
echo -e ""
echo -e "${GREEN}################################################################${NC}"
