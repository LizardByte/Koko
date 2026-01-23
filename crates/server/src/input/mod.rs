//! Input handling module for mouse and keyboard events.

pub mod x11_input;

use serde::{Deserialize, Serialize};

/// Mouse button types
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MouseButton {
    /// Left mouse button
    Left,
    /// Middle mouse button
    Middle,
    /// Right mouse button
    Right,
    /// Additional button 4
    Button4,
    /// Additional button 5
    Button5,
}

/// Mouse event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MouseEvent {
    /// Mouse moved to absolute position
    Move { x: i32, y: i32 },
    /// Mouse button pressed
    ButtonDown { button: MouseButton },
    /// Mouse button released
    ButtonUp { button: MouseButton },
    /// Mouse wheel scrolled
    Scroll { delta_x: i32, delta_y: i32 },
}

/// Keyboard event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyEvent {
    /// Key pressed
    KeyDown { code: u32, key: String },
    /// Key released
    KeyUp { code: u32, key: String },
}

/// Input event from client
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InputEvent {
    /// Mouse event
    Mouse(MouseEvent),
    /// Keyboard event
    Keyboard(KeyEvent),
}

/// Input handler trait
pub trait InputHandler: Send + Sync {
    /// Handle a mouse event
    fn handle_mouse(
        &self,
        event: MouseEvent,
    ) -> Result<(), Box<dyn std::error::Error>>;

    /// Handle a keyboard event
    fn handle_keyboard(
        &self,
        event: KeyEvent,
    ) -> Result<(), Box<dyn std::error::Error>>;

    /// Handle any input event
    fn handle_event(
        &self,
        event: InputEvent,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match event {
            InputEvent::Mouse(e) => self.handle_mouse(e),
            InputEvent::Keyboard(e) => self.handle_keyboard(e),
        }
    }
}

/// Get the platform-specific input handler
pub fn get_input_handler() -> Result<Box<dyn InputHandler>, Box<dyn std::error::Error>> {
    #[cfg(target_os = "linux")]
    {
        Ok(Box::new(x11_input::X11InputHandler::new()?))
    }

    #[cfg(not(target_os = "linux"))]
    {
        Err("Input handling only supported on Linux".into())
    }
}
