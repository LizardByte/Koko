//! Monitor detection and management for Linux.

use x11rb::connection::Connection;
use x11rb::protocol::randr;
use x11rb::rust_connection::RustConnection;

/// Information about a monitor
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    /// Monitor index
    pub index: usize,
    /// Monitor name
    pub name: String,
    /// X position
    pub x: i32,
    /// Y position
    pub y: i32,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Whether this is the primary monitor
    pub primary: bool,
}

/// Get all available monitors
pub fn get_monitors() -> Result<Vec<MonitorInfo>, Box<dyn std::error::Error>> {
    #[cfg(target_os = "linux")]
    {
        get_monitors_x11()
    }

    #[cfg(not(target_os = "linux"))]
    {
        Err("Monitor detection only supported on Linux".into())
    }
}

#[cfg(target_os = "linux")]
fn get_monitors_x11() -> Result<Vec<MonitorInfo>, Box<dyn std::error::Error>> {
    let (conn, screen_num) = RustConnection::connect(None)?;
    let screen = &conn.setup().roots[screen_num];
    let window = screen.root;

    // Query RandR for monitor information
    let resources = randr::get_screen_resources(&conn, window)?.reply()?;

    let mut monitors = Vec::new();

    for (idx, output) in resources.outputs.iter().enumerate() {
        let output_info = randr::get_output_info(&conn, *output, 0)?.reply()?;

        if output_info.connection != randr::Connection::CONNECTED {
            continue;
        }

        if output_info.crtc == 0 {
            continue;
        }

        let crtc_info = randr::get_crtc_info(&conn, output_info.crtc, 0)?.reply()?;

        let name = String::from_utf8_lossy(&output_info.name).to_string();

        monitors.push(MonitorInfo {
            index: idx,
            name,
            x: crtc_info.x as i32,
            y: crtc_info.y as i32,
            width: crtc_info.width as u32,
            height: crtc_info.height as u32,
            primary: idx == 0, // TODO: Actually detect primary monitor
        });
    }

    if monitors.is_empty() {
        // Fallback to root window dimensions
        monitors.push(MonitorInfo {
            index: 0,
            name: "Default".to_string(),
            x: 0,
            y: 0,
            width: screen.width_in_pixels as u32,
            height: screen.height_in_pixels as u32,
            primary: true,
        });
    }

    Ok(monitors)
}
