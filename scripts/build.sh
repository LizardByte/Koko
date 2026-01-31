#!/bin/bash
# Build and installation script for Koko Remote Desktop

set -e

echo "========================================="
echo "Koko Remote Desktop - Build & Install"
echo "========================================="
echo ""

# Check if running on Linux
if [[ "$OSTYPE" != "linux-gnu"* ]]; then
    echo "ERROR: This script only supports Linux"
    exit 1
fi

# Check for required commands
command -v cargo >/dev/null 2>&1 || {
    echo "ERROR: Rust/Cargo not found. Install from https://rustup.rs/"
    exit 1
}

echo "Step 1: Checking system dependencies..."
echo ""

# Check for GStreamer
if ! pkg-config --exists gstreamer-1.0; then
    echo "WARNING: GStreamer not found"
    echo "Install with:"
    echo "  sudo apt-get install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev"
    echo ""
    read -p "Continue anyway? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# Check for X11
if ! pkg-config --exists x11; then
    echo "WARNING: X11 development files not found"
    echo "Install with:"
    echo "  sudo apt-get install libx11-dev libxrandr-dev"
    echo ""
    read -p "Continue anyway? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

echo "Step 2: Building server..."
echo ""
cargo build --release -p koko

echo ""
echo "Step 3: Building client..."
echo ""
cargo build --release -p koko-client

echo ""
echo "Step 4: Running tests..."
echo ""
cargo test --workspace || echo "WARNING: Some tests failed"

echo ""
echo "========================================="
echo "Build complete!"
echo "========================================="
echo ""
echo "Binaries:"
echo "  Server: ./target/release/koko"
echo "  Client: ./target/release/koko-client"
echo ""
echo "Next steps:"
echo "  1. Install binaries (optional):"
echo "     sudo cp target/release/koko /usr/local/bin/"
echo "     sudo cp target/release/koko-client /usr/local/bin/"
echo ""
echo "  2. Copy example configuration:"
echo "     mkdir -p ~/.local/share/koko"
echo "     cp config.example.toml ~/.local/share/koko/config.toml"
echo ""
echo "  3. Start the server:"
echo "     ./target/release/koko"
echo ""
echo "  4. Start the client:"
echo "     ./target/release/koko-client"
echo ""
echo "  5. Read the documentation:"
echo "     docs/QUICKSTART.md"
echo "     docs/REMOTE_DESKTOP.md"
echo ""

# Offer to install
echo ""
read -p "Install binaries to /usr/local/bin? (y/N) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    sudo cp target/release/koko /usr/local/bin/
    sudo cp target/release/koko-client /usr/local/bin/
    sudo chmod +x /usr/local/bin/koko
    sudo chmod +x /usr/local/bin/koko-client
    echo "Binaries installed successfully!"
    echo ""

    # Offer to install systemd service
    read -p "Install systemd service? (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        sudo cp docs/koko@.service /etc/systemd/system/
        sudo systemctl daemon-reload
        echo ""
        echo "Systemd service installed!"
        echo "Enable and start with:"
        echo "  sudo systemctl enable koko@$USER"
        echo "  sudo systemctl start koko@$USER"
        echo ""
    fi
fi

echo "Installation complete! 🎉"

