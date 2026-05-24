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
SETUP_HTTPS="false"
AEGIS_DOMAIN=""
AEGIS_EMAIL=""
AI_PROVIDER="none"
AI_API_KEY=""
AI_MODEL=""
AEGIS_HTTP_PORT="8000"

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
# Picks the HTTP port for Aegis. If the default (8000) is already taken — common
# on shared servers — prompt for a free one instead of failing to bind.
detect_http_port() {
    local candidate=8000
    if ss -tulpn 2>/dev/null | grep -q ":${candidate} " || ss -tln 2>/dev/null | grep -q ":${candidate} "; then
        warn "Puerto ${candidate} en uso."
        
        # Non-interactive shell check
        if [ ! -t 0 ]; then
            candidate="8080"
            if ss -tulpn 2>/dev/null | grep -q ":${candidate} " || ss -tln 2>/dev/null | grep -q ":${candidate} "; then
                candidate="8090"
                if ss -tulpn 2>/dev/null | grep -q ":${candidate} " || ss -tln 2>/dev/null | grep -q ":${candidate} "; then
                    error "Puertos 8000, 8080 y 8090 ocupados. Liberá uno o instalá interactivo para elegir."
                fi
            fi
            warn "Shell no interactivo → usando puerto alternativo ${candidate}."
            AEGIS_HTTP_PORT="$candidate"
            log "Puerto HTTP Aegis: ${AEGIS_HTTP_PORT}"
            return
        fi

        echo ""
        echo -e "${YELLOW}¿Qué puerto HTTP debe usar Aegis?${NC}"
        local used_ports
        used_ports=$(ss -tulpn 2>/dev/null | grep LISTEN | grep -oP ':\K\d+(?= )' | sort -nu | tr '\n' ' ' || true)
        if [[ -n "$used_ports" ]]; then
            echo "  Puertos en uso detectados: $used_ports"
        fi
        read -rp "Puerto HTTP para Aegis [default: 8080]: " user_port
        candidate="${user_port:-8080}"
        
        # Verify that the candidate is also free
        while ss -tulpn 2>/dev/null | grep -q ":${candidate} " || ss -tln 2>/dev/null | grep -q ":${candidate} "; do
            warn "Puerto ${candidate} también en uso."
            read -rp "Elegí otro puerto: " candidate
        done
    fi
    AEGIS_HTTP_PORT="$candidate"
    log "Puerto HTTP Aegis: ${AEGIS_HTTP_PORT}"
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

    # Detect conflict and assign customizable port
    detect_http_port
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

    echo ""
    echo -e "${YELLOW}--- ACCESO HTTPS (recomendado para voz y acceso remoto) ---${NC}"
    echo "  [1] Sin HTTPS — solo acceso local en http://IP:8000 [DEFAULT]"
    echo "  [2] Con HTTPS — requiere un dominio apuntando a este servidor"
    echo "      (Let's Encrypt automático via Caddy)"
    read -rp "Selección [1-2]: " https_choice
    case "${https_choice:-1}" in
        2) SETUP_HTTPS="true" ;;
        *) SETUP_HTTPS="false" ;;
    esac

    SETUP_CADDY="false"
    if [[ "$SETUP_HTTPS" == "true" ]]; then
        local pub_ip
        pub_ip=$(curl -s ifconfig.me 2>/dev/null || echo 'desconocida')
        echo ""
        echo -e "${YELLOW}Requisitos para HTTPS:${NC}"
        echo "  1. Tu dominio debe apuntar a tu IP pública: ${pub_ip}"
        echo "  2. Puertos 80 y 443 deben estar abiertos en tu router"
        echo ""
        read -rp "¿Cumplís estos requisitos? [s/N]: " ready
        if [[ "$ready" != "s" && "$ready" != "S" ]]; then
            SETUP_HTTPS="false"
            SETUP_CADDY="false"
        else
            read -rp "Dominio (ej: aegis.midominio.com): " AEGIS_DOMAIN
            read -rp "Email para Let's Encrypt (para notificaciones de renovación): " AEGIS_EMAIL
            
            echo ""
            echo -e "${YELLOW}--- REVERSE PROXY ---${NC}"
            echo "¿Este servidor ya tiene un reverse proxy (nginx, Caddy, NPM, Traefik)?"
            echo "  [1] No — Aegis instala y configura Caddy automáticamente"
            echo "  [2] Sí — Solo expongo Aegis en un puerto interno, yo configuro el proxy"
            read -rp "Selección [1-2, default 2]: " proxy_choice
            case "${proxy_choice:-2}" in
                1) SETUP_CADDY="true" ;;
                *) SETUP_CADDY="false" ;;
            esac
        fi
    fi

    echo ""
    echo -e "${YELLOW}--- CONFIGURACIÓN DEL MOTOR DE IA ---${NC}"
    echo -e "Aegis necesita al menos un proveedor de IA para funcionar."
    echo ""
    echo "  [1] OpenRouter  — acceso a 100+ modelos con una sola API key"
    echo "                    (recomendado: claude-3-haiku, gpt-4o-mini)"
    echo "                    Obtené tu key en: https://openrouter.ai/keys"
    echo ""
    echo "  [2] OpenAI      — GPT-4o, GPT-4o-mini"
    echo "                    Obtené tu key en: https://platform.openai.com/api-keys"
    echo ""
    echo "  [3] Anthropic   — Claude 3.5 Haiku, Claude 3 Opus"
    echo "                    Obtené tu key en: https://console.anthropic.com/keys"
    echo ""
    echo "  [4] Configurar después — el sistema arranca pero los agentes"
    echo "                           no van a funcionar hasta que configures"
    echo "                           un proveedor desde la UI"
    echo ""
    read -rp "Selección [1-4]: " provider_choice

    AI_PROVIDER=""
    AI_API_KEY=""
    AI_MODEL=""

    case "${provider_choice:-4}" in
        1)
            AI_PROVIDER="openrouter"
            read -rp "OpenRouter API Key (sk-or-...): " AI_API_KEY
            echo ""
            echo "Modelo recomendado: anthropic/claude-3-haiku (rápido, económico)"
            echo "Otros: openai/gpt-4o-mini, meta-llama/llama-3.1-8b-instruct:free"
            read -rp "Modelo [default: anthropic/claude-3-haiku]: " AI_MODEL
            AI_MODEL="${AI_MODEL:-anthropic/claude-3-haiku}"
            ;;
        2)
            AI_PROVIDER="openai"
            read -rp "OpenAI API Key (sk-...): " AI_API_KEY
            read -rp "Modelo [default: gpt-4o-mini]: " AI_MODEL
            AI_MODEL="${AI_MODEL:-gpt-4o-mini}"
            ;;
        3)
            AI_PROVIDER="anthropic"
            read -rp "Anthropic API Key (sk-ant-...): " AI_API_KEY
            read -rp "Modelo [default: claude-3-5-haiku-20241022]: " AI_MODEL
            AI_MODEL="${AI_MODEL:-claude-3-5-haiku-20241022}"
            ;;
        4)
            warn "Sin proveedor configurado. Los agentes no funcionarán hasta configurar uno en la UI."
            AI_PROVIDER="none"
            ;;
        *)
            warn "Selección inválida. Sin proveedor configurado."
            AI_PROVIDER="none"
            ;;
    esac

    if [[ -n "$AI_API_KEY" ]]; then
        log "Proveedor: ${AI_PROVIDER}, Modelo: ${AI_MODEL}"
    fi

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

