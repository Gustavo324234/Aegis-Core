.PHONY: build build-embed dev clean ui kernel

# Default target
build: ui kernel

# Build with embedded UI
build-embed: ui
	@echo "📦 Building kernel with embedded UI..."
	cargo build --release -p ank-server --features embed-ui

ui:
	@echo "🎨 Building UI..."
	cd shell/ui && npm ci && npm run build

kernel:
	@echo "🦀 Building Kernel..."
	cargo build --release -p ank-server

dev:
	@echo "🏃 Starting dev mode..."
	@export UI_DIST_PATH=$(shell pwd)/shell/ui/dist && cargo run -p ank-server

clean:
	@echo "🧹 Cleaning..."
	rm -rf shell/ui/dist
	cargo clean
