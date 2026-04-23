#!/usr/bin/env bash
# ==============================================================================
# AEGIS OS — UNIFIED INSTALLER (SRE GRADE)
# ==============================================================================
# OS: Ubuntu / Debian
# Tickets: CORE-040, CORE-122
# ==============================================================================

set -euo pipefail

# --- Configuration ---
CONFIG_DIR="/etc/aegis"
BIN_DIR="/usr/local/bin"
DATA_DIR="/var/lib/aegis"
UI_DIST_PATH="/usr/share/aegis/ui"
ENV_FILE="$CONFIG_DIR/aegis.env"
LOG_FILE="/root/aegis_install.log"
INSTALL_ROOT="/opt/aegis"

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
INFERENCE_PROFILE="cloud"

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

# --- System Audit ---
run_system_audit() {
    log "Performing System Audit..."

    local cpu_cores ram_gb docker_status nvidia_status="Not Detected"
    cpu_cores=$(nproc)
    ram_gb=$(free -g | awk '/^Mem:/{print $2}')
    docker_status=$(command -v docker > /dev/null 2>&1 && echo "Installed" || echo "Missing")

    if command -v nvidia-smi > /dev/null 2>&1 && nvidia-smi > /dev/null 2>&1; then
        nvidia_status=$(nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null | head -n 1)
    fi

    if [ "${USE_TUI:-false}" = true ]; then
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
    # RECONEXIÓN A TTY: Si estamos en un pipe (curl | bash), reconectamos stdin para ser interactivos
    if [ ! -t 0 ]; then
        log "Non-interactive shell detected. Defaulting to Native mode."
        return
    fi

    print_banner
    echo -e "${YELLOW}--- CONFIGURACIÓN DE DESPLIEGUE ---${NC}"
    echo "Seleccione el modo de instalación:"
    echo "  [1] Nativo (Directo en el Host, máximo rendimiento) [DEFAULT]"
    echo "  [2] Docker (Aisla dependencias, más limpio)"
    read -rp "Selección [1-2]: " mode_choice
    case "${mode_choice:-1}" in
        2) INSTALL_MODE="2" ;;
        *) INSTALL_MODE="1" ;;
    esac

    echo ""
    echo -e "${YELLOW}--- PREFERENCIA COGNITIVA (IA) ---${NC}"
    echo "Aegis necesita saber cómo procesar instrucciones:"
    echo "  [1] Cloud Only     (Usa OpenRouter/OpenAI, requiere internet, muy inteligente) [DEFAULT]"
    echo "  [2] Local Only     (Usa Ollama/Llama.cpp local, privacidad total, requiere GPU/RAM)"
    echo "  [3] Hybrid Smart   (Aegis decide: local para tareas simples, nube para complejas)"
    read -rp "Selección [1-3]: " ia_choice
    case "${ia_choice:-1}" in
        2) export AEGIS_INIT_PREF="LocalOnly" ;;
        3) export AEGIS_INIT_PREF="HybridSmart" ;;
        *) export AEGIS_INIT_PREF="CloudOnly" ;;
    esac

    echo ""
    echo -e "${YELLOW}--- PERFIL DE HARDWARE ---${NC}"
    echo "Defina la potencia de este nodo:"
    echo "  [1] Tier 1 (Low End / Laptop / VPS) [DEFAULT]"
    echo "  [2] Tier 2 (Workstation / RTX 3060+)"
    echo "  [3] Tier 3 (SRE Grade / A100 / H100 Cluster)"
    read -rp "Selección [1-3]: " hw_choice
    export HW_PROFILE="${hw_choice:-1}"

}

