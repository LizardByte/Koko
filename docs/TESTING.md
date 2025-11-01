# Testing Guide - Koko Remote Desktop

This guide provides comprehensive testing procedures for Koko Remote Desktop.

## Prerequisites

Before testing, ensure:
- [ ] Server and client binaries are built successfully
- [ ] All system dependencies are installed
- [ ] You have access to a Linux machine with X11
- [ ] Network connectivity is available

## Unit Testing

### Running All Tests
```bash
cd /path/to/Koko
cargo test --workspace
```

### Running Server Tests Only
```bash
cargo test -p koko
```

### Running with Output
```bash
cargo test --workspace -- --nocapture
```

## Integration Testing

### 1. Server Startup Test

**Objective**: Verify server starts without errors

```bash
# Start server
./target/release/koko

# Expected output:
# - Configuration loaded
# - Database initialized
# - Certificates generated (if first run)
# - Web server started
# - No errors in logs
```

**Verification**:
- [ ] Server starts without panics
- [ ] Log file created: `~/.local/share/koko/logs/koko.log`
- [ ] Database created: `~/.local/share/koko/koko.db`
- [ ] Certificates created: `~/.local/share/koko/cert.pem` and `key.pem`
- [ ] Server listens on configured port (default 8080)

### 2. Web Interface Test

**Objective**: Verify web interface is accessible

```bash
# Open browser
firefox https://localhost:8080
# or
curl -k https://localhost:8080
```

**Verification**:
- [ ] HTTPS connection established
- [ ] Login page displays
- [ ] No JavaScript errors in console
- [ ] Swagger UI accessible at `/swagger-ui/`
- [ ] RapiDoc accessible at `/rapidoc/`

### 3. User Creation Test

**Objective**: Create first admin user

Via Web UI:
1. Navigate to `https://localhost:8080`
2. Fill in username and password
3. Submit registration

**Verification**:
- [ ] User created successfully
- [ ] Can log in with credentials
- [ ] JWT token received
- [ ] User has admin privileges

### 4. Authentication Test

**Objective**: Test JWT authentication flow

```bash
# Login and get token
curl -k -X POST https://localhost:8080/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"password"}'

# Use token for authenticated request
curl -k -X GET https://localhost:8080/user/info \
  -H "Authorization: Bearer YOUR_TOKEN_HERE"
```

**Verification**:
- [ ] Login returns valid JWT token
- [ ] Token works for authenticated endpoints
- [ ] Invalid token returns 401 Unauthorized
- [ ] Token expires after configured time

## Remote Desktop Testing

### 5. Monitor Detection Test

**Objective**: Verify monitors are detected correctly

**Server Side**:
```bash
# Check server logs for monitor detection
tail -f ~/.local/share/koko/logs/koko.log | grep -i monitor
```

**Expected Output**:
```
INFO Detected 2 monitors
INFO Monitor 0: DP-1 (1920x1080+0+0)
INFO Monitor 1: HDMI-1 (1920x1080+1920+0)
```

**Verification**:
- [ ] All connected monitors detected
- [ ] Correct resolutions reported
- [ ] Correct positions reported
- [ ] Primary monitor identified

### 6. Hardware Encoding Test

**Objective**: Verify hardware encoding is available

```bash
# Check for NVIDIA NVENC
gst-inspect-1.0 nvh264enc

# Check for AMD/Intel VAAPI
gst-inspect-1.0 vaapih264enc

# Check for software fallback
gst-inspect-1.0 x264enc
```

**Server Logs**:
```bash
tail -f ~/.local/share/koko/logs/koko.log | grep -i "encoder\|gstreamer"
```

**Verification**:
- [ ] Hardware encoder detected (if GPU present)
- [ ] GStreamer plugins loaded successfully
- [ ] Pipeline created without errors
- [ ] Encoding starts successfully

### 7. Screen Capture Test

**Objective**: Verify screen capture is working

```bash
# Test GStreamer capture pipeline manually
gst-launch-1.0 ximagesrc ! videoconvert ! autovideosink

# Test with encoding
gst-launch-1.0 ximagesrc ! videoconvert ! x264enc ! fakesink
```

**Server Logs**:
```bash
tail -f ~/.local/share/koko/logs/koko.log | grep -i capture
```

**Verification**:
- [ ] Screen content captured
- [ ] No visual artifacts
- [ ] Framerate stable
- [ ] CPU/GPU usage acceptable

### 8. Client Connection Test

**Objective**: Test client can connect to server

**Steps**:
1. Start server: `./target/release/koko`
2. Start client: `./target/release/koko-client`
3. Enter connection details
4. Click "Connect"

**Verification**:
- [ ] WebSocket connection established
- [ ] Authentication successful
- [ ] Monitor list received
- [ ] No connection errors

### 9. Video Streaming Test

**Objective**: Verify video is streaming to client

**Client Side**:
- Watch for "Received frame" messages
- Check FPS counter in UI

