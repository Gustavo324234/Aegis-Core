#!/usr/bin/env bash
# ==============================================================================
# AEGIS DISTRO — BARE METAL INSTALLER (SRE GRADE)
# ==============================================================================
# Automates disk partitioning, cryptographic setup, NixOS bootstrapping,
# and system service mapping for installing Aegis OS.
# ==============================================================================

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
NC='\033[0m'

log()     { echo -e "${CYAN}[INSTALL-INFO]$(date '+ %H:%M:%S')${NC} - $1"; }
success() { echo -e "${GREEN}[INSTALL-OK]$(date '+ %H:%M:%S')${NC} - $1"; }
warn()    { echo -e "${YELLOW}[INSTALL-WARN]$(date '+ %H:%M:%S')${NC} - $1"; }
error()   { echo -e "${RED}[INSTALL-ERROR]$(date '+ %H:%M:%S')${NC} - $1" >&2; exit 1; }

# Require root
if [[ "$EUID" -ne 0 ]]; then
    error "Aegis Installer must be run as root (sudo)."
fi

# Print banner
echo -e "${CYAN}"
cat << "EOF"
    ___  _____ _____ _____ _____   _____ _____   _           _        _ _
   / _ \|  ___|  __ \_   _/  ___| |  _  /  ___| | |         | |      | | |
  / /_\ \ |__ | |  \/ | | \ `--.  | | | \ `--.  | |__   __ _| | _____| | |
  |  _  |  __|| | __  | |  `--. \ | | | |`--. \ | '_ \ / _` | |/ / _ \ | |
  | | | | |___| |_\ \_| |_/\__/ / \ \_/ /\__/ / | |_) | (_| |   <  __/ | |
  \_| |_\____/ \____/\___/\____/   \___/\____/  |_.__/ \__,_|_|\_\___|_|_|
EOF
echo -e "                 Aegis OS — Distro Installer${NC}"
echo "----------------------------------------------------------------------"

# 1. Target Disk Selection
echo ""
echo "Select the target drive to partition and install Aegis OS on:"
lsblk -o NAME,SIZE,TYPE,MODEL
echo ""
read -rp "Enter target disk name (e.g. sda, nvme0n1): " TARGET_DISK_NAME

if [[ -z "$TARGET_DISK_NAME" ]]; then
    error "Disk name cannot be empty."
fi

DISK="/dev/$TARGET_DISK_NAME"
if [[ ! -e "$DISK" ]]; then
    error "Disk $DISK does not exist."
fi

# DANGER WARN
echo -e "${RED}⚠️  WARNING: ALL DATA ON $DISK WILL BE IRREVERSIBLY DESTROYED! ⚠️${NC}"
read -rp "Are you absolutely sure you want to proceed? [y/N]: " confirm
if [[ "$confirm" != "y" && "$confirm" != "Y" ]]; then
    error "Installation cancelled by operator."
fi

# 2. Partitioning (GPT layout)
log "Wiping existing partition table on $DISK..."
parted --script "$DISK" mklabel gpt

log "Creating Boot/EFI partition (512MB)..."
parted --script "$DISK" mkpart primary fat32 1MiB 513MiB
parted --script "$DISK" set 1 esp on

log "Creating System root partition (20GB)..."
parted --script "$DISK" mkpart primary ext4 513MiB 20.5GiB

log "Creating Encrypted data partition (Remaining space)..."
parted --script "$DISK" mkpart primary ext4 20.5GiB 100%

# Verify partition numbers and suffixes
# For nvme devices, partitions are nvme0n1p1, for sata they are sda1
P_BOOT="${DISK}1"
P_ROOT="${DISK}2"
P_CRYPT="${DISK}3"
if [[ "$TARGET_DISK_NAME" =~ "nvme" ]]; then
    P_BOOT="${DISK}p1"
    P_ROOT="${DISK}p2"
    P_CRYPT="${DISK}p3"
fi

# 3. Formatting
log "Formatting Boot partition..."
mkfs.vfat -F32 -n AEGIS_BOOT "$P_BOOT"

log "Formatting Root partition..."
mkfs.ext4 -F -L AEGIS_ROOT "$P_ROOT"

# LUKS setup on encrypted partition
log "Initializing LUKS container for writeable /data..."
echo -e "${YELLOW}Enter passphrase for disk encryption:${NC}"
cryptsetup luksFormat --type luks2 "$P_CRYPT"
cryptsetup open "$P_CRYPT" crypted-data

log "Formatting unlocked Crypt volume with ext4..."
mkfs.ext4 -L AEGIS_DATA "/dev/mapper/crypted-data"

# 4. Mount System to /mnt for NixOS Install
log "Mounting partitions to /mnt..."
mount -o ro,noatime,noload "$P_ROOT" "/mnt" 2>/dev/null || mount "$P_ROOT" "/mnt"
mkdir -p "/mnt/boot"
mount "$P_BOOT" "/mnt/boot"

mkdir -p "/mnt/data"
mount -o rw,noatime "/dev/mapper/crypted-data" "/mnt/data"

# Bind mount directories for installer context
mkdir -p "/mnt/etc/aegis" "/mnt/var/lib/aegis"
mkdir -p "/mnt/data/etc/aegis" "/mnt/data/var/lib/aegis"
mount --bind "/mnt/data/etc/aegis" "/mnt/etc/aegis"
mount --bind "/mnt/data/var/lib/aegis" "/mnt/var/lib/aegis"

# 5. Populate Configuration Files
log "Populating NixOS configurations..."
mkdir -p "/mnt/etc/nixos"

# Copy local nix definitions
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DISTRO_ROOT="$(dirname "$SCRIPT_DIR")"

cp "$DISTRO_ROOT/nixos/"* "/mnt/etc/nixos/"

# 6. Execute NixOS Bootstrap
log "Installing NixOS packages and services (this will download Nix configurations)..."
nixos-install --flake "/mnt/etc/nixos#aegis-node" --no-root-passwd

# 7. Write first-boot setup env
log "Setting up first-boot environment envelopes..."
ROOT_KEY=$(openssl rand -hex 32)
cat > "/mnt/data/etc/aegis/aegis.env" <<EOF
AEGIS_ROOT_KEY=${ROOT_KEY}
AEGIS_DATA_DIR=/var/lib/aegis
AEGIS_AGENTS_CONFIG_DIR=/etc/aegis/agents
AEGIS_MODEL_PROFILE=cloud
AEGIS_HTTP_PORT=8000
ANK_HTTP_PORT=8000
UI_DIST_PATH=/usr/share/aegis/ui
HW_PROFILE=1
DEFAULT_MODEL_PREF=CloudOnly
EOF
chmod 600 "/mnt/data/etc/aegis/aegis.env"

success "----------------------------------------------------------------------"
success "   AEGIS OS INSTALLED SUCCESSFULLY."
success "   Remove the installation medium and reboot the node."
success "   On boot, enter your LUKS passphrase to unlock the /data layer."
success "   Aegis Web UI will be serving immediately on http://localhost:8000"
success "----------------------------------------------------------------------"