show_inference_profile_menu() {
    if [ ! -t 0 ]; then
        log "Non-interactive shell. Defaulting to inference profile: cloud."
        INFERENCE_PROFILE="cloud"
        return
    fi

    echo ""
    echo "┌─────────────────────────────────────────────────────────────┐"
    echo "│              AEGIS OS — INFERENCE PROFILE                   │"
    echo "├─────────────────────────────────────────────────────────────┤"
    echo "│  [1] Cloud only  — API keys (OpenRouter, OpenAI, etc.)      │"
    echo "│      Recommended for VPS/servers without local GPU          │"
    echo "│                                                             │"
    echo "│  [2] Local only  — Ollama / local models (no API keys)     │"
    echo "│      Recommended for air-gapped or GPU-equipped machines    │"
    echo "│                                                             │"
    echo "│  [3] Hybrid      — Cloud + local fallback                  │"
    echo "│      Best of both worlds if you have GPU + API keys        │"
    echo "└─────────────────────────────────────────────────────────────┘"
    echo ""
    read -rp "Selection [1-3, default 1]: " choice
    case "${choice:-1}" in
        2) INFERENCE_PROFILE="local" ;;
        3) INFERENCE_PROFILE="hybrid" ;;
        *) INFERENCE_PROFILE="cloud" ;;
    esac
    log "Inference profile: ${INFERENCE_PROFILE}"
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

install_cloudflared() {
    if command -v cloudflared &>/dev/null; then
        log "cloudflared ya instalado — omitiendo"
        return
    fi
    log "Instalando cloudflared (tunnel remoto)..."
    local arch_str="amd64"
    [[ "$(uname -m)" == "aarch64" ]] && arch_str="arm64"
    local url="https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-${arch_str}"
    
    # Limpia instalaciones previas rotas
    rm -f /usr/bin/cloudflared /usr/local/bin/cloudflared
    
    # Intenta instalar en /usr/bin que es el PATH estándar para servicios
    if curl -L --fail --silent "$url" -o /usr/bin/cloudflared && chmod 755 /usr/bin/cloudflared; then
        success "cloudflared instalado en /usr/bin"
    elif curl -L --fail --silent "$url" -o /usr/local/bin/cloudflared && chmod 755 /usr/local/bin/cloudflared; then
        success "cloudflared instalado en /usr/local/bin"
    else
        warn "No se pudo instalar cloudflared — acceso remoto deshabilitado"
    fi
}

