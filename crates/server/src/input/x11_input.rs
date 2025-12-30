//! X11 input handling implementation using rdev.

use crate::input::{InputHandler, KeyEvent, MouseButton, MouseEvent};
use rdev::{Button, EventType, Key, simulate};
use std::sync::Mutex;

/// X11 input handler
pub struct X11InputHandler {
    last_x: Mutex<i32>,
    last_y: Mutex<i32>,
}

impl X11InputHandler {
    /// Create a new X11 input handler
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            last_x: Mutex::new(0),
            last_y: Mutex::new(0),
        })
    }

    /// Convert MouseButton to rdev Button
    fn to_rdev_button(button: MouseButton) -> Button {
        match button {
            MouseButton::Left => Button::Left,
            MouseButton::Middle => Button::Middle,
            MouseButton::Right => Button::Right,
            MouseButton::Button4 => Button::Unknown(8),
            MouseButton::Button5 => Button::Unknown(9),
        }
    }

    /// Convert key string to rdev Key
    fn to_rdev_key(key_str: &str) -> Option<Key> {
        // Map common keys - this is a simplified mapping
        match key_str.to_lowercase().as_str() {
            "a" => Some(Key::KeyA),
            "b" => Some(Key::KeyB),
            "c" => Some(Key::KeyC),
            "d" => Some(Key::KeyD),
            "e" => Some(Key::KeyE),
            "f" => Some(Key::KeyF),
            "g" => Some(Key::KeyG),
            "h" => Some(Key::KeyH),
            "i" => Some(Key::KeyI),
            "j" => Some(Key::KeyJ),
            "k" => Some(Key::KeyK),
            "l" => Some(Key::KeyL),
            "m" => Some(Key::KeyM),
            "n" => Some(Key::KeyN),
            "o" => Some(Key::KeyO),
            "p" => Some(Key::KeyP),
            "q" => Some(Key::KeyQ),
            "r" => Some(Key::KeyR),
            "s" => Some(Key::KeyS),
            "t" => Some(Key::KeyT),
            "u" => Some(Key::KeyU),
            "v" => Some(Key::KeyV),
            "w" => Some(Key::KeyW),
            "x" => Some(Key::KeyX),
            "y" => Some(Key::KeyY),
            "z" => Some(Key::KeyZ),
            "0" => Some(Key::Num0),
            "1" => Some(Key::Num1),
            "2" => Some(Key::Num2),
            "3" => Some(Key::Num3),
            "4" => Some(Key::Num4),
            "5" => Some(Key::Num5),
            "6" => Some(Key::Num6),
            "7" => Some(Key::Num7),
            "8" => Some(Key::Num8),
            "9" => Some(Key::Num9),
            "enter" | "return" => Some(Key::Return),
            "escape" | "esc" => Some(Key::Escape),
            "backspace" => Some(Key::Backspace),
            "tab" => Some(Key::Tab),
            "space" => Some(Key::Space),
            "shift" | "shiftleft" => Some(Key::ShiftLeft),
            "shiftright" => Some(Key::ShiftRight),
            "control" | "controlleft" | "ctrl" => Some(Key::ControlLeft),
            "controlright" | "ctrlright" => Some(Key::ControlRight),
            "alt" | "altleft" => Some(Key::Alt),
            "altright" => Some(Key::AltGr),
            "meta" | "super" | "metaleft" => Some(Key::MetaLeft),
            "metaright" | "superright" => Some(Key::MetaRight),
            "capslock" => Some(Key::CapsLock),
            "f1" => Some(Key::F1),
            "f2" => Some(Key::F2),
            "f3" => Some(Key::F3),
            "f4" => Some(Key::F4),
            "f5" => Some(Key::F5),
            "f6" => Some(Key::F6),
            "f7" => Some(Key::F7),
            "f8" => Some(Key::F8),
            "f9" => Some(Key::F9),
            "f10" => Some(Key::F10),
            "f11" => Some(Key::F11),
            "f12" => Some(Key::F12),
            "arrowup" | "up" => Some(Key::UpArrow),
            "arrowdown" | "down" => Some(Key::DownArrow),
            "arrowleft" | "left" => Some(Key::LeftArrow),
            "arrowright" | "right" => Some(Key::RightArrow),
            "home" => Some(Key::Home),
            "end" => Some(Key::End),
            "pageup" => Some(Key::PageUp),
            "pagedown" => Some(Key::PageDown),
            "delete" => Some(Key::Delete),
            "insert" => Some(Key::Insert),
            _ => None,
        }
    }
}

impl InputHandler for X11InputHandler {
    fn handle_mouse(
        &self,
        event: MouseEvent,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match event {
            MouseEvent::Move { x, y } => {
                // Store current position
                *self.last_x.lock().unwrap() = x;
                *self.last_y.lock().unwrap() = y;

                // Simulate mouse move
                simulate(&EventType::MouseMove {
                    x: x as f64,
                    y: y as f64,
                })
                .map_err(|e| format!("Failed to move mouse: {:?}", e))?;
            }
            MouseEvent::ButtonDown { button } => {
                let rdev_button = Self::to_rdev_button(button);
                simulate(&EventType::ButtonPress(rdev_button))
                    .map_err(|e| format!("Failed to press button: {:?}", e))?;
            }
            MouseEvent::ButtonUp { button } => {
                let rdev_button = Self::to_rdev_button(button);
                simulate(&EventType::ButtonRelease(rdev_button))
                    .map_err(|e| format!("Failed to release button: {:?}", e))?;
            }
            MouseEvent::Scroll { delta_x, delta_y } => {
                // rdev uses scroll deltas differently
                if delta_y != 0 {
                    simulate(&EventType::Wheel {
                        delta_x: 0,
                        delta_y: delta_y as i64,
                    })
                    .map_err(|e| format!("Failed to scroll: {:?}", e))?;
                }
                if delta_x != 0 {
                    simulate(&EventType::Wheel {
                        delta_x: delta_x as i64,
                        delta_y: 0,
                    })
                    .map_err(|e| format!("Failed to scroll: {:?}", e))?;
                }
            }
        }
        Ok(())
    }

    fn handle_keyboard(
        &self,
        event: KeyEvent,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let key =
            Self::to_rdev_key(&event.key).ok_or_else(|| format!("Unknown key: {}", event.key))?;

        match event {
            KeyEvent::KeyDown { .. } => {
                simulate(&EventType::KeyPress(key))
                    .map_err(|e| format!("Failed to press key: {:?}", e))?;
            }
            KeyEvent::KeyUp { .. } => {
                simulate(&EventType::KeyRelease(key))
                    .map_err(|e| format!("Failed to release key: {:?}", e))?;
            }
        }
        Ok(())
    }
}