**Server Logs**:
```bash
tail -f ~/.local/share/koko/logs/koko.log | grep -i "frame\|streaming"
```

**Verification**:
- [ ] Video frames received by client
- [ ] Frame rate matches configured value (±5 fps)
- [ ] Video quality acceptable
- [ ] No dropped frames
- [ ] Latency < 100ms

### 10. Mouse Input Test

**Objective**: Test mouse control from client

**Test Cases**:
1. **Mouse Movement**
   - Move mouse in client
   - Verify cursor moves on server
   - [ ] Movement smooth and responsive
   - [ ] Coordinates accurate

2. **Left Click**
   - Click in client
   - Verify click registered on server
   - [ ] Clicks work correctly

3. **Right Click**
   - Right-click in client
   - Verify context menu appears on server
   - [ ] Right-clicks work correctly

4. **Middle Click**
   - Middle-click in client
   - [ ] Middle-clicks work correctly

5. **Scroll Wheel**
   - Scroll in client
   - Verify scrolling on server
   - [ ] Scrolling smooth
   - [ ] Direction correct

**Verification**:
- [ ] All mouse buttons work
- [ ] Movement tracking accurate
- [ ] No input lag
- [ ] Works across all monitors

### 11. Keyboard Input Test

**Objective**: Test keyboard input from client

**Test Cases**:

1. **Alphanumeric Keys**
   - Type in client: `The quick brown fox jumps over the lazy dog 0123456789`
   - [ ] All characters appear correctly on server

2. **Special Characters**
   - Type: `!@#$%^&*()_+-=[]{}|;:'",.<>?/~`
   - [ ] All special characters work

3. **Function Keys**
   - Press F1-F12
   - [ ] Function keys work

4. **Modifier Keys**
   - Test Shift, Ctrl, Alt, Super
   - Test combinations: Ctrl+C, Ctrl+V, Alt+Tab, etc.
   - [ ] All modifiers work
   - [ ] Key combinations work

5. **Arrow Keys**
   - Test Up, Down, Left, Right
   - [ ] Arrow keys work

6. **Special Keys**
   - Test Enter, Escape, Tab, Backspace, Delete
   - [ ] All special keys work

**Verification**:
- [ ] All keys register correctly
- [ ] No missed keystrokes
- [ ] Modifiers work properly
- [ ] No repeat issues

### 12. Clipboard Sync Test

**Objective**: Test clipboard synchronization

**Test Case 1: Server → Client**
1. Copy text on server
2. Wait 1 second
3. Check client clipboard
4. [ ] Text copied to client clipboard

**Test Case 2: Client → Server**
1. Copy text in client UI
2. Check server clipboard
3. [ ] Text copied to server clipboard

**Test Case 3: Bidirectional**
1. Copy text on server
2. Verify on client
3. Copy different text on client
4. Verify on server
5. [ ] Both directions work
6. [ ] No echo loops

**Verification**:
- [ ] Text syncs correctly
- [ ] Updates within 1 second
- [ ] No duplicate syncs
- [ ] Large text (1MB+) works

### 13. Multi-Monitor Test

**Objective**: Test multi-monitor support

**Prerequisites**: System with 2+ monitors

**Test**:
1. Configure server for all monitors
2. Connect client
3. Verify monitor list
4. Test input on each monitor

**Verification**:
- [ ] All monitors detected
- [ ] Can view each monitor
- [ ] Input works on each monitor
- [ ] Coordinates mapped correctly

### 14. Multi-Client Test

**Objective**: Test multiple simultaneous clients

**Test**:
1. Start server
2. Connect client 1
3. Connect client 2
4. Connect client 3

**Verification**:
- [ ] All clients connect successfully
- [ ] All clients receive video
- [ ] Input from one client works
- [ ] No interference between clients
- [ ] Server CPU/memory usage acceptable

### 15. Disconnect/Reconnect Test

**Objective**: Test connection stability

**Test Case 1: Clean Disconnect**
1. Connect client
2. Click "Disconnect"
3. [ ] Clean disconnection
4. [ ] Server logs disconnect

**Test Case 2: Network Interruption**
1. Connect client
2. Disable network briefly
3. Re-enable network
4. [ ] Client reconnects automatically (if implemented)
5. [ ] Or shows error and allows retry

**Verification**:
- [ ] No server crashes on disconnect
- [ ] Resources cleaned up properly
- [ ] Can reconnect after disconnect

## Performance Testing

### 16. Latency Test

**Objective**: Measure end-to-end latency

**Method**:
1. Display timer on server (millisecond precision)
2. Record video of client screen
3. Compare timestamps

**Acceptable Latency**:
- Local network (1Gbps): < 30ms
- Local network (100Mbps): < 60ms
- VPN/Remote: < 150ms

**Verification**:
- [ ] Latency within acceptable range
- [ ] Latency stable over time
- [ ] No significant jitter

### 17. CPU/GPU Usage Test

**Objective**: Measure resource usage