install_native() {
    log "Starting native installation..."

    install_cloudflared

    if ! id -u aegis >/dev/null 2>&1; then
        log "Creating 'aegis' system user..."
        useradd --system --no-create-home --shell /sbin/nologin aegis >> "$LOG_FILE" 2>&1
    fi

    mkdir -p "$CONFIG_DIR" "$DATA_DIR" "$UI_DIST_PATH"
    mkdir -p "$DATA_DIR/logs" "$DATA_DIR/plugins" "$DATA_DIR/users"
    chown -R aegis:aegis "$DATA_DIR"
    chmod -R 750 "$DATA_DIR"

    local release_url
    release_url="https://github.com/${GITHUB_ORG}/${GITHUB_REPO}/releases/download/${RELEASE_TAG}"
    local bin_file
    bin_file="ank-server-linux-${ARCH}.tar.gz"

    log "Downloading ank-server binary (${RELEASE_TAG})..."
    curl -L --fail --progress-bar "${release_url}/${bin_file}" -o "/tmp/${bin_file}" || error "Download failed"

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
    log "Installing aegis CLI..."
    local script_dir
    script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    if [[ -f "${script_dir}/aegis" ]]; then
        cp "${script_dir}/aegis" "${BIN_DIR}/aegis"
    else
        curl -L --fail --silent \
            "https://raw.githubusercontent.com/${GITHUB_ORG}/${GITHUB_REPO}/main/installer/aegis" \
            -o "${BIN_DIR}/aegis" >> "$LOG_FILE" 2>&1 \
            || warn "Could not download aegis CLI"
    fi
    chmod +x "${BIN_DIR}/aegis"
    success "aegis CLI installed → ${BIN_DIR}/aegis"

    # Generate environment file
    if [[ ! -f "$ENV_FILE" ]]; then
        log "Generating AEGIS_ROOT_KEY..."
        local root_key
        root_key=$(openssl rand -hex 32)
        cat > "$ENV_FILE" <<EOF
AEGIS_ROOT_KEY=${root_key}
AEGIS_DATA_DIR=${DATA_DIR}
AEGIS_MTLS_STRICT=false
AEGIS_MODEL_PROFILE=${INFERENCE_PROFILE}
UI_DIST_PATH=${UI_DIST_PATH}
HW_PROFILE=${HW_PROFILE:-1}
DEFAULT_MODEL_PREF=${AEGIS_INIT_PREF:-CloudOnly}
EOF
        chmod 640 "$ENV_FILE"
        chown aegis:aegis "$ENV_FILE"
        success "Environment file → ${ENV_FILE}"
        success "Inference profile: ${INFERENCE_PROFILE}"
    else
        warn "Existing ${ENV_FILE} preserved."
        # Asegurar que las vars requeridas están presentes
        grep -q "UI_DIST_PATH"          "$ENV_FILE" || echo "UI_DIST_PATH=${UI_DIST_PATH}"               >> "$ENV_FILE"
        grep -q "AEGIS_DATA_DIR"        "$ENV_FILE" || echo "AEGIS_DATA_DIR=${DATA_DIR}"                 >> "$ENV_FILE"
        grep -q "AEGIS_MODEL_PROFILE"   "$ENV_FILE" || echo "AEGIS_MODEL_PROFILE=${INFERENCE_PROFILE}"   >> "$ENV_FILE"
    fi

    # Write mode file
    echo "native" > "$CONFIG_DIR/mode"

    # Install systemd service
    log "Installing systemd service..."
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

    {
        systemctl daemon-reload
        systemctl enable aegis.service
        systemctl start aegis.service
    } >> "$LOG_FILE" 2>&1
    success "Systemd service and CLI installed."
}

install_docker() {
    log "Starting Docker installation..."
    mkdir -p "$CONFIG_DIR"
    mkdir -p "$INSTALL_ROOT"

    local raw_url="https://raw.githubusercontent.com/${GITHUB_ORG}/${GITHUB_REPO}/main/installer/docker-compose.yml"
    log "Fetching docker-compose.yml..."
    curl -L --fail --silent \
        "$raw_url" \
        -o "${INSTALL_ROOT}/docker-compose.yml" >> "$LOG_FILE" 2>&1 \
        || error "Failed to download docker-compose.yml"

    if [[ ! -f "${INSTALL_ROOT}/.env" ]]; then
        local root_key
        root_key=$(openssl rand -hex 32)
        cat > "${INSTALL_ROOT}/.env" <<EOF
AEGIS_ROOT_KEY=${root_key}
AEGIS_MTLS_STRICT=false
AEGIS_MODEL_PROFILE=${INFERENCE_PROFILE}
HW_PROFILE=${HW_PROFILE:-1}
DEFAULT_MODEL_PREF=${AEGIS_INIT_PREF:-CloudOnly}
EOF
        chmod 600 "${INSTALL_ROOT}/.env"
        success "Docker .env created → ${INSTALL_ROOT}/.env"
        success "Inference profile: ${INFERENCE_PROFILE}"
    else
        warn ".env already exists — preserving existing keys."
        grep -q "AEGIS_MODEL_PROFILE" "${INSTALL_ROOT}/.env" \
            || echo "AEGIS_MODEL_PROFILE=${INFERENCE_PROFILE}" >> "${INSTALL_ROOT}/.env"
    fi



    mkdir -p "$CONFIG_DIR"
    echo "docker" > "$CONFIG_DIR/mode"

    local script_dir
    script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    if [[ -f "${script_dir}/aegis" ]]; then
        cp "${script_dir}/aegis" "${BIN_DIR}/aegis"
        chmod +x "${BIN_DIR}/aegis"
    fi

    log "Pulling image and starting containers..."
    { cd "$INSTALL_ROOT" && docker compose up -d; } >> "$LOG_FILE" 2>&1 \
        || error "Docker Compose failed. Check ${LOG_FILE} for details."

    success "Docker installation complete."
}

