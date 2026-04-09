#!/usr/bin/env bash
# ============================================================
# setup-service.sh — Instala Aegis como servicio systemd
# ============================================================

set -euo pipefail

# Configuración
SERVICE_NAME="aegis"
INSTALL_DIR="/opt/aegis"
BIN_DIR="/usr/local/bin"
CONFIG_DIR="/etc/aegis"
DATA_DIR="/var/lib/aegis"
UI_DIST_PATH="/usr/share/aegis/ui"

if [[ "$EUID" -ne 0 ]]; then
    echo "Error: Este script debe ejecutarse como root (sudo)." >&2
    exit 1
fi

echo "→ Configurando Aegis OS en modo nativo..."

# 1. Crear usuario del sistema
if ! id -u "$SERVICE_NAME" >/dev/null 2>&1; then
    echo "→ Creando usuario $SERVICE_NAME..."
    useradd --system --no-create-home --shell /sbin/nologin "$SERVICE_NAME"
fi

# 2. Crear directorios
echo "→ Creando directorios..."
mkdir -p "$CONFIG_DIR" "$DATA_DIR" "$UI_DIST_PATH"
chown "$SERVICE_NAME:$SERVICE_NAME" "$DATA_DIR"

# 3. Generar archivo de entorno si no existe
ENV_FILE="$CONFIG_DIR/aegis.env"
if [[ ! -f "$ENV_FILE" ]]; then
    echo "→ Generando archivo de configuración inicial en $ENV_FILE..."
    ROOT_KEY=$(openssl rand -hex 32)
    cat > "$ENV_FILE" <<EOF
AEGIS_ROOT_KEY=${ROOT_KEY}
AEGIS_DATA_DIR=${DATA_DIR}
AEGIS_MTLS_STRICT=false
UI_DIST_PATH=${UI_DIST_PATH}
EOF
    chmod 600 "$ENV_FILE"
    chown "$SERVICE_NAME:$SERVICE_NAME" "$ENV_FILE"
fi

# 4. Instalar Unit file
echo "→ Instalando unit systemd..."
cp aegis.service /etc/systemd/system/
systemctl daemon-reload
systemctl enable "$SERVICE_NAME"

echo "→ Instalación del servicio completada."
echo "  Usa 'systemctl start $SERVICE_NAME' para iniciar el sistema."
echo "  Logs: journalctl -u $SERVICE_NAME -f"