**Tools**:
```bash
# CPU usage
top -p $(pgrep koko)

# GPU usage (NVIDIA)
nvidia-smi -l 1

# GPU usage (AMD)
radeontop
```

**Acceptable Usage** (1080p @ 60fps):
- CPU (hardware encoding): < 20%
- CPU (software encoding): < 80%
- GPU: < 40%
- Memory: < 500MB

**Verification**:
- [ ] CPU usage acceptable
- [ ] GPU usage acceptable (if HW encoding)
- [ ] No memory leaks over time
- [ ] Consistent performance

### 18. Bandwidth Test

**Objective**: Measure network bandwidth usage

**Tools**:
```bash
# Monitor network usage
iftop -i eth0

# Or
nethogs
```

**Expected Bandwidth** (typical):
- 1080p @ 60fps, 10Mbps bitrate: ~10-12 Mbps
- 720p @ 30fps, 5Mbps bitrate: ~5-7 Mbps

**Verification**:
- [ ] Bandwidth matches configured bitrate
- [ ] No excessive spikes
- [ ] Stable over time

### 19. Stress Test

**Objective**: Test under load

**Test Scenarios**:

1. **High Motion Content**
   - Play video with lots of movement
   - [ ] Quality maintained
   - [ ] No frame drops

2. **Multiple Clients**
   - Connect 10 clients simultaneously
   - [ ] All clients work
   - [ ] Performance acceptable

3. **Extended Duration**
   - Run for 8 hours continuously
   - [ ] No crashes
   - [ ] No memory leaks
   - [ ] Performance stable

## Security Testing

### 20. Authentication Test

**Test Cases**:

1. **Invalid Credentials**
   - Try logging in with wrong password
   - [ ] Returns 401 Unauthorized
   - [ ] No information leak

2. **Rate Limiting**
   - Try 10 failed logins rapidly
   - [ ] Account temporarily locked
   - [ ] Returns appropriate error

3. **Token Validation**
   - Try using expired token
   - [ ] Returns 401 Unauthorized
   - Try using modified token
   - [ ] Returns 401 Unauthorized

**Verification**:
- [ ] Authentication secure
- [ ] No bypass possible
- [ ] Rate limiting works

### 21. Encryption Test

**Objective**: Verify all traffic is encrypted

**Tools**:
```bash
# Capture traffic
sudo tcpdump -i any -w capture.pcap port 8080

# Analyze with Wireshark
wireshark capture.pcap
```

**Verification**:
- [ ] All HTTP traffic is HTTPS
- [ ] All WebSocket traffic is WSS
- [ ] No plaintext credentials
- [ ] TLS 1.2+ used

## Regression Testing

After any code changes:

- [ ] All unit tests pass
- [ ] Server starts without errors
- [ ] Client connects successfully
- [ ] Video streaming works
- [ ] Input forwarding works
- [ ] Clipboard sync works
- [ ] No performance degradation
- [ ] No new security issues

## Test Environment Setup

### Minimal Test Environment
- Server: Ubuntu 22.04 VM, 2 CPU, 4GB RAM
- Client: Ubuntu 22.04 VM, 1 CPU, 2GB RAM
- Network: Local bridge (1Gbps)

### Recommended Test Environment
- Server: Ubuntu 22.04, i7 CPU, 16GB RAM, NVIDIA GPU
- Clients: Multiple devices (Linux, Windows, macOS)
- Network: Real network with firewall, NAT

## Reporting Issues

When reporting bugs, include:

1. **System Information**
   - OS and version
   - Kernel version
   - CPU/GPU model
   - GStreamer version

2. **Configuration**
   - Server config.toml
   - Command line options

3. **Logs**
   - Server logs: `~/.local/share/koko/logs/koko.log`
   - Client output

4. **Reproduction Steps**
   - Exact steps to reproduce
   - Expected behavior
   - Actual behavior

5. **Additional Info**
   - Network setup
   - Firewall rules
   - Any error messages

## Continuous Integration

For CI/CD pipelines:

```yaml
# Example GitHub Actions workflow
name: Test

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      
      - name: Install dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y libgstreamer1.0-dev libx11-dev
      
      - name: Build
        run: cargo build --workspace
      
      - name: Run tests
        run: cargo test --workspace
      
      - name: Clippy
        run: cargo clippy --workspace -- -D warnings
```

## Success Criteria

A successful test run should have:

- ✅ All unit tests passing
- ✅ Server starts without errors
- ✅ Client connects successfully
- ✅ Video streams at configured FPS
- ✅ Latency < 100ms on local network
- ✅ All input works correctly
- ✅ Clipboard syncs bidirectionally
- ✅ No crashes during 1-hour test
- ✅ CPU/GPU usage acceptable
- ✅ No security vulnerabilities found

## Conclusion

This comprehensive testing guide ensures Koko Remote Desktop works correctly across all features and use cases. Regular testing prevents regressions and maintains quality.