wait_and_show() {
    local PROTOCOL="http"
    local CURL_FLAGS=("-s")

    log "Waiting for Aegis to initialize (max 60s)..."
    local attempts=0
    while [[ $attempts -lt 30 ]]; do
        if curl "${CURL_FLAGS[@]}" "${PROTOCOL}://localhost:8000/health" 2>/dev/null | grep -q "Online"; then
            break
        fi
        sleep 2
        attempts=$((attempts + 1))
    done

    if curl "${CURL_FLAGS[@]}" "${PROTOCOL}://localhost:8000/health" 2>/dev/null | grep -q "Online"; then
        success "Aegis is UP at ${PROTOCOL}://localhost:8000"

        local token=""
        if [[ "${INSTALL_MODE}" == "1" ]]; then
            token=$(journalctl -u aegis --since "5 min ago" --no-pager 2>/dev/null \
                | grep -oP '(?<=setup_token=)\S+' | tail -n 1 || true)
        else
            token=$(docker compose -f "${INSTALL_ROOT}/docker-compose.yml" logs ank-server 2>&1 \
                | grep -oP '(?<=setup_token=)\S+' | tail -n 1 || true)
        fi

        local tunnel_url=""
        local conn_info
        conn_info=$(curl -s "${PROTOCOL}://localhost:8000/api/system/connection-info" || true)
        if [[ -n "$conn_info" ]]; then
            tunnel_url=$(echo "$conn_info" | grep -oP '(?<="tunnel_url":")[^"]+')
        fi

        echo ""
        echo -e "${GREEN}################################################################${NC}"
        echo -e "${GREEN}#          AEGIS OS — INSTALLATION COMPLETE                    #${NC}"
        echo -e "${GREEN}################################################################${NC}"
        echo ""
        local ip
        ip=$(hostname -I 2>/dev/null | awk '{print $1}') || ip="localhost"

        echo -e "  Inference profile: ${CYAN}${INFERENCE_PROFILE}${NC}"
        echo ""

        if [[ -n "$tunnel_url" ]]; then
            echo -e "${GREEN}  Remote Access (HTTPS):${NC}"
            echo -e "  ${CYAN}${tunnel_url}${NC}"
            echo ""
        fi

        if [[ -n "$token" ]]; then
            echo -e "${CYAN}  Local Setup URL:${NC}"
            echo -e "  ${GREEN}${PROTOCOL}://${ip}:8000?setup_token=${token}${NC}"
            echo ""
            echo -e "  Token expires in 30 minutes."
            echo -e "  To regenerate: ${CYAN}sudo aegis token${NC}"
        else
            echo -e "  Local Access (HTTP): ${CYAN}${PROTOCOL}://${ip}:8000${NC}"
            echo -e "  Run ${CYAN}sudo aegis token${NC} to get the setup URL."
        fi
        
        if [[ -z "$tunnel_url" ]]; then
            echo ""
            echo -e "${YELLOW}  Note: Cloudflare Tunnel is still initializing.${NC}"
            echo -e "  Run ${CYAN}sudo aegis status${NC} in a few minutes to get the HTTPS URL."
        fi
        echo -e "${GREEN}################################################################${NC}"
        echo ""
        echo -e "  Log file: ${LOG_FILE}"
    else
        warn "Health check timed out. Check: journalctl -u aegis -n 50"
    fi
}

# --- Main ---
check_root
detect_arch
show_main_menu
show_inference_profile_menu
install_dependencies

if [[ "$INSTALL_MODE" == "1" ]]; then
    install_native
    wait_and_show
else
    install_docker
    wait_and_show
fi
