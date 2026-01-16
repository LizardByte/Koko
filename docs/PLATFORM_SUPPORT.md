# Platform Support and Conditional Compilation

## Overview

Koko Remote Desktop has been designed with platform-specific features, where the remote desktop functionality is exclusive to Linux systems, while the core web server and authentication features work across all platforms.

## Platform Support Matrix

| Feature | Linux | macOS | Windows |
|---------|-------|-------|---------|
| Web Server | ✅ | ✅ | ✅ |
| Authentication | ✅ | ✅ | ✅ |
| User Management | ✅ | ✅ | ✅ |
| Database | ✅ | ✅ | ✅ |
| **Remote Desktop** | ✅ | ❌ | ❌ |
| Screen Capture | ✅ | ❌ | ❌ |
| Input Forwarding | ✅ | ❌ | ❌ |
| Clipboard Sync | ✅ | ❌ | ❌ |
| Multi-Monitor | ✅ | ❌ | ❌ |

## Conditional Compilation

### Linux-Only Modules

The following modules are only compiled on Linux:

```rust
#[cfg(target_os = "linux")]
pub mod capture;      // Screen capture with GStreamer

#[cfg(target_os = "linux")]
pub mod clipboard;    // Clipboard synchronization

#[cfg(target_os = "linux")]
pub mod input;        // Input simulation

#[cfg(target_os = "linux")]
pub mod streaming;    // WebSocket streaming
```

### Linux-Only Dependencies

These dependencies are only included when building for Linux:

```toml
[target.'cfg(target_os = "linux")'.dependencies]
gstreamer = "0.23"              # Video encoding
gstreamer-app = "0.23"          # GStreamer app integration
gstreamer-video = "0.23"        # Video handling
x11rb = "0.13"                  # X11 protocol
xcb = "1.4"                     # X11 connection (indirect)
rdev = "0.5"                    # Input simulation
arboard = "3.4"                 # Clipboard access
futures = "0.3"                 # Async utilities
rocket_ws = "0.1.1"             # WebSocket for Rocket
```

### Web Server Configuration

The web server initialization adapts based on the target platform:

**On Linux:**
- Full remote desktop features enabled
- Screen capture initialized
- Clipboard monitoring started
- WebSocket streaming endpoint available at `/stream`

**On macOS/Windows:**
- Core web features only
- Authentication and user management
- API documentation (Swagger/RapiDoc)
- No remote desktop endpoints

## Building for Different Platforms

### Linux Build
```bash
# Full functionality including remote desktop
cargo build --release
```

### macOS/Windows Build
```bash
# Core functionality only (no remote desktop)
cargo build --release
```

### Cross-Compilation
```bash
# Build Linux target from another platform
cargo build --release --target x86_64-unknown-linux-gnu
```

## Runtime Behavior

### On Linux

When starting the server on Linux:
1. Web server starts on configured port
2. Screen capture manager initializes
3. GStreamer pipeline created with hardware encoding (if available)
4. Clipboard monitoring begins
5. WebSocket endpoint `/stream` becomes available
6. Clients can connect for remote desktop access

### On macOS/Windows

When starting the server on non-Linux platforms:
1. Web server starts on configured port
2. Authentication system active
3. User management available
4. API documentation accessible
5. Remote desktop features not available
6. Attempting to access `/stream` returns 404

## Client Compatibility

### Desktop Client
The desktop client (`koko-client`) is designed to run on multiple platforms:
- Linux (primary target)
- Windows (cross-platform GUI)
- macOS (cross-platform GUI)

However, it requires a Linux server to connect to for remote desktop functionality.

## Feature Detection

### At Compile Time

The Rust compiler automatically excludes Linux-specific code when building for other platforms:

```rust
// This code only compiles on Linux
#[cfg(target_os = "linux")]
fn start_capture() {
    // Screen capture implementation
}

// This code compiles on all platforms
fn start_web_server() {
    // Web server implementation
}
```

### At Runtime

For cases where runtime detection is needed:

```rust
if cfg!(target_os = "linux") {
    // Linux-specific initialization
} else {
    log::warn!("Remote desktop features not available on this platform");
}
```

## CI/CD Considerations

### GitHub Actions Matrix

The CI pipeline builds for multiple targets:

```yaml
matrix:
  include:
    - target: x86_64-unknown-linux-gnu      # Full features
    - target: aarch64-unknown-linux-gnu     # Full features (ARM)
    - target: x86_64-apple-darwin           # Core only
    - target: x86_64-pc-windows-msvc        # Core only
```

### Platform-Specific Dependencies

System dependencies are only installed for Linux builds:

```yaml
- name: Install system dependencies (Linux)
  if: contains(matrix.os, 'ubuntu')
  run: |
    sudo apt-get install -y \
      libgstreamer1.0-dev \
      libx11-dev \
      libxrandr-dev
```

## Testing

### Linux Tests
All tests run on Linux, including:
- Screen capture tests
- Input simulation tests
- Clipboard sync tests
- Full integration tests

### Cross-Platform Tests
Core functionality tests run on all platforms:
- Authentication tests
- Database tests
- Configuration tests
- Web API tests

## Error Messages

When accessing Linux-only features on other platforms:

```
Error: Remote desktop features are only available on Linux
```

Or in logs:
```
WARN: Screen capture not available on this platform
WARN: Input forwarding not available on this platform
```

## Future Platform Support

### Planned
- **Wayland support** on Linux (alternative to X11)
- **Windows capture** using Windows Graphics Capture API
- **macOS capture** using AVFoundation

### Not Planned
- Mobile platforms (iOS, Android) for server
  - But mobile clients may be developed

## Development

### Testing on Linux
```bash
# Full test suite
cargo test --workspace

# Linux-specific tests
cargo test --workspace --features linux-only
```

### Testing on macOS/Windows
```bash
# Core tests only
cargo test -p koko --lib
```

### Cross-Platform Development

When developing features that work across platforms, avoid platform-specific dependencies in shared code. Use traits and feature flags to abstract platform differences.

## Documentation

Platform-specific features are marked in the documentation:

```rust
/// Start screen capture (Linux only)
///
/// # Platform Support
/// - ✅ Linux (X11)
/// - ❌ macOS
/// - ❌ Windows
#[cfg(target_os = "linux")]
pub fn start_capture() { ... }
```

## Troubleshooting

### Compilation Errors on Non-Linux

If you see errors about missing modules like `capture` or `streaming` on non-Linux platforms, this is expected. The server will compile without remote desktop features.

### Missing Dependencies on Linux

If you see linker errors on Linux:
```bash
# Install required system dependencies
sudo apt-get install -y \
  libgstreamer1.0-dev \
  libx11-dev \
  libxrandr-dev
```

### Runtime Feature Detection

To check if remote desktop features are available:

```bash
# Linux: Should show streaming endpoint
curl -k https://localhost:8080/stream

# macOS/Windows: Returns 404
curl -k https://localhost:8080/stream
```

## Summary

- **Core Features**: Work on all platforms (Linux, macOS, Windows)
- **Remote Desktop**: Linux-only (X11 required)
- **Client**: Cross-platform, connects to Linux server
- **Compilation**: Automatic conditional compilation based on target
- **CI/CD**: Builds for all platforms, tests appropriately

This design allows the project to compile and run on multiple platforms while keeping the remote desktop functionality focused on its primary Linux target.

