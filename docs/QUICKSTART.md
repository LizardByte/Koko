# Quick Start Guide - Koko Remote Desktop

This guide will help you get Koko Remote Desktop up and running quickly on Linux.

## Prerequisites

- Ubuntu 20.04+ or Debian 11+ (or compatible Linux distribution)
- X11 display server
- Rust 1.70+ (will be installed if not present)

## Step 1: Install System Dependencies

```bash
# Update package lists
sudo apt-get update

# Install GStreamer and X11 dependencies
sudo apt-get install -y \
    libgstreamer1.0-dev \
    libgstreamer-plugins-base1.0-dev \
    gstreamer1.0-plugins-base \
    gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-bad \
    gstreamer1.0-plugins-ugly \
    gstreamer1.0-libav \
    libx11-dev \
    libxrandr-dev \
    libxi-dev \
    libxcb1-dev \
    libxcb-randr0-dev \
    libxcb-xtest0-dev \
    libxcb-xinerama0-dev \
    libxcb-shape0-dev \
    libxcb-xkb-dev \
    pkg-config

# Optional: Install hardware encoding support
# For NVIDIA GPUs:
sudo apt-get install -y gstreamer1.0-plugins-nvcodec

# For AMD/Intel GPUs:
sudo apt-get install -y gstreamer1.0-vaapi
```

## Step 2: Install Rust (if not already installed)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

## Step 3: Clone and Build

```bash
# Clone the repository
git clone https://github.com/LizardByte/Koko.git
cd Koko

# Build the server
cargo build --release -p koko

# Build the client
cargo build --release -p koko-client
```

## Step 4: Set Up the Server

```bash
# Run the server for first-time setup
./target/release/koko

# The server will:
# - Create configuration files
# - Generate self-signed certificates
# - Initialize the database
# - Start the web server

# Server will be available at: https://localhost:8080
```

## Step 5: Create First User

Open your browser and navigate to `https://localhost:8080`

1. Accept the self-signed certificate warning (or install custom certificates)
2. Create the first admin user through the web interface
3. Note your username and password

## Step 6: Start the Client

On the client machine (can be the same machine for testing):

```bash
./target/release/koko-client
```

In the client application:
1. Enter server URL: `wss://localhost:8080/stream` (or server IP)
2. Enter your username and password
3. Click "Connect"

## Step 7: Test the Connection

Once connected, you should see:
- Live video stream of your desktop
- Mouse and keyboard input working
- Clipboard synchronization active

## Configuration

### Server Configuration

Edit `~/.local/share/koko/config.toml`:

```toml
[server]
address = "0.0.0.0"
port = 8080
use_https = true

[remote_desktop.capture]
framerate = 60
hw_accel = "auto"  # auto, nvenc, vaapi, or none
bitrate = 10000    # kbps
preset = "ultrafast"
monitors = []      # empty = all monitors

[remote_desktop.streaming]
enable_clipboard = true
max_clients = 10
```

Restart the server after configuration changes.

## Verify Installation

### Check GStreamer Plugins

```bash
# List all H.264 related plugins
gst-inspect-1.0 | grep -i h264

# Check for hardware encoders
gst-inspect-1.0 nvh264enc   # NVIDIA
gst-inspect-1.0 vaapih264enc # AMD/Intel

# Test screen capture
gst-launch-1.0 ximagesrc ! videoconvert ! autovideosink
```

### Check Server Logs

```bash
# Server logs are in:
~/.local/share/koko/logs/

# View latest log:
tail -f ~/.local/share/koko/logs/koko.log
```

## Troubleshooting

### Server won't start

```bash
# Check if port is already in use
sudo netstat -tulpn | grep 8080

# Check logs for errors
cat ~/.local/share/koko/logs/koko.log
```

### Video not displaying

```bash
# Test GStreamer pipeline manually
gst-launch-1.0 ximagesrc ! videoconvert ! x264enc ! fakesink

# Check for missing plugins
gst-inspect-1.0 x264enc
```

### High CPU usage

- Enable hardware encoding (NVENC or VAAPI)
- Lower the framerate
- Reduce bitrate
- Use faster encoder preset

### Input lag

- Use wired network connection
- Enable hardware encoding
- Reduce network latency
- Lower framerate if necessary

## Next Steps

- Read the full documentation: `docs/REMOTE_DESKTOP.md`
- Set up custom SSL certificates for production
- Configure firewall rules for remote access
- Set up multiple user accounts
- Tune performance settings for your hardware

## Security Notes

⚠️ **Important for Production Use**:

1. Replace self-signed certificates with trusted certificates
2. Use strong passwords for all accounts
3. Enable firewall and only expose necessary ports
4. Use VPN for remote access when possible
5. Regularly update the software and dependencies
6. Monitor server logs for unauthorized access attempts

## Getting Help

- Check the logs: `~/.local/share/koko/logs/`
- View documentation: `docs/REMOTE_DESKTOP.md`
- Report issues: GitHub Issues
- Community support: Project discussions

## Performance Benchmarks

Expected latency with optimal setup:
- Local network (1Gbps): 10-30ms
- Local network (100Mbps): 20-50ms
- VPN/Remote: 50-150ms (depends on network)

Hardware encoding recommended for:
- 1080p @ 60fps: Required
- 1440p @ 60fps: Mandatory
- 4K @ 30fps: Mandatory

## Uninstallation

```bash
# Remove binaries
rm target/release/koko
rm target/release/koko-client

# Remove data (WARNING: This deletes all configuration and databases)
rm -rf ~/.local/share/koko/
```

