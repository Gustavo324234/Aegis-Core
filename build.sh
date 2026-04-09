#!/usr/bin/env bash
set -euo pipefail

# Aegis Unified Build Script
# This script compiles both the UI and the Kernel.

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SHELL_UI_DIR="$PROJECT_ROOT/shell/ui"

echo "🎨 Building UI (React)..."
if [ -d "$SHELL_UI_DIR" ]; then
    cd "$SHELL_UI_DIR"
    npm ci
    npm run build
    cd "$PROJECT_ROOT"
else
    echo "❌ Error: shell/ui directory not found!"
    exit 1
fi

echo "🦀 Building Kernel (Rust)..."
# Check if we want to embed the UI
if [[ "${1:-}" == "--embed" ]]; then
    echo "📦 Embedding UI into binary..."
    cargo build --release -p ank-server --features embed-ui
else
    cargo build --release -p ank-server
fi

echo ""
echo "✅ Build complete!"

UI_DIST_PATH="$SHELL_UI_DIR/dist"
echo "🚀 To run:"
if [[ "${1:-}" == "--embed" ]]; then
    echo "   ./target/release/ank-server"
else
    echo "   UI_DIST_PATH=\"$UI_DIST_PATH\" ./target/release/ank-server"
fi