install_caddy() {
    if command -v caddy &>/dev/null; then
        log "Caddy ya instalado — omitiendo"
        return
    fi
    log "Instalando Caddy..."
    apt-get install -y debian-keyring debian-archive-keyring apt-transport-https -qq >> "$LOG_FILE" 2>&1
    curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' \
        | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
    curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' \
        | tee /etc/apt/sources.list.d/caddy-stable.list
    apt-get update -qq >> "$LOG_FILE" 2>&1
    apt-get install -y caddy -qq >> "$LOG_FILE" 2>&1
    success "Caddy instalado"
}

configure_caddy() {
    local domain="$1"
    local email="$2"

    cat > /etc/caddy/Caddyfile <<EOF
{
    email ${email}
}

${domain} {
    reverse_proxy localhost:${AEGIS_HTTP_PORT}

    @websockets {
        header Connection *Upgrade*
        header Upgrade websocket
    }
    reverse_proxy @websockets localhost:${AEGIS_HTTP_PORT}
}
EOF

    systemctl enable caddy >> "$LOG_FILE" 2>&1
    systemctl restart caddy >> "$LOG_FILE" 2>&1
    success "Caddy configurado para ${domain}"
    success "HTTPS automático via Let's Encrypt"
}

install_native() {
    log "Starting native installation..."

    if [[ "$SETUP_HTTPS" == "true" && "$SETUP_CADDY" == "true" ]]; then
        install_caddy
    elif [[ "$SETUP_HTTPS" != "true" ]]; then
        install_cloudflared
    fi

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

    log "Downloading agent instruction files..."
    mkdir -p "$CONFIG_DIR/agents"
    if curl -L --fail --progress-bar \
        "${release_url}/agents-config.tar.gz" -o "/tmp/agents-config.tar.gz" 2>/dev/null; then
        tar -xzf "/tmp/agents-config.tar.gz" -C "$CONFIG_DIR/agents"
        rm -f "/tmp/agents-config.tar.gz"
        chown -R aegis:aegis "$CONFIG_DIR/agents"
        success "Agent instruction files → ${CONFIG_DIR}/agents"
    else
        warn "agents-config.tar.gz not in release — using compiled-in fallbacks"
    fi

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
AEGIS_AGENTS_CONFIG_DIR=${CONFIG_DIR}/agents
AEGIS_MODEL_PROFILE=${INFERENCE_PROFILE}
AEGIS_HTTP_PORT=${AEGIS_HTTP_PORT}
ANK_HTTP_PORT=${AEGIS_HTTP_PORT}
UI_DIST_PATH=${UI_DIST_PATH}
HW_PROFILE=${HW_PROFILE:-1}
DEFAULT_MODEL_PREF=${AEGIS_INIT_PREF:-CloudOnly}
# Release builds refuse to start without an explicit AEGIS_PLUGIN_ROOT_KEY
# (hex-encoded ed25519 public key, ≥32 bytes). Until we ship a key-generation
# command, opt into the unsigned-plugin mode so the installer's first boot
# doesn't fail. To harden later: generate a real keypair, set
# AEGIS_PLUGIN_ROOT_KEY=<hex>, and remove the line below.
AEGIS_ALLOW_INSECURE_PLUGINS=1
EOF
        if [[ "$SETUP_HTTPS" == "true" && -n "$AEGIS_DOMAIN" ]]; then
            echo "AEGIS_DOMAIN=${AEGIS_DOMAIN}" >> "$ENV_FILE"
            echo "AEGIS_BASE_URL=https://${AEGIS_DOMAIN}" >> "$ENV_FILE"
        fi
        # CORE-285: Escribir configuración del motor de IA
        if [[ "${AI_PROVIDER:-none}" != "none" && -n "${AI_API_KEY:-}" ]]; then
            cat >> "$ENV_FILE" <<EOF
AEGIS_DEFAULT_PROVIDER=${AI_PROVIDER}
AEGIS_DEFAULT_MODEL=${AI_MODEL}
AEGIS_DEFAULT_API_KEY=${AI_API_KEY}
EOF
            success "Motor de IA configurado: ${AI_PROVIDER} / ${AI_MODEL}"
        fi
        chmod 640 "$ENV_FILE"
        chown aegis:aegis "$ENV_FILE"
        success "Environment file → ${ENV_FILE}"
        success "Inference profile: ${INFERENCE_PROFILE}"
    else
        warn "Existing ${ENV_FILE} preserved."
        # Asegurar que las vars requeridas están presentes
        grep -q "UI_DIST_PATH"              "$ENV_FILE" || echo "UI_DIST_PATH=${UI_DIST_PATH}"                   >> "$ENV_FILE"
        grep -q "AEGIS_DATA_DIR"            "$ENV_FILE" || echo "AEGIS_DATA_DIR=${DATA_DIR}"                     >> "$ENV_FILE"
        grep -q "AEGIS_MODEL_PROFILE"       "$ENV_FILE" || echo "AEGIS_MODEL_PROFILE=${INFERENCE_PROFILE}"       >> "$ENV_FILE"
        grep -q "AEGIS_AGENTS_CONFIG_DIR"   "$ENV_FILE" || echo "AEGIS_AGENTS_CONFIG_DIR=${CONFIG_DIR}/agents"   >> "$ENV_FILE"
        grep -q "AEGIS_HTTP_PORT"           "$ENV_FILE" || echo "AEGIS_HTTP_PORT=${AEGIS_HTTP_PORT}"             >> "$ENV_FILE"
        grep -q "ANK_HTTP_PORT"             "$ENV_FILE" || echo "ANK_HTTP_PORT=${AEGIS_HTTP_PORT}"               >> "$ENV_FILE"
        # Backfill on upgrade: release builds added by PR #277 refuse to start
        # without AEGIS_PLUGIN_ROOT_KEY. Preserve the previous "no signature
        # verification" behaviour (the binary used to silently fall back to a
        # zeroed key, which is no more secure than this flag) so the upgrade
        # doesn't break running deployments.
        grep -q "AEGIS_PLUGIN_ROOT_KEY\|AEGIS_ALLOW_INSECURE_PLUGINS" "$ENV_FILE" \
            || echo "AEGIS_ALLOW_INSECURE_PLUGINS=1"                             >> "$ENV_FILE"
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

    if [[ "$SETUP_HTTPS" == "true" && -n "$AEGIS_DOMAIN" ]]; then
        configure_caddy "$AEGIS_DOMAIN" "$AEGIS_EMAIL"
    fi
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
AEGIS_MODEL_PROFILE=${INFERENCE_PROFILE}
AEGIS_HTTP_PORT=${AEGIS_HTTP_PORT}
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
        if curl "${CURL_FLAGS[@]}" "${PROTOCOL}://localhost:${AEGIS_HTTP_PORT}/health" 2>/dev/null | grep -q "Online"; then
            break
        fi
        sleep 2
        attempts=$((attempts + 1))
    done

    if curl "${CURL_FLAGS[@]}" "${PROTOCOL}://localhost:${AEGIS_HTTP_PORT}/health" 2>/dev/null | grep -q "Online"; then
        success "Aegis is UP at ${PROTOCOL}://localhost:${AEGIS_HTTP_PORT}"

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
        conn_info=$(curl -s "${PROTOCOL}://localhost:${AEGIS_HTTP_PORT}/api/system/connection-info" || true)
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
        if [[ "${AI_PROVIDER:-none}" != "none" ]]; then
            echo -e "  Motor de IA: ${CYAN}${AI_PROVIDER} / ${AI_MODEL}${NC}"
        else
            echo -e "  ${YELLOW}⚠ Motor de IA: No configurado — configurá en Settings → Engine${NC}"
        fi
        echo ""

        if [[ "$SETUP_HTTPS" == "true" && "$SETUP_CADDY" == "true" && -n "$AEGIS_DOMAIN" ]]; then
            echo -e "${GREEN}  Acceso HTTPS:${NC}"
            echo -e "  ${CYAN}https://${AEGIS_DOMAIN}${NC}"
            echo -e "  Certificado: Let's Encrypt (renovación automática)"
            echo ""
        elif [[ "$SETUP_HTTPS" == "true" && "$SETUP_CADDY" == "false" ]]; then
            echo -e "${GREEN}  Aegis corriendo internamente en: http://localhost:${AEGIS_HTTP_PORT}${NC}"
            echo -e "  Para exponerlo públicamente, agregá un Proxy Host en tu reverse proxy:"
            echo -e "    → Forward Hostname: localhost"
            echo -e "    → Forward Port:     ${AEGIS_HTTP_PORT}"
            echo -e "    → Websockets:       ${GREEN}habilitado${NC} (requerido para chat y agentes)"
            echo ""
        elif [[ -n "$tunnel_url" ]]; then
            echo -e "${GREEN}  Remote Access (HTTPS):${NC}"
            echo -e "  ${CYAN}${tunnel_url}${NC}"
            echo ""
        fi

        if [[ -n "$token" ]]; then
            echo -e "${CYAN}  Local Setup URL:${NC}"
            echo -e "  ${GREEN}${PROTOCOL}://${ip}:${AEGIS_HTTP_PORT}?setup_token=${token}${NC}"
            echo ""
            echo -e "  Token expires in 30 minutes."
            echo -e "  To regenerate: ${CYAN}sudo aegis token${NC}"
        else
            echo -e "  Local Access (HTTP): ${CYAN}${PROTOCOL}://${ip}:${AEGIS_HTTP_PORT}${NC}"
            echo -e "  Run ${CYAN}sudo aegis token${NC} to get the setup URL."
        fi

        if [[ "$SETUP_HTTPS" != "true" && -z "$tunnel_url" ]]; then
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
run_system_audit
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
