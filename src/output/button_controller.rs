use super::output_const::*;
use std::mem;
use std::sync::Arc;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::RTCDataChannel;
use winapi::um::winuser::*;
use winput::Button;

/// # ButtonController
///
/// The `ButtonController` struct provides functionality for handling keyboard and mouse events
/// via a WebRTC data channel.
pub struct ButtonController {}

impl ButtonController {
    /// Creates a new `ButtonController`.
    pub fn new() -> ButtonController {
        ButtonController {}
    }

    /// Starts the keyboard controller by registering a callback for incoming messages on the
    /// provided WebRTC data channel.
    ///
    /// # Arguments
    ///
    /// * `ch` - An Arc reference to the RTCDataChannel.
    pub fn start_keyboard_controller(ch: Arc<RTCDataChannel>) {
        ch.on_message(Box::new(move |msg: DataChannelMessage| {
            Box::pin(async move {
                let s = String::from_utf8_lossy(&msg.data);
                let (action, rest) = s.split_at(1);

                match action {
                    PRESS_KEYBOARD_ACTION => {
                        let key = match rest.parse::<u8>() {
                            Ok(k) => k,
                            Err(e) => {
                                log::error!("BUTTON CONTROLLER | Error parsing u8: {}", e);
                                return;
                            }
                        };
                        send_input_key(key as i32, false);
                    }
                    RELEASE_KEYBOARD_ACTION => {
                        let key = match rest.parse::<u8>() {
                            Ok(k) => k,
                            Err(e) => {
                                log::error!("BUTTON CONTROLLER | Error parsing u8: {}", e);
                                return;
                            }
                        };
                        send_input_key(key as i32, true);
                    }
                    PRESS_MOUSE_ACTION => {
                        let key = match rest.parse::<u8>() {
                            Ok(k) => k,
                            Err(e) => {
                                log::error!("BUTTON CONTROLLER | Error parsing u8: {}", e);
                                return;
                            }
                        };
                        let button = get_mouse_button(key);
                        winput::press(button);
                    }
                    RELEASE_MOUSE_ACTION => {
                        let key = match rest.parse::<u8>() {
                            Ok(k) => k,
                            Err(e) => {
                                log::error!("BUTTON CONTROLLER | Error parsing u8: {}", e);
                                return;
                            }
                        };
                        let button = get_mouse_button(key);
                        winput::release(button);
                    }
                    SCROLL_HORIZONTAL_ACTION => {
                        let delta = match rest.parse::<f32>() {
                            Ok(k) => k,
                            Err(e) => {
                                log::error!("BUTTON CONTROLLER | Error parsing f32: {}", e);
                                return;
                            }
                        };
                        winput::Mouse::scrollh(delta);
                    }
                    SCROLL_VERTICAL_ACTION => {
                        let delta = match rest.parse::<f32>() {
                            Ok(k) => k,
                            Err(e) => {
                                log::error!("BUTTON CONTROLLER | Error parsing f32: {}", e);
                                return;
                            }
                        };
                        winput::Mouse::scroll(delta)
                    }
                    _ => {}
                }
            })
        }));
    }
}

/// Maps the numeric key value to the corresponding mouse button.
fn get_mouse_button(key: u8) -> Button {
    match key {
        0 => Button::Left,
        1 => Button::Right,
        2 => Button::Middle,
        3 => Button::X1,
        4 => Button::X2,
        _ => Button::Left, //TODO: fix this
    }
}

/// Sends a keyboard input event.
///
/// # Arguments
///
/// * `virtual_key` - The virtual key code.
/// * `up` - Indicates whether the key is being released (true) or pressed (false)
pub fn send_input_key(virtual_key: i32, up: bool) {
    unsafe {
        let mut input = INPUT {
            type_: INPUT_KEYBOARD,
            u: std::mem::zeroed(),
        };
        *input.u.ki_mut() = KEYBDINPUT {
            wVk: virtual_key as u16,
            dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
            dwExtraInfo: 1,
            wScan: KEYEVENTF_SCANCODE as u16,
            time: 0,
        };

        SendInput(1, &mut input, mem::size_of::<INPUT>() as i32);
    }
}

impl Default for ButtonController {
    fn default() -> Self {
        Self::new()
    }
}
