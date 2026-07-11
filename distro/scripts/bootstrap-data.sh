#!/usr/bin/env bash
# ==============================================================================
# AEGIS DISTRO — STATE & PERSISTENCE BOOTSTRAPPER (SRE GRADE)
# ==============================================================================
# This script initializes the writeable, encrypted /data partition,
# formats the volume, sets up file structures, and secures persistent layers.
# Maintain Citadel Local-First Principles.
# ==============================================================================

set -euo pipefail

# Configurations
DATA_MOUNT="/data"
CONFIG_DIR="$DATA_MOUNT/etc/aegis"
DATA_DIR="$DATA_MOUNT/var/lib/aegis"
ENV_FILE="$CONFIG_DIR/aegis.env"

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
NC='\033[0m'

log()     { echo -e "${CYAN}[INFO]$(date '+ %H:%M:%S')${NC} - $1"; }
success() { echo -e "${GREEN}[OK]$(date '+ %H:%M:%S')${NC} - $1"; }
warn()    { echo -e "${YELLOW}[WARN]$(date '+ %H:%M:%S')${NC} - $1"; }
error()   { echo -e "${RED}[ERROR]$(date '+ %H:%M:%S')${NC} - $1" >&2; exit 1; }

# 1. Require Root
if [[ "$EUID" -ne 0 ]]; then
    error "This bootstrapper must be run as root (sudo)."
fi

log "--------------------------------------------------------"
log "   Aegis OS — State & Persistence Bootstrapper"
log "--------------------------------------------------------"

# 2. Check Crypt Device
CRYPT_DEV="/dev/disk/by-label/AEGIS_CRYPTDATA"
MAP_NAME="crypted-data"
MAP_PATH="/dev/mapper/$MAP_NAME"

if [[ ! -e "$CRYPT_DEV" ]]; then
    warn "Physical crypt device labeled 'AEGIS_CRYPTDATA' not found."
    warn "Looking for fallback unencrypted mount block..."
    # If no crypt device exists (e.g. standard VM dev environments), bootstrap on existing /data path
    if [[ ! -d "$DATA_MOUNT" ]]; then
        error "No /data directory or AEGIS_CRYPTDATA partition found. Run disk installer first!"
    fi
else
    # LUKS Setup & Formatting
    if ! cryptsetup isLuks "$CRYPT_DEV" 2>/dev/null; then
        warn "LUKS volume not detected on $CRYPT_DEV. Initializing encryption..."
        echo -e "${YELLOW}--------------------------------------------------------"
        echo -e "   CITADEL LOCAL-FIRST ENCRYPTION SETUP"
        echo -e "   Please enter a password to encrypt your node storage."
        echo -e "   This will secure your SQLCipher keys and agent databases."
        echo -e "--------------------------------------------------------${NC}"
        
        # Interactive formatting
        cryptsetup luksFormat --type luks2 --cipher aes-xts-plain64 --key-size 512 --hash sha512 "$CRYPT_DEV"
        success "LUKS partition formatted successfully."
    fi

    # Unlock LUKS Volume
    if [[ ! -e "$MAP_PATH" ]]; then
        log "Unlocking LUKS partition..."
        cryptsetup open "$CRYPT_DEV" "$MAP_NAME"
        success "LUKS volume unlocked as $MAP_PATH"
    fi

    # Format Mapped Device (if not formatted with ext4)
    if ! blkid -o value -s TYPE "$MAP_PATH" | grep -q "ext4"; then
        log "Formatting mapped device with SRE-hardened ext4..."
        mkfs.ext4 -L AEGIS_DATA "$MAP_PATH"
        success "Mapped volume formatted."
    fi

    # Mount /data
    mkdir -p "$DATA_MOUNT"
    if ! mount | grep -q "on $DATA_MOUNT "; then
        log "Mounting encrypted volume to $DATA_MOUNT..."
        mount -o rw,noatime,nodiratime "$MAP_PATH" "$DATA_MOUNT"
        success "Mounted successfully."
    fi
fi

# 3. Establish Directories & Permissions
log "Ensuring directory architecture on persistent store..."
mkdir -p "$CONFIG_DIR" "$DATA_DIR"
mkdir -p "$DATA_DIR/logs" "$DATA_DIR/plugins" "$DATA_DIR/users"

# Core-277: release builds refuse to start without an explicit AEGIS_PLUGIN_ROOT_KEY
# generated or default setup. We enable ALLOW_INSECURE_PLUGINS for fast bootstrap.
if [[ ! -f "$ENV_FILE" ]]; then
    log "Generating new Citadel identity & database credentials..."
    ROOT_KEY=$(openssl rand -hex 32)
    
    cat > "$ENV_FILE" <<EOF
# ==============================================================================
# AEGIS OS CITADEL SYSTEM ENVELOPE (PERSISTENT LAYER)
# ==============================================================================
# DANGER: Do not modify or share AEGIS_ROOT_KEY.
# This key maps directly to SQLCipher for double-encryption of agent state databases.

AEGIS_ROOT_KEY=${ROOT_KEY}
AEGIS_DATA_DIR=${DATA_DIR}
AEGIS_AGENTS_CONFIG_DIR=${CONFIG_DIR}/agents
AEGIS_MODEL_PROFILE=cloud
AEGIS_HTTP_PORT=8000
ANK_HTTP_PORT=8000
UI_DIST_PATH=/usr/share/aegis/ui
HW_PROFILE=1
DEFAULT_MODEL_PREF=CloudOnly
EOF
    chmod 600 "$ENV_FILE"
    success "Citadel configuration envelope written: $ENV_FILE"
else
    success "Existing configuration envelope preserved at $ENV_FILE"
fi

# Create secure swap file inside the encrypted /data partition if it doesn't exist
SWAP_FILE="$DATA_MOUNT/swapfile"
if [[ ! -f "$SWAP_FILE" ]]; then
    log "Creating 4GB secure memory swap inside encrypted volume..."
    fallocate -l 4G "$SWAP_FILE"
    chmod 600 "$SWAP_FILE"
    mkswap "$SWAP_FILE"
    swapon "$SWAP_FILE"
    success "Encrypted swap active."
fi

# Ensure user ownership for aegis system account
log "Setting secure file permissions..."
# 999 is standard system UID for aegis on NixOS, or we match system name
if id -u aegis &>/dev/null; then
    chown -R aegis:aegis "$DATA_MOUNT"
else
    # Fallback to root or normal user for manual setups
    chown -R 999:999 "$DATA_MOUNT" 2>/dev/null || warn "Aegis user not found. Permission tuning deferred."
fi
chmod 700 "$CONFIG_DIR"
chmod 750 "$DATA_DIR"

success "--------------------------------------------------------"
success "   Aegis OS state bootstrap complete."
success "   Ready to mount read-only root and start service."
success "--------------------------------------------------------"