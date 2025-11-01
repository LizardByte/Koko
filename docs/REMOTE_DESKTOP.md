# Koko Remote Desktop

An ultra-low latency remote desktop solution for Linux, designed for secure thin client environments where users cannot store files locally.

## Features

- **Ultra-Low Latency Streaming**: Uses GStreamer with hardware-accelerated encoding (NVIDIA NVENC, AMD VAAPI) for minimal latency
- **Shared Input**: Full mouse and keyboard support forwarded from client to server
- **Shared Clipboard**: Bidirectional clipboard synchronization
- **Multi-Monitor Support**: Capture and stream multiple monitors simultaneously
- **GPU & Software Encoding**: Supports NVIDIA, AMD GPU encoding with automatic fallback to software encoding
- **Authentication**: Secure username/password authentication from client to server
- **Linux Only**: Optimized specifically for Linux systems (X11)

## Architecture

### Server (`crates/server`)
The server runs on the Linux machine that users connect to. It:
- Captures screen content using X11
- Encodes video using GStreamer (H.264 with hardware acceleration when available)
- Handles mouse and keyboard input from clients
- Synchronizes clipboard content
- Provides WebSocket API for client connections

### Client (`crates/client-desktop`)
The thin client application that connects to the server. It:
- Receives and decodes H.264 video stream
- Displays video with minimal latency
- Captures and forwards mouse/keyboard input
- Synchronizes clipboard with server

## Requirements

### Server Requirements
- Linux with X11
- GStreamer 1.x with plugins:
  - gstreamer1.0-plugins-base
  - gstreamer1.0-plugins-good
  - gstreamer1.0-plugins-bad
  - gstreamer1.0-plugins-ugly
  - gstreamer1.0-libav
- For NVIDIA GPU: gstreamer1.0-plugins-nvcodec
- For AMD/Intel GPU: gstreamer1.0-vaapi
- X11 development libraries

### Client Requirements
- Linux (or other platforms - client is cross-platform)
- GStreamer 1.x for video decoding
- Modern GPU for smooth video playback

## Installation

### Install GStreamer Dependencies (Ubuntu/Debian)

```bash
# Server dependencies
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
    libxi-dev

# For NVIDIA GPUs
sudo apt-get install gstreamer1.0-plugins-nvcodec

# For AMD/Intel GPUs
sudo apt-get install gstreamer1.0-vaapi
```

### Build from Source

```bash
# Build server
cargo build --release -p koko

# Build client
cargo build --release -p koko-client
```

## Usage

### Starting the Server

```bash
# Run the server
./target/release/koko

# The server will start on port 8080 by default
# Access the web interface at https://localhost:8080
```

### Configuration

Server configuration is stored in the application data directory. Key settings:

- `server.address`: Server bind address (default: "0.0.0.0")
- `server.port`: Server port (default: 8080)
- `server.use_https`: Enable HTTPS (recommended, default: true)

### Starting the Client

```bash
# Run the client
./target/release/koko-client
```

In the client:
1. Enter server URL (e.g., `wss://server-ip:8080/stream`)
2. Enter username and password
3. Click "Connect"

## API Endpoints

### WebSocket Streaming API

**Endpoint**: `/stream`

**Authentication**: Requires valid JWT token in query parameter

**Server → Client Messages**:
```json
{
  "type": "Frame",
  "data": [bytes],
  "width": 1920,
  "height": 1080,
  "timestamp": 123456789,
  "monitor_index": 0
}
```

```json
{
  "type": "Monitors",
  "monitors": [
    {
      "index": 0,
      "name": "DP-1",
      "x": 0,
      "y": 0,
      "width": 1920,
      "height": 1080,
      "primary": true
    }
  ]
}
```

```json
{
  "type": "Clipboard",
  "text": "clipboard content",
  "timestamp": 123456789
}
```

**Client → Server Messages**:
```json
{
  "type": "Input",
  "event": {
    "type": "Mouse",
    "Move": { "x": 100, "y": 200 }
  }
}
```

```json
{
  "type": "Clipboard",
  "text": "clipboard content"
}
```

```json
{
  "type": "GetMonitors"
}
```

## Performance Tuning

### Server-Side

1. **Hardware Encoding**: Ensure GPU drivers are properly installed
   - NVIDIA: Use latest NVIDIA drivers with NVENC support
   - AMD: Ensure VAAPI drivers are installed

2. **Bitrate**: Adjust in `CaptureConfig` (default: 10000 kbps)
   - Lower for slower networks
   - Higher for better quality

3. **Frame Rate**: Default 60 FPS, can be adjusted

4. **Encoder Preset**: 
   - "ultrafast" for lowest latency (default)
   - "fast" for better quality at slight latency cost

### Network Requirements

- Recommended: 10+ Mbps bandwidth per client
- Latency: < 50ms for optimal experience
- Wired Ethernet strongly recommended over WiFi

## Security

- HTTPS/WSS encryption for all communications
- JWT-based authentication
- Password hashing with bcrypt
- Self-signed certificates auto-generated (use custom certificates in production)

## Limitations

- **Linux Only**: Server requires Linux with X11 (Wayland support planned)
- **H.264 Codec**: Currently only supports H.264 encoding
- **Single User**: Designed for one user per server instance

## Troubleshooting

### Video Not Displaying

1. Check GStreamer plugins are installed:
```bash
gst-inspect-1.0 | grep -i h264
```

2. Check hardware encoder availability:
```bash
# NVIDIA
gst-inspect-1.0 nvh264enc

# VAAPI
gst-inspect-1.0 vaapih264enc
```

3. Check server logs for encoding errors

### High Latency

1. Verify network bandwidth and latency
2. Enable hardware encoding
3. Reduce video quality/bitrate
4. Use wired connection

### Input Not Working

1. Ensure server has permission to simulate input events
2. Check firewall is not blocking WebSocket connection
3. Verify authentication token is valid

## Development

### Running Tests

```bash
cargo test --workspace
```

### Checking Code

```bash
cargo clippy --workspace
cargo fmt --check
```

## License

See LICENSE file for details.

## Contributing

Contributions welcome! Please ensure:
- Code is formatted with `rustfmt`
- Tests pass
- No clippy warnings
- Linux-focused (primary target)

## Roadmap

- [ ] Wayland support
- [ ] Audio streaming
- [ ] Multi-user support
- [ ] Session recording
- [ ] Advanced compression options (AV1, VP9)
- [ ] Mobile clients (iOS, Android)
- [ ] File transfer restrictions
- [ ] Admin dashboard
- [ ] Load balancing for multiple servers

